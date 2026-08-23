//! Q-035 S2 (O68 Q3.B): an eval context with no standard root, a context
//! whose standard root does not project the name, and a name the root
//! projects but the engine cannot provide, are three different `%cause`s.
//!
//! The acceptor could not calibrate a CLI probe for the empty context
//! (production always installs a root). This crate test is the missing
//! column; see `docs/a_name_is_no_longer_a_credential_handover.md` S2.

use nlang_interpreter::{BottomCause, ComboVal, EvalContext, Ouroboros, Value};
use nlang_parser::parse_program;

fn cause_of(oo: &Ouroboros, ctx: &mut EvalContext, src: &str) -> BottomCause {
    let program = parse_program(&format!("r: {src}")).expect("parse");
    match oo.eval_observed(&program.fields[0].value, ctx) {
        Value::Bottom(d) => d.cause,
        other => panic!("REACH: expected a bottom, got {}", other.to_nlang(0)),
    }
}

#[test]
fn empty_standard_root_is_not_the_same_as_a_missing_or_dead_name() {
    let oo = Ouroboros::new_in_memory();
    let add = r#"{{ %builtin: "math.add", %morphism: #true }} (1,2)"#;
    let invented = r#"{{ %builtin: "nonexistent.thing", %morphism: #true }} (6,3)"#;
    let dead = r#"{{ %builtin: "math.bitAnd", %morphism: #true }} (6,3)"#;

    let mut none = EvalContext::new(ComboVal::default());
    let no_root = cause_of(&oo, &mut none, add);
    assert_eq!(
        no_root,
        BottomCause::NoStandardRoot,
        "an uninstalled context must say so by name, not fold into a missing name"
    );

    let mut installed =
        EvalContext::new(ComboVal::default()).with_standard_root(oo.root_with_system());
    let unprojected = cause_of(&oo, &mut installed, invented);
    assert_eq!(
        unprojected,
        BottomCause::UnprojectedBuiltin,
        "an installed root that does not project the name must say that"
    );

    let mut installed2 =
        EvalContext::new(ComboVal::default()).with_standard_root(oo.root_with_system());
    let unprovided = cause_of(&oo, &mut installed2, dead);
    assert_eq!(
        unprovided,
        BottomCause::UnprovidedBuiltin,
        "a projected name this engine cannot provide must not share the missing-name answer"
    );

    assert_ne!(no_root, unprojected);
    assert_ne!(no_root, unprovided);
    assert_ne!(unprojected, unprovided);
}

#[test]
fn current_standard_root_projects_the_dead_names_and_not_inventions() {
    let oo = Ouroboros::new_in_memory();
    let names = oo.root_with_system().collect_projected_builtins();
    assert!(names.contains("math.add"), "live name missing: {names:?}");
    assert!(
        names.contains("process.exit"),
        "dangerous name missing: {names:?}"
    );
    for dead in [
        "math.bitAnd",
        "math.bitNot",
        "math.bitOr",
        "math.bitXor",
        "math.shl",
        "math.shr",
    ] {
        assert!(names.contains(dead), "dead name {dead} not projected");
    }
    assert!(!names.contains("nonexistent.thing"));
    assert_eq!(
        names.len(),
        251,
        "projected-name count moved: {}",
        names.len()
    );
}
