// The half that was never written (O58 + O61 + O62, pre-committed by work
// order: docs/the_half_that_was_never_written_handover.md).
//
// ── The claim ────────────────────────────────────────────────────────────
//
// A root's CAID is hash(standard root ⊕ user content) -- all 67 KB of a thing
// that is never written to disk. This arc unmerges it: the standard root
// becomes an addressable object the root NAMES, and a lookup layer behind the
// root rather than fields merged into it.
//
// Riding along, because they move the same thing and identity should move
// once (WORK_QUEUE §9.0):
//   O61  the propagated effect tag moves into the hashed %effect field
//   O62  absent means #pure; only non-⊥ effects materialise
//
// ── What this file can and cannot witness ────────────────────────────────
//
// It CANNOT witness that old stores still open. That needs a store written by
// a binary whose standard root differs, and no in-tree test can build one --
// the same limit Q-025 recorded. C3 stands in for it with the one assertion a
// test CAN make: this build must still SUPPORT the pre-split digest. If the
// delivery forgets the historical row, C3 goes red. The real evidence is the
// cross-binary matrix at acceptance, not this file.
//
// ── Probe integrity ──────────────────────────────────────────────────────
//
// Reds are `#[ignore]`d and MUST be red at the baseline for the reason stated
// in each. The delivery may remove `#[ignore]` and NOTHING else in this file.
// Assertions pin behaviour, never the wording of a refusal.

use std::path::Path;
use std::process::Command;

/// The standard root this arc starts from. After the arc it is HISTORICAL --
/// O61/O62 change the standard root's contents, so its digest moves -- and it
/// must stay supported or every store written before this arc is unopenable.
const PRE_SPLIT_STANDARD_ROOT: &str =
    "65f52e2da48baa550d7340c0fdc214fd1f9925577a96ffec59bc34f8b2bcbe72";

fn oo_cmd(dir: &Path) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_oo"));
    c.current_dir(dir)
        .env("OO_IDENTITY", dir.join("identity-for-tests"))
        .env("OO_NODE_HOME", dir.join("node-home-for-tests"));
    c
}

