// E4 nominal `@Name` reference probes (2026-07-11 — docs/nominal_ref_handover.md;
// gap recorded in nlang-spec ENGINE_SYNC「Range 語義補完缺口」E4).
//
// Resolution: builtin type names stay markers (reserved set, not shadowable);
// every other `@Name` resolves through the normal lookup chain (lazy force,
// record_dep); not-found keeps the Unknown pass-through fallback.
// Trinity: a dereferenced type def is just a value — sealed `{{}}` templates
// keep SPEC_03 exhaustive-schema semantics (extra field → ⊥), open `{}`
// templates constrain listed fields only.
//
// Root cause (fixed): `is_type_constraint_path` used to treat ANY `@`-prefixed
// name as an opaque marker BEFORE scope/root lookup.

use nlang_interpreter::{Ouroboros, Universe, Value};
use nlang_parser::ast::{AtomKind, Path, PathAnchor, Span};
use nlang_parser::parse_program;
use num_bigint::BigInt;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

fn tmp_dir() -> PathBuf {
    let mut d = std::env::temp_dir();
    d.push(format!(
        "nlang-e4-{}-{}",
        std::process::id(),
        DIR_SEQ.fetch_add(1, Ordering::SeqCst)
    ));
    fs::remove_dir_all(&d).ok();
    fs::create_dir_all(&d).unwrap();
    d
}

/// Evolve every program field, then observe `path` ("a" or "a.b").
/// Evolution conflict (⊥ at evolve time) is returned as Err — for bottom
/// expectations both channels count.
fn run_observe(src: &str, path: &str) -> Result<Value, String> {
    let dir = tmp_dir();
    let engine = Ouroboros::init(&dir).unwrap();
    let mut universe = Universe::new(None, engine.root_with_system());
    let program = parse_program(src).unwrap();
    for f in &program.fields {
        universe
            .evolve(&engine, f)
            .map_err(|e| format!("evolve: {e:?}"))?;
    }
    let p = Path {
        anchor: PathAnchor::Bare,
        segments: path.split('.').map(|s| s.to_string()).collect(),
        span: Span::default(),
    };
    Ok(universe.observe(&engine, &p))
}

fn assert_obs_int(src: &str, path: &str, expect: i64) {
    match run_observe(src, path) {
        Ok(Value::Atom(AtomKind::Int(n), _, _)) => {
            assert_eq!(n, BigInt::from(expect), "{src:?} :: {path}")
        }
        other => panic!("{src:?} :: {path} must be {expect}, got {other:?}"),
    }
}

fn assert_obs_bottom(src: &str, path: &str) {
    match run_observe(src, path) {
        Err(_) => {}               // evolution conflict — counts as ⊥
        Ok(Value::Bottom(_)) => {} // observed ⊥
        Ok(other) => panic!("{src:?} :: {path} must be _|_, got {other:?}"),
    }
}

// ─────────────────────────────────────────────────────────────────────────
// RED LINES — violating values silently accepted today / deref shape
// ─────────────────────────────────────────────────────────────────────────

#[test]
// RED LINE (E4): defined type must ENFORCE on merge — the silent
// acceptance is the bug (README front-page example class)
fn e4_violating_merge_is_bottom() {
    assert_obs_bottom(
        "@Adult: { age: 18.. }\nminor: { name: \"Bob\", age: 15 } & @Adult",
        "minor",
    );
}

#[test]
// RED LINE (E4): mirror side — same lookup path, not a duplicate
fn e4_violating_merge_mirror() {
    assert_obs_bottom(
        "@Adult: { age: 18.. }\nminor: @Adult & { name: \"Bob\", age: 15 }",
        "minor",
    );
}

#[test]
// RED LINE (E4): observing the reference yields the DEFINITION,
// not the opaque marker
fn e4_deref_shape_is_definition() {
    match run_observe("@Adult: { age: 18.. }\nprobe: @Adult", "probe") {
        Ok(Value::Combo(cv)) => {
            assert!(
                cv.get_field("%kind").is_none(),
                "must not be a type_constraint marker"
            );
            assert!(cv.get_field("age").is_some(), "definition fields visible");
        }
        other => panic!("probe must be the defined combo, got {other:?}"),
    }
}

