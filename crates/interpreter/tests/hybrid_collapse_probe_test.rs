// G6 hybrid value-context collapse probes (2026-07-13, pre-committed by
// work order — docs/g6_hybrid_collapse_handover.md).
//
// MEASURED DEFECT (single root): value-context collapse only recognizes
// PURE wrappers (`is_pure_wrapper` = %val + %-meta only). A hybrid node
// (%val + non-% data fields, SYNTAX_06 §4 #6) never collapses:
// observation prints the full combo, arithmetic and pipe args are
// ⊥ #conflict.
//
// RULING (SYNTAX_06 §4 #6 unified value-context law + SYNTAX_07 §4 #6
// duality, adjudicated 2026-07-13):
//   Value contexts read %val: collapsed observation (recursively in the
//   rendered tree), math operands, atomic cmp (G1, already done).
//   `x |> inc` → 2 falls out of body math; the ARGUMENT passes whole so
//   body navigation (`p.name`) still works — do NOT peel at binding.
//   NOT value contexts: coordinate navigation, `=` family (extensional),
//   structural observation `<<x>>` (full node incl. %val — the duality),
//   plain combos without %val (stay ⊥ in math/atomic-cmp).

use nlang_interpreter::{Ouroboros, Universe};
use nlang_parser::ast::{Path, PathAnchor, Span};
use nlang_parser::parse_program;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

fn tmp_dir() -> nlang_interpreter::ScratchDir {
    nlang_interpreter::ScratchDir::new("g6probe")
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

fn assert_bottom_conflict(src: &str) {
    let got = observe_nlang(src, "out");
    assert!(
        got.starts_with("_|_"),
        "{src:?} :: out — expected ⊥, got {got:?}"
    );
    assert!(
        got.contains("#conflict"),
        "{src:?} :: out — expected #conflict cause, got {got:?}"
    );
}

const HYBRID: &str = "x: 1 & { name: \"Alice\" }\n";

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — value contexts read %val
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_observe_hybrid_reads_val() {
    // L1-37. Collapsed observation of the hybrid itself → the atom.
    // (Supersedes combo_equality_probe_test's temporary display pin,
    // removed in the same commit that pre-committed this file.)
    assert_obs("x: 1 & { name: \"Alice\" }\nout: x", "1");
}

#[test]
fn red_math_hybrid_left_operand() {
    // L1-38.
    assert_obs("x: 1 & { name: \"Alice\" }\nout: x + 1", "2");
}

#[test]
fn red_math_hybrid_right_operand() {
    assert_obs("out: 10 + (2 & { n: \"m\" })", "12");
}

#[test]
fn red_math_hybrid_both_operands() {
    assert_obs("a: 1 & { u: \"p\" }\nb: 2 & { v: \"q\" }\nout: a + b", "3");
}

#[test]
fn red_pipe_hybrid_arg_body_math() {
    // L1-39. Falls out of body math once operands peel; arg passes whole.
    assert_obs(
        "inc: (n -> n + 1)\nx: 1 & { name: \"Alice\" }\nout: x |> inc",
        "2",
    );
}

#[test]
fn red_apply_hybrid_arg_juxtaposition() {
    assert_obs("dbl: (n -> n * 2)\nx: 3 & { note: \"n\" }\nout: dbl x", "6");
}

#[test]
fn red_nested_hybrid_renders_collapsed() {
    // Collapse recurses through the rendered tree (unified law).
    assert_obs("out: { h: 3 & { n: \"x\" } }", "{\n  h: 3\n}");
}

#[test]
fn red_list_element_hybrid_renders_collapsed() {
    assert_obs("out: [1 & { n: \"a\" }, 2]", "[1, 2]");
}

// ─────────────────────────────────────────────────────────────────────────
// ACTIVE PINS — the duality and the non-value contexts must not move
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn pin_structural_hybrid_full_node() {
    // SYNTAX_07 §4 #6: <<x>> keeps the full node — %val visible.
    let src = "x: 1 & { name: \"Alice\" }\nst: <<x>>\nout: st";
    assert_obs(src, "{\n  %val: 1\n  name: \"Alice\"\n}");
}

#[test]
fn pin_structural_alias_stays_full() {
    // Ref-mediated observation = structural view (SYNTAX_07 §2 #4 live
    // reference); an alias of the structural handle keeps the full node.
    let src = "x: 1 & { name: \"Alice\" }\nst: <<x>>\nalias: st\nout: alias";
    assert_obs(src, "{\n  %val: 1\n  name: \"Alice\"\n}");
}

#[test]
fn pin_structural_literal_stays_full() {
    assert_obs(
        "out: <<1 & { name: \"Bob\" }>>",
        "{\n  %val: 1\n  name: \"Bob\"\n}",
    );
}

#[test]
fn pin_nav_hybrid_field() {
    assert_obs("x: 1 & { name: \"Alice\" }\nout: x.name", "\"Alice\"");
}

#[test]
fn pin_structural_literal_nav_transparent() {
    // ACCEPTANCE REPAIR pin (2026-07-13): the %structural mark is a
    // display filter, transparent to navigation. Delivery regression:
    // `lit: <<1 & {name:"Bob"}>>` then `lit.name` → `_` (marker combo
    // open-miss) while v0.2.6 gave "Bob". navigate_segments now unwraps
    // the mark like a pure wrapper (SYNTAX_07 §4 #7).
    assert_obs("lit: <<1 & { name: \"Bob\" }>>\nout: lit.name", "\"Bob\"");
    // Structural of a PATH navigates too (Ref-mediated).
    assert_obs(
        "x: 1 & { name: \"Alice\" }\nst: <<x>>\nout: st.name",
        "\"Alice\"",
    );
}

#[test]
fn pin_pipe_body_navigation_still_works() {
    // Argument passes WHOLE: body navigation must survive the fix.
    // (Red flag for peel-at-binding overreach.)
    assert_obs(
        "getname: (p -> p.name)\nx: 1 & { name: \"Alice\" }\nout: x |> getname",
        "\"Alice\"",
    );
}

#[test]
fn pin_eqeq_hybrid_peels() {
    // G1 (#12): already lawful.
    assert_obs("x: 1 & { name: \"Alice\" }\nout: x == 1", "#true");
}

#[test]
fn pin_lattice_eq_hybrid_no_peel() {
    // `=` compares extensional structure — hybrids never peel there.
    assert_obs("out: (3 & { note: \"n\" }) = 3", "#false");
}

#[test]
fn pin_pure_wrapper_collapse() {
    assert_obs("out: 1 & { %note: \"m\" }", "1");
}

#[test]
fn pin_plain_combo_observe_unchanged() {
    assert_obs("out: { name: \"Alice\" }", "{\n  name: \"Alice\"\n}");
}

#[test]
fn pin_plain_combo_math_stays_conflict() {
    // Peel recognizes %val ONLY — plain combos stay ⊥ in math.
    assert_bottom_conflict("out: { a: 1 } + 1");
}
