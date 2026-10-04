use clap::{Parser, Subcommand};
use nlang_interpreter::value::{BottomCause, BottomDetail};
use nlang_interpreter::{
    CommitMeta, ContentHash, EffectTag, Ouroboros, Privilege, Universe, Value,
};
use nlang_parser::ast::{AtomKind, FieldKey};
use nlang_parser::parse_program;
use std::fs;
use std::io::{stdin, stdout, Write};
use std::path::{Path, PathBuf};

/// Exclusive lock over a store's commit critical section.
/// `commit`, `refine`, `squash`, `rollback`, and `migrate` take it.
/// Reads, `evolve`, and `gc` do not. Not compare-and-swap on HEAD.
///
/// The lock is `std::fs::File::{try_lock, lock}` (stable 1.89), the same
/// exclusive-file API on every target std supports. Taken on the existing
/// `.oo/format` file — a new lock file would be a layout change. Process-held;
/// crash releases it. Does not write the file.
///
/// `migrate` replaces that file by rename. A waiter that opened the old
/// inode can be granted its lock after the name points somewhere else.
/// The lock counts only when the open file is still the directory entry;
/// otherwise it is dropped and the path is opened again.
struct CommitLock {
    file: fs::File,
}

/// Identity of a directory entry. Unix is device + inode. Windows is
/// volume serial + file index from `GetFileInformationByHandle` (stable;
/// the same pair `MetadataExt` would report). A platform that reports
/// neither has no replacement check; the exclusive lock is the whole
/// section there.
#[cfg(not(windows))]
fn file_id(meta: &fs::Metadata) -> Option<(u64, u64)> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        return Some((meta.dev(), meta.ino()));
    }
    #[cfg(not(unix))]
    {
        let _ = meta;
        None
    }
}

#[cfg(windows)]
fn windows_file_id(file: &fs::File, path: &Path) -> anyhow::Result<(u64, u64)> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
    };
    let mut info = unsafe { std::mem::zeroed::<BY_HANDLE_FILE_INFORMATION>() };
    let rc = unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) };
    if rc == 0 {
        let e = std::io::Error::last_os_error();
        anyhow::bail!(
            "cannot lock {}: {}",
            path.display(),
            oo::operator_io_reason(&e)
        );
    }
    let index = ((info.nFileIndexHigh as u64) << 32) | (info.nFileIndexLow as u64);
    Ok((info.dwVolumeSerialNumber as u64, index))
}

#[cfg(not(windows))]
fn still_the_directory_entry(file: &fs::File, path: &Path) -> anyhow::Result<bool> {
    let open_meta = file.metadata().map_err(|e| {
        anyhow::anyhow!(
            "cannot lock {}: {}",
            path.display(),
            oo::operator_io_reason(&e)
        )
    })?;
    let path_meta = match fs::metadata(path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => {
            return Err(anyhow::anyhow!(
                "cannot lock {}: {}",
                path.display(),
                oo::operator_io_reason(&e)
            ));
        }
    };
    match (file_id(&open_meta), file_id(&path_meta)) {
        (Some(open_id), Some(path_id)) => Ok(open_id == path_id),
        (None, None) => Ok(true),
        _ => Ok(false),
    }
}

#[cfg(windows)]
fn still_the_directory_entry(file: &fs::File, path: &Path) -> anyhow::Result<bool> {
    let open_id = windows_file_id(file, path)?;
    let path_file = match fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => {
            return Err(anyhow::anyhow!(
                "cannot lock {}: {}",
                path.display(),
                oo::operator_io_reason(&e)
            ));
        }
    };
    let path_id = windows_file_id(&path_file, path)?;
    Ok(open_id == path_id)
}

impl CommitLock {
    /// Returns whether this process had to wait for another holder.
    /// Waited-then-empty is "consumed", not "Nothing to commit" (S2/S3).
    fn acquire(base: &Path) -> anyhow::Result<(Self, bool)> {
        let oo = base.join(".oo");
        fs::create_dir_all(&oo).map_err(|e| {
            anyhow::anyhow!("cannot write {}: {}", oo.display(), oo::operator_io_reason(&e))
        })?;
        // Lock an already-declared file. A new `.oo/commit.lock` is a layout
        // change (p1 / p4). `format` exists for any store this path can open.
        // Open read+write so Windows LockFileEx can take an exclusive lock;
        // do not truncate. The write bit is the lock, not a rewrite of the
        // declaration — a readable `format` can still refuse this open.
        let path = oo.join("format");
        let mut contended = false;
        loop {
            let file = fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(&path)
                .map_err(|e| {
                    anyhow::anyhow!(
                        "cannot lock {}: {}",
                        path.display(),
                        oo::operator_io_reason(&e)
                    )
                })?;
            match file.try_lock() {
                Ok(()) => {}
                Err(std::fs::TryLockError::WouldBlock) => {
                    file.lock().map_err(|e| {
                        anyhow::anyhow!(
                            "cannot lock {}: {}",
                            path.display(),
                            oo::operator_io_reason(&e)
                        )
                    })?;
                    contended = true;
                }
                Err(std::fs::TryLockError::Error(e)) => {
                    anyhow::bail!(
                        "cannot lock {}: {}",
                        path.display(),
                        oo::operator_io_reason(&e)
                    )
                }
            }
            if still_the_directory_entry(&file, &path)? {
                return Ok((Self { file }, contended));
            }
            let _ = file.unlock();
        }
    }
}

impl Drop for CommitLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

/// Operator-facing coordinate + cause (no file wrapper).
/// Uses `detail.path` when present; otherwise the field-key fallback.
/// Never Debug of AST keys or spans.
fn format_conflict_where(detail: &BottomDetail, field_fallback: Option<&str>) -> String {
    let cause = bottom_cause_tag(detail.cause);
    let coord = detail
        .path
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .or(field_fallback.map(str::trim).filter(|s| !s.is_empty()));
    match coord {
        Some(c) => format!("{cause} at {c}"),
        None => cause.to_string(),
    }
}

/// Full operator-facing evolve failure text (where_the_conflict_is / W3'-a).
fn format_evolution_conflict(detail: &BottomDetail, field_fallback: Option<&str>) -> String {
    format!(
        "Evolution Conflict: {}",
        format_conflict_where(detail, field_fallback)
    )
}

fn bottom_cause_tag(c: BottomCause) -> &'static str {
    match c {
        BottomCause::Conflict => "#conflict",
        BottomCause::MissingKey => "#missing_key",
        BottomCause::FuelExhausted => "#fuel_exhausted",
        BottomCause::Timeout => "#timeout",
        BottomCause::PeerUnreachable => "#peer_unreachable",
        BottomCause::PeerClosed => "#peer_closed",
        BottomCause::PeerTimeout => "#peer_timeout",
        BottomCause::Divergent => "#divergent",
        BottomCause::InvalidPath => "#invalid_path",
        BottomCause::PrivateAccessViolation => "#private_access_violation",
        BottomCause::NumericalError => "#numerical_error",
        BottomCause::ArithmeticOnAnchor => "#arithmetic_on_anchor",
        BottomCause::H1Split => "#h1_split",
        BottomCause::H2Split => "#h2_split",
        BottomCause::SemanticEclipse => "#semantic_eclipse",
        BottomCause::NoContext => "#no_context",
        BottomCause::OutOfHorizon => "#out_of_horizon",
        BottomCause::SystemReserved => "#system_reserved",
        BottomCause::InvalidConfig => "#invalid_config",
        BottomCause::EffectViolation => "#effect_violation",
        BottomCause::PrivilegedRequired => "#privileged_required",
        BottomCause::StoreBoundary => "#store_boundary",
        BottomCause::CaidMismatch => "#caid_mismatch",
        BottomCause::PeerNotImplemented => "#peer_not_implemented",
        BottomCause::PeerUnknownStatus => "#peer_unknown_status",
        BottomCause::PeerRefused => "#peer_refused",
        BottomCause::RoutingBudgetExceeded => "#routing_budget_exceeded",
        BottomCause::MaxDepthExceeded => "#max_depth_exceeded",
        BottomCause::StackOverflow => "#stack_overflow",
        BottomCause::ObjectUndecodable => "#object_undecodable",
        BottomCause::StandardRootUnavailable => "#standard_root_unavailable",
        BottomCause::NoStandardRoot => "#no_standard_root",
        BottomCause::UnprojectedBuiltin => "#unprojected_builtin",
        BottomCause::UnprovidedBuiltin => "#unprovided_builtin",
        BottomCause::NoUniverse => "#no_universe",
        BottomCause::Unwritable => "#unwritable",
    }
}

fn real_cwd() -> anyhow::Result<PathBuf> {
    std::env::current_dir().map_err(|e| {
        anyhow::anyhow!(
            "cannot read the working directory: {}",
            oo::operator_io_reason(&e)
        )
    })
}

std::thread_local! {
    static SELECTED_UNIVERSE: std::cell::RefCell<Option<PathBuf>> =
        const { std::cell::RefCell::new(None) };
}

/// Store directory for this process. `--universe` / `--ephemeral` replace it.
/// File arguments stay on the process directory: `File::open` does not come
/// through here.
fn cwd() -> anyhow::Result<PathBuf> {
    if let Some(path) = SELECTED_UNIVERSE.with(|slot| slot.borrow().clone()) {
        return Ok(path);
    }
    real_cwd()
}

/// Removes an `--ephemeral` directory when the command returns.
struct SelectedUniverse {
    ephemeral: Option<PathBuf>,
}

impl Drop for SelectedUniverse {
    fn drop(&mut self) {
        SELECTED_UNIVERSE.with(|slot| *slot.borrow_mut() = None);
        if let Some(path) = self.ephemeral.take() {
            let _ = fs::remove_dir_all(path);
        }
    }
}

fn select_universe(universe: Option<&Path>, ephemeral: bool) -> anyhow::Result<SelectedUniverse> {
    if ephemeral && universe.is_some() {
        anyhow::bail!("--ephemeral and --universe name two universes; pass one");
    }
    if ephemeral {
        let path = std::env::temp_dir().join(format!(
            "oo-ephemeral-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        fs::create_dir(&path).map_err(|e| {
            anyhow::anyhow!(
                "cannot write {}: {}",
                path.display(),
                oo::operator_io_reason(&e)
            )
        })?;
        SELECTED_UNIVERSE.with(|slot| *slot.borrow_mut() = Some(path.clone()));
        return Ok(SelectedUniverse {
            ephemeral: Some(path),
        });
    }
    if let Some(path) = universe {
        let abs = if path.is_absolute() {
            path.to_path_buf()
        } else {
            real_cwd()?.join(path)
        };
        SELECTED_UNIVERSE.with(|slot| *slot.borrow_mut() = Some(abs));
    }
    Ok(SelectedUniverse { ephemeral: None })
}

