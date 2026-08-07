// A detection that changes nothing is not a detection (2026-08-07,
// pre-committed by work order: docs/verdict_must_gate_handover.md).
//
// ── What is on the floor ─────────────────────────────────────────────────
//
// Measured on v0.11.0 (`dev b5f39bc`), a workspace with three commits, six
// objects, and the middle commit's bytes overwritten with `not json at all`:
//
//     oo gc: 6 objects, 3 reachable, 3 collectable (508123 bytes)
//     integrity #object_undecodable: reachable digest b1c9af9f… cannot be decoded
//     oo gc: removed 3 objects, freed 508123 bytes
//     exit=0
//
// It says it cannot decode a *reachable* object, and then deletes three
// objects and exits zero. Nothing connects those two lines. Afterwards
// `oo log` cannot walk the history at all — the store is not merely missing
// a decoration, it is missing the rest of itself.
//
// The `continue` in `gc.rs` means *treat it as a leaf*. The object itself is
// already in `seen` (inserted at gc.rs:101, before the read at :102 and the
// decode at :108), so the object survives — the `GcReport.integrity` doc
// comment "reported, never swept" is true of it. What it is not true of is
// everything reachable only through it.
//
// ── The quieter half ─────────────────────────────────────────────────────
//
// Same workspace, but the middle commit's bytes replaced with **another
// valid object's bytes** — the oldest commit's:
//
//     oo gc: 6 objects, 4 reachable, 2 collectable (255537 bytes)
//     oo gc: removed 2 objects, freed 255537 bytes
//     exit=0
//     occurrences of "caid_mismatch" in the output: 0
//
// No integrity line at all. The walk followed the tamperer's references
// instead of the real ones and swept what the real ones pointed at. This is
// `storage.rs:209 read_raw_digest` handing back bytes without recomputing the
// address, where `get_value` (storage.rs:224) does recompute and returns
// `CaidMismatch`. Two read paths, one of them exempt.
//
// ── Why this needs no ruling ─────────────────────────────────────────────
//
// REAL_03 §6.6 already says all three parts, since 2026-07-26/29:
//
//   重算義務 (MUST)            — every path that fetches by CAID must
//                                recompute the address after decoding.
//   消費端不得丟棄裁決 (MUST)   — a detection silently dropped by the caller
//                                is equivalent to no detection; only
//                                absent/opaque may continue, `#caid_mismatch`
//                                and `#object_undecodable` must abort and
//                                report.
//   裁決必須為真 (MUST)        — and do not emit `#object_undecodable`
//                                because you picked the wrong decoder. The
//                                store holds Commits as well as values; that
//                                is the v0.2.52 `oo inspect` precedent.
//
// So this arc is conformance work. P1 and P3 are the two clauses that cut
// *against* the fix, and they are green today: absence must stay survivable,
// and a healthy store must stay unaccused.
//
// ── The trap in measuring it ─────────────────────────────────────────────
//
// `local_gc_probe_test.rs:133 refs_of` is a test-side walker, and it shares
// the engine's blind spot exactly — it is also a JSON parse. Using it to
// compute the expected survivor set *after* tampering would be a blind man
// checking a blind man. Every expectation below is therefore computed
// **before** the bytes are touched.
//
// ── The write half ───────────────────────────────────────────────────────
//
// v0.11.0 routed durable writes through `storage::atomic_write`. Three sites
// did not get the memo, and two of them fail in a way v0.11.0 was written to
// abolish: `path.with_extension("…tmp")` is the *same name for every
// process*, so two concurrent writers interleave in one temp file and then
// one of them renames the interleaving into place. R5/R6 squat that name
// with a directory, which turns a race into an assertion.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use nlang_interpreter::discovery_config::DiscoveryConfig;
use nlang_interpreter::oodp::AffiliationClaim;
use nlang_interpreter::peers::{self, PeerDirectoryState};

// ── harness ─────────────────────────────────────────────────────────────

fn fresh_dir(tag: &str) -> nlang_interpreter::ScratchDir {
    nlang_interpreter::ScratchDir::new(&format!("verdict-{tag}"))
}

