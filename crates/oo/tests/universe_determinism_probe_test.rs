// The engine stops forging the governance root (2026-07-27,
// pre-committed by work order: docs/universe_determinism_handover.md).
//
// ── The headline, measured on v0.2.44 ────────────────────────────────────
//
// Six fresh repositories, the same one-line source, six processes:
//
//   root CAID   be91ecfe…  7b48ed60…  611724cb…
//               5c39042f…  57564041…  51a0fe12…
//   value CAID  hash:sha256:v2:…:681781ef…   (one value, six times)
//
// Six different universes from one source. Content addressing works for
// values and does not work for the universe.
//
// Every leaf of two roots from separate processes — 2,588 paths, exactly one
// differs:
//
//   /Combo/system/Official/Combo/data/architects/Atom[0]/Str
//
// which is hex(Identity::new_random().public_key), minted at every
// `Ouroboros::init` and written into the universe by `root_with_system()`.
// The hash is correct; the content really is different. What is wrong is
// that a random number is part of the universe.
//
// ── It is worse than a random number ─────────────────────────────────────
// ORDER_01 §117 defines `~%Official.architects` as the TRUST ROOT, a SET of
// Voters; §88 says `~%Official` changes only through the RFC process; and
// SPEC_10 §93 says a refine signer must be in that set. The engine mints a
// string, not a set; a local random self-appointment, not a governance root;
// at process start, not by RFC. `~%Official` is not even in SPEC_13 §3.1's
// genesis seed list.
//
// And the appointment is load-bearing: the same key is inserted into
// `architect_registry`, so `oo refine` demands `--sign` and the signature
// always verifies — because the signer is the sole architect. An authority
// check that passes because the checker appointed itself is a lying audit
// surface (v0.2.41 `#squash` precedent).
//
// ── Rulings ──────────────────────────────────────────────────────────────
// A  `architects` leaves the universe root. It is a claim about this
//    workspace's trust configuration, not semantic content — the assertion
//    layer of discussion 025, where `.oo/architects.json` already lives.
//    Removal is functionally inert: `verify_refine_authority` reads the
//    registry, never the root field.
// B  The engine stops appointing itself. A fresh repository has an empty
//    registry, so refine is bootstrap-exempt and needs no signature. This is
//    a relaxation, and the honest one: the refusal it replaces required only
//    a signature the same process could always produce.
// C  (acceptor's corollary) An unverified refine must be RECORDED as
//    unverified, in `RefineInfo` — which `Commit::content_hash` does not hash
//    beyond source/target digests, so commit CAIDs stay put. NOT in
//    `CommitMeta`, whose Debug string is hashed. R7 pins that.
//
// ── A consequence worth stating ──────────────────────────────────────────
// A and B together mean the local engine can no longer be whitelisted at
// all: its identity is random per process AND no longer observable. So an
// authority-VERIFIED refine becomes unconstructible locally until identity
// persistence lands. R5b measures the other side of that coin, which is the
// sharpest statement of what was wrong: today, a whitelist that does not
// contain you does not stop you signing for yourself.
//
// That is not a dead end, it is the queue — and the party the engine is
// missing is the operator. REAL_01 §7.2 `[Core Requirement]` already says the
// engine loads a public-key whitelist from `~/.oo/authorized_keys`: the HOME
// directory, issued out of band, with a lifecycle and a revocation list. The
// engine reads none of it; it reads the workspace file `.oo/architects.json`
// and otherwise appoints itself. So an empty registry does not mean "no
// architect exists" — it means nobody has declared one yet, which is what a
// cold start honestly looks like.
//
// The shape is settled precedent: SPEC_08 §6.1.2 ruling P1 — a privilege
// cannot be self-granted from inside a program; it arrives through a trusted
// out-of-band channel. Architecthood is P1 one layer down, and the engine's
// self-appointment is precisely the self-grant P1 forbids.
//
// Order of the sequel, so it is not attempted piecemeal:
//   A (identity out of the root)  →  persistence (stable key, assertion
//   layer)  →  operator declaration  →  an authority check that can fail.
// A is persistence's PREREQUISITE, not its rival: persistence instead of A
// would leave the root differing between repositories, but after A the
// identity is not in the root at all.
//
// ── Anti-vacuity is this file's theme ────────────────────────────────────
// Every comparison gate first asserts both sides are well-formed and
// non-empty — a 64-hex digest, a leaf count in the thousands — because this
// arc exists partly because the ACCEPTOR's own v0.2.44 stability script
// compared `None` to `None` across 143 vectors and reported a perfect score.
// It read the key `content_digest`; the key is `digest`. A comparison that
// cannot fail has not been made.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

