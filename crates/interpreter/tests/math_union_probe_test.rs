// Math × Union distribution probes (2026-07-17, pre-committed by work
// order — docs/math_union_handover.md). Queue item "math×Top 聯集值
// 語境" (taint-scope arc exposure) — REDIAGNOSED wider on measurement:
// bare-Top math is healthy (`_ + 1` → `_`, all ops); the hole is that
// eval_math has NO Union arm at all — every arithmetic op over ANY
// union operand falls to the Conflict catch-all:
//   (2|9)+1 / 1+(2|9) / (1|2)+(10|20) / (2|9)*2 / 0-(2|9) /
//   ("a"|"b")+"x" / 10/(0|2) / field spelling / (_|9)+1 (the ledgered
//   face) / (big|2)+1 — ALL ⊥ #conflict today, a "claims to know"
//   lie about a superposition.
// Meanwhile pipe (`(2|9)|>f` → 3|10) and morphism apply (`/f (2|9)`)
// already distribute — same law, implemented there only.
//
// LAW (all existing — zero new adjudication; engine follows law):
//   - SPEC_07 §4 疊加態平等演化: operations distribute over union
//     branches equally (already cited by the unify distribution arm,
//     L1-32 nav projection, pipe distributivity §2).
//   - Union-cull arc (SPEC_08 §3.2.2 #5 + REAL_04 §4 supplement):
//     per-branch ⊥ results are culled; all-⊥ → primary member's ⊥
//     verbatim; Top and #blur branch results SURVIVE.
//   - G3/SPEC_08 §3.2.2 #1: value-context blur absorption per branch
//     (single-value `big + 1` → #blur already law and green).
//   - Determinism: left-operand-major distribution order (natural
//     recursion: distribute left, then right within each branch);
//     display keeps encounter order (canonical display order is a
//     separate ledger item).
// NOT in scope: `<`/`<=` × union (§4.10 ledger — FROZEN pin below);
//   `=` over unions (G1 structural equality — (2|9)=9 is #false BY LAW,
//   equality is not a distributing op — pin guards against overreach);
//   unary `-(2|9)` spelling (grammar: unary_expr takes no parens —
//   pre-existing, `0-(2|9)` covers the semantic face); %max_branches
//   budget discipline (reuse the unify-arm cap; workspace guards).

use nlang_interpreter::{Ouroboros, Universe};
use nlang_parser::parse_program;
use nlang_parser::ast::{Path, PathAnchor, Span};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

fn tmp_dir() -> PathBuf {
    let mut d = std::env::temp_dir();
    d.push(format!(
        "nlang-mathunion-{}-{}",
        std::process::id(),
        DIR_SEQ.fetch_add(1, Ordering::SeqCst)
    ));
    fs::remove_dir_all(&d).ok();
    fs::create_dir_all(&d).unwrap();
    d
}

/// 64 MiB thread — parser/eval recursion headroom (established pattern).
fn observe_nlang(src: &str, path: &str) -> String {
    let src = src.to_string();
    let path = path.to_string();
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(move || {
            let dir = tmp_dir();
            let engine = Ouroboros::init(&dir).unwrap();
            let mut universe = Universe::new(None, engine.root_with_system());
            let program = parse_program(&src).unwrap();
            for f in &program.fields {
                let _ = universe.evolve(&engine, f);
            }
            let p = Path {
                anchor: PathAnchor::Bare,
                segments: path.split('.').map(|s| s.to_string()).collect(),
                span: Span::default(),
            };
            universe.observe(&engine, &p).to_nlang(0)
        })
        .unwrap()
        .join()
        .unwrap()
}

fn assert_obs(src: &str, expect: &str) {
    let got = observe_nlang(src, "out");
    assert_eq!(got, expect, "{src:?} :: out");
}

