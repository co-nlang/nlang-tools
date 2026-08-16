// A digest that was not there (Q-030, pre-committed by work order:
// docs/a_digest_that_was_not_there_handover.md).
//
// ── The claim ────────────────────────────────────────────────────────────
//
// `hash:sha256:v1:` -- a CAID whose digest is the empty string -- parses.
// `ContentHash::parse` checks three things: at least four colon-separated
// parts, the `hash:sha256` prefix, and that the digest is hex-decodable.
// `hex::decode("")` returns `Ok(vec![])`, so an empty digest is a valid CAID
// and nothing anywhere checks its length. `storage.rs:476` then slices its
// first two characters and the process dies.
//
// Measured 2026-08-16, and it predates the Q-029 delivery -- the same input
// reproduces on the v0.24.0 baseline binary.
//
// Reach, measured, not inferred:
//   online   #fetch                       kills the node process
//            #discover                    answers normally
//            #find_node                   answers #malformed (correct)
//   local    inspect / rollback / refine  panic
//            squash                       stopped by the ancestor check first
//            node discover / find-node    never touch the store
//
// The node dies whole, not per-connection: `main.rs:521` is
// `for stream in listener.incoming()` with no spawn, and there is no
// `catch_unwind` or `panic::set_hook` anywhere in `crates/oo/src/`.
// That -- panic isolation -- is deliberately NOT this arc (ruled
// 2026-08-16); this file only witnesses that the input stops arriving.
//
// ── Why the fix belongs in `parse` ───────────────────────────────────────
//
// The correct online answer is already written and merely unreachable:
// `oodp.rs:371` answers `#conflict %reason: unparseable_caid` when `%hash`
// is present but did not parse. Making `parse` reject an empty digest makes
// that existing branch fire. P1 asserts exactly that string, so a repair that
// invents a new answer instead of reaching the existing one will not pass.
//
// The length rule is the general form: sha256 digests are 32 bytes, and
// today `hash:sha256:v1:ab` is accepted as one (P4/P5).
//
// ── Probe integrity ──────────────────────────────────────────────────────
//
// Reds are `#[ignore]`d and MUST be red at the baseline for the reason
// stated in each. The delivery may remove `#[ignore]` and NOTHING else in
// this file. C0 runs first: a node that never came up cannot witness that it
// survived anything.

mod common;

use std::path::Path;
use std::process::Command;

const EMPTY_V1: &str = "hash:sha256:v1:";
const EMPTY_V2: &str = "hash:sha256:v2:_:AAA:";
const SHORT_V1: &str = "hash:sha256:v1:ab";
const GOOD_V1: &str =
    "hash:sha256:v1:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn oo_cmd(dir: &Path) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_oo"));
    c.current_dir(dir)
        .env("OO_IDENTITY", dir.join("identity-for-tests"))
        .env("OO_NODE_HOME", dir.join("node-home-for-tests"));
    c
}

fn oo(dir: &Path, args: &[&str]) -> String {
    oo_ok(dir, args).0
}

