// #rollback + #squash — the history-chain privileged operations (2026-07-26,
// pre-committed by work order: docs/history_ops_handover.md).
// SPEC_08 §6.2, the remaining operation bodies.
//
// Unlike #pin these do NOT touch the lattice: they move HEAD and rewrite the
// commit chain. Both are SINGLE commands (not split across evolve→commit), so
// the "intent is not authority" trap the #pin arc fell into does not arise for
// the operation itself — the capability is presented in the same process that
// applies the effect.
//
// ── The abandonment record (ruling, 2026-07-26, user) ────────────────────
// SPEC_08 §6.2 gives #rollback no audit tag, "tracked by the history chain".
// Measured: that does not hold in n/. After a rollback `oo log` walks parents
// from the new HEAD, so the abandoned segment leaves the history entirely —
// the objects survive in the store but nothing can enumerate them. git can say
// this because it has a reflog; n/ has none, and in n/ the chain matters more
// than in git (git keeps the trail local and pushes a clean result; here the
// chain IS the record).
//
// So: rollback itself creates no commit (it only moves the pointer), but the
// NEXT commit records the head that was abandoned — in commit metadata, never
// in a value, so no CAID moves (§6.2 fingerprint guarantee). That is the
// moment the divergence actually enters the chain.
//
// This gives the history graph a SECOND edge kind: parent (solid, convergence
// lineage) and abandoned (dashed, a divergence marker). It stays a DAG — no
// cycles, no merge ambiguity — and the extra edge is real structure that git
// hides rather than complexity invented here. In n/'s own terms it is the
// n/^op up-arrow made visible (discussion/021: rollback is anti-monotone).
//
// ── Hiding, and why #squash is n/'s GC ───────────────────────────────────
// RULING (user): #squash MAY compress over an abandonment record; its own
// `#privileged_squash` marker carries the fact forward. So granularity is
// losable but the FACT is not: you can remove content, you cannot remove that
// removal happened. Squashing a squash is still marked.
//
// This is also why squash must drop BOTH parent and abandoned edges in its
// range. Abandoned commits are permanently reachable via the new edge, so a
// reachability GC would never collect them — #squash is the ONLY operation
// that can make anything unreachable, and it is privileged and audited.
// Reclaiming the bytes afterwards is then a mechanical, unprivileged sweep.
// (No GC exists today; the store is append-only. Out of scope here — this arc
// only has to make unreachability POSSIBLE.)
//
// MEASURED (baseline, v0.2.40): `oo rollback` and `oo squash` are unrecognized
// subcommands; the capability slots `rollback`/`squash` already parse via
// `--grant` (declared inert in the selective-discharge arc).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

fn fresh_dir() -> PathBuf {
    let mut d = std::env::temp_dir();
    d.push(format!(
        "nlang-hist-{}-{}",
        std::process::id(),
        DIR_SEQ.fetch_add(1, Ordering::SeqCst)
    ));
    fs::remove_dir_all(&d).ok();
    fs::create_dir_all(&d).unwrap();
    d
}

fn oo(dir: &Path, args: &[&str]) -> String {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_oo"));
    for a in args {
        cmd.arg(a);
    }
    let out = cmd.current_dir(dir).output().unwrap();
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout).trim(),
        String::from_utf8_lossy(&out.stderr).trim()
    )
}

fn write(dir: &Path, name: &str, src: &str) {
    fs::write(dir.join(name), src).unwrap();
}

/// Commits `f1: 1`, `f2: 2`, `f3: 3` as three commits. Returns their CAIDs
/// oldest-first.
fn three_commit_history(dir: &Path) -> Vec<String> {
    for i in 1..=3 {
        write(dir, "s.n", &format!("f{i}: {i}\n"));
        oo(dir, &["evolve", "s.n"]);
        oo(dir, &["commit", "-m", &format!("c{i}")]);
    }
    let mut caids = log_caids(dir);
    caids.reverse(); // log is newest-first
    caids
}

/// CAIDs in `oo log` order (newest first).
fn log_caids(dir: &Path) -> Vec<String> {
    oo(dir, &["log"])
        .lines()
        .filter_map(|l| l.strip_prefix("commit ").map(|s| s.trim().to_string()))
        .collect()
}

/// Whether the store still holds the object for `caid` (abandoned commits must
/// survive — rollback drops reachability, never bytes).
fn object_exists(dir: &Path, caid: &str) -> bool {
    let hex = caid.rsplit(':').next().unwrap();
    dir.join(".oo")
        .join("objects")
        .join("sha256")
        .join(&hex[0..2])
        .join(&hex[2..])
        .exists()
}

