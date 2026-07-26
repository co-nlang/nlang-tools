// CAS read integrity — a path is not an identity (2026-07-26,
// pre-committed by work order: docs/cas_integrity_handover.md).
//
// ── The headline, measured verbatim on v0.2.42 ───────────────────────────
//
//   $ oo inspect hash:sha256:v2:_:/5+TjN...:b0e7f5bd...
//   CAID:   hash:sha256:v2:_:/5+TjN...:b0e7f5bd...
//   MASA:   _
//   Sketch: /5+TjN/DktgqAICg+47dgbiuWQD//+eC...
//
//   { a: 1, b: "two" }          ← before the bytes on disk were edited
//   { a: 1, b: "XXX" }          ← after; SAME command, SAME CAID printed
//
// The engine prints an identity and then hands back a value that is not
// that identity. `read_object` resolves a path from the digest and returns
// whatever sits there; nothing ever re-addresses the content. Content
// addressing exists precisely so that identity does not depend on where the
// bytes live, and the local store is the one place n/ forgets that.
//
// ── Why this is not a security patch (discussion 025 §8) ─────────────────
// Degree 0 (identity — are these bytes what they claim?) is where n/ is
// merciless: CAID is exact equality, no blur, no tolerance. Degree ≥1
// (semantics — given they are what they claim, do they agree?) is where
// disagreement is content rather than error. The mercilessness at degree 0
// is what FUNDS the permissiveness at degree ≥1. An engine that trusts bytes
// at degree 0 attaches an unwritten condition to every semantic guarantee
// above it: "provided nobody touched the filesystem." This arc removes that
// condition. It is the floor under "conflict is not an error", not a lock.
//
// ── Measured before writing (先量後寫) ───────────────────────────────────
//
// ROUND-TRIP STABILITY — the question that could have vetoed the arc: if
// stored objects do not reproduce their own address on read-back, switching
// verification on bricks every existing store. Ran the WHOLE conformance
// corpus (143 vectors) into one store and re-addressed every object:
// 125/125 parseable objects stable. Verification is safe to enable.
//
// Storage addressing uses the UNSALTED `content_hash()`. `get_horizon_salt()`
// (SystemTime::now) feeds only `content_hash_with_salt` at lib.rs:1889, an
// observation-surface `%id`. So `#blur` does not make a stored address
// nondeterministic — checked, because it would have been fatal.
//
// COMMITS ARE v1, VALUES ARE v2. `Commit::content_hash` returns
// `ContentHash::v1(digest)` — no sketch. So commit verification is digest-only
// by construction, and REAL_03 §9.2's spectral clause applies to values.
//
// THE ONE UNREADABLE OBJECT. Of 126 objects, one could not be deserialized
// at all: 5,091,205 bytes, JSON depth 646, past serde_json's 128 default.
// Source: `conformance/L2/20-deep-recursion-type.n` — a SHIPPED, PASSING
// vector, at DEFAULT configuration. Two lines are enough:
//     @Tree: { v: @int, next: @Tree | () }
//     out: 1
// Located by bisection: the expansion needs neither the meet nor the
// navigation, only the type definition; and it splits in two —
//     oo evolve + commit  → 260 KB, next×3,   JSON depth 20   (readable)
//     oo run              → 5.09 MB, next×128, JSON depth 646 (NOT readable)
// The extra 20× comes from run_one_shot's store-put loop (main.rs:590),
// which observes every top-level bare-path field and puts it. `observe`
// forces. SPEC_04 §158 says navigation must stay lazy and must NOT force for
// absorption, and names this exact type as the divergence case:
//   「導航保持惰性，不得為吸收而強制固化 union(否則遞迴型 `@Tree | ()`
//     導航發散)」
// SPEC_12's recursion table says structural recursion is `#recursive_lazy`,
// 「惰性展開，不觸發發散」. So this is a spec violation, not a design choice.
// (Observing `@Tree` itself does not terminate within 5 minutes — §158's
// "would diverge" is not hypothetical, merely unvisited by any vector. L2-20
// is titled 遞迴型別終止 and observes `t.v`; it tests termination, and the
// value path is genuinely fine. Laziness is the clause nothing tests.)
//
// BLAST RADIUS of dropping the loop, measured by disabling it and running
// everything: workspace 1450/0/3, conformance 143/143, and the unreadable
// object disappears. NOTHING in the suite or the corpus depends on it.
//
// ── Rulings ───────────────────────────────────────────────────────────────
// R-1 (user, 2026-07-26): this arc = A3 (read verification) + defect 2 (the
//     forcing store-put loop). Defect 1 — eager expansion in the type layer,
//     the 260 KB at evolve — is ledgered, not fixed here. It reaches type
//     unification and possibly CAIDs and vectors; it is its own arc.
//
// R-2 (acceptor, stated for veto): fix defect 2 by REMOVING the store-put
//     loop, not by making it lazy. Reasons, in order of weight:
//       * `oo run` is a one-shot PURE universe (settled by measurement in the
//         #pin arc; `oo evolve` is the persistent-universe command). A command
//         whose contract is "no durable state" writing durable state is a
//         category error, independent of laziness.
//       * Its outputs are orphans by construction — no commit references them,
//         nothing enumerates them. It manufactures precisely the garbage the
//         GC arc will have to sweep.
//       * Nothing depends on it (measured above).
//       * The capability it provides already exists deliberately, as
//         `~%Engine./save`. Making it explicit rather than automatic is the
//         same ruling the #squash arc reached about forgetting: the things
//         that matter should be asked for.
//     Fallback if vetoed: keep the loop but store the staged value without
//     observing. That preserves the stated intent lazily, at the cost of
//     storing thunk-bearing values whose CAIDs are the lazy ones.
//
// R-3 (acceptor, stated for veto): verify the FULL v2 CAID for values —
//     digest AND lattice_sketch AND masa_ref. REAL_03 §9.2 gives engines
//     without spectral support a digest-only door; this engine has spectral
//     support, and §9.2's second bullet then says it 應同時驗證譜特徵與內容
//     指紋的一致性. `hash_to_path` keys on the digest alone, so a CAID whose
//     digest is real but whose sketch is forged resolves today — that is the
//     hole §9.2 is describing, and it is directly testable.
//
// R-4 (acceptor, stated for veto): THREE outcomes, not a boolean.
//       verified   — recomputed address equals the requested one
//       corrupt    — recomputed address differs; the bytes are lying
//       unreadable — cannot deserialize; corruption CANNOT be ruled out, and
//                    the engine must say so rather than guess
//     Today all three collapse into one string: `run_inspect` maps every
//     `get_value` error to "CAID not found in local store". A verifier that
//     cannot separate corruption from a legitimately-undecodable object is
//     not worth switching on — the same legibility rule the v0.2.41 arc
//     reached about audit faces.
//
// ── Do not touch ──────────────────────────────────────────────────────────
// The raw read layer is deliberately raw. effect_cached_probe_test's header
// records why: `#cached` solidification hooks the USER-FACING fetch-by-CAID
// boundary only, never raw `get_value`, so that commit-root reconstruction,
// refine monotonicity and content_hash comparisons stay bit-exact (REAL_04
// determinism). Verification is READ-ONLY and does not conflict — but it is
// the same function, so: verify, and change nothing else there.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

