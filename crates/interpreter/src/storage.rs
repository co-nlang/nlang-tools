use crate::value::{CaidVersion, ComboVal, Commit, ContentHash, EffectTag, HashAlgorithm, Value};
use crate::StandardRootSet;
use anyhow::Result;
use serde::Serialize;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Write `contents` so a concurrent reader never sees a truncated (or briefly
/// absent) target. Same-directory temp + `fsync` + `rename`. The temp is
/// removed on any failure; success leaves no sidecar under the parent.
///
/// Atomicity of `rename` holds only within one filesystem — the temp is
/// created in the target's parent for that reason (atomic_writes arc).
pub fn atomic_write(path: &Path, contents: impl AsRef<[u8]>) -> Result<()> {
    let contents = contents.as_ref();
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;

    // Prefix avoids the letters "tmp" so a leaked file cannot pass for an
    // object shard name under P1's leftover scan — and leading-dot keeps it
    // out of casual directory listings.
    let mut tmp = tempfile::Builder::new()
        .prefix(".partial-")
        .tempfile_in(parent)
        .map_err(|e| anyhow::anyhow!("atomic_write temp create {}: {e}", parent.display()))?;
    tmp.write_all(contents)
        .map_err(|e| anyhow::anyhow!("atomic_write write {}: {e}", path.display()))?;
    tmp.as_file()
        .sync_all()
        .map_err(|e| anyhow::anyhow!("atomic_write fsync {}: {e}", path.display()))?;

    // persist = rename over the target; on failure the TempPath still deletes
    // the temp when dropped, so nothing is left for a directory walk to find.
    tmp.persist(path)
        .map_err(|e| anyhow::anyhow!("atomic_write install {}: {}", path.display(), e.error))?;
    Ok(())
}

/// Distinct CAS read outcomes (SPEC_08 / REAL_03 §8; cas_integrity arc).
/// Callers must not collapse these into one "not found" string.
#[derive(Debug, Clone)]
pub enum StoreReadError {
    /// No object at the digest-keyed path.
    NotFound { requested: ContentHash },
    /// Object present and decoded, but recomputed address ≠ requested
    /// (`#caid_mismatch` — the bytes are lying).
    CaidMismatch {
        requested: ContentHash,
        recomputed: ContentHash,
    },
    /// Object present but cannot be deserialized; integrity unknown
    /// (`#object_undecodable`).
    ObjectUndecodable {
        requested: ContentHash,
        detail: String,
    },
    /// Object is held, but its format-3 standard root is not shipped by this
    /// engine, so it cannot be opened.
    StandardRootUnavailable {
        requested: ContentHash,
        standard_digest: String,
    },
}

impl std::fmt::Display for StoreReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreReadError::NotFound { requested } => {
                write!(f, "CAID not found in local store: {requested}")
            }
            StoreReadError::CaidMismatch {
                requested,
                recomputed,
            } => write!(
                f,
                "#caid_mismatch: object at digest path is corrupt (integrity failure); \
                 requested {requested}, recomputed {recomputed}"
            ),
            StoreReadError::ObjectUndecodable { requested, detail } => write!(
                f,
                "#object_undecodable: object present for {requested} but cannot be decoded \
                 (integrity unknown): {detail}"
            ),
            StoreReadError::StandardRootUnavailable {
                requested,
                standard_digest,
            } => write!(
                f,
                "refusing root: standard root digest {standard_digest} is unavailable for {requested}"
            ),
        }
    }
}

impl std::error::Error for StoreReadError {}

/// Full v2 CAID match for values (digest + lattice_sketch + masa_ref).
/// v1 requests compare digest only (REAL_03 §9.2 digest-only door).
/// Shared by local store reads and peer-fetch (REAL_03 §6.6) — one comparator.
pub fn value_address_matches(requested: &ContentHash, recomputed: &ContentHash) -> bool {
    if requested.digest != recomputed.digest {
        return false;
    }
    match requested.version {
        CaidVersion::V1 => true,
        CaidVersion::V2 => {
            recomputed.version == CaidVersion::V2
                && requested.masa_ref == recomputed.masa_ref
                && requested.lattice_sketch == recomputed.lattice_sketch
        }
    }
}

