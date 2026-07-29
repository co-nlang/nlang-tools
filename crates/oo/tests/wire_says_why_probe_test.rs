// The wire says why (2026-07-29, pre-committed by work order:
// docs/wire_says_why_handover.md).
//
// REAL_02 §3.2 requires four things to be distinguishable, gives three codes,
// says so about itself, and adds: 「在裁定之前,本條不得被引用為已達成的保證。」
//
// ── What this file is really about ───────────────────────────────────────
//
// The server half is the smaller half. The larger half is that the *client*
// turns protocol-level answers into integrity verdicts:
//
//     peer answers #teapot          → integrity #undecodable, ⊥ #caid_mismatch
//     peer answers #not_implemented → integrity #mismatch,    ⊥ #caid_mismatch
//
// A peer newer than this client is recorded as serving undecodable objects,
// which is the worst possible direction: every future extension of the
// protocol makes older clients accuse newer nodes of corruption. Both are
// REAL_03 §6.6 `裁決必須為真` violations, the same family as the `oo inspect`
// false verdict repaired in v0.2.53.
//
// ── The stub peer ────────────────────────────────────────────────────────
//
// R5–R9 need a peer that answers a chosen status, including statuses this
// engine has never heard of. A real node cannot produce those — that is the
// point of forward compatibility — so the probes stand up a socket that says
// exactly one thing. It is the only way to test what this client does when
// the other end is from the future.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use std::fs;

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

// ── harness ─────────────────────────────────────────────────────────────

fn fresh_dir(tag: &str) -> PathBuf {
    let mut d = std::env::temp_dir();
    d.push(format!(
        "nlang-wire-{}-{}-{}",
        tag,
        std::process::id(),
        DIR_SEQ.fetch_add(1, Ordering::SeqCst)
    ));
    fs::remove_dir_all(&d).ok();
    fs::create_dir_all(&d).unwrap();
    d
}

fn oo_cmd(dir: &Path) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_oo"));
    c.current_dir(dir)
        .env("OO_IDENTITY", dir.join("identity-for-tests"))
        .env("OO_NODE_HOME", dir.join("node-home-for-tests"));
    c
}

/// stdout and stderr kept apart: the integrity log goes to stderr, and this
/// file is about telling those two apart.
fn oo_split(dir: &Path, args: &[&str]) -> (String, String) {
    let out = oo_cmd(dir).args(args).output().unwrap();
    (
        String::from_utf8_lossy(&out.stdout).trim().to_string(),
        String::from_utf8_lossy(&out.stderr).trim().to_string(),
    )
}

fn oo(dir: &Path, args: &[&str]) -> String {
    let (o, e) = oo_split(dir, args);
    format!("{o}{e}")
}

fn write(dir: &Path, name: &str, src: &str) {
    fs::write(dir.join(name), src).unwrap();
}

fn init(dir: &Path) {
    oo(dir, &["run", "--help"]);
    write(dir, "seed.n", "seed: { ok: #true }\n");
    oo(dir, &["run", "seed.n"]);
}

/// Store a value and return its CAID. `--observe` prints an effect annotation
/// after the value, so the CAID is the first quoted field and nothing else.
fn store(dir: &Path, expr: &str) -> String {
    write(dir, "st.n", &format!("v: ~%Discovery./identify_and_store {expr}\n"));
    let (out, _) = oo_split(dir, &["run", "st.n", "--observe", "v"]);
    let caid = out
        .split('"')
        .nth(1)
        .unwrap_or_else(|| panic!("no CAID in {out:?}"))
        .to_string();
    assert!(caid.starts_with("hash:sha256:"), "store() got {caid:?}");
    caid
}

/// Flip a byte in the middle of a stored object.
fn corrupt(dir: &Path, caid: &str) {
    let hex = caid.rsplit(':').next().unwrap();
    let p = dir
        .join(".oo/objects/sha256")
        .join(&hex[..2])
        .join(&hex[2..]);
    let mut b = fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    let mid = b.len() / 2;
    b[mid] ^= 0xFF;
    fs::write(&p, b).unwrap();
}

// ── a real serving node ─────────────────────────────────────────────────

struct Node { child: Child, port: u16, log: PathBuf }
impl Drop for Node {
    fn drop(&mut self) { self.child.kill().ok(); self.child.wait().ok(); }
}

fn free_port() -> u16 {
    for _ in 0..64 {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let p = l.local_addr().unwrap().port();
        if p > 24000 { return p; }
    }
    panic!("no free port above 24000");
}

