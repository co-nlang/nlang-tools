// A success that was a bottom.
// Rulings: nlang-spec/meta/oo/STATUS.md D56, D57 (and B1, commit.md 2.1.3,
//          which they finish)
// Recon:   nlang-tools/docs/a_success_that_was_a_bottom_recon.md
// Order:   nlang-tools/docs/a_success_that_was_a_bottom_handover.md
//
// -- What this arc is ----------------------------------------------------
//
// One writer, three commands, and the operator is never told:
//
//     c: c + 1
//     oo evolve   -> no output, rc 0
//     oo status   -> prints `c: c + 1` back, looking intact
//     oo commit   -> "Commit successful", rc 0
//     the root    -> c: _|_ (%cause: #divergent)
//
// SPEC_10 4.1.2 already carries the MUST this breaks: the engine must name
// the coordinates a bottom landed on rather than reporting success, and
// its own reason sentence is that silently fixing a contradiction makes
// the operator decide without knowing. That clause is from 2026-08-06 and
// has never been implemented.
//
// -- Why it was thought out of scope, and why it is not (D56) ------------
//
// 4.1.2 scopes itself to automatic convergence and excludes "explicitly
// fixing content that contains a bottom". The ruling is that "explicit"
// is not a state of the world -- it is a claim about the operator's
// intent, and that claim is only true if they were told. The only thing
// that can tell them is the reporting MUST in the same section. So the
// exclusion cannot be entered without first satisfying the clause it
// excludes, and today's behaviour is not "out of scope" but a state that
// should not have been reachable.
//
// -- Why the mark goes on the commit (D57) ------------------------------
//
// Measured: the same bottom-bearing content committed in two separate
// repositories gives a byte-identical root, cabe2ee25f29cc9a... Consent is
// not a value, so it cannot be in the root, so "landed silently" and "was
// reported and the operator proceeded" are the same record. You cannot
// infer consent from the outcome -- the information is genuinely not
// there. Record it separately or lose it.
//
// It is AUDIT, not AUTHORIZATION. Committing a bottom is not forbidden
// (2.3.1 says a bottom MUST be written with its cause), so nothing needs
// permitting: no grant, no flag. SPEC_08 6.2 forbids marking on the mere
// presence of a capability -- "the mark must reflect the fact, not the
// flag". And 6.2 plus REAL_01 7.3 put audit on the Commit rather than in
// a workspace file, because a workspace file sits in the assertion layer
// and offers nothing against an adversary who can write it. A savepoint
// is a workspace file.
//
// -- Out of scope, do not touch ------------------------------------------
//
//   * The `message` on a bottom being Rust Debug output, and not being
//     covered by its own CAID. In the Inbox, interrupt-candidate, service
//     plane. The report here must NOT copy it -- name the coordinate and
//     the TAG_REGISTRY cause, nothing else.
//   * `oo status` showing an unreduced thunk where the root will hold a
//     bottom. Real, but it is an evaluation-depth question, not a
//     reporting one, and 2.2.1's clauses do not reach it (its formula
//     never fires: two combos meet to a combo). Goes to the Inbox.
//   * The B path (a workset-level bottom from the parallel fold) which
//     aborts at rc 1. That is 2.2.2's own product promise; leave it.
//   * Q-016, Q-018.
//
// -- Probe integrity ------------------------------------------------------
//
// Reds are `#[ignore]`d and MUST be red at the baseline for the reason
// stated. The delivery may remove `#[ignore]` and NOTHING else in this
// file -- `rustfmt` included.
//
// R1 and R2 are spelling-agnostic. R1 asks that the commit output name
// the coordinate and the cause; it does not pin the sentence. R2 asks
// that the commit object carry something the clean commit does not; it
// does not pin the field name.
//
// NOT PROBED, stated so no one mistakes silence for coverage:
//   * That the operator CONSENTED. A one-shot CLI cannot ask, so what is
//     actually true is "the coordinates were reported and the commit
//     proceeded". Per 6.2 the mark must not claim more than happened, so
//     the order requires the delivery to say in writing what its mark
//     asserts. No assertion here can check a claim's wording.

use std::path::Path;
use std::process::Command;

