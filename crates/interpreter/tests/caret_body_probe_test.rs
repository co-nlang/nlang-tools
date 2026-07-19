// Morphism-body `^` binding probes (2026-07-19, pre-committed by work
// order — docs/caret_body_handover.md). Queue item "態射體內 ^ 綁定"
// (exposed at the ^-resolution arc), RULED 2026-07-19 (option A):
// `^` is a DEFINITION-TIME PATH ABBREVIATION, not an observed-time
// reading (user's design intent — morphism-body ^ was never considered
// originally; the definition-site reading is the original concept).
//
// MEASURED (v0.2.24) — the CHIMERA chain: body-eval `^` chain =
//   [definition-closure frames (holder → up, EXCLUDING root)]
//   ++ [call-site container chain (call container → … → root)]
// Nine faces confirmed: hop 1 is lexical (holder), hop (frames+1)
// onward LEAKS into the caller's dynamic chain — the same literal's
// `^^` reads different worlds per call site (at h: 9; at root: 5);
// root-held morphisms are dynamic from hop 1 (`^.k` at h → 9).
//
// LAW (SPEC_07 §4.2.3 addendum, 2026-07-19): body `^` chain =
// DEFINITION-side container chain to the end ([holder → … → root];
// body counts as one level inside the holder, isomorphic to nested
// literals), captured at definition, call-site independent; overshoot
// → ⊥ #out_of_horizon at observation. Call-site data's only channel
// = `$` (P1–P5 explicit passing). Three channels, one law each:
// bare name = lexical chain / `$` = dynamic input / `^` = definition-
// side strict coordinates.
// NOT in scope: field-RHS `^` semantics (^-arc law, pinned in
//   caret_probe_test.rs); bare-name lexical machinery (frames still
//   serve name resolution — only the ^-hop chain SOURCE changes);
//   `$` binding (P1–P5); morphism-vs-pipe semantic prose (editorial,
//   ledgered separately).

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
        "nlang-caretbody-{}-{}",
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
// RED GATES — body ^ chain is definition-side to the end
// ─────────────────────────────────────────────────────────────────────────

#[test]
#[ignore] // RED GATE — remove at delivery
fn red_body_tail_definition_side() {
    // L2-79 twin. chain [c, root]: ^^ = root (5). Today leaks to h (9).
    assert_obs(
        "k: 5\nc: {k: 7, f: (n -> ^^.k)}\nh: {k: 9, r: 3 |> c.f}\nout: h.r",
        "5",
    );
}

#[test]
#[ignore] // RED GATE — remove at delivery
fn red_body_root_def_holder_level() {
    // L2-80 twin. Root-held: chain [root]; ^ = root (5). Today: h (9).
    assert_obs("k: 5\nf: (n -> ^.k)\nh: {k: 9, r: 3 |> f}\nout: h.r", "5");
}

#[test]
#[ignore] // RED GATE — remove at delivery
fn red_body_call_site_independent() {
    // THE literal-locality face: same literal, two call sites, ONE value.
    let src_h = "k: 5\nc: {k: 7, f: (n -> ^^.k)}\nh: {k: 9, r: 3 |> c.f}\nout: h.r";
    let src_root = "k: 5\nc: {k: 7, f: (n -> ^^.k)}\nout: 3 |> c.f";
    assert_eq!(observe_nlang(src_h, "out"), "5", "call at h");
    assert_eq!(observe_nlang(src_root, "out"), "5", "call at root");
}

#[test]
#[ignore] // RED GATE — remove at delivery
fn red_body_overshoot_honest() {
    // chain [c, root]: ^^^ overshoots → ⊥ #out_of_horizon (existing
    // law). Today the chimera supplies h and answers 5.
    assert_obs(
        "k: 5\nc: {k: 7, f: (n -> ^^^.k)}\nh: {k: 9, r: 3 |> c.f}\nout: (h.r).%cause",
        "#out_of_horizon",
    );
}

#[test]
#[ignore] // RED GATE — remove at delivery
fn red_body_deep_def_tail() {
    // Two frames [d, c] then root: ^^^ = root (5). Today: h (9).
    assert_obs(
        "k: 5\nc: {k: 7, d: {k: 8, f: (n -> ^^^.k)}}\nh: {k: 9, r: 3 |> c.d.f}\nout: h.r",
        "5",
    );
}

#[test]
#[ignore] // RED GATE — remove at delivery
fn red_body_apply_form() {
    // Dual-spelling lesson: morphism apply, same law as pipe.
    assert_obs(
        "k: 5\nc: {k: 7, f: (n -> ^^.k)}\nh: {k: 9, r: /c.f 3}\nout: h.r",
        "5",
    );
}

// ─────────────────────────────────────────────────────────────────────────
// ACTIVE PINS — hop 1 base, name/$ channels, field-RHS law
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn pin_body_holder_level() {
    // Hop 1 = holder (body counts as one level inside c) — green today,
    // base level does NOT move.
    assert_obs(
        "k: 5\nc: {k: 7, f: (n -> ^.k)}\nh: {k: 9, r: 3 |> c.f}\nout: h.r",
        "7",
    );
    assert_obs("k: 5\nc: {k: 7, f: (n -> ^.k)}\nout: 3 |> c.f", "7");
}

#[test]
fn pin_body_deep_two_frames() {
    // Deep definition: ^ = d, ^^ = c (both lexical today, stay).
    assert_obs(
        "k: 5\nc: {k: 7, d: {k: 8, f: (n -> ^.k)}}\nh: {k: 9, r: 3 |> c.d.f}\nout: h.r",
        "8",
    );
    assert_obs(
        "k: 5\nc: {k: 7, d: {k: 8, f: (n -> ^^.k)}}\nh: {k: 9, r: 3 |> c.d.f}\nout: h.r",
        "7",
    );
}

#[test]
fn pin_bare_name_lexical_channel() {
    // Channel 1: bare names = lexical chain (SPEC_04), untouched.
    assert_obs(
        "k: 5\nc: {k: 7, f: (n -> k)}\nh: {k: 9, r: 3 |> c.f}\nout: h.r",
        "7",
    );
}

#[test]
fn pin_dollar_dynamic_channel() {
    // Channel 2: `$` = the explicit dynamic input (P1), untouched.
    assert_obs("out: 3 |> (n -> $ + 1)", "4");
}

#[test]
fn pin_field_rhs_caret_unchanged() {
    // Field-RHS ^ (nested literal) reads the holder's parent — the
    // isomorphism the body law is built on (caret_probe guards more).
    assert_obs("c: { k: 7, w: { q: ^.k } }\nout: c.w.q", "7");
}
