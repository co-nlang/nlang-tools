use nlang_interpreter::{Ouroboros, EvalContext};
use nlang_interpreter::value::{Value, EffectTag, ComboVal};
use nlang_parser::ast::AtomKind;
use indexmap::IndexMap;
use num_bigint::BigInt;

fn make_oo() -> Ouroboros { Ouroboros::new_in_memory() }

fn int_val(n: i64) -> Value {
    Value::Atom(AtomKind::Int(BigInt::from(n)), EffectTag::Pure, None)
}

fn make_list(items: Vec<Value>) -> Value {
    let len = items.len();
    let mut fields = IndexMap::new();
    for (i, v) in items.into_iter().enumerate() {
        fields.insert(i.to_string(), v);
    }
    fields.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None));
    fields.insert("%len".to_string(), int_val(len as i64));
    Value::Combo(ComboVal::new(fields, false, IndexMap::new(), EffectTag::Pure, vec![]))
}

fn call(oo: &Ouroboros, ctx: &mut EvalContext, name: &str, arg: Value) -> Value {
    let f = oo.builtin_registry.get(name).expect("builtin not found").clone();
    f(arg, oo, ctx)
}

fn make_combo_2(a: Value, b: Value) -> Value {
    let mut f = IndexMap::new();
    f.insert("0".to_string(), a);
    f.insert("1".to_string(), b);
    Value::Combo(ComboVal::new(f, false, IndexMap::new(), EffectTag::Pure, vec![]))
}

fn make_pred_builtin(name: &str) -> Value {
    let mut f = IndexMap::new();
    f.insert("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None));
    f.insert("%builtin".to_string(), Value::Atom(AtomKind::Str(name.to_string()), EffectTag::Pure, None));
    Value::Combo(ComboVal::new(f, true, IndexMap::new(), EffectTag::Pure, vec![]))
}

fn is_tag(v: &Value, t: &str) -> bool {
    if let Value::Atom(AtomKind::Tag(s), _, _) = v.collapse() { s.trim_start_matches('#') == t } else { false }
}

fn closed_combo() -> Value {
    Value::Combo(ComboVal::new(IndexMap::new(), true, IndexMap::new(), EffectTag::Pure, vec![]))
}

fn open_combo() -> Value {
    Value::Combo(ComboVal::new(IndexMap::new(), false, IndexMap::new(), EffectTag::Pure, vec![]))
}

#[test]
fn test_list_any_true() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let list = make_list(vec![open_combo(), closed_combo()]);
    let pred = make_pred_builtin("refl.is_cocoon");
    let arg = make_combo_2(pred, list);
    let result = call(&oo, &mut ctx, "list.any", arg);
    assert!(is_tag(&result, "true"), "list.any with closed combo should be #true: {:?}", result);
}

#[test]
fn test_list_any_false() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let list = make_list(vec![open_combo(), open_combo(), int_val(1)]);
    let pred = make_pred_builtin("refl.is_cocoon");
    let arg = make_combo_2(pred, list);
    let result = call(&oo, &mut ctx, "list.any", arg);
    assert!(is_tag(&result, "false"), "list.any with no closed combo should be #false: {:?}", result);
}

#[test]
fn test_list_any_empty() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let list = make_list(vec![]);
    let pred = make_pred_builtin("refl.is_cocoon");
    let arg = make_combo_2(pred, list);
    let result = call(&oo, &mut ctx, "list.any", arg);
    assert!(is_tag(&result, "false"), "list.any empty should be #false: {:?}", result);
}

#[test]
fn test_list_all_true() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let list = make_list(vec![closed_combo(), closed_combo()]);
    let pred = make_pred_builtin("refl.is_cocoon");
    let arg = make_combo_2(pred, list);
    let result = call(&oo, &mut ctx, "list.all", arg);
    assert!(is_tag(&result, "true"), "list.all all closed should be #true: {:?}", result);
}

#[test]
fn test_list_all_false() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let list = make_list(vec![closed_combo(), open_combo()]);
    let pred = make_pred_builtin("refl.is_cocoon");
    let arg = make_combo_2(pred, list);
    let result = call(&oo, &mut ctx, "list.all", arg);
    assert!(is_tag(&result, "false"), "list.all with open combo should be #false: {:?}", result);
}

#[test]
fn test_list_find_found() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let list = make_list(vec![open_combo(), closed_combo(), open_combo()]);
    let pred = make_pred_builtin("refl.is_cocoon");
    let arg = make_combo_2(pred, list);
    let result = call(&oo, &mut ctx, "list.find", arg);
    if let Value::Combo(ref cv) = result {
        assert!(cv.get_field("%val").is_some(), "list.find closed should return Some: {:?}", result);
    } else {
        panic!("list.find should return Some, got {:?}", result);
    }
}

#[test]
fn test_list_find_not_found() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let list = make_list(vec![open_combo(), open_combo()]);
    let pred = make_pred_builtin("refl.is_cocoon");
    let arg = make_combo_2(pred, list);
    let result = call(&oo, &mut ctx, "list.find", arg);
    assert!(is_tag(&result, "none"), "list.find with no cocoon should be #none: {:?}", result);
}
