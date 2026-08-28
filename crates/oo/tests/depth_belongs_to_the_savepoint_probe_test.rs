// Depth belongs to the savepoint.
// Rulings: nlang-spec/meta/oo/STATUS.md D42-D47 (full text meta/oo/commit.md 1.10)
// Recon:   nlang-tools/docs/arc_a_recon.md (engine survey)
//          + acceptor readings in nlang-spec/meta/WORK_QUEUE.md 3
// Order:   nlang-tools/docs/depth_belongs_to_the_savepoint_handover.md
//
// -- What this arc is ----------------------------------------------------
//
// `oo evolve` is one action doing two jobs. Injection meets a definition
// into the universe; observation spends fuel converging a path. D42 split
// them. D46 then says how far anyone looked stays OUT of a commit unless
// the cell cannot be run again, and D47 says a savepoint is produced when
// injection moves the lattice position or observation actually reduces a
// thunk.
//
// Today the engine does the opposite of D46 (i): commit forces every thunk.
// Measured v0.37.0 -- staged holds `c: a + b`, the committed root holds
// `c: 3`. And when the thunk cannot be evaluated, forcing it writes `_`,
// which is the lattice identity, so the definition does not become wrong,
// it DISAPPEARS: supply the missing inputs afterwards and the field is gone
// from the next root entirely.
//
// D46 (ii) is ALREADY RIGHT and must stay right: an `#io` cell's forced
// value does enter the commit, tagged `#cached` (SPEC_08 4.2.4). G2 pins it.
//
// -- Out of scope, do not touch ------------------------------------------
//
//   * The standard root digest 7038e250... is a red line (G1).
//   * `~%Config` spelling. Both outcomes are legislated (SPEC_09 line 10):
//     the root path form is legal, the combo form is not. G3 pins the live
//     one. Do not "fix" the dead one.
//   * `BlurDetail.partial`'s misplacement (recon Q5). Named, not ordered.
//   * A2 (`Commit.parent`) and A3 (the log's canonical form).
//
// -- Probe integrity ------------------------------------------------------
//
// Reds are `#[ignore]`d and MUST be red at the baseline for the reason
// stated in each. The delivery may remove `#[ignore]` and NOTHING else in
// this file. If a pin here is wrong, say so in the report -- do not edit it.
//
// Every red asserts REACH before it asserts a value.
//
// NOT PROBED, stated so no one mistakes silence for coverage:
//   * S1, the savepoint layer itself. There is no savepoint entity in the
//     engine today (recon Q4: one file `.oo/staged`, no order, no identity,
//     deleted at commit), and its CLI surface is deliberately undesigned --
//     D42 puts the composite entry point in UX, and UX is out of this arc.
//     A probe would have to invent the surface it is meant to check. The
//     order asks for the on-disk shape and a runnable command in writing
//     instead (5.4 Q1).
//   * D47's two clauses. They are decidable only against S1, which has no
//     probe; asking for one here would pin an invented API. Same treatment
//     (5.4 Q2).
//
// Baseline measured 2026-08-28 against the v0.37.0 tag build: 5 green,
// 4 red. Every red fails at ITS OWN assertion, not at a REACH guard.

use std::path::Path;
use std::process::Command;

