// Spread privacy-preservation probes (2026-07-15, pre-committed by work
// order — docs/spread_privacy_handover.md).
//
// LAW (SPEC_03 §3.1, EXISTING — engine-follows-law): "私有保全:若展開
// 操作位於目標 Combo 外部,則目標中的 `~` 私有欄位將不會被包含在展開
// 結果中" — external spread must exclude the local axis (anti-leak).
// MEASURED on v0.2.11: spread carries all six axes unconditionally;
// `q: { ...p, peek: ~s }` at root obtains a copy of p's secret and the
// private-axis sealing makes it READABLE (peek → 1) — an exfiltration
// primitive. (On v0.2.10 the copy travelled too but was unreadable.)
// Inside/outside criterion = the SPEC_04 §3.1 #5 geometric route: the
// spread TARGET combo appearing in the current scope chain = insider.
// NOT in scope: `&` merge semantics (six-axis unify is value-level law;
// literal RHS seals separately — pinned), spread collision merge
// (overwrite-vs-intersect divergence, separate case), morphism-body
// root-name resolution (separate case).

use nlang_interpreter::{Ouroboros, Universe};
use nlang_parser::ast::{Path, PathAnchor, Span};
use nlang_parser::parse_program;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

fn tmp_dir() -> nlang_interpreter::ScratchDir {
    nlang_interpreter::ScratchDir::new("sprdpriv")
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
// RED GATES — external spread excludes the local axis
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_external_spread_excludes_local() {
    // L2-36. Today: 1 — the exfiltration primitive.
    assert_obs(
        "p: { ~s: 1, a: 2 }\nq: { ...p, peek: ~s }\nout: q.peek",
        "_",
    );
}

#[test]
fn red_external_spread_eq_public_only() {
    // L2-37: the spread result carries ONLY the public axes.
    assert_obs(
        "p: { ~s: 1, a: 2 }\nq: { ...p }\nout: q = { a: 2 }",
        "#true",
    );
}

#[test]
fn red_nested_external_spread_excludes() {
    // External at any depth: w's subtree never contains p in its chain.
    assert_obs(
        "p: { ~s: 1, a: 2 }\nw: { grab: { ...p, peek: ~s } }\nout: w.grab.peek",
        "_",
    );
}

// ─────────────────────────────────────────────────────────────────────────
// ACTIVE PINS — insider spread keeps; public travel; adjacent scope frozen
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn pin_insider_spread_corpus_shape() {
    // L2-38 / test_entropy shape: spreading a PRIVATE target's public
    // fields (x is public within ~c) — unaffected by the exclusion.
    assert_obs("~c: { x: 1 }\nd: { ...~c, z: 3 }\nout: d.x", "1");
}

#[test]
fn pin_insider_nested_spread_keeps_local() {
    // Spread INSIDE the target's own scope keeps the local axis
    // (SPEC_03 clause is conditional on being external).
    assert_obs(
        "p: { ~s: 1, a: 2, c2: { ...p, rd: ~s } }\nout: p.c2.rd",
        "1",
    );
}

#[test]
fn pin_spread_public_fields_travel() {
    assert_obs("p: { ~s: 1, a: 2 }\nq: { ...p }\nout: q.a", "2");
}

#[test]
fn pin_spec03_example_shape() {
    // SPEC_03 §3.1's own example: { a: 1, ...~c } → { a: 1, b: 2 }.
    assert_obs(
        "~c: { b: 2 }\nresult_spread: { a: 1, ...~c }\nout: result_spread.b",
        "2",
    );
}

#[test]
fn pin_unify_merge_untouched() {
    // `&` merge is value-level six-axis law — NOT this order's scope.
    assert_obs("p: { ~s: 1, a: 2 }\nq: p & { b: 3 }\nout: q.b", "3");
}

#[test]
fn pin_unify_literal_seal_no_steal() {
    // Frozen current: the `&` RHS literal seals separately — no read.
    assert_obs(
        "p: { ~s: 1, a: 2 }\nq: p & { peek2: ~s }\nout: q.peek2",
        "_",
    );
}

#[test]
fn pin_outward_block_regression_guard() {
    // Private-axis arc must stay closed through this one.
    let got = observe_nlang("p: { ~s: 1 }\nout: p.~s", "out");
    assert!(
        got.starts_with("_|_") && got.contains("private_access_violation"),
        "outward block regressed: {got:?}"
    );
}

#[test]
fn pin_display_strip_regression_guard() {
    let got = observe_nlang("p: { ~s: 1, pub: 2 }\nout: p", "out");
    assert!(
        !got.contains("~s") && got.contains("pub"),
        "display strip regressed: {got:?}"
    );
}