const SRC: &str = "v: { hello: \"world\" }\n";

/// Golden CAID of `{ hello: "world" }` — measured stable across every process
/// and both binaries. Ordinary values must NOT move in this arc.
const GOLDEN_VALUE_CAID: &str = "hash:sha256:v2:_:gICS1LCf09bLAQD//5HUsJ/T1ssBAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA:681781ef857ac859326d707bdfcd04fc939b78e7c9060dd674d9a8be536f2ae4";

// ── harness ─────────────────────────────────────────────────────────────

fn fresh_dir() -> PathBuf {
    let mut d = std::env::temp_dir();
    d.push(format!(
        "nlang-det-{}-{}",
        std::process::id(),
        DIR_SEQ.fetch_add(1, Ordering::SeqCst)
    ));
    fs::remove_dir_all(&d).ok();
    fs::create_dir_all(&d).unwrap();
    d
}

fn oo(dir: &Path, args: &[&str]) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_oo"))
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout).trim(),
        String::from_utf8_lossy(&out.stderr).trim()
    )
}

/// A repository with `SRC` evolved and committed.
fn repo() -> PathBuf {
    let d = fresh_dir();
    fs::write(d.join("s.n"), SRC).unwrap();
    oo(&d, &["evolve", "s.n"]);
    let out = oo(&d, &["commit", "-m", "x"]);
    assert!(
        out.contains("Commit successful"),
        "repo() failed to commit: {out}"
    );
    d
}

fn head_commit(dir: &Path) -> String {
    let c = oo(dir, &["log"])
        .lines()
        .find_map(|l| l.strip_prefix("commit ").map(|s| s.trim().to_string()))
        .unwrap_or_default();
    assert!(c.starts_with("hash:sha256:"), "no HEAD commit in {dir:?}");
    c
}

fn object_path(dir: &Path, caid: &str) -> PathBuf {
    let d = caid.rsplit(':').next().unwrap();
    dir.join(".oo")
        .join("objects")
        .join("sha256")
        .join(&d[..2])
        .join(&d[2..])
}

fn commit_json(dir: &Path) -> serde_json::Value {
    let p = object_path(dir, &head_commit(dir));
    serde_json::from_slice(&fs::read(&p).unwrap_or_else(|e| panic!("{p:?}: {e}"))).unwrap()
}

/// Hex digest of the universe root this repository's HEAD points at.
///
/// The key is `digest`. It may serialise as a hex string or a byte array;
/// both are normalised here. Anything else is a HARNESS failure and panics —
/// it must never silently become a value that compares equal to another
/// failure. That is the exact bug this arc's headline measurement had.
fn root_digest(dir: &Path) -> String {
    let root = commit_json(dir)["root"].clone();
    let dg = &root["digest"];
    let hex = if let Some(s) = dg.as_str() {
        s.to_string()
    } else if let Some(a) = dg.as_array() {
        a.iter()
            .map(|b| format!("{:02x}", b.as_u64().expect("digest byte")))
            .collect()
    } else {
        panic!("commit root has no usable `digest` field: {root}");
    };
    assert!(
        hex.len() == 64 && hex.chars().all(|c| c.is_ascii_hexdigit()),
        "root digest is not a 64-hex string: {hex:?}"
    );
    hex
}

fn root_caid(dir: &Path) -> String {
    let root = commit_json(dir)["root"].clone();
    let sketch = root["lattice_sketch"].as_str().unwrap_or("").to_string();
    let masa = root["masa_ref"].as_str().unwrap_or("Top");
    let masa = if masa == "Top" { "_" } else { masa };
    if sketch.is_empty() {
        format!("hash:sha256:v1:{}", root_digest(dir))
    } else {
        format!("hash:sha256:v2:{masa}:{sketch}:{}", root_digest(dir))
    }
}