fn fresh_dir() -> PathBuf {
    let mut d = std::env::temp_dir();
    d.push(format!(
        "nlang-cas-{}-{}",
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
    if let Some(p) = dir.join(name).parent() {
        fs::create_dir_all(p).ok();
    }
    fs::write(dir.join(name), src).unwrap();
}

/// A universe with one commit, so the store exists and HEAD is set.
fn seeded(dir: &Path) {
    write(dir, "s.n", "x: 1\n");
    oo(dir, &["evolve", "s.n"]);
    oo(dir, &["commit", "-m", "seed"]);
}

/// Store `value` through the deliberate path and return its full v2 CAID.
fn save(dir: &Path, value: &str) -> String {
    let out = oo(dir, &["eval", &format!("~%Engine./save({value})")]);
    out.trim()
        .trim_start_matches('"')
        .split('"')
        .next()
        .unwrap_or("")
        .to_string()
}

fn digest_of(caid: &str) -> String {
    caid.rsplit(':').next().unwrap().to_string()
}

fn object_path(dir: &Path, caid: &str) -> PathBuf {
    let hex = digest_of(caid);
    dir.join(".oo")
        .join("objects")
        .join("sha256")
        .join(&hex[0..2])
        .join(&hex[2..])
}

/// Edit an object's bytes in place, leaving its filename — the digest — alone.
fn tamper(dir: &Path, caid: &str, from: &str, to: &str) {
    let p = object_path(dir, caid);
    let s = fs::read_to_string(&p).unwrap();
    let s2 = s.replacen(from, to, 1);
    assert_ne!(s, s2, "tamper precondition: {from:?} must occur in the object");
    fs::write(&p, s2).unwrap();
}

/// Every object currently in the store.
fn objects(dir: &Path) -> Vec<PathBuf> {
    fn walk(d: &Path, out: &mut Vec<PathBuf>) {
        if let Ok(rd) = fs::read_dir(d) {
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, out);
                } else {
                    out.push(p);
                }
            }
        }
    }
    let mut v = Vec::new();
    walk(&dir.join(".oo").join("objects"), &mut v);
    v
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — verification
// ─────────────────────────────────────────────────────────────────────────