fn commit_address_matches(requested: &ContentHash, recomputed: &ContentHash) -> bool {
    // Commits are v1 by construction (`Commit::content_hash` → ContentHash::v1).
    requested.digest == recomputed.digest
}

/// The `.oo/` layout and the CAS encoding are independent declarations.
/// Legacy stores used one bare number for both; new stores name each axis.
pub const STORE_LAYOUT_VERSION: u32 = 2;
pub const OBJECT_ENCODING_VERSION: u32 = 4;
const MIN_READABLE_STORE_FORMAT_VERSION: u32 = 1;

pub struct ObjectStore {
    root: PathBuf,
    encoding: u32,
}

fn ensure_supported_encoding(v: u32) -> Result<()> {
    if (MIN_READABLE_STORE_FORMAT_VERSION..=OBJECT_ENCODING_VERSION).contains(&v) {
        Ok(())
    } else {
        anyhow::bail!(
            "object encoding version {v} is not supported by this engine \
             (understands encoding {MIN_READABLE_STORE_FORMAT_VERSION} through {OBJECT_ENCODING_VERSION}); refusing to open"
        )
    }
}

fn has_cas_objects(path: &Path) -> bool {
    let Ok(entries) = fs::read_dir(path) else { return false };
    entries.flatten().any(|entry| {
        let path = entry.path();
        path.is_file() || (path.is_dir() && has_cas_objects(&path))
    })
}

impl ObjectStore {
    pub fn encoding_version(&self) -> u32 {
        self.encoding
    }

    fn declared_encoding(base_dir: &Path) -> Result<u32> {
        let oo = base_dir.join(".oo");
        let raw = fs::read_to_string(oo.join("format")).map_err(|_| anyhow::anyhow!(
            "cannot determine this store's layout: `.oo/format` is absent; refusing to open"
        ))?;
        let declaration = raw.trim();
        if declaration == format!("layout={STORE_LAYOUT_VERSION}") {
            let objects = fs::read_to_string(oo.join("objects.format")).map_err(|_| anyhow::anyhow!(
                "cannot determine this store's object encoding: `.oo/objects.format` is absent; refusing to open"
            ))?;
            objects.trim().strip_prefix("encoding=").and_then(|v| v.parse::<u32>().ok()).ok_or_else(|| {
                anyhow::anyhow!("object encoding declaration {:?} is not supported", objects.trim())
            })
        } else if let Ok(v) = declaration.parse::<u32>() {
            Ok(v)
        } else {
            anyhow::bail!("store layout declaration {declaration:?} is not supported")
        }
    }

    /// Verify declarations without writing them. An absent declaration carries
    /// no trustworthy information about a store the engine did not create.
    pub fn ensure_format(base_dir: &Path) -> Result<()> {
        let oo = base_dir.join(".oo");
        if !oo.exists() {
            return Ok(());
        }
        let layout = oo.join("format");
        let raw = fs::read_to_string(&layout).map_err(|_| anyhow::anyhow!(
            "cannot determine this store's layout: `.oo/format` is absent; refusing to open"
        ))?;
        let declaration = raw.trim();
        if declaration == format!("layout={STORE_LAYOUT_VERSION}") {
            let objects = fs::read_to_string(oo.join("objects.format")).map_err(|_| anyhow::anyhow!(
                "cannot determine this store's object encoding: `.oo/objects.format` is absent; refusing to open"
            ))?;
            let Some(v) = objects.trim().strip_prefix("encoding=").and_then(|v| v.parse::<u32>().ok()) else {
                anyhow::bail!("object encoding declaration {:?} is not supported; refusing to open", objects.trim());
            };
            ensure_supported_encoding(v)?;
        } else if let Ok(v) = declaration.parse::<u32>() {
            // Legacy conflated declaration: layout 1, encoding N. Reading it
            // is compatible and intentionally non-mutating.
            ensure_supported_encoding(v)?;
        } else {
            anyhow::bail!("store layout declaration {declaration:?} is not supported; refusing to open");
        }
        Ok(())
    }

