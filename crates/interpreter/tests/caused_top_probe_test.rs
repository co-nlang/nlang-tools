// Two-kinds-of-Top probes (2026-07-20, pre-committed by work order —
// docs/caused_top_handover.md).
//
// RULING C (2026-07-20, amends SPEC_01 §2.4.2 before it ever shipped):
// `_` was one spelling carrying two jobs, and absorption could not tell
// them apart:
//   * BARE `_` = lattice top, the user saying "anything" — it ABSORBS.
//   * CAUSED `_` (#static_cycle, #no_coordinate, …) = a PHASE marker,
//     "no answer here yet" — an epistemic statement, not a claim of
//     "unconstrained". It is a DIAGNOSTIC MEMBER and is exempt.
// Diagnostic exemption (blur + caused Top), three clauses as one:
//   1. never absorbed;
//   2. never absorbs;
//   3. a value CONTAINING a diagnostic member at ANY depth may not act
//      as the absorber — it only covers the other because some
//      coordinate is undetermined, and letting it swallow a known value
//      would erase the known with the unknown (`{v: 9} | p` where
//      `p.v` is a caused Top: BOTH branches stand).
// Open-miss navigation now mints caused Top `#no_coordinate` (was bare).
// Consumption still evaporates the cause (REAL_04 provenance class), so
// `(_#c | 3) + 1` still collapses under the bare-Top rule.
//
// MEASURED (dev @ absorption delivery): every diagnostic face collapses
// — `({a:1} | 7).a` → `_`, `(q.b).%cause` → `_` (bare, no cause),
// `{v:9} | p` at `.v` → `_`. Bare-Top and refinement absorption are
// healthy and must not move.
//
// Open migrations (acceptor, this commit): union_nav, union_bottom_cull,
// taint_scope (six faces), conformance L2-72 restored + L2-90/91/92.
// NOT in scope: undefined bare-NAME reads (`out: nosuchname` → Top;
// spread no-op law keys off it — ledgered), W4, effect-system wave.

use nlang_interpreter::{Ouroboros, Universe};
use nlang_parser::ast::{Path, PathAnchor, Span};
use nlang_parser::parse_program;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

fn tmp_dir() -> nlang_interpreter::ScratchDir {
    nlang_interpreter::ScratchDir::new("caustop")
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
// RED GATES — open miss becomes a caused Top (TAG_REGISTRY #no_coordinate)
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_open_miss_carries_cause() {
    // L2-91 twin. Display stays `_`; only the provenance is new.
    assert_obs("q: { a: 1 }\nout: (q.b).%cause", "#no_coordinate");
    assert_obs("q: { a: 1 }\nout: q.b", "_");
}

#[test]
fn pin_bare_top_has_no_cause() {
    // DEMOTED at calibration (protocol: a green red-gate is no gate) —
    // already true today. The other side of the split: an explicit `_`
    // is causeless, which is exactly why it absorbs. Pinned so the
    // #no_coordinate work cannot accidentally give bare Top a cause.
    assert_obs("t: _\nout: t.%cause", "_");
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — diagnostic exemption (clauses 1 & 2)
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_union_nav_open_miss_survives() {
    // L2-92 twin: branch 2 has no `.a` — that is a diagnostic, not a
    // licence to swallow branch 1.
    assert_obs("out: ({ a: 1 } | 7).a", "1 | _");
    assert_obs("u: {a: 1} | 7\nout: u.a", "1 | _");
}

#[test]
fn red_caused_top_neither_absorbs_nor_absorbed() {
    // Direct join of a static-cycle Top with a value: both stand.
    assert_obs("p: { v: p.v }\nout: p.v | 9", "9 | _");
    assert_obs("p: { v: p.v }\nout: 9 | p.v", "9 | _");
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — clause 3: containing a diagnostic disqualifies the absorber
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_container_with_caused_top_cannot_absorb() {
    // L2-72 twin. `{v: _#static_cycle}` ⊇ `{v: 9}` ONLY because v is
    // undetermined — absorbing would erase the known 9.
    assert_obs("p: { v: p.v }\nu: { v: 9 } | p\nout: u.v", "9 | _");
    assert_obs("p: { v: p.v }\nu: p | { v: 9 }\nout: u.v", "9 | _");
    assert_obs(
        "p: { v: p.v }\nu: { v: 9 } | p | { v: 8 }\nout: u.v",
        "8 | 9 | _",
    );
}

#[test]
fn pin_container_with_blur_cannot_absorb() {
    // DEMOTED at calibration: already green (blur's meet keeps the two
    // apart today). Same clause-3 face in its blur flavour — pinned so
    // the caused-Top wiring reaches the identical verdict by the RULED
    // route rather than by accident.
    let got = observe_nlang(
        &format!(
            "big: {}\nb: {{ v: big }}\nu: {{ v: 9 }} | b\nout: u.v",
            flat_chain(4000)
        ),
        "out",
    );
    assert!(
        got.contains(" | ") && got.contains("#blur"),
        "container holding a horizon must not absorb a known sibling: {got:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// ACTIVE PINS — the absorption wins that must NOT be undone
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn pin_bare_top_still_absorbs() {
    // L2-89: the user wrote `_` — swallowing is its meaning.
    assert_obs("out: 9 | _", "_");
    assert_obs("out: _ | 9", "_");
    assert_obs("u: _ | 9\nout: u + 1", "_");
}

#[test]
fn pin_refinement_absorption_intact() {
    assert_obs("out: (@int | 1) = @int", "#true");
    assert_obs("out: ({a: 1} | {a: 1, b: 2}) = {a: 1}", "#true");
    assert_obs("out: (1 | 3 | @int) = @int", "#true");
}

#[test]
fn pin_ordinary_unions_intact() {
    assert_obs("out: 1 | 2", "1 | 2");
    assert_obs("out: 1 | 1", "1");
    assert_obs("out: ({a: 1} | {a: 2}).a", "1 | 2");
    assert_obs("out: (1 & 2) | 9", "9");
}

#[test]
fn pin_static_cycle_cause_and_bare_display() {
    assert_obs("p: { v: p.v }\nout: (p.v).%cause", "#static_cycle");
    assert_obs("p: { v: p.v }\nout: p.v", "_");
}

#[test]
fn pin_blur_exemption_intact() {
    let src = format!("big: {}\n", flat_chain(4000));
    let got = observe_nlang(&format!("{src}out: {{v: 1}} | big"), "out");
    assert!(
        got.contains(" | ") && got.contains("#blur"),
        "blur branch still survives absorption: {got:?}"
    );
}
