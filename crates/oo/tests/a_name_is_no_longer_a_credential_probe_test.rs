// A name is no longer a credential.
// Ruling: nlang-spec/meta/oo/STATUS.md O68 (Q1-Q5, four of five settled).
// Recon:  docs/a_name_is_the_only_credential_recon.md (§4.1 is the acceptor's).
// Order:  docs/a_name_is_no_longer_a_credential_handover.md
//
// ── What this arc is ─────────────────────────────────────────────────────
//
// Dispatch never consults the root. `lib.rs:3064-3066` reads `%builtin`
// off the value and goes straight to `builtin_registry`. Whether THIS
// universe's standard root ever projected that name is a question nobody
// asks. O68 Q3 = B: the credential comes from the context, not the value.
//
// ── What this arc is NOT ─────────────────────────────────────────────────
//
// It does NOT stop `{{ %builtin: "process.exit" }} 7` from exiting 7.
// Measured 2026-08-23 on v0.30.0: all seven dangerous names ARE projected
// by the standard root, so a root-membership gate passes them. That path
// closes under Q2a (write layer), which is unruled and sits downstream of
// arc D -- a legal alias `add: ~%Math./add` and a hand-written forgery are
// byte-identical today, so a parse-layer refusal would have false
// positives. C3 pins this on purpose. If C3 goes red, the delivery went
// out of scope; that is not extra credit.
//
// Also out of scope: Q1 (by-reference import), Q5 (effect gate), and
// removing the six dead names from the standard root -- that last one
// moves the standard-root digest and therefore every universe, so it
// rides the queued epoch (D-2/D-3/Q1), not this arc.
//
// ── What it does buy ─────────────────────────────────────────────────────
//
// Three facts that share one answer today get three named answers, and
// the root stops being a coincidence. Measured 2026-08-23:
//   registry(245) \ standard root(251) = empty   -- forging grants nothing
//   standard root \ registry            = 6      -- and the containment is
//                                                   an accident, not a rule
// Both of these give `#conflict` today and cannot be told apart:
//   {{ %builtin: "nonexistent.thing" }} (6,3)  -> #conflict
//   {{ %builtin: "math.bitAnd" }}      (6,3)  -> #conflict
// The second is a name the standard root DOES project and the engine
// cannot provide. Folding those two into one answer is the class Q-031
// closed; this is its seventh call site.
//
// ── Probe integrity ──────────────────────────────────────────────────────
//
// Reds are `#[ignore]`d and MUST be red at the baseline for the reason
// stated in each. The delivery may remove `#[ignore]` and NOTHING else in
// this file. If a pin here is wrong, say so in the report -- do not edit.
//
// Baseline measured 2026-08-23 on dev f43d22d / oo 0.30.0: 5 green, 4 red.
// Each red asserts its REACH before asserting the property. An assertion
// that only witnesses the absence of an error witnesses nothing (earned on
// the previous arc, where three red drafts were green at the baseline).

use std::path::Path;
use std::process::Command;

fn scratch(tag: &str) -> nlang_interpreter::ScratchDir {
    nlang_interpreter::ScratchDir::new(&format!("namecred-{tag}"))
}

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

/// Exit status, which is the whole point of the `process.exit` pins.
fn oo_status(dir: &Path, args: &[&str]) -> i32 {
    let mut c = Command::new(env!("CARGO_BIN_EXE_oo"));
    c.current_dir(dir)
        .env("OO_IDENTITY", dir.join("identity-for-tests"))
        .env("OO_NODE_HOME", dir.join("node-home-for-tests"));
    c.args(args).output().expect("oo runs").status.code().unwrap_or(-1)
}

/// `(<expr>).%cause` as a trimmed string. `_` means "not a bottom".
fn cause_of(dir: &Path, expr: &str) -> String {
    oo(dir, &["eval", &format!("({expr}).%cause")]).trim().to_string()
}

fn walk(p: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(p) {
        for e in rd.flatten() {
            let path = e.path();
            if path.is_dir() {
                out.extend(walk(&path));
            } else {
                out.push(path);
            }
        }
    }
    out
}