fn oo_raw(dir: &Path, args: &[&str]) -> (String, bool) {
    let out = Command::new(env!("CARGO_BIN_EXE_oo"))
        .args(args)
        .current_dir(dir)
        .env("OO_IDENTITY", dir.join("identity-for-tests"))
        .env("OO_NODE_HOME", dir.join("node-home-for-tests"))
        .output()
        .unwrap();
    (
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout).trim(),
            String::from_utf8_lossy(&out.stderr).trim()
        ),
        out.status.success(),
    )
}

fn oo(dir: &Path, args: &[&str]) -> String {
    oo_raw(dir, args).0
}

fn objects_dir(dir: &Path) -> PathBuf {
    dir.join(".oo").join("objects").join("sha256")
}

/// Every object in the store: digest → byte length.
fn store_map(dir: &Path) -> BTreeMap<String, u64> {
    let mut out = BTreeMap::new();
    let Ok(top) = fs::read_dir(objects_dir(dir)) else {
        return out;
    };
    for a in top.flatten() {
        if !a.path().is_dir() {
            continue;
        }
        let pre = a.file_name().to_string_lossy().to_string();
        for b in fs::read_dir(a.path()).unwrap().flatten() {
            let rest = b.file_name().to_string_lossy().to_string();
            out.insert(format!("{pre}{rest}"), b.metadata().unwrap().len());
        }
    }
    out
}

fn digests(dir: &Path) -> BTreeSet<String> {
    store_map(dir).into_keys().collect()
}

fn object_path(dir: &Path, digest: &str) -> PathBuf {
    objects_dir(dir).join(&digest[..2]).join(&digest[2..])
}

fn digest_of_caid(caid: &str) -> String {
    caid.rsplit(':').next().unwrap().trim().to_string()
}

fn head_digest(dir: &Path) -> String {
    digest_of_caid(
        fs::read_to_string(dir.join(".oo").join("HEAD"))
            .unwrap()
            .trim(),
    )
}

fn hex_of(v: &serde_json::Value) -> Option<String> {
    if let Some(s) = v.as_str() {
        return (s.len() == 64).then(|| s.to_string());
    }
    let a = v.as_array()?;
    let hex: String = a
        .iter()
        .map(|b| format!("{:02x}", b.as_u64().unwrap_or(0)))
        .collect();
    (hex.len() == 64).then_some(hex)
}

/// `(parent, root)` of a commit object, read directly off the disk.
///
/// Deliberately *not* a general walker: the whole point of this file is that
/// a general walker is what went blind. This reads the two fields a commit
/// has and nothing else, so it cannot be fooled by what it does not look at.
fn commit_links(dir: &Path, digest: &str) -> (Option<String>, Option<String>) {
    let bytes = fs::read(object_path(dir, digest)).unwrap();
    let j: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let p = j
        .get("parent")
        .and_then(|x| x.get("digest"))
        .and_then(hex_of);
    let r = j.get("root").and_then(|x| x.get("digest")).and_then(hex_of);
    (p, r)
}

/// The commit chain, newest first, with each commit's root.
fn chain(dir: &Path) -> Vec<(String, Option<String>)> {
    let mut out = Vec::new();
    let mut cur = Some(head_digest(dir));
    while let Some(d) = cur {
        let (p, r) = commit_links(dir, &d);
        out.push((d, r));
        cur = p;
    }
    out
}

fn write_src(dir: &Path, src: &str) {
    fs::write(dir.join("u.n"), src).unwrap();
}

/// A workspace with `n` generations committed. Six objects for `n == 3`:
/// three commits, each with its own root value.
fn repo_with_history(tag: &str, n: usize) -> nlang_interpreter::ScratchDir {
    let d = fresh_dir(tag);
    for i in 1..=n {
        write_src(&d, &format!("gen{i}: {{ n: {i} }}\n"));
        oo(&d, &["evolve", "u.n"]);
        let out = oo(&d, &["commit", "-m", &format!("gen{i}")]);
        assert!(
            out.contains("hash:"),
            "LIVENESS: generation {i} did not commit: {out}"
        );
    }
    d
}