fn oo(dir: &Path, args: &[&str]) -> (String, i32) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_oo"));
    c.current_dir(dir)
        .env("OO_IDENTITY", dir.join("identity-for-tests"))
        .env("OO_NODE_HOME", dir.join("node-home-for-tests"));
    let o = c.args(args).output().expect("oo runs");
    (
        format!(
            "{}{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        ),
        o.status.code().unwrap_or(-1),
    )
}

fn scratch(tag: &str) -> nlang_interpreter::ScratchDir {
    nlang_interpreter::ScratchDir::new(&format!("bottom-{tag}"))
}

fn write(d: &Path, name: &str, body: &str) {
    std::fs::write(d.join(name), body).expect("write source");
}

fn head_digest(d: &Path) -> String {
    let raw = std::fs::read_to_string(d.join(".oo").join("HEAD")).expect("HEAD exists");
    raw.trim().rsplit(':').next().unwrap_or("").to_string()
}

fn object_text(d: &Path, digest: &str) -> String {
    let (a, b) = digest.split_at(2);
    std::fs::read_to_string(d.join(".oo").join("objects").join("sha256").join(a).join(b))
        .unwrap_or_default()
}

/// Stage `src`, commit it, and hand back the commit's output and exit code.
fn evolve_and_commit(d: &Path, src: &str) -> (String, i32) {
    write(d, "c.n", src);
    let (_, rc) = oo(d, &["evolve", "c.n"]);
    assert_eq!(rc, 0, "REACH: evolve should succeed for {src:?}");
    oo(d, &["commit", "-m", "x"])
}

// ---------------------------------------------------------------- R1 ----
//
// RED at the baseline. Measured 2026-08-31 against the v0.41.0 tag build:
// the entire output of `oo commit` for `c: c + 1` is one line,
// "Commit successful: hash:sha256:v1:fc879575...", rc 0. It contains
// neither the coordinate `c` nor the cause `#divergent`, while the root
// it just wrote holds `c: _|_ (%cause: #divergent)`.
//
// SPEC_10 4.1.2, reporting content (MUST): the engine must say which
// coordinates the bottom landed on, and not merely report success.

#[test]
fn r1_a_commit_that_writes_a_bottom_says_where_it_landed() {
    let s = scratch("r1");
    let d = s.path();
    let (out, _) = evolve_and_commit(d, "c: c + 1\n");

    assert!(
        out.contains('c'),
        "the commit must name the coordinate the bottom landed on; it said:\n{out}"
    );
    assert!(
        out.contains("#divergent"),
        "the commit must name the TAG_REGISTRY cause (#divergent), not just \
         report success; it said:\n{out}"
    );
    assert!(
        !out.contains("EffectTag"),
        "the report must not copy the host-language debug text out of the \
         bottom's message (SPEC_10 2.2.1, and the Inbox row on that field); \
         it said:\n{out}"
    );
}

// ---------------------------------------------------------------- R2 ----
//
// RED at the baseline: nothing on the commit distinguishes "a bottom was
// reported and this commit went ahead anyway" from an ordinary commit.
// Measured: the same bottom-bearing content in two separate repositories
// produces one identical root, so the outcome cannot carry the fact.
//
// Asserted as "the bottom-bearing commit's object holds something the
// clean one does not", which pins the audit fact without pinning the
// field's name or spelling.

#[test]
fn r2_the_commit_records_that_it_was_reported() {
    let s = scratch("r2");
    let d = s.path();

    let clean = scratch("r2-clean");
    let cd = clean.path();
    let (_, rc) = evolve_and_commit(cd, "ok: 1\n");
    assert_eq!(rc, 0, "REACH: a clean commit should succeed");
    let clean_obj = object_text(cd, &head_digest(cd));
    assert!(!clean_obj.is_empty(), "REACH: clean commit object readable");

    let (_, _) = evolve_and_commit(d, "c: c + 1\n");
    let marked = object_text(d, &head_digest(d));
    assert!(!marked.is_empty(), "REACH: bottom commit object readable");

    let clean_keys: std::collections::BTreeSet<&str> =
        clean_obj.split_whitespace().collect();
    let extra: Vec<&str> = marked
        .split_whitespace()
        .filter(|t| !clean_keys.contains(t) && t.contains(':'))
        .collect();

    assert!(
        !extra.is_empty(),
        "a commit that wrote a bottom must carry an audit fact a clean \
         commit does not (D57). The two objects differ in nothing \
         structural:\nclean:  {clean_obj}\nmarked: {marked}"
    );
}