    pub fn init(base_dir: &Path) -> Result<Self> {
        let oo = base_dir.join(".oo");
        // An empty `.oo/` is often a home for pre-store configuration such as
        // discovery.n. It contains no durable object whose shape could be
        // misread, so it is safe to initialise. HEAD or a CAS file makes it a
        // store someone may already have written and therefore needs a proven
        // declaration before we open it.
        let new_store = !oo.join("HEAD").exists() && !has_cas_objects(&oo.join("objects"));
        if new_store {
            fs::create_dir_all(&oo)?;
            atomic_write(&oo.join("format"), format!("layout={STORE_LAYOUT_VERSION}\n"))?;
            atomic_write(&oo.join("objects.format"), format!("encoding={OBJECT_ENCODING_VERSION}\n"))?;
        } else {
            Self::ensure_format(base_dir)?;
        }
        let encoding = if new_store {
            OBJECT_ENCODING_VERSION
        } else {
            Self::declared_encoding(base_dir)?
        };
        let root = oo.join("objects");
        if !root.exists() {
            fs::create_dir_all(&root)?;
        }
        Ok(Self { root, encoding })
    }

    /// Digest path for an object (sha256/ab/cdef…).
    pub fn digest_path(&self, digest_hex: &str) -> PathBuf {
        let algo_dir = "sha256";
        self.root
            .join(algo_dir)
            .join(&digest_hex[0..2])
            .join(&digest_hex[2..])
    }

    pub fn object_exists_digest(&self, digest_hex: &str) -> bool {
        if digest_hex.len() < 4 {
            return false;
        }
        self.digest_path(digest_hex).exists()
    }

    /// List every object digest (64 hex) under the store.
    pub fn list_digests(&self) -> Result<Vec<(String, u64)>> {
        let mut out = Vec::new();
        let sha = self.root.join("sha256");
        if !sha.exists() {
            return Ok(out);
        }
        for a in fs::read_dir(&sha)? {
            let a = a?;
            if !a.path().is_dir() {
                continue;
            }
            let pre = a.file_name().to_string_lossy().to_string();
            for b in fs::read_dir(a.path())? {
                let b = b?;
                if !b.path().is_file() {
                    continue;
                }
                let rest = b.file_name().to_string_lossy().to_string();
                let len = b.metadata()?.len();
                out.push((format!("{pre}{rest}"), len));
            }
        }
        Ok(out)
    }

    pub fn remove_digest(&self, digest_hex: &str) -> Result<()> {
        let p = self.digest_path(digest_hex);
        if p.exists() {
            fs::remove_file(&p)?;
        }
        // Empty two-hex-digit directory.
        if let Some(parent) = p.parent() {
            if parent.exists() && fs::read_dir(parent)?.next().is_none() {
                let _ = fs::remove_dir(parent);
            }
        }
        Ok(())
    }

    pub fn read_raw_digest(&self, digest_hex: &str) -> Result<Vec<u8>> {
        let p = self.digest_path(digest_hex);
        if !p.exists() {
            anyhow::bail!("not found");
        }
        Ok(fs::read(p)?)
    }

    pub fn put_value(&self, value: &Value) -> Result<ContentHash> {
        let durable = if self.encoding < 4 {
            value.for_legacy_cas_storage()
        } else {
            value.for_cas_storage()
        };
        let hash = durable.content_hash();
        let content = canonical_cas_json(&durable)?;
        self.write_object(&hash, content)?;
        Ok(hash)
    }

