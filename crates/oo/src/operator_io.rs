//! Operator-facing file IO. Host `io::Error` Display (and its `os error N`)
//! is not a sentence the engine is allowed to hand over (REAL_01 §1.3).
//!
//! The mapping itself lives in the interpreter so the store (another crate)
//! uses the same total function. The catch-all arm is why this is a class.

use std::path::Path;

pub use nlang_interpreter::operator_io_reason;

/// rustc ignores SIGPIPE, so `println!` turns EPIPE into a panic that quotes
/// libstd and `os error 32`. Restore SIG_DFL at process start: a closed
/// reader then kills the process instead of talking. One call covers every
/// operator-facing write (101 `print!` sites in `main.rs` plus nlint).
/// `fmt` already discards `writeln!` errors, which is why output size never
/// decided who panicked.
pub fn restore_sigpipe_default() {
    #[cfg(unix)]
    {
        extern "C" {
            fn signal(signum: i32, handler: usize) -> usize;
        }
        // POSIX: SIGPIPE = 13, SIG_DFL = 0. rustc has already set SIG_IGN.
        unsafe {
            signal(13, 0);
        }
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
