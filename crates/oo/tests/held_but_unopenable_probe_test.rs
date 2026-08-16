// Held but unopenable (Q-031 = Q-029 layers two and three, pre-committed by
// work order: docs/held_but_unopenable_handover.md).
//
// ── The claim ────────────────────────────────────────────────────────────
//
// Q-029's first layer put a gate on the universe: an engine that cannot open
// a store's root now refuses to write to it. Underneath that gate, five call
// sites still fold "I hold these bytes and cannot open them" into a different
// answer, because the refusal is an `anyhow!` string rather than a
// `StoreReadError`, so `downcast_ref` yields `None` and every one of them
// merges `None` into the `NotFound` arm.
//
//   universe.rs:989   Ok(None)                      refine's monotonicity
//                                                   check is skipped
//   universe.rs:1063  break                         shadow scan truncates
//   universe.rs:1082  continue                      shadow scan skips a commit
//   disc.rs:200       Err(false) = "not there"      a held object is called absent
//   oodp.rs:388       refuse(NotFound, "not_held")  a false claim, on the wire
//
// Ruled 2026-08-16 (O59): the shadow scan must abort the whole operation and
// say so, and the wire answer is `#not_found` with a NEW `%reason`,
// `#standard_root_unavailable` -- the status set does not grow (O57-C).
//
// ── What "present but unopenable" is, and why the harness can build one ──
//
// It is an object whose bytes are on disk and whose root names a standard
// root this build does not ship. Measured 2026-08-16: the read path resolves
// the standard root BEFORE verifying an object's address, so a root object
// written at an address that does not match its bytes still answers
// `refusing root: … is unavailable` rather than `#caid_mismatch`. C0 asserts
// exactly that, because if it ever stopped being true every probe here would
// be refusing for the wrong reason.
//
// Commit objects are different -- measured, they ARE address-verified before
// use -- so `mixed_history` places each fabricated commit at its own
// recomputed address, in two passes, ancestor first.
//
// ── The discriminator ────────────────────────────────────────────────────
//
// C1 is not decoration. An operand that is genuinely NOT HELD lets refine
// proceed, by design (REAL_03 §9.1, opacity). What is wrong is the
// conflation, not the skip. A repair that makes refine refuse for absent
// operands too has broken the design instead of fixing the defect, and C1 is
// the only thing standing between this arc and that outcome.
//
// ── Probe integrity ──────────────────────────────────────────────────────
//
// Reds are `#[ignore]`d and MUST be red at the baseline for the reason stated
// in each. The delivery may remove `#[ignore]` and NOTHING else in this file.
// Assertions pin behaviour, never the wording of a refusal.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

const ZERO: &str = "0000000000000000000000000000000000000000000000000000000000000000";
/// Well-formed, correct length, and no object anywhere behind it.
const ABSENT: &str =
    "hash:sha256:v1:1111111111111111111111111111111111111111111111111111111111111111";

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

fn objects(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(top) = std::fs::read_dir(dir.join(".oo/objects/sha256")) {
        for a in top.flatten() {
            if let Ok(inner) = std::fs::read_dir(a.path()) {
                for b in inner.flatten() {
                    out.push(b.path());
                }
            }
        }
    }
    out.sort();
    out
}

fn addr_of(p: &Path) -> String {
    format!(
        "{}{}",
        p.parent().unwrap().file_name().unwrap().to_string_lossy(),
        p.file_name().unwrap().to_string_lossy()
    )
}

fn caid(addr: &str) -> String {
    format!("hash:sha256:v1:{addr}")
}

/// The store's own root object (the one carrying a standard-root sentinel).
fn own_root(dir: &Path) -> (PathBuf, String) {
    let mut found = None;
    for p in objects(dir) {
        let s = std::fs::read_to_string(&p).unwrap_or_default();
        // Skip anything the harness itself planted: after `plant_unopenable_root`
        // the store holds two objects carrying a sentinel, and only one of them
        // is the store's own.
        if s.contains("__nlang_system_digest") && !s.contains(ZERO) {
            assert!(found.is_none(), "harness: more than one root object");
            found = Some((p.clone(), s));
        }
    }
    found.expect("harness: the store must have a root object")
}

