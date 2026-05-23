use nlang_interpreter::{Ouroboros, Value, ComboVal, EvalContext, EffectTag};
use nlang_parser::ast::{Expr, ExprKind, AtomKind, Path, PathAnchor, Span};
use indexmap::IndexMap;
use num_bigint::BigInt;

fn empty_ouroboros() -> Ouroboros {
    Ouroboros::new_in_memory()
}

fn path_expr(name: &str) -> Expr {
    Expr::new(
        ExprKind::Path(Path {
            anchor: PathAnchor::Bare,
            segments: vec![name.to_string()],
            span: Span::new(0, 0),
        }),
        Span::new(0, 0),
    )
}

fn int_expr(v: i64) -> Expr {
    Expr::new(ExprKind::Atom(AtomKind::Int(BigInt::from(v))), Span::new(0, 0))
}

#[test]
#[ignore = "Known Issue: Stack Overflow on deep thunks"]
fn test_chained_thunk_performance() {
let oo = empty_ouroboros();
    let mut root = ComboVal::default();

    // 建立深度為 50 的鏈條：v0 -> v1 + 1, v1 -> v2 + 1, ..., v49 -> 100
    let n = 50;
    for i in 0..n {
        let key = format!("v{}", i);
        let expr = if i == n - 1 {
            int_expr(100)
        } else {
            // 簡化：這裡我們直接模擬 ExprKind::Add(v_{i+1}, 1)
            Expr::new(
                ExprKind::Add(
                    Box::new(path_expr(&format!("v{}", i + 1))),
                    Box::new(int_expr(1))
                ),
                Span::new(0, 0)
            )
        };

        root.insert_field(&key, Value::Thunk {
            expr: Box::new(expr),
            closure: vec![],
            effect: EffectTag::Pure,
        });
    }

    let mut ctx = EvalContext::new(root).with_fuel(10000);
    
    // 1. 首次觀測 v0 (觸發連鎖坍縮)
    let start = std::time::Instant::now();
    let v0_path = Path {
        anchor: PathAnchor::Bare,
        segments: vec!["v0".to_string()],
        span: Span::new(0, 0),
    };
    let res = oo.resolve_path(&v0_path, &mut ctx);
    let duration = start.elapsed();
    
    assert_eq!(res, Value::Atom(AtomKind::Int(BigInt::from(100 + (n as i64) - 1)), EffectTag::Pure, None));
    println!("First observation took: {:?}", duration);

    // 2. 第二次觀測 v0 (預期瞬間完成，利用記憶化)
    let start2 = std::time::Instant::now();
    let res2 = oo.resolve_path(&v0_path, &mut ctx);
    let duration2 = start2.elapsed();
    
    assert_eq!(res2, res);
    println!("Second observation took: {:?}", duration2);
    assert!(duration2 < duration);
}

#[test]
fn test_sparse_combo_definition_speed() {
    let _oo = empty_ouroboros();
    let mut root_fields = IndexMap::new();
    
    // 建立一個包含 1000 個複雜計算 Thunk 的 Combo
    let n = 1000;
    let start = std::time::Instant::now();
    for i in 0..n {
        root_fields.insert(format!("f{}", i), Value::Thunk {
            expr: Box::new(int_expr(i as i64)), // 雖然只是 int，但包裹在 Thunk 中
            closure: vec![],
            effect: EffectTag::Pure,
        });
    }
    let duration = start.elapsed();
    
    println!("Defining {} thunks took: {:?}", n, duration);
    // 定義過程應該極快，因為不執行 eval
    assert!(duration.as_millis() < 50); 
}