    /// Persist a universe root. The standard-library axis is engine-owned and
    /// immutable for one engine build, so history names that table by content
    /// digest rather than copying its body into every root.
    pub fn put_root(&self, root: &ComboVal, standard: &ComboVal) -> Result<ContentHash> {
        if self.encoding < 4 {
            // Encoding 3 named roots by their hydrated body.  The container
            // declaration, not the standard-root table, selects this rule.
            let mut logical_root = root.clone();
            hydrate_standard_root(&mut logical_root, standard);
            let value = Value::Combo(logical_root);
            let hash = value.content_hash();
            let mut durable = value.for_legacy_cas_storage();
            let Value::Combo(ref mut durable_root) = durable else { unreachable!() };
            project_standard_root(durable_root, standard);
            self.write_object(&hash, encode_readable_cas_json(&durable)?)?;
            return Ok(hash);
        }
        // The named table is a first-class CAS object.  Its digest is the
        // sentinel's dependency and it must be fetchable on a fresh store.
        let standard = Value::Combo(standard.clone()).for_cas_storage();
        let standard_hash = standard.content_hash();
        // A table is a real CAS Combo, stored without the root Value envelope.
        // `get_value` accepts this representation under the same CAID.
        let Value::Combo(standard_combo) = &standard else { unreachable!() };
        let raw_combo = canonical_cas_json(standard_combo)?;
        let packed = format!("standard-root:{}", hex::encode(raw_combo));
        self.write_object(&standard_hash, canonical_cas_json(&packed)?)?;

        let value = Value::Combo(root.clone());
        let mut durable = value.for_cas_storage();
        let Value::Combo(ref mut durable_root) = durable else { unreachable!() };
        project_standard_root(durable_root, match &standard { Value::Combo(root) => root, _ => unreachable!() });
        let hash = durable.content_hash();
        let content = encode_readable_cas_json(&durable)?;
        self.write_object(&hash, content)?;
        Ok(hash)
    }

    /// Decode a universe root and resolve its format-3 standard-library
    /// dependency against this engine's table. A digest mismatch is a refusal,
    /// not a best-effort substitution with today's builtins.
    pub fn get_root(&self, hash: &ContentHash, standards: &StandardRootSet) -> Result<ComboVal> {
        let content = self.read_object_raw(hash)?;
        let value: Value = serde_json::from_str(&content).map_err(|e| StoreReadError::ObjectUndecodable {
            requested: hash.clone(), detail: e.to_string(),
        })?;
        let has_standard = match &value {
            Value::Combo(root) => root.system.contains_key(SYSTEM_DIGEST_KEY),
            _ => anyhow::bail!("Invalid root"),
        };
        // Formats 1 and 2 stored their roots self-contained. Only a format-3
        // root carries the digest sentinel that authorises engine-owned table
        // hydration (and therefore standard-root validation).
        if has_standard {
            let Value::Combo(root) = &value else { unreachable!() };
            resolve_standard_root(root, standards, hash)?;
        }
        if self.encoding < 4 && has_standard {
            let mut hydrated = value.clone();
            let Value::Combo(ref mut hydrated_root) = hydrated else { unreachable!() };
            let standard = resolve_standard_root(hydrated_root, standards, hash)?.clone();
            // Encoding 3's logical value had every nested system table
            // hydrated before its address was computed.  Restoring only the
            // top-level standard coordinates is not the historical rule.
            hydrate_system_table(hydrated_root, standards, hash)?;
            hydrate_standard_root(hydrated_root, &standard);
            let recomputed = hydrated.content_hash();
            if !value_address_matches(hash, &recomputed) {
                return Err(StoreReadError::CaidMismatch { requested: hash.clone(), recomputed }.into());
            }
            let Value::Combo(root) = hydrated else { unreachable!() };
            return Ok(root);
        }
        let recomputed = value.content_hash();
        if !value_address_matches(hash, &recomputed) {
            return Err(StoreReadError::CaidMismatch { requested: hash.clone(), recomputed }.into());
        }
        let Value::Combo(root) = value else { unreachable!() };
        Ok(root)
    }

