// Every level of nesting doubles the universe (2026-08-10, pre-committed by
// work order: docs/every_level_doubles_the_universe_recon.md §8).
//
// ── What is on the floor ─────────────────────────────────────────────────
//
// Sixteen levels of ordinary nested combos do not finish. Peak RSS doubles
// per nesting level and converges on exactly 2.0:
//
//     nest=8    23,108 KB
//     nest=10   42,764 KB   x1.85
//     nest=12  128,560 KB   x1.82
//     nest=13  250,432 KB   x1.94
//     nest=14  503,684 KB   x2.01
//
// extrapolating to ~2 GB at sixteen levels and ~32 GB at twenty. Wall clock
// shows the same ratio (x2.15/level).
//
// It is not the parser — `oo fmt` on the same file takes 0.01 s — and not the
// node count: 200 fields in ONE level take 0.33 s, while a depth-8 binary tree
// of ~511 nodes does not finish. Cost is 2^depth for a chain and 4^depth when
// each level has two children.
//
// ── Mechanism, confirmed at runtime ──────────────────────────────────────
//
// `value.rs seal_defining_scope`:
//
//     let frame = c.clone();                        // deep-clones the subtree
//     Value::Thunk { closure, .. } => closure.push(frame.clone())
//
// and `Thunk.closure` is `Vec<ComboVal>` BY VALUE, not `Arc<ComboVal>`. The
// frame at level n already contains the frames pushed at level n-1, so
// size(n) = 2*size(n-1).
//
// An A/B spike gated that function behind an env var (a DIAGNOSTIC, never the
// proposed fix — sealing implements SPEC_04 §2.1/§3.1 lexical scope and cannot
// be removed). Same binary:
//
//     nest=16   seal on: timeout(20s)   seal off: 0.06 s
//     nest=40   seal on: timeout        seal off: 0.12 s
//
// ── What the fix is, and what it is not ──────────────────────────────────
//
// R-1: share the frames behind `Arc<ComboVal>`. Three things that could have
// blocked it are cleared: frames are never mutated after being pushed (no
// iter_mut / get_mut / index assignment anywhere on `scopes` or `closure`),
// Thunk equality compares closures structurally so `Arc` deref preserves it,
// and the durable form already holds no large closures so serde's `rc`
// feature changes no on-disk bytes.
//
// R-2, AND THIS IS WHY THE PROBES BELOW ASSERT COMPLETION RATHER THAN A
// GROWTH CURVE: `Arc` takes the cost from 2^n to about n^2, NOT to n. The
// frame clone still deep-copies that level's `ComboVal` structure. Reaching
// linear would mean `Arc`-ing `Value::Combo` itself, which is a different size
// of change and is NOT in this arc. A probe that asserted linearity would be
// asserting something the ruling does not require.
//
// R-3: sealing's EFFECT must not change. Only its cost may.
//
// ── Not in this arc ──────────────────────────────────────────────────────
//
// D2 — `%fuel` is not charged on this path at all (`fuel: 20` completes a
// fourteen-level nest that does ~2^14 units of work; `force_recursive` calls
// `check_resources` with the cost hard-coded to 0). Fixing D1 does not fix
// that, and fixing D2 alone would turn ordinary nesting into
// `#fuel_exhausted`. Separate arc, D1 first.

use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

// ── harness ─────────────────────────────────────────────────────────────

fn fresh(tag: &str) -> nlang_interpreter::ScratchDir {
    let d = nlang_interpreter::ScratchDir::new(&format!("nestdouble-{tag}"));
    let _ = Command::new(env!("CARGO_BIN_EXE_oo"))
        .arg("init")
        .current_dir(&*d)
        .env("OO_IDENTITY", d.join("identity-for-tests"))
        .env("OO_NODE_HOME", d.join("node-home-for-tests"))
        .output();
    d
}

