// Union lazy-⊥ cull probes (2026-07-17, pre-committed by work order —
// docs/union_cull_handover.md). G4 惰性⊥ ledger item (⊥-meta arc,
// 2026-07-14 exposure): "剔除只認即時 Bottom,thunk 漏".
//
// LAW (all existing — zero new adjudication; engine follows law):
//   - SPEC_07 L1-32: union path navigation = per-branch projection
//     (平等演化之觀測投影).
//   - SPEC_08 §3.2.2 #5 parenthetical: 「僅 `_|_` 支剔除」 — ⊥ branches
//     are culled from union results; #blur branches SURVIVE (guarded by
//     blur_boundary_probe_test::red_union_nav_blur_branch_survives);
//     Top branches survive (honest superposition, `1 | _`).
//   - ERROR_CODES note + REAL_04 §4: all-⊥ union → single ⊥ carrying
//     the PRIMARY cause (five-rank priority: #divergent > violations >
//     lattice family > resource family > #missing_key).
//   - Engineering supplement (blur-absorb verbatim + cause-canon T3
//     honest-message precedents): the all-⊥ result is the primary-rank
//     MEMBER's ⊥ passed out VERBATIM (message/detail preserved; tie →
//     encounter-order leftmost), not a fresh tag-only mint.
//
// MEASURED on v0.2.19 (dev a411caa): the cull law has no single home —
// it lives only in the unify Union-distribution arm (root evolve path)
// and the nav Union projection arm's immediate-Bottom match. Two leaks:
//   T1  nav projection leaves projected fields UNFORCED (Stage 2), so a
//       thunk forcing to ⊥ never matches Value::Bottom — `u.a` shows
//       `1 | _|_ #conflict` both orders; all-⊥ shows a bare double-⊥
//       union; `%cause` on the leaked union projects into a CAUSE UNION
//       (`#divergent | #conflict`); `u.a = 1` → #false lie.
//   T2  force_recursive's Union arm normalizes WITHOUT culling — a
//       direct `|` with a ⊥ member inside a field leaks at observation:
//       `{v: (1&2)|5}` observed at .v → `⊥ | 5` (NEW exposure, wider
//       than the ledgered nav face).
//   T3  all-⊥ mints are tag-only / generic: nav arm `primary.into()`
//       drops the member's honest message; root evolve all-⊥ funnels to
//       normalize_union's "empty union after normalize" jargon mint.
// Healthy today (pins): immediate-⊥ cull (cocoon miss), mixed-rank
// primary pick, Top/blur survivors, second-segment self-heal, direct
// root `|` cull, union commutativity/multiset `=`.
// NOT in scope: `<`/`<=` × union/blur order relations (§4.10 ledger);
// canonical display order; dispatch/apply/membership distribution sites
// (healthy, other laws); static-cycle × union projection CONTEXT
// DIVERGENCE (ledgered 另案, calibration exposure: `p: {v: p.v}` in
// `{v:9}|p` observed at .v gives `9 | _` via CLI evolve-solidified path
// but `9 | ⊥ #divergent` via lazy in-harness projection — the SPEC_12
// two-tier line classifies the same member differently per context; do
// NOT pin either shape here, adjudication belongs to the SPEC_12 family).

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
        "nlang-unioncull-{}-{}",
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

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — thunk-⊥ leaks (T1), observe-exit leak (T2), mint honesty (T3)
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_nav_thunk_bottom_culled() {
    // T1. Today: `1 | _|_ (%cause: #conflict) ;; …` — thunk ⊥ leaks.
    assert_obs("u: {a: 1}|{a: (2&3)}\nout: u.a", "1");
}

#[test]
fn red_nav_thunk_bottom_culled_rev() {
    // T1, mirrored order (dual-spelling lesson: pin BOTH orders).
    assert_obs("u: {a: (2&3)}|{a: 1}\nout: u.a", "1");
}

#[test]
fn red_nav_three_branch_survivor() {
    // T1. Today: `⊥ | ⊥ | 5`.
    assert_obs("u: {a: (1&2)}|{a: (3&4)}|{a: 5}\nout: u.a", "5");
}