fn head(dir: &Path) -> String {
    fs::read_to_string(dir.join(".oo").join("HEAD")).unwrap_or_default()
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — #rollback
// ─────────────────────────────────────────────────────────────────────────

#[test]
#[ignore]
fn red_rollback_moves_head() {
    // The operation: HEAD lands on the named commit and the log tip follows.
    let d = fresh_dir();
    let c = three_commit_history(&d);
    let got = oo(&d, &["rollback", &c[0], "--grant", "rollback"]);
    assert!(!got.contains("error"), "rollback must succeed: {got:?}");
    assert_eq!(
        log_caids(&d).first().map(String::as_str),
        Some(c[0].as_str()),
        "HEAD must sit on the rollback target"
    );
}

#[test]
#[ignore]
fn red_rollback_requires_the_capability() {
    let d = fresh_dir();
    let c = three_commit_history(&d);
    let before = head(&d);
    let got = oo(&d, &["rollback", &c[0]]);
    assert!(
        got.contains("privileged_required") || got.contains("privilege"),
        "rollback without the capability must be refused: {got:?}"
    );
    assert_eq!(head(&d), before, "a refused rollback must not move HEAD");
}

#[test]
#[ignore]
fn red_rollback_capability_is_operation_specific() {
    // Axis-1: a different §6.2 capability must not authorize it.
    let d = fresh_dir();
    let c = three_commit_history(&d);
    let before = head(&d);
    let got = oo(&d, &["rollback", &c[0], "--grant", "pin"]);
    assert!(
        got.contains("privileged_required") || got.contains("privilege"),
        "pin must not authorize #rollback: {got:?}"
    );
    assert_eq!(head(&d), before);
}

#[test]
#[ignore]
fn red_rollback_is_recorded_in_the_next_commit() {
    // THE ruling. Rollback creates no commit, but the divergence must enter
    // the chain when work resumes: the next commit names the abandoned head.
    let d = fresh_dir();
    let c = three_commit_history(&d);
    let abandoned = c[2].clone(); // the tip we are leaving behind
    oo(&d, &["rollback", &c[0], "--grant", "rollback"]);
    write(&d, "n.n", "later: 9\n");
    oo(&d, &["evolve", "n.n"]);
    oo(&d, &["commit", "-m", "resumed"]);
    let log = oo(&d, &["log"]);
    // The abandoned tip must appear as a RECORD, not as a chain member — the
    // distinction is the whole point, and asserting only "the log mentions it"
    // passes vacuously when the rollback never happened (it is then still the
    // tip). Calibration caught exactly that.
    assert!(
        !log_caids(&d).contains(&abandoned),
        "precondition: the abandoned tip must have left the parent chain"
    );
    assert!(
        log.contains(&abandoned),
        "the next commit must record the abandoned head: {log:?}"
    );
}

#[test]
#[ignore]
fn red_abandoned_commits_are_not_deleted() {
    // Rollback drops REACHABILITY, never bytes. Only #squash may make
    // anything unreachable, and even then collection is a separate sweep.
    let d = fresh_dir();
    let c = three_commit_history(&d);
    oo(&d, &["rollback", &c[0], "--grant", "rollback"]);
    // Precondition first: without it, "the object still exists" is trivially
    // true whenever the rollback did not happen at all.
    assert_eq!(
        log_caids(&d).first().map(String::as_str),
        Some(c[0].as_str()),
        "precondition: the rollback must have happened"
    );
    assert!(
        object_exists(&d, &c[2]),
        "the abandoned commit object must survive a rollback"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — #squash
// ─────────────────────────────────────────────────────────────────────────

#[test]
#[ignore]
fn red_squash_compresses_the_range() {
    // `oo squash <base>`: everything after <base> up to HEAD becomes ONE
    // commit whose parent is <base>. Three commits onto the first ⟹ two.
    let d = fresh_dir();
    let c = three_commit_history(&d);
    let got = oo(&d, &["squash", &c[0], "--grant", "squash"]);
    assert!(!got.contains("error"), "squash must succeed: {got:?}");
    let after = log_caids(&d);
    assert_eq!(after.len(), 2, "range must collapse to one commit over the base");
    assert_eq!(
        after.last().map(String::as_str),
        Some(c[0].as_str()),
        "the base must remain the parent"
    );
}

#[test]
#[ignore]
fn red_squash_requires_the_capability() {
    let d = fresh_dir();
    let c = three_commit_history(&d);
    let before = head(&d);
    let got = oo(&d, &["squash", &c[0]]);
    assert!(
        got.contains("privileged_required") || got.contains("privilege"),
        "squash without the capability must be refused: {got:?}"
    );
    assert_eq!(head(&d), before, "a refused squash must not rewrite the chain");
}

#[test]
#[ignore]
fn red_squash_result_is_marked() {
    // §6.2 audit tag #privileged_squash — the compression must be visible.
    let d = fresh_dir();
    let c = three_commit_history(&d);
    oo(&d, &["squash", &c[0], "--grant", "squash"]);
    assert!(
        oo(&d, &["log"]).to_lowercase().contains("squash"),
        "the squashed commit must be marked in the history"
    );
}

#[test]
#[ignore]
fn red_squash_preserves_the_universe() {
    // §6.2 fingerprint face: compressing HISTORY must not change what the
    // universe IS. f1 was bound to 1; after squashing, rebinding it to 2 must
    // still conflict exactly as before.
    let d = fresh_dir();
    let c = three_commit_history(&d);
    oo(&d, &["squash", &c[0], "--grant", "squash"]);
    // Precondition first — "the universe is unchanged" is trivially true when
    // no squash occurred.
    assert_eq!(
        log_caids(&d).len(),
        2,
        "precondition: the squash must have happened"
    );
    write(&d, "z.n", "f1: 2\n");
    assert!(
        oo(&d, &["evolve", "z.n"]).contains("Evolution Conflict"),
        "squash must not alter the committed universe"
    );
}

#[test]
#[ignore]
fn red_squash_over_an_abandonment_keeps_the_fact() {
    // THE ruling's other half. A squash MAY compress away an abandonment
    // record — granularity is losable — but its own marker carries the fact
    // forward: the chain still says a privileged removal happened here.
    let d = fresh_dir();
    let c = three_commit_history(&d);
    oo(&d, &["rollback", &c[0], "--grant", "rollback"]);
    write(&d, "n.n", "later: 9\n");
    oo(&d, &["evolve", "n.n"]);
    oo(&d, &["commit", "-m", "resumed"]); // carries the abandonment record
    oo(&d, &["squash", &c[0], "--grant", "squash"]);
    let log = oo(&d, &["log"]);
    assert!(
        log.to_lowercase().contains("squash"),
        "the removal itself must remain visible: {log:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// ACTIVE PINS — everything not privileged is untouched
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn pin_ordinary_history_is_unmarked() {
    // Markers must be specific to privileged operations.
    let d = fresh_dir();
    three_commit_history(&d);
    let log = oo(&d, &["log"]).to_lowercase();
    assert!(!log.contains("squash") && !log.contains("abandon"));
}

#[test]
fn pin_ordinary_log_walks_the_whole_chain() {
    let d = fresh_dir();
    let c = three_commit_history(&d);
    assert_eq!(log_caids(&d).len(), 3);
    assert_eq!(log_caids(&d).last().map(String::as_str), Some(c[0].as_str()));
}

#[test]
fn pin_commit_and_evolve_unaffected() {
    // The ordinary evolve/commit path must not shift.
    let d = fresh_dir();
    three_commit_history(&d);
    write(&d, "e.n", "brand_new: 7\n");
    assert!(!oo(&d, &["evolve", "e.n"]).contains("Evolution Conflict"));
    assert!(oo(&d, &["commit", "-m", "more"]).contains("Commit successful"));
}

#[test]
fn pin_history_capabilities_do_not_authorize_discharge() {
    // Carried forward: the §6.2 capabilities stay mutually distinct.
    let d = fresh_dir();
    write(&d, "c.n", "out: ~%Effect./runPure 42\n");
    for grant in ["rollback", "squash"] {
        let got = oo(&d, &["run", "c.n", "--grant", grant, "--observe", "out"]);
        assert!(
            got.contains("privileged_required"),
            "granting {grant} must not authorize discharge: {got:?}"
        );
    }
}

#[test]
fn pin_effect_override_does_not_authorize_history_ops() {
    // And the converse direction.
    let d = fresh_dir();
    let c = three_commit_history(&d);
    let before = head(&d);
    oo(&d, &["rollback", &c[0], "--grant", "effect_override"]);
    oo(&d, &["squash", &c[0], "--grant", "effect_override"]);
    assert_eq!(
        head(&d),
        before,
        "effect_override must authorize neither history operation"
    );
}
