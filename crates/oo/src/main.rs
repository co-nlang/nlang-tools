use nlang_interpreter::{
    Ouroboros, Universe, Value, ContentHash, CommitMeta, EffectTag, Privilege,
};
use nlang_parser::ast::{AtomKind, FieldKey};
use nlang_parser::parse_program;
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
use std::fs;
use std::io::{stdin, stdout, Write};

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
        /// SPEC: effect_override[:tag[+tag]*] | pin | commit | rollback | squash
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
    Test { #[arg(long)] static_only: bool, #[arg(short, long)] pattern: Option<String>, files: Vec<PathBuf> },
    Repl, Status, Log,
    Commit {
        #[arg(short, long)] message: Option<String>,
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
    Fmt { file: PathBuf, #[arg(short, long)] write: bool },
    Serve { #[arg(short, long, default_value_t = 8080)] port: u16 },
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
    /// Tier 1 linter (pure syntax / pure graph theory) — see docs/linter_tier1_handover.md
    Lint {
        /// .n file or directory (recursive)
        path: PathBuf,
        /// emit JSON (tier1-v1 schema)
        #[arg(long)]
        json: bool,
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
        Commands::Serve { port } => run_serve(port),
        Commands::Status => run_status(),
        Commands::Log => run_log(),
        Commands::Commit { message, grants, privileged } => {
            run_commit(message, grants, privileged)
        }
        Commands::Refine { source, target, sign, message } => run_refine(source, target, sign, message),
        Commands::Repl => run_repl(),
        Commands::Test { static_only, pattern, files } => run_test(static_only, pattern, files),
        Commands::Eval {
            expr,
            privileged,
            grants,
        } => run_eval(expr, privileged, grants),
        Commands::Inspect { caid } => run_inspect(caid),
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
        let program = parse_program(&input).map_err(|e| anyhow::anyhow!("Parse Error in {:?}: {}", file, e))?;
        for f in &program.fields {
            if let Err(e) = universe.evolve(&engine, &f) {
                anyhow::bail!("Evolution Conflict in {:?}: {:?} at {:?}", file, e, f.key);
            }
        }
    }
    universe.save_staged(&engine, &std::env::current_dir()?)?;
    Ok(())
}

fn run_serve(port: u16) -> anyhow::Result<()> {
    use std::io::{BufRead, BufReader};
    let listener = std::net::TcpListener::bind(format!("0.0.0.0:{}", port))?;
    let current_dir = std::env::current_dir()?;
    let engine = Ouroboros::init(&current_dir)?;
    println!("n/ Raw Mover serving truth at port {}", port);

    for stream in listener.incoming() {
        if let Ok(mut stream) = stream {
            if let Ok(stream_clone) = stream.try_clone() {
                let mut reader = BufReader::new(stream_clone);
                let mut request = String::new();
                if reader.read_line(&mut request).is_ok() {
                    let caid_str = request.trim();
                    println!("NDP Request for CAID: {}", caid_str);
                    if let Ok(caid) = ContentHash::parse(caid_str) {
                        if let Ok(val) = engine.store.get_value(&caid) {
                            if let Ok(json) = serde_json::to_string(&val) {
                                let _ = stream.write_all(json.as_bytes());
                                let _ = stream.flush();
                                println!("NDP Served: {}", caid_str);
                            }
                        } else {
                            println!("NDP Miss: {}", caid_str);
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn run_status() -> anyhow::Result<()> {
    let engine = Ouroboros::init(&std::env::current_dir()?)?;
    let universe = load_universe(&engine, &std::env::current_dir()?)?;
    if universe.is_dirty {
        println!("Staged changes:");
        println!("{}", Value::Combo(universe.staged.clone()).to_nlang(0));
        println!("Total Logical Entropy: {} bits", Value::Combo(universe.staged).bits());
    } else {
        println!("Universe is static (no staged changes).");
    }
    Ok(())
}

fn run_log() -> anyhow::Result<()> {
    let engine = Ouroboros::init(&std::env::current_dir()?)?;
    let _universe = load_universe(&engine, &std::env::current_dir()?)?;
    let history = engine.log()?;
    for (hash, meta, kind) in history {
        println!("commit {}", hash);
        // SPEC_08 §6.2 audit markers on the commit surface (never in values).
        if kind == nlang_interpreter::CommitKind::Pin {
            println!("    pin");
        }
        if kind == nlang_interpreter::CommitKind::Squash {
            println!("    squash");
        }
        if let Some(ref abs) = meta.abandoned {
            for a in abs {
                // "abandoned" substring is the probe-visible audit record (R1).
                println!("    abandoned {}", a);
            }
        }
        if let Some(msg) = meta.message { println!("    {}", msg); }
        let date = std::time::UNIX_EPOCH + std::time::Duration::from_millis(meta.timestamp);
        println!("    Date: {:?}", date);
        println!();
    }
    Ok(())
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
        .map_err(|e| anyhow::anyhow!("Invalid rollback CAID '{}': {}", caid, e))?;
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
        message: Some(format!(
            "compressed {squashed} commit(s) onto {}",
            &caid
        )),
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis() as u64,
        author: Some("oo-cli".to_string()),
        abandoned: None,
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
    if !universe.is_dirty { anyhow::bail!("Nothing to commit"); }
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
    let meta = CommitMeta {
        message,
        timestamp: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_millis() as u64,
        author: Some("oo-cli".to_string()),
        abandoned: None,
    };
    let hash = universe.commit(&engine, &std::env::current_dir()?, meta)?;
    println!("Commit successful: {}", hash);
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
        .map(|s| ContentHash::parse(s)
            .map_err(|e| anyhow::anyhow!("Invalid source CAID '{}': {}", s, e)))
        .collect::<anyhow::Result<_>>()?;

    let target_caids: Vec<ContentHash> = targets
        .iter()
        .map(|s| ContentHash::parse(s)
            .map_err(|e| anyhow::anyhow!("Invalid target CAID '{}': {}", s, e)))
        .collect::<anyhow::Result<_>>()?;

    let authority = if sign {
        let payload = nlang_interpreter::authority::compute_refine_payload(
            &source_caids,
            &target_caids,
        );
        let auth = nlang_interpreter::authority::sign_refine(&payload, &engine.identity)
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
    };

    let hash = universe.refine(&engine, &cur, source_caids, target_caids, authority, meta)?;
    println!("Refine commit: {}", hash);

    // Report shadow-affected commits
    if let Ok(commit) = engine.store.get_commit(&hash) {
        if let Some(ri) = commit.refine_info {
            if !ri.shadow_affected.is_empty() {
                println!("Shadow: {} historical commit(s) will be semantically updated:", ri.shadow_affected.len());
                for ch in &ri.shadow_affected {
                    println!("  {}", ch);
                }
            }
        }
    }
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
        if input == "exit" { break; }
        if input.is_empty() { continue; }

        match parse_program(input) {
            Ok(program) => {
                for f in &program.fields {
                    if let Err(e) = universe.evolve(&engine, &f) {
                        println!("Evolution Conflict: {:?}", e);
                    } else {
                        // 嘗試觀測剛剛進化的欄位
                        let path = match &f.key {
                            FieldKey::Named { name, .. } | FieldKey::Quoted(name) => {
                                nlang_parser::ast::Path { anchor: nlang_parser::ast::PathAnchor::Bare, segments: vec![name.clone()], span: nlang_parser::ast::Span::default() }
                            }
                            _ => continue,
                        };
                        let res = universe.observe(&engine, &path);
                        println!("=> {}", res.to_nlang(0));
                    }
                }
            }
            Err(e) => println!("Parse Error: {}", e),
        }
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
        "commit" => Ok(Privilege {
            commit: true,
            ..Privilege::NONE
        }),
        "rollback" => Ok(Privilege {
            rollback: true,
            ..Privilege::NONE
        }),
        "squash" => Ok(Privilege {
            squash: true,
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
            "unknown grant SPEC `{spec}` (allowed: effect_override[:tag[+tag]*], pin, commit, rollback, squash)"
        ),
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
    // One-shot: pure universe, no local staged load.
    // SPEC_03 simultaneity: all files/fields are one snapshot — evolve
    // everything first; only then store-put (CAID) and --observe.
    // Observing per-field mid-evolve solidifies reified thunks before later
    // fields land (false `_` on forward refs).
    let mut universe = Universe::new(None, engine.root_with_system());

    // Collect single-segment bare field paths for post-evolve store-put.
    let mut store_paths: Vec<nlang_parser::ast::Path> = Vec::new();

    for file in files {
        let input = fs::read_to_string(&file)?;
        let program = parse_program(&input).map_err(|e| anyhow::anyhow!("Parse Error in {:?}: {}", file, e))?;
        for f in &program.fields {
            if let Err(e) = universe.evolve(&engine, &f) {
                anyhow::bail!("Evolution Conflict in {:?}: {:?} at {:?}", file, e, f.key);
            }
            match &f.key {
                FieldKey::Named { name, .. } | FieldKey::Quoted(name) => {
                    store_paths.push(nlang_parser::ast::Path {
                        anchor: nlang_parser::ast::PathAnchor::Bare,
                        segments: vec![name.clone()],
                        span: nlang_parser::ast::Span::default(),
                    });
                }
                FieldKey::Path(p)
                    if p.anchor == nlang_parser::ast::PathAnchor::Bare && p.segments.len() == 1 =>
                {
                    store_paths.push(p.clone());
                }
                _ => {}
            }
        }
    }

    // Store-put after full evolve (purpose preserved: values in Store for CAID).
    for path in &store_paths {
        let val = universe.observe(&engine, path);
        let _ = engine.store.put_value(&val);
    }

    if let Some(path_str) = observe {
        let path = parse_path_only(&path_str)?;
        let result = universe.observe(&engine, &path);
        println!("{}", result.to_nlang(0));
    } else if format {
        println!("{}", Value::Combo(universe.staged).to_nlang(0));
    }
    Ok(())
}

fn run_fmt(file: PathBuf, write: bool) -> anyhow::Result<()> {
    let input = fs::read_to_string(&file)?;
    let mut program = parse_program(&input).map_err(|e| anyhow::anyhow!("Parse Error: {}", e))?;
    program.canonicalize();
    let formatted = program.to_nlang();
    if write { fs::write(file, formatted)?; } else { println!("{}", formatted); }
    Ok(())
}

fn run_eval(expr: String, privileged: bool, grants: Vec<String>) -> anyhow::Result<()> {
    let cur = std::env::current_dir()?;
    let mut engine = Ouroboros::init(&cur).unwrap_or_else(|_| Ouroboros::new_in_memory());
    apply_cli_privilege(&mut engine, privileged, &grants)?;

    let mut universe = Universe::new(None, engine.root_with_system());

    let parsed_expr = nlang_parser::parse_expr_only(expr.trim())
        .map_err(|e| anyhow::anyhow!("Parse error: {}", e))?;

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
            anyhow::bail!("Eval error: {:?}", e);
        }
    }

    let path = nlang_parser::ast::Path {
        anchor: nlang_parser::ast::PathAnchor::Bare,
        segments: vec!["__eval_result".to_string()],
        span: nlang_parser::ast::Span { start: 0, end: 0 },
    };
    let result = universe.observe(&engine, &path);
    println!("{}", result.to_nlang(0));
    Ok(())
}

fn run_inspect(caid_str: String) -> anyhow::Result<()> {
    let cur = std::env::current_dir()?;
    let engine = Ouroboros::init(&cur)
        .unwrap_or_else(|_| Ouroboros::new_in_memory());

    let hash = ContentHash::parse(&caid_str)
        .map_err(|_| anyhow::anyhow!("Invalid CAID format: {}", caid_str))?;

    let val = engine
        .store
        .get_value(&hash)
        .map_err(|_| anyhow::anyhow!("CAID not found in local store: {}", caid_str))?;
    // SPEC_08 §4.2.4: inspect is user-facing observation — solidify active
    // tags to #cached on the display projection (store object stays raw).
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

fn load_universe(engine: &Ouroboros, path: &Path) -> anyhow::Result<Universe> {
    let mut u = match Universe::load(engine, path) { Ok(u) => u, Err(_) => Universe::new(None, engine.root_with_system()), };
    let _ = u.load_staged(path);
    Ok(u)
}

fn parse_path_only(s: &str) -> anyhow::Result<nlang_parser::ast::Path> {
    let expr = nlang_parser::parse_expr_only(s).map_err(|e| anyhow::anyhow!("{}", e))?;
    if let nlang_parser::ast::ExprKind::Path(p) = expr.kind { Ok(p) } else { Err(anyhow::anyhow!("Not a path")) }
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
            println!("Evolving field: {}", match &f.key { FieldKey::Named { name, .. } => name.clone(), FieldKey::Quoted(q) => q.clone(), FieldKey::Path(p) => p.to_key(), _ => "unknown".to_string() });
            if let Err(e) = universe.evolve(&engine, f) {
                println!("FAIL: {:?} (Evolution error: {:?})", file, e);
                failed += 1;
                evolve_failed = true;
                break;
            }
        }
        if evolve_failed { continue; }
        
        let mut has_test = false;
        for f in &program.fields {
            let name = match &f.key {
                FieldKey::Named { name, .. } => name.clone(),
                FieldKey::Quoted(q) => q.clone(),
                FieldKey::Path(p) if p.anchor == nlang_parser::ast::PathAnchor::Bare && p.segments.len() == 1 => {
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
        if !has_test {
            skipped += 1;
        }
    }
    
    println!("\nTest Summary: {} passed, {} failed, {} skipped files without tests", passed, failed, skipped);
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
