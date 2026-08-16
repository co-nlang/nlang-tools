// A limit you cannot choose (2026-08-09, pre-committed by work order:
// docs/a_limit_you_cannot_choose_handover.md).
//
// ── What is on the floor ─────────────────────────────────────────────────
//
// v0.15.0 ships a crash we made. W4″ turned `max_unification_depth` from an
// inert knob into a live one; the operator can now set it past what the
// native stack survives:
//
//   depth 256 (default)  → exit 0, #blur { %cause: #max_depth_exceeded }
//   depth 488            → exit 0
//   depth 499 … 100000   → exit 134
//
//     thread 'oo-main' has overflowed its stack
//     fatal runtime error: stack overflow, aborting
//
// v0.7.0 at depth 4000 exits 0 — the knob was dead there, so the crash could
// not be reached. Attribution is ours.
//
// main.rs already runs the CLI on a 64 MiB thread, with a comment saying eval
// recursion "can exceed the default main-thread stack before the engine depth
// horizon engages". That mitigation held only because the horizon never moved.
// 64 MiB / ~490 frames ≈ **134 KB per frame** — the real cause is frame size,
// not stack size. Out of scope here (§9 of the work order), but recorded so
// nobody "fixes" this by asking for 512 MiB.
//
// ── The distinction this arc is about ────────────────────────────────────
//
// `max_unification_depth` is a POLICY the operator sets: "stop here."
// A stack overflow is an INCAPACITY: "I cannot go there."
// Reporting an incapacity under a policy's name is exactly what ERROR_CODES
// §2.7.1 ruled out last week, relocated. Note the consequence: §2.7.2 judged
// `#stack_overflow` a duplicate of `#max_depth_exceeded` and kept it out of
// the registry — that judgement does not survive this arc. They are the
// incapacity/policy pair, and the name that was going to be discarded is
// exactly the one now needed.
//
// ── What these probes are not ────────────────────────────────────────────
//
// Not a fix for frame size, and not a fix for O42.
//
// CORRECTED 2026-08-09 (O42 recon): this comment used to say a `#blur`'s CAID
// is `sha256(now_nanos)` outright. Measurement says the salt is fixed
// (`sha256("default")`) for blurs minted at OBSERVE, and a clock reading only
// for blurs minted at EVOLVE — one call site, `universe.rs`. So fuel-side
// blurs are reproducible today and unify-side ones are not. The original
// claim came from reading `storage.rs` without measuring its call site.
//
// R2 sidesteps O42 either way, by requiring that the hard limit never mint a
// blur at all: a blur claims an addressable snapshot, and an aborted stack
// has none.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

// ── harness ─────────────────────────────────────────────────────────────

fn fresh(tag: &str) -> nlang_interpreter::ScratchDir {
    let d = nlang_interpreter::ScratchDir::new(&format!("hardlimit-{tag}"));
    let _ = Command::new(env!("CARGO_BIN_EXE_oo"))
        .arg("init")
        .current_dir(&*d)
        .env("OO_IDENTITY", d.join("identity-for-tests"))
        .env("OO_NODE_HOME", d.join("node-home-for-tests"))
        .output();
    d
}

fn oo_out(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_oo"))
        .args(args)
        .current_dir(dir)
        .env("OO_IDENTITY", dir.join("identity-for-tests"))
        .env("OO_NODE_HOME", dir.join("node-home-for-tests"))
        .output()
        .unwrap()
}

