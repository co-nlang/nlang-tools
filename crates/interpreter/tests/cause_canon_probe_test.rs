// Cause-canon audit probes (2026-07-17, pre-committed by work order —
// docs/cause_canon_handover.md).
//
// RULING (approved 2026-07-17): the audit confirmed the engine already
// carries the LAW for all three fixes — zero new adjudication:
//   1. Two-source spread order dependence: unify has Blur×Bottom =
//      Bottom BOTH orders (⊥ is the lattice bottom); the spread arm's
//      blur early-return skips remaining sources, so {...big, ...bot}
//      leaks #fuel_exhausted where the derivation chain
//      ({t} & unbox(s1) & unbox(s2)) demands ⊥ #conflict. Fix: blur
//      does NOT early-return — keep folding remaining sources through
//      unify (⊥ early-return stays lawful: ⊥ absorbs everything).
//   2. Phantom #io: predict_effect blanket-tags any `~%…` path IO,
//      lying about pure morphisms (SPEC_09 §4 effect table: ~%Math is
//      pure; ~%Env/… genuinely IO). Fix: drop the blanket, let the
//      existing lookup read the ACTUAL stored effect.
//   3. #invalid_path last live mint (lib.rs follow_refine CAID parse
//      failure) retired → #conflict + honest error message upstream
//      (get_live_value currently claims "Refinement cycle detected").
//      No behavioral probe (needs store corruption); acceptance greps
//      mint sites == decode-only.
// LEDGER CORRECTION (帳載修正): the "`&`×blur CAID non-preservation"
// exposure is WITHDRAWN — it was a measurement artifact (three separate
// processes ⇒ three horizon salts ⇒ CAIDs necessarily differ). In-process
// the engine honors §3.2.2 #1 verbatimness: pinned permanently below.
// NOT in scope: forward-ref × spread (frozen; note: bot defined AFTER
// the spread observation gave #fuel_exhausted — that anomaly belongs to
// the frozen case, recorded there); REAL_04 §2/§4 rewrite (spec-side,
// acceptor's job); ~%Repl/%state effects.

use nlang_interpreter::{Ouroboros, Universe};
use nlang_parser::ast::{Path, PathAnchor, Span};
use nlang_parser::parse_program;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

fn tmp_dir() -> PathBuf {
    let mut d = std::env::temp_dir();
    d.push(format!(
        "nlang-causecanon-{}-{}",
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

fn flat_chain(n: usize) -> String {
    vec!["1"; n].join(" + ")
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — two-source spread: the fold must not stop at blur
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_two_source_bottom_after_blur() {
    // L2-63. Derivation: state after ...big = blur; blur & unbox(bot)
    // = ⊥ (unify Blur×Bottom, both orders) carrying bot's cause.
    assert_obs(
        &format!(
            "bot: 1 & 2\nbig: {}\nout: ({{ ...big, ...bot }}).%cause",
            flat_chain(4000)
        ),
        "#conflict",
    );
}

#[test]
fn red_blur_fold_continues_through_fields() {
    // The fold continues through interleaved plain fields too.
    assert_obs(
        &format!(
            "bot: 1 & 2\nbig: {}\nout: ({{ ...big, x: 1, ...bot }}).%cause",
            flat_chain(4000)
        ),
        "#conflict",
    );
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — effect-prediction honesty (SPEC_09 §4 effect table)
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_combo_system_apply_effect_clean() {
    // L2-64. ~%Math is PURE (SPEC_09 §4) — no phantom io tag.
    assert_obs("c: { v: ~%Math.abs (0 - 3) }\nout: c.v", "3");
}

#[test]
fn red_combo_system_pipe_effect_clean() {
    // Dual-spelling rule: apply AND pipe forms both pinned.
    assert_obs("c: { v: 2 |> ~%Math.abs }\nout: c.v", "2");
}

// ─────────────────────────────────────────────────────────────────────────
// ACTIVE PINS — laws the fixes must not disturb
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn pin_two_source_bottom_first() {
    // ⊥ early-return is lawful (⊥ absorbs everything, blur included).
    assert_obs(
        &format!(
            "bot: 1 & 2\nbig: {}\nout: ({{ ...bot, ...big }}).%cause",
            flat_chain(4000)
        ),
        "#conflict",
    );
}

#[test]
fn pin_blur_merge_caid_verbatim() {
    // L2-65 (green law pin, §3.2.2 #1): `&` absorption carries the
    // SOURCE snapshot verbatim — in-process, both orders. Permanent
    // replacement for the withdrawn cross-process artifact exposure.
    assert_obs(
        &format!(
            "big: {}\nout: (big & {{ b: 1 }}).%caid == big.%caid",
            flat_chain(4000)
        ),
        "#true",
    );
    assert_obs(
        &format!(
            "big: {}\nout: ({{ b: 1 }} & big).%caid == big.%caid",
            flat_chain(4000)
        ),
        "#true",
    );
}

#[test]
fn pin_unify_blur_bottom_both_orders() {
    // The existing unify law the spread fix leans on.
    assert_obs(
        &format!(
            "bot: 1 & 2\nbig: {}\nout: (big & bot).%cause",
            flat_chain(4000)
        ),
        "#conflict",
    );
    assert_obs(
        &format!(
            "bot: 1 & 2\nbig: {}\nout: (bot & big).%cause",
            flat_chain(4000)
        ),
        "#conflict",
    );
}

#[test]
fn pin_root_system_use_clean() {
    // Root-level system use was already clean — no over-correction.
    assert_obs("out: ~%Math.abs (0 - 7)", "7");
}

#[test]
fn pin_single_source_blur_absorb_still() {
    // Blur-spread arc law survives the fold rewrite (single source).
    assert_obs(
        &format!(
            "big: {}\nout: ({{ b: 1, ...big }}).%cause",
            flat_chain(4000)
        ),
        "#fuel_exhausted",
    );
}
