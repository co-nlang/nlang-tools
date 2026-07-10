// Stage 3 acceptance probes: the two ultimate-vector lines the Stage 3
// delivery omitted from its own suite (handover §4, 2026-07-07):
//   _: v.w.x.a  ;; → "Logic"（路徑導向，有限步）— PENDING remediation (deref
//                  re-entry must supply the $ frame; currently #no_context)
//   _: v        ;; 全量 → 視界截斷（自指迴歸至 fuel/depth 視界）
//
// Acceptance-fix context (2026-07-07): unify Ref-preservation arms + deref
// cost + force_recursive depth guard were added during acceptance; before
// those, evolve snapshotted <<_.>> to the pristine system root (A-case
// semantics) and full observation of v crashed with a stack overflow.

use nlang_interpreter::{Ouroboros, Universe, Value};
use nlang_interpreter::value::{BottomCause, BlurCause};
use nlang_parser::parse_program;
use nlang_parser::ast::AtomKind;
use std::fs;
use std::path::PathBuf;

fn tmp_dir(tag: &str) -> PathBuf {
    let mut d = std::env::temp_dir();
    d.push(format!("nlang-stage3-probe-{}-{}", tag, std::process::id()));
    fs::remove_dir_all(&d).ok();
    fs::create_dir_all(&d).unwrap();
    d
}

fn build_universe(dir: &PathBuf) -> (Ouroboros, Universe) {
    build_universe_with(dir, true)
}

// minimal=false: stdlib-heavy root (realistic); minimal=true: bare root — the
// self-referential horizon probe uses this because each deref cycle deep-copies
// the whole root (memory scales root-size × horizon-depth; see remediation).
fn build_universe_with(dir: &PathBuf, minimal: bool) -> (Ouroboros, Universe) {
    let engine = Ouroboros::init(dir).unwrap();
    let root = if minimal {
        nlang_interpreter::value::ComboVal::default()
    } else {
        engine.root_with_system()
    };
    let mut universe = Universe::new(None, root);
    let src = "s: \"Logic\" |> { a: $ }\nw: { x: $.s }\nv: <<_.>> |> <<_.>>";
    let program = parse_program(src).unwrap();
    for field in &program.fields {
        universe.evolve(&engine, field).unwrap();
    }
    (engine, universe)
}

fn path_of(segments: &[&str]) -> nlang_parser::ast::Path {
    nlang_parser::ast::Path {
        anchor: nlang_parser::ast::PathAnchor::Bare,
        segments: segments.iter().map(|s| s.to_string()).collect(),
        span: nlang_parser::ast::Span::default(),
    }
}

fn contains_horizon(v: &Value) -> bool {
    match v {
        Value::Blur(bd) => matches!(bd.cause,
            BlurCause::FuelExhausted | BlurCause::StackOverflow),
        Value::Bottom(d) => matches!(d.cause,
            BottomCause::FuelExhausted | BottomCause::Divergent),
        // walk by reference: all_fields_iter() yields OWNED clones, which on a
        // horizon-deep nested chain is O(depth x total_size) memory — the
        // helper OOMs on a value the engine produced just fine.
        Value::Combo(cv) => cv.data.values()
            .chain(cv.types.values()).chain(cv.rules.values())
            .chain(cv.meta.values()).chain(cv.system.values())
            .chain(cv.local.values())
            .any(contains_horizon),
        Value::Union(items) => items.iter().any(contains_horizon),
        _ => false,
    }
}

// C-case ground truth: the live reference must survive evolve un-dereferenced.
// (Pre-fix, unify's missing Ref arms snapshotted it — the rejected A case.)
#[test]
fn probe_evolve_stores_live_ref_not_snapshot() {
    let dir = tmp_dir("store");
    let (_engine, universe) = build_universe(&dir);
    match universe.staged.get_field("v") {
        Some(Value::Ref(_)) => {}
        other => panic!(
            "v must be stored as a live Ref (C-case), not an evolve-time snapshot; got {:?}",
            other.map(|v| format!("{:?}", v).chars().take(80).collect::<String>())),
    }
}

// Vector line 4: observe v.w.x.a → "Logic" (path-directed, finite).
// REMEDIATION COMPLETE (F1): deref re-entry supplies the $ frame.
#[test]
fn probe_v_w_x_a_yields_logic() {
    let dir = tmp_dir("nav");
    let (engine, universe) = build_universe(&dir);
    let obs = universe.observe(&engine, &path_of(&["v", "w", "x", "a"]));
    match &obs {
        Value::Atom(AtomKind::Str(s), _, _) => assert_eq!(s, "Logic"),
        other => panic!("v.w.x.a should observe to \"Logic\", got {:?}", other),
    }
}

// Vector line 5: observe v (full) → horizon truncation, NOT a hang and NOT a
// stack-overflow crash. Runs on a wide-stack thread: the semantic horizon
// (fuel/depth) engages at ~150 deref cycles, which exceeds the 2MiB default
// test-thread stack under fat debug frames (production main thread is 8MiB).
#[test]
fn probe_v_full_hits_fuel_horizon() {
    run_full_v_probe("full", true);
}

// F2 acceptance: same probe against the stdlib-heavy root. Pre-Arc this was
// SIGKILL (OOM): sub_context deep-copied the whole root on every thunk force,
// O(depth × N_fields × |root|). With root behind Arc the clone is a refcount
// bump and the horizon engages before memory does.
#[test]
fn probe_v_full_stdlib_root_hits_horizon_no_oom() {
    run_full_v_probe("full-stdlib", false);
}

fn run_full_v_probe(tag: &'static str, minimal: bool) {
    let handle = std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(move || {
            let dir = tmp_dir(tag);
            let (engine, universe) = build_universe_with(&dir, minimal);
            let obs = universe.observe(&engine, &path_of(&["v"]));
            assert!(contains_horizon(&obs),
                "full observation of self-referential v should truncate at the horizon, got a {} without horizon markers",
                match &obs { Value::Combo(_) => "Combo", _ => "non-Combo" });
        })
        .unwrap();
    handle.join().unwrap();
}