    pub fn get_value(&self, hash: &ContentHash) -> Result<Value> {
        let content = self.read_object_raw(hash)?;
        let mut value: Value = match serde_json::from_str(&content) {
            Ok(value) => value,
            Err(value_error) => match serde_json::from_str::<String>(&content)
                .ok()
                .and_then(|packed| packed.strip_prefix("standard-root:").map(str::to_owned))
                .and_then(|hex| hex::decode(hex).ok())
                .and_then(|raw| serde_json::from_slice::<ComboVal>(&raw).ok())
            {
                Some(combo) => Value::Combo(combo),
                None => return Err(StoreReadError::ObjectUndecodable {
                    requested: hash.clone(),
                    detail: value_error.to_string(),
                }.into()),
            },
        };
        if let Value::Combo(root) = &mut value {
            if root.system.contains_key(SYSTEM_DIGEST_KEY) {
                // All ordinary readers use one decoder. A format-3 root has
                // no valid standalone Value interpretation: resolve it before
                // judging its address, rather than making every caller guess.
                let engine = crate::Ouroboros::new_in_memory();
                resolve_standard_root(root, &engine.standard_roots, hash)?;
            }
        }
        if self.encoding < 4 {
            if let Value::Combo(root) = &mut value {
                if root.system.contains_key(SYSTEM_DIGEST_KEY) {
                    let engine = crate::Ouroboros::new_in_memory();
                    let standard =
                        resolve_standard_root(root, &engine.standard_roots, hash)?.clone();
                    hydrate_system_table(root, &engine.standard_roots, hash)?;
                    hydrate_standard_root(root, &standard);
                    let recomputed = value.content_hash();
                    if !value_address_matches(hash, &recomputed) {
                        return Err(StoreReadError::CaidMismatch { requested: hash.clone(), recomputed }.into());
                    }
                    return Ok(value);
                }
            }
        }
        let recomputed = value.content_hash();
        if !value_address_matches(hash, &recomputed) {
            return Err(StoreReadError::CaidMismatch {
                requested: hash.clone(),
                recomputed,
            }
            .into());
        }
        Ok(value)
    }

    /// Read the root's declared standard-library dependency without resolving
    /// it. Used by `oo status` so availability is observable before a load.
    pub fn root_standard_digest(&self, hash: &ContentHash) -> Result<Option<String>> {
        let content = self.read_object_raw(hash)?;
        let value: Value = serde_json::from_str(&content).map_err(|e| StoreReadError::ObjectUndecodable {
            requested: hash.clone(), detail: e.to_string(),
        })?;
        let Value::Combo(root) = value else { anyhow::bail!("Invalid root") };
        Ok(match root.system.get(SYSTEM_DIGEST_KEY) {
            Some(Value::Atom(nlang_parser::ast::AtomKind::Str(digest), _, _)) => Some(digest.clone()),
            _ => None,
        })
    }

    pub fn put_commit(&self, commit: &Commit) -> Result<ContentHash> {
        let hash = commit.content_hash();
        let content = canonical_cas_json(commit)?;
        self.write_object(&hash, content)?;
        Ok(hash)
    }

    pub fn get_commit(&self, hash: &ContentHash) -> Result<Commit> {
        let content = self.read_object_raw(hash)?;
        let commit: Commit =
            serde_json::from_str(&content).map_err(|e| StoreReadError::ObjectUndecodable {
                requested: hash.clone(),
                detail: e.to_string(),
            })?;
        let recomputed = commit.content_hash();
        if !commit_address_matches(hash, &recomputed) {
            return Err(StoreReadError::CaidMismatch {
                requested: hash.clone(),
                recomputed,
            }
            .into());
        }
        Ok(commit)
    }

    pub fn get_head(&self, base_dir: &Path) -> Result<Option<ContentHash>> {
        let head_path = base_dir.join(".oo").join("HEAD");
        if !head_path.exists() {
            return Ok(None);
        }
        let s = fs::read_to_string(head_path)?;
        ContentHash::parse(&s.trim())
            .map(Some)
            .map_err(|e| anyhow::anyhow!("{:?}", e))
    }

    pub fn set_head(&self, base_dir: &Path, hash: &ContentHash) -> Result<()> {
        let oo_dir = base_dir.join(".oo");
        if !oo_dir.exists() {
            fs::create_dir_all(&oo_dir)?;
        }
        let head_path = oo_dir.join("HEAD");
        atomic_write(&head_path, hash.to_string())?;
        Ok(())
    }

