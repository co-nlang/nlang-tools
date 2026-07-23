// #cached solidification probes (2026-07-24, pre-committed by work order
// — docs/effect_cached_handover.md). 效應系統波 arc 2 (§4.2.4).
//
// RULING (SPEC_08 §4.2.4, no new ruling — user framing 2026-07-24):
// "once data is collapsed AND obtains a STABLE CAID, its active tags
// (#io/#nondet/#state) are transformed (solidified) to #cached IN THE
// OBSERVED RESULT" — 不動 CAID, 只影觀測結果. The operational trigger for
// "stable CAID" = the value was FETCHED FROM THE STORE by its content
// address (store-committed history is fixed). A freshly-computed value
// stays active (arc-1 pin / L2-83 holds). Re-activation (§4.1 matrix
// #cached & #io = #io | #cached) is FREE via arc-1 set-union: a cached
// value composed with a fresh active effect unions to {cached, active}.
//
// TRIGGER SCOPE (the hook): solidify at the USER-FACING fetch-by-CAID
// boundary only — ~%Discovery./fetch, ~%Discovery./find, `oo inspect`
// display. NOT raw get_value: universe commit-root reconstruction +
// refine monotonicity check (universe.rs) and refine-follow (lib.rs)
// must stay RAW so commit-chain CAIDs and content_hash comparisons are
// untouched (REAL_04 determinism); the NDP wire-serve (main.rs) serves
// RAW so the receiving peer solidifies on ITS own fetch.
//
// MEASURED (baseline, v0.2.34 dev): a store-fetched value keeps its
// active tag —
//   (fetch (save io)).%effect            → #io       (want #cached)
//   (fetch (save {a:io,b:nondet})).%effect→ #io|#nondet (want #cached)
//   { x: fetched_io, y: fresh_io }.%effect→ #io       (want #cached | #io)
// Fresh values, pure fetches, and fetched content are already correct
// and MUST NOT move. Canonical multi-tag order (MEASURED) is
// alphabetical: #cached | #io, #cached | #nondet.
//
// NOT in scope (ledgered follow-on): static guard #effect_violation
// (§4.3), runPure handler (§4.3), #ext: tags (§4.1), full tag-set CAID
// participation (§4.1). Cached participates in composition only.

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
        "nlang-effcached-{}-{}",
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

// save = ~%Discovery./identify_and_store  (engine.save: force + put_value)
// fetch = ~%Discovery./fetch              (store.get_value by CAID)

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — store-fetched values solidify to #cached (SPEC_08 §4.2.4)
// ─────────────────────────────────────────────────────────────────────────

#[test]
#[ignore]
fn red_fetch_io_solidifies() {
    // A store round-trip fixes the io result's history → #cached.
    assert_obs(
        "s: ~%Discovery./identify_and_store (~%Time.now _)\n\
         f: ~%Discovery./fetch s\n\
         out: f.%effect",
        "#cached",
    );
}

#[test]
#[ignore]
fn red_fetch_multi_active_collapses() {
    // Every active tag collapses to the single #cached (uncertainty fixed).
    assert_obs(
        "s: ~%Discovery./identify_and_store { a: (~%Time.now _), b: (~%Math./random _) }\n\
         f: ~%Discovery./fetch s\n\
         out: f.%effect",
        "#cached",
    );
}

#[test]
#[ignore]
fn red_fetch_nested_field_solidifies() {
    // Solidification is recursive — a nested field of a fetched combo
    // reads #cached too (whole retrieved subtree is fixed history).
    assert_obs(
        "s: ~%Discovery./identify_and_store { v: (~%Time.now _) }\n\
         f: ~%Discovery./fetch s\n\
         out: f.v.%effect",
        "#cached",
    );
}

#[test]
#[ignore]
fn red_reactivation_union() {
    // §4.2.4 re-activation / §4.1 matrix: a #cached value re-entering an
    // active-effect composition regains the active tag ALONGSIDE cached
    // (arc-1 set-union). Baseline: fetched shows #io so io|io = #io.
    assert_obs(
        "s: ~%Discovery./identify_and_store (~%Time.now _)\n\
         f: ~%Discovery./fetch s\n\
         both: { x: f, y: (~%Time.now _) }\n\
         out: both.%effect",
        "#cached | #io",
    );
}

#[test]
#[ignore]
fn red_fetch_display_tail() {
    // SPEC_11 §3.4 tail: the fetched value's diagnostic tail shows the
    // solidified tag (baseline showed #io).
    let got = observe_nlang(
        "s: ~%Discovery./identify_and_store (~%Time.now _)\n\
         out: ~%Discovery./fetch s",
        "out",
    );
    assert!(
        got.contains(";; %effect: #cached"),
        "fetched value carries the #cached tail: {got:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// ACTIVE PINS — invariants solidification must preserve
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn pin_fresh_io_stays_active() {
    // L2-83 twin: a freshly-computed io value (never store-fetched) is
    // still active #io — solidification triggers on stable CAID only.
    assert_obs("t: ~%Time.now _\nout: t.%effect", "#io");
}

#[test]
fn pin_fresh_combo_stays_active() {
    // A fresh combo with an io field is active #io, not #cached.
    assert_obs("c: { v: (~%Time.now _) }\nout: c.%effect", "#io");
}

#[test]
fn pin_fresh_multi_active_unchanged() {
    // arc-1 set-union on FRESH values is untouched: still #io | #nondet
    // (only store-fetch collapses actives to cached).
    assert_obs(
        "c: { a: (~%Time.now _), b: (~%Math./random _) }\nout: c.%effect",
        "#io | #nondet",
    );
}

#[test]
fn pin_fetch_pure_stays_pure() {
    // A fetched PURE value has no active tag to solidify → stays #pure.
    assert_obs(
        "s: ~%Discovery./identify_and_store 42\n\
         f: ~%Discovery./fetch s\n\
         out: f.%effect",
        "#pure",
    );
}

#[test]
fn pin_fetch_content_preserved() {
    // Solidification touches only the effect projection — the fetched
    // VALUE content round-trips unchanged.
    assert_obs(
        "s: ~%Discovery./identify_and_store 42\n\
         out: ~%Discovery./fetch s",
        "42",
    );
}

#[test]
fn pin_bottom_meta_whitelist_unchanged() {
    // ⊥ meta whitelist stays %cause/%caid — %effect passes the ⊥ through.
    let got = observe_nlang("bot: 1 & 2\nout: bot.%effect", "out");
    assert!(
        got.starts_with("_|_") && got.contains("#conflict"),
        "⊥.%effect passes the bottom through: {got:?}"
    );
}
