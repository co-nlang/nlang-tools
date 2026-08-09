// A knob that does nothing (2026-08-09, pre-committed by work order:
// docs/a_knob_that_does_nothing_handover.md).
//
// ── What is on the floor ─────────────────────────────────────────────────
//
// Measured on v0.14.0 (`dev 9cff223`), every knob by the two-point method —
// same input, two comparable knob values, does the threshold move? Setting
// one value and seeing nothing happen is not a measurement: it cannot tell
// "the knob is dead" from "this input never reached that gate".
//
//   fuel                   ✓ works   5 → #fuel_exhausted / 100000 → _
//   strategy               ✓ works   #strict → ⊥ / #blur → #blur
//   timeout                ✗ dead    timeout: 1 (ms), a 2286 ms run completes
//   max_branches           ✗ dead    cap 2, all 11 branches survive
//   max_unification_depth  ✗ dead    threshold pinned at 256 for 8/64/256/4000
//   max_lifting_depth      ✗ never read at all  — O39, out of scope
//   max_pattern_nodes      ✗ never read at all  — O39, out of scope
//
// And none of the seven survives a commit: every `~%Config.<knob>` evolves
// fine and then dies at `oo commit` with a bare `Error: Commit failed`,
// while `x: 1` in the same repo commits cleanly.
//
// ── What this arc decided ────────────────────────────────────────────────
//
// O37: horizon parameters do not enter history. The only reason to preserve
// them is CAID consistency, and that is already recorded *in* the CAID —
// SPEC_08 §3.2.1 requires a `#blur`'s CAID to contain its horizon params.
// Landing form (b): the commit proceeds, `~%Config` stays staged, and the
// engine SAYS it was not committed. Not (a) "the whole commit fails" — an
// ordinary `x: 1` must not be punished for sharing a file with a knob.
// Saying so is not optional: silence is allowed for "no obligation" (D36),
// never for "something you wrote was dropped".
//
// O38: fix the dead knobs, no transition. Five knobs have never worked, so
// nobody can be relying on their not working.
//
// ── What these probes are not ────────────────────────────────────────────
//
// Not an implementation of `max_lifting_depth` / `max_pattern_nodes` (O39:
// carried as acknowledged-unimplemented, not abolished). P3 exists so they
// are not quietly dropped from the knob table on the way past.
//
// Not a fix for `Error: Commit failed` — that message is W3′-b. Note the
// causality: this arc removes W3′-b's only known live case.
//
// ── The pin that is the ruling ───────────────────────────────────────────
//
// P2 is O37 in executable form: the same `x: 1`, with and without a
// `~%Config` line beside it, must commit to the same root CAID. If horizon
// params ever enter the committed root, P2 goes red immediately.

use std::fs;
use std::path::Path;
use std::process::Command;

// ── harness ─────────────────────────────────────────────────────────────

fn fresh(tag: &str) -> nlang_interpreter::ScratchDir {
    let d = nlang_interpreter::ScratchDir::new(&format!("knob-{tag}"));
    let out = Command::new(env!("CARGO_BIN_EXE_oo"))
        .arg("init")
        .current_dir(&*d)
        .env("OO_IDENTITY", d.join("identity-for-tests"))
        .env("OO_NODE_HOME", d.join("node-home-for-tests"))
        .output()
        .unwrap();
    let _ = out;
    d
}

fn oo_raw(dir: &Path, args: &[&str]) -> (String, bool) {
    let out = Command::new(env!("CARGO_BIN_EXE_oo"))
        .args(args)
        .current_dir(dir)
        .env("OO_IDENTITY", dir.join("identity-for-tests"))
        .env("OO_NODE_HOME", dir.join("node-home-for-tests"))
        .output()
        .unwrap();
    (
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
        out.status.success(),
    )
}

fn oo(dir: &Path, args: &[&str]) -> String {
    oo_raw(dir, args).0
}

/// `oo run u.n -o out` on a fresh repo holding `src`.
fn run_src(tag: &str, src: &str) -> String {
    let d = fresh(tag);
    fs::write(d.join("u.n"), src).unwrap();
    oo(&d, &["run", "u.n", "-o", "out"])
}

/// The root CAID of HEAD, or None if there is no commit.
fn head_root(dir: &Path) -> Option<String> {
    let log = oo(dir, &["log"]);
    let commit = log
        .split_whitespace()
        .find(|t| t.starts_with("hash:sha256:v1:"))?
        .to_string();
    let head = oo(dir, &["inspect", &commit]);
    head.lines()
        .find(|l| l.trim_start().starts_with("root:"))
        .and_then(|l| l.rsplit(':').next())
        .map(|s| s.trim().to_string())
}

/// A chain of `n` additions — the shape whose depth budget is at issue.
fn chain(n: usize) -> String {
    vec!["1"; n].join(" + ")
}

/// A union of `n` integers.
fn union(n: usize) -> String {
    (1..=n)
        .map(|i| i.to_string())
        .collect::<Vec<_>>()
        .join(" | ")
}

