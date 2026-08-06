// Private-axis enforcement probes (2026-07-15, pre-committed by work
// order — docs/private_axis_handover.md).
//
// RULING (SPEC_04 §3.1 #1–#5 + §2.2, approved 2026-07-15). Measured on
// v0.2.10: the private axis is fully INVERTED — outward blockage lets
// everything through (`p.~s` leaks, sibling steal leaks, `_.~x` leaks),
// inward visibility is fully broken (the spec's OWN factory example
// returns `_`; bare `~key` does not resolve through the scope chain
// inside combos — only root-level privates live, because root fields sit
// directly in scope), morphism capture dies (⊥ #conflict from `_` + 3),
// and external display shows the secrets outright.
// Law:
//   #1/#2 inward: bare `~key` resolves via the scope chain (defining
//        combo + descendants; ancestor lifting = shared privacy).
//   #3/#5 outward: a dotted `.~key` segment descending into a combo is
//        EXTERNAL LOCATING → ⊥ #private_access_violation, always
//        (insiders always have the bare name; `_.~key` included).
//   #4  display projection (collapsed AND structural) strips the local
//        axis; CAID/`=`/content identity keep all six axes.
//   §3.3 value capture: a morphism body's bare `~key` resolves through
//        its DEFINING closure scope; external callers get the value,
//        never a path back in.
//   §2.2 `~.` anchor ABOLISHED (redundant with bare `~key`; grammar
//        never had it — pinned so it cannot return unadjudicated).
// TRAP: system-axis keys (`~%Config` …) also start with `~` — the strip
// and the dotted block MUST exempt the `~%` prefix (L2-23 guards).

use nlang_interpreter::{Ouroboros, Universe};
use nlang_parser::ast::{Path, PathAnchor, Span};
use nlang_parser::parse_program;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

fn tmp_dir() -> nlang_interpreter::ScratchDir {
    nlang_interpreter::ScratchDir::new("privax")
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

fn assert_private_violation(src: &str) {
    let got = observe_nlang(src, "out");
    assert!(
        got.starts_with("_|_") && got.contains("private_access_violation"),
        "{src:?} :: out — expected privacy violation, got {got:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — outward blockage (#3/#5)
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_outward_dotted_blocked() {
    // Today: 1 (leak).
    assert_private_violation("p: { ~s: 1 }\nout: p.~s");
}

#[test]
fn red_sibling_steal_blocked() {
    assert_private_violation("a: { ~s: 1 }\nb: { steal: a.~s }\nout: b.steal");
}

#[test]
fn red_root_anchor_dotted_blocked() {
    // `_.` jumps to root as an external coordinate — descent is external.
    assert_private_violation("~x: 7\nout: _.~x");
}

#[test]
fn red_violation_type_meta() {
    // L2-32 mirror. cocoon_shape: %type is not a meta alias — ⊥ passes
    // through with its cause (read via .%cause for the tag form).
    let got = observe_nlang("p: { ~s: 1 }\nout: (p.~s).%type", "out");
    assert!(
        got.starts_with("_|_") && got.contains("#private_access_violation"),
        "⊥.%type must pass private_access_violation through: {got:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — inward visibility (#1/#2)
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_spec_factory_example() {
    // L2-33: the spec's own §3.2 example. Today: `_`.
    assert_obs(
        "factory: {\n    ~secret_seed: 42\n    product_a: { val: ~secret_seed + 1 }\n}\nout: factory.product_a.val",
        "43",
    );
}

#[test]
fn red_inward_same_combo() {
    assert_obs("p: { ~s: 1, get: ~s + 1 }\nout: p.get", "2");
}

#[test]
fn red_inward_grandchild_lifting() {
    // Shared privacy reaches all descendants.
    assert_obs("p: { ~s: 1, c: { d: { v: ~s + 2 } } }\nout: p.c.d.v", "3");
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — morphism value capture (§3.3)
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_morphism_capture() {
    // L2-34. Today: ⊥ #conflict (`_` + 3).
    assert_obs("p: { ~s: 5, add: (x -> x + ~s) }\nout: 3 |> p.add", "8");
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — display projection strips local axis (#4)
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_display_strips_local() {
    // L2-35.
    let got = observe_nlang("p: { ~s: 1, pub: 2 }\nout: p", "out");
    assert!(
        !got.contains("~s") && got.contains("pub"),
        "collapsed display must strip local axis: {got:?}"
    );
}

#[test]
fn red_structural_strips_local() {
    let got = observe_nlang("p: { ~s: 1, pub: 2 }\nout: <<p>>", "out");
    assert!(
        !got.contains("~s") && got.contains("pub"),
        "structural display must strip local axis too: {got:?}"
    );
}

#[test]
fn red_nested_display_strips_local() {
    let got = observe_nlang("w: { inner: { ~k: 9, v: 1 } }\nout: w", "out");
    assert!(
        !got.contains("~k") && got.contains("v: 1"),
        "strip applies at every depth: {got:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// ACTIVE PINS — living faces, system-axis exemption, identity axes
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn pin_root_bare_private_lives() {
    // Root-level bare resolution works today and must keep working
    // (root fields sit directly in scope = insider route).
    assert_obs("~x: 7\nout: ~x + 1", "8");
}

#[test]
fn pin_public_nav_unaffected() {
    assert_obs("p: { ~s: 1, pub: 2 }\nout: p.pub", "2");
}

#[test]
fn pin_system_axis_exempt_nav() {
    // TRAP guard: `~%` is the SYSTEM axis, not local — dotted navigation
    // must stay open (L2-23 companion).
    assert_obs("out: ~%Config.fuel", "10000");
}

#[test]
fn pin_eq_keeps_local_axis() {
    // G1 #11 six axes: local participates in `=` — strip is display-only.
    assert_obs("x: { ~s: 1, a: 2 }\ny: { a: 2 }\nout: x = y", "#false");
}

#[test]
fn pin_content_id_keeps_local_axis() {
    assert_obs(
        "x: { ~s: 1, a: 2 }\ny: { a: 2 }\nout: x.%id == y.%id",
        "#false",
    );
}

#[test]
fn pin_tilde_anchor_stays_unparsed() {
    // §2.2 abolition: `~.` must not come back without adjudication.
    assert!(
        parse_program("p: { ~s: 1, me: ~.s }\nout: p.me").is_err(),
        "~. anchor must stay out of the grammar"
    );
}

#[test]
fn pin_root_private_spread_lives() {
    // Corpus shape (test_entropy): root-level spread of a private = bare
    // route, must keep working.
    assert_obs("~c: { x: 1 }\nd: { ...~c, z: 3 }\nout: d.x", "1");
}

#[test]
fn pin_blur_and_bottom_meta_unaffected() {
    // Adjacent: ⊥ %cause still collapses to the tag; %type is passthrough.
    assert_obs("bad: 1 & 2\nout: bad.%cause", "#conflict");
}