fn write_object(dir: &Path, addr: &str, body: &str) -> PathBuf {
    let d = dir.join(".oo/objects/sha256").join(&addr[0..2]);
    std::fs::create_dir_all(&d).unwrap();
    let p = d.join(&addr[2..]);
    std::fs::write(&p, body).unwrap();
    p
}

fn store(tag: &str) -> nlang_interpreter::ScratchDir {
    let d = nlang_interpreter::ScratchDir::new(&format!("unopenable-{tag}"));
    std::fs::write(d.join("a.n"), "app: { k1: 1 }\n").unwrap();
    oo(&d, &["evolve", "a.n"]);
    let out = oo(&d, &["commit", "-m", "one"]);
    assert!(
        out.contains("Commit successful"),
        "harness: the store must have a commit, got: {out}"
    );
    d
}

/// Put a copy of the store's root into it, naming a standard root nobody
/// ships. Returns that object's CAID. The store itself stays openable.
fn plant_unopenable_root(dir: &Path) -> String {
    let (_, body) = own_root(dir);
    let real = body
        .split('"')
        .find(|t| t.len() == 64 && t.chars().all(|c| c.is_ascii_hexdigit()))
        .expect("harness: could not read the sentinel digest")
        .to_string();
    assert_ne!(real, ZERO, "harness: the store already names 0000…");
    let addr = format!("ab{}", "c".repeat(62));
    write_object(dir, &addr, &body.replace(&real, ZERO));
    caid(&addr)
}

/// Build history whose HEAD opens but whose ancestor does not.
///
/// Two passes because a commit is address-verified: the ancestor must reach
/// its final address before the head can name it.
fn mixed_history(dir: &Path) -> String {
    let bad_root = plant_unopenable_root(dir);
    let bad_addr = bad_root.rsplit(':').next().unwrap().to_string();

    let commit_path = objects(dir)
        .into_iter()
        .find(|p| !std::fs::read_to_string(p).unwrap_or_default().contains("\"Combo\""))
        .expect("harness: the store must have a commit object");
    let commit: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&commit_path).unwrap()).unwrap();

    let hash_struct = |addr: &str| {
        let mut h = commit["root"].clone();
        h["digest"] = serde_json::Value::Array(
            hex::decode(addr)
                .unwrap()
                .into_iter()
                .map(|b| serde_json::Value::from(b))
                .collect(),
        );
        h
    };
    let recomputed = |dir: &Path| -> String {
        let out = oo(dir, &["log"]);
        let i = match out.find("recomputed hash:sha256:v1:") {
            Some(i) => i + "recomputed hash:sha256:v1:".len(),
            None => panic!(
                "harness: a fabricated commit must be reported as a mismatch so \
                 its true address can be read back; `oo log` said: {out}"
            ),
        };
        out[i..i + 64].to_string()
    };
    let set_head = |dir: &Path, addr: &str| {
        std::fs::write(dir.join(".oo/HEAD"), caid(addr)).unwrap();
    };

    let mut ancestor = commit.clone();
    ancestor["parent"] = serde_json::Value::Null;
    ancestor["meta"]["message"] = serde_json::Value::from("ancestor");
    ancestor["root"] = hash_struct(&bad_addr);
    let tmp = format!("ba{}", "d".repeat(62));
    let p = write_object(dir, &tmp, &ancestor.to_string());
    set_head(dir, &tmp);
    let a_addr = recomputed(dir);
    std::fs::remove_file(p).unwrap();
    write_object(dir, &a_addr, &ancestor.to_string());

    let mut head = commit.clone();
    head["parent"] = hash_struct(&a_addr);
    head["meta"]["message"] = serde_json::Value::from("head");
    let tmp = format!("ce{}", "a".repeat(62));
    let p = write_object(dir, &tmp, &head.to_string());
    set_head(dir, &tmp);
    let b_addr = recomputed(dir);
    std::fs::remove_file(p).unwrap();
    write_object(dir, &b_addr, &head.to_string());
    set_head(dir, &b_addr);
    caid(&b_addr)
}