/// Smallest `n` in (lo, hi] whose addition chain no longer converges.
fn depth_threshold(knob: u64) -> usize {
    let (mut lo, mut hi) = (4usize, 400usize);
    while hi - lo > 1 {
        let mid = (lo + hi) / 2;
        let src = format!(
            "~%Config.max_unification_depth: {knob}\nout: {}\n",
            chain(mid)
        );
        let out = run_src(&format!("d{knob}-{mid}"), &src);
        if out.contains("#max_depth_exceeded") || out.contains("#blur") {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    lo
}

// ════════════════════════════════════════════════════════════════════════
//  CONTROL — green before and after
// ════════════════════════════════════════════════════════════════════════

/// C1 — the two knobs that already work must keep working.
///
/// This arc makes three dead knobs live. A delivery that rewired the config
/// path could easily kill the two live ones on the way; nothing else here
/// would notice.
#[test]
fn c1_the_working_knobs_keep_working() {
    let low = run_src("c1a", "~%Config.fuel: 5\nv: <<_.>>\nout: v.%cause\n");
    assert!(
        low.contains("#fuel_exhausted"),
        "fuel: 5 no longer exhausts: {low}"
    );
    let high = run_src("c1b", "~%Config.fuel: 100000\nv: <<_.>>\nout: v.%cause\n");
    assert!(
        !high.contains("#fuel_exhausted"),
        "fuel: 100000 still exhausts — the fuel knob stopped moving: {high}"
    );

    let strict = run_src(
        "c1c",
        "~%Config.fuel: 5\n~%Config.strategy: #strict\nv: <<_.>>\nout: v\n",
    );
    let blur = run_src(
        "c1d",
        "~%Config.fuel: 5\n~%Config.strategy: #blur\nv: <<_.>>\nout: v\n",
    );
    assert!(
        strict.contains("_|_"),
        "strategy #strict no longer yields ⊥: {strict}"
    );
    assert!(
        blur.contains("#blur"),
        "strategy #blur no longer yields #blur: {blur}"
    );
}

/// C2 — an ordinary commit still lands, and the "did it land" detector works.
#[test]
fn c2_ordinary_commit_lands_and_the_detector_is_armed() {
    let d = fresh("c2");
    fs::write(d.join("u.n"), "x: 1\n").unwrap();
    oo(&d, &["evolve", "u.n"]);
    let out = oo(&d, &["commit", "-m", "t"]);
    assert!(out.contains("hash:"), "an ordinary commit failed: {out}");

    let root = head_root(&d).expect("LIVENESS: no root CAID after a commit");
    assert_eq!(
        root.len(),
        64,
        "LIVENESS: root CAID is not a digest: {root}"
    );

    // Armed: a repo with different content must give a different root, or
    // the pins below would hold no matter what the delivery did.
    let e = fresh("c2b");
    fs::write(e.join("u.n"), "x: 2\n").unwrap();
    oo(&e, &["evolve", "u.n"]);
    oo(&e, &["commit", "-m", "t"]);
    let other = head_root(&e).expect("LIVENESS: no root CAID in the control repo");
    assert_ne!(
        root, other,
        "LIVENESS: two different universes hash the same — the root-CAID \
         detector cannot tell anything apart"
    );
}

// ════════════════════════════════════════════════════════════════════════
//  RED — one claim each
// ════════════════════════════════════════════════════════════════════════

/// R1 — `timeout` bites.
#[test]
fn r1_timeout_bites() {
    const HEAVY: &str = "~%Config.fuel: 5000000\nout: ~%List./range 1 200000 |> ~%List./sum\n";
    let tight = run_src("r1a", &format!("~%Config.timeout: 1\n{HEAVY}"));
    let loose = run_src("r1b", &format!("~%Config.timeout: 600000\n{HEAVY}"));

    assert!(
        loose.contains("19999900000"),
        "LIVENESS: the heavy computation does not complete even with a large \
         timeout, so a small one proves nothing: {loose}"
    );
    assert!(
        !tight.contains("19999900000"),
        "a 2.3-second computation completed under `timeout: 1` — the timeout \
         knob does nothing: {tight}"
    );
}

/// R2 — `max_branches` bites.
#[test]
fn r2_max_branches_bites() {
    let capped = run_src(
        "r2a",
        &format!("~%Config.max_branches: 2\nout: ({}) + 1\n", union(11)),
    );
    let open = run_src(
        "r2b",
        &format!("~%Config.max_branches: 64\nout: ({}) + 1\n", union(11)),
    );

    let count = |s: &str| s.matches('|').count();
    assert!(
        count(&open) >= 10,
        "LIVENESS: the uncapped branch arithmetic did not produce 11 branches: {open}"
    );
    assert!(
        count(&capped) < count(&open),
        "a cap of 2 left as many branches as a cap of 64 — max_branches does \
         nothing.\n  capped: {capped}\n  open:   {open}"
    );
}

/// R3 — `max_unification_depth` bites: two knob values, two thresholds.
#[test]
fn r3_depth_knob_moves_the_threshold() {
    let low = depth_threshold(16);
    let high = depth_threshold(300);
    assert!(
        low < high,
        "the depth threshold did not move with the knob (16 → {low}, \
         300 → {high}); measured baseline is 256 for every setting"
    );
}

/// R4 — a knob beside an ordinary write does not punish the write.
#[test]
fn r4_config_beside_a_write_still_commits() {
    let d = fresh("r4");
    fs::write(d.join("u.n"), "~%Config.fuel: 7\nx: 1\n").unwrap();
    let ev = oo(&d, &["evolve", "u.n"]);
    assert!(
        !ev.contains("Conflict"),
        "LIVENESS: the fixture did not even evolve: {ev}"
    );

    let out = oo(&d, &["commit", "-m", "t"]);
    assert!(
        out.contains("hash:"),
        "a commit failed because a horizon knob shared the file: {out}"
    );
    assert!(
        head_root(&d).is_some(),
        "LIVENESS: commit reported success but there is no HEAD root"
    );
}

/// R5 — and the engine says the knob was not committed, and keeps it staged.
///
/// Silence is allowed for "there is no obligation" (D36). It is not allowed
/// for "something you wrote was dropped".
#[test]
fn r5_the_engine_says_config_was_not_committed() {
    let d = fresh("r5");
    fs::write(d.join("u.n"), "~%Config.fuel: 7\nx: 1\n").unwrap();
    oo(&d, &["evolve", "u.n"]);
    let out = oo(&d, &["commit", "-m", "t"]);
    assert!(out.contains("hash:"), "LIVENESS: no commit: {out}");

    let said = out.contains("~%Config") || out.to_lowercase().contains("config");
    assert!(
        said,
        "the commit dropped `~%Config` and said nothing about it: {out}"
    );

    let status = oo(&d, &["status"]);
    assert!(
        status.contains("Config"),
        "`~%Config` is gone from staging too — it was silently discarded, \
         not held as session state: {status}"
    );
}

// ════════════════════════════════════════════════════════════════════════
//  PINS — green before and after
// ════════════════════════════════════════════════════════════════════════

/// P1 — this arc does not move identity.
#[test]
fn p1_plain_commit_root_is_unchanged() {
    const KNOWN: &str = "aa1b70f7c262cd9f0d80ada7d4f6c7bf2dc62b83ef8d3ca0fb642a6ff88f7ed1";
    let d = fresh("p1");
    fs::write(d.join("u.n"), "x: 1\n").unwrap();
    oo(&d, &["evolve", "u.n"]);
    oo(&d, &["commit", "-m", "t"]);
    assert_eq!(
        head_root(&d).as_deref(),
        Some(KNOWN),
        "the root CAID of a plain `x: 1` commit moved"
    );
}

/// P2 — **O37 in executable form.**
///
/// Same `x: 1`, with and without a `~%Config` line beside it: the committed
/// root must be the same CAID. Horizon parameters are observation conditions,
/// not universe content.
///
/// Today this holds vacuously — the `~%Config` side cannot commit at all. It
/// stops being vacuous the moment R4 goes green, and from then on it is the
/// only thing standing between "the knob is session state" and "the knob is
/// in your history".
#[test]
fn p2_config_never_enters_the_committed_root() {
    let plain = fresh("p2a");
    fs::write(plain.join("u.n"), "x: 1\n").unwrap();
    oo(&plain, &["evolve", "u.n"]);
    oo(&plain, &["commit", "-m", "t"]);
    let a = head_root(&plain).expect("LIVENESS: plain commit produced no root");

    let withcfg = fresh("p2b");
    fs::write(withcfg.join("u.n"), "~%Config.fuel: 7\nx: 1\n").unwrap();
    oo(&withcfg, &["evolve", "u.n"]);
    oo(&withcfg, &["commit", "-m", "t"]);
    match head_root(&withcfg) {
        // Before R4: the knob side cannot commit. Nothing to compare, and
        // that absence is itself the defect R4 names — not a pin failure.
        None => {}
        Some(b) => assert_eq!(
            a, b,
            "a horizon knob changed the committed root — `~%Config` entered \
             history (O37)"
        ),
    }
}

/// P3 — the knob table does not shrink.
///
/// O39 keeps `max_lifting_depth` and `max_pattern_nodes` as acknowledged-
/// unimplemented rather than abolished. They must still be accepted, not
/// quietly dropped while the neighbouring knobs are being wired up.
#[test]
fn p3_every_knob_name_is_still_accepted() {
    for knob in [
        "fuel: 100",
        "timeout: 100",
        "max_branches: 8",
        "max_unification_depth: 8",
        "max_lifting_depth: 8",
        "max_pattern_nodes: 8",
        "strategy: #blur",
    ] {
        let d = fresh(&format!("p3-{}", knob.split(':').next().unwrap()));
        fs::write(d.join("u.n"), format!("~%Config.{knob}\nx: 1\n")).unwrap();
        let out = oo(&d, &["evolve", "u.n"]);
        assert!(
            !out.contains("#invalid_config"),
            "`~%Config.{knob}` was rejected — the knob table shrank: {out}"
        );
    }
}
