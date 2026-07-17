// Static-cycle taint scoping probes (2026-07-17, pre-committed by work
// order — docs/taint_scope_handover.md). Successor of the union-cull
// arc's "static-cycle × union projection context divergence" ledger
// item — REDIAGNOSED this arc (帳載修正第九次): the "CLI vs harness"
// framing was a measurement artifact. The true variable is FORCE ORDER
// within one observation context.
//
// LAW (all existing — zero new adjudication; engine follows law):
//   SPEC_12 §1.1: two-tier line — pure-reference cycle (裸名+純路徑)
//   → caused Top (#static_cycle); any transform hop → ⊥ #divergent.
//   Q2: "非純引用即變換" classifies the CYCLE'S OWN hops. Q4 guard:
//   格律中立 + 不傳播 — the cause evaporates on consumption and the
//   classification must not leak across unrelated evaluations.
//
// ROOT CAUSE (instrumented worktree, 2026-07-17): chain_transform_taint
// is written BACK to the parent ctx after every force
// (`ctx.taint ||= call_ctx.taint`, "once transform, always transform")
// — chain state globalized to the whole observation context. Forcing
// ANY non-pure-ref thunk (even the literal `9`; TAINT_SET fired on
// expr_kind=atom) permanently taints the ctx, and every LATER static-
// cycle re-entry in the same observation is misclassified as transform
// → ⊥ #divergent → (since the union-cull arc) lawfully CULLED, so the
// `_` member silently vanishes. Downward inheritance (sub_context
// clone) is CORRECT — a real transform cycle taints its own chain and
// the re-entry fires inside it; only the upward write-back is the bug.
// Measured order dependence: `{v:9}|p` at .v → `9` (member erased) but
// `p|{v:9}` → `_ | 9` (healthy); alias and mutual-cycle members same;
// twin `=` on identically-spelled unions → #false (equality lie).
// Sibling faces (`w: {a:1+1, b:p.v}`) measure healthy today only
// because p.v classifies during evolve BEFORE the arithmetic forces —
// incidental timing, not correct scoping.
// NOT in scope: math over Top-membered unions (`(_|9)+1` → ⊥ #conflict
// today — separate value-context question, ledgered); TopCaused vs Top
// dedupe in normalize; canonical display order; the union-cull arc's
// machinery (guarded by union_bottom_cull_probe_test — must stay green).

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
        "nlang-taintscope-{}-{}",
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
// RED GATES — sibling-force taint pollution misclassifies static cycles
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_union_static_member_survives() {
    // Today: `9` — branch1's literal force taints the ctx, p.v re-entry
    // reads transform → ⊥ #divergent → culled. Law: static → caused Top
    // survivor, honest `9 | _`.
    assert_obs("p: {v: p.v}\nu: {v: 9}|p\nout: u.v", "9 | _");
}

#[test]
fn red_union_static_member_survives_mid() {
    // Three branches, static member in the middle. Today: `9 | 8`.
    assert_obs("p: {v: p.v}\nu: {v: 9}|p|{v: 8}\nout: u.v", "9 | _ | 8");
}

#[test]
fn red_union_static_member_alias() {
    // Alias spelling (dual-spelling lesson). Today: `9`.
    assert_obs("p: {v: p.v}\nal: p\nu: {v: 9}|al\nout: u.v", "9 | _");
}

#[test]
fn red_union_mutual_static_member() {
    // Mutual pure-reference cycle member (SPEC_12 互指 = same tier).
    // Today: `9`.
    assert_obs(
        "a1: {v: b1.v}\nb1: {v: a1.v}\nu: {v: 9}|a1\nout: u.v",
        "9 | _",
    );
}

#[test]
fn red_field_join_static_member() {
    // In-field direct `|` — RED in-harness (`9`: observe-time forcing
    // hits the polluted ctx) while the CLI run measures `_ | 9` (evolve
    // pre-classified) — the very context-dependence this arc removes.
    assert_obs("p: {v: p.v}\nw: {q: p.v | 9}\nout: w.q", "_ | 9");
}

#[test]
fn red_union_twin_eq() {
    // Today: #false — identically-spelled unions solidify differently
    // depending on taint state at force time (equality lie; twin-eq
    // tripwire family). After scoping: both sides `9 | _`-shaped.
    assert_obs(
        "p: {v: p.v}\nu1: p|{v: 9}\nu2: p|{v: 9}\nout: u1 = u2",
        "#true",
    );
}

// ─────────────────────────────────────────────────────────────────────────
// ACTIVE PINS — healthy faces that must survive the taint rescoping
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn pin_static_direct_top() {
    // SPEC_12 §1.1 tier 1: pure path self-loop → caused Top, displays _.
    assert_obs("p: {v: p.v}\nout: p.v", "_");
}

#[test]
fn pin_static_cause_readable() {
    assert_obs("p: {v: p.v}\nout: (p.v).%cause", "#static_cycle");
}

#[test]
fn pin_union_static_first_order() {
    // Static member FIRST already healthy (classified before the
    // literal taints) — must stay through the rescoping.
    assert_obs("p: {v: p.v}\nu: p|{v: 9}\nout: u.v", "_ | 9");
}

#[test]
fn pin_direct_join_static_root() {
    // Root-level direct `|` healthy (evolve-time classification lands
    // before any sibling force). In-field spelling is a RED gate above.
    assert_obs("p: {v: p.v}\nout: p.v | 9", "_ | 9");
}

#[test]
fn pin_transform_member_still_culled() {
    // Transform cycle member stays ⊥ #divergent → lawfully culled
    // (union-cull arc). Rescoping must not un-taint REAL transforms.
    assert_obs("m: {a: m.a + 1}\nu: {a: 1}|m\nout: u.a", "1");
}

#[test]
fn pin_transform_direct_still_divergent() {
    // The cycle's own hop taints its own chain — downward inheritance
    // stays. L2-54 family.
    assert_obs("m: {a: m.a + 1}\nout: (m.a).%cause", "#divergent");
}

#[test]
fn pin_combo_sibling_static() {
    // Sibling arithmetic + static cycle in one combo — healthy today
    // (incidental evolve-time classification), lawful `_` either way.
    assert_obs("p: {v: p.v}\nw: {a: 1 + 1, b: p.v}\nout: w.b", "_");
}

#[test]
fn pin_thunk_bottom_cull_still() {
    // Union-cull arc guard: thunk-⊥ member still culled (adjacent arc
    // must not regress while taint moves to chain scope).
    assert_obs("u: {a: 1}|{a: (2&3)}\nout: u.a", "1");
}