fn serve(dir: &Path) -> Node {
    let port = free_port();
    let log = dir.join(format!("serve-{port}.log"));
    let f = fs::File::create(&log).unwrap();
    let child = oo_cmd(dir)
        .args(["node", "serve", "--port", &port.to_string()])
        .stdout(Stdio::from(f.try_clone().unwrap()))
        .stderr(Stdio::from(f))
        .spawn()
        .unwrap();
    let node = Node { child, port, log };
    for _ in 0..40 {
        std::thread::sleep(Duration::from_millis(100));
        if TcpStream::connect(("127.0.0.1", port)).is_ok() { return node; }
    }
    panic!("`oo node serve` never came up: {}", fs::read_to_string(&node.log).unwrap_or_default());
}

fn ask_raw(port: u16, payload: &str) -> String {
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    s.write_all(payload.as_bytes()).unwrap();
    if !payload.ends_with('\n') { s.write_all(b"\n").unwrap(); }
    s.flush().unwrap();
    s.shutdown(std::net::Shutdown::Write).ok();
    let mut buf = Vec::new();
    s.read_to_end(&mut buf).ok();
    String::from_utf8_lossy(&buf).to_string()
}

fn field_of(reply: &str, key: &str) -> Option<String> {
    let j: serde_json::Value = serde_json::from_str(reply.trim()).ok()?;
    let v = j.get(key).or_else(|| j.get(key.trim_start_matches('%')))?;
    Some(v.as_str()?.trim().trim_start_matches('#').to_string())
}

fn status_of(reply: &str) -> String {
    field_of(reply, "%status").unwrap_or_else(|| "<none>".into())
}

fn reason_of(reply: &str) -> Option<String> {
    field_of(reply, "%reason")
}

// ── a peer from the future ──────────────────────────────────────────────

/// Answers exactly one JSON body to every connection, then closes.
struct Stub { port: u16, stop: Arc<AtomicBool> }

impl Drop for Stub {
    fn drop(&mut self) { self.stop.store(true, Ordering::SeqCst); }
}