fn hex_digest(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Simple n/-style label for a field key when `path` was not computed.
fn field_key_label(key: &FieldKey) -> String {
    match key {
        FieldKey::Named { name, prefix } => {
            let p = match prefix {
                Some(nlang_parser::ast::Prefix::Logic) => "/",
                Some(nlang_parser::ast::Prefix::Type) => "@",
                Some(nlang_parser::ast::Prefix::Meta) => "%",
                Some(nlang_parser::ast::Prefix::System) => "~%",
                Some(nlang_parser::ast::Prefix::Private)
                | Some(nlang_parser::ast::Prefix::Local)
                | Some(nlang_parser::ast::Prefix::Data)
                | None => "",
            };
            format!("{p}{}", name.trim())
        }
        FieldKey::Quoted(n) => n.trim().to_string(),
        FieldKey::Path(p) => p
            .segments
            .iter()
            .map(|s| s.trim())
            .collect::<Vec<_>>()
            .join("."),
        _ => String::new(),
    }
}

/// One concept, one help string (REAL_01 §1.4). Every command that exposes
/// `--grant` / `--privileged` must cite these, not a local paraphrase.
const HELP_GRANT: &str = "Grant one named capability (repeatable; union). Use this for pin, rollback, squash, gc, migrate, or effect_override; use --privileged only when you want every capability at once";
const HELP_PRIVILEGED: &str = "Grant every §6 capability at once. Cannot be set from inside an n/ program (SPEC_08 §6.1.2). Prefer --grant when only one capability is needed";
const HELP_MESSAGE: &str = "Human-readable message stored on the recorded event";
const HELP_FILES: &str = "n/ source files to read";
const HELP_UNIVERSE: &str = "Use the universe in this directory instead of the one where this command is run";
const HELP_EPHEMERAL: &str = "Evaluate against an anonymous temporary universe and leave this universe untouched";
const HELP_PEER_TO: &str = "Peer address host:port";
const HELP_OPERATOR_KEY: &str = "Operator public key to trust (64 lowercase hex)";

#[derive(Parser)]
#[command(author, version = env!("OO_VERSION"), about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Inject files into a read-only view of this universe's committed root and observe. Does not stage or commit. An explicit ~%Engine./save writes this universe's object store; with no universe it answers #no_universe
    Run {
        #[arg(required = true, help = HELP_FILES)]
        files: Vec<PathBuf>,
        /// Path to observe after evolving, instead of dumping the whole universe
        #[arg(short, long)]
        observe: Option<String>,
        /// Print the universe as n/; takes no value — a word after this flag is a file name, not a format
        #[arg(short, long)]
        format: bool,
        #[arg(long, help = HELP_PRIVILEGED)]
        privileged: bool,
        #[arg(
            long = "grant",
            value_name = "SPEC",
            action = clap::ArgAction::Append,
            help = HELP_GRANT
        )]
        grants: Vec<String>,
        #[arg(long, value_name = "DIR", help = HELP_UNIVERSE)]
        universe: Option<PathBuf>,
        #[arg(long, help = HELP_EPHEMERAL)]
        ephemeral: bool,
    },
    /// Stage file contents into this workspace's working set; does not record a commit
    Evolve {
        #[arg(required = true, help = HELP_FILES)]
        files: Vec<PathBuf>,
        /// Request overwrite of committed coordinates; also requires `--grant pin` (request is not capability)
        #[arg(long)]
        pin: bool,
        #[arg(
            long = "grant",
            value_name = "SPEC",
            action = clap::ArgAction::Append,
            help = HELP_GRANT
        )]
        grants: Vec<String>,
        #[arg(long, value_name = "DIR", help = HELP_UNIVERSE)]
        universe: Option<PathBuf>,
    },
    /// Observe `test_` fields from this universe's committed root and report pass/fail (use lint for a static graph check that does not run)
    Test {
        /// Check that `test_` fields parse, without observing them
        #[arg(long)]
        static_only: bool,
        /// Only run tests whose names contain this substring
        #[arg(short, long)]
        pattern: Option<String>,
        #[arg(help = HELP_FILES)]
        files: Vec<PathBuf>,
        #[arg(long, value_name = "DIR", help = HELP_UNIVERSE)]
        universe: Option<PathBuf>,
        #[arg(long, help = HELP_EPHEMERAL)]
        ephemeral: bool,
    },
    /// Read-eval-print loop from this universe's committed root. Each line is printed and kept in this session only; it is not staged or committed. The working set does not count. Where there is no universe, the root is empty. An explicit ~%Engine./save writes this universe's object store; with no universe it answers #no_universe
    Repl {
        #[arg(long, value_name = "DIR", help = HELP_UNIVERSE)]
        universe: Option<PathBuf>,
        #[arg(long, help = HELP_EPHEMERAL)]
        ephemeral: bool,
    },
    /// Show the staged working set, or that the universe is static
    Status {
        #[arg(long, value_name = "DIR", help = HELP_UNIVERSE)]
        universe: Option<PathBuf>,
    },
    /// List commits from HEAD backward (the history; status is the working set)
    Log {
        #[arg(long, value_name = "DIR", help = HELP_UNIVERSE)]
        universe: Option<PathBuf>,
    },
    /// Record the staged working set as a new commit and move HEAD
    Commit {
        #[arg(short, long, help = HELP_MESSAGE)]
        message: Option<String>,
        #[arg(
            long = "grant",
            value_name = "SPEC",
            action = clap::ArgAction::Append,
            help = HELP_GRANT
        )]
        grants: Vec<String>,
        #[arg(long, help = HELP_PRIVILEGED)]
        privileged: bool,
        #[arg(long, value_name = "DIR", help = HELP_UNIVERSE)]
        universe: Option<PathBuf>,
    },
    /// Write a signed refinement from source coordinates onto target coordinates
    Refine {
        /// Coordinates this refinement copies from
        #[arg(short, long, required = true, num_args = 1..)]
        source: Vec<String>,
        /// Coordinates this refinement writes into (the destination, not `--source`)
        #[arg(short, long, required = true, num_args = 1..)]
        target: Vec<String>,
        /// Sign the refinement with the operator key
        #[arg(long)]
        sign: bool,
        #[arg(short, long, help = HELP_MESSAGE)]
        message: Option<String>,
        #[arg(long, value_name = "DIR", help = HELP_UNIVERSE)]
        universe: Option<PathBuf>,
    },
    /// Print canonical n/ for a file (use --write to replace the file; use evolve to stage it)
    Fmt {
        /// n/ source file to format
        file: PathBuf,
        /// Replace the file with the canonical form; without this flag, print to stdout
        #[arg(short, long)]
        write: bool,
    },
    /// Universe node: serve, advertise, and discover over OODP (REAL_01 §1.2)
    Node {
        #[command(subcommand)]
        action: NodeCmd,
    },
    /// Evaluate one n/ expression from this universe's committed root and print it. The working set does not count. Where there is no universe, the root is empty
    Eval {
        /// n/ expression to evaluate (quote it for the shell)
        expr: String,
        #[arg(long, help = HELP_PRIVILEGED)]
        privileged: bool,
        #[arg(
            long = "grant",
            value_name = "SPEC",
            action = clap::ArgAction::Append,
            help = HELP_GRANT
        )]
        grants: Vec<String>,
        #[arg(long, value_name = "DIR", help = HELP_UNIVERSE)]
        universe: Option<PathBuf>,
        #[arg(long, help = HELP_EPHEMERAL)]
        ephemeral: bool,
    },
    /// Print a stored object by CAID (the bytes, not a path in the working set)
    Inspect {
        /// CAID of the object to print (hash:sha256:v1:… or v2:…)
        caid: String,
        #[arg(long, value_name = "DIR", help = HELP_UNIVERSE)]
        universe: Option<PathBuf>,
    },
    /// Move HEAD to a historical commit without creating one. Requires `--grant rollback`
    Rollback {
        /// Commit CAID that becomes HEAD
        caid: String,
        #[arg(
            long = "grant",
            value_name = "SPEC",
            action = clap::ArgAction::Append,
            help = HELP_GRANT
        )]
        grants: Vec<String>,
        #[arg(long, help = HELP_PRIVILEGED)]
        privileged: bool,
        #[arg(long, value_name = "DIR", help = HELP_UNIVERSE)]
        universe: Option<PathBuf>,
    },
    /// Fold commits after BASE through HEAD into one. Requires `--grant squash`
    Squash {
        /// Base commit CAID; it survives as the parent of the squashed commit
        caid: String,
        #[arg(
            long = "grant",
            value_name = "SPEC",
            action = clap::ArgAction::Append,
            help = HELP_GRANT
        )]
        grants: Vec<String>,
        #[arg(long, help = HELP_PRIVILEGED)]
        privileged: bool,
        #[arg(long, value_name = "DIR", help = HELP_UNIVERSE)]
        universe: Option<PathBuf>,
    },
    /// Remove unreachable objects under `.oo/objects/`. Requires `--grant gc`; never automatic
    Gc {
        #[arg(
            long = "grant",
            value_name = "SPEC",
            action = clap::ArgAction::Append,
            help = HELP_GRANT
        )]
        grants: Vec<String>,
        #[arg(long, help = HELP_PRIVILEGED)]
        privileged: bool,
        /// Report what would be removed; do not delete
        #[arg(long)]
        dry_run: bool,
        #[arg(long, value_name = "DIR", help = HELP_UNIVERSE)]
        universe: Option<PathBuf>,
    },
    /// Advance the container layout declaration only. Requires `--grant migrate`; does not move HEAD
    Migrate {
        #[arg(
            long = "grant",
            value_name = "SPEC",
            action = clap::ArgAction::Append,
            help = HELP_GRANT
        )]
        grants: Vec<String>,
        #[arg(long, help = HELP_PRIVILEGED)]
        privileged: bool,
        #[arg(long, value_name = "DIR", help = HELP_UNIVERSE)]
        universe: Option<PathBuf>,
    },
    /// Show the operator public key and identity file path; mint them on first use
    Identity,
    /// Static graph linter (no evaluation; use test to run `test_` fields)
    Lint {
        /// .n file, or a directory walked for `.n` files
        path: PathBuf,
        /// Write the report as JSON instead of the human summary
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum NodeCmd {
    /// Serve OODP on TCP (REAL_02 §3.2). Request/response carry `%status`.
    Serve {
        /// TCP port this process listens on (the peer's address is `--to` on other commands)
        #[arg(short, long, default_value_t = 8080)]
        port: u16,
    },
    /// Print this workspace's node id (CAID of the node public key) and key path.
    Id,
    /// Send a signed OODP `#advertise` to a peer and print `%status` / `%reason`.
    Advertise {
        /// Peer address `host:port`
        #[arg(long = "to", value_name = "HOST:PORT", help = HELP_PEER_TO)]
        to: String,
        /// Service CAID to list (repeatable; empty list is a liveness announcement)
        #[arg(long = "service", value_name = "CAID", action = clap::ArgAction::Append)]
        services: Vec<String>,
        /// Claimed listening port (signed); default matches `oo node serve`
        #[arg(long = "listen-port", default_value_t = 8080)]
        listen_port: u16,
    },
    /// Query a peer's service index for who advertises `--target`.
    Discover {
        /// Peer address `host:port`
        #[arg(long = "to", value_name = "HOST:PORT", help = HELP_PEER_TO)]
        to: String,
        /// Service CAID to look up
        #[arg(long = "target", value_name = "CAID")]
        target: String,
    },
    /// Kademlia FIND_NODE: k closest known peers to a 160-bit id.
    #[command(name = "find-node")]
    FindNode {
        #[arg(long = "to", value_name = "HOST:PORT", help = HELP_PEER_TO)]
        to: String,
        /// Exactly 40 lowercase hex characters (not a CAID).
        #[arg(long = "target", value_name = "HEX40")]
        target: String,
    },
    /// Mint an affiliation claim for this workspace's node (operator-signed).
    /// Persists beside the node key; serving attaches it without the operator key.
    Affiliate {
        /// Claim lifetime in seconds (default and max: 30 days).
        #[arg(long = "ttl-secs")]
        ttl_secs: Option<i64>,
    },
    /// List known peers and any verified affiliation operator key.
    Peers,
    /// Manage workspace affiliation trust roots (`.oo/discovery.n`).
    Trust {
        #[command(subcommand)]
        action: TrustCmd,
    },
}

#[derive(Subcommand)]
enum TrustCmd {
    /// List affiliation roots (sorted hex keys). Missing file = empty, no write.
    List,
    /// Add an operator public key (64 lowercase hex).
    Add {
        #[arg(value_name = "OPERATOR_KEY", help = HELP_OPERATOR_KEY)]
        operator_key: String,
    },
    /// Remove an operator public key.
    Remove {
        #[arg(value_name = "OPERATOR_KEY", help = HELP_OPERATOR_KEY)]
        operator_key: String,
    },
}

fn main() -> anyhow::Result<()> {
    oo::restore_sigpipe_default();
    // Eval recursion (morphism apply / left-deep math) can exceed the default
    // main-thread stack before the engine depth horizon engages. Interpreter
    // probes use 64 MiB threads; match that for the CLI entrypoint.
    const STACK: usize = 64 * 1024 * 1024;
    let handle = match std::thread::Builder::new()
        .name("oo-main".into())
        .stack_size(STACK)
        .spawn(main_on_large_stack)
    {
        Ok(handle) => handle,
        // Host refused the thread (EAGAIN / address-space). Same name as the
        // parser-thread spawn failure: the operator's remedy is a process
        // limit, not flattening the program. Do not leak Os Debug.
        Err(_) => anyhow::bail!("#host_resource_denied"),
    };
    match handle.join() {
        Ok(result) => result,
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

fn main_on_large_stack() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Run {
            files,
            observe,
            format,
            privileged,
            grants,
            universe,
            ephemeral,
        } => run_one_shot(
            files,
            observe,
            format,
            privileged,
            grants,
            universe,
            ephemeral,
        ),
        Commands::Evolve {
            files,
            pin,
            grants,
            universe,
        } => run_evolve(files, pin, grants, universe),
        Commands::Fmt { file, write } => run_fmt(file, write),
        Commands::Node { action } => match action {
            NodeCmd::Serve { port } => run_serve(port),
            NodeCmd::Id => run_node_id(),
            NodeCmd::Advertise {
                to,
                services,
                listen_port,
            } => run_node_advertise(to, services, listen_port),
            NodeCmd::Discover { to, target } => run_node_discover(to, target),
            NodeCmd::FindNode { to, target } => run_node_find_node(to, target),
            NodeCmd::Affiliate { ttl_secs } => run_node_affiliate(ttl_secs),
            NodeCmd::Peers => run_node_peers(),
            NodeCmd::Trust { action } => match action {
                TrustCmd::List => run_node_trust_list(),
                TrustCmd::Add { operator_key } => run_node_trust_add(operator_key),
                TrustCmd::Remove { operator_key } => run_node_trust_remove(operator_key),
            },
        },
        Commands::Status { universe } => run_status(universe),
        Commands::Log { universe } => run_log(universe),
        Commands::Commit {
            message,
            grants,
            privileged,
            universe,
        } => run_commit(message, grants, privileged, universe),
        Commands::Refine {
            source,
            target,
            sign,
            message,
            universe,
        } => run_refine(source, target, sign, message, universe),
        Commands::Repl { universe, ephemeral } => run_repl(universe, ephemeral),
        Commands::Test {
            static_only,
            pattern,
            files,
            universe,
            ephemeral,
        } => run_test(static_only, pattern, files, universe, ephemeral),
        Commands::Eval {
            expr,
            privileged,
            grants,
            universe,
            ephemeral,
        } => run_eval(expr, privileged, grants, universe, ephemeral),
        Commands::Inspect { caid, universe } => run_inspect(caid, universe),
        Commands::Identity => run_identity(),
        Commands::Rollback {
            caid,
            grants,
            privileged,
            universe,
        } => run_rollback(caid, grants, privileged, universe),
        Commands::Squash {
            caid,
            grants,
            privileged,
            universe,
        } => run_squash(caid, grants, privileged, universe),
        Commands::Gc {
            grants,
            privileged,
            dry_run,
            universe,
        } => run_gc(grants, privileged, dry_run, universe),
        Commands::Migrate {
            grants,
            privileged,
            universe,
        } => run_migrate(grants, privileged, universe),
        Commands::Lint { path, json } => {
            let code = oo::nlint::run_cli(&path, json);
            std::process::exit(code);
        }
    }
}

const NO_UNIVERSE: &str = "no universe here: start one with evolve";

fn require_universe(base: &Path) -> anyhow::Result<()> {
    if nlang_interpreter::storage::universe_content(base)? {
        Ok(())
    } else {
        anyhow::bail!("{NO_UNIVERSE}")
    }
}

/// A command that does not need a universe still reads node settings.
/// Values stay in an ephemeral store until a universe exists.
/// One-shot view. A named directory with no universe is a refusal.
/// Standing where there is none, or `--ephemeral`, is an empty root.
/// The working set is not loaded.
fn one_shot_view(
    engine: &Ouroboros,
    cur: &Path,
    named: bool,
) -> anyhow::Result<Universe> {
    if !engine.holds_universe {
        if named {
            anyhow::bail!("{NO_UNIVERSE}");
        }
        return Ok(Universe::new_with_standard(
            None,
            nlang_interpreter::value::ComboVal::default(),
            engine.root_with_system(),
        ));
    }
    refuse_lost_context(&engine.store, cur)?;
    Universe::load(engine, cur)
}

fn engine_for_one_shot(
    cur: &Path,
    named: bool,
    ephemeral: bool,
) -> anyhow::Result<Ouroboros> {
    if ephemeral || !nlang_interpreter::storage::universe_content(cur)? {
        if named && !ephemeral {
            anyhow::bail!("{NO_UNIVERSE}");
        }
        return Ouroboros::without_universe(cur);
    }
    Ouroboros::init(cur)
}

fn engine_keeping_settings(base: &Path) -> anyhow::Result<Ouroboros> {
    if nlang_interpreter::storage::universe_content(base)? {
        Ouroboros::init(base)
    } else {
        Ouroboros::without_universe(base)
    }
}

fn refuse_lost_context(
    store: &nlang_interpreter::storage::ObjectStore,
    base: &Path,
) -> anyhow::Result<()> {
    if store.get_head(base)?.is_none() && nlang_interpreter::savepoint::records_a_commit(base)? {
        anyhow::bail!("{}", nlang_interpreter::savepoint::LOST_CONTEXT);
    }
    Ok(())
}

fn run_evolve(
    files: Vec<PathBuf>,
    pin: bool,
    grants: Vec<String>,
    universe: Option<PathBuf>,
) -> anyhow::Result<()> {
    let _hold = select_universe(universe.as_deref(), false)?;
    let cur = cwd()?;
    let mut engine = Ouroboros::init(&cur)?;
    // Reuse the same grant parser as run/eval — never a second code path.
    apply_cli_privilege(&mut engine, false, &grants)?;
    refuse_lost_context(&engine.store, &cur)?;
    // Two-step gate (SPEC_08 §6.2 / P1): `--pin` is the request; `--grant pin`
    // is the capability. Request without capability is a loud refuse — never
    // silently downgraded to ordinary (conflicting) evolve.
    if pin && !engine.privilege.pin {
        anyhow::bail!(
            "#privileged_required: --pin requires --grant pin (privilege.pin capability)"
        );
    }
    let mut universe = load_universe(&engine, &cur)?;
    universe.pin_mode = pin;
    // The working set this command read, before its own fields land.
    let before = universe.staged.clone();

    for file in files {
        let input = oo::read_source_file(&file).map_err(|m| anyhow::anyhow!("{m}"))?;
        let program = match parse_program(&input) {
            Ok(program) => program,
            Err(error) => return Err(anyhow::anyhow!("Parse Error in {:?}: {}", file, error)),
        };
        for f in &program.fields {
            if let Err(e) = universe.evolve(&engine, &f) {
                let fb = field_key_label(&f.key);
                anyhow::bail!(
                    "Evolution Conflict in \"{}\": {}",
                    file.display(),
                    format_conflict_where(&e, Some(&fb))
                );
            }
        }
    }
    universe.save_staged(&engine, &cur, &before)?;
    print_integrity_incidents(&engine);
    Ok(())
}

enum CappedRequestLine {
    Eof,
    Line(Vec<u8>),
    TooLarge,
}

/// Read one OODP request line without ever accumulating more than `limit`
/// payload bytes. `BufReader` may hold its normal small read buffer, but a
/// hostile line cannot become an unbounded application allocation.
fn read_capped_request_line<R: std::io::BufRead>(
    reader: &mut R,
    limit: usize,
) -> std::io::Result<CappedRequestLine> {
    let mut request = Vec::with_capacity(limit);
    loop {
        let (consume, append, complete, too_large) = {
            let buffer = reader.fill_buf()?;
            if buffer.is_empty() {
                return Ok(if request.is_empty() {
                    CappedRequestLine::Eof
                } else {
                    CappedRequestLine::Line(request)
                });
            }

            let remaining = limit.saturating_sub(request.len());
            // Include one byte beyond the permitted payload only to accept a
            // newline immediately after an exactly-limit-sized request.
            let searchable = buffer.len().min(remaining.saturating_add(1));
            if let Some(newline) = buffer[..searchable].iter().position(|&b| b == b'\n') {
                (newline + 1, newline, true, false)
            } else if request.len() >= limit || buffer.len() > remaining {
                (0, 0, false, true)
            } else {
                (buffer.len(), buffer.len(), false, false)
            }
        };

        if too_large {
            return Ok(CappedRequestLine::TooLarge);
        }

        if append > 0 {
            let buffer = reader.fill_buf()?;
            request.extend_from_slice(&buffer[..append]);
        }
        reader.consume(consume);
        if complete {
            return Ok(CappedRequestLine::Line(request));
        }
    }
}

fn run_serve(port: u16) -> anyhow::Result<()> {
    use nlang_interpreter::oodp;
    use std::io::BufReader;
    let listener = std::net::TcpListener::bind(format!("0.0.0.0:{port}")).map_err(|e| {
        anyhow::anyhow!(
            "cannot listen on port {port}: {}",
            oo::operator_io_reason(&e)
        )
    })?;
    let bound_port = listener.local_addr().map_err(|e| {
        anyhow::anyhow!(
            "cannot listen on port {port}: {}",
            oo::operator_io_reason(&e)
        )
    })?.port();
    let current_dir = cwd()?;
    let engine = engine_keeping_settings(&current_dir)?;
    // %source = node id (CAID of the node public key), not the listen port.
    // Two ports on one workspace share one id; two workspaces do not.
    let source_id = engine.node_id()?.to_string();
    println!(
        "n/ OODP node serving at port {} (node {})",
        bound_port, source_id
    );
    // advert_persistence §3.2 — load report on the serve log (probes parse it).
    if let Some(ref rep) = engine.peers_load_report {
        if let Some(ref line) = rep.log_line {
            println!("{}", line);
        }
    }

    for stream in listener.incoming() {
        if let Ok(mut stream) = stream {
            // Observed host for #advertise (Q1): connection peer, not the claim.
            let peer_host = stream
                .peer_addr()
                .map(|a| a.ip().to_string())
                .unwrap_or_else(|_| "0.0.0.0".into());
            if let Ok(stream_clone) = stream.try_clone() {
                let mut reader = BufReader::new(stream_clone);
                match read_capped_request_line(&mut reader, oodp::MAX_DISCOVER_RESPONSE_BYTES) {
                    Ok(CappedRequestLine::TooLarge) => {
                        let body = oodp::encode_response_reason(
                            oodp::OodpStatus::Rejected,
                            Some("request_too_large"),
                            None,
                            &source_id,
                            0,
                        );
                        let _ = stream.write_all(body.as_bytes());
                        let _ = stream.flush();
                        println!("OODP Request rejected: #request_too_large");
                    }
                    Ok(CappedRequestLine::Line(request)) => {
                        if let Ok(request) = String::from_utf8(request) {
                            let line = request.trim();
                            println!("OODP Request: {}", line);
                            let (body, log) =
                                oodp::serve_request(&engine, line, &source_id, &peer_host);
                            let _ = stream.write_all(body.as_bytes());
                            let _ = stream.flush();
                            println!("{}", log);
                        }
                    }
                    Ok(CappedRequestLine::Eof) | Err(_) => {}
                }
            }
        }
    }
    Ok(())
}

fn run_node_id() -> anyhow::Result<()> {
    // Same shape as `oo identity`: id line, then path. Mint/load on demand.
    let cur = cwd()?;
    let engine = engine_keeping_settings(&cur)?;
    let id = engine.node_id()?;
    let path = nlang_interpreter::Identity::node_key_path(&cur)?;
    // Force the key onto disk so the path we print is the key that exists.
    let _ = engine.node_identity()?;
    println!("{}", id);
    println!("path: {}", path.display());
    Ok(())
}

/// Mint an affiliation claim signed by the **operator** key for this node id.
/// Persists beside the node key (not under workspace `.oo/`).
fn run_node_affiliate(ttl_secs: Option<i64>) -> anyhow::Result<()> {
    use nlang_interpreter::oodp::{
        affiliation_claim_path, mint_affiliation_claim, MAX_AFFILIATION_LIFETIME_SECS,
    };

    let cur = cwd()?;
    let engine = engine_keeping_settings(&cur)?;
    // Node id for *this* workspace (minting a node key is allowed here —
    // affiliation is an actual network-identity need).
    let node_id = engine.node_id()?.to_string();
    let node_key_path = nlang_interpreter::Identity::node_key_path(&cur)?;
    let _ = engine.node_identity()?;

    // Operator key — same one `oo identity` reports (R1 / REAL_01 §7.5.2).
    let operator = engine.identity()?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let ttl = ttl_secs.unwrap_or(MAX_AFFILIATION_LIFETIME_SECS);
    if ttl <= 0 {
        anyhow::bail!("affiliation ttl must be positive");
    }
    if ttl > MAX_AFFILIATION_LIFETIME_SECS {
        anyhow::bail!(
            "affiliation ttl {ttl}s exceeds maximum {MAX_AFFILIATION_LIFETIME_SECS}s (30 days)"
        );
    }
    let expires = now + ttl;
    let claim =
        mint_affiliation_claim(&operator, &node_id, expires).map_err(|e| anyhow::anyhow!("{e}"))?;
    let path = affiliation_claim_path(&node_key_path);
    claim.write_file(&path).map_err(|e| {
        anyhow::anyhow!(
            "cannot write {}: {}",
            path.display(),
            oo::operator_io_reason(&e)
        )
    })?;

    // Probe R1 parses whitespace tokens: 128-hex signature and a plausible expiry.
    println!("node: {}", node_id);
    println!("operator_key: {}", claim.operator_key);
    println!("signature: {}", claim.signature);
    println!("expires: {}", claim.expires);
    println!("path: {}", path.display());
    Ok(())
}

fn run_node_trust_list() -> anyhow::Result<()> {
    use nlang_interpreter::discovery_config::DiscoveryConfig;
    let cur = cwd()?;
    // Load via the same path as init; do not create the file.
    let cfg = DiscoveryConfig::load(&cur)?;
    for k in &cfg.affiliation_roots {
        println!("{k}");
    }
    Ok(())
}

fn run_node_trust_add(operator_key: String) -> anyhow::Result<()> {
    use nlang_interpreter::discovery_config::{validate_operator_key, DiscoveryConfig};
    // Validate before any write so a bad key never manufactures the file.
    validate_operator_key(&operator_key)?;
    let cur = cwd()?;
    let mut cfg = DiscoveryConfig::load(&cur)?;
    let _ = cfg.add(&operator_key)?;
    cfg.write(&cur)?;
    println!("added {operator_key}");
    Ok(())
}

fn run_node_trust_remove(operator_key: String) -> anyhow::Result<()> {
    use nlang_interpreter::discovery_config::{validate_operator_key, DiscoveryConfig};
    validate_operator_key(&operator_key)?;
    let cur = cwd()?;
    let mut cfg = DiscoveryConfig::load(&cur)?;
    let _ = cfg.remove(&operator_key)?;
    cfg.write(&cur)?;
    println!("removed {operator_key}");
    Ok(())
}

/// List known peers and verified affiliation operator keys (derived, not stored).
fn run_node_peers() -> anyhow::Result<()> {
    let cur = cwd()?;
    let engine = engine_keeping_settings(&cur)?;
    // Refresh derived affiliation from verbatim ad (R9: re-verify on every view).
    nlang_interpreter::peers::refresh_affiliations(&engine);
    let dir = engine
        .peer_adverts
        .read()
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let mut rows: Vec<_> = dir.values().collect();
    rows.sort_by(|a, b| a.node_id.cmp(&b.node_id));
    for adv in rows {
        // node_id (full CAID) so the digest tail is recoverable; operator only
        // when verified.
        match &adv.verified_operator_key {
            Some(op) => println!("{} operator {}", adv.node_id, op),
            None => println!("{}", adv.node_id),
        }
    }
    Ok(())
}

fn run_node_advertise(to: String, services: Vec<String>, listen_port: u16) -> anyhow::Result<()> {
    use nlang_interpreter::oodp;
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::time::Duration;

    let cur = cwd()?;
    let engine = engine_keeping_settings(&cur)?;
    let identity = engine.node_identity()?;
    let (_ad, _nid, req) =
        oodp::signed_advert_nlang(&identity, &services, listen_port, 10, 15, &engine)
            .map_err(|e| anyhow::anyhow!("{e}"))?;

    let addr: std::net::SocketAddr = to
        .parse()
        .map_err(|_| anyhow::anyhow!("--to must be host:port, got {to}"))?;
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(5)).map_err(|e| {
        if e.kind() == std::io::ErrorKind::TimedOut {
            anyhow::anyhow!("#peer_timeout")
        } else {
            anyhow::anyhow!("#peer_unreachable")
        }
    })?;
    let peer_end = |e: std::io::Error| anyhow::anyhow!("#{}", oodp::session_cause(&e).as_tag());
    stream
        .set_read_timeout(Some(oodp::OODP_READ_TIMEOUT))
        .map_err(peer_end)?;
    stream
        .set_write_timeout(Some(oodp::OODP_READ_TIMEOUT))
        .map_err(peer_end)?;
    stream.write_all(req.as_bytes()).map_err(peer_end)?;
    stream.flush().map_err(peer_end)?;
    let mut buf = Vec::new();
    match stream.read_to_end(&mut buf) {
        Ok(0) => anyhow::bail!("#peer_closed"),
        Ok(_) => {}
        Err(e) => return Err(peer_end(e)),
    }
    let text = String::from_utf8_lossy(&buf);
    if text.trim().is_empty() {
        anyhow::bail!("#peer_closed");
    }
    // Print status (+ reason when rejected) for the operator.
    if let Ok(j) = serde_json::from_str::<serde_json::Value>(text.trim()) {
        if let Some(s) = j.get("%status").and_then(|v| v.as_str()) {
            print!("{s}");
            if let Some(r) = j.get("%reason").and_then(|v| v.as_str()) {
                print!(" {r}");
            }
            println!();
            return Ok(());
        }
    }
    println!("{}", text.trim());
    Ok(())
}