    // O42 R-1: get_horizon_salt removed — clock salt is forbidden in blur CAID.

    fn write_object(&self, hash: &ContentHash, content: String) -> Result<()> {
        let path = self.hash_to_path(hash);
        // Content-addressed: same bytes → same path. Skip when already present
        // (idempotent). First install is temp+rename so a concurrent reader
        // never sees a truncated object, and two racing first-writers both
        // rename the same payload (no TOCTOU "check then write" gap).
        if path.exists() {
            return Ok(());
        }
        self.upgrade_format_for_cas_write()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        atomic_write(&path, content)?;
        Ok(())
    }

    /// A write may declare the encoding it is about to install. Legacy stores
    /// are migrated here, never by a read path.
    fn upgrade_format_for_cas_write(&self) -> Result<()> {
        let oo = self
            .root
            .parent()
            .ok_or_else(|| anyhow::anyhow!("object store has no .oo parent"))?;
        Self::ensure_format(oo.parent().ok_or_else(|| anyhow::anyhow!("object store has no base directory"))?)?;
        let layout = fs::read_to_string(oo.join("format"))?;
        if layout.trim() != format!("layout={STORE_LAYOUT_VERSION}") {
            atomic_write(&oo.join("format"), format!("layout={STORE_LAYOUT_VERSION}\n"))?;
        }
        atomic_write(&oo.join("objects.format"), format!("encoding={}\n", self.encoding))
    }

    /// Read raw bytes at the digest path. Absence → `NotFound` (not IO prose).
    fn read_object_raw(&self, hash: &ContentHash) -> Result<String> {
        let path = self.hash_to_path(hash);
        if !path.exists() {
            return Err(StoreReadError::NotFound {
                requested: hash.clone(),
            }
            .into());
        }
        fs::read_to_string(path).map_err(|e| {
            StoreReadError::ObjectUndecodable {
                requested: hash.clone(),
                detail: format!("read failed: {e}"),
            }
            .into()
        })
    }

    pub fn save_architects(
        &self,
        base_dir: &Path,
        architects: &std::collections::HashSet<String>,
    ) -> anyhow::Result<()> {
        let dir = base_dir.join(".oo");
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("architects.json");
        let list: Vec<&String> = architects.iter().collect();
        let json = serde_json::to_string(&list)?;
        atomic_write(&path, json)?;
        Ok(())
    }

    pub fn load_architects(
        &self,
        base_dir: &Path,
    ) -> anyhow::Result<std::collections::HashSet<String>> {
        let path = base_dir.join(".oo").join("architects.json");
        if !path.exists() {
            return Ok(std::collections::HashSet::new());
        }
        let json = std::fs::read_to_string(path)?;
        let list: Vec<String> = serde_json::from_str(&json)?;
        Ok(list.into_iter().collect())
    }

    fn hash_to_path(&self, hash: &ContentHash) -> PathBuf {
        let algo_dir = match hash.algorithm {
            HashAlgorithm::Sha256 => "sha256",
        };
        let hex = hex::encode(&hash.digest);
        self.root.join(algo_dir).join(&hex[0..2]).join(&hex[2..])
    }
}

fn standard_table_digest(standard: &ComboVal) -> String {
    hex::encode(Value::Combo(standard.clone()).content_hash().digest)
}

const SYSTEM_DIGEST_KEY: &str = "__nlang_system_digest";

fn project_standard_root(root: &mut ComboVal, standard: &ComboVal) {
    for (actual, base) in [
        (&mut root.data, &standard.data), (&mut root.types, &standard.types),
        (&mut root.rules, &standard.rules), (&mut root.meta, &standard.meta),
        (&mut root.system, &standard.system), (&mut root.local, &standard.local),
    ] {
        actual.retain(|key, value| base.get(key) != Some(value));
    }
    root.system.clear();
    root.system.insert(SYSTEM_DIGEST_KEY.to_string(), Value::Atom(
        nlang_parser::ast::AtomKind::Str(standard_table_digest(standard)), EffectTag::Pure, None,
    ));
}