#[cfg(unix)]
fn inode(p: &Path) -> u64 {
    use std::os::unix::fs::MetadataExt;
    fs::metadata(p).unwrap().ino()
}

// ════════════════════════════════════════════════════════════════════════
//  CONTROL — green before and after. Without these, a red below could pass
//  because the walker found nothing, or because the repo never worked.
// ════════════════════════════════════════════════════════════════════════

/// C1 — an untouched three-commit store is entirely reachable, `oo gc`
/// succeeds, and it collects nothing.
///
/// If the engine's walker silently found nothing, this store would read as
/// 100% garbage and every red below would "pass" by deleting everything.
#[test]
fn c1_untouched_store_is_whole_and_gc_collects_nothing() {
    let d = repo_with_history("c1", 3);
    let before = digests(&d);
    assert_eq!(before.len(), 6, "expected 3 commits + 3 roots: {before:?}");

    let (out, ok) = oo_raw(&d, &["gc", "--grant", "gc"]);
    assert!(ok, "gc failed on a healthy store: {out}");
    assert!(
        out.contains("6 objects, 6 reachable, 0 collectable"),
        "healthy store did not read as fully reachable: {out}"
    );
    assert_eq!(
        before,
        digests(&d),
        "gc removed something from a healthy store"
    );
    assert_eq!(
        oo(&d, &["log"]).matches("commit hash:").count(),
        3,
        "history is not walkable after a no-op gc"
    );
}

/// C2 — the descendants R2 will claim are missing are *present* first.
///
/// Standing rule: a red that asserts absence must assert a presence in the
/// same execution, or it can be green at baseline for the wrong reason.
#[test]
fn c2_the_descendants_exist_before_anyone_tampers() {
    let d = repo_with_history("c2", 3);
    let c = chain(&d);
    assert_eq!(c.len(), 3, "expected a three-commit chain: {c:?}");

    let all = digests(&d);
    for (commit, root) in &c {
        assert!(all.contains(commit), "commit {commit} is not in the store");
        let r = root.as_ref().expect("every commit has a root");
        assert!(all.contains(r), "root {r} of {commit} is not in the store");
    }
}

// ════════════════════════════════════════════════════════════════════════
//  RED — must fail on `dev b5f39bc`, for the reason each name states
// ════════════════════════════════════════════════════════════════════════

/// R1 — an undecodable *reachable* object must stop the sweep.
///
/// REAL_03 §6.6: `#object_undecodable` must abort the check and report. A
/// `live` set computed from an incomplete walk is a false answer, and
/// collection acts on it irreversibly.
#[test]
fn r1_undecodable_reachable_object_stops_the_sweep() {
    let d = repo_with_history("r1", 3);
    let before = digests(&d);
    let mid = chain(&d)[1].0.clone();

    fs::write(object_path(&d, &mid), b"not json at all").unwrap();

    let (out, ok) = oo_raw(&d, &["gc", "--grant", "gc"]);
    assert!(
        !ok,
        "gc exited zero after failing to decode a reachable object:\n{out}"
    );
    assert_eq!(
        before,
        digests(&d),
        "gc collected objects although its walk was incomplete:\n{out}"
    );
}

/// R2 — and the descendants reachable only through it are still on disk.
///
/// The expected set is computed **before** the tamper, because the only
/// walkers available afterwards share the engine's blind spot.
#[test]
fn r2_descendants_of_the_undecodable_object_survive() {
    let d = repo_with_history("r2", 3);
    let c = chain(&d);
    let mid = c[1].0.clone();

    // Everything reachable only through the middle commit.
    let mut must_survive: BTreeSet<String> = BTreeSet::new();
    must_survive.insert(c[1].1.clone().unwrap()); // the middle commit's root
    must_survive.insert(c[2].0.clone()); // the oldest commit
    must_survive.insert(c[2].1.clone().unwrap()); // and its root
    for x in &must_survive {
        assert!(digests(&d).contains(x), "{x} was not there to begin with");
    }

    fs::write(object_path(&d, &mid), b"not json at all").unwrap();
    let (out, _) = oo_raw(&d, &["gc", "--grant", "gc"]);

    let after = digests(&d);
    let gone: Vec<&String> = must_survive
        .iter()
        .filter(|x| !after.contains(*x))
        .collect();
    assert!(
        gone.is_empty(),
        "{} objects reachable only through an undecodable object were deleted: {gone:?}\n{out}",
        gone.len()
    );
}

