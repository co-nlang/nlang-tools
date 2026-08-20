// G2 decomposition probes (2026-07-12, pre-committed by work order —
// docs/g2_shadow_multiparam_handover.md).
//
// The corpus-cleanup ledger entry "G2: `/`-prefixed curried defs break
// every application form" was RE-DIAGNOSED 2026-07-12. Measured truth:
//   G2-M  multi-param sugar `x y -> body` (SYNTAX_11 §table: legal,
//         auto-curry) parses as Morphism{param: Apply(x,y)} and is
//         packaged into a dispatch table that never fires — broken for
//         bare AND slash defs alike. `/` was never the variable.
//   G2-S  a user `/name:` def whose coordinate collides with a ROOT
//         builtin rule (today exactly `/add` = math.add cocoon) evolves
//         silently, then poisons the ENTIRE universe at observe-entry
//         unify(root, staged) — every observation returns ⊥ #conflict
//         with no path. Data-axis conflicts, by contrast, fail loudly
//         at evolve with a named path (measured: exit 1 vs exit 0).
//   G2-C  do_unify's Atom×Combo arm absorbs an atom into ANY combo as
//         `%val` — including closed morphism cocoons (`/add: 7` grows
//         the builtin cocoon a `%val: 7` key, no conflict). Morphisms
//         are not value-carriers.
// Non-colliding `/` defs (`/myadd`, `/assert_eq`) work in ALL
// application forms today — pinned below.
//
// RULINGS (adjudicated 2026-07-12):
//   M: `x y -> body` ≡ `x -> (y -> body)` — desugar at AST build
//      (SPEC_14 §2.3 fold precedent). Fold ONLY Apply chains whose
//      leaves are all bare single-segment paths; other param shapes
//      keep current behavior. Tuple params `((x, y) -> …)` are a
//      DIFFERENT form (SYNTAX_11 rule 4) and are out of scope (G5:
//      dispatch-side destructure unimplemented — separate order).
//   S: root coordinates evolve monotonically; an incoming top-level
//      binding whose unify with the existing root binding is ⊥ MUST
//      fail at the evolve boundary with a named Evolution Conflict
//      (same UX as staged-staged data conflicts). No silent poisoning.
//   C: unify(Atom, morphism-combo) = ⊥ #conflict. The %val absorb
//      stays for NON-morphism combos (pinned below).

use nlang_interpreter::{Ouroboros, Universe, Value};
use nlang_parser::ast::{Path, PathAnchor, Span};
use nlang_parser::parse_program;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

