use indexmap::IndexMap;
use nlang_interpreter::value::{ComboVal, EffectTag, Value};
use nlang_interpreter::{EvalContext, Ouroboros};
use nlang_parser::ast::AtomKind;

fn make_oo() -> Ouroboros {
    Ouroboros::new_in_memory()
}

fn bytes_val(b: Vec<u8>) -> Value {
    Value::Atom(AtomKind::Bytes(b), EffectTag::Pure, None)
}
fn str_val(s: &str) -> Value {
    Value::Atom(AtomKind::Str(s.to_string()), EffectTag::Pure, None)
}
fn combo1(a: Value) -> Value {
    let mut m = IndexMap::new();
    m.insert("0".to_string(), a);
    Value::Combo(ComboVal::new(
        m,
        false,
        IndexMap::new(),
        EffectTag::Pure,
        vec![],
    ))
}
fn combo2(a: Value, b: Value) -> Value {
    let mut m = IndexMap::new();
    m.insert("0".to_string(), a);
    m.insert("1".to_string(), b);
    Value::Combo(ComboVal::new(
        m,
        false,
        IndexMap::new(),
        EffectTag::Pure,
        vec![],
    ))
}
fn call(oo: &Ouroboros, ctx: &mut EvalContext, name: &str, arg: Value) -> Value {
    oo.builtin_registry.get(name).unwrap().clone()(arg, oo, ctx)
}

#[test]
fn test_sha256_output_is_32_bytes() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(
        &oo,
        &mut ctx,
        "bytes.sha256",
        combo1(bytes_val(b"hello".to_vec())),
    );
    if let Value::Atom(AtomKind::Bytes(b), _, _) = r {
        assert_eq!(b.len(), 32);
    } else {
        panic!("expected Bytes");
    }
}

#[test]
fn test_sha256_deterministic() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r1 = call(
        &oo,
        &mut ctx,
        "bytes.sha256",
        combo1(bytes_val(b"abc".to_vec())),
    );
    let r2 = call(
        &oo,
        &mut ctx,
        "bytes.sha256",
        combo1(bytes_val(b"abc".to_vec())),
    );
    assert_eq!(r1, r2);
}

#[test]
fn test_base64_encode_decode_roundtrip() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let original = b"hello world!".to_vec();
    let encoded = call(
        &oo,
        &mut ctx,
        "bytes.base64_encode",
        combo1(bytes_val(original.clone())),
    );
    let decoded = call(&oo, &mut ctx, "bytes.base64_decode", combo1(encoded));
    if let Value::Atom(AtomKind::Bytes(b), _, _) = decoded {
        assert_eq!(b, original);
    } else {
        panic!("expected Bytes");
    }
}

#[test]
fn test_base64_decode_invalid_returns_none() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(
        &oo,
        &mut ctx,
        "bytes.base64_decode",
        combo1(str_val("not!!valid!!")),
    );
    assert!(matches!(r, Value::Atom(AtomKind::Tag(t), _, _) if t == "none"));
}

#[test]
fn test_hmac_sha256_output_is_32_bytes() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let key = bytes_val(b"secret".to_vec());
    let msg = bytes_val(b"message".to_vec());
    let r = call(&oo, &mut ctx, "bytes.hmac_sha256", combo2(key, msg));
    if let Value::Atom(AtomKind::Bytes(b), _, _) = r {
        assert_eq!(b.len(), 32);
    } else {
        panic!("expected Bytes");
    }
}

#[test]
fn test_hmac_sha256_different_keys_differ() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let msg = bytes_val(b"data".to_vec());
    let r1 = call(
        &oo,
        &mut ctx,
        "bytes.hmac_sha256",
        combo2(bytes_val(b"key1".to_vec()), msg.clone()),
    );
    let r2 = call(
        &oo,
        &mut ctx,
        "bytes.hmac_sha256",
        combo2(bytes_val(b"key2".to_vec()), msg),
    );
    assert_ne!(r1, r2);
}