/// Output plus whether the command succeeded.
///
/// The probes below assert on BEHAVIOUR, never on the wording of a refusal.
/// The first draft of this file asserted `contains("Invalid CAID")`, which the
/// messages did not contain, and the delivery changed the messages to match --
/// a probe steering the product. Pin what the command DID: it failed, it never
/// reached the store, it did not panic.
fn oo_ok(dir: &Path, args: &[&str]) -> (String, bool) {
    let o = oo_cmd(dir).args(args).output().expect("oo runs");
    (
        format!(
            "{}{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        ),
        o.status.success(),
    )
}

/// A malformed CAID must be rejected before any lookup happens. A well-formed
/// but absent one produces this line; a malformed one must not.
const REACHED_THE_STORE: &str = "not found in local store";

/// A store with one commit in it, so every probe below asks a live store.
fn store(tag: &str) -> nlang_interpreter::ScratchDir {
    let d = nlang_interpreter::ScratchDir::new(&format!("nodigest-{tag}"));
    std::fs::write(d.join("a.n"), "app: { k1: 1 }\n").unwrap();
    oo(&d, &["evolve", "a.n"]);
    let out = oo(&d, &["commit", "-m", "one"]);
    assert!(
        out.contains("Commit successful"),
        "harness: the store must have a commit, got: {out}"
    );
    d
}

/// Send one OODP request line and read the reply. `None` = the node did not
/// answer, which for these probes means it is gone.
fn ask(port: u16, body: &str) -> Option<String> {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::time::Duration;
    let mut s = TcpStream::connect(("127.0.0.1", port)).ok()?;
    s.set_read_timeout(Some(Duration::from_secs(3))).ok()?;
    s.write_all(format!("{body}\n").as_bytes()).ok()?;
    s.flush().ok()?;
    let mut buf = String::new();
    match s.read_to_string(&mut buf) {
        Ok(0) => None,
        Ok(_) => Some(buf),
        Err(_) if buf.is_empty() => None,
        Err(_) => Some(buf),
    }
}

// ── C0 ── the harness ────────────────────────────────────────────────────

/// Green at the baseline. A node comes up, answers a well-formed `#fetch`
/// for an object it does not hold, and is still running afterwards. Every
/// "it survived" below is worthless without this.
#[test]
fn c0_a_node_answers_a_well_formed_fetch_and_stays_up() {
    let d = store("c0");
    let mut node = common::serve(oo_cmd(&d), d.join("serve.log"));
    let reply = ask(
        node.port,
        &format!(r#"{{ %op: #fetch, %hash: "{GOOD_V1}", %from: "x" }}"#),
    )
    .expect("control: a well-formed fetch must be answered");
    assert!(
        reply.contains("#not_held"),
        "control: an absent object answers #not_held, got: {reply}"
    );
    assert!(
        node.child.try_wait().unwrap().is_none(),
        "control: the node must still be running: {}",
        common::read_log(&node.log)
    );
    let _ = node.child.kill();
}

// ── P1 ── the remote half ────────────────────────────────────────────────

/// RED at the baseline: the node process dies, answering nothing.
///
/// The assertion names the exact reply because `oodp.rs:371` already
/// contains it. A repair that reaches that branch passes; one that invents a
/// different answer has changed the protocol instead of fixing the parser.
#[test]
fn p1_an_empty_digest_over_the_wire_is_refused_and_the_node_survives() {
    let d = store("p1");
    let mut node = common::serve(oo_cmd(&d), d.join("serve.log"));
    let reply = ask(
        node.port,
        &format!(r#"{{ %op: #fetch, %hash: "{EMPTY_V1}", %from: "x" }}"#),
    );
    // Settle before asking whether it lived: a process that aborted on this
    // request is not reaped the instant the socket closes, and `try_wait`
    // would report it alive. Without this the red lands on the reply
    // assertion and names the wrong defect.
    std::thread::sleep(std::time::Duration::from_millis(500));
    let alive = node.child.try_wait().unwrap().is_none();
    let log = common::read_log(&node.log);
    let _ = node.child.kill();

    assert!(alive, "the node must survive the request; log: {log}");
    let reply = reply.expect("the node must answer, not hang up");
    assert!(
        reply.contains("unparseable_caid"),
        "the answer already written at oodp.rs:371 must be the one that fires, got: {reply}"
    );
    assert!(
        !log.contains("panicked"),
        "nothing may panic on this path; log: {log}"
    );
}

/// RED at the baseline for the same reason, one layer out: after a malformed
/// request the node must still serve the NEXT caller. Separate from P1
/// because a node can answer one request and then die.
#[test]
fn p2_a_node_still_serves_the_next_caller_after_a_malformed_digest() {
    let d = store("p2");
    let mut node = common::serve(oo_cmd(&d), d.join("serve.log"));
    let _ = ask(
        node.port,
        &format!(r#"{{ %op: #fetch, %hash: "{EMPTY_V1}", %from: "x" }}"#),
    );
    let second = ask(
        node.port,
        &format!(r#"{{ %op: #fetch, %hash: "{GOOD_V1}", %from: "x" }}"#),
    );
    let _ = node.child.kill();
    let second = second.expect("the node must still answer an ordinary request");
    assert!(
        second.contains("#not_held"),
        "the second caller must get the ordinary answer, got: {second}"
    );
}

// ── P3 ── the local half ─────────────────────────────────────────────────

/// RED at the baseline: `inspect`, `rollback` and `refine` all panic.
///
/// One probe for three sites because they share one call: each reaches
/// `hash_to_path` through the store. The assertion is per-command so the
/// failure message names which one.
#[test]
fn p3_no_cli_entry_panics_on_an_empty_digest() {
    let d = store("p3");
    let cases: [(&str, Vec<&str>); 3] = [
        ("inspect", vec!["inspect", EMPTY_V1]),
        ("rollback", vec!["rollback", EMPTY_V1, "--grant", "rollback"]),
        (
            "refine",
            vec!["refine", "--source", EMPTY_V1, "--target", EMPTY_V1, "-m", "x"],
        ),
    ];
    for (name, args) in cases {
        let (out, ok) = oo_ok(&d, &args);
        assert!(
            !out.contains("panicked"),
            "`oo {name}` must not panic, got: {out}"
        );
        assert!(!ok, "`oo {name}` must fail, got: {out}");
        assert!(
            !out.contains(REACHED_THE_STORE),
            "`oo {name}` must reject before any lookup, got: {out}"
        );
    }
}

// ── P4, P5 ── the general rule ───────────────────────────────────────────

/// RED at the baseline: `hash:sha256:v1:ab` is accepted as a sha256 CAID and
/// looked up in the store. A sha256 digest is 32 bytes; one byte is not a
/// short sha256, it is not one at all.
///
/// The red asserts a rejection, so it asserts an acceptance in the same run:
/// a full-length digest must still be looked up normally.
#[test]
fn p4_a_digest_of_the_wrong_length_is_not_a_sha256_caid() {
    let d = store("p4");
    let good = oo(&d, &["inspect", GOOD_V1]);
    assert!(
        good.contains(REACHED_THE_STORE),
        "presence: a full-length digest must still reach the store, got: {good}"
    );
    let (short, ok) = oo_ok(&d, &["inspect", SHORT_V1]);
    assert!(!ok, "a one-byte digest must fail, got: {short}");
    assert!(
        !short.contains(REACHED_THE_STORE),
        "a one-byte digest is not a sha256 CAID and must never be looked up, got: {short}"
    );
}

/// RED at the baseline: the v2 arm has the same hole, and it is a separate
/// `hex::decode` call on a separate field, so fixing v1 alone leaves it.
#[test]
fn p5_the_v2_form_rejects_an_empty_digest_too() {
    let d = store("p5");
    let (out, ok) = oo_ok(&d, &["inspect", EMPTY_V2]);
    assert!(!out.contains("panicked"), "v2 must not panic, got: {out}");
    assert!(!ok, "v2 must fail, got: {out}");
    assert!(
        !out.contains(REACHED_THE_STORE),
        "v2 must reject before any lookup, got: {out}"
    );
}

// ── C1 ── what must not regress ──────────────────────────────────────────

/// Green at the baseline and must STAY green. Two CAID forms that are
/// already rejected correctly, and one that is already accepted correctly.
/// A length rule written too tightly (or a regex that forgets v2) breaks
/// these before it breaks anything else.
#[test]
fn c1_the_forms_that_already_work_keep_working() {
    let d = store("c1");
    for bad in ["hash:sha256:v1:a", "hash:sha256:v1:zz", "hash:md5:v1:abcd"] {
        let (out, ok) = oo_ok(&d, &["inspect", bad]);
        assert!(!ok, "`{bad}` must stay rejected, got: {out}");
        assert!(
            !out.contains(REACHED_THE_STORE),
            "`{bad}` must stay rejected before any lookup, got: {out}"
        );
    }
    let out = oo(&d, &["inspect", GOOD_V1]);
    assert!(
        out.contains(REACHED_THE_STORE),
        "a well-formed absent CAID must stay a store miss, got: {out}"
    );
    // The store's own real CAID must still resolve -- the strongest control:
    // a length rule that rejects real digests would fail here first.
    let head = std::fs::read_to_string(d.join(".oo/HEAD")).unwrap_or_default();
    let head = head.trim();
    assert!(head.starts_with("hash:sha256:"), "presence: HEAD is a CAID");
    let (out, ok) = oo_ok(&d, &["inspect", head]);
    assert!(
        ok,
        "the store's own HEAD must stay a CAID this engine can resolve, got: {out}"
    );
}