fn oo(dir: &Path, args: &[&str]) -> String {
    let o = oo_out(dir, args);
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

fn chain(n: usize) -> String {
    vec!["1"; n].join(" + ")
}

/// Chain length for the two probes below that must reach the EVALUATOR's
/// ceilings.
///
/// ACCEPTOR EDIT (a_limit_you_cannot_catch, 2026-08-11). These were 5000. The
/// parser arc introduced an AST-depth ceiling of 4096, so a 5000-term chain is
/// now refused at PARSE and never reaches the evaluator at all:
///
///   * C1 below went red — nothing was staged, so there was no horizon to
///     report (`Universe is static`);
///   * R2 below stayed GREEN FOR THE WRONG REASON — it asserts the absence of
///     `#max_depth_exceeded` and `#blur`, and an empty universe contains
///     neither. The evaluator's `HARD_RECURSION_LIMIT` it was written to
///     exercise was no longer reached by it at all.
///
/// 1000 is under the parser ceiling (4096) and over both evaluator limits it
/// must cross — the default depth policy (256) for C1 and
/// `HARD_RECURSION_LIMIT` (400) for R2. Measured after the change: C1's fixture
/// reports `#max_depth_exceeded`, R2's reports `#stack_overflow` from the
/// evaluator. The 5000 was always incidental; what these probes pin is which
/// side of a limit the report comes from, not how long the chain is.
const EVALUATOR_REACHING_CHAIN: usize = 1000;

/// Evolve a deep addition chain under a given depth knob; return the raw
/// process outcome, because a crash has no value to inspect.
fn evolve_deep(tag: &str, knob: Option<u64>, terms: usize) -> Output {
    let d = fresh(tag);
    let mut src = String::new();
    if let Some(k) = knob {
        src.push_str(&format!("~%Config.max_unification_depth: {k}\n"));
    }
    src.push_str(&format!("big: {}\n", chain(terms)));
    fs::write(d.join("u.n"), src).unwrap();
    let out = oo_out(&d, &["evolve", "u.n"]);
    // keep the dir alive until after the call
    drop(d);
    out
}

fn crashed(o: &Output) -> bool {
    let err = String::from_utf8_lossy(&o.stderr);
    o.status.code() != Some(0) && o.status.code() != Some(1)
        || err.contains("overflowed its stack")
        || err.contains("fatal runtime error")
}

/// `oo run u.n -o out` on a fresh repo holding `src`.
fn run_src(tag: &str, src: &str) -> String {
    let d = fresh(tag);
    fs::write(d.join("u.n"), src).unwrap();
    oo(&d, &["run", "u.n", "-o", "out"])
}

fn head_root(dir: &Path) -> Option<String> {
    let log = oo(dir, &["log"]);
    let c = log
        .split_whitespace()
        .find(|t| t.starts_with("hash:sha256:v1:"))?
        .to_string();
    oo(dir, &["inspect", &c])
        .lines()
        .find(|l| l.trim_start().starts_with("root:"))
        .and_then(|l| l.rsplit(':').next())
        .map(|s| s.trim().to_string())
}

// ════════════════════════════════════════════════════════════════════════
//  CONTROL — green before and after
// ════════════════════════════════════════════════════════════════════════

/// C1 — the default-depth horizon still works, and still exits cleanly.
///
/// R1 asserts "does not crash". Without this, a delivery that made every deep
/// expression fail early would satisfy R1 while destroying the horizon.
#[test]
fn c1_default_depth_still_gives_a_horizon() {
    let o = evolve_deep("c1", None, 5000);
    assert!(
        !crashed(&o),
        "LIVENESS: the default-depth path itself crashes: {:?}",
        String::from_utf8_lossy(&o.stderr)
    );
    let d = fresh("c1b");
    fs::write(
        d.join("u.n"),
        format!("big: {}\n", chain(EVALUATOR_REACHING_CHAIN)),
    )
    .unwrap();
    oo(&d, &["evolve", "u.n"]);
    let st = oo(&d, &["status"]);
    assert!(
        st.contains("#max_depth_exceeded"),
        "the default-depth horizon stopped reporting #max_depth_exceeded: {st}"
    );
}

/// C2 — ordinary commit works; `Nothing to commit` still exists.
#[test]
fn c2_commit_paths_are_intact() {
    let d = fresh("c2");
    fs::write(d.join("u.n"), "x: 1\n").unwrap();
    oo(&d, &["evolve", "u.n"]);
    assert!(
        oo(&d, &["commit", "-m", "t"]).contains("hash:"),
        "an ordinary commit failed"
    );
    let again = oo(&d, &["commit", "-m", "t"]);
    assert!(
        again.contains("Nothing to commit"),
        "LIVENESS: the `Nothing to commit` path is gone — R3 would have no \
         message to expect: {again}"
    );
}

// ════════════════════════════════════════════════════════════════════════
//  RED — one claim each
// ════════════════════════════════════════════════════════════════════════

/// R1 — no knob value may end the process abnormally.
///
/// Measured on the exit status, not on a value: a `dump core` never gets to
/// answer a question about its `%cause`.
#[test]
fn r1_a_large_depth_knob_must_not_crash() {
    for knob in [499u64, 4000, 100_000] {
        let o = evolve_deep(&format!("r1-{knob}"), Some(knob), 5000);
        assert!(
            !crashed(&o),
            "max_unification_depth: {knob} ended the process abnormally \
             (code {:?}): {}",
            o.status.code(),
            String::from_utf8_lossy(&o.stderr)
        );
    }
}

/// R2 — the hard limit is not the policy limit, and never mints a blur.
#[test]
fn r2_the_hard_limit_has_its_own_name() {
    let d = fresh("r2");
    fs::write(
        d.join("u.n"),
        format!(
            "~%Config.max_unification_depth: 100000\n~%Config.strategy: #strict\nbig: {}\n",
            chain(EVALUATOR_REACHING_CHAIN)
        ),
    )
    .unwrap();
    let o = oo_out(&d, &["evolve", "u.n"]);
    assert!(
        !crashed(&o),
        "LIVENESS: still crashing, so there is no report to inspect"
    );
    let st = oo(&d, &["status"]);
    // ACCEPTOR HARDENING (2026-08-11). Every assertion below is an ABSENCE, so
    // an empty universe satisfied all of them — which is exactly how the parser
    // arc hollowed this probe out without turning it red. Assert the presence
    // of the report first: the evaluator's ceiling must actually have fired.
    assert!(
        st.contains("#stack_overflow"),
        "the evaluator's hard limit was never reached, so the assertions below \
         would be vacuous: {st}"
    );
    assert!(
        !st.contains("#max_depth_exceeded"),
        "an implementation limit is reported under the operator's policy \
         name: {st}"
    );
    assert!(
        !st.contains("#blur"),
        "stack exhaustion minted a #blur — a blur claims an addressable \
         snapshot and an aborted recursion has none (see O42): {st}"
    );
}

/// R3 — a stage holding only `~%Config` is nothing to commit.
#[test]
fn r3_config_only_stage_is_nothing_to_commit() {
    let d = fresh("r3");
    fs::write(d.join("u.n"), "~%Config.fuel: 7\n").unwrap();
    oo(&d, &["evolve", "u.n"]);
    let out = oo(&d, &["commit", "-m", "t"]);
    assert!(
        out.contains("Nothing to commit"),
        "a stage holding only a horizon knob minted a commit: {out}"
    );
    let st = oo(&d, &["status"]);
    assert!(
        st.contains("Config"),
        "the knob was dropped from staging — O37 says it stays: {st}"
    );
}

/// R4 — `#_` is accepted where the ruling allows it.
#[test]
fn r4_timeout_accepts_the_order_supremum() {
    let d = fresh("r4");
    fs::write(d.join("u.n"), "~%Config.timeout: #_\nx: 1\n").unwrap();
    let out = oo(&d, &["evolve", "u.n"]);
    assert!(
        !out.contains("#invalid_config"),
        "`~%Config.timeout: #_` was rejected: {out}"
    );
}

/// R5 — and refused where lifting the bound would remove the last guard.
///
/// The criterion, not a list: a knob may take `#_` only if every path it
/// governs still has another bound after it is lifted.
#[test]
fn r5_the_criterion_is_enforced_both_ways() {
    for knob in ["max_branches", "max_pattern_nodes"] {
        let d = fresh(&format!("r5a-{knob}"));
        fs::write(d.join("u.n"), format!("~%Config.{knob}: #_\nx: 1\n")).unwrap();
        let out = oo(&d, &["evolve", "u.n"]);
        assert!(
            !out.contains("#invalid_config"),
            "`{knob}: #_` was rejected, but lifting a width bound leaves fuel \
             and depth in force: {out}"
        );
    }
    for knob in ["fuel", "max_unification_depth", "max_lifting_depth"] {
        let d = fresh(&format!("r5b-{knob}"));
        fs::write(d.join("u.n"), format!("~%Config.{knob}: #_\nx: 1\n")).unwrap();
        let out = oo(&d, &["evolve", "u.n"]);
        assert!(
            out.contains("#invalid_config"),
            "`{knob}: #_` was accepted — lifting it removes the last bound on \
             a path it governs: {out}"
        );
    }
}

// ════════════════════════════════════════════════════════════════════════
//  PINS
// ════════════════════════════════════════════════════════════════════════

/// P1 — Q-032 deliberately moved root identity once by separating the
/// standard root; this remains a literal pin on the post-Q-032 address.
#[test]
fn p1_plain_commit_root_is_unchanged() {
    // ACCEPTOR (W4‴): this pin MOVED, and the delivery moved it — a violation
    // (probe edit rights are the acceptor's). The VALUE is correct; the fault
    // is the work order, which declared the arc non-breaking. Mechanism:
    // O41 rewrites the genesis `~%Config` (timeout 1000 → `#_`); `~%Config`
    // lives on the SYSTEM axis; `serialize_combo` folds `cv.system` into the
    // CAID (W8′ M2). So changing one genesis default moves EVERY root.
    // Breaking entry #10.
    const KNOWN: &str = "fcfcf264e4f52ca6241e207defaba25b71057440835c8bb70760e23b767b26a1";
    let d = fresh("p1");
    fs::write(d.join("u.n"), "x: 1\n").unwrap();
    oo(&d, &["evolve", "u.n"]);
    oo(&d, &["commit", "-m", "t"]);
    assert_eq!(head_root(&d).as_deref(), Some(KNOWN), "root CAID moved");
}

/// P2 — O37 holds: a knob beside a write does not change the committed root.
#[test]
fn p2_config_never_enters_the_committed_root() {
    let a = fresh("p2a");
    fs::write(a.join("u.n"), "x: 1\n").unwrap();
    oo(&a, &["evolve", "u.n"]);
    oo(&a, &["commit", "-m", "t"]);
    let ra = head_root(&a).expect("LIVENESS: plain commit produced no root");

    let b = fresh("p2b");
    fs::write(b.join("u.n"), "~%Config.fuel: 7\nx: 1\n").unwrap();
    oo(&b, &["evolve", "u.n"]);
    oo(&b, &["commit", "-m", "t"]);
    let rb = head_root(&b).expect("LIVENESS: config-beside-write produced no root");
    assert_eq!(ra, rb, "a horizon knob changed the committed root (O37)");
}

/// P3 — W4″'s three knobs stay live. Two points each.
#[test]
fn p3_the_knobs_w4pp_made_live_stay_live() {
    // depth: a chain that fits under 300 but not under 16
    let tight = run_src(
        "p3a",
        &format!("~%Config.max_unification_depth: 16\nout: {}\n", chain(200)),
    );
    let loose = run_src(
        "p3b",
        &format!("~%Config.max_unification_depth: 300\nout: {}\n", chain(200)),
    );
    assert!(
        tight.contains("#max_depth_exceeded"),
        "depth 16 stopped biting: {tight}"
    );
    assert!(
        loose.contains("200"),
        "depth 300 no longer completes a 200-term chain: {loose}"
    );

    // branches
    let u: String = (1..=11)
        .map(|i| i.to_string())
        .collect::<Vec<_>>()
        .join(" | ");
    let capped = run_src(
        "p3c",
        &format!("~%Config.max_branches: 2\nout: ({u}) + 1\n"),
    );
    let open = run_src(
        "p3d",
        &format!("~%Config.max_branches: 64\nout: ({u}) + 1\n"),
    );
    assert!(
        capped.matches('|').count() < open.matches('|').count(),
        "max_branches stopped biting.\n  capped: {capped}\n  open: {open}"
    );

    // timeout — explicit values only; the genesis default is the ruling's job
    const HEAVY: &str = "~%Config.fuel: 5000000\nout: ~%List./range 1 200000 |> ~%List./sum\n";
    let quick = run_src("p3e", &format!("~%Config.timeout: 1\n{HEAVY}"));
    let patient = run_src("p3f", &format!("~%Config.timeout: 600000\n{HEAVY}"));
    assert!(
        patient.contains("19999900000"),
        "LIVENESS: the heavy computation no longer completes: {patient}"
    );
    assert!(
        !quick.contains("19999900000"),
        "timeout: 1 stopped biting: {quick}"
    );
}
