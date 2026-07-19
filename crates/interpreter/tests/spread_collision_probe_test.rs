// Spread collision-merge probes (2026-07-16, pre-committed by work
// order — docs/spread_collision_handover.md).
//
// LAW (SPEC_03, adjudicated 2026-07-16):
//   §3.1  Collision Merge — spread-vs-field key overlap is INTERSECT
//         (`&`), never overwrite ("`...` 始終遵循格論合併", §3.1.1).
//   §1.1  Repeated-Key Merge (NEW) — repeated keys in one literal are
//         the degenerate form of parallel definition: also `&`, order
//         irrelevant; path keys included ({a:{x:1}, a.y:2} → a merges).
//   §3.1  Heterogeneous spread — Atom spreads as { %val: v }; Top is a
//         no-op; Bottom collapses the whole target, PROPAGATING the
//         source's own %cause (2026-07-16 amendment: the old "#conflict"
//         wording predated the ⊥-meta rectification; minting a fresh
//         cause = horizon erasure).
//   §3.1  Circular Spread Protection — spreading yourself or an
//         ancestor is logical divergence → ⊥ #divergent. Duty covers
//         the DIRECT name form (under-construction name stack); alias
//         detours fall to the fuel horizon (record, don't chase).
// MEASURED on v0.2.12: collision = last-wins everywhere (field-then-
// spread, spread-then-field, double spread, range/union/meta/path
// keys); atom spread yields nothing; ⊥ spread silently swallowed;
// self-spread silently dropped, ancestor-spread runs away to fuel.
// NOT in scope: forward-ref × spread (eager construction — frozen pin,
// separate case), spread collision on effect release, `&` merge, Blur
// spread source (record current behavior only).

use nlang_interpreter::{Ouroboros, Universe};
use nlang_parser::parse_program;
use nlang_parser::ast::{Path, PathAnchor, Span};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