/// R3 — bytes that decode fine but are *not* the requested object.
///
/// The payload is another real object from the same store, so the walk
/// genuinely follows it and genuinely computes a different `live` set —
/// an adversarial input that participates rather than one that is merely
/// malformed (the v0.2.50 lesson).
///
/// Measured today: two objects removed, exit 0, and the string
/// `caid_mismatch` appears **zero** times in the output.
#[test]
fn r3_valid_bytes_at_the_wrong_address_are_caught() {
    let d = repo_with_history("r3", 3);
    let before = digests(&d);
    let c = chain(&d);
    let (mid, oldest) = (c[1].0.clone(), c[2].0.clone());

    let borrowed = fs::read(object_path(&d, &oldest)).unwrap();
    assert!(
        serde_json::from_slice::<serde_json::Value>(&borrowed).is_ok(),
        "the payload must itself be a valid object, or this proves nothing"
    );
    fs::write(object_path(&d, &mid), &borrowed).unwrap();

    let (out, ok) = oo_raw(&d, &["gc", "--grant", "gc"]);
    assert!(
        out.contains("caid_mismatch"),
        "bytes at the wrong address produced no verdict at all:\n{out}"
    );
    assert!(
        !ok,
        "gc exited zero on a store whose bytes are lying:\n{out}"
    );
    assert_eq!(
        before,
        digests(&d),
        "gc swept using the tamperer's references:\n{out}"
    );
}

/// R4 — the affiliation claim must not be written in place.
///
/// `oodp.rs:630` is a bare `std::fs::write`, which truncates and rewrites the
/// existing file: a crash between the two leaves a claim that is neither the
/// old one nor the new one. Same inode across two writes is the race-free
/// signature of writing in place (the probe shape v0.11.0 settled on).
#[cfg(unix)]
#[test]
fn r4_affiliation_claim_is_replaced_not_overwritten() {
    let d = fresh_dir("r4");
    let p = d.join(".oo").join("affiliation");

    AffiliationClaim {
        operator_key: "op-one".into(),
        signature: "sig-one".into(),
        expires: 1,
    }
    .write_file(&p)
    .unwrap();
    let first = inode(&p);

    AffiliationClaim {
        operator_key: "op-two".into(),
        signature: "sig-two".into(),
        expires: 2,
    }
    .write_file(&p)
    .unwrap();
    let second = inode(&p);

    assert_ne!(
        first, second,
        "the claim was rewritten in place (inode unchanged) — there is a window \
         in which the file on disk is neither claim"
    );
    assert!(
        fs::read_to_string(&p).unwrap().contains("op-two"),
        "LIVENESS: the second write did not land"
    );
}

/// R5 — peer-directory compaction must not depend on a name it does not own.
///
/// `peers.rs:451` builds its temp path as `with_extension("directory.tmp")`,
/// which is the same string in every process. Squatting that one name turns
/// the concurrent-writer race into a deterministic assertion: today the write
/// fails, `.ok()?` swallows it, and compaction silently does not happen.
#[test]
fn r5_compaction_survives_a_squatted_temp_name() {
    let d = fresh_dir("r5");
    let path = peers::directory_path(&d);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::create_dir_all(path.with_extension("directory.tmp")).unwrap();

    let mut state = PeerDirectoryState::default();
    let done = peers::compact(&d, "node-under-test", &HashMap::new(), &mut state);

    assert!(
        done.is_some(),
        "compaction gave up because one predictable filename was taken"
    );
    assert!(
        path.exists(),
        "compaction reported nothing and wrote nothing"
    );
}

