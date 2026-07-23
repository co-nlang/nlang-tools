// Effect composition = SET-UNION probes (2026-07-23, pre-committed by
// work order — docs/effect_union_handover.md). 效應系統波, arc 1.
//
// RULING (2026-07-23, user): the effect model migrates from a TOTALLY-
// ORDERED SCALAR (EffectTag { Pure<State<IO<NonDet }, composed by .max())
// to a SET / join-semilattice matching SPEC_08 §4.1's composition matrix.
// io / nondet / state are INCOMPARABLE siblings; `|` is the real join;
// the scalar total order was a category error that silently drops tags
// (io ⊔ nondet collapsed to #nondet — the IO fact vanished). The set
// model is the honest lattice and n/'s格論 ground.
//
// MEASURED (baseline, v0.2.33 dev): every multi-effect composition
// collapses to the scalar max —
//   {a:io, b:nondet}.%effect      → #nondet   (want #io | #nondet)
//   {a:io, b:state}.%effect       → #io       (want #io | #state)
//   {a:nondet, b:state}.%effect   → #nondet   (want #nondet | #state)
//   {a:io,b:nondet,c:state}       → #nondet   (want #io | #nondet | #state)
//   ({a:io} & {b:nondet})         → #nondet   (want #io | #nondet)
// Idempotency ({a:io,b:io}→#io), pure default, single-tag reads, cocoon
// shield, union-value projection ((5|io).%effect → #io | #pure), and the
// ⊥ meta-whitelist are already correct and MUST NOT move.
//
// Canonical multi-tag order (MEASURED via literal `#io | #nondet` etc.)
// = SPEC_01 §2.4.1 union display = alphabetical, order-independent:
// #io | #nondet | #state, and #io | #pure. The delivered `.%effect`
// read builds a normalize_union of the tag atoms (inheriting §2.4.1
// order); the `;; %effect:` display tail renders in the SAME order.
//
// NOT in scope (ledgered follow-on arcs, do NOT implement here):
//   • #cached solidification on stable CAID (§4.2.4) — the Cached tag may
//     be RESERVED in the type but no builtin produces it in arc 1.
//   • #ext:<id> custom tags (§4.1) — open-ended, not in the fixed set yet.
//   • static guard: pure-ctx + #io → ⊥ #effect_violation (§4.3).
//   • ~%Effect./runPure handler + %privilege_token (§4.3).
//   • full tag-SET participation in CAID normalization (§4.1 參與義務) —
//     arc 1 keeps the existing BINARY pure/impure horizon salt unchanged;
//     see the handover's CAID-stability guard (to_serial_byte legacy map).

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
        "nlang-effunion-{}-{}",
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

// Effect sources (match the spellings the effect_meta probes use):
//   io     = ~%Time.now _
//   nondet = ~%Math./random _
//   state  = ~%Engine./equivalence_map _

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — composition is set-union, not scalar max (SPEC_08 §4.1)
// ─────────────────────────────────────────────────────────────────────────

#[test]
#[ignore]
fn red_compose_io_nondet() {
    // Two incomparable active effects join to a two-tag set — the IO fact
    // survives instead of being swallowed by #nondet (scalar max).
    assert_obs(
        "c: { a: (~%Time.now _), b: (~%Math./random _) }\nout: c.%effect",
        "#io | #nondet",
    );
}

#[test]
#[ignore]
fn red_compose_io_state() {
    // Scalar max returned #io (IO=2 > State=1), dropping #state entirely.
    assert_obs(
        "c: { a: (~%Time.now _), b: (~%Engine./equivalence_map _) }\nout: c.%effect",
        "#io | #state",
    );
}

#[test]
#[ignore]
fn red_compose_nondet_state() {
    assert_obs(
        "c: { a: (~%Math./random _), b: (~%Engine./equivalence_map _) }\nout: c.%effect",
        "#nondet | #state",
    );
}

#[test]
#[ignore]
fn red_compose_three_tags() {
    // Full three-way join — scalar max kept only #nondet.
    assert_obs(
        "c: { a: (~%Time.now _), b: (~%Math./random _), c: (~%Engine./equivalence_map _) }\nout: c.%effect",
        "#io | #nondet | #state",
    );
}

