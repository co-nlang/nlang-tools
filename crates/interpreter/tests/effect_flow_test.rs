use indexmap::IndexMap;
use nlang_interpreter::{ComboVal, EffectTag, EvalContext, Ouroboros, Value};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;

fn empty_ouroboros() -> Ouroboros {
    Ouroboros::new_in_memory()
}

fn io_combo() -> Value {
    let mut cv = ComboVal::default();
    cv.effect = EffectTag::IO;
    Value::Combo(cv)
}

#[test]
fn test_effect_propagation_unify() {
    let oo = empty_ouroboros();
    let pure_val = Value::Atom(AtomKind::Int(BigInt::from(1)), EffectTag::Pure, None);
    let tainted_val = io_combo();

    // Pure & IO -> IO
    let res = oo.unify(pure_val, tainted_val);
    assert_eq!(res.effect(), EffectTag::IO);
}

#[test]
fn test_cocoon_isolation_logic() {
    let _oo = empty_ouroboros();

    // side_effect_cocoon = {{ IO }}
    let cocoon = Value::Combo(ComboVal::new(
        IndexMap::from_iter(vec![("internal".to_string(), io_combo())]),
        true,
        IndexMap::new(),
        EffectTag::Pure,
        vec![],
    ));

    // 隔離檢測：容器本身應為 Pure (因為 c.closed == true)
    assert_eq!(cocoon.effect(), EffectTag::Pure);
}

#[test]
fn test_morphism_tainting() {
    let oo = empty_ouroboros();
    let mut ctx = EvalContext::new(oo.root_with_system());

    // 觀測 ~%Time./now (這是 IO 態射)
    let f = oo.root_with_system().get_field("~%Time").unwrap().clone();
    // 獲取 /now 態射
    let now_m = match f {
        Value::Combo(c) => c.get_field("/now").unwrap().clone(),
        _ => panic!("Expected Combo"),
    };

    assert_eq!(now_m.effect(), EffectTag::IO, "Morphism should be IO");

    // 應用 IO 態射 -> 結果應帶 IO 顏色
    let res = oo.apply_morphism(
        now_m,
        Value::Atom(AtomKind::Unit, EffectTag::Pure, None),
        &mut ctx,
    );
    assert_eq!(res.effect(), EffectTag::IO, "Result should inherit IO");
}
