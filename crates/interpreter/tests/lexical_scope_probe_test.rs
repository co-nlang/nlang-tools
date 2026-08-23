// Lexical-scope probes (2026-07-16, pre-committed by work order —
// docs/lexical_scope_handover.md).
//
// LAW (SPEC_04 §2.1, EXISTING — engine-follows-law, zero new rulings):
// `resolve_bare_name(s, H)` walks the scope chain inner→outer; at each
// level fields(H) covers the Data/Type/Logic/Meta axes; first match wins
// (inner shadows outer); exhausted chain → Top (open world). §3.3: a
// morphism body resolves bare names through its DEFINING closure scope.
// MEASURED on v0.2.13: the lexical chain inside PUBLIC combos is dead —
// sibling reads return `_`, holder-sibling morphism capture dies, non-root
// ancestor lifting dies, and shadowing tells a WRONG-VALUE lie (inner k
// skipped, outer k substituted: `k:5; c:{k:7, d:k+1}` → 6, law says 8).
// Adding one `~` field revives the whole chain: the private-axis arc's
// seal_defining_scope skips frame injection when `local` is empty (its
// anti-pollution guard), which also severs PUBLIC lexical visibility.
// TRIPWIRES: the guard existed to protect Thunk equality/unify — twin
// literal `=` and `%id` stability are pinned below; if the chosen fix
// breaks those pins, STOP and report (do not weaken pins).
// NOT in scope: eq×thunk forcing (`x = {k:5, d:6}` is #false today —
// measure-and-report duty only; a flip to #true is law-direction, any
// other flip = stop), forward-ref × spread (frozen elsewhere).

use nlang_interpreter::{Ouroboros, Universe};
use nlang_parser::ast::{Path, PathAnchor, Span};
use nlang_parser::parse_program;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

