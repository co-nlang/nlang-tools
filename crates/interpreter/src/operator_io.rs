//! Map a host `io::Error` to a short n/-adjacent reason. Host Display (and
//! its `os error N`) is not a sentence the engine is allowed to hand over
//! (REAL_01 §1.3). Total: every `ErrorKind` has an arm, including `_`.

use std::io;

/// Never includes `os error`, `ErrorKind` Debug, or a source path from the
/// host runtime. The catch-all is the reason this is a class, not a list of
/// the kinds we have seen so far.
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

#[cfg(test)]
mod tests {
    use super::operator_io_reason;
    use std::io::{Error, ErrorKind};

    #[test]
    fn every_mapped_kind_is_engine_words() {
        let kinds = [
            ErrorKind::NotFound,
            ErrorKind::PermissionDenied,
            ErrorKind::IsADirectory,
            ErrorKind::InvalidData,
            ErrorKind::InvalidInput,
            ErrorKind::AlreadyExists,
            ErrorKind::BrokenPipe,
            ErrorKind::Other,
        ];
        for kind in kinds {
            let reason = operator_io_reason(&Error::from(kind));
            assert!(
                !reason.contains("os error")
                    && !reason.contains("ErrorKind")
                    && !reason.contains("std::"),
                "host words in {kind:?} → {reason:?}"
            );
        }
    }
}