/// Every leaf of a JSON document, keyed by its sorted path.
fn leaves(v: &serde_json::Value, path: &str, out: &mut BTreeMap<String, String>) {
    match v {
        serde_json::Value::Object(m) => {
            for (k, sub) in m {
                leaves(sub, &format!("{path}/{k}"), out);
            }
        }
        serde_json::Value::Array(a) => {
            for (i, sub) in a.iter().enumerate() {
                leaves(sub, &format!("{path}[{i}]"), out);
            }
        }
        other => {
            out.insert(path.to_string(), other.to_string());
        }
    }
}

fn root_leaves(dir: &Path) -> BTreeMap<String, String> {
    let p = object_path(dir, &root_caid(dir));
    let v: serde_json::Value =
        serde_json::from_slice(&fs::read(&p).unwrap_or_else(|e| panic!("{p:?}: {e}"))).unwrap();
    let mut out = BTreeMap::new();
    leaves(&v, "", &mut out);
    assert!(
        out.len() > 1000,
        "root has only {} leaves — it was not parsed properly",
        out.len()
    );
    out
}

/// A source CAID the shadow scan / refine can name, plus a target.
fn stored(dir: &Path, expr: &str) -> String {
    fs::write(
        dir.join("i.n"),
        format!("id: ~%Discovery./identify_and_store {expr}\n"),
    )
    .unwrap();
    let caid = oo(dir, &["run", "i.n", "--observe", "id"])
        .trim()
        .trim_start_matches('"')
        .split('"')
        .next()
        .unwrap()
        .to_string();
    assert!(caid.starts_with("hash:sha256:"), "stored() got {caid:?}");
    caid
}

// ── R1 ──────────────────────────────────────────────────────────────────

/// One source, N processes, one universe. SPEC_13 §4.1.2 義務 #1.
#[test]
fn red_one_source_yields_one_universe_across_processes() {
    let digests: Vec<String> = (0..6).map(|_| root_digest(&repo())).collect();

    // LIVENESS: six well-formed digests really were produced. `root_digest`
    // panics rather than returning a sentinel, so a set of size 1 can only
    // mean six genuine agreements.
    assert_eq!(digests.len(), 6);

    let distinct: std::collections::BTreeSet<&String> = digests.iter().collect();
    assert_eq!(
        distinct.len(),
        1,
        "one source produced {} different universes: {:#?}",
        distinct.len(),
        distinct
    );
}

// ── R2 ──────────────────────────────────────────────────────────────────

/// Exhaustive: no leaf of the universe root may vary between processes.
#[test]
fn red_no_leaf_of_the_root_varies_between_processes() {
    let a = root_leaves(&repo());
    let b = root_leaves(&repo());

    // LIVENESS: both roots parsed to thousands of leaves over the same key
    // set — asserted inside `root_leaves`, plus the shape check here.
    assert_eq!(
        a.keys().collect::<Vec<_>>(),
        b.keys().collect::<Vec<_>>(),
        "the two roots do not even have the same shape"
    );

    let differing: Vec<&String> = a.keys().filter(|k| a.get(*k) != b.get(*k)).collect();
    assert!(
        differing.is_empty(),
        "{} of {} leaf paths differ between two processes: {:#?}",
        differing.len(),
        a.len(),
        differing
    );
}

// ── R3 ──────────────────────────────────────────────────────────────────

/// The engine must not mint a local `~%Official.architects` into the universe.
#[test]
fn red_engine_does_not_mint_a_governance_root() {
    let d = fresh_dir();

    // LIVENESS / CONTROL: `~%Official` is still mounted, so a missing
    // `architects` means removal and not a broken module.
    //
    // Rewritten by the ACCEPTOR during the identity_persistence arc, which
    // retires `/sign_refine` from the language surface — this control used
    // to name that morphism and would otherwise have become a false red for
    // that delivery. `{{` and not "not bottom": a module removed from the
    // (open) system root evaluates to `_`, so "not bottom" is not a control.
    let module = oo(&d, &["eval", "~%Official"]);
    assert!(
        module.contains("{{"),
        "control: ~%Official must still be mounted: {module}"
    );

    let got = oo(&d, &["eval", "~%Official.architects"]);
    let hexish = got
        .trim()
        .trim_matches('"')
        .chars()
        .filter(|c| c.is_ascii_hexdigit())
        .count();
    assert!(
        hexish < 64,
        "the engine still mints a key into the universe: {got}"
    );
}

// ── R4 ──────────────────────────────────────────────────────────────────