fn head_of(dir: &Path) -> String {
    std::fs::read_to_string(dir.join(".oo/HEAD")).unwrap_or_default().trim().into()
}

// ── C0, C1 ── the harness and the discriminator ──────────────────────────

/// Green at the baseline. The planted object is HELD (its bytes are on disk)
/// and UNOPENABLE (the engine says so by name), while the store around it
/// still opens. If the read path ever begins verifying an object's address
/// before resolving its standard root, this fails first and every probe below
/// is disarmed rather than silently testing something else.
#[test]
fn c0_the_harness_plants_an_object_that_is_held_and_unopenable() {
    let d = store("c0");
    let planted = plant_unopenable_root(&d);

    let st = oo(&d, &["status"]);
    assert!(
        st.contains("(available)"),
        "harness: the store itself must stay openable, got: {st}"
    );
    let out = oo(&d, &["inspect", &planted]);
    assert!(
        out.contains("refusing root") && out.contains(ZERO),
        "harness: the planted object must be refused BY STANDARD ROOT, not by \
         address, got: {out}"
    );
    let absent = oo(&d, &["inspect", ABSENT]);
    assert!(
        absent.contains("not found in local store"),
        "harness: the contrast case must be a plain miss, got: {absent}"
    );
}

/// Green at the baseline and must STAY green. An operand that is genuinely not
/// held lets `refine` proceed -- REAL_03 §9.1, opacity, by design. The defect
/// this arc closes is the CONFLATION of "unopenable" with "absent", so a
/// repair that also stops refine for absent operands has changed the design.
#[test]
fn c1_an_operand_that_is_genuinely_absent_still_lets_refine_proceed() {
    let d = store("c1");
    let (root, _) = own_root(&d);
    let mine = caid(&addr_of(&root));
    let out = oo(&d, &["refine", "--source", ABSENT, "--target", &mine, "-m", "x"]);
    assert!(
        out.contains("Refine commit"),
        "an absent operand is opaque, not an error: {out}"
    );
}

// ── P1 ── universe.rs:989 ────────────────────────────────────────────────

/// RED at the baseline: `refine` commits. The operand could not be opened, so
/// it took the "not held, opaque" arm and the geometric monotonicity check
/// never ran -- the fail-open that site's own comment names.
#[test]
#[ignore = "baseline: `Refine commit: …` and HEAD moves — measured 2026-08-16 on v0.24.1"]
fn p1_refine_refuses_an_operand_it_holds_but_cannot_open() {
    let d = store("p1");
    let planted = plant_unopenable_root(&d);
    let (root, _) = own_root(&d);
    let mine = caid(&addr_of(&root));
    let before = head_of(&d);
    assert!(before.starts_with("hash:sha256:"), "presence: HEAD is a CAID");

    let out = oo(&d, &["refine", "--source", &planted, "--target", &mine, "-m", "x"]);
    assert!(
        !out.contains("Refine commit"),
        "refine must not commit against an operand it cannot check: {out}"
    );
    assert!(
        out.contains(ZERO),
        "the refusal must name the standard root it lacks: {out}"
    );
    assert_eq!(before, head_of(&d), "HEAD must not move");
}

// ── P2 ── universe.rs:1063 / :1082 ───────────────────────────────────────