// ---------------------------------------------------------------- G1 ----
//
// GREEN and the red line. D57's whole cost argument is that the field is
// optional and absent on ordinary commits, so no existing digest moves.
// If this goes red, the mark is being written unconditionally and every
// clean commit in the world just changed address.

#[test]
fn g1_a_clean_commit_keeps_its_identity() {
    let s = scratch("g1");
    let d = s.path();
    let (_, rc) = evolve_and_commit(d, "x: 0\n");
    assert_eq!(rc, 0, "REACH: the commit failed");

    let (out, _) = oo(d, &["inspect", &format!("hash:sha256:v1:{}", head_digest(d))]);
    assert!(
        out.contains("31745ef0e8bfde3d8a2673b7dce5bb5cd74f3a7f2cc6f5422aa043c8dce5589a"),
        "`x: 0` must still root at 31745ef0...; inspect said:\n{out}"
    );

    let mut n = 0;
    let mut stack = vec![d.join(".oo").join("objects")];
    while let Some(p) = stack.pop() {
        if let Ok(rd) = std::fs::read_dir(&p) {
            for e in rd.flatten() {
                if e.path().is_dir() {
                    stack.push(e.path());
                } else {
                    n += 1;
                }
            }
        }
    }
    assert_eq!(n, 3, "a solid `x: 0` universe is three objects");
}

// ---------------------------------------------------------------- G2 ----
//
// GREEN and must stay green. B1, and D56 after it, say the fix is the
// REPORT, not an abort. The obvious way to make R1 pass is to refuse the
// commit, and that is precisely the half 4.1.2 dismantled: whether to fix
// a contradiction into history belongs to the operator, not the clause.
//
// So: a commit carrying a bottom still lands, and still exits zero. The
// commit did not fail; the operator was told.

#[test]
fn g2_a_reported_bottom_still_lands() {
    let s = scratch("g2");
    let d = s.path();
    let (out, rc) = evolve_and_commit(d, "c: c + 1\n");
    assert_eq!(
        rc, 0,
        "reporting is not aborting (B1 / D56). The commit must still \
         succeed; it said:\n{out}"
    );
    assert!(
        d.join(".oo").join("HEAD").exists(),
        "HEAD must exist: the commit landed"
    );

    let (root, _) = oo(d, &["inspect", &format!("hash:sha256:v1:{}", head_digest(d))]);
    assert!(
        root.contains("root:"),
        "the commit must have a root; inspect said:\n{root}"
    );
}

// ---------------------------------------------------------------- G3 ----
//
// GREEN and must stay green. The B path -- a workset-level bottom out of
// the parallel fold -- aborts at rc 1 with a named coordinate today, and
// SPEC_10 2.2.2 promises exactly that ("the commit must fail", "the exit
// code must not be zero"). This arc changes the A path only.
//
// Seeded rather than raced: two injection files with conflicting values
// for one key, so the fold has to reconcile them. No scheduler is pinned.

#[test]
fn g3_the_fold_path_still_refuses() {
    let s = scratch("g3");
    let d = s.path();
    write(d, "a.n", "k: 1\n");
    let (_, rc) = oo(d, &["evolve", "a.n"]);
    assert_eq!(rc, 0, "REACH: first injection failed");

    let inj = d.join(".oo").join("injections");
    let first = std::fs::read_dir(&inj)
        .expect("REACH: injections dir")
        .flatten()
        .map(|e| e.path())
        .find(|p| p.is_file())
        .expect("REACH: one injection on disk");
    let body = std::fs::read_to_string(&first).expect("REACH: injection readable");
    let conflicting = body.replace("k: 1", "k: 2");
    assert_ne!(conflicting, body, "REACH: the seed edit did nothing");
    std::fs::write(inj.join("ffffffffffffffffffffffffffffffff"), conflicting)
        .expect("REACH: seed the second injection");

    let (out, rc) = oo(d, &["commit", "-m", "x"]);
    assert_ne!(
        rc, 0,
        "SPEC_10 2.2.2: a workset that folded to a bottom must fail the \
         commit with a non-zero exit code; it said:\n{out}"
    );
    assert!(
        out.contains('k'),
        "and it must name the coordinate; it said:\n{out}"
    );
}