fn tmp_dir() -> PathBuf {
    let mut d = std::env::temp_dir();
    d.push(format!(
        "nlang-sprdcol-{}-{}",
        std::process::id(),
        DIR_SEQ.fetch_add(1, Ordering::SeqCst)
    ));
    fs::remove_dir_all(&d).ok();
    fs::create_dir_all(&d).unwrap();
    d
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

fn assert_bottom(src: &str, cause_frag: &str) {
    let got = observe_nlang(src, "out");
    assert!(
        got.starts_with("_|_") && got.contains(cause_frag),
        "{src:?} :: out — expected ⊥ with {cause_frag:?}, got {got:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — C1 collision merge = intersect, never overwrite (§3.1/§3.1.1)
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_collision_field_then_spread() {
    // L2-39. Today: 2 (overwrite).
    assert_bottom("q: { a: 1, ...{ a: 2 } }\nout: q.a", "conflict");
}

#[test]
fn red_collision_spread_then_field() {
    // Lattice merge is order-blind. Today: 1.
    assert_bottom("q: { ...{ a: 2 }, a: 1 }\nout: q.a", "conflict");
}

#[test]
fn red_collision_double_spread_spec_example() {
    // SPEC_03 §3.1.1's own visual example. Today: #error.
    assert_bottom(
        "~base: { status: #ok, priority: 1 }\n~patch: { status: #error }\nresult: { ...~base, ...~patch }\nout: result.status",
        "conflict",
    );
}

#[test]
fn red_collision_range_refines() {
    // Compatible constraints REFINE: (1..10) & (5..20) = 5..10. Today: 5..20.
    assert_obs("q: { a: 1..10, ...{ a: 5..20 } }\nout: q.a", "5..10");
}

#[test]
fn red_collision_union_refines() {
    // (1|2) & (2|3) = 2. Today: 2 | 3.
    assert_obs("q: { a: 1 | 2, ...{ a: 2 | 3 } }\nout: q.a", "2");
}

#[test]
fn red_collision_meta_key() {
    // Same law on the meta axis. Today: 2.
    assert_bottom("q: { %m: 1, ...{ %m: 2 } }\nout: q.%m", "conflict");
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — C1b repeated keys in one literal (§1.1, NEW law)
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_repeated_literal_key() {
    // L2-40. Today: 2 (last wins).
    assert_bottom("q: { a: 1, a: 2 }\nout: q.a", "conflict");
}

#[test]
fn red_path_key_merges_not_replaces() {
    // {a: {x:1}, a.y: 2} → a = {x:1} & {y:2}. Today: a = {y:2}, x lost.
    let got = observe_nlang("q: { a: { x: 1 }, a.y: 2 }\nout: q.a", "out");
    assert!(
        got.contains("x: 1") && got.contains("y: 2"),
        "path-key sibling must merge, got {got:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — C2 heterogeneous: atom spreads as {%val: v} (§3.1)
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_atom_spread_tag() {
    // L2-41. Today: `_` (silently skipped).
    assert_obs("q: { ...#ok }\nout: q.%val", "#ok");
}

#[test]
fn red_atom_spread_number() {
    assert_obs("q: { ...5 }\nout: q.%val", "5");
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — C3 Bottom spread collapses target, cause propagates (§3.1)
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_bottom_spread_collapses_target() {
    // Today: {b: 1} (⊥ silently swallowed).
    assert_bottom("bad: 1 & 2\nq: { b: 1, ...bad }\nout: q", "conflict");
}

#[test]
fn red_bottom_spread_cause_propagates() {
    // Q1 ruling: the source ⊥'s own cause travels — no fresh #conflict
    // minting (horizon erasure). Today: {b: 1}.
    assert_bottom(
        "p: { ~s: 1 }\nbad: p.~s\nq: { b: 1, ...bad }\nout: q",
        "private_access_violation",
    );
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — C4 circular spread → ⊥ #divergent (§3.1, direct name form)
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_circular_self_spread() {
    // L2-42. Today: {x: 1} (spread of the unbound self silently drops).
    assert_bottom("a: { x: 1, ...a }\nout: a", "divergent");
}

#[test]
fn red_circular_ancestor_spread() {
    // Today: runaway nested expansion until fuel.
    assert_bottom("a: { b: { ...a } }\nout: a.b", "divergent");
}

// ─────────────────────────────────────────────────────────────────────────
// ACTIVE PINS — healthy faces that must survive the change
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn pin_double_spread_noncolliding_travels() {
    // §3.1.1 example's healthy half: priority is not contested.
    assert_obs(
        "~base: { status: #ok, priority: 1 }\n~patch: { status: #error }\nresult: { ...~base, ...~patch }\nout: result.priority",
        "1",
    );
}

#[test]
fn pin_same_value_collision_stays() {
    // 1 & 1 = 1 — intersect of equals is invisible.
    assert_obs("q: { a: 1, ...{ a: 1 } }\nout: q.a", "1");
}

#[test]
fn pin_type_refine_collision() {
    // @int & 5 = 5 (today green by coincidence of overwrite; law-green
    // after the fix — must stay).
    assert_obs("q: { ...{ a: @int }, a: 5 }\nout: q.a", "5");
}

#[test]
fn pin_list_spread_indices() {
    assert_obs("q: { ...[10, 20] }\nout: q.0", "10");
    assert_obs("q: { ...[10, 20] }\nout: q.1", "20");
}

#[test]
fn pin_top_spread_noop() {
    assert_obs("q: { b: 1, ..._ }\nout: q.b", "1");
}

#[test]
fn pin_undefined_spread_noop() {
    // Undefined name = Top = no-op (the Top rule; NOT divergence —
    // circular duty is only for names under construction).
    assert_obs("q: { ...no_such_name, b: 1 }\nout: q.b", "1");
}

#[test]
fn pin_cocoon_unbox_and_target_stays_open() {
    assert_obs("q: { a: 1, ...{{ b: 2 }} }\nout: q.b", "2");
    // Target Attribute Preservation: spread never closes an open target.
    assert_obs("q: { ...{{ a: 1 }} }\nr: q & { b: 2 }\nout: r.b", "2");
}

#[test]
fn pin_spread_privacy_regression_guard() {
    // Previous arc stays closed: external spread excludes local axis.
    assert_obs("p: { ~s: 1, a: 2 }\nq: { ...p, peek: ~s }\nout: q.peek", "_");
}

#[test]
fn pin_insider_spread_keeps_local() {
    assert_obs("p: { ~s: 1, a: 2, c2: { ...p, rd: ~s } }\nout: p.c2.rd", "1");
}

#[test]
fn pin_enum_relation_seed_untouched() {
    // TRAP guard: `#{}` relation-seeded entries keep their current
    // semantics — collision-intersect applies to explicit fields and
    // spreads, not to relation seeding.
    let got = observe_nlang("~H: #{ #a < #b }\nout: ~H.#a", "out");
    assert!(
        got.contains("#a") && got.contains("%rank: 1"),
        "enum relation seed regressed: {got:?}"
    );
    assert_obs("~H: #{ #a < #b }\nout: ~H.#a < ~H.#b", "#true");
}

#[test]
fn pin_forward_ref_spread_frozen() {
    // UNFROZEN 2026-07-19 (forward_spread arc): spread expands at
    // observation convergence — source position is irrelevant.
    assert_obs("q: { ...later, b: 1 }\nlater: { a: 7 }\nout: q.a", "7")
}