fn flat_chain(n: usize) -> String {
    vec!["1"; n].join(" + ")
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — eval_math must distribute over union operands
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_math_union_left() {
    assert_obs("out: (2|9) + 1", "3 | 10");
}

#[test]
fn red_math_union_right() {
    assert_obs("out: 1 + (2|9)", "3 | 10");
}

#[test]
fn red_math_union_both_left_major() {
    // Cartesian via nested distribution; left-operand-major order.
    assert_obs("out: (1|2) + (10|20)", "11 | 12 | 21 | 22");
}

#[test]
fn red_math_union_mul_and_sub() {
    assert_obs("out: (2|9) * 2", "4 | 18");
    assert_obs("out: 0 - (2|9)", "-9 | -2");
}

#[test]
fn red_math_union_string_concat() {
    assert_obs("out: (\"a\"|\"b\") + \"x\"", "\"ax\" | \"bx\"");
}

#[test]
fn red_math_union_top_branch_survives() {
    // MIGRATED (2026-07-20, union_absorption): Top-family collapse —
    // `_ | 9` → `_`, then `_ + 1` → `_` (SPEC_01 §2.4.2; L2-89/L2-75).
    assert_obs("u: _ | 9\nout: u + 1", "_");
}

#[test]
fn red_math_union_static_top_branch() {
    // MIGRATED (2026-07-20, union_absorption): TopCaused | 3 collapses to
    // the cycle Top; arithmetic on the cycle alone is #divergent (same as
    // bare `p.v + 1`), not a superposed `4 | _`.
    let got = observe_nlang("p: {v: p.v}\nu: p.v | 3\nout: u + 1", "out");
    assert!(
        got == "_"
            || (got.starts_with("_|_") && got.contains("divergent")),
        "absorbed cycle Top under math: {got:?}"
    );
}

#[test]
fn red_math_union_divzero_branch_culled() {
    // Per-branch ⊥ (#numerical_error) culled; survivor collapses.
    assert_obs("out: 10 / (0|2)", "5");
}

#[test]
fn red_math_union_blur_branch_survives() {
    // Blur branch absorbs the op (G3 value context) and survives as a
    // #blur member next to the computed branch.
    let got = observe_nlang(
        &format!("big: {}\nout: (big|2) + 1", flat_chain(4000)),
        "out",
    );
    assert!(
        got.starts_with("3 | ") && got.contains("#blur") && got.contains("fuel_exhausted"),
        "blur branch must survive math distribution: {got:?}"
    );
}

#[test]
fn red_math_union_field_spelling() {
    // Dual-spelling lesson: stored-field union, same law.
    assert_obs("w: {v: 2|9}\nout: w.v + 1", "3 | 10");
}

// ─────────────────────────────────────────────────────────────────────────
// ACTIVE PINS — single-value laws and adjacent frozen scope
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn pin_math_top_bare_open() {
    // Bare-Top math healthy (measured all ops): unknown stays unknown.
    assert_obs("out: _ + 1", "_");
    assert_obs("out: _ * 3", "_");
}

#[test]
fn pin_math_top_open_miss() {
    assert_obs("c: {b: 2}\nout: c.a + 1", "_");
}

#[test]
fn pin_math_blur_single_absorbs() {
    // G3 law anchor: single-value blur math absorbs (no union involved).
    let got = observe_nlang(&format!("big: {}\nout: big + 1", flat_chain(4000)), "out");
    assert!(
        got.starts_with("#blur") && got.contains("fuel_exhausted"),
        "single blur math must absorb: {got:?}"
    );
}

#[test]
fn pin_pipe_distribution_law_anchor() {
    // The law this arc extends to math — already green via pipe/apply.
    assert_obs("f: (n -> n + 1)\nout: (2|9) |> f", "3 | 10");
    assert_obs("f: (n -> n + 1)\nout: /f (2|9)", "3 | 10");
}

#[test]
fn pin_eq_union_structural_not_distributive() {
    // G1: `=` is solidified structural equality, NOT a distributing op.
    // (2|9) = 9 is #false BY LAW — the fix must not overreach into `=`.
    assert_obs("out: (2|9) = 9", "#false");
}

#[test]
fn pin_cmp_union_frozen() {
    // MIGRATED (2026-07-20, order-wave W3): union order lands via meet
    // reduction — distinct sets are incomparable → clean #false (not ⊥).
    // Numeric magnitude remains ~%Math./lt family.
    assert_obs("out: (2 | 9) < 5", "#false");
}

#[test]
fn pin_math_bottom_operand_short_circuit() {
    // Whole-operand ⊥ short-circuit unchanged (G3 trap-2 order).
    assert_obs("out: ((1&2) + 1).%cause", "#conflict");
}