fn run_node_discover(to: String, target: String) -> anyhow::Result<()> {
    use nlang_interpreter::oodp;

    let cur = cwd()?;
    let engine = engine_keeping_settings(&cur)?;
    let result = oodp::remote_discover_oodp(&engine, &to, &target)
        .map_err(|e| anyhow::anyhow!("#{}", e.as_tag()))?;

    // Record before any "accepted" sentence. A reply that did not land in
    // the directory is not a successful discover (SPEC_10 §2.2.1).
    let hops = result.envelope_hops;
    for p in &result.accepted {
        let addr = if p.observed_host.is_empty() {
            String::new()
        } else {
            format!("{}:{}", p.observed_host, p.listen_port)
        };
        engine.record_peer_advert(nlang_interpreter::PeerAdvert {
            node_id: p.node_id.clone(),
            public_key_hex: p.public_key_hex.clone(),
            services: Vec::new(),
            addr,
            observed_host: p.observed_host.clone(),
            listen_port: p.listen_port,
            capacity: 0,
            ttl: 15,
            ts: 0,
            hops: hops as i64,
            ad_source: p.ad_source.clone(),
            received_at: std::time::SystemTime::now(),
            verified_operator_key: p.verified_operator_key.clone(),
            provenance: nlang_interpreter::ObservationProvenance::Relayed,
            admission_seq: 0,
            received_at_unparseable: false,
            admission_seq_unparseable: false,
        })?;
    }

    let reasons: String = result
        .drop_reasons
        .iter()
        .map(|(k, n)| format!("{k}={n}"))
        .collect::<Vec<_>>()
        .join(",");
    let reasons = if reasons.is_empty() {
        "none".into()
    } else {
        reasons
    };
    eprintln!(
        "OODP Discover reply: peers={} accepted={} dropped={} ({}) stale_bound={}",
        result.peers_in,
        result.accepted.len(),
        result.dropped,
        reasons,
        oodp::DISCOVER_STALE_SECS
    );

    if result.status == "not_implemented" {
        println!("#not_implemented");
        return Ok(());
    }
    println!("#{}", result.status);
    for p in &result.accepted {
        println!(
            "{} {}:{} (host unverified, hops={hops} claimed)",
            p.node_id, p.observed_host, p.listen_port
        );
    }
    Ok(())
}