fn tmp_dir() -> nlang_interpreter::ScratchDir {
    nlang_interpreter::ScratchDir::new("g2probe")
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
            let mut universe = Universe::new(None, engine.root_with_system());
            let program = parse_program(&src).unwrap();
            for f in &program.fields {
                // Ignore evolve errors here — observation probes assert on
                // the observed value; evolve-status probes use
                // evolve_statuses below.
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

/// Per-field evolve verdicts (Ok / Err) — the G2-S/G2-C boundary probes.
fn evolve_statuses(src: &str) -> Vec<bool> {
    let src = src.to_string();
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(move || {
            let dir = tmp_dir();
            let engine = Ouroboros::init(&dir).unwrap();
            let mut universe = Universe::new(None, engine.root_with_system());
            let program = parse_program(&src).unwrap();
            program
                .fields
                .iter()
                .map(|f| universe.evolve(&engine, f).is_ok())
                .collect()
        })
        .unwrap()
        .join()
        .unwrap()
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — G2-M multi-param auto-curry (SYNTAX_11 §table)
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_multiparam_bare_juxta() {
    // today: "_" (dispatch never fires)
    assert_obs("beq: x y -> x == y\nout: beq 5 5", "#true");
}

#[test]
fn red_multiparam_slash_juxta() {
    // today: "_" — identical failure to bare (the `/` was never the variable)
    assert_obs("/aeq: x y -> x == y\nout: aeq 5 5", "#true");
}

#[test]
fn red_multiparam_slash_paren_chain() {
    assert_obs("/aeq: x y -> x == y\nout: (/aeq 5) 5", "#true");
}

#[test]
fn red_multiparam_pipe() {
    // today: pipe silently returns the piped value (5)
    assert_obs("aeq: x y -> x == y\nout: 5 |> aeq 5", "#true");
}

#[test]
fn red_multiparam_three_params() {
    assert_obs("th: x y z -> x + y + z\nout: th 1 2 3", "6");
}

#[test]
fn red_multiparam_neq_arm() {
    // #false side — proves the body actually evaluates, not a vacuous #true
    assert_obs("aeq: x y -> x == y\nout: aeq 5 6", "#false");
}

#[test]
fn red_multiparam_equiv_explicit_curry() {
    // The ruling IS the equivalence: sugar and explicit spelling agree
    assert_obs(
        "m: x y -> x * 100 + y\ne: (x -> (y -> x * 100 + y))\nout: (m 3 5) == (e 3 5)",
        "#true",
    );
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — G2-S root-builtin shadow must fail LOUDLY at evolve
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_shadow_builtin_add_morphism_errs_at_evolve() {
    // O65: `/add` is no longer a standard-root coordinate, so a user
    // `/add:` is an ordinary overlay (same as `/myadd`).
    let st = evolve_statuses("/add: (x -> (y -> x + y))\nz: 42");
    assert!(st[0], "user /add is a free name after O65");
    assert!(st[1], "unrelated field z must not be blamed");
}

#[test]
fn red_shadow_builtin_add_atom_errs_at_evolve() {
    // O65: no standard-root `/add` cocoon remains to collide with.
    let st = evolve_statuses("/add: 7\nz: 42");
    assert!(st[0], "user /add is a free name after O65");
    assert!(st[1]);
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATE — G2-C atom × morphism = ⊥ (unify-level observable)
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_atom_meet_morphism_is_bottom() {
    // today: absorbs into the dispatch combo as %val
    assert_obs("m: (x -> x)\nout: m & 7", "_|_ (%cause: #conflict)");
}

// ─────────────────────────────────────────────────────────────────────────
// ACTIVE pins — everything that works today and must SURVIVE the fix
// ─────────────────────────────────────────────────────────────────────────

#[test] // ACTIVE pin: non-colliding slash def, juxtaposition
fn pin_slash_noncolliding_juxta() {
    assert_obs("/myadd: (x -> (y -> x + y))\nout: myadd 3 5", "8");
}

#[test] // ACTIVE pin: non-colliding slash def, explicit paren chain
fn pin_slash_noncolliding_paren_chain() {
    assert_obs("/myadd: (x -> (y -> x + y))\nout: (/myadd 3) 5", "8");
}

#[test] // ACTIVE pin: the corpus assert-library shape, explicit curry
fn pin_slash_assert_eq_false_arm() {
    assert_obs(
        "/assert_eq: (x -> (y -> x == y))\nout: assert_eq 5 6",
        "#false",
    );
}

#[test] // ACTIVE pin: non-colliding slash def evolves clean — the G2-S
        // collision check must fire ONLY on real root collisions
fn pin_slash_noncolliding_evolve_ok() {
    let st = evolve_statuses("/myadd: (x -> (y -> x + y))\nz: 42");
    assert!(st.iter().all(|ok| *ok));
}

#[test] // ACTIVE pin: bare data-axis def does NOT collide with builtin
        // rules-axis /add — distinct coordinates, user def wins lookup
fn pin_bare_add_def_shadows_nothing() {
    assert_obs("add: (x -> (y -> x * 100 + y))\nout: add 3 5", "305");
}

#[test] // ACTIVE pin: arithmetic lives in `~%Math`; top-level `/add` is gone (O65).
fn pin_bare_lookup_falls_to_builtin() {
    assert_obs("out: ~%Math./add 3 5", "8");
}

#[test] // ACTIVE pin: combo-local /add never touches root — no collision
fn pin_combo_local_slash_add_ok() {
    assert_obs(
        "obj: { /add: (x -> (y -> x + y)), v: 10 }\nout: obj.v",
        "10",
    );
}

#[test] // ACTIVE pin: %val absorb for NON-morphism combos stays — the
        // G2-C fix must be scoped to morphisms only.
        // G6 collapsed observe peels hybrid %val; prove the hybrid shape
        // via structural dual (SYNTAX_07) + field navigation.
fn pin_nonmorphism_val_absorb_survives() {
    assert_obs(
        "x: { note: \"n\" } & 5\nout: <<x>>",
        "{\n  %val: 5\n  note: \"n\"\n}",
    );
    assert_obs("x: { note: \"n\" } & 5\nout: x.note", "\"n\"");
}

#[test] // ACTIVE pin: staged-staged data conflict still errs at evolve
        // (the loud path G2-S is being aligned WITH)
fn pin_data_axis_conflict_errs_at_evolve() {
    let st = evolve_statuses("a: 1\na: 2");
    assert!(st[0]);
    assert!(!st[1], "second a: must Err at evolve (existing law)");
}

#[test] // ACTIVE pin: explicit-curry anonymous morphism applies inline
fn pin_anonymous_explicit_curry_inline() {
    assert_obs("ec: (x -> (y -> x + y))\nout: ec 3 5", "8");
}
