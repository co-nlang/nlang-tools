// System-axis (~%) ownership probes (2026-07-16, pre-committed by work
// order — docs/system_axis_handover.md).
//
// RULING (SPEC_09 ownership clause, approved 2026-07-16): the `~%`
// namespace is ENGINE-MINTED ONLY. Stdlib CAIDs are the shared identity
// basis; any user LHS write to a `~%` coordinate — existing module
// (`~%Math: 5`), path key (`~%Math.add: 7`), novel name (`~%Mine: …`) —
// is illegal EVEN IF monotone (ownership criterion, not content).
// Two violation tracks:
//   root LHS  → loud named error at the evolve boundary (same machinery
//               family as the G2-S root monotone law; exit 1 at CLI);
//   combo key → that field mints ⊥ %cause #system_reserved (node-level,
//               composes through nav/apply, NO self-heal — the lexical
//               chain must NOT skip the illegal field to reach the real
//               system module; self-healing hides the crime).
// EXEMPTION: root `~%Config.<bare-field>` writes = the horizon-parameter
// canonical family (SPEC_08 §3.1) — stay legal. Combo-level ~%Config is
// NOT exempt (node hints have the %fuel downgrade channel).
// RHS fully preserved: alias / intersection-import / path use unchanged
// ("intersection IS import", SYNTAX_05).
// MEASURED on v0.2.16+: three mutually inconsistent behaviors coexist —
// root shadow silently IGNORED (evolve Ok, builtin intact, user lied
// to), combo shadow silently EFFECTIVE (poisons lexical scope, c.v → _),
// novel names freely minted. Wart: `c.~%Math` displays `9 ;; %effect:
// #io` (phantom io tag) — expected to die with ⊥ minting; if it
// survives elsewhere, record, don't chase.
// NOT in scope: parser spelling (parse of `~%sys: 1` stays LEGAL — the
// violation is semantic; parser goldens pin it); `~` private axis
// (longest-match boundary pinned below); ~%Config field-name validation.

use nlang_interpreter::{Ouroboros, Universe};
use nlang_parser::ast::{Path, PathAnchor, Span};
use nlang_parser::parse_program;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

fn tmp_dir() -> nlang_interpreter::ScratchDir {
    nlang_interpreter::ScratchDir::new("sysaxis")
}

/// 64 MiB thread — parser/eval recursion headroom (established pattern).
/// Returns (evolve_all_ok, observation of `path` as canonical text).
fn run_program(src: &str, path: &str) -> (bool, String) {
    let src = src.to_string();
    let path = path.to_string();
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(move || {
            let dir = tmp_dir();
            let engine = Ouroboros::init(&dir).unwrap();
            let mut universe = Universe::new(None, engine.root_with_system());
            let program = parse_program(&src).unwrap();
            let mut all_ok = true;
            for f in &program.fields {
                if universe.evolve(&engine, f).is_err() {
                    all_ok = false;
                }
            }
            let p = Path {
                anchor: PathAnchor::Bare,
                segments: path.split('.').map(|s| s.to_string()).collect(),
                span: Span::default(),
            };
            (all_ok, universe.observe(&engine, &p).to_nlang(0))
        })
        .unwrap()
        .join()
        .unwrap()
}

fn assert_obs(src: &str, expect: &str) {
    let (_, got) = run_program(src, "out");
    assert_eq!(got, expect, "{src:?} :: out");
}

/// Root LHS system-axis write must FAIL LOUDLY at the evolve boundary.
fn assert_evolve_rejects(src: &str) {
    let (all_ok, _) = run_program(src, "out");
    assert!(
        !all_ok,
        "{src:?} — root ~% write must error at the evolve boundary (silent \
         acceptance = user lied to)"
    );
}

