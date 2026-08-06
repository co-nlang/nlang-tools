//! Ephemeral workspace directories that remove themselves on drop.
//!
//! Probes and `Ouroboros::new_in_memory` create real `.oo/` trees under the
//! process temp directory. Without RAII cleanup those trees accumulate across
//! test runs. Hold a [`ScratchDir`] (or the engine's internal ephemeral root)
//! for the lifetime of the workspace; dropping it deletes the tree.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{fs, io};

static SEQ: AtomicUsize = AtomicUsize::new(0);

/// A unique directory under the system temp path. Deleted when dropped.
///
/// Implements [`AsRef<Path>`] and [`std::ops::Deref`] to [`Path`] so existing
/// helpers that take `&Path` keep working with `&scratch` / `scratch.as_ref()`.
#[derive(Debug)]
pub struct ScratchDir {
    path: PathBuf,
}

impl ScratchDir {
    /// Create `nlang-{prefix}-{pid}-{seq}` under the system temp directory.
    pub fn new(prefix: &str) -> Self {
        let mut path = std::env::temp_dir();
        // Sanitize prefix so callers can pass tags with path-ish characters.
        let safe: String = prefix
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '-'
                }
            })
            .collect();
        path.push(format!(
            "nlang-{}-{}-{}",
            if safe.is_empty() { "tmp" } else { &safe },
            std::process::id(),
            SEQ.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create scratch dir");
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Persist the directory past this guard (e.g. deliberate leftover for a
    /// follow-up process). Prefer not to use this in ordinary probes.
    pub fn keep(self) -> PathBuf {
        let path = self.path.clone();
        std::mem::forget(self);
        path
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        // Best-effort: a still-running child may hold files open; ignore errors.
        let _ = fs::remove_dir_all(&self.path);
    }
}

impl AsRef<Path> for ScratchDir {
    fn as_ref(&self) -> &Path {
        &self.path
    }
}

impl std::ops::Deref for ScratchDir {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.path
    }
}

/// Create an ephemeral store root for in-memory engines (same naming as probes).
pub fn ephemeral_store_root() -> io::Result<tempfile::TempDir> {
    tempfile::Builder::new().prefix("nlang-test-").tempdir()
}
