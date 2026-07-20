// Order-wave W3 probes (2026-07-20, pre-committed by work order —
// docs/order_w3_handover.md).
//
// SCOPE (wave plan approved 2026-07-20; zero NEW adjudication): order
// on NON-ATOMS lands. Law is already written: SYNTAX_06 §2.1
// `A <= B ⟺ (A & B) = A` (subset via meet reduction), `<`/`>` proper
// (A <= B ∧ ¬(B <= A)), §3 combo examples verbatim; unions = branch-set
// inclusion (same reduction — union meet + set equality already
// healthy). Order × #blur follows the `=` two-stage law (§4 #13, ruled
// with the wave plan): same CAID → reflexive verdicts (<=/>= #true,
// </> #false); different identity ABSORBS into the horizon (never
// #false, never #conflict).
//
// MEASURED (v0.2.30): raw materials healthy — `((1|2) & (1|2|3)) =
// (1|2)` → #true, `({a:1} & {a:@int}) = {a:1}` → #true, open-world
// meet adds fields → #false. Only the wiring is missing: every
// non-atom order face is ⊥ #conflict (union/combo/mixed/blur).
// W3 = pure wiring into the existing meet + G1 solidified equality.
//
// Open migrations (acceptor): order_wave W2 fence pin, combo_equality
// pin_combo_lte_stays_conflict, blur_boundary lt/lte frozen pins.
//
// NOT in scope: %super/%predicate (W4), atom faces (W2 landed),
// `=`/`==` families, binary-builtin × union distribution (ledgered).

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
        "nlang-ordw3-{}-{}",
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
// RED GATES — combo order (SYNTAX_06 §2.1 / §3)
// ─────────────────────────────────────────────────────────────────────────

#[test]
#[ignore]
fn red_combo_subtype() {
    // L2-85 twin + §3 examples verbatim.
    assert_obs("out: {a: 1} <= {a: @int}", "#true");
    assert_obs("out: {a: @int} <= {a: 1}", "#false");
    assert_obs(
        "out: {name: \"Alice\"} <= {name: @str, age: @int}",
        "#false",
    );
    // More fields = more info = smaller set (open world).
    assert_obs("out: {a: 1, b: 2} <= {a: 1}", "#true");
}

#[test]
#[ignore]
fn red_combo_proper_and_reflexive() {
    assert_obs("out: {a: 1} < {a: @int}", "#true");
    assert_obs("out: {a: 1} < {a: 1}", "#false");
    assert_obs("out: {a: 1} <= {a: 1}", "#true");
    assert_obs("out: {a: @int} > {a: 1}", "#true");
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — union order (branch-set inclusion via the same reduction)
// ─────────────────────────────────────────────────────────────────────────

#[test]
#[ignore]
fn red_union_inclusion() {
    // L2-86 twin.
    assert_obs("out: (1 | 2) <= (1 | 2 | 3)", "#true");
    assert_obs("out: (1 | 2 | 3) <= (1 | 2)", "#false");
    assert_obs("out: (1 | 2) < (1 | 2 | 3)", "#true");
    // Equal sets, either spelling: <= yes, proper no.
    assert_obs("out: (1 | 2) <= (2 | 1)", "#true");
    assert_obs("out: (1 | 2) < (2 | 1)", "#false");
}

#[test]
#[ignore]
fn red_mixed_atom_union_type() {
    assert_obs("out: 1 <= (1 | 2)", "#true");
    assert_obs("out: (1 | 2) <= 1", "#false");
    assert_obs("out: 1 < (1 | 2)", "#true");
    // Every branch inhabits the type space.
    assert_obs("out: (1 | 2) <= @int", "#true");
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — order × #blur: the `=` two-stage law (SYNTAX_06 §4 #13)
// ─────────────────────────────────────────────────────────────────────────

#[test]
#[ignore]
fn red_blur_order_reflexive_same_caid() {
    // Same binding = same horizon identity: the set equals itself.
    let src = format!("big: {}\n", flat_chain(4000));
    assert_obs(&format!("{src}out: big <= big"), "#true");
    assert_obs(&format!("{src}out: big < big"), "#false");
}

#[test]
#[ignore]
fn red_blur_order_absorbs() {
    // Different identity: undetermined within the horizon — absorb,
    // never #false, never #conflict.
    let src = format!("big: {}\n", flat_chain(4000));
    let got = observe_nlang(&format!("{src}out: big <= 1"), "out");
    assert!(got.starts_with("#blur"), "blur lte absorbs: {got:?}");
    let got = observe_nlang(&format!("{src}out: 1 <= big"), "out");
    assert!(got.starts_with("#blur"), "lte blur absorbs: {got:?}");
    let got = observe_nlang(&format!("{src}out: big < {{a: 1}}"), "out");
    assert!(got.starts_with("#blur"), "blur lt combo absorbs: {got:?}");
}

// ─────────────────────────────────────────────────────────────────────────
// ACTIVE PINS — boundaries that must not move
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn pin_extremes_nonatom_unchanged() {
    // Extreme laws already answer before the non-atom arms.
    assert_obs("out: (1 & 2) <= {a: 1}", "#true");
    assert_obs("out: {a: 1} <= _", "#true");
    assert_obs("out: _ <= {a: 1}", "#false");
}

#[test]
fn pin_eq_families_untouched() {
    assert_obs("out: (1 | 2) = (2 | 1)", "#true");
    assert_obs("out: {a: 1} = {a: 1}", "#true");
    let got = observe_nlang("out: ({a: 1} == {a: 1}).%cause", "out");
    assert_eq!(got, "#conflict", "== family misuse stays loud: {got:?}");
}

#[test]
fn pin_atom_faces_w2_intact() {
    assert_obs("out: 3 <= 5", "#false");
    assert_obs("out: 1 <= @int", "#true");
    assert_obs("out: 2 <= 2", "#true");
}

#[test]
fn pin_meet_and_reduction_materials() {
    // The raw materials W3 wires into — must stay healthy.
    assert_obs("out: ((1 | 2) & (1 | 2 | 3)) = (1 | 2)", "#true");
    assert_obs("out: ({a: 1} & {a: @int}) = {a: 1}", "#true");
}