fn run_node_find_node(to: String, target: String) -> anyhow::Result<()> {
    use nlang_interpreter::oodp;

    let cur = cwd()?;
    let engine = engine_keeping_settings(&cur)?;
    let result = oodp::remote_find_node_oodp(&engine, &to, &target)
        .map_err(|e| anyhow::anyhow!("#{}", e.as_tag()))?;

    if result.status == "oversize" {
        println!("#oversize");
        return Ok(());
    }
    // v0.2.51 peers answer unknown ops with #conflict — surface cleanly.
    println!("#{}", result.status);
    let hops = result.envelope_hops;
    for p in &result.accepted {
        println!(
            "{} {}:{} (host unverified, hops={hops} claimed)",
            p.node_id, p.observed_host, p.listen_port
        );
    }
    Ok(())
}

fn run_status(universe: Option<PathBuf>) -> anyhow::Result<()> {
    let _hold = select_universe(universe.as_deref(), false)?;
    let current_dir = cwd()?;
    require_universe(&current_dir)?;
    let engine = Ouroboros::init(&current_dir)?;
    refuse_lost_context(&engine.store, &current_dir)?;
    if let Some(head) = engine.store.get_head(&current_dir)? {
        let commit = engine.store.get_commit(&head)?;
        match engine.store.root_standard_digest(&commit.root)? {
            Some(digest) => println!(
                "Standard root dependency: {digest} ({})",
                if engine.supports_standard_root(&digest) {
                    "available"
                } else {
                    "unavailable"
                }
            ),
            None => println!("Standard root dependency: self-contained (pre-sentinel)"),
        }
    } else {
        println!("Standard root dependency: current (no committed root yet)");
    }
    let universe = match load_universe(&engine, &current_dir) {
        Ok(universe) => universe,
        Err(error) => {
            anyhow::bail!("Universe unavailable: {error}");
        }
    };
    if let Some(d) = &universe.workset_bottom {
        // D49 / D33: both injections stay; the fold reports ⊥ at the
        // coordinate and the process must not exit 0.
        println!("Conflict");
        println!("{}", format_conflict_where(d, None));
        anyhow::bail!("{}", format_conflict_where(d, None))
    }
    if universe.is_dirty {
        println!("Staged changes:");
        println!("{}", Value::Combo(universe.staged.clone()).to_nlang(0));
        println!(
            "Total Logical Entropy: {} bits",
            Value::Combo(universe.staged).bits()
        );
    } else {
        println!("Universe is static (no staged changes).");
    }
    Ok(())
}

fn run_log(universe: Option<PathBuf>) -> anyhow::Result<()> {
    let _hold = select_universe(universe.as_deref(), false)?;
    let cur = cwd()?;
    require_universe(&cur)?;
    let engine = Ouroboros::init(&cur)?;
    refuse_lost_context(&engine.store, &cur)?;
    // A historical root that names an unavailable standard table is not an
    // empty universe. `log` is a read of that history, so surface the named
    // refusal instead of silently falling back to genesis.
    let _universe = Universe::load(&engine, &cur)?;
    // Surface CAS integrity failures distinctly (tampered commit chain).
    let history = engine
        .log(&cur)
        .map_err(|e| format_store_read_error(e, "HEAD chain"))?;
    for (hash, meta, kind) in history {
        println!("commit {}", hash);
        // SPEC_08 §6.2 audit markers as bare machine lines. Messages always
        // print as `message: …` so a human message cannot reproduce a marker
        // (privileged_effect_audit R4). history_ops pins `trim() == "squash"`.
        if kind == nlang_interpreter::CommitKind::Pin {
            println!("    pin");
        }
        if kind == nlang_interpreter::CommitKind::Squash {
            println!("    squash");
        }
        if meta.privileged_effect == Some(true) {
            println!("    privileged_effect");
        }
        if let Some(ref abs) = meta.abandoned {
            for a in abs {
                // R-b / local_gc §3.5: the fact survives; mark when content is gone.
                let present = ContentHash::parse(a)
                    .map(|h| nlang_interpreter::gc::content_present(&engine.store, &h))
                    .unwrap_or(false);
                if present {
                    println!("    abandoned {}", a);
                } else {
                    println!("    abandoned {} (content collected)", a);
                }
            }
        }
        // universe_determinism: refine authority status lives on RefineInfo
        // (not CommitMeta — Debug of meta is hashed into the commit CAID).
        //
        // This is a `match` and not `if let Ok(…)` ON PURPOSE: the latter is
        // the discarded-verdict shape REAL_03 §6.6 條款四 forbids, it was an
        // acceptance repair in the universe_determinism arc, and the record of
        // why was shortened away in this arc's delivery. Restored, because the
        // comment is the only thing standing between the next refactor and
        // reintroducing it.
        match engine.store.get_commit(&hash) {
            Ok(commit) => {
                if commit.refine_info.is_some() {
                    if let Some(line) = read_authority_line(&commit, hash.version) {
                        println!("    {line}");
                    }
                }
            }
            Err(e) => {
                eprintln!("    {}", format_store_read_error(e, &hash.to_string()));
            }
        }
        if let Some(msg) = meta.message {
            // ACCEPTOR REPAIR: EVERY line is prefixed, not just the first.
            // Measured on the delivered build, with no capability of any kind:
            //
            //   oo commit -m $'x\n    privileged_effect\n    pin'
            //   commit hash:sha256:v1:c7431d77…
            //       message: x
            //       privileged_effect        ← byte-identical to the marker
            //       pin                      ← byte-identical to the marker
            //
            // The `message: ` prefix protected the first line and emitted the
            // rest raw at whatever indentation the message chose. R4 asks that
            // a message cannot reproduce a marker; a message is not one line.
            for line in msg.lines() {
                println!("    message: {}", line);
            }
            if msg.is_empty() {
                println!("    message: ");
            }
        }
        // print_what_can_be_read (W8'-a): RFC-3339 UTC, not SystemTime Debug.
        println!("    Date: {}", format_commit_date_ms(meta.timestamp));
        println!();
    }
    Ok(())
}

