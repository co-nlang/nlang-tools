// Type reflection: %super derived hierarchy + %type→%name retirement
// (2026-07-22, pre-committed by work order — docs/type_super_reflection_handover.md).
//
// RULING R1 (2026-07-22, user adjudicated): `@T`'s content fields
// %super/%predicate (SPEC_03 §4 isomorphism, SPEC_05 §3.2) are NOT a
// nominal type layer — n/ is a pure structural lattice, so type-checking
// already happens by `&` (SPEC_05 §5 generics). What a lattice value DOES
// need is the reverse of monotone convergence: convergence goes DOWN
// (`&` refines), but "how is this datum handled" is looked up UP the
// hierarchy. `%super` is that upward link — a DERIVED reflection view
// (like %kind, SPEC_05 §4), holding the SPEC_09 §2.1 hierarchy parent.
//   - %super: derived; the §2.1 TREE immediate parent (user pointed at
//     §2.1 — the tree, not the §2.3 non-immediate table). @any (⊤) has
//     no super → honest open-miss `_`.
//   - %predicate: RETIRED. The constraint IS the structural combo, checked
//     by `&`; a separate %predicate (P_instance ⊑ P_type subsumption) only
//     means anything in a NOMINAL layer = R2, ledgered (candidate future
//     mechanism for cross-engine custom-type exchange), not the core.
//   - %type "Name" payload: RETIRED as the last %type fossil (cocoon arc
//     2026-07-19 retired the rest). The type NAME reflection spelling
//     converges to `%name` (unifying with stdlib type nodes). The internal
//     mint (dispatch.rs) renames %type→%name; `.%type` becomes an ordinary
//     open-miss.
//
// MEASURED (v0.2.32): `(@int).%super` → `_`, `(@u8).%super` → `_`,
// `(@int).%name` → `_` (all unimplemented); `(@int).%type` → `"int"`
// (the observable payload leak this arc retires); `@int` displays
// `{{ %kind: #type, %type: "int" }}` (leaks the payload). Healthy:
// `(@int).%kind` → `#type`; structural membership `1 & @int` and
// SPEC_05 §5 generic meet unchanged.
//
// NOT in scope: %predicate implementation (retired to R2); nominal
// name-based subtyping (R2); the §2.1-tree vs §2.3-table float discrepancy
// (flagged for spec closure — float's immediate super = @complex per the
// tree; §2.3 lists a non-immediate ancestor); atom `%kind` inference.

use nlang_interpreter::{Ouroboros, Universe};
use nlang_parser::ast::{Path, PathAnchor, Span};
use nlang_parser::parse_program;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

fn tmp_dir() -> nlang_interpreter::ScratchDir {
    nlang_interpreter::ScratchDir::new("typesuper")
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
            let mut universe = Universe::new_with_standard(
                None,
                engine.root_with_system(),
                engine.root_with_system(),
            );
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

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — %super derived hierarchy link (SPEC_09 §2.1 tree)
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_super_numeric_chain() {
    // Unambiguous §2.1 immediate parents (int/complex directly under num).
    assert_obs("out: (@int).%super = @num", "#true");
    assert_obs("out: (@complex).%super = @num", "#true");
    assert_obs("out: (@num).%super = @any", "#true");
}

#[test]
fn red_super_fixed_width_to_int() {
    // Fixed-width投影 sit under @int (§2.1 tree; FFI boundary row §2.3).
    assert_obs("out: (@u8).%super = @int", "#true");
    assert_obs("out: (@i32).%super = @int", "#true");
}

#[test]
fn red_super_record_to_combo() {
    assert_obs("out: (@record).%super = @combo", "#true");
}

#[test]
fn red_super_chain_navigable() {
    // The back-link is a real type value → the chain composes to the top.
    assert_obs("out: ((@int).%super).%super = @any", "#true");
    // The super is itself a type (kind preserved through the link).
    assert_obs("out: ((@int).%super).%kind = #type", "#true");
}

#[test]
fn red_super_user_type_is_combo() {
    // A user-defined field-structure type is a @combo (§2.1: @combo =
    // 任意欄位結構). Its handler family is the structural combo.
    assert_obs(
        "@Box: { value: @int }\nout: (@Box).%super = @combo",
        "#true",
    );
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — name reflection converges to %name; %type payload retired
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_name_reflection_via_name() {
    // The type NAME reads through `%name` (was the internal %type payload).
    assert_obs("out: (@int).%name", "\"int\"");
    assert_obs("out: (@str).%name", "\"str\"");
}

#[test]
fn red_type_payload_retired() {
    // The last %type fossil: `.%type` on a type marker is now an ordinary
    // open-miss (the payload field was renamed to %name).
    assert_obs("out: (@int).%type", "_");
}

#[test]
fn red_display_no_type_leak() {
    // The marker's display must not leak the fossil %type; it shows %name.
    let got = observe_nlang("out: @int", "out");
    assert!(
        !got.contains("%type") && got.contains("%name") && got.contains("int"),
        "type marker display shows %name, never fossil %type: {got:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// ACTIVE PINS — boundaries that must not move
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn pin_any_top_has_no_super() {
    // ⊤ contains everything → no super. Honest open-miss, not a fabricated
    // self-loop.
    assert_obs("out: (@any).%super", "_");
}

#[test]
fn pin_atom_super_out_of_scope() {
    // %super lives on TYPE values only; a plain datum open-misses.
    assert_obs("out: (42).%super", "_");
}

#[test]
fn pin_predicate_stays_retired() {
    // %predicate is an R2 (nominal) concept — never minted in the core.
    // It must remain an ordinary open-miss, not grow a value.
    assert_obs("out: (@int).%predicate", "_");
}

#[test]
fn pin_kind_unchanged() {
    assert_obs("out: (@int).%kind", "#type");
}

#[test]
fn pin_structural_membership_unchanged() {
    // The lattice type-check (SPEC_05 §5) is untouched by adding reflection.
    assert_obs("out: 1 & @int", "1");
    let got = observe_nlang("out: (\"hi\" & @int).%cause", "out");
    assert_eq!(
        got, "#conflict",
        "wrong-type meet stays ⊥ #conflict: {got:?}"
    );
}

#[test]
fn pin_generic_specialization_unchanged() {
    // SPEC_05 §5 generic meet: valid instance converges, invalid → ⊥.
    assert_obs(
        "@Box: { ~@T: @any, value: ~@T }\n@IntBox: @Box & { ~@T: @int }\nok: { value: 42 } & @IntBox\nout: ok.value",
        "42",
    );
}
