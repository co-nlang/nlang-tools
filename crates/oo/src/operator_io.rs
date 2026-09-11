//! Operator-facing file IO. Host `io::Error` Display (and its `os error N`)
//! is not a sentence the engine is allowed to hand over (REAL_01 §1.3).

use std::io;
use std::path::Path;

/// Map a host IO failure to a short n/-adjacent reason. Never includes
/// `os error`, `ErrorKind` Debug, or a source path from the host runtime.
pub fn operator_io_reason(err: &io::Error) -> &'static str {
    use io::ErrorKind::*;
    match err.kind() {
        NotFound => "file not found",
        PermissionDenied => "permission denied",
        IsADirectory => "is a directory",
        InvalidData | InvalidInput => "not valid text",
        AlreadyExists => "already exists",
        _ => "unreadable",
    }
}

/// Read a source file for a CLI entry point. The only string an operator
/// can see on failure is produced here, so a sixth caller cannot leak errno
/// by forgetting `with_context`.
pub fn read_source_file(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {}", path.display(), operator_io_reason(&e)))
}

/// Same class as [`read_source_file`], for the write half of `fmt --write`.
pub fn write_source_file(path: &Path, contents: &str) -> Result<(), String> {
    std::fs::write(path, contents).map_err(|e| {
        format!(
            "cannot write {}: {}",
            path.display(),
            operator_io_reason(&e)
        )
    })
}