fn flat_chain(n: usize) -> String {
    vec!["1"; n].join(" + ")
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — root track: loud evolve-boundary rejection
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_root_shadow_module_loud() {
    assert_evolve_rejects("~%Math: 5\nout: 1");
}

#[test]
fn red_root_shadow_path_loud() {
    assert_evolve_rejects("~%Math.add: 7\nout: 1");
}

#[test]
fn red_root_novel_loud() {
    assert_evolve_rejects("~%Mine: 5\nout: 1");
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — combo track: field mints ⊥ #system_reserved
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_combo_system_key_bottom() {
    // L2-60.
    assert_obs(
        "c: { ~%Math: 9 }\nout: (c.~%Math).%cause",
        "#system_reserved",
    );
}

#[test]
fn red_combo_novel_key_bottom() {
    // L2-61.
    assert_obs(
        "d: { ~%Mine: 5 }\nout: (d.~%Mine).%cause",
        "#system_reserved",
    );
}

#[test]
fn red_combo_poison_diagnosable_no_self_heal() {
    // The measured lie: local ~%Math shadows the real module and use
    // sites silently die to `_`. After the fix the illegal field is ⊥
    // and v composes to ⊥ WITH THE CAUSE — diagnosable. The lexical
    // chain must NOT self-heal to the real ~%Math (which would give 3).
    assert_obs(
        "c: { ~%Math: 9, v: ~%Math.abs (0 - 3) }\nout: (c.v).%cause",
        "#system_reserved",
    );
}

// ─────────────────────────────────────────────────────────────────────────
// ACTIVE PINS — exemption family, RHS faces, axis boundaries
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn pin_config_fuel_write() {
    // THE trap pin (SPEC_08 §3.1 canonical family): root ~%Config bare
    // fields stay writable and EFFECTIVE.
    assert_obs(
        &format!(
            "~%Config.fuel: 50\nbig: {}\nout: big.%cause",
            flat_chain(300)
        ),
        "#max_depth_exceeded",
    );
}

#[test]
fn pin_config_write_smoke() {
    // Exemption path must not trip the new rejection.
    let (all_ok, got) = run_program("~%Config.fuel: 1000000\nout: 1 + 1", "out");
    assert!(all_ok, "~%Config write must evolve cleanly");
    assert_eq!(got, "2");
}

#[test]
fn pin_rhs_alias() {
    // RHS read: aliasing a system module is legal.
    assert_obs("m: ~%Math\nout: 2 |> m.abs", "2");
}

#[test]
fn pin_rhs_path_use() {
    // L2-62 shape (green law pin): direct path use.
    assert_obs("out: ~%Math.abs (0 - 7)", "7");
}

#[test]
fn pin_root_import_merge() {
    // "Intersection IS import" (SYNTAX_05): RHS ~% in a root merge field
    // must keep evolving cleanly.
    let (all_ok, got) = run_program("_: ~%Cond\nout: 1", "out");
    assert!(all_ok, "root RHS import merge must stay legal");
    assert_eq!(got, "1");
}

#[test]
fn pin_data_axis_name_free() {
    // The reservation is the AXIS, not the names: a data-axis field
    // named `add` is untouched (G2-S guarded the LOGIC axis separately).
    assert_obs("add: 5\nout: add", "5");
}

#[test]
fn pin_combo_path_key_whole_field_bottom() {
    // ACCEPTANCE REPAIR PIN (2026-07-16): the SECOND spelling. A
    // forbidden path key ({~%Math.add: 7}) minted ⊥ only at the LEAF and
    // materialized `~%Math: {add: ⊥}` as a user coordinate — the silent
    // shadow lie resurrected via path-key spelling (dual-spelling lesson
    // #3). The whole field collapses at the FIRST segment.
    assert_obs(
        "c: { ~%Math.add: 7 }\nout: (c.~%Math).%cause",
        "#system_reserved",
    );
    // Poison scenario through the second spelling stays diagnosable.
    assert_obs(
        "c: { ~%Math.add: 7, v: ~%Math.abs (0 - 3) }\nout: (c.v).%cause",
        "#system_reserved",
    );
}

#[test]
fn pin_private_axis_untouched() {
    // Longest-match boundary: `~` (private/local) is a different axis;
    // insider lexical read through it stays intact.
    assert_obs("c: { ~z: 9, k: ~z + 1 }\nout: c.k", "10");
}
