use clap::{Parser, Subcommand};
use nlang_interpreter::value::{BottomCause, BottomDetail};
use nlang_interpreter::{
    CommitMeta, ContentHash, EffectTag, Ouroboros, Privilege, Universe, Value,
};
use nlang_parser::ast::{AtomKind, FieldKey};
use nlang_parser::{is_parser_nesting_limit_error, parse_program};
use std::fs;
use std::io::{stdin, stdout, Write};
use std::path::{Path, PathBuf};

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
    }
}

/// The parser fence is an incapacity boundary, so CLI parse entry points render
/// it as the language's Bottom and succeed cleanly instead of exposing a host
/// parser failure to the operator.
fn print_parser_stack_overflow() {
    println!("{}", Value::from(BottomCause::StackOverflow).to_nlang(0));
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

#[derive(Parser)]
#[command(author, version = env!("OO_VERSION"), about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Run {
        #[arg(required = true)]
        files: Vec<PathBuf>,
        #[arg(short, long)]
        observe: Option<String>,
        #[arg(short, long)]
        format: bool,
        /// Full §6 grant (back-compat: all operations + all active tags).
        /// Cannot be set from inside an n/ program (SPEC_08 §6.1.2).
        #[arg(long)]
        privileged: bool,
        /// Selective capability grant (repeatable; accumulates by union).
        /// SPEC: effect_override[:tag[+tag]*] | pin | commit | rollback | squash | gc
        #[arg(long = "grant", value_name = "SPEC", action = clap::ArgAction::Append)]
        grants: Vec<String>,
    },
    Evolve {
        #[arg(required = true)]
        files: Vec<PathBuf>,
        /// Request privileged overwrite of committed coordinates (SPEC_08 §6.2).
        /// Requires `--grant pin` (two-step: request + capability).
        #[arg(long)]
        pin: bool,
        /// Selective capability grant (repeatable). Same SPEC as `run --grant`.
        #[arg(long = "grant", value_name = "SPEC", action = clap::ArgAction::Append)]
        grants: Vec<String>,
    },
    Test {
        #[arg(long)]
        static_only: bool,
        #[arg(short, long)]
        pattern: Option<String>,
        files: Vec<PathBuf>,
    },
    Repl,
    Status,
    Log,
    Commit {
        #[arg(short, long)]
        message: Option<String>,
        /// ACCEPTANCE REPAIR: a pin-pending commit APPLIES the privileged
        /// overwrite, so the capability must be presented here too — the
        /// staged intent file is not authority (SPEC_08 §6.1.2).
        #[arg(long = "grant", value_name = "SPEC", action = clap::ArgAction::Append)]
        grants: Vec<String>,
        /// Full §6 grant (back-compat: all operations + all active tags).
        #[arg(long)]
        privileged: bool,
    },
    Refine {
        #[arg(short, long, required = true, num_args = 1..)]
        source: Vec<String>,
        #[arg(short, long, required = true, num_args = 1..)]
        target: Vec<String>,
        #[arg(long)]
        sign: bool,
        #[arg(short, long)]
        message: Option<String>,
    },
    Fmt {
        file: PathBuf,
        #[arg(short, long)]
        write: bool,
    },
    /// Universe node (REAL_01 §1.2 宇宙節點) — serve / later id, discover.
    Node {
        #[command(subcommand)]
        action: NodeCmd,
    },
    /// Evaluate a nlang expression inline
    Eval {
        /// nlang expression to evaluate (wrap in quotes for shell safety)
        expr: String,
        /// Full §6 grant (same as `run --privileged`).
        #[arg(long)]
        privileged: bool,
        /// Selective capability grant (repeatable; accumulates by union).
        #[arg(long = "grant", value_name = "SPEC", action = clap::ArgAction::Append)]
        grants: Vec<String>,
    },
    /// Inspect a value in the local store by CAID
    Inspect {
        /// CAID string (hash:sha256:v2:...)
        caid: String,
    },
    /// Move HEAD to a historical commit (SPEC_08 §6.2 `#rollback`).
    /// Requires `--grant rollback`. Does not create a commit; the next
    /// ordinary commit records the abandoned former HEAD in its meta.
    Rollback {
        /// Target commit CAID
        caid: String,
        #[arg(long = "grant", value_name = "SPEC", action = clap::ArgAction::Append)]
        grants: Vec<String>,
        #[arg(long)]
        privileged: bool,
    },
    /// Compress commits after BASE up to HEAD into one (SPEC_08 §6.2 `#squash`).
    /// Requires `--grant squash`. Parent of the result is BASE; root content
    /// is HEAD's root (universe unchanged). Marked `CommitKind::Squash`.
    Squash {
        /// Base commit CAID (survives as parent of the squashed commit)
        caid: String,
        #[arg(long = "grant", value_name = "SPEC", action = clap::ArgAction::Append)]
        grants: Vec<String>,
        #[arg(long)]
        privileged: bool,
    },
    /// Local store GC: remove unreachable objects under `.oo/objects/`.
    /// Requires `--grant gc`. Never automatic (local_gc / discussion 025).
    Gc {
        #[arg(long = "grant", value_name = "SPEC", action = clap::ArgAction::Append)]
        grants: Vec<String>,
        #[arg(long)]
        privileged: bool,
        /// Mark phase + report only; remove nothing.
        #[arg(long)]
        dry_run: bool,
    },
    /// Show the operator public key (64 hex) and the identity file path.
    /// Mints at `OO_IDENTITY` or `~/.oo/identity` on first use.
    Identity,
    /// Tier 1 linter (pure syntax / pure graph theory) — see docs/linter_tier1_handover.md
    Lint {
        /// .n file or directory (recursive)
        path: PathBuf,
        /// emit JSON (tier1-v1 schema)
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum NodeCmd {
    /// Serve OODP on TCP (REAL_02 §3.2). Request/response carry `%status`.
    Serve {
        #[arg(short, long, default_value_t = 8080)]
        port: u16,
    },
    /// Print this workspace's node id (CAID of the node public key) and key path.
    Id,
    /// Send a signed OODP `#advertise` to a peer and print `%status` / `%reason`.
    Advertise {
        /// Peer address `host:port`
        #[arg(long = "to", value_name = "HOST:PORT")]
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
        #[arg(long = "to", value_name = "HOST:PORT")]
        to: String,
        /// Service CAID to look up
        #[arg(long = "target", value_name = "CAID")]
        target: String,
    },
    /// Kademlia FIND_NODE: k closest known peers to a 160-bit id.
    #[command(name = "find-node")]
    FindNode {
        #[arg(long = "to", value_name = "HOST:PORT")]
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
        #[arg(value_name = "OPERATOR_KEY")]
        operator_key: String,
    },
    /// Remove an operator public key.
    Remove {
        #[arg(value_name = "OPERATOR_KEY")]
        operator_key: String,
    },
}

fn main() -> anyhow::Result<()> {
    // Eval recursion (morphism apply / left-deep math) can exceed the default
    // main-thread stack before the engine depth horizon engages. Interpreter
    // probes use 64 MiB threads; match that for the CLI entrypoint.
    const STACK: usize = 64 * 1024 * 1024;
    let handle = std::thread::Builder::new()
        .name("oo-main".into())
        .stack_size(STACK)
        .spawn(main_on_large_stack)
        .expect("spawn oo-main thread");
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
        } => run_one_shot(files, observe, format, privileged, grants),
        Commands::Evolve { files, pin, grants } => run_evolve(files, pin, grants),
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
        Commands::Status => run_status(),
        Commands::Log => run_log(),
        Commands::Commit {
            message,
            grants,
            privileged,
        } => run_commit(message, grants, privileged),
        Commands::Refine {
            source,
            target,
            sign,
            message,
        } => run_refine(source, target, sign, message),
        Commands::Repl => run_repl(),
        Commands::Test {
            static_only,
            pattern,
            files,
        } => run_test(static_only, pattern, files),
        Commands::Eval {
            expr,
            privileged,
            grants,
        } => run_eval(expr, privileged, grants),
        Commands::Inspect { caid } => run_inspect(caid),
        Commands::Identity => run_identity(),
        Commands::Rollback {
            caid,
            grants,
            privileged,
        } => run_rollback(caid, grants, privileged),
        Commands::Squash {
            caid,
            grants,
            privileged,
        } => run_squash(caid, grants, privileged),
        Commands::Gc {
            grants,
            privileged,
            dry_run,
        } => run_gc(grants, privileged, dry_run),
        Commands::Lint { path, json } => {
            let code = oo::nlint::run_cli(&path, json);
            std::process::exit(code);
        }
    }
}

