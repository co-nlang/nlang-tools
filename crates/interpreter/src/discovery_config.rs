//! Workspace-local affiliation trust roots (discovery_trust arc / #3c-b2 ②).
//!
//! File: `<workspace>/.oo/discovery.n` — closed **data**, never evaluated as a
//! program. Field `affiliation_roots` is a list of 64-lowercase-hex Ed25519
//! public keys. Absence and `affiliation_roots: []` are both an empty set;
//! malformed / unreadable is a named error (never silently empty).

use nlang_parser::ast::{AtomKind, ExprKind, FieldKey, PathAnchor};
use nlang_parser::parse_program;
use std::collections::BTreeSet;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

pub const CONFIG_FILE: &str = "discovery.n";
pub const FIELD_NAME: &str = "affiliation_roots";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiscoveryConfig {
    pub affiliation_roots: BTreeSet<String>,
}

impl DiscoveryConfig {
    pub fn path(base_dir: &Path) -> PathBuf {
        base_dir.join(".oo").join(CONFIG_FILE)
    }

    /// Load from workspace. Missing file → empty set. Present but bad → Err.
    pub fn load(base_dir: &Path) -> anyhow::Result<Self> {
        let path = Self::path(base_dir);
        let entry_meta = match fs::symlink_metadata(&path) {
            Ok(meta) => meta,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "discovery.n: cannot inspect {}: {e}",
                    path.display()
                ));
            }
        };
        let meta = if entry_meta.file_type().is_symlink() {
            fs::metadata(&path)
                .map_err(|e| anyhow::anyhow!("discovery.n: cannot read {}: {e}", path.display()))?
        } else {
            entry_meta
        };
        if !meta.is_file() {
            anyhow::bail!(
                "discovery.n: path is not a regular file: {}",
                path.display()
            );
        }
        let text = fs::read_to_string(&path)
            .map_err(|e| anyhow::anyhow!("discovery.n: cannot read {}: {e}", path.display()))?;
        parse_config_text(&text, &path)
    }

    /// Canonical rewrite: one key per line, sorted. Atomic via
    /// [`crate::storage::atomic_write`] (unique temp name — not `discovery.n.tmp`).
    pub fn write(&self, base_dir: &Path) -> anyhow::Result<()> {
        let path = Self::path(base_dir);
        let body = self.to_nlang();
        crate::storage::atomic_write(&path, body)
            .map_err(|e| anyhow::anyhow!("discovery.n: cannot write {}: {e}", path.display()))
    }

    pub fn to_nlang(&self) -> String {
        if self.affiliation_roots.is_empty() {
            return format!("{FIELD_NAME}: []\n");
        }
        let mut s = format!("{FIELD_NAME}: [\n");
        for k in &self.affiliation_roots {
            s.push_str(&format!("    \"{k}\",\n"));
        }
        // Trailing comma is fine in n/; keep one key per line, sorted.
        s.push_str("]\n");
        s
    }

    pub fn add(&mut self, key: &str) -> anyhow::Result<bool> {
        validate_operator_key(key)?;
        Ok(self.affiliation_roots.insert(key.to_string()))
    }

    pub fn remove(&mut self, key: &str) -> anyhow::Result<bool> {
        validate_operator_key(key)?;
        Ok(self.affiliation_roots.remove(key))
    }
}

/// Exactly 64 lowercase ASCII hex characters.
pub fn validate_operator_key(key: &str) -> anyhow::Result<()> {
    if key.len() != 64 {
        anyhow::bail!(
            "affiliation root key must be exactly 64 lowercase hex characters (got length {})",
            key.len()
        );
    }
    if !key.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
        if key.bytes().any(|b| matches!(b, b'A'..=b'F')) {
            anyhow::bail!(
                "affiliation root key must be lowercase hex (uppercase rejected; got non-lowercase)"
            );
        }
        anyhow::bail!("affiliation root key must be 64 lowercase hex characters");
    }
    Ok(())
}

fn parse_config_text(text: &str, path: &Path) -> anyhow::Result<DiscoveryConfig> {
    let program = parse_program(text)
        .map_err(|e| anyhow::anyhow!("discovery.n: parse error in {}: {e}", path.display()))?;

    let mut found_roots = false;
    let mut roots = BTreeSet::new();

    for field in &program.fields {
        let name = match &field.key {
            FieldKey::Path(p) if p.anchor == PathAnchor::Bare && p.segments.len() == 1 => {
                &p.segments[0]
            }
            _ => {
                anyhow::bail!(
                    "discovery.n: unknown or non-literal field key in {}",
                    path.display()
                );
            }
        };
        if name != FIELD_NAME {
            anyhow::bail!(
                "discovery.n: unknown field `{name}` in {} (only `{FIELD_NAME}` is allowed)",
                path.display()
            );
        }
        if found_roots {
            anyhow::bail!(
                "discovery.n: duplicate field `{FIELD_NAME}` in {}",
                path.display()
            );
        }
        found_roots = true;
        roots = parse_roots_list(&field.value.kind, path)?;
    }

    if !found_roots {
        anyhow::bail!(
            "discovery.n: missing required field `{FIELD_NAME}` in {}",
            path.display()
        );
    }

    Ok(DiscoveryConfig {
        affiliation_roots: roots,
    })
}

fn parse_roots_list(kind: &ExprKind, path: &Path) -> anyhow::Result<BTreeSet<String>> {
    let items = match kind {
        ExprKind::List(items) => items,
        _ => {
            anyhow::bail!(
                "discovery.n: `{FIELD_NAME}` must be a list of strings in {} (got non-list)",
                path.display()
            );
        }
    };
    let mut roots = BTreeSet::new();
    for item in items {
        let s = match &item.kind {
            ExprKind::Atom(AtomKind::Str(s)) => s.clone(),
            ExprKind::Atom(_) => {
                anyhow::bail!(
                    "discovery.n: `{FIELD_NAME}` members must be string literals in {}",
                    path.display()
                );
            }
            _ => {
                anyhow::bail!(
                    "discovery.n: `{FIELD_NAME}` members must be string literals (no morphisms/paths) in {}",
                    path.display()
                );
            }
        };
        validate_operator_key(&s)
            .map_err(|e| anyhow::anyhow!("discovery.n: invalid key in {}: {e}", path.display()))?;
        roots.insert(s);
    }
    Ok(roots)
}
