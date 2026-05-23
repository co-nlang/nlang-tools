use nlang_interpreter::{Ouroboros, Universe};
use nlang_parser::{parse_program, parse_expr_only, ast::ExprKind};
use std::fs;
use std::path::PathBuf;

fn parse_path_only(s: &str) -> anyhow::Result<nlang_parser::ast::Path> {
    let expr = parse_expr_only(s).map_err(|e| anyhow::anyhow!("{}", e))?;
    if let ExprKind::Path(p) = expr.kind { Ok(p) } else { Err(anyhow::anyhow!("Not a path")) }
}

fn normalize(s: &str) -> String {
    let s = s.trim();
    if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
        s[1..s.len()-1].replace("\\\"", "\"").replace("\\n", "\n")
    } else {
        s.to_string()
    }
}

#[test]
fn run_all_integration_tests() {
    let mut d = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    d.push("../../tests");
    
    let mut tests = Vec::new();
    collect_tests(&d, &mut tests);
    tests.sort();
    
    let mut passed = 0;
    let mut failed = 0;
    let mut skipped = 0;
    
    for test_file in tests {
        let content = fs::read_to_string(&test_file).unwrap();
        
        let observe = extract_meta(&content, "@observe:");
        let expect = extract_meta(&content, "@expect:");
        
        if observe.is_none() || expect.is_none() {
            skipped += 1;
            continue;
        }
        
        let observe = observe.unwrap();
        let expect = expect.unwrap();
        
        let program = match parse_program(&content) {
            Ok(p) => p,
            Err(e) => {
                println!("FAIL: {:?} (Parse error: {})", test_file, e);
                failed += 1;
                continue;
            }
        };
        
        let engine = Ouroboros::new_in_memory();
        let mut universe = Universe::new(None, engine.root_with_system());
        
        let mut evolve_failed = false;
        for f in &program.fields {
            if let Err(e) = universe.evolve(&engine, &f) {
                println!("FAIL: {:?} (Evolution error: {:?})", test_file, e);
                failed += 1;
                evolve_failed = true;
                break;
            }
        }
        if evolve_failed { continue; }
        
        let path = match parse_path_only(&observe) {
            Ok(p) => p,
            Err(e) => {
                println!("FAIL: {:?} (Observe path parse error: {})", test_file, e);
                failed += 1;
                continue;
            }
        };
        
        let result = universe.observe(&engine, &path);
        let actual_raw = result.to_nlang(0);
        
        let actual_norm = normalize(&actual_raw);
        let expected_norm = normalize(&expect);
        
        if actual_norm == expected_norm || actual_raw.trim() == expected_norm {
            passed += 1;
            println!("PASS: {:?}", test_file);
        } else {
            println!("FAIL: {:?}", test_file);
            println!("  Expected: {}", expected_norm);
            println!("  Actual:   {}", actual_norm);
            failed += 1;
        }
    }
    
    println!("Integration Tests: {} passed, {} failed, {} skipped", passed, failed, skipped);
    assert_eq!(failed, 0, "Some integration tests failed");
}

fn collect_tests(dir: &std::path::Path, tests: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries {
            if let Ok(entry) = entry {
                let path = entry.path();
                if path.is_dir() {
                    if path.file_name().and_then(|s| s.to_str()) == Some("pending") {
                        continue;
                    }
                    collect_tests(&path, tests);
                } else if path.extension().and_then(|s| s.to_str()) == Some("n") {
                    tests.push(path);
                }
            }
        }
    }
}

fn extract_meta(content: &str, tag: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with(";;") {
            let s = trimmed[2..].trim_start();
            if s.starts_with(tag) {
                return Some(s[tag.len()..].trim().to_string());
            }
        }
    }
    None
}