/// CAID of this universe's committed user root object -- neither the
/// standard root (a JSON string starting `"standard-root:`) nor the commit
/// (which carries `"parent"`).
fn user_root_caid(dir: &Path) -> String {
    let mut found = None;
    for e in walk(&dir.join(".oo").join("objects")) {
        let body = std::fs::read_to_string(&e).unwrap_or_default();
        if body.starts_with('"') || body.contains("\"parent\"") {
            continue;
        }
        let file = e.file_name().unwrap().to_string_lossy().to_string();
        let sub = e.parent().unwrap().file_name().unwrap().to_string_lossy().to_string();
        found = Some(format!("hash:sha256:v1:{sub}{file}"));
    }
    found.expect("a committed user root object exists")
}

// ── C1-C5: green at the baseline, must stay green ────────────────────────

#[test]
fn c1_legitimate_dispatch_still_works() {
    let d = scratch("c1");
    let got = oo(&d, &["eval", "~%Math./add (1,2)"]);
    assert!(
        got.trim() == "3",
        "the gate must not break dispatch through the standard root: {got:?}"
    );
}

#[test]
fn c2_a_second_module_still_dispatches() {
    let d = scratch("c2");
    let got = oo(&d, &["eval", "~%Math./gt (2,1)"]);
    assert!(
        got.trim() == "#true",
        "`math.gt` is registered and projected; it must keep dispatching: {got:?}"
    );
}