fn stub_peer(body: &str) -> Stub {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    listener.set_nonblocking(true).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let stop_t = stop.clone();
    let body = format!("{}\n", body.trim());
    std::thread::spawn(move || {
        while !stop_t.load(Ordering::SeqCst) {
            match listener.accept() {
                Ok((mut c, _)) => {
                    let mut buf = [0u8; 4096];
                    c.set_read_timeout(Some(Duration::from_millis(300))).ok();
                    let _ = c.read(&mut buf);
                    let _ = c.write_all(body.as_bytes());
                    let _ = c.flush();
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(_) => break,
            }
        }
    });
    Stub { port, stop }
}

/// Fetch one CAID from the stub and return (observed value, stderr).
fn fetch_from(dir: &Path, port: u16, caid: &str) -> (String, String) {
    write(
        dir,
        "f.n",
        &format!(
            "ok:  ~%Discovery./connect (\"p\", \"tcp://127.0.0.1:{port}\")\n\
             got: ~%Discovery./fetch \"{caid}\"\n"
        ),
    );
    oo_split(dir, &["run", "f.n", "--observe", "got", "--privileged"])
}

fn cause_of(observed: &str) -> String {
    match observed.split("%cause:").nth(1) {
        Some(rest) => rest.trim().trim_end_matches(')').trim().trim_start_matches('#').to_string(),
        None => format!("<no cause in {observed:?}>"),
    }
}

fn integrity_lines(stderr: &str) -> Vec<String> {
    stderr
        .lines()
        .filter(|l| l.trim_start().starts_with("integrity #"))
        .map(str::to_string)
        .collect()
}

const ABSENT_CAID: &str =
    "hash:sha256:v1:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

/// The seven server-side cases the arc is about.
fn seven_cases(node: &Node, held: &str, corrupt_caid: &str) -> Vec<(&'static str, String)> {
    vec![
        ("held + intact", ask_raw(node.port, &format!("{{{{ %op: #fetch, %hash: \"{held}\" }}}}"))),
        ("held but corrupt", ask_raw(node.port, &format!("{{{{ %op: #fetch, %hash: \"{corrupt_caid}\" }}}}"))),
        ("not held", ask_raw(node.port, &format!("{{{{ %op: #fetch, %hash: \"{ABSENT_CAID}\" }}}}"))),
        ("unknown op", ask_raw(node.port, "{{ %op: #nonsense, %from: \"x\" }}")),
        ("field missing", ask_raw(node.port, "{{ %op: #fetch, %from: \"x\" }}")),
        ("malformed line", ask_raw(node.port, "this is not a request")),
        ("unparseable caid", ask_raw(node.port, "{{ %op: #fetch, %hash: \"not-a-caid\" }}")),
    ]
}

// ════════════════════════════════════════════════════════════════════════
//  CONTROL
// ════════════════════════════════════════════════════════════════════════

/// C0 — a well-formed request is still answered with the value.
///
/// Leads the file. Every red below asks what the server says when something
/// is wrong, and a server that had stopped answering at all would satisfy
/// most of them.
#[test]
fn c0_a_wellformed_request_is_still_answered() {
    let dir = fresh_dir("c0");
    init(&dir);
    let caid = store(&dir, "{ treasure: \"c0\" }");
    let node = serve(&dir);
    let r = ask_raw(node.port, &format!("{{{{ %op: #fetch, %hash: \"{caid}\" }}}}"));
    assert_eq!(status_of(&r), "success", "a held object was not served: {r}");
    assert!(r.contains("treasure"), "the value did not come back: {r}");
}

// ════════════════════════════════════════════════════════════════════════
//  REDS
// ════════════════════════════════════════════════════════════════════════

/// R1 — an op this node does not serve says so.
#[test]
fn r1_unknown_op_is_not_implemented() {
    let dir = fresh_dir("r1");
    init(&dir);
    let node = serve(&dir);
    let r = ask_raw(node.port, "{{ %op: #nonsense, %from: \"x\" }}");
    assert_eq!(status_of(&r), "not_implemented", "{r}");
    assert_eq!(reason_of(&r).as_deref(), Some("unknown_op"), "{r}");
}

/// R2 — REAL_02 §3.2's MUST, as one assertion.
#[test]
fn r2_corrupt_and_unknown_op_are_distinguishable() {
    let dir = fresh_dir("r2");
    init(&dir);
    let bad = store(&dir, "{ victim: \"r2\" }");
    corrupt(&dir, &bad);
    let node = serve(&dir);

    let corrupt_reply = ask_raw(node.port, &format!("{{{{ %op: #fetch, %hash: \"{bad}\" }}}}"));
    let unknown_reply = ask_raw(node.port, "{{ %op: #nonsense, %from: \"x\" }}");

    let a = (status_of(&corrupt_reply), reason_of(&corrupt_reply));
    let b = (status_of(&unknown_reply), reason_of(&unknown_reply));
    assert_ne!(
        a, b,
        "a corrupt object and an op this node does not serve answered \
         identically. A client cannot tell 'ask someone else' from 'fix your \
         packet', which is the MUST REAL_02 §3.2 says of itself is unmet"
    );
}

/// R3 — nothing that is not `#success` answers without saying why.
#[test]
fn r3_every_non_success_carries_a_reason() {
    let dir = fresh_dir("r3");
    init(&dir);
    let held = store(&dir, "{ ok: \"r3\" }");
    let bad = store(&dir, "{ victim: \"r3\" }");
    corrupt(&dir, &bad);
    let node = serve(&dir);

    let mut silent = Vec::new();
    for (name, reply) in seven_cases(&node, &held, &bad) {
        if status_of(&reply) == "success" {
            continue;
        }
        if reason_of(&reply).is_none() {
            silent.push(format!("{name} -> #{} with no %reason", status_of(&reply)));
        }
    }
    assert!(
        silent.is_empty(),
        "these answers refused without saying why: {silent:#?}"
    );
}

/// R4 — the reason names which `#conflict` this is.
#[test]
fn r4_the_reason_names_which_conflict_it_is() {
    let dir = fresh_dir("r4");
    init(&dir);
    let bad = store(&dir, "{ victim: \"r4\" }");
    corrupt(&dir, &bad);
    let node = serve(&dir);

    let corrupt_reply = ask_raw(node.port, &format!("{{{{ %op: #fetch, %hash: \"{bad}\" }}}}"));
    assert_eq!(status_of(&corrupt_reply), "conflict", "{corrupt_reply}");
    assert_eq!(
        reason_of(&corrupt_reply).as_deref(),
        Some("caid_mismatch"),
        "the one #conflict that IS an integrity verdict must say so: {corrupt_reply}"
    );

    let missing = ask_raw(node.port, "{{ %op: #fetch, %from: \"x\" }}");
    assert_eq!(reason_of(&missing).as_deref(), Some("missing_field"), "{missing}");

    let garbage = ask_raw(node.port, "this is not a request");
    assert_eq!(reason_of(&garbage).as_deref(), Some("malformed"), "{garbage}");
}

/// R5 — "I do not serve that op" is not an integrity failure.
#[test]
fn r5_not_implemented_is_not_an_integrity_incident() {
    let dir = fresh_dir("r5");
    init(&dir);
    let stub = stub_peer(r##"{"%status":"#not_implemented","%source":"a-node","%hops":0}"##);
    let (out, err) = fetch_from(&dir, stub.port, ABSENT_CAID);
    assert_eq!(cause_of(&out), "peer_not_implemented", "observed: {out}");
    assert!(
        integrity_lines(&err).is_empty(),
        "a peer that said it does not serve this op was recorded as having \
         failed an integrity check: {err}"
    );
}

/// R6 — a peer newer than this client is not a broken peer.
#[test]
fn r6_an_unknown_status_is_not_an_integrity_incident() {
    let dir = fresh_dir("r6");
    init(&dir);
    let stub = stub_peer(r##"{"%status":"#teapot","%source":"a-newer-node","%hops":0}"##);
    let (out, err) = fetch_from(&dir, stub.port, ABSENT_CAID);
    assert_eq!(cause_of(&out), "peer_unknown_status", "observed: {out}");
    assert!(
        integrity_lines(&err).is_empty(),
        "a peer speaking a dialect this client does not know was recorded as \
         serving undecodable objects. Every future extension of the protocol \
         would make older clients accuse newer nodes of corruption: {err}"
    );
}

/// R7 — an unexplained refusal is a refusal, not an accusation.
#[test]
fn r7_an_unexplained_conflict_is_a_refusal() {
    let dir = fresh_dir("r7");
    init(&dir);
    let stub = stub_peer(r##"{"%status":"#conflict","%source":"an-older-node","%hops":0}"##);
    let (out, err) = fetch_from(&dir, stub.port, ABSENT_CAID);
    assert_eq!(cause_of(&out), "peer_refused", "observed: {out}");
    assert!(
        integrity_lines(&err).is_empty(),
        "an accusation you cannot substantiate was written into a \
         reputational record: {err}"
    );
}

/// P7 — a substantiated accusation is still made. The other half of R7.
///
/// CALIBRATION MOVED THIS. It was written as a red and is green at baseline
/// for the wrong reason: today *every* `#conflict` accuses, so "the reasoned
/// one accuses" holds trivially. A probe that is green before the work and
/// green after it is a pin, not a target — and this one is worth keeping,
/// because R7's fix ("stop accusing") would otherwise be satisfiable by never
/// accusing anyone. R7 is the red; this is what stops the repair going too
/// far. Neither alone pins the line.
#[test]
fn p7_a_reasoned_caid_mismatch_still_accuses() {
    let dir = fresh_dir("r8");
    init(&dir);
    let stub = stub_peer(
        r##"{"%status":"#conflict","%reason":"#caid_mismatch","%source":"a-node","%hops":0}"##,
    );
    let (out, err) = fetch_from(&dir, stub.port, ABSENT_CAID);
    assert_eq!(cause_of(&out), "caid_mismatch", "observed: {out}");
    assert_eq!(
        integrity_lines(&err).len(),
        1,
        "a peer that reported an address-verification failure must still be \
         recorded — a check that never accuses anyone is not a check: {err}"
    );
}

/// R9 — a scan past a refusing peer reports absence, not corruption.
#[test]
fn r9_a_scan_past_a_refusing_peer_says_missing() {
    let dir = fresh_dir("r9");
    let holder = fresh_dir("r9-holder");
    init(&dir);
    init(&holder);
    let honest = serve(&holder);
    let stub = stub_peer(r##"{"%status":"#not_implemented","%source":"a-node","%hops":0}"##);

    write(
        &dir,
        "f.n",
        &format!(
            "a:   ~%Discovery./connect (\"refuser\", \"tcp://127.0.0.1:{}\")\n\
             b:   ~%Discovery./connect (\"honest\",  \"tcp://127.0.0.1:{}\")\n\
             got: ~%Discovery./fetch \"{ABSENT_CAID}\"\n",
            stub.port, honest.port
        ),
    );
    let (out, err) = oo_split(&dir, &["run", "f.n", "--observe", "got", "--privileged"]);
    assert_eq!(
        cause_of(&out),
        "missing_key",
        "one peer refused and another honestly did not hold it, so nobody has \
         it — ERROR_CODES says 純粹「無人持有」不適用 #caid_mismatch. observed: {out}"
    );
    assert!(integrity_lines(&err).is_empty(), "{err}");
}

// ════════════════════════════════════════════════════════════════════════
//  PINS
// ════════════════════════════════════════════════════════════════════════

/// P1 — the status set does not grow. Distinguishability lives in `%reason`.
#[test]
fn p1_the_status_set_did_not_grow() {
    let dir = fresh_dir("p1");
    init(&dir);
    let held = store(&dir, "{ ok: \"p1\" }");
    let bad = store(&dir, "{ victim: \"p1\" }");
    corrupt(&dir, &bad);
    let node = serve(&dir);

    let allowed = ["success", "not_found", "conflict", "not_implemented", "rejected"];
    let mut unexpected = Vec::new();
    for (name, reply) in seven_cases(&node, &held, &bad) {
        let s = status_of(&reply);
        if !allowed.contains(&s.as_str()) {
            unexpected.push(format!("{name} -> #{s}"));
        }
    }
    assert!(
        unexpected.is_empty(),
        "a new %status appeared: {unexpected:#?}. REAL_02 §3.2 keeps the set \
         small and stable across ops; a new op must not inflate it"
    );
}

/// P2 — a held object still arrives.
#[test]
fn p2_a_held_object_still_arrives() {
    let dir = fresh_dir("p2");
    init(&dir);
    let caid = store(&dir, "{ treasure: \"p2\" }");
    let node = serve(&dir);
    let r = ask_raw(node.port, &format!("{{{{ %op: #fetch, %hash: \"{caid}\" }}}}"));
    assert_eq!(status_of(&r), "success", "{r}");
    assert!(r.contains("treasure"), "{r}");
}

/// P3 — absence is still absence, not conflict.
#[test]
fn p3_not_found_is_still_not_found() {
    let dir = fresh_dir("p3");
    init(&dir);
    let node = serve(&dir);
    let r = ask_raw(node.port, &format!("{{{{ %op: #fetch, %hash: \"{ABSENT_CAID}\" }}}}"));
    assert_eq!(
        status_of(&r),
        "not_found",
        "not holding something is not a conflict: {r}"
    );
}

/// P4 — `#rejected` and its `%reason` are unchanged (REAL_02 §4.2.2).
#[test]
fn p4_advertise_rejection_is_unchanged() {
    let dir = fresh_dir("p4");
    init(&dir);
    let node = serve(&dir);
    let r = ask_raw(node.port, "{{ %op: #advertise, %from: \"x\" }}");
    assert_eq!(status_of(&r), "rejected", "{r}");
    assert!(reason_of(&r).is_some(), "#rejected lost its %reason: {r}");
}

/// P5 — silence is still never an answer.
#[test]
fn p5_silence_is_still_never_an_answer() {
    let dir = fresh_dir("p5");
    init(&dir);
    let node = serve(&dir);
    let adversarial = [
        "",
        "\n",
        "{{",
        "{{ %op: }}",
        "{{ %op: #fetch, %hash: 12345 }}",
        "{{ %op: #fetch, %hash: \"hash:sha256:v1:zz\" }}",
        "\u{0}\u{1}\u{2}",
    ];
    for p in adversarial {
        let r = ask_raw(node.port, p);
        assert!(
            !r.trim().is_empty(),
            "{p:?} was answered with silence — the whole point of §3.2 is that \
             0 bytes must not stand for anything"
        );
    }
}

/// P6 — an advertisement body that *computes* is still refused if it fails
/// the ladder.
///
/// Standing rule since v0.2.50: adversarial cases at a remote-input entry
/// point must include a payload that computes, not only malformed shapes. A
/// body of the right shape with a wrong signature is the one that exercises
/// the verification path rather than the parser.
#[test]
fn p6_a_computing_payload_is_still_refused() {
    let dir = fresh_dir("p6");
    init(&dir);
    let node = serve(&dir);
    let ad = "{{ node_id: \"hash:sha256:v1:00\", public_key: \"aa\", services: [], \
               listen_port: 8080, capacity: 10, ts: 1, ttl: 15, \
               signature: \"deadbeef\" }}";
    let r = ask_raw(
        node.port,
        &format!("{{{{ %op: #advertise, %from: \"hash:sha256:v1:00\", %ad: {ad} }}}}"),
    );
    assert_ne!(status_of(&r), "success", "a forged advertisement was accepted: {r}");
    assert!(
        !r.trim().is_empty(),
        "a forged advertisement was answered with silence: {r}"
    );
}