fn tmp_dir() -> nlang_interpreter::ScratchDir {
    nlang_interpreter::ScratchDir::new("lexscope")
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
            let mut universe = Universe::new_with_standard(
                None,
                engine.root_with_system(),
                engine.root_with_system(),
            );
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
// RED GATES — sibling visibility (§2.1)
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_sibling_thunk() {
    // L2-43. Today: `_`.
    assert_obs("c: { k: 5, d: k + 1 }\nout: c.d", "6");
}

#[test]
fn red_sibling_chained() {
    // d needs k, e needs d and k. Today: `_`.
    assert_obs("c: { k: 5, d: k + 1, e: d + k }\nout: c.e", "11");
}

#[test]
fn red_display_siblings_resolved() {
    // Collapsed display forces fields — today shows `d: _`.
    let got = observe_nlang("c: { k: 5, d: k + 1 }\nout: c", "out");
    assert!(
        got.contains("d: 6") && got.contains("k: 5"),
        "display must resolve sibling thunks: {got:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — holder-sibling capture in morphism bodies (§2.1 + §3.3)
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_holder_morphism() {
    // L2-44. Today: `_`. Both spellings (inline + binding split).
    assert_obs("c: { k: 5, f: (x -> x + k) }\nout: 1 |> c.f", "6");
    assert_obs("c: { k: 5, f: (x -> x + k) }\ng: 1 |> c.f\nout: g", "6");
}

#[test]
fn red_holder_morphism_spread() {
    // Morphism body spreads a holder sibling. Today: `_`.
    assert_obs(
        "c: { p2: { a: 2 }, f: (x -> { ...p2 }) }\ng: 1 |> c.f\nout: g.a",
        "2",
    );
}

#[test]
fn red_nested_holder_morphism() {
    // Today: `_`.
    assert_obs("w: { c: { k: 5, f: (x -> x + k) } }\nout: 1 |> w.c.f", "6");
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — ancestor lifting (§2.1)
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_grandparent_lifting() {
    // L2-46: lifting through a NON-ROOT ancestor. Today: `_`.
    assert_obs("w: { k: 5, c: { d: k + 1 } }\nout: w.c.d", "6");
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — shadowing: inner match wins (§2.1 first-match)
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_shadowing_inner_first() {
    // L2-45. Today: 6 — the WRONG-VALUE lie (outer k substituted).
    assert_obs("k: 5\nc: { k: 7, d: k + 1 }\nout: c.d", "8");
}

#[test]
fn red_shadowing_morphism() {
    // Same lie through a morphism body. Today: 6.
    assert_obs("k: 5\nc: { k: 7, f: (x -> x + k) }\nout: 1 |> c.f", "8");
}

// ─────────────────────────────────────────────────────────────────────────
// ACTIVE PINS — living faces + anti-pollution tripwires
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn pin_root_lifting() {
    // Root fields sit in ctx.scopes — alive today, must stay.
    assert_obs("k: 5\nc: { d: k + 1 }\nout: c.d", "6");
}

#[test]
fn pin_arg_shadows_root() {
    assert_obs("p: { a: 2 }\nf: (p -> p + 1)\nout: 3 |> f", "4");
}

#[test]
fn pin_curried_capture() {
    assert_obs("k: 5\nf: (x -> (y -> x + y + k))\nout: 2 |> (1 |> f)", "8");
}

#[test]
fn pin_private_combo_chain_lives() {
    // Seal-triggered chain (local non-empty) works today — must survive
    // whatever unification of the two paths the fix chooses.
    assert_obs("c: { ~z: 9, k: 5, f: (x -> x + k) }\nout: 1 |> c.f", "6");
}

#[test]
fn pin_factory_spec_example() {
    assert_obs(
        "factory: {\n    ~seed: 42\n    product: { val: ~seed + 1 }\n}\nout: factory.product.val",
        "43",
    );
}

#[test]
fn pin_morphism_root_spread() {
    // The healed former-ledger form (root combo spread in morphism body).
    assert_obs(
        "p: { a: 2 }\ngive: (x -> { ...p })\ng: 1 |> give\nout: g.a",
        "2",
    );
}

#[test]
fn pin_undefined_bare_open() {
    // Open world: exhausted chain → Top; arithmetic keeps it open.
    assert_obs("c: { d: zz + 1 }\nout: c.d", "_");
}

#[test]
fn pin_twin_literal_eq_tripwire() {
    // ANTI-POLLUTION TRIPWIRE: two identical literals with sibling-
    // referencing thunks stay `=`. If the fix breaks this, STOP — the
    // seal guard's original fear came true; report, don't weaken.
    assert_obs(
        "x: { k: 5, d: k + 1 }\ny: { k: 5, d: k + 1 }\nout: x = y",
        "#true",
    );
}

#[test]
fn pin_caid_stability_tripwire() {
    // ANTI-POLLUTION TRIPWIRE: content identity ignores frames.
    assert_obs("x: { a: 1 }\ny: { a: 1 }\nout: x.%id == y.%id", "#true");
}

#[test]
fn pin_insider_spread_keeps_local() {
    assert_obs(
        "p: { ~s: 1, a: 2, c2: { ...p, rd: ~s } }\nout: p.c2.rd",
        "1",
    );
}

#[test]
fn pin_external_spread_excludes_local() {
    assert_obs(
        "p: { ~s: 1, a: 2 }\nq: { ...p, peek: ~s }\nout: q.peek",
        "_",
    );
}

// ─────────────────────────────────────────────────────────────────────────
// ACCEPTANCE-REPAIR PIN (2026-07-16): %id observes solidified content.
// The delivery's frame injection split %id across nesting depths for
// identical spellings (root twin #true but cross-depth #false — a content
// lie the same-depth tripwire missed). Repair: the %id arm hashes
// force_recursive(current), the same solidification the observe exit uses.
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn pin_caid_cross_depth_repair() {
    assert_obs(
        "a1: { k: 5, d: k + 1 }\nb1: { q2: { k: 5, d: k + 1 } }\nout: a1.%id == (b1.q2).%id",
        "#true",
    );
    // Local axis still participates (G1 six-axis identity).
    assert_obs(
        "x: { ~s: 1, a: 2 }\ny: { a: 2 }\nout: x.%id == y.%id",
        "#false",
    );
}
