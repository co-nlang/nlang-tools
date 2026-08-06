// Union canonical display order probes (2026-07-18, pre-committed by
// work order — docs/display_order_handover.md). Queue item "canonical
// 顯示序", RULED 2026-07-18 = SPEC_01 §2.4.1.
//
// MEASURED disease (v0.2.21): CAID is already order-free (bn_serial
// sorts branch digests, value.rs:1413) so the store dedupes multiset-
// equal unions — and the FIRST-STORED spelling wins display globally:
//   a: 9|2 + b: 2|9         → both print "9 | 2"
//   (file order reversed)   → both print "2 | 9"
//   z: 9|2 before (2|9)+0   → m prints "9 | 2" (unrelated field
//                             rewrites a spelling at a distance)
// Semantics healthy (multiset eq, a = b → #true). Display is a
// function of evolution HISTORY, not of the value. §2.4.1 makes
// spelling a function of the value.
//
// LAW (SPEC_01 §2.4.1, ruled 2026-07-18):
//   - Sort key = (type-family rank, intra-family order):
//     numbers ascending → strings lex → tag atoms lex → structured
//     values (range/list/combo) by canonical display string lex →
//     #blur snapshots by display string lex → Top (incl. TopCaused)
//     LAST. ⊥ never appears in multi-branch display (cull law);
//     defensive last. Stable sort.
//   - DISPLAY LAYER ONLY (observe projection + to_nlang family). The
//     internal branch vector is NOT reordered — tropical budget
//     truncation, left-operand-major distribution evaluation order,
//     and fuel order are untouched.
//   - FORBIDDEN: sorting by CAID/digest (#blur CAID carries a
//     per-instance salt → cross-process nondeterminism).
// NOT in scope: normalize_union / unify vector order (construction
//   layer stays encounter-order); max_branches / tropical cap
//   semantics (which branches SURVIVE is unchanged); CAID/bn_serial
//   (already canonical — pin guards it); `=` multiset equality
//   (already law — pin guards it).

use nlang_interpreter::{Ouroboros, Universe};
use nlang_parser::ast::{Path, PathAnchor, Span};
use nlang_parser::parse_program;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

fn tmp_dir() -> nlang_interpreter::ScratchDir {
    nlang_interpreter::ScratchDir::new("disporder")
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
// RED GATES — display must be the canonical sorted spelling
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_display_numeric_sorted() {
    assert_obs("out: 9 | 2 | 5", "2 | 5 | 9");
}

#[test]
fn red_display_spelling_independent() {
    // THE disease face: first-stored spelling must no longer win.
    // Both spellings of the same value print the one canonical form.
    let a = observe_nlang("a: 9 | 2\nb: 2 | 9\nout: a", "out");
    let b = observe_nlang("a: 9 | 2\nb: 2 | 9\nout: b", "out");
    assert_eq!(a, "2 | 9", "a spelling must be canonical");
    assert_eq!(b, "2 | 9", "b spelling must be canonical");
}

#[test]
fn red_display_no_action_at_distance() {
    // Unrelated earlier field must not rewrite m's spelling — and the
    // canonical spelling is sorted regardless of either source spelling.
    assert_obs("z: 9 | 2\nout: (2|9) + 0", "2 | 9");
}

#[test]
fn red_display_dedupe_then_sorted() {
    assert_obs("out: 2 | 1 | 2", "1 | 2");
}

#[test]
fn red_display_type_rank_mixed() {
    // numbers < strings < tag atoms (L2-77 twin).
    assert_obs("out: \"b\" | 3 | #t | 1", "1 | 3 | \"b\" | #t");
}

#[test]
fn red_display_strings_lex() {
    assert_obs("out: \"b\" | \"a\"", "\"a\" | \"b\"");
}

#[test]
fn red_display_top_last() {
    // MIGRATED (2026-07-20, union_absorption): Top-family collapse —
    // `9 | _` → `_` (SPEC_01 §2.4.2); display order of Top is moot.
    assert_obs("u: _ | 9\nout: u", "_");
}

#[test]
fn red_display_float_int_ascending() {
    assert_obs("out: 2.5 | 1 | 3", "1 | 2.5 | 3");
}

#[test]
fn red_display_math_result_sorted() {
    // Distribution EVALUATES left-major (semantics untouched) but the
    // displayed result is canonical: (9|2)+1 → 3 | 10, not 10 | 3.
    assert_obs("out: (9|2) + 1", "3 | 10");
}

#[test]
fn red_display_blur_after_values() {
    // Blur branch sorts after solid values, before Top.
    let got = observe_nlang(&format!("big: {}\nout: big | 2", flat_chain(4000)), "out");
    assert!(
        got.starts_with("2 | ") && got.contains("#blur"),
        "blur must sort after solid values: {got:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// ACTIVE PINS — identity/equality/semantics must not move
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn pin_eq_multiset_unchanged() {
    // SPEC_01 commutativity: equality was already spelling-blind.
    assert_obs("out: (1 | 2) = (2 | 1)", "#true");
}

#[test]
fn pin_caid_already_order_free() {
    // bn_serial sorts digests today — display change must not touch it.
    assert_obs("a: 9 | 2\nb: 2 | 9\nout: a.%caid = b.%caid", "#true");
}

#[test]
fn pin_already_sorted_stable() {
    assert_obs("out: 1 | 2", "1 | 2");
}

#[test]
fn pin_single_survivor_collapse() {
    // Cull + collapse unaffected (union-cull arc law).
    assert_obs("out: (1&2) | 5", "5");
}

#[test]
fn pin_all_bottom_verbatim() {
    // All-⊥ verbatim primary-cause passthrough is NOT a union display.
    let got = observe_nlang("u: {a: (2&3)}|{a: (1&2)}\nout: u.a", "out");
    assert!(
        got.starts_with("_|_") && !got.contains(" | "),
        "all-⊥ must stay verbatim primary ⊥: {got:?}"
    );
}

#[test]
fn pin_nav_cull_value_unchanged() {
    // Union nav projection semantics unchanged (only spelling law).
    // MIGRATED (2026-07-20, union_absorption): Top-family collapse.
    // MIGRATED-2 (2026-07-20, caused_top ruling C): the open-miss /
    // static-cycle Top is a CAUSED Top = diagnostic member — exempt from
    // absorption (SPEC_01 §2.4.2). Bare `_` still absorbs.
    assert_obs("u: {a: 1}|7\nout: u.a", "1 | _");
}

#[test]
fn pin_pipe_distribution_semantics() {
    // Pipe still distributes; result displays canonically (already
    // ascending here — green today, guards against semantic drift).
    assert_obs("f: (n -> n + 1)\nout: (2|9) |> f", "3 | 10");
}
