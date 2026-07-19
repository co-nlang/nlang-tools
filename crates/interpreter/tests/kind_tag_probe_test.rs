// %kind tag unification probes (2026-07-19, pre-committed by work
// order — docs/kind_tag_handover.md). %kind super-conflict ruling B3:
// `#type` wins; `#type_constraint` retires everywhere in the engine
// (spec keeps the string only at REAL_01 L224 — a $kind field in the
// LSP-protocol JSON layer, different namespace).
//
// MEASURED (v0.2.23): engine mints TWO %kind spellings for the type
// role — stdlib type nodes (@option/@result, lib.rs) mint
// `%kind: #type`; nominal constraint markers (type_constraint.rs:60,
// dispatch.rs:101) mint `%kind: #type_constraint`. Reader at
// type_constraint.rs:244 (+ inline at :126) keys on the string
// "type_constraint" — the is-marker check must migrate WITH the mints
// and must NOT start matching stdlib type nodes (they carry
// %kind: #type + %name, no %type payload).
//
// LAW: SPEC_03 §4 role table (`%kind: #type`) + SPEC_05 §3.2 note
// (2026-07-19): `#type` = canonical; internal payload field stays
// engine-internal (B2 — spelling unpromised, display unchanged).
// CAID note: marker cocoons' %kind tag change → one-time legal CAID
// shift for constraint-marker nodes.
// NOT in scope: %super/%predicate implementation (B5, queued);
//   marker payload field rename (B2 leaves it internal as-is);
//   type-key dispatch spelling `{ @int: … }` (measured inert in pipe
//   form — separate ledger, do not "fix" en route).

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
        "nlang-kindtag-{}-{}",
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
// RED GATES — marker %kind must be the canonical #type
// ─────────────────────────────────────────────────────────────────────────

#[test]
#[ignore] // RED GATE — remove at delivery
fn red_marker_kind_is_type() {
    // Builtin marker cocoon in structural view: canonical role tag.
    let got = observe_nlang("out: <<@{ @int }>>", "out");
    assert!(
        !got.contains("type_constraint") && got.contains("%kind: #type"),
        "marker %kind must be canonical #type: {got:?}"
    );
}

#[test]
#[ignore] // RED GATE — remove at delivery
fn red_marker_kind_str_builtin() {
    // Second builtin spelling — same law, guards a per-site miss.
    let got = observe_nlang("out: <<@{ @str }>>", "out");
    assert!(
        !got.contains("type_constraint") && got.contains("%kind: #type"),
        "@str marker %kind must be canonical #type: {got:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// ACTIVE PINS — enforcement behavior must not move
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn pin_builtin_refine_works() {
    assert_obs("x: 5 & @int\nout: x", "5");
    assert_obs("out: (\"s\" & @int).%cause", "#conflict");
}

#[test]
fn pin_nominal_refine_works() {
    assert_obs("@Pos: 1..\nx: 5 & @Pos\nout: x", "5");
    assert_obs("@Pos: 1..\nout: (0 & @Pos).%cause", "#conflict");
}

#[test]
fn pin_stdlib_type_node_not_marker() {
    // @option carries %kind: #type + %name TODAY — after unification
    // the is-marker check must not swallow stdlib type nodes.
    assert_obs("o: #none & @option\nout: o", "#none");
}

#[test]
fn pin_marker_payload_display_unchanged() {
    // B2: payload field is engine-internal — this arc does NOT rename
    // or hide it (its spelling is unpromised, but drive-by changes are
    // out of scope).
    let got = observe_nlang("out: <<@{ @int }>>", "out");
    assert!(
        got.contains("\"int\""),
        "marker payload display must not change in this arc: {got:?}"
    );
}

#[test]
fn pin_anonset_transparent_unchanged() {
    // @{ e } ≡ e for resolvable expressions (2026-07-10 ruling).
    assert_obs("out: @{ 5 }", "5");
    assert_obs("@Pos: 1..\nout: <<@{ @Pos }>>", "1..#_");
}