fn run_evolve(files: Vec<PathBuf>, pin: bool, grants: Vec<String>) -> anyhow::Result<()> {
    let cur = std::env::current_dir()?;
    let mut engine = Ouroboros::init(&cur)?;
    // Reuse the same grant parser as run/eval — never a second code path.
    apply_cli_privilege(&mut engine, false, &grants)?;
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

    for file in files {
        let input = fs::read_to_string(&file)?;
        let program = match parse_program(&input) {
            Ok(program) => program,
            Err(error) if is_parser_nesting_limit_error(error.as_ref()) => {
                print_parser_stack_overflow();
                return Ok(());
            }
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
    universe.save_staged(&engine, &std::env::current_dir()?)?;
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
    let listener = std::net::TcpListener::bind(format!("0.0.0.0:{}", port))?;
    let bound_port = listener.local_addr()?.port();
    let current_dir = std::env::current_dir()?;
    let engine = Ouroboros::init(&current_dir)?;
    // %source = node id (CAID of the node public key), not the listen port.
    // Two ports on one workspace share one id; two workspaces do not.
    let source_id = engine.node_id()?.to_string();
    println!("n/ OODP node serving at port {} (node {})", bound_port, source_id);
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
    let cur = std::env::current_dir()?;
    let engine = Ouroboros::init(&cur)?;
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

    let cur = std::env::current_dir()?;
    let engine = Ouroboros::init(&cur)?;
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
    claim
        .write_file(&path)
        .map_err(|e| anyhow::anyhow!("write claim {}: {e}", path.display()))?;

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
    let cur = std::env::current_dir()?;
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
    let cur = std::env::current_dir()?;
    let mut cfg = DiscoveryConfig::load(&cur)?;
    let _ = cfg.add(&operator_key)?;
    cfg.write(&cur)?;
    println!("added {operator_key}");
    Ok(())
}

fn run_node_trust_remove(operator_key: String) -> anyhow::Result<()> {
    use nlang_interpreter::discovery_config::{validate_operator_key, DiscoveryConfig};
    validate_operator_key(&operator_key)?;
    let cur = std::env::current_dir()?;
    let mut cfg = DiscoveryConfig::load(&cur)?;
    let _ = cfg.remove(&operator_key)?;
    cfg.write(&cur)?;
    println!("removed {operator_key}");
    Ok(())
}

/// List known peers and verified affiliation operator keys (derived, not stored).
fn run_node_peers() -> anyhow::Result<()> {
    let cur = std::env::current_dir()?;
    let engine = Ouroboros::init(&cur)?;
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

    let cur = std::env::current_dir()?;
    let engine = Ouroboros::init(&cur)?;
    let identity = engine.node_identity()?;
    let (_ad, _nid, req) =
        oodp::signed_advert_nlang(&identity, &services, listen_port, 10, 15, &engine)
            .map_err(|e| anyhow::anyhow!("{e}"))?;

    let addr: std::net::SocketAddr = to
        .parse()
        .map_err(|_| anyhow::anyhow!("--to must be host:port, got {to}"))?;
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(5))?;
    stream.set_read_timeout(Some(oodp::OODP_READ_TIMEOUT))?;
    stream.set_write_timeout(Some(oodp::OODP_READ_TIMEOUT))?;
    stream.write_all(req.as_bytes())?;
    stream.flush()?;
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).ok();
    let text = String::from_utf8_lossy(&buf);
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

    let cur = std::env::current_dir()?;
    let engine = Ouroboros::init(&cur)?;
    let result = oodp::remote_discover_oodp(&engine, &to, &target)
        .map_err(|e| anyhow::anyhow!("discover transport: {e:?}"))?;

    // Operator log (§3.9).
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
    let hops = result.envelope_hops;
    for p in &result.accepted {
        // Parenthesis is required output (R-a at the human surface).
        println!(
            "{} {}:{} (host unverified, hops={hops} claimed)",
            p.node_id, p.observed_host, p.listen_port
        );
        // Affiliation path two of three: record the relayed peer (and claim
        // verdict) into this workspace's directory so `oo node peers` sees it.
        let addr = if p.observed_host.is_empty() {
            String::new()
        } else {
            format!("{}:{}", p.observed_host, p.listen_port)
        };
        // Services unknown from a bare discover entry — leave empty; the
        // durable record still carries the full ad_source for re-verify.
        let _ = engine.record_peer_advert(nlang_interpreter::PeerAdvert {
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
            // Receiver-local: learned via #discover, even if envelope hops is 0.
            provenance: nlang_interpreter::ObservationProvenance::Relayed,
            admission_seq: 0, // assigned in record_peer_advert
            received_at_unparseable: false,
            admission_seq_unparseable: false,
        });
    }
    Ok(())
}