#[test]
fn c3_the_forgery_still_exits_and_that_is_the_scope_line() {
    // NOT a defect being preserved -- a scope line being pinned. All seven
    // dangerous names are in the standard root, so a root-membership gate
    // passes them by construction. Closing this needs Q2a, which is
    // unruled. If this goes red, the delivery overreached.
    let d = scratch("c3");
    let code = oo_status(
        &d,
        &["eval", r#"{{ %builtin: "process.exit", %morphism: #true }} 7"#],
    );
    assert_eq!(
        code, 7,
        "this arc does not close the write-layer path (Q2a); see the order §0"
    );
}

#[test]
fn c4_no_builtin_key_is_still_a_conflict() {
    let d = scratch("c4");
    let got = oo(&d, &["eval", "{{ %kind: #x }} 7"]);
    assert!(
        got.contains("#conflict"),
        "control: applying a non-morphism cocoon is #conflict: {got:?}"
    );
}

#[test]
fn c5_a_universe_holding_a_forgery_stays_readable_and_writable() {
    // O68 Q4 = C: the read path never refuses. The mark S4 asks for must
    // not become a gate.
    let d = scratch("c5");
    std::fs::write(
        d.join("a.n"),
        "boom: {{ %builtin: \"process.exit\", %morphism: #true }}\nv: 1\n",
    )
    .unwrap();
    oo(&d, &["evolve", "a.n"]);
    let first = oo(&d, &["commit", "-m", "forged"]);
    assert!(first.contains("hash:"), "LIVENESS: first commit: {first:?}");

    let caid = user_root_caid(&d);
    let seen = oo(&d, &["inspect", &caid]);
    assert!(
        seen.contains("process.exit"),
        "the read path must still open it: {seen:?}"
    );

    // A refinement, not a contradiction: `v: 1` -> `v: 2` is not monotone
    // and `evolve` correctly declines it, which would prove nothing here.
    std::fs::write(
        d.join("b.n"),
        "boom: {{ %builtin: \"process.exit\", %morphism: #true }}\nv: 1\nw: 2\n",
    )
    .unwrap();
    oo(&d, &["evolve", "b.n"]);
    let second = oo(&d, &["commit", "-m", "again"]);
    assert!(
        second.contains("hash:"),
        "a universe holding a forgery must remain writable: {second:?}"
    );
}

// ── R1-R4: red at the baseline ───────────────────────────────────────────

#[test]
#[ignore = "baseline: an invented name and a dead name share `#conflict`"]
fn r1_an_invented_name_gets_its_own_cause() {
    let d = scratch("r1");
    let cause = cause_of(&d, r#"{{ %builtin: "nonexistent.thing", %morphism: #true }} (6,3)"#);
    // REACH: it must be a bottom at all. `_` would mean the dispatch site
    // was never entered and the rest of this test would prove nothing.
    assert!(
        cause.starts_with('#'),
        "REACH: dispatch must produce a bottom with a cause, got {cause:?}"
    );
    assert_ne!(
        cause, "#conflict",
        "a name the standard root does not project must say so by name"
    );
}

#[test]
#[ignore = "baseline: `math.bitAnd` falls over as `#conflict`, same as an invented name"]
fn r2_a_dead_name_gets_its_own_cause() {
    // `math.bitAnd` IS projected by the standard root and is NOT in the
    // registry. It passes the gate and then falls over one layer down.
    let d = scratch("r2");
    let cause = cause_of(&d, r#"{{ %builtin: "math.bitAnd", %morphism: #true }} (6,3)"#);
    assert!(
        cause.starts_with('#'),
        "REACH: dispatch must produce a bottom with a cause, got {cause:?}"
    );
    assert_ne!(
        cause, "#conflict",
        "the root promised this name and the engine cannot provide it -- say that"
    );
}

#[test]
#[ignore = "baseline: both are `#conflict`, so they compare equal"]
fn r3_the_two_are_told_apart() {
    // The discrimination IS the claim. R1 and R2 can each be satisfied by
    // one new cause used for both; this one cannot.
    let d = scratch("r3");
    let invented = cause_of(&d, r#"{{ %builtin: "nonexistent.thing", %morphism: #true }} (6,3)"#);
    let dead = cause_of(&d, r#"{{ %builtin: "math.bitAnd", %morphism: #true }} (6,3)"#);
    assert!(
        invented.starts_with('#') && dead.starts_with('#'),
        "REACH: both must be bottoms, got {invented:?} and {dead:?}"
    );
    assert_ne!(
        invented, dead,
        "`the root never named it` and `the root named it, I cannot provide it` \
         are two different facts and must not share one answer (Q-031's class)"
    );
}

#[test]
#[ignore = "baseline: `oo inspect` renders a forged %builtin exactly like a real one"]
fn r4_a_user_written_builtin_is_distinguishable() {
    // O68 Q4.C's attached duty: leave a QUERYABLE mark. This pins the
    // property, not the spelling -- the delivery picks how to say it and
    // documents it in the report. What is pinned: a universe carrying a
    // user-authored `%builtin` must be distinguishable, through some
    // queryable surface, from one that carries none.
    let forged = scratch("r4a");
    std::fs::write(
        forged.join("a.n"),
        "boom: {{ %builtin: \"process.exit\", %morphism: #true }}\nv: 1\n",
    )
    .unwrap();
    oo(&forged, &["evolve", "a.n"]);
    let c1 = oo(&forged, &["commit", "-m", "forged"]);
    assert!(c1.contains("hash:"), "REACH: forged universe must commit: {c1:?}");

    let clean = scratch("r4b");
    std::fs::write(clean.join("a.n"), "boom: {{ %kind: #x }}\nv: 1\n").unwrap();
    oo(&clean, &["evolve", "a.n"]);
    let c2 = oo(&clean, &["commit", "-m", "clean"]);
    assert!(c2.contains("hash:"), "REACH: control universe must commit: {c2:?}");

    // The forged one must be reported as carrying user-authored `%builtin`
    // somewhere the control one is not. Today the two `inspect` outputs
    // differ only by the value itself, which is not a mark -- it is the
    // thing a mark would be about.
    let f = oo(&forged, &["inspect", &user_root_caid(&forged)]);
    let c = oo(&clean, &["inspect", &user_root_caid(&clean)]);
    let marked = |s: &str| {
        let s = s.to_lowercase();
        s.contains("user-authored") || s.contains("user_builtin") || s.contains("unattested")
    };
    assert!(
        marked(&f) && !marked(&c),
        "a queryable mark must exist on the forged universe and not on the control.\n\
         forged: {f:?}\n clean: {c:?}\n\
         If the delivery reports it somewhere other than `inspect`, say so and \
         the acceptor will recalibrate this probe -- do not edit it."
    );
}