/// A fresh repository has no architect, so a refine needs no signature.
#[test]
fn red_fresh_repository_has_no_self_appointed_architect() {
    let d = repo();
    let src = stored(&d, "{ old: 1 }");
    let tgt = stored(&d, "{ old: 1, new: 2 }");

    // LIVENESS: the same refine WITH a signature is accepted today, so the
    // command, the CAIDs and the repository are all live; only the unsigned
    // form is in question.
    let signed = oo(
        &repo_with(&d),
        &["refine", "-s", &src, "-t", &tgt, "-m", "s", "--sign"],
    );
    assert!(
        signed.contains("Refine commit:"),
        "control: a signed refine must work: {signed}"
    );

    let unsigned = oo(&d, &["refine", "-s", &src, "-t", &tgt, "-m", "u"]);
    assert!(
        unsigned.contains("Refine commit:"),
        "a fresh repository still demands a signature it invented for itself: {unsigned}"
    );
}

/// A sibling repository seeded from `d`'s sources, for control arms that must
/// not disturb the repository under test.
fn repo_with(d: &Path) -> PathBuf {
    let n = fresh_dir();
    for f in ["s.n", "i.n"] {
        if let Ok(b) = fs::read(d.join(f)) {
            fs::write(n.join(f), b).unwrap();
        }
    }
    oo(&n, &["evolve", "s.n"]);
    oo(&n, &["commit", "-m", "x"]);
    n
}

// ── R5 ──────────────────────────────────────────────────────────────────

/// An unverified refine must be recorded as unverified. Paired against an
/// ordinary commit, which must not carry the marker.
///
/// CALIBRATION NOTE — this is the one gate whose baseline redness is
/// DEPENDENT. At baseline an unsigned refine is refused outright (that is
/// R4's defect), so this test fails at `refine did not run` and never reaches
/// the assertion it is named for. That is honest but weaker than the other
/// gates: it only starts measuring its own subject once B is implemented.
/// It still does its job — a delivery that fixes R4 and omits the marker
/// fails here on the marker assertion — but do not read its baseline failure
/// as evidence about recording.
#[test]
fn red_an_unverified_refine_says_so() {
    let d = repo();
    let src = stored(&d, "{ old: 1 }");
    let tgt = stored(&d, "{ old: 1, new: 2 }");

    let ordinary = commit_json(&d).to_string();
    assert!(
        !ordinary.to_lowercase().contains("unverified"),
        "control: an ordinary commit must not carry the marker: {ordinary}"
    );

    let out = oo(&d, &["refine", "-s", &src, "-t", &tgt, "-m", "u"]);
    assert!(out.contains("Refine commit:"), "refine did not run: {out}");

    let refined = commit_json(&d).to_string();
    assert!(
        refined.to_lowercase().contains("unverified"),
        "a refine that verified no authority is indistinguishable from one \
         that did: {refined}"
    );
}

/// The sharpest statement of what was wrong: a whitelist that does not
/// contain you must not let you sign for yourself.
#[test]
fn red_a_whitelist_without_you_refuses_your_signature() {
    let d = repo();
    let src = stored(&d, "{ old: 1 }");
    let tgt = stored(&d, "{ old: 1, new: 2 }");

    fs::write(
        d.join(".oo").join("architects.json"),
        r#"["00000000000000000000000000000000000000000000000000000000000000ff"]"#,
    )
    .unwrap();

    // LIVENESS: the whitelist file is where the engine looks.
    assert!(d.join(".oo").join("architects.json").exists());

    let out = oo(&d, &["refine", "-s", &src, "-t", &tgt, "-m", "s", "--sign"]);
    assert!(
        !out.contains("Refine commit:"),
        "a whitelist that does not contain this engine still accepted its own \
         signature — the authority check passes because the checker appointed \
         itself: {out}"
    );
}

// ── R6 ──────────────────────────────────────────────────────────────────

/// Federation at the root: the CAID engine B computes for its own universe
/// must resolve against engine A's store.
#[test]
fn red_two_engines_resolve_the_same_universe() {
    let a = repo();
    let b = repo();

    let want = root_caid(&b);
    assert!(want.contains(':'), "root_caid produced nothing usable");

    fs::write(
        b.join("f.n"),
        format!(
            "conn: ~%Discovery./connect {{ 0: \"A\", 1: \"{}\" }}\n\
             got:  ~%Discovery./fetch   {{ 0: \"A\", 1: \"{want}\" }}\n",
            a.display()
        ),
    )
    .unwrap();

    // LIVENESS: the peer really is reachable — A resolves its own root.
    let self_check = oo(&a, &["inspect", &root_caid(&a)]);
    assert!(
        self_check.contains("CAID:"),
        "control: A cannot read its own root: {self_check}"
    );

    let got = oo(&b, &["run", "f.n", "--observe", "got"]);
    assert!(
        !got.contains("_|_"),
        "engine B's universe address does not exist in engine A's store, \
         though both were built from the same source: {got}"
    );
}

