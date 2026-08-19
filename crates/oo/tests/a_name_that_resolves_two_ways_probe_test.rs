// A name that resolves two ways (Q-033 audit §2:
// docs/a_root_only_one_engine_can_build_audit.md).
//
// ── The claim ────────────────────────────────────────────────────────────
//
// O58's work order (§2.1) ruled the lookup direction and forbade changing
// it: the user root is consulted BEFORE the standard root, so a user
// definition wins. The Q-032 delivery (a71a69b) added two lookup sites and
// got only one of them right --
//
//   bare name   lib.rs:3673 ctx.root -> :3700 ctx.standard_root   (correct)
//   projection  lib.rs:3813 ctx.standard_root -> :3840 ctx.root   (inverted)
//
// so ONE name resolves to TWO values depending on whether it is projected.
// The user's field is committed into the root object and enters its
// identity, and the engine that wrote it cannot read it back.
//
// ── Why these assertions, and not P1's ───────────────────────────────────
//
// The previous arc's P1 asserted that `evolve` did not print "Error". It
// went green while the outcome it stood for was never delivered, because
// the failure is at OBSERVATION, not at write. Every red below observes.
//
// ── Probe integrity ──────────────────────────────────────────────────────
//
// Reds are `#[ignore]`d and MUST be red at the baseline for the reason
// stated in each. A fix may remove `#[ignore]` and NOTHING else in this
// file. Assertions pin behaviour, never the wording of a refusal.

use std::path::Path;
use std::process::Command;

fn oo(dir: &Path, args: &[&str]) -> String {
    let mut c = Command::new(env!("CARGO_BIN_EXE_oo"));
    c.current_dir(dir)
        .env("OO_IDENTITY", dir.join("identity-for-tests"))
        .env("OO_NODE_HOME", dir.join("node-home-for-tests"));
    let o = c.args(args).output().expect("oo runs");
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

fn scratch(tag: &str) -> nlang_interpreter::ScratchDir {
    nlang_interpreter::ScratchDir::new(&format!("tworesolve-{tag}"))
}

/// Observe `app` for one source; returns the printed observation.
fn observe(tag: &str, src: &str) -> String {
    let d = scratch(tag);
    std::fs::write(d.join("a.n"), src).unwrap();
    oo(&d, &["run", "a.n", "--observe", "app"])
}

// ── C1..C4 ── controls: green at baseline, MUST stay green ───────────────

/// A name the standard root does not occupy projects to the user's value.
/// If this is not green the harness itself is broken, not the engine.
#[test]
fn c1_control_a_free_name_projects_to_the_user_value() {
    let out = observe("c1", "@zzz: { mine: 9 }\napp: { v: @zzz.mine }\n");
    assert!(
        out.contains("v: 9"),
        "a name outside the standard root must project to the user's value, got: {out}"
    );
}

/// The standard root is still reachable through projection when the user
/// has NOT defined the name. The fix must not sever the lookup, only
/// reorder it.
#[test]
fn c2_control_the_standard_root_still_projects_when_unshadowed() {
    let out = observe("c2", "app: { v: ~%Math./abs }\n");
    assert!(
        out.contains("math.abs"),
        "an unshadowed standard-root name must still project, got: {out}"
    );
}

/// The bare-name path is already correct and must remain so.
#[test]
fn c3_control_the_bare_name_already_yields_the_user_value() {
    let out = observe("c3", "/add: { mine: 1 }\napp: { v: /add }\n");
    assert!(
        out.contains("mine: 1"),
        "bare `/add` must yield the user's combo, got: {out}"
    );
}

/// E4: the reserved validator set is a SEPARATE axis from lookup order.
/// A user definition must not weaken `@int` as a constraint. This governs
/// `&`, not projection, and the fix must leave it untouched.
#[test]
fn c4_control_a_user_def_does_not_weaken_the_reserved_validator() {
    let out = observe("c4", "@int: { hacked: 1 }\napp: { v: @int & 10 }\n");
    assert!(
        out.contains("v: 10"),
        "`@int` must still validate as a constraint, got: {out}"
    );
}

// ── R1..R4 ── reds ───────────────────────────────────────────────────────

/// RED at baseline: projection consults the standard root first, so
/// `/add.mine` finds the standard root's closed cocoon and reports the
/// user's own field missing.
#[test]
fn r1_a_user_definition_is_projectable() {
    let out = observe("r1", "/add: { mine: 1 }\napp: { v: /add.mine }\n");
    assert!(
        out.contains("v: 1"),
        "the user's `/add.mine` must be observable, got: {out}"
    );
}

/// RED at baseline: the same inversion seals the three standard-root type
/// coordinates. This is the lookup order, NOT E4 -- c4 pins the validator
/// axis separately, and `@int` (reserved but absent from the standard
/// root) already projects to the user's value today.
#[test]
fn r2_the_standard_root_type_coordinates_are_projectable() {
    for n in ["@list", "@option", "@result"] {
        let out = observe(
            "r2",
            &format!("{n}: {{ mine: 2 }}\napp: {{ v: {n}.mine }}\n"),
        );
        assert!(
            out.contains("v: 2"),
            "`{n}.mine` must be observable once lookup order is uniform, got: {out}"
        );
    }
}

/// RED at baseline: bare and projected forms of ONE name disagree. This is
/// the defect stated without reference to any particular coordinate --
/// whatever `/add` means, both lines must mean the same thing by it.
#[test]
fn r3_one_name_does_not_resolve_two_ways() {
    let out = observe(
        "r3",
        "/add: { mine: 1 }\napp: { bare: /add.mine, viaparen: (/add).mine }\n",
    );
    let agree = (out.contains("bare: 1") && out.contains("viaparen: 1"))
        || (!out.contains("bare: 1") && !out.contains("viaparen: 1"));
    assert!(
        agree && out.contains("bare: 1"),
        "both forms of one name must resolve to the user's value, got: {out}"
    );
}

/// RED at baseline: the write/read asymmetry. The field is committed into
/// the root object and enters its identity; the engine that wrote it must
/// be able to read it back.
#[test]
fn r4_what_the_root_object_holds_can_be_read_back() {
    let d = scratch("r4");
    std::fs::write(d.join("a.n"), "/add: { mine: 1 }\napp: { v: 1 }\n").unwrap();
    let e = oo(&d, &["evolve", "a.n"]);
    assert!(!e.contains("Error"), "harness: evolve must succeed, got: {e}");
    let c = oo(&d, &["commit", "-m", "x"]);
    assert!(
        c.contains("Commit successful"),
        "harness: commit must succeed, got: {c}"
    );

    // The field really is in the committed root object ...
    let addr = c
        .split_whitespace()
        .find(|t| t.starts_with("hash:sha256:"))
        .expect("harness: commit prints an address")
        .to_string();
    let commit = oo(&d, &["inspect", &addr]);
    let root = commit
        .lines()
        .find_map(|l| l.strip_prefix("root:"))
        .expect("harness: commit names a root")
        .trim()
        .to_string();
    let obj = oo(&d, &["inspect", &root]);
    assert!(
        obj.contains("mine: 1"),
        "harness: the root object must carry the user's field, got: {obj}"
    );

    // ... so reading it back must not report it missing.
    let out = oo(&d, &["run", "a.n", "--observe", "app"]);
    let back = observe("r4b", "/add: { mine: 1 }\napp: { v: /add.mine }\n");
    assert!(
        back.contains("v: 1"),
        "a field held in the root object must be readable, got: {back} (commit side: {out})"
    );
}