/// What `oo log` may say about a refine. A signature that verifies is the
/// signer's public key. On a new-form commit the writer's stored word is
/// inside the address, so it is printed beside that key and named as the
/// writer's record. A legacy commit does not print the stored word: no
/// signature stays `unattested`.
fn read_authority_line(
    commit: &nlang_interpreter::Commit,
    version: nlang_interpreter::CaidVersion,
) -> Option<String> {
    let ri = commit.refine_info.as_ref()?;
    let new_form = version != nlang_interpreter::CaidVersion::V1;
    let recorded = if new_form {
        ri.authority_status.as_deref()
    } else {
        None
    };
    let head = if let Some((pk, coverage)) = nlang_interpreter::authority::signature_coverage(commit)
    {
        let what = match coverage {
            nlang_interpreter::authority::SignatureCoverage::Commit => "commit",
            nlang_interpreter::authority::SignatureCoverage::SourcesAndTargets => {
                "sources and targets"
            }
        };
        format!("refine authority: {pk} ({what})")
    } else if ri.authority.is_some() {
        "refine authority: signature did not verify".to_string()
    } else if new_form && recorded.is_some() {
        "refine authority:".to_string()
    } else if ri.authority_status.is_some() {
        return Some("refine authority: unattested".to_string());
    } else {
        return None;
    };
    match recorded {
        Some(word) => Some(format!("{head} (writer recorded: {word})")),
        None => Some(head),
    }
}

/// Commit meta timestamp is milliseconds since Unix epoch (see `run_commit`).
fn format_commit_date_ms(ms: u64) -> String {
    let secs = (ms / 1000) as i64;
    let nsec = ((ms % 1000) * 1_000_000) as u32;
    match chrono::DateTime::from_timestamp(secs, nsec) {
        Some(dt) => dt.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
        None => format!("{ms}"), // defensive; probe still requires a year when valid
    }
}

fn run_rollback(
    caid: String,
    grants: Vec<String>,
    privileged: bool,
    universe: Option<PathBuf>,
) -> anyhow::Result<()> {
    let _hold = select_universe(universe.as_deref(), false)?;
    let cur = cwd()?;
    require_universe(&cur)?;
    // HEAD is read in `Universe::load` and moved in `Universe::rollback`.
    // Both sit inside this section.
    let (_commit_lock, _) = CommitLock::acquire(&cur)?;
    let mut engine = Ouroboros::init(&cur)?;
    apply_cli_privilege(&mut engine, privileged, &grants)?;
    if !engine.privilege.rollback {
        anyhow::bail!(
            "#privileged_required: rollback requires --grant rollback (privilege.rollback capability)"
        );
    }
    let target =
        ContentHash::parse(&caid).map_err(|e| anyhow::anyhow!("Invalid CAID '{}': {}", caid, e))?;
    let mut universe = load_universe(&engine, &cur)?;
    universe.rollback(&engine, &cur, &target)?;
    println!("Rolled back to {}", target);
    Ok(())
}

fn run_squash(
    caid: String,
    grants: Vec<String>,
    privileged: bool,
    universe: Option<PathBuf>,
) -> anyhow::Result<()> {
    let _hold = select_universe(universe.as_deref(), false)?;
    let cur = cwd()?;
    require_universe(&cur)?;
    // `commits_after` and `Universe::squash` both read HEAD. The count in
    // the message is the chain this critical section squashes.
    let (_commit_lock, _) = CommitLock::acquire(&cur)?;
    let mut engine = Ouroboros::init(&cur)?;
    apply_cli_privilege(&mut engine, privileged, &grants)?;
    if !engine.privilege.squash {
        anyhow::bail!(
            "#privileged_required: squash requires --grant squash (privilege.squash capability)"
        );
    }
    let base = ContentHash::parse(&caid)
        .map_err(|e| anyhow::anyhow!("Invalid squash base CAID '{}': {}", caid, e))?;
    let mut universe = load_universe(&engine, &cur)?;
    // ACCEPTANCE REPAIR: the auto-message must not be the bare word "squash".
    // `oo log` prints the machine-set kind marker on its own line as
    // "    squash"; an identically-worded message renders a second, visually
    // indistinguishable line, so a reader (or a test) cannot tell whether the
    // AUDIT MARKER is present or only a human message that happens to say so.
    // An audit surface that cannot be verified by inspection is not an audit
    // surface. The message now states what was compressed.
    let squashed = universe.commits_after(&engine, &cur, &base).unwrap_or(0);
    let meta = CommitMeta {
        message: Some(format!("compressed {squashed} commit(s) onto {}", &caid)),
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis() as u64,
        author: Some("oo-cli".to_string()),
        abandoned: None,
        privileged_effect: None,
        reported_bottoms: None,
    };
    let hash = universe.squash(&engine, &cur, &base, meta)?;
    println!("Squash commit: {}", hash);
    Ok(())
}

/// D86. The HEAD CAID that holds `combos`, when every one of them is
/// already at that HEAD. `Ok(None)` is not that sentence.
fn already_in_head(
    engine: &Ouroboros,
    base: &Path,
    combos: &[nlang_interpreter::value::ComboVal],
) -> anyhow::Result<Option<String>> {
    match Universe::head_if_it_holds(engine, base, combos)? {
        Some(head) => Ok(Some(format!("already in HEAD {head}"))),
        None => Ok(None),
    }
}

fn run_commit(
    message: Option<String>,
    grants: Vec<String>,
    privileged: bool,
    universe: Option<PathBuf>,
) -> anyhow::Result<()> {
    let _hold = select_universe(universe.as_deref(), false)?;
    let cur = cwd()?;
    require_universe(&cur)?;
    let mut engine = Ouroboros::init(&cur)?;
    apply_cli_privilege(&mut engine, privileged, &grants)?;
    refuse_lost_context(&engine.store, &cur)?;
    // Before the lock: list, then read. A member that disappears between
    // those two reads was not verified. D86 is only for content this
    // process did read. G3 lists zero (the previous commit already returned).
    let listed_before = nlang_interpreter::injections::paths(&cur)?.len();
    let (seen_committable, unverified) = match nlang_interpreter::injections::load_all(&cur) {
        Ok(members) => {
            let seen = members
                .into_iter()
                .filter(|m| {
                    nlang_interpreter::universe::injection_has_committable_content(
                        &engine,
                        &m.combo,
                    )
                })
                .map(|m| m.combo)
                .collect::<Vec<_>>();
            (seen, false)
        }
        Err(e)
            if e.to_string()
                .starts_with(nlang_interpreter::injections::CONSUMED_MSG) =>
        {
            (Vec::new(), true)
        }
        Err(e) => return Err(e),
    };
    let (_commit_lock, contended) = CommitLock::acquire(&cur)?;
    // `acquire` returns only when the lock is on the current `.oo/format`.
    // A migrate that renamed the file while this commit waited is not the
    // store `engine` opened above. Read HEAD and the layout from this one.
    let mut engine = Ouroboros::init(&cur)?;
    apply_cli_privilege(&mut engine, privileged, &grants)?;
    refuse_lost_context(&engine.store, &cur)?;
    let listed_count = nlang_interpreter::injections::paths(&cur)?.len();
    let mut universe = load_universe(&engine, &cur)?;
    if let Some(d) = &universe.workset_bottom {
        anyhow::bail!("Evolution Conflict: {}", format_conflict_where(d, None));
    }
    // W4‴ (a): "dirty" for commit means content beyond `~%Config`. A stage
    // holding only horizon knobs is session state (O37) — reuse the existing
    // `Nothing to commit` path; knobs stay staged.
    if !universe.is_dirty
        || !nlang_interpreter::universe::staged_has_committable_content(&universe.staged)
    {
        // O37: a Config-only stage is honestly empty of committable
        // content. `listed_count > 0` is those knob members, not a race.
        // A member that is still here and folds to nothing committable
        // (a literal `_`) was not taken by another commit. Consumed is only
        // a wait, or a member that disappeared between the two listings.
        let config_only = universe.staged.get_field("~%Config").is_some();
        let lost_a_member = listed_count < listed_before;
        if config_only {
            anyhow::bail!("Nothing to commit");
        }
        // C2: the members are still here, and proposals_at already checked
        // them against this HEAD. Name that HEAD only when the check holds.
        let held = universe.held_committable().to_vec();
        if let Some(sentence) = already_in_head(&engine, &cur, &held)? {
            anyhow::bail!("{sentence}");
        }
        if contended || lost_a_member {
            // C1: content read before the wait, checked against the HEAD
            // that exists now. Unread content keeps the consumed answer.
            if !unverified {
                if let Some(sentence) = already_in_head(&engine, &cur, &seen_committable)? {
                    anyhow::bail!("{sentence}");
                }
            }
            anyhow::bail!("{}", nlang_interpreter::injections::CONSUMED_MSG);
        }
        anyhow::bail!("Nothing to commit");
    }
    // ACCEPTANCE REPAIR (privilege escalation, 2026-07-26): the commit is where
    // the privileged overwrite is APPLIED, so the capability must be presented
    // HERE, through the trusted channel — not inferred as authority from the
    // durable intent (layout 5 injection metadata, or a legacy
    // `.oo/pin_pending`). Trusting durable intent once let an unprivileged
    // program obtain #pin semantics and falsely mark its commit — exactly the
    // tokenless backdoor SPEC_08 §6.1.2 forbids.
    if universe.pin_pending && !engine.privilege.pin {
        anyhow::bail!(
            "#privileged_required: this commit applies a pinned overwrite; \
             re-present the capability (oo commit --grant pin)"
        );
    }
    // SPEC_08 §6.2 授權時點: commit fixes a discharge into history — must
    // re-present effect_override. Member `effect_tags` (and a leftover
    // `.oo/effect_pending` on layout ≤ 4) are intent only.
    //
    // ACCEPTOR REPAIR: the presented capability must COVER the tags actually
    // discharged, not merely exist. The delivered build checked `is_none()`,
    // so a discharge of `io` was authorised at commit by
    // `--grant effect_override:nondet` — a capability that would not have
    // authorised the discharge in the first place (SPEC_08 §6.1.4 axis 2,
    // `C ⊇ E`). Re-presenting *a* capability is not re-presenting *the*
    // capability.
    if let Some(discharged) = universe.effect_pending {
        let covered = engine
            .privilege
            .effect_override
            .map(|c| c.contains_all(discharged))
            .unwrap_or(false);
        if !covered {
            anyhow::bail!(
                "#privileged_required: this commit fixes privileged-discharged \
                 content into history (discharged {discharged}); re-present a \
                 capability covering it (oo commit --grant \
                 effect_override:<tags>)"
            );
        }
    }
    let meta = CommitMeta {
        message,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis() as u64,
        author: Some("oo-cli".to_string()),
        abandoned: None,
        privileged_effect: None, // set by Universe::commit from effect_pending
        reported_bottoms: None,  // set by Universe::commit from the projected root
    };
    let (hash, config_not_committed, reported) =
        universe.commit(&engine, &cwd()?, meta)?;
    // S2: name every leaf, then the success line. rc stays 0 (G2).
    // Same shape as format_conflict_where; never print `message`.
    for (coord, cause) in &reported {
        if coord.is_empty() {
            println!("{cause}");
        } else {
            println!("{cause} at {coord}");
        }
    }
    println!("Commit successful: {}", hash);
    // O37: horizon knobs are session state — never silent when dropped from history.
    if config_not_committed {
        println!(
            "note: ~%Config was not committed (horizon parameters stay staged as session state)"
        );
    }
    Ok(())
}

