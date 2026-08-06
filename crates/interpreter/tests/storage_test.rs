use indexmap::IndexMap;
use nlang_interpreter::{ComboVal, EffectTag, ObjectStore, Value};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;
use std::env;
use std::fs;

#[test]
fn test_object_store_persistence() {
    // 1. 準備測試目錄
    let temp_dir = nlang_interpreter::ScratchDir::new("test-storage");

    // 2. 初始化 Store
    let store = ObjectStore::init(&temp_dir).expect("Failed to init store");

    // 3. 建立一個 Combo 並存入
    let val = Value::Combo(ComboVal::new(
        IndexMap::from_iter(vec![
            (
                "name".to_string(),
                Value::Atom(
                    AtomKind::Str("Ouroboros".to_string()),
                    EffectTag::Pure,
                    None,
                ),
            ),
            (
                "version".to_string(),
                Value::Atom(AtomKind::Int(BigInt::from(2)), EffectTag::Pure, None),
            ),
        ]),
        true,
        IndexMap::new(),
        EffectTag::Pure,
        vec![],
    ));

    let hash = store.put_value(&val).expect("Failed to put value");
    println!("Stored with hash: {}", hash.to_string());

    // 4. 從 Store 讀取
    let loaded_val = store.get_value(&hash).expect("Failed to get value");

    // 5. 驗證
    assert_eq!(val, loaded_val);
    println!("Loaded successfully and content matches!");
}
