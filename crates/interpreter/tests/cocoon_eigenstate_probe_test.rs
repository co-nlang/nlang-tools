// Cocoon eigenstate-default probes (2026-07-16, pre-committed by work
// order — docs/cocoon_eigenstate_handover.md).
//
// LAW (SPEC_03 §1.2/§1.3, EXISTING — engine-follows-law, zero rulings):
//   §1.2 #1  reading an undefined field of a Cocoon returns ⊥ immediately
//            (%cause #missing_key — the §1.2.1 example's spelling).
//   §1.2 #2  merge rejection fires only for NON-TOP extra fields in the
//            other operand; a Top field is no constraint and must pass.
//   §1.3     eigenstate default: Cocoon.k = ⊥ for any undefined k.
// MEASURED on v0.2.14: access face fully open (`{{a:1}}.b`, `{{}}.x`,
// cause-cocoon `.zz` all `_`); Top-field merge MISREJECTED
// (`cc & {a:1, b:_}` → ⊥ #missing_key); union nav keeps the dead branch
// (`({{a:1}} | {b:2}).b` → `_ | 2`, should cull to `2`).
// BOUNDARIES (do not cross):
//   - `%`-meta and `~%` system segments are OTHER AXES — meta reads stay
//     open per the F-series laws (%kind/%cause pins below).
//   - Bare-NAME resolution misses inside a cocoon body are §2.1 lexical
//     open-world (`{{d: zz+1}}.d` → `_` stays) — coordinate access `.k`
//     is the eigenstate axis; the two mechanisms must not be conflated.
//   - Open combos and atoms keep the F4 open-world nav (`{a:1}.b` → `_`).

use nlang_interpreter::{Ouroboros, Universe};
use nlang_parser::ast::{Path, PathAnchor, Span};
use nlang_parser::parse_program;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

fn tmp_dir() -> PathBuf {
    let mut d = std::env::temp_dir();
    d.push(format!(
        "nlang-ccegn-{}-{}",
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

fn assert_missing_key(src: &str) {
    let got = observe_nlang(src, "out");
    assert!(
        got.starts_with("_|_") && got.contains("missing_key"),
        "{src:?} :: out — expected ⊥ #missing_key, got {got:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — eigenstate access (§1.2 #1 / §1.3)
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_cocoon_access_bottom() {
    // L2-50. Today: `_`. Both spellings (inline nav + binding split).
    assert_missing_key("cc: {{ a: 1 }}\nout: cc.b");
    assert_missing_key("cc: {{ a: 1 }}\ng: cc.b\nout: g");
}

#[test]
fn red_cocoon_access_type_meta() {
    // cocoon_shape: %type alias retired — use passthrough form; %cause
    // still collapses to the class tag.
    let got = observe_nlang("cc: {{ a: 1 }}\nout: (cc.b).%type", "out");
    assert!(
        got.starts_with("_|_") && got.contains("#missing_key"),
        "⊥.%type must pass #missing_key through: {got:?}"
    );
}

#[test]
fn red_empty_cocoon_access() {
    assert_missing_key("cc: {{}}\nout: cc.x");
}

#[test]
fn red_nested_cocoon_access() {
    assert_missing_key("w: { cc: {{ a: 1 }} }\nout: w.cc.b");
}

#[test]
fn red_cause_cocoon_undefined_key() {
    // REAL_04 §1: the %cause carrier IS a cocoon — same law. Today: `_`.
    assert_missing_key("bad: 1 & 2\nout: (bad.%cause).zz");
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — merge rejection scope (§1.2 #2: non-Top fields only)
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_merge_top_field_allowed() {
    // L2-51. Today: ⊥ #missing_key (misrejection — Top is no constraint).
    assert_obs("cc: {{ a: 1 }}\nr: cc & { a: 1, b: _ }\nout: r.a", "1");
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — union navigation culls the ⊥ branch
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_union_missing_cull() {
    // L2-52. Today: `_ | 2`.
    assert_obs("u: {{ a: 1 }} | { b: 2 }\nout: u.b", "2");
}

// ─────────────────────────────────────────────────────────────────────────
// ACTIVE PINS — healthy faces + axis boundaries
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn pin_merge_rejection_spec_example() {
    // SPEC_03 §1.2.1's own example — already lawful.
    let got = observe_nlang(
        "@StrictUser: {{ name: @str }}\nresult: @StrictUser & { name: \"Alice\", age: 30 }\nout: result",
        "out",
    );
    assert!(
        got.starts_with("_|_") && got.contains("missing_key"),
        "merge rejection regressed: {got:?}"
    );
}

#[test]
fn pin_merge_conflict_same_key() {
    let got = observe_nlang("cc: {{ a: 1 }}\nout: cc & { a: 2 }", "out");
    assert!(
        got.starts_with("_|_") && got.contains("conflict"),
        "same-key conflict regressed: {got:?}"
    );
}

#[test]
fn pin_cocoon_defined_access() {
    assert_obs("cc: {{ a: 1 }}\nout: cc.a", "1");
}

#[test]
fn pin_meta_reads_stay_open() {
    // %-meta is another axis — the eigenstate ⊥ must not swallow it.
    assert_obs("cc: {{ a: 1 }}\nout: cc.%kind", "_");
    assert_obs("cc: {{ a: 1 }}\nout: cc.%cause", "_");
}

#[test]
fn pin_cause_cocoon_val_still_reads() {
    // F2: %val on the cause carrier keeps working.
    assert_obs("bad: 1 & 2\nout: (bad.%cause).%val", "#conflict");
}

#[test]
fn pin_bare_lift_inside_cocoon() {
    // Lexical chain (previous arc) unaffected.
    assert_obs("k: 5\ncc: {{ d: k + 1 }}\nout: cc.d", "6");
}

#[test]
fn pin_bare_miss_inside_cocoon_stays_open() {
    // BOUNDARY: lexical resolution miss is §2.1 open world — NOT the
    // coordinate-access eigenstate axis. Must stay `_`.
    assert_obs("cc: {{ d: zz + 1 }}\nout: cc.d", "_");
}

#[test]
fn pin_open_combo_access_stays_open() {
    assert_obs("c: { a: 1 }\nout: c.b", "_");
}

#[test]
fn pin_atom_nav_stays_open() {
    // F4 (L2-31): atoms keep the open world.
    assert_obs("out: (1).name", "_");
}

#[test]
fn pin_cocoon_unbox_spread() {
    assert_obs("q: { a: 1, ...{{ b: 2 }} }\nout: q.b", "2");
}
