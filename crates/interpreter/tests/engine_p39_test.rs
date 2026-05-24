use nlang_interpreter::{Ouroboros, EvalContext};
use nlang_interpreter::value::{Value, EffectTag};
use nlang_parser::ast::AtomKind;
use indexmap::IndexMap;
use nlang_interpreter::value::ComboVal;
use num_bigint::BigInt;

fn make_oo() -> Ouroboros { Ouroboros::new_in_memory() }

fn str_val(s: &str) -> Value { Value::Atom(AtomKind::Str(s.to_string()), EffectTag::Pure, None) }

fn combo1(a: Value) -> Value {
    let mut m = IndexMap::new(); m.insert("0".to_string(), a);
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]))
}

fn call(oo: &Ouroboros, ctx: &mut EvalContext, name: &str, arg: Value) -> Value {
    oo.builtin_registry.get(name).unwrap().clone()(arg, oo, ctx)
}

fn caid_of(n: i64) -> String {
    Value::Atom(AtomKind::Int(BigInt::from(n)), EffectTag::Pure, None)
        .content_hash()
        .to_string()
}

fn is_list(v: &Value) -> bool {
    matches!(v, Value::Combo(c) if matches!(c.get_field("%kind"), Some(Value::Atom(AtomKind::Tag(t), _, _)) if t == "list"))
}

#[test]
fn test_equivalence_map_empty_returns_kind_tag() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "engine.equivalence_map", Value::Top);
    assert!(matches!(r, Value::Combo(_)), "should return a Combo");
    if let Value::Combo(ref c) = r {
        assert!(
            matches!(c.get_field("%kind"), Some(Value::Atom(AtomKind::Tag(t), _, _)) if t == "equivalence_map"),
            "%kind should be #equivalence_map"
        );
        assert!(
            matches!(c.get_field("%count"), Some(Value::Atom(AtomKind::Int(n), _, _)) if n == &BigInt::from(0i64)),
            "%count should be 0 when refine_map is empty"
        );
        let entries = c.get_field("entries").expect("should have entries field");
        assert!(is_list(entries), "entries should be a list");
    }
}

#[test]
fn test_equivalence_map_effect_is_state() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "engine.equivalence_map", Value::Top);
    assert_eq!(r.effect(), EffectTag::State);
}

#[test]
fn test_resolve_unknown_caid_returns_itself() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let caid_str = caid_of(9999);
    let r = call(&oo, &mut ctx, "engine.resolve", combo1(str_val(&caid_str)));
    match &r {
        Value::Atom(AtomKind::Str(s), EffectTag::State, _) => {
            assert_eq!(s, &caid_str, "unrefined CAID should resolve to itself");
        }
        other => panic!("expected Str(State), got {:?}", other),
    }
}

#[test]
fn test_resolve_follows_one_hop() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let caid_a = caid_of(1001);
    let caid_b = caid_of(2001);

    {
        let mut map = oo.refine_map.write().unwrap();
        map.insert(caid_a.clone(), vec![caid_b.clone()]);
    }

    let r = call(&oo, &mut ctx, "engine.resolve", combo1(str_val(&caid_a)));
    match &r {
        Value::Atom(AtomKind::Str(s), EffectTag::State, _) => {
            assert_eq!(s, &caid_b, "resolve should follow A → B");
        }
        other => panic!("expected Str(State), got {:?}", other),
    }
}

#[test]
fn test_equivalence_map_shows_refined_entry() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let caid_a = caid_of(3001);
    let caid_b = caid_of(4001);

    {
        let mut map = oo.refine_map.write().unwrap();
        map.insert(caid_a.clone(), vec![caid_b.clone()]);
    }

    let r = call(&oo, &mut ctx, "engine.equivalence_map", Value::Top);
    if let Value::Combo(ref c) = r {
        assert!(
            matches!(c.get_field("%count"), Some(Value::Atom(AtomKind::Int(n), _, _)) if n == &BigInt::from(1i64)),
            "%count should be 1"
        );
        let entries = c.get_field("entries").expect("entries field");
        if let Value::Combo(ref lc) = entries {
            let entry = lc.get_field("0").expect("entries[0]");
            if let Value::Combo(ref ec) = entry {
                let from = ec.get_field("from").expect("entry.from");
                let to   = ec.get_field("to").expect("entry.to");
                assert!(matches!(from, Value::Atom(AtomKind::Str(s), _, _) if s == &caid_a), "from should be caid_a");
                assert!(matches!(to,   Value::Atom(AtomKind::Str(s), _, _) if s == &caid_b), "to should be caid_b");
            } else { panic!("entries[0] should be a Combo"); }
        } else { panic!("entries should be a Combo"); }
    } else { panic!("result should be a Combo"); }
}
