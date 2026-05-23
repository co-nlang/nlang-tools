use nlang_interpreter::Ouroboros;

/// Verify genesis seed CAIDs are stable.
/// If this test fails, the module definitions in root_with_system() have changed.
/// Run with --nocapture to see the "UPDATE:" lines, copy them into genesis.rs.
#[test]
fn seed_caids_are_stable() {
    let oo = Ouroboros::new_in_memory();
    let root = oo.root_with_system();

    let seeds: Vec<(&str, &str)> = vec![
        ("~%Math",       nlang_interpreter::genesis::SEED_MATH),
        ("~%List",       nlang_interpreter::genesis::SEED_LIST),
        ("~%Cond",       nlang_interpreter::genesis::SEED_COND),
        ("~%String",     nlang_interpreter::genesis::SEED_STRING),
        ("~%Complex",    nlang_interpreter::genesis::SEED_COMPLEX),
        ("~%Reflection", nlang_interpreter::genesis::SEED_REFL),
        ("~%Time",       nlang_interpreter::genesis::SEED_TIME),
        ("~%Discovery",  nlang_interpreter::genesis::SEED_DISCOVERY),
    ];

    // Verify every seed matches its constant
    let mut all_ok = true;
    for (path, expected_seed) in &seeds {
        let val = root.get_field(path).unwrap();
        let computed = val.content_hash_v1().to_string();
        if &computed != expected_seed {
            eprintln!("UPDATE: {} => \"{}\"", path, computed);
            all_ok = false;
        }
    }

    if !all_ok {
        panic!("Seed CAID mismatch. Copy the UPDATE: lines above into genesis.rs");
    }
}
