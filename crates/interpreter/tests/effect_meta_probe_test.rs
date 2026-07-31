// %effect meta-read + diagnostic-annotation-layer probes (2026-07-20,
// pre-committed by work order — docs/effect_meta_handover.md).
//
// RULING C (2026-07-20): the `;; %effect:` display tail is CODIFIED as
// the first statutory member of the diagnostic annotation layer
// (SPEC_11 §3.4 — engine MAY append comment-form diagnostics; parser-
// invisible; never the sole carrier of semantic info), AND `.%effect`
// becomes a readable meta lens (SPEC_08 §4.1 元欄觀測): default #pure,
// open combo = contagion join (§4.2.1, already computed at
// construction), cocoon = shield #pure, unions distribute per
// projection, explicit %effect field wins (SYNTAX_08 writable meta).
//
// MEASURED (v0.2.26): `.%effect` reads Top (`_`) on EVERY regular
// value — atoms, combos, cocoons, unions. The stored tags exist
// (Value::effect(), ComboVal effect join at construction with the
// closed-skip shield) but no navigation arm exposes them. The orphan
// tests/effect_taint.n "passes" only because `_ == #io` → `_` and the
// test harness counts Top as PASS (vacuous truth — ledgered separate
// case).
//
// NOT in scope: solidification #io→#cached (§4.2.4), static guard
// #effect_violation (§4.3), multi-tag set-join matrix (engine max is a
// lawful single-tag simplification), CAID participation (§4.1) — all
// ledgered to the 效應系統波 (REAL_05 external boundary).

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
        "nlang-effmeta-{}-{}",
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
// RED GATES — `.%effect` meta lens (SPEC_08 §4.1 元欄觀測)
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_effect_read_io_atom() {
    // L2-83 twin.
    assert_obs("t: ~%Time.now _\nout: t.%effect", "#io");
}

#[test]
fn red_effect_read_pure_default() {
    assert_obs("out: (42).%effect", "#pure");
    assert_obs("pc: { v: 1 }\nout: pc.%effect", "#pure");
}

#[test]
fn red_effect_read_combo_contagion() {
    // §4.2.1 structural contagion — already joined at construction
    // (open combos: me = max(field effects)); the lens just exposes it.
    assert_obs("c: { v: ~%Time.now _ }\nout: c.%effect", "#io");
}

#[test]
fn red_effect_read_cocoon_shield() {
    // L2-84 twin. §4.2.1 Shield: contagion stops at the cocoon wall —
    // the cocoon's OWN tag stays #pure even with an io field inside.
    assert_obs("k: {{ v: ~%Time.now _ }}\nout: k.%effect", "#pure");
}

#[test]
fn red_effect_read_nondet() {
    assert_obs("r: ~%Math./random _\nout: r.%effect", "#nondet");
}

#[test]
fn red_effect_read_union_distributes() {
    // Unions distribute lenses per projection (SPEC_07); canonical
    // display order sorts tag atoms lexically (SPEC_01 §2.4.1).
    assert_obs("out: (5 | (~%Time.now _)).%effect", "#io | #pure");
}

#[test]
fn red_effect_read_unify_join() {
    // §4.1 composition: & unification joins effects (pure ∘ io = io).
    // This is the honest twin of the orphan tests/effect_taint.n.
    assert_obs("u1: { a: 1 } & { b: ~%Time.now _ }\nout: u1.%effect", "#io");
}

// ─────────────────────────────────────────────────────────────────────────
// ACTIVE PINS — annotation-layer shape + boundaries that must not move
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn pin_effect_tail_io_atom() {
    // SPEC_11 §3.4 statutory member: io atom carries the comment tail.
    let got = observe_nlang("t: ~%Time.now _\nout: t", "out");
    assert!(
        got.contains(";; %effect: #io"),
        "io atom keeps the diagnostic tail: {got:?}"
    );
}

#[test]
fn pin_effect_tail_pure_silent() {
    // Pure values show NO tail — the annotation layer stays silent.
    assert_obs("out: 42", "42");
}

#[test]
fn pin_effect_field_spoof_precedence() {
    // Explicit %effect FIELD is a stored writable meta field
    // (SYNTAX_08) — field lookup wins over the engine tag lens.
    assert_obs("spoof: { %effect: #io }\nout: spoof.%effect", "#io");
}

#[test]
fn pin_bottom_meta_whitelist_unchanged() {
    // ⊥ meta whitelist stays %cause/%caid — %effect is NOT added;
    // the read passes the ⊥ through (F1 compositionality).
    let got = observe_nlang("bot: 1 & 2\nout: bot.%effect", "out");
    assert!(
        got.starts_with("_|_") && got.contains("#conflict"),
        "⊥.%effect passes the bottom through: {got:?}"
    );
    assert_obs("bot: 1 & 2\nout: (bot).%cause", "#conflict");
}

#[test]
fn repair_pin_cocoon_shield_in_parent() {
    // ACCEPTANCE REPAIR (2026-07-20): predict_effect's Combo arm joined
    // fields regardless of `closed` — a cocoon LITERAL inside a combo
    // leaked its interior io into the parent's %effect (#io), while the
    // ALIAS spelling of the same cocoon stayed #pure (value-side effect
    // is healthy). §4.2.1: contagion stops at the cocoon wall in BOTH
    // spellings; open-in-open nesting still propagates.
    assert_obs("w: { k: {{ v: ~%Time.now _ }} }\nout: w.%effect", "#pure");
    assert_obs(
        "kref: {{ v: ~%Time.now _ }}\nw3: { k: kref }\nout: w3.%effect",
        "#pure",
    );
    assert_obs("w2: { c: { v: ~%Time.now _ } }\nout: w2.%effect", "#io");
}

#[test]
fn pin_blur_meta_whitelist_unchanged() {
    // Blur meta whitelist stays %cause/%caid — %effect read on a blur
    // absorbs (horizon snapshot; SPEC_08 §3.2.2 #5).
    let got = observe_nlang(
        &format!("big: {}\nout: (big).%effect", flat_chain(4000)),
        "out",
    );
    assert!(
        got.starts_with("#blur"),
        "blur.%effect absorbs into the horizon: {got:?}"
    );
}