/// RED at the baseline: `refine` commits and moves HEAD while the shadow scan
/// walks into an ancestor whose root cannot be opened and silently stops.
///
/// O59-B: abort the whole operation and say so. A scan that cannot finish
/// gives an answer that is wrong in a way its caller cannot see.
#[test]
#[ignore = "baseline: `Refine commit: …`, HEAD moves, nothing said — measured 2026-08-16 on v0.24.1"]
fn p2_refine_aborts_when_the_shadow_scan_meets_a_root_it_cannot_open() {
    let d = store("p2");
    let _head_caid = mixed_history(&d);
    let (root, _) = own_root(&d);
    let mine = caid(&addr_of(&root));

    let log = oo(&d, &["log"]);
    assert!(
        log.matches("commit hash:sha256:").count() >= 2,
        "presence: the history must actually reach the ancestor: {log}"
    );
    let before = head_of(&d);

    let out = oo(&d, &["refine", "--source", &mine, "--target", &mine, "-m", "x"]);
    assert!(
        !out.contains("Refine commit"),
        "refine must abort rather than scan half the history: {out}"
    );
    assert!(
        out.contains(ZERO),
        "the abort must name the standard root it lacks: {out}"
    );
    assert_eq!(before, head_of(&d), "HEAD must not move");
}

// ── P3 ── disc.rs:200 ────────────────────────────────────────────────────

/// RED at the baseline: `disc.fetch` answers the same thing for an object it
/// holds but cannot open as for one that is not there.
///
/// The assertion is the difference itself, not either answer -- naming what
/// each should be would pin a spelling this arc has not ruled on.
#[test]
#[ignore = "baseline: both answers are identical — measured 2026-08-16"]
fn p3_a_local_fetch_tells_unopenable_apart_from_absent() {
    use nlang_interpreter::Ouroboros;
    let d = store("p3");
    let planted = plant_unopenable_root(&d);

    let engine = Ouroboros::init(&d).expect("engine opens the store");
    let mut ctx = engine.eval_context();
    let call = |s: &str, ctx: &mut _| {
        let f = engine.builtin_registry.get("disc.fetch").unwrap().clone();
        f(
            nlang_interpreter::value::Value::Atom(
                nlang_parser::ast::AtomKind::Str(s.to_string()),
                nlang_interpreter::value::EffectTag::Pure,
                None,
            ),
            &engine,
            ctx,
        )
    };
    let unopenable = format!("{:?}", call(&planted, &mut ctx));
    let absent = format!("{:?}", call(ABSENT, &mut ctx));
    assert_ne!(
        unopenable, absent,
        "an object this node HOLDS must not answer the same as one it does not"
    );
}

// ── P4 ── oodp.rs:388 ────────────────────────────────────────────────────

/// RED at the baseline: the node answers `#not_held` about bytes it holds.
///
/// O59-A: `#not_found` with a new `%reason: #standard_root_unavailable`.
/// The status must NOT change -- REAL_02 §130 forbids the status set growing,
/// and the caller's remedy really is to ask a different node.
#[test]
#[ignore = "baseline: answers `#not_held` — measured 2026-08-16 on v0.24.1"]
fn p4_the_wire_does_not_claim_absence_for_bytes_the_node_holds() {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::time::Duration;

    let d = store("p4");
    let planted = plant_unopenable_root(&d);
    let mut node = common::serve(oo_cmd(&d), d.join("serve.log"));

    let ask = |body: String| -> String {
        let mut s = TcpStream::connect(("127.0.0.1", node.port)).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        s.write_all(format!("{body}\n").as_bytes()).unwrap();
        s.flush().unwrap();
        let mut buf = String::new();
        let _ = s.read_to_string(&mut buf);
        buf
    };
    let held = ask(format!(
        r#"{{ %op: #fetch, %hash: "{planted}", %from: "x" }}"#
    ));
    let absent = ask(format!(r#"{{ %op: #fetch, %hash: "{ABSENT}", %from: "x" }}"#));
    let _ = node.child.kill();

    assert!(
        absent.contains("not_held"),
        "presence: a genuine miss must still answer #not_held, got: {absent}"
    );
    assert!(
        held.contains("standard_root_unavailable"),
        "bytes the node holds must not be reported as absent, got: {held}"
    );
}
