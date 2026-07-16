// Forward-reference resolution probes (2026-07-12 —
// docs/forward_ref_handover.md).
//
// Ruling: fields of one one-shot program are SIMULTANEOUS (SPEC_03 merge
// commutativity). A bare-path reference (`out: mid`, thunk lives at out)
// is NOT a self-loop (`s: { v: s.v }`, thunk lives at s.v). True cycles
// stay ⊥ #divergent (both-sides pins below).

use nlang_interpreter::{Ouroboros, Universe, Value};
use nlang_interpreter::value::BottomCause;
use nlang_parser::parse_program;
use nlang_parser::ast::{AtomKind, Path, PathAnchor, Span};
use num_bigint::BigInt;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

fn tmp_dir() -> PathBuf {
    let mut d = std::env::temp_dir();
    d.push(format!(
        "nlang-fwdref-{}-{}",
        std::process::id(),
        DIR_SEQ.fetch_add(1, Ordering::SeqCst)
    ));
    fs::remove_dir_all(&d).ok();
    fs::create_dir_all(&d).unwrap();
    d
}

/// 64 MiB thread — debug test stacks are too small for eval recursion.
fn run_observe(src: &str, path: &str) -> Value {
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
                universe.evolve(&engine, f).unwrap();
            }
            let p = Path {
                anchor: PathAnchor::Bare,
                segments: path.split('.').map(|s| s.to_string()).collect(),
                span: Span::default(),
            };
            universe.observe(&engine, &p)
        })
        .unwrap()
        .join()
        .unwrap()
}

fn assert_int(src: &str, path: &str, expect: i64) {
    match run_observe(src, path) {
        Value::Atom(AtomKind::Int(n), _, _) => {
            assert_eq!(n, BigInt::from(expect), "{src:?} :: {path}")
        }
        other => panic!("{src:?} :: {path} must be {expect}, got {other:?}"),
    }
}

fn assert_divergent(src: &str, path: &str) {
    match run_observe(src, path) {
        Value::Bottom(d) => assert_eq!(
            d.cause,
            BottomCause::Divergent,
            "{src:?} :: {path} must be #divergent, got cause {:?}",
            d.cause
        ),
        other => panic!("{src:?} :: {path} must be _|_ #divergent, got {other:?}"),
    }
}

// ─────────────────────────────────────────────────────────────────────────
// RED LINES — bare-path reference chains false-trigger #divergent today
// ─────────────────────────────────────────────────────────────────────────

#[test]
// RED LINE: 3-hop bare-path chain is a reference, not a cycle
fn fwd_chain_resolves() {
    assert_int("out: mid\nmid: base\nbase: 1", "out", 1);
}

#[test]
// RED LINE: 4-hop bare-path chain — same mechanism, deeper
fn fwd_chain_deep_resolves() {
    assert_int("o: c1\nc1: c2\nc2: c3\nc3: 9", "o", 9);
}

// ─────────────────────────────────────────────────────────────────────────
// ACTIVE pins — forward refs that WORK today (engine level) must survive,
// and true cycles must STAY divergent (⊥-side pins, both-sides rule)
// ─────────────────────────────────────────────────────────────────────────

#[test] // ACTIVE pin: bare forward ref resolves in-process today
fn pin_fwd_bare() {
    assert_int("out: a\na: 5", "out", 5);
}

#[test] // ACTIVE pin: two forward refs in one expression
fn pin_fwd_two_operands() {
    assert_int("out: a + b\na: 2\nb: 3", "out", 5);
}

#[test] // ACTIVE pin: forward ref to a morphism
fn pin_fwd_morphism() {
    assert_int("out: /f 5\nf: (n -> n * 2)", "out", 10);
}

#[test] // ACTIVE pin: forward ref from inside a container
fn pin_fwd_from_container() {
    assert_int("s: { v: w }\nw: 7", "s.v", 7);
}

#[test] // ACTIVE pin: math-shaped chain already resolves (no path_coord)
fn pin_fwd_chain_math() {
    assert_int("out: mid + 1\nmid: base + 1\nbase: 1", "out", 3);
}

#[test] // ACTIVE pin: forward ref + later refinement of the target
fn pin_fwd_then_refine() {
    assert_int("out: a\na: 1..9\na: 5", "out", 5);
}

// MIGRATED (2026-07-16, by the acceptor): `pin_ref_cycle_still_divergent`
// froze root `a: b / b: a` → ⊥ as an engineering guard against the chain
// fix UN-DETECTING cycles. SPEC_12 §1.1 (static-cycle adjudication) rules
// the pure-reference cycle → Top — a deliberate lawful answer, not a
// detection loss; transform cycles stay pinned ⊥ right below. Successor
// gate: red_root_static_mutual_top in static_cycle_probe_test.rs.

#[test] // ACTIVE pin (⊥ side): reference cycle through math stays divergent
fn pin_ref_cycle_math_still_divergent() {
    assert_divergent("a: b\nb: a + 1\nout: a", "out");
}

#[test] // ACTIVE pin: backward ref control
fn pin_bwd_control() {
    assert_int("a: 5\nout: a + 1", "out", 6);
}