fn run_refine(
    sources: Vec<String>,
    targets: Vec<String>,
    sign: bool,
    message: Option<String>,
    universe: Option<PathBuf>,
) -> anyhow::Result<()> {
    let _hold = select_universe(universe.as_deref(), false)?;
    let cur = cwd()?;
    require_universe(&cur)?;
    // `refuse_lost_context`, `Universe::load`, and `Universe::refine` read
    // HEAD. Nothing above this lock reads the store except the witness check.
    let (_commit_lock, _) = CommitLock::acquire(&cur)?;
    let engine = Ouroboros::init(&cur)?;
    refuse_lost_context(&engine.store, &cur)?;
    let mut universe = load_universe(&engine, &cur)?;

    let source_caids: Vec<ContentHash> = sources
        .iter()
        .map(|s| ContentHash::parse(s).map_err(|e| anyhow::anyhow!("Invalid CAID '{}': {}", s, e)))
        .collect::<anyhow::Result<_>>()?;

    let target_caids: Vec<ContentHash> = targets
        .iter()
        .map(|s| ContentHash::parse(s).map_err(|e| anyhow::anyhow!("Invalid CAID '{}': {}", s, e)))
        .collect::<anyhow::Result<_>>()?;

    let identity = if sign {
        Some(
            engine
                .identity()
                .map_err(|e| anyhow::anyhow!("Signing failed: {}", e))?,
        )
    } else {
        None
    };
    // Layout 7 signs the commit after every field of V, including
    // authority_status, is fixed. Older declarations keep the source/target
    // signature and say so.
    let authority = if sign && !engine.store.signs_the_commit() {
        let payload =
            nlang_interpreter::authority::compute_refine_payload(&source_caids, &target_caids);
        let auth = nlang_interpreter::authority::sign_refine(
            &payload,
            identity.as_ref().expect("sign loaded the identity"),
        )
        .map_err(|e| anyhow::anyhow!("Signing failed: {}", e))?;
        println!(
            "note: this store signs sources and targets only; oo migrate --grant migrate signs the commit"
        );
        Some(auth)
    } else {
        None
    };
    let signer = if sign && engine.store.signs_the_commit() {
        identity.as_ref()
    } else {
        None
    };

    let meta = CommitMeta {
        message,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis() as u64,
        author: Some("oo-cli".to_string()),
        abandoned: None,
        privileged_effect: None,
        reported_bottoms: None,
    };

    let hash = universe.refine(
        &engine,
        &cur,
        source_caids,
        target_caids,
        authority,
        meta,
        signer,
    )?;
    println!("Refine commit: {}", hash);

    // Report shadow-affected commits (D5: do not swallow a failed read-back).
    match engine.store.get_commit(&hash) {
        Ok(commit) => {
            if let Some(ri) = commit.refine_info {
                // Write-time word, from the check just performed. The read
                // path (`oo log`) does not print this word.
                if ri.authority_status.is_some() {
                    if hash.version == nlang_interpreter::CaidVersion::V1 {
                        println!("Refine authority: unattested");
                    } else if let Some(ref status) = ri.authority_status {
                        println!("Refine authority: {}", status);
                    }
                }
                if let Some(a) = &ri.authority {
                    println!("Refine signer: {}", a.signer_pubkey_hex);
                }
                if !ri.shadow_affected.is_empty() {
                    println!(
                        "Shadow: {} historical commit(s) will be semantically updated:",
                        ri.shadow_affected.len()
                    );
                    for ch in &ri.shadow_affected {
                        println!("  {}", ch);
                    }
                }
            }
        }
        Err(e) => {
            eprintln!(
                "refine: failed to read back commit for shadow report: {}",
                format_store_read_error(e, &hash.to_string())
            );
        }
    }
    print_integrity_incidents(&engine);
    Ok(())
}

/// A path that reads a coordinate back. Bare `_`, `_|_`, `#_|_`, and `#_`
/// are literals in `resolve_path`, so those spellings are read from the
/// root (`_.…`) instead of the literal.
fn coordinate_path(segment: &str) -> nlang_parser::ast::Path {
    let segment = segment.trim().to_string();
    let literal = matches!(segment.as_str(), "_" | "_|_" | "#_|_" | "#_");
    nlang_parser::ast::Path {
        anchor: if literal {
            nlang_parser::ast::PathAnchor::Root
        } else {
            nlang_parser::ast::PathAnchor::Bare
        },
        segments: vec![segment],
        span: nlang_parser::ast::Span { start: 0, end: 0 },
    }
}

/// The path evolve stored this key at. `Err` means the line was accepted
/// and no coordinate holds its value (pattern, spread, or a path evolve
/// does not write).
fn repl_readback(key: &FieldKey) -> Result<nlang_parser::ast::Path, ()> {
    match key {
        FieldKey::Named { name, prefix } => {
            let trimmed = name.trim();
            let stored = match prefix {
                Some(nlang_parser::ast::Prefix::Logic) => format!("/{trimmed}"),
                Some(nlang_parser::ast::Prefix::Type) => format!("@{trimmed}"),
                Some(nlang_parser::ast::Prefix::Meta) => format!("%{trimmed}"),
                Some(nlang_parser::ast::Prefix::System) => format!("~%{trimmed}"),
                Some(nlang_parser::ast::Prefix::Private) => format!("~{trimmed}"),
                Some(nlang_parser::ast::Prefix::Local)
                | Some(nlang_parser::ast::Prefix::Data)
                | None => trimmed.to_string(),
            };
            Ok(coordinate_path(&stored))
        }
        FieldKey::Quoted(name) if name != "..." => Ok(coordinate_path(name)),
        FieldKey::Path(p) if p.anchor == nlang_parser::ast::PathAnchor::Bare => {
            if p.segments.len() == 1 {
                Ok(coordinate_path(&p.segments[0]))
            } else if p.segments.len() == 2 && p.segments[0].trim() == "~%Config" {
                Ok(p.clone())
            } else if p.segments.first().is_some_and(|s| s.trim().starts_with("~%")) {
                Err(())
            } else if !p.segments.is_empty() {
                // The dotted key landed as the nested combo. Read the leaf.
                Ok(p.clone())
            } else {
                Err(())
            }
        }
        _ => Err(()),
    }
}

fn print_observed(universe: &Universe, engine: &Ouroboros, path: &nlang_parser::ast::Path) -> Value {
    let res = universe.observe(engine, path);
    println!("=> {}", res.to_nlang(0));
    res
}

/// Where an observation ○ may be written. `--ephemeral` and a place with
/// no universe write none.
fn observation_root<'a>(engine: &Ouroboros, base: &'a Path, ephemeral: bool) -> Option<&'a Path> {
    if ephemeral || !engine.holds_universe {
        None
    } else {
        Some(base)
    }
}

fn answer_should_be_recorded(
    trace: &nlang_interpreter::observation::AnswerTrace,
    value: &Value,
) -> bool {
    trace.reduced_thunk()
        || trace.touched_horizon()
        || matches!(value, Value::Bottom(_))
        || value.contains_blur()
}

fn record_if_leaves(
    base: Option<&Path>,
    question: &str,
    value: &Value,
    trace: &nlang_interpreter::observation::AnswerTrace,
) -> anyhow::Result<()> {
    let Some(base) = base else {
        return Ok(());
    };
    if !answer_should_be_recorded(trace, value) {
        return Ok(());
    }
    nlang_interpreter::savepoint::record_observation(base, question, &value.to_nlang(0))?;
    Ok(())
}

fn during_answer<T>(f: impl FnOnce() -> T) -> (T, std::sync::Arc<nlang_interpreter::observation::AnswerTrace>) {
    let trace = nlang_interpreter::observation::AnswerTrace::new();
    let _guard = nlang_interpreter::observation::push_answer_trace(std::sync::Arc::clone(&trace));
    (f(), trace)
}

fn repl_unobserved(key: &FieldKey) -> String {
    match key {
        FieldKey::Pattern(e) => format!("@{{{}}}", e.to_nlang(0)),
        other => other.to_string_canonical(),
    }
}

fn staged_coord_names(universe: &Universe) -> Vec<String> {
    let mut keys = universe.staged.field_keys();
    for k in universe.staged.local.keys() {
        keys.push(format!("~{k}"));
    }
    keys
}

fn run_repl(universe_dir: Option<PathBuf>, ephemeral: bool) -> anyhow::Result<()> {
    let _hold = select_universe(universe_dir.as_deref(), ephemeral)?;
    let cur = cwd()?;
    let engine = engine_for_one_shot(&cur, universe_dir.is_some(), ephemeral)?;
    let mut universe = one_shot_view(&engine, &cur, universe_dir.is_some())?;
    let record_at = observation_root(&engine, &cur, ephemeral);
    let mut session: Vec<String> = Vec::new();
    println!("n/ Ouroboros REPL (Genesis)");
    println!("Type 'exit' to quit.");

    loop {
        print!("n> ");
        stdout().flush()?;
        let mut input = String::new();
        let bytes_read = stdin().read_line(&mut input)?;

        // If 0 bytes read, it's EOF
        if bytes_read == 0 {
            println!("\nGoodbye!");
            break;
        }

        let input = input.trim();
        if input == "exit" {
            break;
        }
        if input.is_empty() {
            continue;
        }

        match parse_program(input) {
            Ok(program) => {
                session.push(input.to_string());
                let transcript = session.join("\n");
                for f in &program.fields {
                    let spread = matches!(&f.key, FieldKey::Quoted(name) if name == "...");
                    let known = repl_readback(&f.key).is_ok();
                    let before = if spread || !known {
                        Some(staged_coord_names(&universe))
                    } else {
                        None
                    };
                    let (pairs, trace) = during_answer(|| -> Vec<(String, Value)> {
                        if let Err(e) = universe.evolve(&engine, &f) {
                            let fb = field_key_label(&f.key);
                            println!("{}", format_evolution_conflict(&e, Some(&fb)));
                            return Vec::new();
                        }
                        let mut pairs = Vec::new();
                        if spread || !known {
                            let after = staged_coord_names(&universe);
                            let before = before.unwrap_or_default();
                            let mut any = false;
                            for name in &after {
                                if !before.iter().any(|b| b == name) {
                                    any = true;
                                    let value = print_observed(
                                        &universe,
                                        &engine,
                                        &coordinate_path(name),
                                    );
                                    pairs.push((name.clone(), value));
                                }
                            }
                            if !any {
                                println!("no coordinate to observe: {}", repl_unobserved(&f.key));
                            }
                        } else if let Ok(path) = repl_readback(&f.key) {
                            let value = print_observed(&universe, &engine, &path);
                            pairs.push((path.to_key(), value));
                        }
                        pairs
                    });
                    for (coord, value) in pairs {
                        record_if_leaves(
                            record_at,
                            &format!("repl\n{coord}\n{transcript}"),
                            &value,
                            &trace,
                        )?;
                    }
                }
            }
            Err(error) => println!("Parse Error: {}", error),
        }
        // ACCEPTANCE REPAIR (peer-fetch arc). The work order named `oo repl`
        // among the commands that must drain the log; it was omitted. Drained
        // per input line, not at exit — an incident that only appears when the
        // session ends is not a report of the line that caused it.
        print_integrity_incidents(&engine);
    }
    Ok(())
}

/// Parse one `--grant SPEC` into a Privilege fragment. Loud fail on unknown.
fn parse_grant_spec(spec: &str) -> anyhow::Result<Privilege> {
    let s = spec.trim();
    match s {
        "pin" => Ok(Privilege {
            pin: true,
            ..Privilege::NONE
        }),
        "commit" => anyhow::bail!(
            "`--grant commit` is retired (SPEC_08 §6.2 2026-07-26): the \
             `#commit` operation had no gate; use pin/rollback/squash/\
             effect_override as needed"
        ),
        "rollback" => Ok(Privilege {
            rollback: true,
            ..Privilege::NONE
        }),
        "squash" => Ok(Privilege {
            squash: true,
            ..Privilege::NONE
        }),
        "gc" => Ok(Privilege {
            gc: true,
            ..Privilege::NONE
        }),
        "migrate" => Ok(Privilege {
            migrate: true,
            ..Privilege::NONE
        }),
        "connect" => Ok(Privilege {
            connect: true,
            ..Privilege::NONE
        }),
        "effect_override" => Ok(Privilege {
            effect_override: Some(EffectTag::all_active()),
            ..Privilege::NONE
        }),
        s if s.starts_with("effect_override:") => {
            let tags_s = &s["effect_override:".len()..];
            if tags_s.is_empty() {
                anyhow::bail!("unknown grant SPEC `{spec}`: empty tag list after effect_override:");
            }
            let mut tags = EffectTag::Pure;
            for part in tags_s.split('+') {
                let t = part.trim();
                let bit = match t {
                    "io" => EffectTag::IO,
                    "nondet" => EffectTag::NonDet,
                    "state" => EffectTag::State,
                    other => {
                        anyhow::bail!(
                            "unknown grant tag `{other}` in SPEC `{spec}` (allowed: io, nondet, state)"
                        );
                    }
                };
                tags = tags.union(bit);
            }
            Ok(Privilege {
                effect_override: Some(tags),
                ..Privilege::NONE
            })
        }
        _ => anyhow::bail!(
            "unknown grant SPEC `{spec}` (allowed: effect_override[:tag[+tag]*], pin, rollback, squash, gc, migrate, connect)"
        ),
    }
}

