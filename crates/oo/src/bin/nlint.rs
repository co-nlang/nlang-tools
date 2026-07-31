// nlint — Tier 1 linter CLI for n/ (handover docs/linter_tier1_handover.md)
use clap::Parser as ClapParser;
use std::path::PathBuf;

#[derive(ClapParser)]
#[command(
    author,
    version,
    about = "Tier 1 linter (pure syntax / pure graph theory) — no obstruction claims"
)]
struct Cli {
    /// .n file or directory (recursive)
    path: PathBuf,
    /// emit JSON (tier1-v1 schema) instead of human-readable summary
    #[arg(long)]
    json: bool,
}

fn main() -> std::process::ExitCode {
    let cli = Cli::parse();
    let code = oo::nlint::run_cli(&cli.path, cli.json);
    std::process::ExitCode::from(code as u8)
}