fn oo(dir: &Path, args: &[&str]) -> String {
    let mut c = Command::new(env!("CARGO_BIN_EXE_oo"));
    c.current_dir(dir)
        .env("OO_IDENTITY", dir.join("identity-for-tests"))
        .env("OO_NODE_HOME", dir.join("node-home-for-tests"));
    let o = c.args(args).output().expect("oo runs");
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

fn scratch(tag: &str) -> nlang_interpreter::ScratchDir {
    nlang_interpreter::ScratchDir::new(&format!("depth-{tag}"))
}

/// Evolve each source in order. Returns the directory.
fn universe(tag: &str, sources: &[&str]) -> nlang_interpreter::ScratchDir {
    let d = scratch(tag);
    for (i, src) in sources.iter().enumerate() {
        let f = format!("s{i}.n");
        std::fs::write(d.join(&f), src).unwrap();
        oo(&d, &["evolve", &f]);
    }
    d
}

/// Commit, then print the committed ROOT object. `oo eval` runs on a blank
/// universe and cannot read a committed field -- inspect is the only
/// authority here (recon Q3).
fn commit_and_read_root(d: &Path, msg: &str) -> String {
    let out = oo(d, &["commit", "-m", msg]);
    let commit = out
        .split_whitespace()
        .find(|w| w.starts_with("hash:"))
        .unwrap_or("")
        .to_string();
    if commit.is_empty() {
        return format!("COMMIT-FAILED: {out}");
    }
    let meta = oo(d, &["inspect", &commit]);
    let root = meta
        .lines()
        .find(|l| l.starts_with("root:"))
        .and_then(|l| l.split_whitespace().nth(1))
        .unwrap_or("")
        .to_string();
    if root.is_empty() {
        return format!("NO-ROOT: {meta}");
    }
    oo(d, &["inspect", &root])
}

fn flat(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

const STD_ROOT: &str = "7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911";

// ------------------------------------------------------------------------
// RED -- what the arc must make true.
// ------------------------------------------------------------------------

/// D46 (i). Split across two files so the thunk is still a thunk when
/// commit begins. Baseline: staged holds `c: a + b`, the root holds `c: 3`.
/// A pure cell's answer carries nothing its definition did not.
#[test]
#[ignore]
fn r1_a_pure_thunk_survives_the_commit() {
    let d = universe("r1", &["c: a + b\n", "a: 1\nb: 2\n"]);
    let staged = oo(&d, &["status"]);
    assert!(
        flat(&staged).contains("c: a + b"),
        "REACH: staging must still hold the definition before commit. got {staged:?}"
    );
    let root = flat(&commit_and_read_root(&d, "r1"));
    assert!(
        root.contains("a: 1"),
        "REACH: the commit must have produced a readable root. got {root:?}"
    );
    assert!(
        root.contains("c: a + b"),
        "D46 (i): a `#pure` cell keeps its definition in the commit. Storing \
         the answer instead stores a computable relation as an object -- what \
         commit.md 1.8 says not to do for DERIVED edges. got {root:?}"
    );
}

/// D46 (i), the half that loses data. Commit a definition the engine cannot
/// evaluate, THEN supply its inputs. Baseline: the first root holds `c: _`,
/// and because top is the lattice identity the field is simply gone from
/// the second root. The definition is unrecoverable.
#[test]
#[ignore]
fn r2_an_unevaluable_definition_is_not_destroyed_by_commit() {
    let d = universe("r2", &["c: a + b\n"]);
    let first = flat(&commit_and_read_root(&d, "r2-one"));
    assert!(
        !first.starts_with("COMMIT-FAILED") && !first.starts_with("NO-ROOT"),
        "REACH: the first commit must succeed. got {first:?}"
    );
    std::fs::write(d.join("later.n"), "a: 1\nb: 2\n").unwrap();
    oo(&d, &["evolve", "later.n"]);
    let second = flat(&commit_and_read_root(&d, "r2-two"));
    assert!(
        second.contains("a: 1") && second.contains("b: 2"),
        "REACH: the second commit must have taken the new inputs. got {second:?}"
    );
    assert!(
        second.contains("c: 3"),
        "A definition committed before its inputs existed must still be there \
         to be satisfied later. Baseline forces it to `_` at commit, and `_` \
         is the lattice identity, so the field vanishes on the next meet -- \
         silent data loss, not a wrong answer. got {second:?}"
    );
}

/// D46 (i) at the SECOND boundary. Top-level evolve already calls
/// `engine.eval` (universe.rs:389), so a closed top-level expression is
/// solid before commit ever runs. Fixing only the commit force
/// (universe.rs:828) leaves this cell solidified into the commit anyway.
#[test]
#[ignore]
fn r3_top_level_pure_computation_is_not_solidified_either() {
    let d = universe("r3", &["top: 1 + 2\n"]);
    let root = flat(&commit_and_read_root(&d, "r3"));
    assert!(
        root.contains(STD_ROOT),
        "REACH: a committed root must carry the standard root digest. got {root:?}"
    );
    assert!(
        root.contains("top: 1 + 2"),
        "D46 (i) has TWO boundaries. Top-level evolve evaluates \
         (universe.rs:389) and commit forces (universe.rs:828); a rule that \
         only covers the second still lets pure computation into history. \
         got {root:?}"
    );
}

/// D46 needs `%effect` to be honest at the forcing boundary. Evolve rewrites
/// a forward-open-miss into a Thunk with `effect: EffectTag::Pure` hardcoded
/// (universe.rs:409-414) instead of calling `predict_effect`. Baseline: once
/// `src` is bound to an `#io` source, `src` reports `#io` and `r` reports
/// nothing -- so under D46 the engine would keep `r` lazy, and `r` is
/// exactly the irreproducible class that must be captured.
#[test]
#[ignore]
fn r4_a_forward_miss_thunk_reports_its_real_effect() {
    let d = universe("r4", &["r: src\n", "src: ~%Time./now #trigger\n"]);
    let staged = flat(&oo(&d, &["status"]));
    assert!(
        staged.contains("src:") && staged.contains("#io"),
        "REACH: the source must be bound and reported as `#io` before the \
         dependent cell's label means anything. got {staged:?}"
    );
    assert!(
        staged.contains("r: src ;; %effect: #io"),
        "A cell that reads an `#io` source is not pure. The label is \
         hardcoded at universe.rs:409-414, so D46 cannot trust `effect()` \
         here. got {staged:?}"
    );
}

// ------------------------------------------------------------------------
// GREEN -- what must not break.
// ------------------------------------------------------------------------

/// Identity red line. D46 changes user roots that hold unforced pure
/// thunks; it must not move the standard root.
#[test]
fn g1_the_standard_root_digest_does_not_move() {
    let d = universe("g1", &["x: 0\n"]);
    let root = flat(&commit_and_read_root(&d, "g1"));
    assert!(
        root.contains(STD_ROOT),
        "the standard root digest is a red line. got {root:?}"
    );
}

/// D46 (ii), ALREADY RIGHT at the baseline. An `#io` cell cannot be rerun,
/// so its forced value must enter the commit, and SPEC_08 4.2.4 turns the
/// active tag into `#cached` when it does. This is the mechanism D46 (ii)
/// cites; the arc must not break it while making (i) true.
#[test]
fn g2_an_io_result_and_its_cached_tag_enter_the_commit() {
    let d = universe("g2", &["box: { t: ~%Time./now #trigger }\n"]);
    let root = flat(&commit_and_read_root(&d, "g2"));
    assert!(
        root.contains("#cached"),
        "an irreproducible cell's forced value must be captured and retagged \
         `#cached` (SPEC_08 4.2.4). got {root:?}"
    );
    assert!(
        !root.contains("~%Time./now"),
        "the io cell must hold its RESULT, not its recipe -- rerunning gives \
         a different answer. got {root:?}"
    );
}

/// SPEC_09 line 10: the root `~%Config.<bare>` write is legal. The combo
/// form is not, and that is legislated too -- do not "fix" it.
#[test]
fn g3_the_config_path_form_still_writes_the_knob() {
    let d = universe("g3", &["~%Config.fuel: 0\n"]);
    let staged = flat(&oo(&d, &["status"]));
    assert!(
        staged.contains("fuel: 0"),
        "root `~%Config.<bare>` is the horizon-parameter family and is \
         write-exempt (SPEC_09 line 10). got {staged:?}"
    );
}

/// D33/D44: bottom may enter history, honestly labelled. A divergent
/// definition must keep saying why.
#[test]
fn g4_a_divergent_definition_still_records_its_cause() {
    let d = universe("g4", &["c: c + 1\n"]);
    let root = flat(&commit_and_read_root(&d, "g4"));
    assert!(
        root.contains("#divergent"),
        "bottom may be committed (D33) but must disclose its cause. got {root:?}"
    );
}

/// An all-solid program has no depth to leave out, so D46 must not change
/// its root at all.
#[test]
fn g5_an_all_solid_program_commits_unchanged() {
    let d = universe("g5", &["a: 1\nb: 2\n"]);
    let root = flat(&commit_and_read_root(&d, "g5"));
    assert!(
        root.contains("a: 1") && root.contains("b: 2") && root.contains(STD_ROOT),
        "a program with nothing lazy in it must commit exactly as before. \
         got {root:?}"
    );
}