fn oo(dir: &Path, args: &[&str]) -> String {
    let o = Command::new(env!("CARGO_BIN_EXE_oo"))
        .args(args)
        .current_dir(dir)
        .env("OO_IDENTITY", dir.join("identity-for-tests"))
        .env("OO_NODE_HOME", dir.join("node-home-for-tests"))
        .output()
        .unwrap();
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

/// Outcome of running `oo` under a wall-clock budget.
enum Ran {
    Done { out: String, took: Duration },
    OverBudget,
}

/// Run `oo` with a deadline, killing it if it overruns. Output goes to a file
/// so it can be read after the poll loop rather than through a pipe that
/// could fill and deadlock on a process we are about to kill.
fn run_within(dir: &Path, args: &[&str], budget: Duration) -> Ran {
    let sink: PathBuf = dir.join("probe-stdout");
    let f = File::create(&sink).unwrap();
    let started = Instant::now();
    let mut child = Command::new(env!("CARGO_BIN_EXE_oo"))
        .args(args)
        .current_dir(dir)
        .env("OO_IDENTITY", dir.join("identity-for-tests"))
        .env("OO_NODE_HOME", dir.join("node-home-for-tests"))
        .stdout(Stdio::from(f))
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    loop {
        match child.try_wait().unwrap() {
            Some(_) => {
                return Ran::Done {
                    out: fs::read_to_string(&sink).unwrap_or_default(),
                    took: started.elapsed(),
                }
            }
            None => {
                if started.elapsed() > budget {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Ran::OverBudget;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
}

/// Generous by design. Today these cases do not finish at all; after the fix
/// they run in ~0.1 s. Anything in between is not a boundary this arc has to
/// resolve, and a wide budget keeps the probe from failing on a loaded
/// machine.
const BUDGET: Duration = Duration::from_secs(15);

/// `z: {{a: {{a: ... 1 ... }}}}`, `depth` levels deep.
fn chain_src(depth: usize) -> String {
    let mut s = String::from("1");
    for _ in 0..depth {
        s = format!("{{{{a: {s}}}}}");
    }
    format!("z: {s}\n")
}

/// Each level holds TWO copies of the level below.
fn branch_src(depth: usize) -> String {
    let mut s = String::from("1");
    for _ in 0..depth {
        s = format!("{{{{a: {s}, b: {s}}}}}");
    }
    format!("z: {s}\n")
}

/// `outer: {{ k: 5, inner: {{ inner: … {{ v: k + 1 }} … }} }}` — `k` is
/// reachable only through the frames that sealing injects.
fn scope_src(levels: usize) -> (String, String) {
    let mut s = String::from("v: k + 1");
    for _ in 0..levels {
        s = format!("inner: {{{{ {s} }}}}");
    }
    let path = format!("outer{}.v", ".inner".repeat(levels));
    (format!("outer: {{{{ k: 5, {s} }}}}\nout: {path}\n"), path)
}

// ════════════════════════════════════════════════════════════════════════
//  CONTROL — green before and after
// ════════════════════════════════════════════════════════════════════════

/// C0 — lexical scope still resolves through sealed frames.
///
/// THIS IS THE CONTROL THE WHOLE ARC HANGS ON. Every red below is of the form
/// "this finishes", and the cheapest way to make a nested combo finish is to
/// stop sealing it — which is exactly what the diagnostic spike did to prove
/// where the cost lives. Sealing implements SPEC_04 §2.1/§3.1: `k` in the
/// fixture below is reachable ONLY through the frame injected into the inner
/// thunks. If this goes red, the delivery bought its speed with the semantics.
#[test]
fn c0_lexical_scope_survives_the_seal() {
    for levels in [1usize, 3, 6] {
        let d = fresh(&format!("c0-{levels}"));
        let (src, path) = scope_src(levels);
        fs::write(d.join("u.n"), &src).unwrap();
        let out = oo(&d, &["run", "u.n", "-o", "out"]);
        assert!(
            out.trim() == "6",
            "`{path}` did not resolve to 6 through {levels} sealed level(s); \
             got:\n{out}"
        );
    }
}

/// C1 — the wide common case is unaffected and still fast.
///
/// 200 fields in one level. The defect is per-DEPTH, not per-node, and a fix
/// that traded width for depth would be a bad trade — most real combos are
/// wide and shallow.
#[test]
fn c1_a_wide_combo_is_unaffected() {
    let d = fresh("c1");
    let fields: Vec<String> = (0..200).map(|i| format!("f{i}: {i}")).collect();
    fs::write(
        d.join("u.n"),
        format!("z: {{{{{}}}}}\nout: z.f199\n", fields.join(", ")),
    )
    .unwrap();
    match run_within(&d, &["run", "u.n", "-o", "out"], BUDGET) {
        Ran::Done { out, .. } => assert_eq!(out.trim(), "199", "wide combo: {out}"),
        Ran::OverBudget => panic!("a 200-field combo stopped finishing"),
    }
}

/// C2 — a depth that works today still produces the same value.
///
/// Forbids buying completion by not evaluating: the reds below would all pass
/// against an engine that answered `_` for anything deeply nested.
#[test]
fn c2_a_shallow_nest_still_has_its_value() {
    let d = fresh("c2");
    fs::write(d.join("u.n"), chain_src(8)).unwrap();
    let out = oo(&d, &["run", "--observe", "_.z", "u.n"]);
    let flat: String = out.chars().filter(|c| !c.is_whitespace()).collect();
    assert_eq!(
        flat, "{{a:{{a:{{a:{{a:{{a:{{a:{{a:{{a:1}}}}}}}}}}}}}}}}",
        "the value of an 8-level nest changed:\n{out}"
    );
}

// ════════════════════════════════════════════════════════════════════════
//  RED — must fail now, must pass after
// ════════════════════════════════════════════════════════════════════════

/// R1 — sixteen levels of ordinary nesting must finish, with the right value.
///
/// Baseline: does not finish (measured to 20 s; extrapolated peak RSS ~2 GB).
/// The value assertion rides along so completion alone cannot satisfy it.
#[test]
#[ignore = "D1: seal_defining_scope deep-clones frames; enable on delivery"]
fn r1_sixteen_levels_of_nesting_finish() {
    let d = fresh("r1");
    fs::write(d.join("u.n"), chain_src(16)).unwrap();
    match run_within(&d, &["run", "--observe", "_.z", "u.n"], BUDGET) {
        Ran::OverBudget => panic!("a 16-level nest did not finish within {BUDGET:?}"),
        Ran::Done { out, .. } => {
            let flat: String = out.chars().filter(|c| !c.is_whitespace()).collect();
            let expected = format!("{}1{}", "{{a:".repeat(16), "}}".repeat(16));
            assert_eq!(flat, expected, "16-level nest finished with a wrong value");
        }
    }
}

/// R2 — forty levels must finish. This is the MEMORY assertion.
///
/// Peak RSS doubles per level and was 504 MB at fourteen. At forty levels the
/// old shape needs on the order of 2^26 times that, so no machine completes
/// this without the fix — which is why a wall-clock budget is a sound proxy
/// for the memory bound and no RSS reading is taken here. (RSS was measured
/// directly during recon; see the work order.)
///
/// It also distinguishes the ruled fix from the unruled one: `Arc` gives
/// about n^2, and 40^2 is nothing. A probe demanding linearity would be
/// demanding more than the ruling grants.
#[test]
#[ignore = "D1: peak RSS doubles per nesting level; enable on delivery"]
fn r2_forty_levels_of_nesting_finish() {
    let d = fresh("r2");
    fs::write(d.join("u.n"), chain_src(40)).unwrap();
    match run_within(&d, &["run", "--observe", "_.z", "u.n"], BUDGET) {
        Ran::OverBudget => panic!("a 40-level nest did not finish within {BUDGET:?}"),
        Ran::Done { out, .. } => assert!(
            out.contains("{{"),
            "a 40-level nest finished without producing a combo:\n{out}"
        ),
    }
}

/// R3 — branching, where the same defect costs 4^depth instead of 2^depth.
///
/// Depth 8 with two children per level is ~511 nodes — fewer than C1's 200
/// fields by any reasonable count of work — and does not finish today. Keeping
/// this separate from R1 means a fix that only helped the single-child spine
/// cannot pass the arc.
#[test]
#[ignore = "D1: branching multiplies the same clone; enable on delivery"]
fn r3_a_branching_nest_finishes() {
    let d = fresh("r3");
    fs::write(d.join("u.n"), branch_src(8)).unwrap();
    match run_within(&d, &["run", "--observe", "_.z", "u.n"], BUDGET) {
        Ran::OverBudget => panic!("a depth-8 branching nest did not finish within {BUDGET:?}"),
        Ran::Done { out, .. } => assert!(
            out.contains("{{"),
            "a depth-8 branching nest finished without producing a combo:\n{out}"
        ),
    }
}
