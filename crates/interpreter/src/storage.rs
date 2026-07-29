use crate::value::{Value, ContentHash, HashAlgorithm, Commit, CaidVersion};
use std::fs;
use std::path::{Path, PathBuf};
use anyhow::Result;
use sha2::Digest;

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

/// On-disk layout version for `.oo/` (local_gc arc). Objects are self-describing
/// via CAID `v1`/`v2`; this is the **layout** marker.
pub const STORE_FORMAT_VERSION: u32 = 1;

pub struct ObjectStore {
    root: PathBuf,
}

impl ObjectStore {
    /// Ensure `.oo/format` is present and understood. Absent → write `1`.
    /// Unknown version → refuse (do not read/write/GC the store).
    pub fn ensure_format(base_dir: &Path) -> Result<()> {
        let oo = base_dir.join(".oo");
        if !oo.exists() {
            return Ok(());
        }
        let path = oo.join("format");
        if path.exists() {
            let raw = fs::read_to_string(&path)?;
            let v = raw.trim();
            if v != STORE_FORMAT_VERSION.to_string() {
                anyhow::bail!(
                    "store format version {v} is not supported by this engine \
                     (understands format {STORE_FORMAT_VERSION}); refusing to open"
                );
            }
        } else {
            // Every existing store is version 1; announce rather than refuse.
            fs::write(&path, format!("{STORE_FORMAT_VERSION}\n"))?;
        }
        Ok(())
    }

    pub fn init(base_dir: &Path) -> Result<Self> {
        Self::ensure_format(base_dir)?;
        let root = base_dir.join(".oo").join("objects");
        if !root.exists() {
            fs::create_dir_all(&root)?;
            // New store: ensure format after creating .oo/
            Self::ensure_format(base_dir)?;
        }
        Ok(Self { root })
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
        let hash = value.content_hash();
        let content = serde_json::to_string_pretty(value)?;
        self.write_object(&hash, content)?;
        Ok(hash)
    }

    pub fn get_value(&self, hash: &ContentHash) -> Result<Value> {
        let content = self.read_object_raw(hash)?;
        let value: Value = serde_json::from_str(&content).map_err(|e| {
            StoreReadError::ObjectUndecodable {
                requested: hash.clone(),
                detail: e.to_string(),
            }
        })?;
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

    pub fn put_commit(&self, commit: &Commit) -> Result<ContentHash> {
        let hash = commit.content_hash();
        let content = serde_json::to_string_pretty(commit)?;
        self.write_object(&hash, content)?;
        Ok(hash)
    }

    pub fn get_commit(&self, hash: &ContentHash) -> Result<Commit> {
        let content = self.read_object_raw(hash)?;
        let commit: Commit = serde_json::from_str(&content).map_err(|e| {
            StoreReadError::ObjectUndecodable {
                requested: hash.clone(),
                detail: e.to_string(),
            }
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
        if !head_path.exists() { return Ok(None); }
        let s = fs::read_to_string(head_path)?;
        ContentHash::parse(&s.trim()).map(Some).map_err(|e| anyhow::anyhow!("{:?}", e))
    }

    pub fn set_head(&self, base_dir: &Path, hash: &ContentHash) -> Result<()> {
        let oo_dir = base_dir.join(".oo");
        if !oo_dir.exists() { fs::create_dir_all(&oo_dir)?; }
        let head_path = oo_dir.join("HEAD");
        fs::write(head_path, hash.to_string())?;
        Ok(())
    }

    pub fn get_horizon_salt(&self) -> ContentHash {
        let mut hasher = sha2::Sha256::new();
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        hasher.update(now.to_le_bytes());
        ContentHash::v1(sha2::Digest::finalize(hasher).to_vec())
    }

    fn write_object(&self, hash: &ContentHash, content: String) -> Result<()> {
        let path = self.hash_to_path(hash);
        if !path.exists() {
            if let Some(parent) = path.parent() { fs::create_dir_all(parent)?; }
            fs::write(&path, content)?;
        }
        Ok(())
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
        std::fs::write(path, json)?;
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
        let algo_dir = match hash.algorithm { HashAlgorithm::Sha256 => "sha256" };
        let hex = hex::encode(&hash.digest);
        self.root.join(algo_dir).join(&hex[0..2]).join(&hex[2..])
    }
}
