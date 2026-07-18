// G4 union-navigation probes (2026-07-12, pre-committed by work order —
// docs/union_nav_handover.md).
//
// RE-DIAGNOSIS: the corpus-cleanup ledger said "union-dedupe invisible to
// path navigation". Counterfactual at v0.2.2 disproved that scope — the
// dedupe was a red herring (it merely collapsed SOME cases to single
// survivors, where navigation works). Measured truth: navigating ANY
// genuine multi-branch Union — literal or evolved — hits
// navigate_segments' catch-all arm (lib.rs `_ => InvalidPath`) → every
// `.field` on a superposition is ⊥ #invalid_path. Union navigation is
// simply unimplemented.
//
// RULING (adjudicated 2026-07-12; SPEC_07 平等演化 / functorial
// observation): path navigation over a Union maps PER BRANCH — each
// branch navigates exactly as a single value would (open-miss → Top,
// non-navigable → ⊥ #invalid_path) — then:
//   ⊥ branches are DROPPED (compatible-survivor rule);
//   all branches ⊥ → ⊥ #invalid_path;
//   survivors → normalize_union (structural dedupe, single collapses).
// Top-miss branches are KEPT (honest superposition: "possibly absent"),
// mirroring the single-combo open-world miss (`({a:1}).b` = `_`, pinned).

use nlang_interpreter::{Ouroboros, Universe, Value};
use nlang_parser::parse_program;
use nlang_parser::ast::{Path, PathAnchor, Span};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

fn tmp_dir() -> PathBuf {
    let mut d = std::env::temp_dir();
    d.push(format!(
        "nlang-g4probe-{}-{}",
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

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — union navigation must project per branch
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_union_nav_common_field_dedupes() {
    // 9 | 9 → normalize → 9
    assert_obs("out: ({ a: 1, c: 9 } | { a: 1, b: 2, c: 9 }).c", "9");
}

#[test]
fn red_union_nav_distinct_values_superpose() {
    assert_obs("out: ({ a: 1 } | { a: 2 }).a", "1 | 2");
}

#[test]
fn red_union_nav_partial_field_keeps_top_branch() {
    // branch 1 open-miss → _ (kept: honest superposition), branch 2 → 2
    assert_obs("out: ({ a: 1 } | { a: 1, b: 2 }).b", "2 | _");
}

// red_union_nav_bottom_branch_dropped MIGRATED 2026-07-14 by the ACCEPTOR:
// #invalid_path abolished — an atom branch is an open miss (`_`), kept like
// any Top-miss branch. Successor red gate:
// bottom_meta_probe_test::red_union_atom_branch_open_miss (`1 | _`).

// pin_union_nav_all_bottom_is_invalid_path MIGRATED 2026-07-14 by the
// ACCEPTOR: #invalid_path abolished; atom branches open-miss and are kept,
// so `(1 | 2).a` = `_ | _` → `_`. Successor red gate:
// bottom_meta_probe_test::red_union_atoms_nav_open.

#[test]
fn red_union_nav_multi_segment() {
    // 5 | 5 → 5 through two segments
    assert_obs(
        "out: ({ p: { q: 5 } } | { p: { q: 5 }, r: 1 }).p.q",
        "5",
    );
}

#[test]
fn red_union_nav_evolved_all_survive() {
    // the original federation shape: all-survive meet, then navigate
    assert_obs(
        "~v1: { a: 1 }\n~v2: { a: 1, b: 2 }\n~u: ~v1 | ~v2\nall: ~u & { c: 9 }\nout: all.c",
        "9",
    );
}

// ─────────────────────────────────────────────────────────────────────────
// ACTIVE pins — single-value navigation is the functorial base case
// ─────────────────────────────────────────────────────────────────────────

#[test] // ACTIVE pin: single combo navigation
fn pin_single_combo_nav() {
    assert_obs("out: ({ a: 1 }).a", "1");
}

#[test] // ACTIVE pin: single-combo open-world miss = Top — the union
        // Top-branch rule mirrors THIS
fn pin_single_combo_open_miss_top() {
    assert_obs("out: ({ a: 1 }).b", "_");
}

// pin_atom_nav_invalid_path MIGRATED 2026-07-14 by the ACCEPTOR:
// #invalid_path abolished — atom data axis is open (hybridization can grow
// fields). Successor red gate: bottom_meta_probe_test::red_atom_nav_open.

#[test] // ACTIVE pin: conflict-kill single survivor navigates (corpus
        // workaround shape — must keep working)
fn pin_conflict_kill_survivor_nav() {
    assert_obs(
        "~v1: { a: 1 }\n~v2: { a: 1, b: 2 }\n~u: ~v1 | ~v2\nout: (~u & { b: 3 }).b",
        "3",
    );
}

#[test] // ACTIVE pin: union display untouched (nav rule must not leak
        // into plain observation of the union itself)
fn pin_union_display_unchanged() {
    assert_obs("out: 1 | 2", "1 | 2");
}

#[test] // ACTIVE pin: union idempotence dedupe untouched
fn pin_union_dedupe_unchanged() {
    assert_obs("out: 1 | 2 | 1", "1 | 2");
}