#[test]
// RED LINE (E4): SPEC_03 exhaustive sealed schema — extra field ⊥
// (exact spec example; today the marker silently passes it)
fn e4_sealed_exhaustive_extra_field_bottom() {
    assert_obs_bottom(
        "@StrictUser: {{ name: @str }}\nr: @StrictUser & { name: \"Alice\", age: 30 }",
        "r",
    );
}

#[test]
// RED LINE (E4): constraints run THROUGH the template fieldwise —
// @float projection visible (1 → 1.0), proves real fieldwise unify
fn e4_projection_through_template() {
    match run_observe("@P: { x: @float }\nr: { x: 1 } & @P", "r.x") {
        Ok(Value::Atom(AtomKind::Float(f), _, _)) => {
            assert!((f - 1.0).abs() < 1e-12)
        }
        other => panic!("r.x must be 1.0 (projection through template), got {other:?}"),
    }
}

#[test]
// RED LINE (E4): recursive type def — deref must be lazy enough to
// terminate AND enforce (v: "s" violates @int)
fn e4_recursive_type_enforces_and_terminates() {
    assert_obs_bottom(
        "@Tree: { v: @int, next: @Tree | () }\nt: { v: \"s\", next: () } & @Tree",
        "t",
    );
}

#[test]
// RED LINE (E4): Union of type refs — distribution must survive the
// new resolution path (arm-order class, 5th time's the charm)
fn e4_union_of_typerefs_enforces() {
    assert_obs_bottom("@Neg: ..0\n@Pos: 1..\nx: 0.5 & (@Neg | @Pos)", "x");
}

// ─────────────────────────────────────────────────────────────────────────
// ACTIVE both-sides pins — must stay green for the RIGHT reason (real deref)
// ─────────────────────────────────────────────────────────────────────────

#[test] // ACTIVE pin: satisfying merge passes via real template meet
fn e4_satisfying_merge_passes() {
    assert_obs_int(
        "@Adult: { age: 18.. }\nuser: { name: \"Alice\", age: 25 } & @Adult",
        "user.age",
        25,
    );
}

#[test] // ACTIVE pin: sealed exact-schema match passes (APP_03 vector)
fn e4_sealed_exact_match_passes() {
    assert_obs_int(
        "@User: {{ name: @str, age: @int }}\nr: { name: \"A\", age: 30 } & @User",
        "r.age",
        30,
    );
}

#[test] // ACTIVE pin: recursive type, satisfying shallow value
fn e4_recursive_shallow_passes() {
    assert_obs_int(
        "@Tree: { v: @int, next: @Tree | () }\nt: { v: 1, next: () } & @Tree",
        "t.v",
        1,
    );
}

#[test] // ACTIVE pin: builtin names are RESERVED — a user def must not shadow
fn e4_builtin_reserved_not_shadowable() {
    assert_obs_int("@int: { hacked: 1 }\na: @int & 10", "a", 10);
}

#[test] // ACTIVE pin: undefined @Name keeps Unknown pass-through fallback
fn e4_undefined_typeref_passthrough() {
    assert_obs_int("b: @Nonexistent & 10", "b", 10);
}

#[test]
// RED LINE (E4): satisfying side of union-of-typerefs — must distribute
// (⊥ | 10) and collapse to the single survivor 10 (same law as
// (1|7) & 1..3 → 1). Calibration itself caught the prior `10|10` wart.
fn e4_union_of_typerefs_passing() {
    assert_obs_int("@Neg: ..0\n@Pos: 1..\nx: 10 & (@Neg | @Pos)", "x", 10);
}

#[test] // ACTIVE guard: direct template merges (no @ref) — the machinery the
        // deref exposes; pinned so E4 cannot shift it
fn guard_direct_template_merges() {
    assert_obs_int(
        "a: { name: \"Alice\", age: 25 } & { age: 18.. }",
        "a.age",
        25,
    );
    assert_obs_bottom("a: { name: \"Alice\", age: 15 } & { age: 18.. }", "a");
    assert_obs_bottom("a: { name: \"Alice\", age: 25 } & {{ age: 18.. }}", "a");
}

#[test] // ACTIVE guard: builtin marker semantics untouched
fn guard_builtin_marker_semantics() {
    assert_obs_int("a: @int & 10", "a", 10);
    assert_obs_bottom("a: @int & \"x\"", "a");
}