#[test]
#[ignore]
fn red_compose_unify_join() {
    // §4.2.2 morphism/§4.1 unification contagion: & joins the two sides'
    // effects. {a:io} & {b:nondet} → the union, not the max.
    assert_obs(
        "u: { a: (~%Time.now _) } & { b: (~%Math./random _) }\nout: u.%effect",
        "#io | #nondet",
    );
}

#[test]
#[ignore]
fn red_compose_idempotent_multi() {
    // §4.1 idempotency E|E=E within a multi-tag set: two io fields + one
    // nondet dedup to exactly {io, nondet}, never #io | #io | #nondet.
    assert_obs(
        "c: { a: (~%Time.now _), b: (~%Math./random _), c: (~%Time.now _) }\nout: c.%effect",
        "#io | #nondet",
    );
}

#[test]
#[ignore]
fn red_compose_display_tail() {
    // SPEC_11 §3.4 diagnostic tail must render the whole set, in the same
    // canonical order as the `.%effect` read (baseline showed #nondet).
    let got = observe_nlang(
        "out: { a: (~%Time.now _), b: (~%Math./random _) }",
        "out",
    );
    assert!(
        got.contains(";; %effect: #io | #nondet"),
        "multi-tag combo carries the full set in its diagnostic tail: {got:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// ACTIVE PINS — invariants the set migration must preserve
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn pin_pure_default() {
    // Empty set renders as #pure, exactly as before.
    assert_obs("out: (42).%effect", "#pure");
    assert_obs("pc: { v: 1 }\nout: pc.%effect", "#pure");
}

#[test]
fn pin_single_tag_unchanged() {
    // A singleton set is a bare atom, NOT a one-element union.
    assert_obs("out: (~%Time.now _).%effect", "#io");
    assert_obs("out: (~%Math./random _).%effect", "#nondet");
    assert_obs("out: (~%Engine./equivalence_map _).%effect", "#state");
}

#[test]
fn pin_idempotent_single() {
    // Two io fields → the singleton {io}, printed as #io (never #io | #io).
    assert_obs(
        "c: { a: (~%Time.now _), b: (~%Time.now _) }\nout: c.%effect",
        "#io",
    );
}

#[test]
fn pin_unify_join_single() {
    // {a:1} & {b:io} → {io}, a bare atom (existing effect_meta green).
    assert_obs(
        "u: { a: 1 } & { b: (~%Time.now _) }\nout: u.%effect",
        "#io",
    );
}

#[test]
fn pin_cocoon_shield() {
    // §4.2.1 shield: contagion (now a set-union) still stops at the wall.
    assert_obs("k: {{ v: (~%Time.now _) }}\nout: k.%effect", "#pure");
    // multi-tag interior still shielded to #pure.
    assert_obs(
        "k: {{ a: (~%Time.now _), b: (~%Math./random _) }}\nout: k.%effect",
        "#pure",
    );
}

#[test]
fn pin_union_value_projection() {
    // A union VALUE projects each branch's effect (SPEC_07) — distinct
    // from an effect SET. This already produced a multi-tag display and
    // MUST stay #io | #pure (branch pure is a real projected tag).
    assert_obs("out: (5 | (~%Time.now _)).%effect", "#io | #pure");
}

#[test]
fn pin_bottom_meta_whitelist_unchanged() {
    // ⊥ meta whitelist stays %cause/%caid — %effect passes the ⊥ through
    // (F1 compositionality), regardless of the effect representation.
    let got = observe_nlang("bot: 1 & 2\nout: bot.%effect", "out");
    assert!(
        got.starts_with("_|_") && got.contains("#conflict"),
        "⊥.%effect passes the bottom through: {got:?}"
    );
}

#[test]
fn pin_single_tail_and_pure_silent() {
    // Display tail: single-tag value keeps `;; %effect: #io`; pure is silent.
    let got = observe_nlang("out: (~%Time.now _)", "out");
    assert!(got.contains(";; %effect: #io"), "single io tail: {got:?}");
    assert_obs("out: 42", "42");
}