/// R6 — and the same for the discovery config.
///
/// `discovery_config.rs:65` is a second, independent implementation of
/// temp+rename. It gets `sync_all` right and the temp *name* wrong.
#[test]
fn r6_discovery_config_survives_a_squatted_temp_name() {
    let d = fresh_dir("r6");
    let path = DiscoveryConfig::path(&d);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::create_dir_all(path.with_extension("n.tmp")).unwrap();

    let cfg = DiscoveryConfig {
        affiliation_roots: ["some-root-key".to_string()].into_iter().collect(),
    };
    assert!(
        cfg.write(&d).is_ok(),
        "the config could not be written because one predictable filename was taken"
    );
    assert!(
        fs::read_to_string(&path).unwrap().contains("some-root-key"),
        "LIVENESS: the config did not land"
    );
}

// ════════════════════════════════════════════════════════════════════════
//  PIN — green today, and the fix must not break them. These are the two
//  clauses of §6.6 that cut against this arc.
// ════════════════════════════════════════════════════════════════════════

/// P1 — an *absent* reachable digest is not fatal.
///
/// §6.6 lets "absent / opaque" continue under §9.1; only undecodable and
/// mismatched must stop. Work-order ruling R3. A delivery that makes every
/// walk failure fatal would pass R1–R3 and break this.
#[test]
fn p1_an_absent_object_does_not_stop_the_walk() {
    let d = repo_with_history("p1", 3);
    let oldest_root = chain(&d)[2].1.clone().unwrap();
    fs::remove_file(object_path(&d, &oldest_root)).unwrap();

    let (out, ok) = oo_raw(&d, &["gc", "--grant", "gc"]);
    assert!(ok, "an absent object made gc fail: {out}");
    assert!(
        !out.contains("object_undecodable"),
        "an absent object was reported as undecodable — the two are different \
         verdicts and §6.6 forbids collapsing them: {out}"
    );
}

/// P2 — diagnosis must not disappear along with the gate.
///
/// The easy way to satisfy R1 is to bail out early. `--dry-run` destroys
/// nothing, so it must still say everything it saw.
#[test]
fn p2_dry_run_still_reports_what_it_saw() {
    let d = repo_with_history("p2", 3);
    let mid = chain(&d)[1].0.clone();
    fs::write(object_path(&d, &mid), b"not json at all").unwrap();

    let (out, _) = oo_raw(&d, &["gc", "--grant", "gc", "--dry-run"]);
    assert!(
        out.contains(&mid),
        "dry-run did not name the object it could not decode: {out}"
    );
    assert!(
        out.contains("objects,") && out.contains("reachable"),
        "dry-run stopped reporting the counts it used to report: {out}"
    );
}

/// P3 — a healthy store is never accused.
///
/// The store holds Commits as well as values. Adding a decoder to satisfy R3
/// is exactly how `oo inspect` came to report `#object_undecodable` for a
/// Commit the engine had just written (v0.2.52). §6.6: 裁決必須為真.
#[test]
fn p3_a_healthy_store_earns_no_verdict() {
    let d = repo_with_history("p3", 3);
    let (out, ok) = oo_raw(&d, &["gc", "--grant", "gc", "--dry-run"]);
    assert!(ok, "dry-run gc failed on a healthy store: {out}");
    assert!(
        !out.contains("object_undecodable") && !out.contains("caid_mismatch"),
        "the engine accused its own objects: {out}"
    );
}

/// P4 — gc still needs its grant, whatever else changes.
///
/// Collection is privileged (SPEC_08 §6.2) and the new failure paths must not
/// become a way to reach it.
#[test]
fn p4_gc_still_requires_its_grant() {
    let d = repo_with_history("p4", 3);
    let mid = chain(&d)[1].0.clone();
    fs::write(object_path(&d, &mid), b"not json at all").unwrap();

    let (out, ok) = oo_raw(&d, &["gc"]);
    assert!(!ok, "gc ran without its grant: {out}");
    assert!(
        out.contains("privileged_required") || out.contains("--grant gc"),
        "refusal did not name the capability: {out}"
    );
    assert_eq!(
        digests(&d).len(),
        6,
        "an ungranted gc removed objects anyway"
    );
}