fn hydrate_standard_root(root: &mut ComboVal, standard: &ComboVal) {
    for (actual, base) in [
        (&mut root.data, &standard.data),
        (&mut root.types, &standard.types),
        (&mut root.rules, &standard.rules),
        (&mut root.meta, &standard.meta),
        (&mut root.system, &standard.system),
        (&mut root.local, &standard.local),
    ] {
        for (key, value) in base {
            actual.entry(key.clone()).or_insert_with(|| value.clone());
        }
    }
}

fn resolve_standard_root<'a>(
    combo: &ComboVal,
    standards: &'a StandardRootSet,
    requested: &ContentHash,
) -> Result<&'a ComboVal> {
    let Some(Value::Atom(nlang_parser::ast::AtomKind::Str(digest), _, _)) = combo.system.get(SYSTEM_DIGEST_KEY) else {
        anyhow::bail!("root does not name a standard root");
    };
    standards.get(digest).ok_or_else(|| {
        StoreReadError::StandardRootUnavailable {
            requested: requested.clone(),
            standard_digest: digest.clone(),
        }
        .into()
    })
}

/// Encoding 3 represented every Combo system table by a digest sentinel on
/// disk but computed the address from the recursively hydrated logical value.
/// This is deliberately selected by the container gate, never by which
/// standard-root digest happens to be named.
fn hydrate_system_table(
    combo: &mut ComboVal,
    standards: &StandardRootSet,
    requested: &ContentHash,
) -> Result<()> {
    if combo.system.contains_key(SYSTEM_DIGEST_KEY) {
        let standard = resolve_standard_root(combo, standards, requested)?;
        combo.system = standard.system.clone();
    }
    for value in combo
        .data
        .values_mut()
        .chain(combo.types.values_mut())
        .chain(combo.rules.values_mut())
        .chain(combo.meta.values_mut())
        .chain(combo.system.values_mut())
        .chain(combo.local.values_mut())
        .chain(combo.pending_spreads.iter_mut())
    {
        hydrate_systems_in_value(value, standards, requested)?;
    }
    Ok(())
}

fn hydrate_systems_in_value(
    value: &mut Value,
    standards: &StandardRootSet,
    requested: &ContentHash,
) -> Result<()> {
    match value {
        Value::Combo(combo) => hydrate_system_table(combo, standards, requested),
        Value::Union(values) => {
            for value in values {
                hydrate_systems_in_value(value, standards, requested)?;
            }
            Ok(())
        }
        Value::Thunk {
            closure, context, ..
        } => {
            for frame in closure {
                hydrate_system_table(std::sync::Arc::make_mut(frame), standards, requested)?;
            }
            if let Some(context) = context {
                hydrate_systems_in_value(context, standards, requested)?;
            }
            Ok(())
        }
        Value::Range { start, end, step } => {
            hydrate_systems_in_value(start, standards, requested)?;
            hydrate_systems_in_value(end, standards, requested)?;
            if let Some(step) = step {
                hydrate_systems_in_value(step, standards, requested)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// Compact, lexically ordered CAS JSON. The typed format-2 `Value` projection
/// is made before this function; `.oo/staged` never uses it. `serde_json`
/// emits its default object map in lexical order, pinned by Q-010a P4.
fn canonical_cas_json<T: Serialize>(value: &T) -> Result<String> {
    Ok(serde_json::to_string(&serde_json::to_value(value)?)?)
}

/// Encode a universe root and refuse the write if this engine cannot decode
/// the bytes (REAL_03 §6.6 family). Self-nested `<<_.>>` trees fail here
/// instead of reporting Commit successful and bricking the store.
fn encode_readable_cas_json(value: &Value) -> Result<String> {
    let content = canonical_cas_json(value)?;
    serde_json::from_str::<Value>(&content).map_err(|e| {
        anyhow::anyhow!("refusing to store a root this engine cannot read back: {e}")
    })?;
    Ok(content)
}