#[test]
#[ignore]
fn red_tampered_object_is_refused_not_returned() {
    // The headline. Paired in one test: the SAME command on the SAME CAID
    // must succeed before the edit and refuse after it, so the gate cannot
    // pass by the read being broken generally.
    let d = fresh_dir();
    seeded(&d);
    let caid = save(&d, r#"{a: 1, b: "two"}"#);
    assert!(caid.starts_with("hash:sha256:v2:"), "save must return a v2 CAID: {caid:?}");

    let before = oo(&d, &["inspect", &caid]);
    assert!(before.contains("\"two\""), "control: the true value must read back: {before:?}");

    tamper(&d, &caid, "\"two\"", "\"XXX\"");
    let after = oo(&d, &["inspect", &caid]);
    assert!(
        !after.contains("XXX"),
        "a value whose bytes no longer match its address must NOT be handed back: {after:?}"
    );
    assert!(
        after.contains("corrupt") || after.contains("integrity") || after.contains("mismatch"),
        "the refusal must name corruption, not masquerade as absence: {after:?}"
    );
}

#[test]
#[ignore]
fn red_tampered_commit_object_is_refused() {
    // Commits are v1 (digest only) — `Commit::content_hash` returns
    // ContentHash::v1. Verification must cover them too: the commit chain is
    // the history, and a forged commit body is a forged history.
    let d = fresh_dir();
    seeded(&d);
    write(&d, "b.n", "y: 2\n");
    oo(&d, &["evolve", "b.n"]);
    oo(&d, &["commit", "-m", "second"]);

    let before = oo(&d, &["log"]);
    assert!(before.contains("second"), "control: the log must read: {before:?}");

    let head = fs::read_to_string(d.join(".oo").join("HEAD")).unwrap();
    tamper(&d, head.trim(), "second", "forged");
    let after = oo(&d, &["log"]);
    assert!(
        !after.contains("forged"),
        "a tampered commit must not be walked as if it were genuine: {after:?}"
    );
    // CONTROL: without this the gate also passes if verification simply broke
    // `oo log` outright, which would be a regression wearing the same face.
    assert!(
        !after.is_empty(),
        "the command must still report something: {after:?}"
    );
    assert!(
        after.contains("corrupt") || after.contains("integrity") || after.contains("mismatch"),
        "the refusal must name corruption rather than fail silently: {after:?}"
    );
}

#[test]
#[ignore]
fn red_corruption_absence_and_validity_are_three_outcomes() {
    // R-4. Today `run_inspect` maps every get_value error to "CAID not found
    // in local store", so corruption reads as absence. Three distinct states
    // must produce three distinguishable answers.
    let d = fresh_dir();
    seeded(&d);
    let good = save(&d, r#"{k: "keep"}"#);
    let doomed = save(&d, r#"{k: "doomed"}"#);
    tamper(&d, &doomed, "\"doomed\"", "\"edited\"");

    let absent = format!(
        "hash:sha256:v1:{}",
        "0".repeat(64)
    );

    let a = oo(&d, &["inspect", &good]);
    let b = oo(&d, &["inspect", &doomed]);
    let c = oo(&d, &["inspect", &absent]);

    // CALIBRATION: asserting only that the three answers differ is VACUOUS —
    // at baseline they already differ, because corruption silently returns
    // the edited value while absence errors. The gate has to pin the SHAPE of
    // each outcome, or the defect itself satisfies it.
    assert!(a.contains("keep"), "valid object must read back: {a:?}");
    assert!(
        !b.contains("edited"),
        "corrupt must not return content at all: {b:?}"
    );
    assert!(
        b.contains("corrupt") || b.contains("integrity") || b.contains("mismatch"),
        "corrupt must be NAMED as corruption: {b:?}"
    );
    assert!(
        c.contains("not found") || c.contains("absent"),
        "absence must still read as absence: {c:?}"
    );
    assert!(
        !b.contains("not found"),
        "corruption must not be reported as absence — a verifier that cannot \
         tell them apart says the same thing whether the store is intact or \
         forged: {b:?}"
    );
}

#[test]
#[ignore]
fn red_forged_sketch_in_the_requested_caid_is_refused() {
    // R-3 / REAL_03 §9.2. `hash_to_path` keys on the digest alone, so a CAID
    // that carries a real digest and a fabricated spectral sketch resolves to
    // the real object today. For a spectral-capable engine §9.2 requires the
    // sketch to be verified against the content too.
    let d = fresh_dir();
    seeded(&d);
    let caid = save(&d, r#"{a: 1, b: "two"}"#);

    let parts: Vec<&str> = caid.split(':').collect();
    assert_eq!(parts.len(), 6, "v2 CAID shape: hash:algo:ver:masa:sketch:digest — {caid:?}");
    let sketch = parts[4];
    let flipped: String = sketch
        .chars()
        .enumerate()
        .map(|(i, c)| if i == 0 { if c == 'A' { 'B' } else { 'A' } } else { c })
        .collect();
    let forged = format!(
        "{}:{}:{}:{}:{}:{}",
        parts[0], parts[1], parts[2], parts[3], flipped, parts[5]
    );
    assert_ne!(forged, caid, "the forged CAID must actually differ");

    let honest = oo(&d, &["inspect", &caid]);
    assert!(honest.contains("\"two\""), "control: the honest CAID must resolve: {honest:?}");

    let got = oo(&d, &["inspect", &forged]);
    assert!(
        !got.contains("\"two\""),
        "a CAID whose spectral sketch does not match the content must not \
         resolve to that content: {got:?}"
    );
}

#[test]
#[ignore]
fn red_forged_masa_ref_in_the_requested_caid_is_refused() {
    // Same hole, the other v2 component.
    let d = fresh_dir();
    seeded(&d);
    let caid = save(&d, r#"{a: 1, b: "two"}"#);
    let parts: Vec<&str> = caid.split(':').collect();
    assert_eq!(parts.len(), 6);
    let forged = format!(
        "{}:{}:{}:{}:{}:{}",
        parts[0], parts[1], parts[2], "deadbeef", parts[4], parts[5]
    );

    let honest = oo(&d, &["inspect", &caid]);
    assert!(honest.contains("\"two\""), "control: {honest:?}");
    let got = oo(&d, &["inspect", &forged]);
    assert!(
        !got.contains("\"two\""),
        "a forged masa_ref must not resolve to the real content: {got:?}"
    );
}

#[test]
#[ignore]
fn red_undecodable_object_is_its_own_outcome() {
    // R-4's third state, and the honest one. If an object cannot be
    // deserialized, verification never runs, so corruption CANNOT be ruled
    // out — and the engine must say that rather than pick a story. This is
    // not hypothetical: the measurement found exactly one such object, from a
    // shipped conformance vector at default configuration.
    let d = fresh_dir();
    seeded(&d);
    let good = save(&d, r#"{k: "keep"}"#);
    let victim = save(&d, r#"{k: "victim"}"#);

    // Valid JSON, far past serde_json's nesting limit.
    let deep = format!("{}{}{}", "[".repeat(400), "1", "]".repeat(400));
    fs::write(object_path(&d, &victim), deep).unwrap();

    let ok = oo(&d, &["inspect", &good]);
    assert!(ok.contains("keep"), "control: {ok:?}");

    let got = oo(&d, &["inspect", &victim]);
    assert!(
        !got.is_empty(),
        "the command must report something for an undecodable object"
    );
    assert!(
        !got.contains("not found"),
        "an object that is PRESENT but undecodable must not be reported as \
         absent — that is the conflation this arc exists to break: {got:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — defect 2, the forcing store-put loop
// ─────────────────────────────────────────────────────────────────────────

#[test]
#[ignore]
fn red_a_run_does_not_force_a_recursive_type() {
    // SPEC_04 §158 names `@Tree | ()` as the case that must not be forced;
    // SPEC_12 says structural recursion is #recursive_lazy. Measured at
    // baseline: this two-line program leaves a 5,091,205-byte object.
    // Paired with the answer, so "the run stopped working" cannot pass it.
    let d = fresh_dir();
    seeded(&d);
    write(
        &d,
        "t.n",
        "@Tree: { v: @int, next: @Tree | () }\nout: 1\n",
    );
    let got = oo(&d, &["run", "t.n", "-o", "out"]);
    assert!(got.contains('1'), "the program must still answer: {got:?}");

    let biggest = objects(&d)
        .iter()
        .map(|p| fs::metadata(p).map(|m| m.len()).unwrap_or(0))
        .max()
        .unwrap_or(0);
    assert!(
        biggest < 100_000,
        "a two-line recursive type definition must not be forced into a \
         multi-megabyte object (largest was {biggest} bytes)"
    );
}

#[test]
#[ignore]
fn red_every_object_a_run_leaves_can_be_read_back() {
    // The general invariant behind the specific case: the store must not
    // contain anything the engine cannot read. Write-only objects are
    // invisible to verification AND to any future reachability sweep.
    let d = fresh_dir();
    seeded(&d);
    write(
        &d,
        "t.n",
        "@Tree: { v: @int, next: @Tree | () }\nt: { v: 1, next: () } & @Tree\nout: t.v\n",
    );
    oo(&d, &["run", "t.n", "-o", "out"]);

    for p in objects(&d) {
        let len = fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
        let depth = {
            let s = fs::read_to_string(&p).unwrap_or_default();
            let (mut cur, mut max) = (0i32, 0i32);
            for ch in s.chars() {
                match ch {
                    '{' | '[' => {
                        cur += 1;
                        max = max.max(cur);
                    }
                    '}' | ']' => cur -= 1,
                    _ => {}
                }
            }
            max
        };
        assert!(
            depth < 128,
            "object {p:?} ({len} bytes) nests {depth} deep — past serde_json's \
             default limit, so the engine can write it but never read it back"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────
// PINS — green at baseline, must stay green
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn pin_ordinary_store_traffic_still_works() {
    // Verification sits on the hot path: universe load, commit, log, history
    // ops all go through get_value/get_commit.
    let d = fresh_dir();
    for i in 1..=3 {
        write(&d, "s.n", &format!("f{i}: {i}\n"));
        oo(&d, &["evolve", "s.n"]);
        oo(&d, &["commit", "-m", &format!("c{i}")]);
    }
    let log = oo(&d, &["log"]);
    let caids: Vec<&str> = log
        .lines()
        .filter_map(|l| l.strip_prefix("commit "))
        .map(str::trim)
        .collect();
    assert_eq!(caids.len(), 3, "three commits must be walkable: {log:?}");

    let r = oo(&d, &["rollback", caids[2], "--grant", "rollback"]);
    assert!(!r.contains("error"), "rollback must still work: {r:?}");
    assert!(oo(&d, &["status"]).contains("static"));
}

#[test]
fn pin_explicit_save_and_inspect_roundtrip() {
    // `~%Engine./save` is the deliberate way to put a value in the store, and
    // it must survive this arc — under R-2 it becomes the ONLY way a program
    // does so on purpose.
    let d = fresh_dir();
    seeded(&d);
    let caid = save(&d, r#"{a: 1, b: "two"}"#);
    assert!(caid.starts_with("hash:sha256:v2:"), "{caid:?}");
    let got = oo(&d, &["inspect", &caid]);
    assert!(got.contains("\"two\"") && got.contains("a: 1"), "{got:?}");
}

#[test]
fn pin_conformance_shaped_values_all_read_back() {
    // A spread of value shapes through the store. Round-trip stability across
    // the full corpus was measured before this arc (125/125); this keeps a
    // sample of it under test.
    let d = fresh_dir();
    seeded(&d);
    for v in [
        "{i: 1, f: 2.5, s: \"hi\", t: #tag}",
        "[1, 2, 3]",
        "{c: {x: 1, y: {z: \"deep\"}}}",
        "{u: (1 | 2)}",
        "{ty: @int}",
        "{neg: -5, big: 123456789012345678901234567890}",
        "{{ p: 1 }}",
    ] {
        let caid = save(&d, v);
        assert!(caid.starts_with("hash:"), "save failed for {v}: {caid:?}");
        let got = oo(&d, &["inspect", &caid]);
        assert!(
            !got.contains("not found") && !got.is_empty(),
            "{v} must read back: {got:?}"
        );
    }
}

#[test]
fn pin_genesis_addresses_are_unchanged() {
    // Verification must not move a single address. If it does, every existing
    // store stops resolving.
    let d = fresh_dir();
    seeded(&d);
    let caid = save(&d, r#"{a: 1, b: "two"}"#);
    assert_eq!(
        digest_of(&caid).len(),
        64,
        "digest must stay a 32-byte sha256: {caid:?}"
    );
    let again = save(&d, r#"{a: 1, b: "two"}"#);
    assert_eq!(caid, again, "the same value must address identically");
}