fn oo(dir: &Path, args: &[&str]) -> String {
    let o = oo_cmd(dir).args(args).output().expect("oo runs");
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

fn scratch(tag: &str) -> nlang_interpreter::ScratchDir {
    nlang_interpreter::ScratchDir::new(&format!("halfwritten-{tag}"))
}

/// Commit one source and return the root object's address.
fn commit_root(dir: &Path, source: &str) -> String {
    std::fs::write(dir.join("a.n"), source).unwrap();
    let e = oo(dir, &["evolve", "a.n"]);
    assert!(
        !e.contains("Error"),
        "harness: evolve must succeed for {source:?}, got: {e}"
    );
    let c = oo(dir, &["commit", "-m", "x"]);
    assert!(
        c.contains("Commit successful"),
        "harness: commit must succeed for {source:?}, got: {c}"
    );
    root_addr(dir)
}

/// The address of the store's single root object.
fn root_addr(dir: &Path) -> String {
    let mut found = None;
    for a in std::fs::read_dir(dir.join(".oo/objects/sha256")).unwrap().flatten() {
        for b in std::fs::read_dir(a.path()).unwrap().flatten() {
            if std::fs::read_to_string(b.path()).unwrap_or_default().contains("\"Combo\"") {
                assert!(found.is_none(), "harness: more than one root object");
                found = Some(format!(
                    "{}{}",
                    a.file_name().to_string_lossy(),
                    b.file_name().to_string_lossy()
                ));
            }
        }
    }
    found.expect("harness: a root object")
}

// ── C0..C3 ── what must not regress ──────────────────────────────────────

/// Green at the baseline and must STAY green. Unmerging must not lose the
/// names: `~%Math` moves from a field of the root to a layer behind it, and
/// every way of reaching it must still reach it.
#[test]
fn c0_the_system_names_still_resolve_after_the_root_stops_carrying_them() {
    let d = scratch("c0");
    std::fs::write(
        d.join("r.n"),
        "a: ~%Math./add(1, 2)\nb: /add(1, 2)\nc: ~%List./len([1, 2, 3])\n",
    )
    .unwrap();
    for (k, want) in [("a", "3"), ("b", "3"), ("c", "3")] {
        let out = oo(&d, &["run", "r.n", "--observe", k]);
        assert!(
            out.trim() == want,
            "`{k}` must still resolve to {want}, got: {out}"
        );
    }
}

/// Green at the baseline and must STAY green: O62 normalises an explicit
/// `#pure` away, but ONLY after the guard. A `#pure` that is a lie must still
/// collapse -- a canonicaliser that strips the field first would destroy the
/// lie instead of catching it.
#[test]
fn c1_a_false_pure_declaration_still_collapses() {
    let d = scratch("c1");
    std::fs::write(d.join("r.n"), "out: { %effect: #pure, v: ~%Time.now _ }\n").unwrap();
    let out = oo(&d, &["run", "r.n", "--observe", "out"]);
    assert!(
        out.contains("effect_violation"),
        "a declared #pure over an #io value must still collapse, got: {out}"
    );
}

/// Green at the baseline and must STAY green. Absent operands stay opaque and
/// unknown names stay free -- this arc frees FOUR occupied coordinates, it
/// does not occupy any new ones.
#[test]
fn c2_names_outside_the_standard_root_stay_free() {
    for n in ["/sub", "@zzz", "@int"] {
        let d = scratch("c2");
        std::fs::write(d.join("a.n"), format!("{n}: {{ mine: 1 }}\n")).unwrap();
        let out = oo(&d, &["evolve", "a.n"]);
        assert!(
            !out.contains("Error"),
            "`{n}` must stay definable, got: {out}"
        );
    }
}

/// Green at the baseline and must STAY green -- and it is the most important
/// control in this arc.
///
/// O61/O62 change the standard root's contents, so its digest moves. Every
/// store written before this arc names the OLD one. v0.23.0 shipped the
/// machinery for holding more than one and it has never been used: the table
/// has exactly one row. This arc is the first time a second row is required,
/// and forgetting it makes every existing store unopenable.
#[test]
fn c3_the_engine_still_supports_the_pre_split_standard_root() {
    let oo = nlang_interpreter::Ouroboros::new_in_memory();
    assert!(
        oo.supports_standard_root(PRE_SPLIT_STANDARD_ROOT),
        "the pre-split standard root {PRE_SPLIT_STANDARD_ROOT} must stay supported; \
         without it every store written before this arc cannot be opened"
    );
}

// ── P1 ── O58: the four occupied coordinates ─────────────────────────────

/// RED at the baseline: all four standard-root coordinates a user can legally
/// write reject a new field, because each is a closed cocoon sitting on a
/// user-visible name.
///
/// This is the `/add` orphan, and the orphan was never one name.
#[test]
#[ignore = "baseline: all four answer #missing_key — measured 2026-08-16 on v0.25.0"]
fn p1_a_user_can_define_the_four_names_the_standard_root_occupies() {
    for n in ["/add", "@list", "@option", "@result"] {
        let d = scratch("p1");
        std::fs::write(d.join("a.n"), format!("{n}: {{ mine: 1 }}\n")).unwrap();
        let out = oo(&d, &["evolve", "a.n"]);
        assert!(
            !out.contains("Error"),
            "`{n}` must be definable once the standard root stops occupying it, got: {out}"
        );
    }
}

// ── P2 ── O58: the standard root becomes addressable ─────────────────────

/// RED at the baseline: the standard root has a CAID and is not an object.
/// It is the only thing in the system with an address that cannot be
/// addressed -- measured, a fresh store holds two objects and neither is it.
#[test]
#[ignore = "baseline: `CAID not found in local store` — measured 2026-08-16 on v0.25.0"]
fn p2_the_standard_root_is_an_object_you_can_ask_for() {
    let d = scratch("p2");
    commit_root(&d, "app: { k1: 1 }\n");
    let named = oo(&d, &["status"]);
    let digest = named
        .split_whitespace()
        .find(|t| t.len() == 64 && t.chars().all(|c| c.is_ascii_hexdigit()))
        .expect("presence: status must name the standard root the store depends on")
        .to_string();
    let out = oo(&d, &["inspect", &format!("hash:sha256:v1:{digest}")]);
    assert!(
        !out.contains("not found in local store"),
        "the standard root a store names must be fetchable by that name, got: {out}"
    );
}

// ── P3 ── O61: the propagated effect reaches the address ─────────────────

/// RED at the baseline, and this is the arc's cleanest demonstration.
///
/// Two programs. One types a string in. The other reads the same string out
/// of the environment. Measured: they commit to the SAME address, and the
/// stored bytes are identical -- a value that came from outside is
/// indistinguishable, by address, from one that was written down.
///
/// SPEC_08 §4.1 states its purpose in as many words: participation in the
/// CAID "ensures that a semantically impure program cannot obtain the same
/// content identity by passing itself off as #pure". It does not even have to
/// pass itself off. It simply gets it.
///
/// The red asserts a difference, so it asserts a sameness in the same run:
/// two genuinely identical pure programs must still agree on one address.
#[test]
#[ignore = "baseline: both commit to 84ad4804… — measured 2026-08-16 on v0.25.0"]
fn p3_a_value_that_came_from_outside_does_not_get_a_pure_address() {
    const VAL: &str = "written-down";
    let literal = {
        let d = scratch("p3-lit");
        commit_root(&d, &format!("app: {{ v: \"{VAL}\" }}\n"))
    };
    let twin = {
        let d = scratch("p3-twin");
        commit_root(&d, &format!("app: {{ v: \"{VAL}\" }}\n"))
    };
    assert_eq!(
        literal, twin,
        "sameness: two identical pure programs must still commit to one address"
    );

    let from_outside = {
        let d = scratch("p3-env");
        std::fs::write(d.join("a.n"), "app: { v: ~%Env./get(\"OO_PROBE_VAR\") }\n").unwrap();
        let e = oo_cmd(&d)
            .env("OO_PROBE_VAR", VAL)
            .args(["evolve", "a.n"])
            .output()
            .unwrap();
        assert!(
            e.status.success(),
            "harness: evolve must succeed, got: {}",
            String::from_utf8_lossy(&e.stderr)
        );
        let c = oo_cmd(&d)
            .env("OO_PROBE_VAR", VAL)
            .args(["commit", "-m", "x"])
            .output()
            .unwrap();
        assert!(
            String::from_utf8_lossy(&c.stdout).contains("Commit successful"),
            "harness: commit must succeed"
        );
        root_addr(&d)
    };
    assert_ne!(
        literal, from_outside,
        "a value read out of the environment must not share an address with one \
         that was typed in"
    );
}

// ── P4 ── O62: absent means pure ─────────────────────────────────────────

/// RED at the baseline: `{ %effect: #pure, v: 1 }` and `{ v: 1 }` read the
/// same through `.%effect` and commit to different addresses. O61 ruled they
/// are the same value, so REAL_03 §6.7 -- bytes are a function of the value --
/// is violated today. This is the probe that closes it.
#[test]
#[ignore = "baseline: d48f4deb… vs b25bfaf… — measured 2026-08-16 on v0.25.0"]
fn p4_writing_pure_explicitly_is_the_same_value_as_not_writing_it() {
    let a = scratch("p4a");
    let b = scratch("p4b");
    let explicit = commit_root(&a, "app: { %effect: #pure, v: 1 }\n");
    let absent = commit_root(&b, "app: { v: 1 }\n");
    assert_eq!(
        explicit, absent,
        "one value must have one address (REAL_03 §6.7)"
    );
}