fn run_migrate(
    grants: Vec<String>,
    privileged: bool,
    universe: Option<PathBuf>,
) -> anyhow::Result<()> {
    let _hold = select_universe(universe.as_deref(), false)?;
    let cur = cwd()?;
    require_universe(&cur)?;
    let mut engine = Ouroboros::init(&cur)?;
    apply_cli_privilege(&mut engine, privileged, &grants)?;
    if !engine.privilege.migrate {
        anyhow::bail!(
            "#privileged_required: migrate requires --grant migrate (privilege.migrate capability)"
        );
    }
    let target = nlang_interpreter::storage::STORE_LAYOUT_VERSION;
    // A store that is already current does not replace `.oo/format`.
    // That answer stays available when the declaration can be read and
    // the critical section cannot (a mode-0400 file still reads).
    let already = |engine: &Ouroboros| -> anyhow::Result<Option<(String, u32, u32)>> {
        let declaration = nlang_interpreter::storage::read_layout_declaration(&cur)?;
        let from_enc = engine.store.encoding_version();
        let to_enc = nlang_interpreter::storage::encoding_after_migration(from_enc);
        let layout_done = nlang_interpreter::storage::layout_declaration_is_current(&declaration);
        let encoding_done = from_enc == to_enc;
        if layout_done && encoding_done {
            Ok(None)
        } else {
            Ok(Some((declaration, from_enc, to_enc)))
        }
    };
    if already(&engine)?.is_none() {
        let from_enc = engine.store.encoding_version();
        println!(
            "Store declarations are already layout={target} and encoding={from_enc}. Nothing was changed."
        );
        return Ok(());
    }
    // The rename of `.oo/format` is the critical section. Re-read after the
    // lock: the pre-lock declaration is not the one that is written.
    let (_commit_lock, _) = CommitLock::acquire(&cur)?;
    let mut engine = Ouroboros::init(&cur)?;
    apply_cli_privilege(&mut engine, privileged, &grants)?;
    let Some((declaration, from_enc, to_enc)) = already(&engine)? else {
        println!(
            "Store declarations are already layout={target} and encoding={}. Nothing was changed.",
            engine.store.encoding_version()
        );
        return Ok(());
    };
    let layout_done = nlang_interpreter::storage::layout_declaration_is_current(&declaration);
    let encoding_done = from_enc == to_enc;
    // REAL_02 §5.1.1: the cost is on the operator's screen before any
    // declaration byte is written. A later write failure still leaves it there.
    println!("{}", migrate_cost(&declaration, from_enc, to_enc));
    stdout().flush()?;
    engine.store.migrate_layout(&cur)?;
    if !layout_done && !encoding_done {
        println!("Migrated store layout to layout={target} and object encoding to encoding={to_enc}.");
    } else if !layout_done {
        println!("Migrated store layout to layout={target}.");
    } else {
        println!("Migrated object encoding to encoding={to_enc}.");
    }
    Ok(())
}

/// Engines that open this store today and will not open it after migration.
///
/// Source is `storage.rs` at each tag (`STORE_FORMAT_VERSION` through v0.21.0,
/// then `STORE_LAYOUT_VERSION` / `OBJECT_ENCODING_VERSION`), not a live run
/// of those binaries. v0.44.0 is the first tag that writes and opens layout 5;
/// its encoding max is already 5, so an encoding advance on a layout=5 store
/// locks out nobody. v0.28.0–v0.31.0 are in the git tags and match their
/// neighbours (layout 2, encoding max 4); they sit inside the ranges below.
///
/// Split-axis, oldest opener (then through v0.43.0, intersected with encoding):
///   layout 2: v0.22.0.  layout 3: v0.42.0.  layout 4: v0.43.0.  layout 5: v0.44.0.
///   encoding 1..=3: v0.22.0.  encoding 4: v0.26.0.  encoding 5: v0.36.0.
/// Newest engine that opens any of those and does not open layout 9 is v0.74.0.
/// layout 6 opens from v0.59.0. layout 7 opens from v0.61.0.
/// layout 8 opens from v0.64.0 (and v0.74.0 still writes it).
/// Bare number (the pre-split `.oo/format`), oldest opener:
///   1: v0.2.55 (exact `"1"`).  2: v0.20.0 (writes 2, reads 1..=2).
///   3: v0.21.0 (writes 3, reads 1..=3).  4: v0.26.0.  5: v0.36.0.
/// v0.22.0 onward still open a bare number whose value is inside their
/// encoding max. v0.19.0 and earlier open only bare `"1"`.
fn migrate_cost(declaration: &str, from_enc: u32, to_enc: u32) -> String {
    let target = nlang_interpreter::storage::STORE_LAYOUT_VERSION;
    let Some(oldest) = first_engine_that_opens(declaration, from_enc) else {
        return format!(
            "Advancing object encoding from encoding={from_enc} to encoding={to_enc} \
             locks out no engine."
        );
    };
    let newest = "v0.74.0";
    let who = if oldest == newest {
        format!("oo {oldest}")
    } else {
        format!("oo {oldest} through {newest}")
    };
    let from = if declaration.starts_with("layout=") {
        declaration.to_string()
    } else {
        format!("bare declaration {declaration}")
    };
    let encoding_note = if from_enc != to_enc {
        format!(
            " Object encoding advances from encoding={from_enc} to encoding={to_enc}."
        )
    } else {
        String::new()
    };
    // v0.44.0 through v0.58.0 open layout 5 and do not open layout 6.
    // Naming them on a layout=6 start would claim this migrate locks out
    // engines that could not open the store before it.
    let layout5_clause = if engine_ord(oldest) <= engine_ord("v0.58.0") {
        format!(
            " That includes every engine that opens layout=5 \
             (oo v0.44.0 through {newest}); none of them open layout={target}."
        )
    } else {
        String::new()
    };
    format!(
        "Migrating this store from {from} to layout={target} will make it unopenable \
         by {who}.{layout5_clause}{encoding_note}"
    )
}

/// Oldest tagged engine that opens this declaration. `None` when the
/// declaration is already layout 9 (only an encoding advance remains, and
/// this engine already reads encoding 1 through 5).
fn first_engine_that_opens(declaration: &str, enc: u32) -> Option<&'static str> {
    let layout = declaration
        .strip_prefix("layout=")
        .and_then(|n| n.parse::<u32>().ok());
    let floor = if let Some(layout) = layout {
        let by_layout = match layout {
            2 => "v0.22.0",
            3 => "v0.42.0",
            4 => "v0.43.0",
            5 => "v0.44.0",
            6 => "v0.59.0",
            7 => "v0.61.0",
            8 => "v0.64.0",
            _ => return None,
        };
        let by_encoding = match enc {
            0..=3 => "v0.22.0",
            4 => "v0.26.0",
            _ => "v0.36.0",
        };
        later_engine(by_layout, by_encoding)
    } else if declaration.parse::<u32>().is_ok() {
        match enc {
            1 => "v0.2.55",
            2 => "v0.20.0",
            3 => "v0.21.0",
            4 => "v0.26.0",
            _ => "v0.36.0",
        }
    } else {
        return None;
    };
    if engine_ord(floor) > engine_ord("v0.74.0") {
        None
    } else {
        Some(floor)
    }
}

fn later_engine<'a>(a: &'a str, b: &'a str) -> &'a str {
    if engine_ord(a) >= engine_ord(b) { a } else { b }
}

/// `vMAJOR.MINOR.PATCH` as a single integer so v0.2.55 sorts before v0.20.0.
fn engine_ord(v: &str) -> u32 {
    let mut parts = v.trim_start_matches('v').split('.');
    let major: u32 = parts.next().unwrap_or("0").parse().unwrap_or(0);
    let minor: u32 = parts.next().unwrap_or("0").parse().unwrap_or(0);
    let patch: u32 = parts.next().unwrap_or("0").parse().unwrap_or(0);
    major * 1_000_000 + minor * 1_000 + patch
}

fn run_gc(
    grants: Vec<String>,
    privileged: bool,
    dry_run: bool,
    universe: Option<PathBuf>,
) -> anyhow::Result<()> {
    let _hold = select_universe(universe.as_deref(), false)?;
    let cur = cwd()?;
    require_universe(&cur)?;
    let mut engine = Ouroboros::init(&cur)?;
    apply_cli_privilege(&mut engine, privileged, &grants)?;
    if !engine.privilege.gc {
        anyhow::bail!("#privileged_required: gc requires --grant gc (privilege.gc capability)");
    }

    match nlang_interpreter::gc::run_gc(&engine.store, &cur, dry_run) {
        Ok(report) => {
            print!("{}", nlang_interpreter::gc::format_plan_report(&report));
            if dry_run {
                println!("oo gc: dry-run — removed 0 objects, freed 0 bytes");
            } else {
                println!("{}", nlang_interpreter::gc::format_done_report(&report));
            }
            Ok(())
        }
        Err(e) => {
            // Walk incomplete: still surface the plan/integrity lines (diagnosis
            // must not vanish with the gate — verdict_must_gate P2 shape).
            if let Ok(plan) = nlang_interpreter::gc::plan_gc(&engine.store, &cur) {
                print!("{}", nlang_interpreter::gc::format_plan_report(&plan));
            }
            anyhow::bail!("{e}")
        }
    }
}

fn apply_cli_privilege(
    engine: &mut Ouroboros,
    privileged: bool,
    grants: &[String],
) -> anyhow::Result<()> {
    if privileged {
        engine.set_privileged(true);
    }
    for g in grants {
        let frag = parse_grant_spec(g)?;
        engine.grant_privilege(frag);
    }
    Ok(())
}

fn run_one_shot(
    files: Vec<PathBuf>,
    observe: Option<String>,
    format: bool,
    privileged: bool,
    grants: Vec<String>,
    universe_dir: Option<PathBuf>,
    ephemeral: bool,
) -> anyhow::Result<()> {
    let _hold = select_universe(universe_dir.as_deref(), ephemeral)?;
    let cur = cwd()?;
    let mut engine = engine_for_one_shot(&cur, universe_dir.is_some(), ephemeral)?;
    apply_cli_privilege(&mut engine, privileged, &grants)?;
    // Read-only view of the committed root. The working set is not loaded,
    // and nothing here stages or commits. SPEC_03 simultaneity: all
    // files/fields are one snapshot — evolve everything first, then
    // --observe. Automatic store-put stays off (cas_integrity R-2).
    // Explicit persistence remains `~%Engine./save`.
    let mut universe = one_shot_view(&engine, &cur, universe_dir.is_some())?;
    let record_at = observation_root(&engine, &cur, ephemeral);

    let mut sources: Vec<(String, String)> = Vec::new();
    let (outcome, trace) = during_answer(|| -> anyhow::Result<Option<Value>> {
        for file in &files {
            let input = oo::read_source_file(file).map_err(|m| anyhow::anyhow!("{m}"))?;
            sources.push((file.display().to_string(), input.clone()));
            let program = match parse_program(&input) {
                Ok(program) => program,
                Err(error) => {
                    return Err(anyhow::anyhow!("Parse Error in {:?}: {}", file, error));
                }
            };
            for f in &program.fields {
                if let Err(e) = universe.evolve(&engine, &f) {
                    let fb = field_key_label(&f.key);
                    anyhow::bail!(
                        "Evolution Conflict in \"{}\": {}",
                        file.display(),
                        format_conflict_where(&e, Some(&fb))
                    );
                }
            }
        }
        if let Some(path_str) = &observe {
            let path = parse_path_only(path_str)?;
            Ok(Some(universe.observe(&engine, &path)))
        } else if format {
            Ok(Some(Value::Combo(universe.staged.clone())))
        } else {
            Ok(None)
        }
    });
    if let Some(result) = outcome? {
        println!("{}", result.to_nlang(0));
        let mut question = String::from("run\n");
        if let Some(path_str) = &observe {
            question.push_str("observe ");
            question.push_str(path_str);
            question.push('\n');
        }
        if format {
            question.push_str("format\n");
        }
        for (path, text) in &sources {
            question.push_str(path);
            question.push('\n');
            question.push_str(text);
            if !text.ends_with('\n') {
                question.push('\n');
            }
        }
        record_if_leaves(record_at, &question, &result, &trace)?;
    }
    print_integrity_incidents(&engine);
    Ok(())
}

fn run_fmt(file: PathBuf, write: bool) -> anyhow::Result<()> {
    let input = oo::read_source_file(&file).map_err(|m| anyhow::anyhow!("{m}"))?;
    let mut program = match parse_program(&input) {
        Ok(program) => program,
        Err(error) => return Err(anyhow::anyhow!("Parse Error: {}", error)),
    };
    program.canonicalize();
    let formatted = program.to_nlang();
    if write {
        oo::write_source_file(&file, &formatted).map_err(|m| anyhow::anyhow!("{m}"))?;
    } else {
        let mut output = stdout();
        let _ = writeln!(output, "{}", formatted);
    }
    Ok(())
}

