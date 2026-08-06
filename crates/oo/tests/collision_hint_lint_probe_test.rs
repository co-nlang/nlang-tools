// R6 repeated-key / spread-collision lint probes (2026-07-17,
// pre-committed by work order — docs/collision_lint_handover.md).
// 想法 D Tier 1 instrument #3 (after R4 use-without-def, R5 config hint).
//
// LAW CONTEXT (SPEC_03 §1.1, 2026-07-16): repeated keys in ONE literal
// are the degenerate form of parallel definition — they MERGE (`&`),
// never overwrite; §3.1 spread collision = intersection. The semantics
// is legal; the SPELLING is suspicious ("同字面量重複鍵屬可疑拼法,
// lint 提示另議" — this rule is that 議). Users arriving from
// overwrite-semantics languages write {a:1, a:2} expecting 2 and get
// ⊥ #conflict (atoms) or a merge (combos). R6 = Warn, spelling stays
// LEGAL.
//
// Conservative stance (寧漏勿誤, R4/R5 precedent) — R6 fires ONLY on:
//   - identical full key spelling repeated inside one container
//     literal ({} and {{}}, any nesting depth), and
//   - spread collision statically visible: `...{literal}` source whose
//     key collides with a sibling field or another literal spread.
// NEVER flagged:
//   - root program fields (x: 1..9 then x: 5 = the refinement idiom);
//   - `...name` sources (keys not statically knowable — Tier 1 never
//     evaluates);
//   - path-key PARTIAL overlap ({a: {x:1}, a.y: 2} = deliberate
//     parallel-definition style; only identical full spelling counts);
//   - the `_` merge key ({_: ~%Cond, _: ~%Math} = multi-import idiom).

use oo::nlint;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

fn lint_src(src: &str) -> Vec<nlint::Diagnostic> {
    let dir = nlang_interpreter::ScratchDir::new("r6lint");
    let p: PathBuf = dir.join("probe.n");
    fs::write(&p, src).unwrap();
    let report = nlint::analyze_file(&p);
    assert!(
        report.parse_error.is_none(),
        "parse failed: {:?}",
        report.parse_error
    );
    report.diagnostics
}

fn r6_count(src: &str) -> usize {
    lint_src(src).iter().filter(|d| d.rule == "R6").count()
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — R6 fires on suspicious repeated spellings
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_r6_repeated_atom_key() {
    // Guaranteed ⊥ #conflict at merge — the classic overwrite trap.
    assert_eq!(r6_count("c: { a: 1, a: 2 }\nout: c"), 1);
}

#[test]
fn red_r6_repeated_combo_key() {
    // Merges fine — still suspicious (likely overwrite intent).
    assert_eq!(r6_count("c: { a: { x: 1 }, a: { y: 2 } }\nout: c"), 1);
}

#[test]
fn red_r6_repeated_path_key() {
    // Identical FULL path spelling twice.
    assert_eq!(r6_count("c: { a.y: 1, a.y: 2 }\nout: c"), 1);
}

#[test]
fn red_r6_spread_literal_collision() {
    // Statically visible collision: literal spread key vs sibling field.
    assert_eq!(r6_count("c: { a: 1, ...{ a: 2 } }\nout: c"), 1);
}

#[test]
fn red_r6_double_literal_spread_collision() {
    assert_eq!(r6_count("c: { ...{ a: 1 }, ...{ a: 2 } }\nout: c"), 1);
}

#[test]
fn red_r6_cocoon_repeated() {
    assert_eq!(r6_count("cc: {{ a: 1, a: 2 }}\nout: cc"), 1);
}

#[test]
fn red_r6_nested_literal() {
    // Any nesting depth.
    assert_eq!(r6_count("w: { c: { a: 1, a: 2 } }\nout: w"), 1);
}

// ─────────────────────────────────────────────────────────────────────────
// ACTIVE PINS — the NEVER-flag boundary (寧漏勿誤)
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn pin_r6_root_refinement_free() {
    // Root repeated fields = the refinement idiom (monotone evolve).
    assert_eq!(r6_count("x: 1..9\nx: 5\nout: x"), 0);
}

#[test]
fn pin_r6_named_spread_free() {
    // Named source: keys unknowable at Tier 1 — never flagged.
    assert_eq!(r6_count("b: { a: 2 }\nc: { a: 1, ...b }\nout: c"), 0);
}

#[test]
fn pin_r6_path_partial_overlap_free() {
    // Parallel-definition style — deliberate, legal, unflagged.
    assert_eq!(r6_count("c: { a: { x: 1 }, a.y: 2 }\nout: c"), 0);
}

#[test]
fn pin_r6_merge_key_free() {
    // `_` merge key repetition = multi-import idiom.
    assert_eq!(r6_count("c: { _: ~%Cond, _: ~%Math, v: 1 }\nout: c"), 0);
}

#[test]
fn pin_r6_distinct_keys_free() {
    assert_eq!(r6_count("c: { a: 1, b: 2, d.e: 3 }\nout: c"), 0);
}

#[test]
fn pin_cross_rule_no_regression() {
    // R4 (use-without-def) and R5 (node horizon hint) keep firing
    // exactly as before; clean source stays clean.
    let src = "c: { %fuel: 50, v: zz_undefined }\nout: c";
    let diags = lint_src(src);
    assert_eq!(diags.iter().filter(|d| d.rule == "R5").count(), 1);
    assert_eq!(diags.iter().filter(|d| d.rule == "R4").count(), 1);
    assert_eq!(diags.iter().filter(|d| d.rule == "R6").count(), 0);
}
