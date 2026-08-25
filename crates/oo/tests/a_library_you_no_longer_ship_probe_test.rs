// A library you no longer ship (Q-025, pre-committed by work order:
// docs/a_library_you_no_longer_ship_handover.md).
//
// ── The claim ────────────────────────────────────────────────────────────
//
// The engine must select a standard root by the digest the ROOT names, not
// by comparing against the one copy it happens to hold.
//
// Today those two are indistinguishable, because there is exactly one.
// Measured 2026-08-15: root_with_system() is byte-for-byte unchanged across
// v0.20.0 -> v0.21.0 -> v0.22.0, digest 65f52e2d…b2bcbe72 throughout.
//
// What being indistinguishable costs, measured with three real binaries:
// add ONE builtin, and v0.22.0 and v0.22.0+one cannot read each other's
// stores in either direction --
//     refusing root: standard root digest … is unavailable
// The wall is correct (REAL_03 §6.8, third MUST). What is missing is a door.
//
// ── What this file can and cannot witness ────────────────────────────────
//
// It CANNOT witness "the engine holds more than one". That needs two engines
// whose standard roots DIFFER, and no in-tree test can build a second one --
// the same reason O54 was verified at acceptance in Q-011. The main evidence
// for this arc is the three-binary matrix in handover §5.2, NOT this file.
//
// Read that twice before concluding from four greens. In Q-011 round 2 all
// nine probes were green while `grep -c hydrate` was 0.
//
// What this file DOES witness: the observable surface (§2.3) and the refusal
// message that stops claiming a singular holding (§2.2), plus two controls
// that must not regress.
//
// ── Probe integrity ──────────────────────────────────────────────────────
//
// Reds are `#[ignore]`d and MUST be red at the baseline for the reason
// stated in each. The delivery may remove `#[ignore]` and nothing else in
// this file. C0 runs first: an assertion about a store is vacuous if the
// store was never built.

use std::path::Path;
use std::process::Command;

fn fresh(tag: &str) -> nlang_interpreter::ScratchDir {
    let d = nlang_interpreter::ScratchDir::new(&format!("noship-{tag}"));
    let _ = Command::new(env!("CARGO_BIN_EXE_oo"))
        .arg("init")
        .current_dir(&*d)
        .env("OO_IDENTITY", d.join("identity-for-tests"))
        .env("OO_NODE_HOME", d.join("node-home-for-tests"))
        .output();
    d
}