fn run_eval(
    expr: String,
    privileged: bool,
    grants: Vec<String>,
    universe_dir: Option<PathBuf>,
    ephemeral: bool,
) -> anyhow::Result<()> {
    let _hold = select_universe(universe_dir.as_deref(), ephemeral)?;
    let cur = cwd()?;
    let mut engine = engine_for_one_shot(&cur, universe_dir.is_some(), ephemeral)?;
    apply_cli_privilege(&mut engine, privileged, &grants)?;
    let mut universe = one_shot_view(&engine, &cur, universe_dir.is_some())?;
    let record_at = observation_root(&engine, &cur, ephemeral);
    let asked = expr.trim().to_string();

    let parsed_expr = match nlang_parser::parse_expr_only(expr.trim()) {
        Ok(expr) => expr,
        Err(error) => return Err(anyhow::anyhow!("Parse error: {}", error)),
    };

    let field = nlang_parser::ast::Field {
        key: nlang_parser::ast::FieldKey::Named {
            prefix: None,
            name: "__eval_result".to_string(),
        },
        value: parsed_expr,
        span: nlang_parser::ast::Span { start: 0, end: 0 },
    };

    let program = nlang_parser::ast::Program {
        fields: vec![field],
    };

    let (outcome, trace) = during_answer(|| -> anyhow::Result<Value> {
        for f in &program.fields {
            if let Err(e) = universe.evolve(&engine, f) {
                let fb = field_key_label(&f.key);
                anyhow::bail!("Eval error: {}", format_conflict_where(&e, Some(&fb)));
            }
        }
        let path = nlang_parser::ast::Path {
            anchor: nlang_parser::ast::PathAnchor::Bare,
            segments: vec!["__eval_result".to_string()],
            span: nlang_parser::ast::Span { start: 0, end: 0 },
        };
        Ok(universe.observe(&engine, &path))
    });
    let result = outcome?;
    println!("{}", result.to_nlang(0));
    record_if_leaves(record_at, &format!("eval\n{asked}"), &result, &trace)?;
    // ACCEPTANCE REPAIR (peer-fetch arc). §6.6 條款四 is not satisfied by the
    // verdict reaching the VALUE: when one source lies and another answers
    // correctly the value is right and the lie is the only trace. Every
    // command that evaluates n/ must drain the log.
    print_integrity_incidents(&engine);
    Ok(())
}

fn format_store_read_error(err: anyhow::Error, caid_str: &str) -> anyhow::Error {
    use nlang_interpreter::storage::StoreReadError;
    if let Some(sre) = err.downcast_ref::<StoreReadError>() {
        // Preserve the three distinct outcomes (R-4); do not flatten to "not found".
        return anyhow::anyhow!("{}", sre);
    }
    // Legacy / unexpected
    anyhow::anyhow!("store read failed for {}: {}", caid_str, err)
}

/// REAL_03 §6.6 條款四: surface integrity verdicts on stderr after evaluation,
/// even when a later peer answered correctly.
fn print_integrity_incidents(engine: &Ouroboros) {
    use nlang_interpreter::IntegrityKind;
    for inc in engine.take_integrity_incidents() {
        let kind = match inc.kind {
            IntegrityKind::Mismatch => "mismatch",
            IntegrityKind::Undecodable => "undecodable",
        };
        eprintln!(
            "integrity #{kind}: requested {} source={}{}",
            inc.requested,
            inc.source,
            if inc.source.contains("truncat") {
                " (shadow scan truncated)"
            } else {
                ""
            }
        );
    }
}

fn run_identity() -> anyhow::Result<()> {
    // Mint/load the operator identity (lazy path). Prints public key + path.
    // Distinct from `oo node id` (operator authorises; node answers on the wire).
    let path = nlang_interpreter::Identity::resolve_path()?;
    let id = nlang_interpreter::Identity::load_or_mint(&path)?;
    println!("{}", id.public_key_hex());
    println!("path: {}", path.display());
    Ok(())
}

fn run_inspect(caid_str: String, universe: Option<PathBuf>) -> anyhow::Result<()> {
    let _hold = select_universe(universe.as_deref(), false)?;
    let cur = cwd()?;
    require_universe(&cur)?;
    let engine = Ouroboros::init(&cur)?;

    let hash = ContentHash::parse(&caid_str)
        .map_err(|_| anyhow::anyhow!("Invalid CAID format: {}", caid_str))?;

    // CAS holds both values and commits. Try value first; fall back to commit
    // so address re-verification works for survivors after GC (local_gc R12).
    let value = match engine.store.get_value(&hash) {
        Ok(value) => Ok(value),
        // A format-3 root deliberately hashes its resolved standard-library
        // table, not the compact digest marker on disk. Retry as a root before
        // reporting an ordinary CAS mismatch.
        Err(original) => engine
            .store
            .get_root(&hash, &engine.standard_roots)
            .map(Value::Combo)
            .map_err(|_| original),
    };
    match value {
        Ok(val) => {
            let val = val.solidify_effects();
            println!("CAID:   {}", caid_str);
            println!("MASA:   {}", hash.masa_ref);
            if !hash.lattice_sketch.is_empty() {
                let sketch_preview = if hash.lattice_sketch.len() > 32 {
                    format!("{}...", &hash.lattice_sketch[..32])
                } else {
                    hash.lattice_sketch.clone()
                };
                println!("Sketch: {}", sketch_preview);
            }
            println!();
            // O68 Q4.C: a queryable mark on user objects that carry
            // meta.builtin. Display only — does not refuse, rewrite, or
            // move any address. Standard-root objects are engine
            // projections by definition and are not marked.
            let digest = hex_digest(&hash.digest);
            if !engine.standard_roots.contains(&digest) && val.holds_meta_builtin() {
                println!("note: user-authored %builtin");
            }
            println!("{}", val.to_nlang(0));
            Ok(())
        }
        Err(e_val) => match engine.store.open_commit(&hash) {
            Ok((resolved, commit)) => {
                println!("CAID:   {}", resolved);
                println!("kind:   commit");
                // Commit.parent is unset on purpose (D18/D52). The predecessor
                // is the ○ ancestor note. Absence of that note is not "no parent".
                let own = hex_digest(&hash.digest);
                if let Some(p) = nlang_interpreter::savepoint::previous_commit(&cur, &commit, &own)?
                {
                    println!("parent: {p}");
                }
                println!("root:   {}", commit.root);
                Ok(())
            }
            Err(_) => Err(anyhow::anyhow!(
                "{}",
                format_store_read_error(e_val, &caid_str)
            )),
        },
    }
}

fn load_universe(engine: &Ouroboros, path: &Path) -> anyhow::Result<Universe> {
    let mut u = Universe::load(engine, path)?;
    u.load_staged(engine, path)?;
    Ok(u)
}

fn parse_path_only(s: &str) -> anyhow::Result<nlang_parser::ast::Path> {
    let expr = nlang_parser::parse_expr_only(s).map_err(|e| anyhow::anyhow!("{}", e))?;
    if let nlang_parser::ast::ExprKind::Path(p) = expr.kind {
        Ok(p)
    } else {
        Err(anyhow::anyhow!("Not a path"))
    }
}

fn run_test(
    static_only: bool,
    pattern: Option<String>,
    files: Vec<PathBuf>,
    universe_dir: Option<PathBuf>,
    ephemeral: bool,
) -> anyhow::Result<()> {
    let hold = select_universe(universe_dir.as_deref(), ephemeral)?;
    let mut all_files = Vec::new();
    for f in files {
        if f.is_dir() {
            collect_files(&f, &mut all_files);
        } else {
            all_files.push(f);
        }
    }

    let cur = cwd()?;
    let engine = engine_for_one_shot(&cur, universe_dir.is_some(), ephemeral)?;
    let record_at = observation_root(&engine, &cur, ephemeral);
    let mut passed = 0;
    let mut failed = 0;
    let mut skipped = 0;

    for file in all_files {
        let input = match oo::read_source_file(&file) {
            Ok(s) => s,
            Err(m) => {
                println!("FAIL: {:?} ({})", file, m);
                failed += 1;
                continue;
            }
        };
        let program = match parse_program(&input) {
            Ok(p) => p,
            Err(e) => {
                println!("FAIL: {:?} (Parse error: {})", file, e);
                failed += 1;
                continue;
            }
        };

        let mut universe = one_shot_view(&engine, &cur, universe_dir.is_some())?;

        let mut evolve_failed = false;
        for f in &program.fields {
            println!(
                "Evolving field: {}",
                match &f.key {
                    FieldKey::Named { name, .. } => name.clone(),
                    FieldKey::Quoted(q) => q.clone(),
                    FieldKey::Path(p) => p.to_key(),
                    _ => "unknown".to_string(),
                }
            );
            if let Err(e) = universe.evolve(&engine, f) {
                let fb = field_key_label(&f.key);
                println!(
                    "FAIL: {:?} ({})",
                    file,
                    format_evolution_conflict(&e, Some(&fb))
                );
                failed += 1;
                evolve_failed = true;
                break;
            }
        }
        if evolve_failed {
            continue;
        }

        let mut has_test = false;
        for f in &program.fields {
            let name = match &f.key {
                FieldKey::Named { name, .. } => name.clone(),
                FieldKey::Quoted(q) => q.clone(),
                FieldKey::Path(p)
                    if p.anchor == nlang_parser::ast::PathAnchor::Bare && p.segments.len() == 1 =>
                {
                    p.segments[0].clone()
                }
                _ => continue,
            };
            if !name.starts_with("test_") {
                continue;
            }

            if let Some(ref pat) = pattern {
                if !name.contains(pat) {
                    continue;
                }
            }
            has_test = true;

            if static_only {
                println!("PASS (static): {:?} - {}", file, name);
                passed += 1;
                continue;
            }

            let path = parse_path_only(&name)?;
            let (result, trace) = during_answer(|| universe.observe(&engine, &path));

            // SPEC_16 §2.2 (ruling B): PASS = definite fact decided by this
            // observation. FAIL = ⊥ / #false / #fail / Top (undetermined —
            // vacuous truth forbidden) / #blur (horizon undetermined).
            match &result {
                Value::Bottom(b) => {
                    println!("FAIL: {:?} - {} (%cause: {:?})", file, name, b.cause);
                    failed += 1;
                }
                Value::Atom(AtomKind::Tag(ref t), _, _) if t == "false" || t == "fail" => {
                    println!("FAIL: {:?} - {} (Returned #{})", file, name, t);
                    failed += 1;
                }
                Value::Top | Value::TopCaused { .. } => {
                    println!(
                        "FAIL: {:?} - {} (undetermined: observation decided nothing)",
                        file, name
                    );
                    failed += 1;
                }
                Value::Blur(d) => {
                    println!(
                        "FAIL: {:?} - {} (blur %cause: {})",
                        file,
                        name,
                        d.cause.as_str()
                    );
                    failed += 1;
                }
                _ => {
                    println!("PASS: {:?} - {}", file, name);
                    passed += 1;
                }
            }
            record_if_leaves(
                record_at,
                &format!("test\n{name}\n{}\n{input}", file.display()),
                &result,
                &trace,
            )?;
        }
        // ACCEPTANCE REPAIR (peer-fetch arc). Drained per file so an incident
        // is attributed to the file that caused it, and BEFORE the summary —
        // this function exits the process on failure, so anything left in the
        // log at the end would never be printed at all.
        print_integrity_incidents(&engine);
        if !has_test {
            skipped += 1;
        }
    }

    println!(
        "\nTest Summary: {} passed, {} failed, {} skipped files without tests",
        passed, failed, skipped
    );
    let failed_count = failed;
    drop(hold);
    if failed_count > 0 {
        std::process::exit(1);
    }
    Ok(())
}

#[cfg(test)]
mod cli_must_speak {
    use super::*;
    use clap::{Command, CommandFactory};

    fn walk(cmd: &Command, path: &str, silent: &mut Vec<String>) {
        for arg in cmd.get_arguments() {
            if arg.is_hide_set() {
                continue;
            }
            let name = arg.get_id().as_str();
            if name == "help" || name == "version" {
                continue;
            }
            let help = arg.get_help().map(|h| h.to_string()).unwrap_or_default();
            if help.trim().is_empty() {
                let spec = arg
                    .get_long()
                    .map(|s| format!("--{s}"))
                    .or_else(|| arg.get_short().map(|c| format!("-{c}")))
                    .unwrap_or_else(|| format!("<{name}>"));
                silent.push(format!("{path} {spec}"));
            }
        }
        for sub in cmd.get_subcommands() {
            if sub.get_name() == "help" {
                continue;
            }
            let here = if path.is_empty() {
                sub.get_name().to_string()
            } else {
                format!("{path} {}", sub.get_name())
            };
            let about = sub.get_about().map(|a| a.to_string()).unwrap_or_default();
            if about.trim().is_empty() {
                silent.push(format!("command `{here}`"));
            }
            walk(sub, &here, silent);
        }
    }

    #[test]
    fn every_command_flag_and_argument_has_help() {
        let mut silent = Vec::new();
        walk(&Cli::command(), "", &mut silent);
        assert!(
            silent.is_empty(),
            "CLI surface has undescribed entries (REAL_01 §1.3 fifth clause):\n{}",
            silent.join("\n")
        );
    }
}

fn collect_files(dir: &std::path::Path, files: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries {
            if let Ok(entry) = entry {
                let path = entry.path();
                if path.is_dir() {
                    collect_files(&path, files);
                } else if path.extension().and_then(|s| s.to_str()) == Some("n") {
                    files.push(path);
                }
            }
        }
    }
}
