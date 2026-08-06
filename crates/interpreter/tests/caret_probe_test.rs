// `^` parent-anchor probes (2026-07-17, pre-committed by work order —
// docs/caret_handover.md).
//
// RULING (approved 2026-07-17; SYNTAX_03 §4.4 + SPEC_07 §4.2.3):
//   Q1  container chain INCLUDES the root universe as the outermost
//       container: `^` = parent of the current container, `^^` from
//       depth 2 = root. Root-level `^` = root has no parent → overshoot.
//   Q2  `^.x` is a STRICT coordinate access at the designated level —
//       parent lacking x → open `_` (superposition default), NO lexical
//       fallback to grandparents (that's the bare-name chain's job; `^`
//       is a path anchor).
//   Q3  LHS `^` DEFINITION KEYS ABOLISHED (grammar level, ~. anchor
//       precedent): a nested literal reaching up to mutate ancestors
//       breaks literal locality — "opening a directory and finding the
//       parent's files growing inside it" (user's filesystem intuition).
//       Parent writes are a redundant spelling of parallel definition
//       at the correct level (one concept, one spelling), and bring a
//       monster family for free (self-collision {d:{^.d:9}}, root loop
//       {d:{^^.w:5}}, cocoon pierce {{d:{^.q:1}}}, spread-carried
//       binding ambiguity). RHS `^` untouched.
// MEASURED on v0.2.18+: engine is OFF BY ONE — `^ⁿ` lands n-1 levels up
// from the enclosing container (`^.a` reads the CURRENT container: 9 in
// the shadowing form = wrong-value lie; `_` when absent), root universe
// unreachable; LHS `^` writes into the current container. Overshoot
// faces (deep + root-level) are healthy since the ⊥-meta arc.
// NOT in scope: `^` × pipe (P2 ban, SYNTAX_12 §2.4 — already law);
// `^` inside morphism bodies (record if ambiguous, don't adjudicate);
// #out_of_horizon details fields (requested/actual depth cocoons).

use nlang_interpreter::{Ouroboros, Universe};
use nlang_parser::ast::{Path, PathAnchor, Span};
use nlang_parser::parse_program;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

fn tmp_dir() -> nlang_interpreter::ScratchDir {
    nlang_interpreter::ScratchDir::new("caret")
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
// RED GATES — ascent lands one level short (off-by-one family)
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_caret_parent_basic() {
    assert_obs("c: { a: 1, d: { v: ^.a } }\nout: c.d.v", "1");
}

#[test]
fn red_caret_parent_shadowing() {
    // L2-66. The decisive form: parent and current both define `a`.
    assert_obs("s: { a: 1, d: { a: 9, v: ^.a } }\nout: s.d.v", "1");
}

#[test]
fn red_caret_two_level() {
    assert_obs("w: { a: 7, m: { n: { v: ^^.a } } }\nout: w.m.n.v", "7");
}

#[test]
fn red_caret_reaches_root() {
    // L2-67. Q1: the container chain tops out AT root, not below it.
    assert_obs("r_a: 42\nc: { d: { v: ^^.r_a } }\nout: c.d.v", "42");
}

#[test]
fn red_caret_arith_operand() {
    assert_obs("t: { a: 1, d: { v: ^.a + 1 } }\nout: t.d.v", "2");
}

#[test]
fn red_caret_lhs_key_rejected() {
    // Q3 abolition — grammar level, both nested and root-level forms,
    // path-key form included.
    assert!(
        parse_program("w: { d: { ^.z: 5 } }").is_err(),
        "nested LHS ^ key must be a parse error"
    );
    assert!(
        parse_program("^.z: 5").is_err(),
        "root-level LHS ^ key must be a parse error"
    );
    assert!(
        parse_program("w: { d: { ^^.z.q: 5 } }").is_err(),
        "LHS ^ path key must be a parse error"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// ACTIVE PINS — healthy overshoot faces, boundaries, RHS spelling
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn pin_caret_overshoot_deep() {
    // L2-68 (green law pin): from depth 2 the chain is d → c3 → root;
    // ^^^ asks one past root → ⊥ #out_of_horizon. Same verdict today
    // (different internal count) — must hold through the fix.
    assert_obs(
        "c3: { a: 1, d: { v: ^^^.a } }\nout: (c3.d.v).%cause",
        "#out_of_horizon",
    );
}

#[test]
fn pin_caret_overshoot_root_level() {
    // Root has no parent — canonical tag (⊥-meta arc face, stays).
    let got = observe_nlang("out: ^.zz", "out");
    assert!(
        got.starts_with("_|_") && got.contains("out_of_horizon"),
        "root-level ^ must overshoot: {got:?}"
    );
}

#[test]
fn pin_caret_no_lexical_fallback() {
    // Q2: ^ is a path anchor, not a search — parent `d` lacks `a`, the
    // answer is open `_`; the engine must NOT walk on to find c.a.
    assert_obs("c: { a: 1, d: { e: { v: ^.a } } }\nout: c.d.e.v", "_");
}

#[test]
fn pin_root_anchor_still() {
    // `_.` absolute anchor untouched by the ascent fix.
    assert_obs("r: 5\nc: { d: { v: _.r } }\nout: c.d.v", "5");
}

#[test]
fn pin_bare_lexical_still() {
    // Bare-name lexical chain (SPEC_04 §2.1) is a different mechanism —
    // must survive the caret rewiring byte-identical.
    assert_obs("c: { a: 1, d: { v: a } }\nout: c.d.v", "1");
}

#[test]
fn pin_rhs_caret_parses() {
    // Q3 abolishes DEFINITION keys only — RHS ^ spelling stays legal.
    assert!(parse_program("v: ^.x").is_ok());
    assert!(parse_program("c: { d: { v: ^^.a } }").is_ok());
}

#[test]
fn pin_caret_twin_eq() {
    // Anti-pollution tripwire: ascent resolution must not leak frames
    // into content (same lesson family as lexical %id).
    assert_obs(
        "x1: { a: 1, d: { v: ^.a } }\nx2: { a: 1, d: { v: ^.a } }\nout: x1 = x2",
        "#true",
    );
}