fn run_node_find_node(to: String, target: String) -> anyhow::Result<()> {
    use nlang_interpreter::oodp;

    let cur = std::env::current_dir()?;
    let engine = Ouroboros::init(&cur)?;
    let result = oodp::remote_find_node_oodp(&engine, &to, &target)
        .map_err(|e| anyhow::anyhow!("find-node transport: {e:?}"))?;

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

fn run_status() -> anyhow::Result<()> {
    let current_dir = std::env::current_dir()?;
    let engine = Ouroboros::init(&current_dir)?;
    if let Some(head) = engine.store.get_head(&current_dir)? {
        let commit = engine.store.get_commit(&head)?;
        match engine.store.root_standard_digest(&commit.root)? {
            Some(digest) => println!(
                "Standard root dependency: {digest} ({})",
                if engine.supports_standard_root(&digest) { "available" } else { "unavailable" }
            ),
            None => println!("Standard root dependency: self-contained (pre-sentinel)"),
        }
    } else {
        println!("Standard root dependency: current (no committed root yet)");
    }
    let universe = match load_universe(&engine, &current_dir) {
        Ok(universe) => universe,
        Err(error) => {
            println!("Universe unavailable: {error}");
            return Ok(());
        }
    };
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

fn run_log() -> anyhow::Result<()> {
    let engine = Ouroboros::init(&std::env::current_dir()?)?;
    // A historical root that names an unavailable standard table is not an
    // empty universe. `log` is a read of that history, so surface the named
    // refusal instead of silently falling back to genesis.
    let _universe = Universe::load(&engine, &std::env::current_dir()?)?;
    // Surface CAS integrity failures distinctly (tampered commit chain).
    let history = engine
        .log()
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
                if let Some(ri) = commit.refine_info {
                    if let Some(ref status) = ri.authority_status {
                        println!("    refine authority: {}", status);
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

/// Commit meta timestamp is milliseconds since Unix epoch (see `run_commit`).
fn format_commit_date_ms(ms: u64) -> String {
    let secs = (ms / 1000) as i64;
    let nsec = ((ms % 1000) * 1_000_000) as u32;
    match chrono::DateTime::from_timestamp(secs, nsec) {
        Some(dt) => dt.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
        None => format!("{ms}"), // defensive; probe still requires a year when valid
    }
}

fn run_rollback(caid: String, grants: Vec<String>, privileged: bool) -> anyhow::Result<()> {
    let cur = std::env::current_dir()?;
    let mut engine = Ouroboros::init(&cur)?;
    apply_cli_privilege(&mut engine, privileged, &grants)?;
    if !engine.privilege.rollback {
        anyhow::bail!(
            "#privileged_required: rollback requires --grant rollback (privilege.rollback capability)"
        );
    }
    let target = ContentHash::parse(&caid)
        .map_err(|e| anyhow::anyhow!("Invalid CAID '{}': {}", caid, e))?;
    let mut universe = load_universe(&engine, &cur)?;
    universe.rollback(&engine, &cur, &target)?;
    println!("Rolled back to {}", target);
    Ok(())
}

fn run_squash(caid: String, grants: Vec<String>, privileged: bool) -> anyhow::Result<()> {
    let cur = std::env::current_dir()?;
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
    let squashed = universe.commits_after(&engine, &base).unwrap_or(0);
    let meta = CommitMeta {
        message: Some(format!("compressed {squashed} commit(s) onto {}", &caid)),
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis() as u64,
        author: Some("oo-cli".to_string()),
        abandoned: None,
        privileged_effect: None,
    };
    let hash = universe.squash(&engine, &cur, &base, meta)?;
    println!("Squash commit: {}", hash);
    Ok(())
}

fn run_commit(
    message: Option<String>,
    grants: Vec<String>,
    privileged: bool,
) -> anyhow::Result<()> {
    let mut engine = Ouroboros::init(&std::env::current_dir()?)?;
    apply_cli_privilege(&mut engine, privileged, &grants)?;
    let mut universe = load_universe(&engine, &std::env::current_dir()?)?;
    // W4‴ (a): "dirty" for commit means content beyond `~%Config`. A stage
    // holding only horizon knobs is session state (O37) — reuse the existing
    // `Nothing to commit` path; knobs stay staged.
    if !universe.is_dirty
        || !nlang_interpreter::universe::staged_has_committable_content(&universe.staged)
    {
        anyhow::bail!("Nothing to commit");
    }
    // ACCEPTANCE REPAIR (privilege escalation, 2026-07-26): the commit is where
    // the privileged overwrite is APPLIED, so the capability must be presented
    // HERE, through the trusted channel — not inferred from `.oo/pin_pending`.
    // That file records intent across two CLI processes; it is not authority.
    // It lives in a directory any n/ program can write (`~%Io./write_file`), so
    // trusting it let an entirely unprivileged program obtain #pin semantics and
    // falsely mark its commit — exactly the tokenless backdoor SPEC_08 §6.1.2
    // forbids. Demonstrated end to end before this repair.
    if universe.pin_pending && !engine.privilege.pin {
        anyhow::bail!(
            "#privileged_required: this commit applies a pinned overwrite; \
             re-present the capability (oo commit --grant pin)"
        );
    }
    // SPEC_08 §6.2 授權時點: commit fixes a discharge into history — must
    // re-present effect_override. `.oo/effect_pending` is intent only.
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
    };
    let (hash, config_not_committed) = universe.commit(&engine, &std::env::current_dir()?, meta)?;
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
) -> anyhow::Result<()> {
    let cur = std::env::current_dir()?;
    let engine = Ouroboros::init(&cur)?;
    let mut universe = load_universe(&engine, &cur)?;

    let source_caids: Vec<ContentHash> = sources
        .iter()
        .map(|s| {
            ContentHash::parse(s).map_err(|e| anyhow::anyhow!("Invalid CAID '{}': {}", s, e))
        })
        .collect::<anyhow::Result<_>>()?;

    let target_caids: Vec<ContentHash> = targets
        .iter()
        .map(|s| {
            ContentHash::parse(s).map_err(|e| anyhow::anyhow!("Invalid CAID '{}': {}", s, e))
        })
        .collect::<anyhow::Result<_>>()?;

    let authority = if sign {
        let payload =
            nlang_interpreter::authority::compute_refine_payload(&source_caids, &target_caids);
        // Sole engine consumer of the private key (identity_persistence).
        let identity = engine
            .identity()
            .map_err(|e| anyhow::anyhow!("Signing failed: {}", e))?;
        let auth = nlang_interpreter::authority::sign_refine(&payload, &identity)
            .map_err(|e| anyhow::anyhow!("Signing failed: {}", e))?;
        Some(auth)
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
    };

    let hash = universe.refine(&engine, &cur, source_caids, target_caids, authority, meta)?;
    println!("Refine commit: {}", hash);

    // Report shadow-affected commits (D5: do not swallow a failed read-back).
    match engine.store.get_commit(&hash) {
        Ok(commit) => {
            if let Some(ri) = commit.refine_info {
                if let Some(ref status) = ri.authority_status {
                    println!("Refine authority: {}", status);
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

fn run_repl() -> anyhow::Result<()> {
    let engine = Ouroboros::init(&std::env::current_dir()?)?;
    let mut universe = load_universe(&engine, &std::env::current_dir()?)?;
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
                for f in &program.fields {
                    if let Err(e) = universe.evolve(&engine, &f) {
                        let fb = field_key_label(&f.key);
                        println!("{}", format_evolution_conflict(&e, Some(&fb)));
                    } else {
                        // 嘗試觀測剛剛進化的欄位
                        let path = match &f.key {
                            FieldKey::Named { name, .. } | FieldKey::Quoted(name) => {
                                nlang_parser::ast::Path {
                                    anchor: nlang_parser::ast::PathAnchor::Bare,
                                    segments: vec![name.clone()],
                                    span: nlang_parser::ast::Span::default(),
                                }
                            }
                            _ => continue,
                        };
                        let res = universe.observe(&engine, &path);
                        println!("=> {}", res.to_nlang(0));
                    }
                }
            }
            Err(error) if is_parser_nesting_limit_error(error.as_ref()) => {
                print_parser_stack_overflow()
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
            "unknown grant SPEC `{spec}` (allowed: effect_override[:tag[+tag]*], pin, rollback, squash, gc, connect)"
        ),
    }
}

fn run_gc(grants: Vec<String>, privileged: bool, dry_run: bool) -> anyhow::Result<()> {
    let cur = std::env::current_dir()?;
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
) -> anyhow::Result<()> {
    let mut engine = Ouroboros::init(&std::env::current_dir()?)?;
    apply_cli_privilege(&mut engine, privileged, &grants)?;
    // One-shot: pure universe, no local staged load, no durable store writes.
    // SPEC_03 simultaneity: all files/fields are one snapshot — evolve
    // everything first, then --observe. Automatic store-put was removed
    // (cas_integrity R-2): it forced recursive types into multi-MB orphans
    // (SPEC_04 §158 / SPEC_12 #recursive_lazy) and contradicted "pure one-shot".
    // Explicit persistence remains `~%Engine./save`.
    let mut universe = Universe::new(None, engine.root_with_system());

    for file in files {
        let input = fs::read_to_string(&file)?;
        let program = match parse_program(&input) {
            Ok(program) => program,
            Err(error) if is_parser_nesting_limit_error(error.as_ref()) => {
                print_parser_stack_overflow();
                return Ok(());
            }
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

    if let Some(path_str) = observe {
        let path = parse_path_only(&path_str)?;
        let result = universe.observe(&engine, &path);
        println!("{}", result.to_nlang(0));
    } else if format {
        println!("{}", Value::Combo(universe.staged).to_nlang(0));
    }
    print_integrity_incidents(&engine);
    Ok(())
}

fn run_fmt(file: PathBuf, write: bool) -> anyhow::Result<()> {
    let input = fs::read_to_string(&file)?;
    let mut program = match parse_program(&input) {
        Ok(program) => program,
        Err(error) if is_parser_nesting_limit_error(error.as_ref()) => {
            print_parser_stack_overflow();
            return Ok(());
        }
        Err(error) => return Err(anyhow::anyhow!("Parse Error: {}", error)),
    };
    program.canonicalize();
    let formatted = program.to_nlang();
    if write {
        fs::write(file, formatted)?;
    } else {
        let mut output = stdout();
        let _ = writeln!(output, "{}", formatted);
    }
    Ok(())
}

fn run_eval(expr: String, privileged: bool, grants: Vec<String>) -> anyhow::Result<()> {
    let cur = std::env::current_dir()?;
    let mut engine = Ouroboros::init(&cur).unwrap_or_else(|_| Ouroboros::new_in_memory());
    apply_cli_privilege(&mut engine, privileged, &grants)?;

    let mut universe = Universe::new(None, engine.root_with_system());

    let parsed_expr = match nlang_parser::parse_expr_only(expr.trim()) {
        Ok(expr) => expr,
        Err(error) if is_parser_nesting_limit_error(error.as_ref()) => {
            print_parser_stack_overflow();
            return Ok(());
        }
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
    let result = universe.observe(&engine, &path);
    println!("{}", result.to_nlang(0));
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

fn run_inspect(caid_str: String) -> anyhow::Result<()> {
    let cur = std::env::current_dir()?;
    let engine = Ouroboros::init(&cur).unwrap_or_else(|_| Ouroboros::new_in_memory());

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
            println!("{}", val.to_nlang(0));
            Ok(())
        }
        Err(e_val) => match engine.store.get_commit(&hash) {
            Ok(commit) => {
                println!("CAID:   {}", caid_str);
                println!("kind:   commit");
                if let Some(p) = &commit.parent {
                    println!("parent: {}", p);
                } else {
                    println!("parent: (none)");
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
    let _ = u.load_staged(path);
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

fn run_test(static_only: bool, pattern: Option<String>, files: Vec<PathBuf>) -> anyhow::Result<()> {
    let mut all_files = Vec::new();
    for f in files {
        if f.is_dir() {
            collect_files(&f, &mut all_files);
        } else {
            all_files.push(f);
        }
    }

    let engine = Ouroboros::init(&std::env::current_dir()?)?;
    let mut passed = 0;
    let mut failed = 0;
    let mut skipped = 0;

    for file in all_files {
        let input = fs::read_to_string(&file)?;
        let program = match parse_program(&input) {
            Ok(p) => p,
            Err(e) => {
                println!("FAIL: {:?} (Parse error: {})", file, e);
                failed += 1;
                continue;
            }
        };

        let mut universe = Universe::new(None, engine.root_with_system());

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
            let result = universe.observe(&engine, &path);

            // SPEC_16 §2.2 (ruling B): PASS = definite fact decided by this
            // observation. FAIL = ⊥ / #false / #fail / Top (undetermined —
            // vacuous truth forbidden) / #blur (horizon undetermined).
            match result {
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
    if failed > 0 {
        std::process::exit(1);
    }
    Ok(())
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
