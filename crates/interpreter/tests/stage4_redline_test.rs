// Stage 4 red-line probes (acceptance, 2026-07-08): the two lines the work
// order prescribed (4c #2) that the delivery omitted.
//
// A. F1 × memo interaction: the memo key's context component must reflect the
//    EFFECTIVE binding (thunk.context OR observer's frame), not just the
//    thunk's own slot. Otherwise observing v.w.x.a ("Logic", bound through
//    the deref frame) poisons the key that direct w.x (#no_context) shares.
// B. Evolve between observations: memo must not serve pre-evolution values
//    (root CAID key component = natural invalidation).

use nlang_interpreter::value::{BottomCause, ComboVal};
use nlang_interpreter::{Ouroboros, Universe, Value};
use nlang_parser::ast::{AtomKind, Path, PathAnchor, Span};
use nlang_parser::parse_program;
use std::fs;
use std::path::PathBuf;

fn tmp_dir(tag: &str) -> PathBuf {
    let mut d = std::env::temp_dir();
    d.push(format!(
        "nlang-stage4-redline-{}-{}",
        tag,
        std::process::id()
    ));
    fs::remove_dir_all(&d).ok();
    fs::create_dir_all(&d).unwrap();
    d
}

fn path_of(segments: &[&str]) -> Path {
    Path {
        anchor: PathAnchor::Bare,
        segments: segments.iter().map(|s| s.to_string()).collect(),
        span: Span::default(),
    }
}

// A. observe v.w.x.a first (caches "Logic" under the shared thunk), then
//    direct w.x MUST still be #no_context.
#[test]
fn redline_deref_frame_result_must_not_poison_open_observation() {
    let dir = tmp_dir("frame");
    let engine = Ouroboros::init(&dir).unwrap();
    let mut universe = Universe::new(None, ComboVal::default());
    let src = "s: \"Logic\" |> { a: $ }\nw: { x: $.s }\nv: <<_.>> |> <<_.>>";
    let program = parse_program(src).unwrap();
    for field in &program.fields {
        universe.evolve(&engine, field).unwrap();
    }

    // 1) through the deref frame: "Logic"
    let via_v = universe.observe(&engine, &path_of(&["v", "w", "x", "a"]));
    match &via_v {
        Value::Atom(AtomKind::Str(s), _, _) => assert_eq!(s, "Logic"),
        other => panic!("v.w.x.a should be \"Logic\", got {:?}", other),
    }

    // 2) direct observation of the open term: MUST stay #no_context
    let direct = universe.observe(&engine, &path_of(&["w", "x"]));
    match &direct {
        Value::Bottom(d) => assert_eq!(d.cause, BottomCause::NoContext,
            "direct w.x must be #no_context, got cause {:?}", d.cause),
        other => panic!(
            "RED LINE: direct w.x must be _|_ #no_context, got {:?} — memo served a frame-bound result to an open observation", other),
    }
}

// B. evolve between observations: second observe must see the new value.
#[test]
fn redline_memo_must_not_survive_evolve() {
    let dir = tmp_dir("evolve");
    let engine = Ouroboros::init(&dir).unwrap();
    let mut universe = Universe::new(None, ComboVal::default());
    let p1 = parse_program("s: \"Logic\" |> { a: $ }\nw: { y: s.a }").unwrap();
    for field in &p1.fields {
        universe.evolve(&engine, field).unwrap();
    }

    let before = universe.observe(&engine, &path_of(&["w", "y"]));
    match &before {
        Value::Atom(AtomKind::Str(v), _, _) => assert_eq!(v, "Logic"),
        other => panic!("w.y should be \"Logic\" before evolve, got {:?}", other),
    }

    // evolve s to a new value (refinement direction irrelevant here — the
    // point is the root changes)
    let p2 = parse_program("t: \"Rust\" |> { a: $ }\nu: { y: t.a }").unwrap();
    for field in &p2.fields {
        universe.evolve(&engine, field).unwrap();
    }

    let after = universe.observe(&engine, &path_of(&["u", "y"]));
    match &after {
        Value::Atom(AtomKind::Str(v), _, _) => assert_eq!(
            v, "Rust",
            "RED LINE: post-evolve observation served a stale value"
        ),
        other => panic!("u.y should be \"Rust\" after evolve, got {:?}", other),
    }
}