// ── R7 — pins an invariant nothing pinned ───────────────────────────────

/// `Commit::content_hash` hashes `format!("{:?}", meta)`. `CommitMeta`
/// therefore carries a HAND-WRITTEN `Debug` that omits `abandoned` when it is
/// `None` — that omission is the only reason commits made before v0.2.41 stay
/// bit-stable, and now that v0.2.43 verifies reads, breaking it would turn
/// every historical commit into `#caid_mismatch` rather than silent drift.
///
/// Nothing pinned it. A `#[derive(Debug)]` would pass review and destroy
/// every repository in existence.
#[test]
fn pin_commit_meta_debug_omits_absent_fields() {
    use nlang_interpreter::CommitMeta;
    let bare = CommitMeta {
        author: Some("a".into()),
        timestamp: 7,
        message: Some("m".into()),
        abandoned: None,
        privileged_effect: None,
    };
    let s = format!("{bare:?}");
    // ACCEPTOR STRENGTHENING (privileged_effect_audit). Naming the fields one
    // by one is a pin that has to be extended every time a field is added, and
    // the arc that added `privileged_effect` showed exactly that: the literal
    // above had to gain the field to compile, while the assertion below still
    // only knew about `abandoned`. Pin the WHOLE rendering instead, so any new
    // optional field that leaks into Debug fails here whether or not anyone
    // remembered to name it.
    assert_eq!(
        s, "CommitMeta { author: Some(\"a\"), timestamp: 7, message: Some(\"m\") }",
        "CommitMeta's Debug must render exactly the three ordinary fields when \
         every optional one is absent — it is hashed into every commit digest"
    );

    let with = CommitMeta {
        abandoned: Some(vec!["h".into()]),
        ..bare.clone()
    };
    assert!(
        format!("{with:?}").contains("abandoned"),
        "a present field must still be recorded"
    );
}

// ── pins ────────────────────────────────────────────────────────────────

/// Ordinary value addresses must not move. This arc is about the universe.
#[test]
fn pin_ordinary_value_caids_do_not_move() {
    let d = fresh_dir();
    assert_eq!(
        stored(&d, "{ hello: \"world\" }"),
        GOLDEN_VALUE_CAID,
        "an ordinary value's address moved"
    );
}

/// Values remain deterministic across processes — the property the universe
/// is supposed to have and does not.
#[test]
fn pin_value_caids_are_deterministic_across_processes() {
    let caids: std::collections::BTreeSet<String> = (0..4)
        .map(|_| stored(&fresh_dir(), "{ a: 1, b: [2, 3] }"))
        .collect();
    assert_eq!(caids.len(), 1, "value CAIDs diverged: {caids:#?}");
}

/// `~%Official` stays mounted.
///
/// Was `pin_official_module_still_signs`, which asserted `/sign_refine` was
/// usable. The identity_persistence arc retires that morphism deliberately
/// (a persistent operator key turns "any n/ program can get a signature"
/// from harmless into the trust root), so the ACCEPTOR narrowed this pin to
/// the part that survives, rather than leaving the delivery a red it was
/// asked to cause.
#[test]
fn pin_official_module_stays_mounted() {
    let d = fresh_dir();
    let out = oo(&d, &["eval", "~%Official"]);
    assert!(
        out.contains("{{"),
        "~%Official is no longer a mounted closed combo: {out}"
    );
}

/// Ordinary commit and log still work, and the commit still verifies on read
/// (v0.2.43 §6.6 is unaffected by this arc).
#[test]
fn pin_commit_and_log_still_verify() {
    let d = repo();
    let log = oo(&d, &["log"]);
    assert!(log.contains("commit hash:sha256:"), "log regressed: {log}");
    assert!(
        !log.contains("caid_mismatch"),
        "a freshly written commit does not verify: {log}"
    );
}
