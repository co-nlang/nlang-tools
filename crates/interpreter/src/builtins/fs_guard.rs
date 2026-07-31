//! Language-layer store trust boundary (SPEC_08 §6.3).
//!
//! A path handed to a filesystem-touching builtin is refused iff, after
//! resolving `.` / `..` / symlinks, **any component equals exactly `.oo`**.
//! Unconditional: no capability unlocks it. Engine paths via ObjectStore /
//! Universe are unaffected — they never call this helper.

use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::value::{BottomCause, BottomDetail, Value};

/// True when `raw`, resolved for boundary purposes, is refused to the language
/// layer: any path component equal to `.oo`, **or** the resolved operator
/// identity path (`OO_IDENTITY` / `~/.oo/identity`). Component-exact for `.oo`
/// (`.oo_peer_a` etc. pass). Identity refusal is path-exact, not directory-wide.
pub fn crosses_store_boundary(raw: &str) -> bool {
    let resolved = resolve_path_for_boundary(raw);
    if path_has_dot_oo_component(&resolved) {
        return true;
    }
    is_operator_identity_path(&resolved) || is_node_key_dir(&resolved)
}

/// Whether `resolved` is inside the node-key directory (`{node_home}/nodes/`).
///
/// ACCEPTOR REPAIR (node_identity). REAL_01 §7.5.3 already requires that a
/// private key be inside this boundary and that **the protection must not
/// depend on its path happening to contain a store-directory component**. The
/// node key is a private key and was outside it: measured, `~%Io./read_file`
/// on a node key returned `#none` — *permitted*, and unreadable only because
/// PKCS#8 DER is not valid UTF-8. That is protection by coincidence, and one
/// byte-reading builtin away from none at all.
///
/// Directory-wide, unlike the operator key's path-exact rule, and the
/// difference is deliberate: `~/.oo/` holds other things (REAL_01 §7.2's
/// `authorized_keys` will live there), whereas `nodes/` holds node keys and
/// nothing else, so no file in it has a language-layer use.
fn is_node_key_dir(resolved: &Path) -> bool {
    let Ok(home) = crate::value::Identity::resolve_node_home() else {
        return false;
    };
    let dir = resolve_path_for_boundary(&home.join("nodes").to_string_lossy());
    let dc: Vec<_> = dir.components().collect();
    let rc: Vec<_> = resolved.components().collect();
    rc.len() > dc.len() && rc[..dc.len()] == dc[..]
}

/// Whether `resolved` is the operator private-key file (identity_persistence D3).
fn is_operator_identity_path(resolved: &Path) -> bool {
    let Ok(id_path) = crate::value::Identity::resolve_path() else {
        return false;
    };
    // Compare after the same resolution used for language paths.
    let id_resolved = resolve_path_for_boundary(&id_path.to_string_lossy());
    // Lexical equality after resolution; also compare as absolute if identity
    // file does not yet exist (resolve may leave relative tails).
    if paths_eq(&resolved, &id_resolved) {
        return true;
    }
    // Fallback: absolute-form comparison when neither path exists yet.
    let raw_abs = if resolved.is_absolute() {
        resolved.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|c| c.join(resolved))
            .unwrap_or_else(|_| resolved.to_path_buf())
    };
    let id_abs = if id_path.is_absolute() {
        id_path.clone()
    } else {
        id_path
    };
    paths_eq(&raw_abs, &id_abs)
}

fn paths_eq(a: &Path, b: &Path) -> bool {
    // Normalize by component (ignore trailing slash differences).
    let ca: Vec<_> = a.components().collect();
    let cb: Vec<_> = b.components().collect();
    ca == cb
}

/// Resolve so that `sub/../.oo/…`, absolute paths, and symlink escapes
/// (`innocent -> .oo`) are judged on the real component sequence.
///
/// Strategy: canonicalize the nearest existing ancestor (symlink-aware), then
/// append remaining components with `.`/`..` normalized. Do not purely-lexical
/// fold `..` across an unresolved (possibly symlinked) prefix.
fn resolve_path_for_boundary(raw: &str) -> PathBuf {
    let path = Path::new(raw);
    let abs = if path.is_absolute() {
        path.to_path_buf()
    } else {
        match std::env::current_dir() {
            Ok(cwd) => cwd.join(path),
            Err(_) => path.to_path_buf(),
        }
    };

    let components: Vec<Component<'_>> = abs.components().collect();
    if components.is_empty() {
        return abs;
    }

    // Longest existing prefix (by component count).
    let mut existing_len = 0usize;
    let mut trial = PathBuf::new();
    for (i, c) in components.iter().enumerate() {
        match c {
            Component::Prefix(p) => trial.push(p.as_os_str()),
            Component::RootDir => trial.push(c.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                trial.push("..");
            }
            Component::Normal(s) => trial.push(s),
        }
        if trial.as_os_str().is_empty() {
            continue;
        }
        if trial.exists() {
            existing_len = i + 1;
        }
    }

    let mut base = PathBuf::new();
    for c in components.iter().take(existing_len) {
        match c {
            Component::Prefix(p) => base.push(p.as_os_str()),
            Component::RootDir => base.push(c.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                base.push("..");
            }
            Component::Normal(s) => base.push(s),
        }
    }

    let base = if base.as_os_str().is_empty() {
        PathBuf::new()
    } else {
        fs::canonicalize(&base).unwrap_or(base)
    };

    let mut result = base;
    for c in components.iter().skip(existing_len) {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                result.pop();
            }
            Component::Normal(s) => result.push(s),
            Component::RootDir => result.push(c.as_os_str()),
            Component::Prefix(p) => result.push(p.as_os_str()),
        }
    }

    // If nothing existed and we never canonicalized, still normalize the raw
    // absolute path so literal `.oo` components are visible.
    if result.as_os_str().is_empty() {
        return abs;
    }
    result
}

fn path_has_dot_oo_component(path: &Path) -> bool {
    path.components()
        .any(|c| matches!(c, Component::Normal(s) if s == ".oo"))
}

/// ⊥ `#store_boundary` with the offending path in the message.
pub fn store_boundary_refusal(raw: &str) -> Value {
    Value::Bottom(Box::new(BottomDetail {
        cause: BottomCause::StoreBoundary,
        message: Some(format!(
            "store boundary: language cannot touch engine/operator path: {raw}"
        )),
        path: Some(raw.to_string()),
        ..Default::default()
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn component_not_prefix() {
        assert!(!crosses_store_boundary(".oo_peer_a/f.txt"));
        assert!(!crosses_store_boundary(".oomisc"));
        assert!(!crosses_store_boundary("foo.oo"));
        assert!(!crosses_store_boundary("sub/.ooo/f.txt"));
        assert!(crosses_store_boundary(".oo"));
        assert!(crosses_store_boundary(".oo/HEAD"));
        assert!(crosses_store_boundary("sub/../.oo/HEAD"));
    }
}