fn oo(dir: &Path, args: &[&str]) -> String {
    let o = Command::new(env!("CARGO_BIN_EXE_oo"))
        .args(args)
        .current_dir(dir)
        .env("OO_IDENTITY", dir.join("identity-for-tests"))
        .env("OO_NODE_HOME", dir.join("node-home-for-tests"))
        .output()
        .unwrap();
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

const SRC: &str = "app: { k1: 1 + 2, v: 10 }\n";

fn committed(tag: &str) -> nlang_interpreter::ScratchDir {
    let d = fresh(tag);
    std::fs::write(d.join("u.n"), SRC).unwrap();
    oo(&d, &["evolve", "u.n"]);
    oo(&d, &["commit", "-m", "probe"]);
    d
}

/// Every CAS object file under `.oo/objects/`. Never truncated.
fn cas_objects(dir: &Path) -> Vec<std::path::PathBuf> {
    fn walk(p: &Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(rd) = std::fs::read_dir(p) else { return };
        for e in rd.flatten() {
            let path = e.path();
            if path.is_dir() {
                walk(&path, out);
            } else {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    walk(&dir.join(".oo").join("objects"), &mut out);
    out.sort();
    out
}

const SENTINEL: &str = "__nlang_system_digest";

/// The standard-root digest this store's root actually names, read out of the
/// object bytes. Deliberately NOT read from any engine API: the probe must be
/// able to disagree with the engine.
fn digest_in_store(dir: &Path) -> Option<String> {
    for p in cas_objects(dir) {
        let Ok(s) = std::fs::read_to_string(&p) else { continue };
        if let Some(hex) = nlang_interpreter::store_codec::named_standard_digest(&s) {
            return Some(hex);
        }
    }
    None
}

/// Rewrite the standard-root digest inside the root object to `to`, in place.
/// The address is NOT recomputed -- that is the point: the digest check must
/// fire before the address check, which is what REAL_03 §6.8's precedent
/// records ("把根裡那個 digest 竄改為零").
fn retarget_digest(dir: &Path, to: &str) -> bool {
    let Some(from) = digest_in_store(dir) else { return false };
    for p in cas_objects(dir) {
        let Ok(s) = std::fs::read_to_string(&p) else { continue };
        if s.contains(SENTINEL) && s.contains(&from) {
            std::fs::write(&p, s.replace(&from, to)).unwrap();
            return true;
        }
    }
    false
}

const ABSENT: &str = "00000000000000000000000000000000000000000000000000000000deadbeef";

// ── C0 — control, runs first ─────────────────────────────────────────────
// Everything below asserts something about a store that names a standard
// root. If no such store can be built, all of it is vacuous.

#[test]
fn c0_a_fresh_store_names_a_standard_root_and_reads_back() {
    let d = committed("c0");

    let digest = digest_in_store(&d).unwrap_or_else(|| {
        panic!(
            "control failed: no root under .oo/objects carries `{SENTINEL}`. \
             Every probe in this file is vacuous until this passes. \
             Objects present: {:?}",
            cas_objects(&d)
        )
    });
    assert_eq!(digest.len(), 64, "digest must be 64 hex chars, got {digest:?}");

    let log = oo(&d, &["log"]);
    assert!(
        log.contains("commit hash:"),
        "control failed: a store the engine just wrote does not read back.\n{log}"
    );
}

// ── C1 — control: the wall must survive this arc ─────────────────────────
// REAL_03 §6.8 third MUST: a digest the engine does not have is a refusal,
// and the message must say WHICH. This arc widens what the engine has; it
// must not remove the refusal for what it still does not have.

#[test]
fn c1_a_digest_no_engine_ever_published_is_still_refused_by_name() {
    let d = committed("c1");
    assert!(retarget_digest(&d, ABSENT), "could not retarget the digest");

    let out = oo(&d, &["log"]);
    assert!(
        out.contains(ABSENT),
        "the refusal must name the digest it could not resolve.\n{out}"
    );
    assert!(
        !out.contains("commit hash:"),
        "a root naming an unheld standard root must NOT be opened.\n{out}"
    );
}

// ── P1 — red: the claim must be observable ───────────────────────────────
// §6.8.2's second MUST is a statement about an engine. A statement that
// cannot be observed cannot be conformance-checked. Today the only way to
// learn whether this engine can open this store is to open it and read the
// error.
//
// Baseline red because `oo status` prints only
//     "Universe is static (no staged changes)."
// and names no digest at all.

#[test]
fn p1_status_names_the_standard_root_this_store_depends_on() {
    let d = committed("p1");
    let actual = digest_in_store(&d).expect("C0 must pass first");

    let status = oo(&d, &["status"]);
    assert!(
        status.contains(&actual),
        "`oo status` must name the standard root this store depends on.\n\
         expected to find {actual}\ngot:\n{status}"
    );
}

// ── P2 — red: the message must stop claiming a singular holding ──────────
// Today, verbatim:
//   refusing root: standard root digest {digest} is unavailable
//   (this engine has {expected})
//
// `this engine has <one>` becomes false the moment the engine holds more
// than one -- which is exactly what this arc delivers. So the false clause
// must go while the naming of what is MISSING stays.
//
// Note the shape: this probe asserts a non-existence AND an existence in the
// same run. Without the existence half, deleting the whole message would
// turn it green.

#[test]
fn p2_the_refusal_names_what_is_missing_without_claiming_a_single_holding() {
    let d = committed("p2");
    assert!(retarget_digest(&d, ABSENT), "could not retarget the digest");

    let out = oo(&d, &["log"]);

    // existence half — REAL_03 §6.8 third MUST must survive
    assert!(
        out.contains(ABSENT),
        "the refusal must still name the digest it could not resolve.\n{out}"
    );

    // non-existence half — the clause that goes false once a table exists
    assert!(
        !out.contains("this engine has"),
        "the refusal must not assert a singular holding once the engine \
         holds a set.\n{out}"
    );
}
