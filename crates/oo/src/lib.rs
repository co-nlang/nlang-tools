// oo — Ouroboros engine CLI + nlint (Tier 1 linter)
pub mod nlint;
pub mod operator_io;
pub mod static_analyzer;

pub use operator_io::{operator_io_reason, read_source_file, write_source_file};