#[test]
fn red_nav_all_bottom_primary_verbatim() {
    // T1+T3. Today: a bare DOUBLE-⊥ union. Law: all-⊥ → single ⊥,
    // primary cause; supplement: the member's ⊥ verbatim (its honest
    // "Incompatible types" message survives — not a tag-only remint).
    let got = observe_nlang("u: {a: (1&2)}|{a: (3&4)}\nout: u.a", "out");
    assert!(
        got.starts_with("_|_") && !got.contains(" | "),
        "all-⊥ union must collapse to a single ⊥: {got:?}"
    );
    assert!(
        got.contains("#conflict") && got.contains("Incompatible types"),
        "primary member's ⊥ must pass out verbatim: {got:?}"
    );
}

#[test]
fn red_cause_meta_divergent_priority() {
    // T1. Today: `#divergent | #conflict` — %cause projects over the
    // LEAKED union into a cause union. After cull: single ⊥ whose
    // primary rank picks #divergent (rank 1) over #conflict (rank 3).
    assert_obs(
        "m: {a: m.a + 1}\nu: m|{a: (1&2)}\nout: (u.a).%cause",
        "#divergent",
    );
}

#[test]
fn red_eq_after_cull() {
    // T1. Today: #false — the leaked ⊥ branch poisons `=` (G1 固化).
    assert_obs("u: {a: 1}|{a: (2&3)}\nout: u.a = 1", "#true");
}

#[test]
fn red_field_join_culled_on_observe() {
    // T2. Today: `_|_ #conflict ;; … | 5` — force_recursive's Union arm
    // normalizes without culling; the observation exit must apply the
    // same law as navigation (NEW exposure, wider than the ledger).
    assert_obs("u: {v: (1&2)|5}\nout: u.v", "5");
}

#[test]
fn red_root_all_bottom_verbatim_message() {
    // T3. Today: `;; empty union after normalize` — engine jargon. The
    // primary member's honest message must survive the all-⊥ collapse
    // on the root evolve path too.
    let got = observe_nlang("w: (1&2)|(3&4)\nout: w", "out");
    assert!(
        got.starts_with("_|_") && got.contains("#conflict"),
        "all-⊥ root join stays single ⊥ #conflict: {got:?}"
    );
    assert!(
        got.contains("Incompatible types") && !got.contains("empty union after normalize"),
        "verbatim member message, not the normalize jargon: {got:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// ACTIVE PINS — healthy faces that must survive the cull rewiring
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn pin_atom_open_branch_superposition() {
    // MIGRATED (2026-07-20, union_absorption): Top-family collapse —
    // open-miss `_` absorbs the definite branch (`1 | _` → `_`).
    // ⊥ cull unchanged; blur remains exempt vs Top.
    // MIGRATED-2 (2026-07-20, caused_top ruling C): the open-miss /
    // static-cycle Top is a CAUSED Top = diagnostic member — exempt from
    // absorption (SPEC_01 §2.4.2). Bare `_` still absorbs.
    assert_obs("u: {a: 1}|7\nout: u.a", "1 | _");
}

#[test]
fn pin_all_top_dedupe() {
    assert_obs("u: {b: 1}|{c: 2}\nout: u.a", "_");
}

#[test]
fn pin_immediate_bottom_cull_cocoon() {
    // Cocoon eigenstate arc face: immediate ⊥ branches cull today and
    // must keep culling — all-⊥ → single primary ⊥.
    assert_obs(
        "u: {{b: 1}}|{{c: 2}}\nout: (u.a).%cause",
        "#missing_key",
    );
}

#[test]
fn pin_mixed_rank_conflict_over_missing() {
    // REAL_04 §4: lattice family (rank 3) beats coordinate miss (rank 5).
    assert_obs(
        "u: {a: (1&2)}|{{c: 2}}\nout: (u.a).%cause",
        "#conflict",
    );
}

#[test]
fn pin_second_seg_self_heal() {
    // Second-segment cull already catches forced-⊥ branches mid-path;
    // must stay green through the rewiring.
    assert_obs("u: {a: {v: 1}}|{a: (2&3)}\nout: u.a.v", "1");
}

#[test]
fn pin_direct_join_root_cull() {
    // Root evolve path (unify Union-distribution arm) culls today.
    assert_obs("out: (1&2)|5", "5");
}

#[test]
fn pin_union_commutativity_eq_still() {
    // SPEC_01 `|` commutativity via multiset `=` (⊥-meta acceptance
    // repair pin family) — the unify-arm touch must not reorder.
    assert_obs("out: (1 | 2) = (2 | 1)", "#true");
}
