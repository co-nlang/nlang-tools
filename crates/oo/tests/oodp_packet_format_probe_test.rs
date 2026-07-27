// Silence means four things (2026-07-27, pre-committed by work order:
// docs/oodp_packet_format_handover.md).
//
// ── The headline, measured on v0.2.47 ────────────────────────────────────
//
// Two engines federate. That part works — B fetches A's value over TCP and A
// logs the request. What does not work is everything the peer has to say when
// it cannot hand over the bytes:
//
//   peer does not have it        →  0 bytes  →  client: _|_ #conflict
//   peer's copy is corrupt       →  0 bytes  →  client: _|_ #conflict
//   peer accepts, never answers  →  oo hangs until killed
//
// The server KNOWS which — it prints `NDP Miss` and, since v0.2.44, prints
// `NDP integrity #caid_mismatch` — and it says none of it on the wire. Four
// situations, one answer, and one of the four is not an answer at all.
//
// That is REAL_03 §6.6 條款三 (三結果必須可分) one layer out, and v0.2.44
// deferred it in as many words: "Wire stays 0 bytes (REAL_02 §3.2 arc)".
//
// ── What the wire owes ───────────────────────────────────────────────────
// REAL_02 §3.2 specifies an envelope, and neither half of it exists:
//
//   request   {{ %op: #discover|#fetch|#advertise, %hash: @caid, %from: @caid }}
//   response  {  %status: #success|#not_found|#conflict, %result, %source, %hops }
//
// today:      request = bare CAID + "\n"      response = bare JSON, or nothing
//
// `%from` is deliberately NOT in this arc (work order Q1): fetch has no use for
// knowing who asks — objects self-authenticate — and `%from` earns its keep in
// `#advertise` and `#discover`, which land with node identity.
//
// ── Why the envelope carries %op from day one ────────────────────────────
// LADD is already implemented: `ladd.rs` has GBB (mass, sketch, masa_ref,
// nerve_structure) and `disc.advertise` / `disc.find` are real code. They read
// and write `oo.gbb_registry` — an in-process `RwLock<HashMap>`.
//
//   LADD is implemented as a local simulation. The packet format is the step
//   that makes it distributed.
//
// So the envelope is built for three ops even though only `#fetch` is served
// here. The other two are then additive, not a second protocol break.
//
// ── A peer's %status is a claim, not a verification ──────────────────────
// P2 pins the thing this arc must not weaken. A peer that answers `#success`
// with some other object's bytes is still refused, because the client
// recomputes the address (v0.2.44). Degree 0 does not soften because the wire
// grew a field where the peer can assert things.
//
// ── A note on the harness ────────────────────────────────────────────────
// The command that runs a node is being renamed in this same arc. The server
// launcher below therefore tries `oo node serve` and falls back to `oo serve`,
// SOLELY so the wire gates are not confounded by the rename. R5 is what decides
// the name; nothing else in this file may be read as tolerating either spelling.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::fs;

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

const PLAIN_SRC: &str = "v: { hello: \"world\" }\n";
const GOLDEN_VALUE_CAID: &str = "hash:sha256:v2:_:gICS1LCf09bLAQD//5HUsJ/T1ssBAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA:681781ef857ac859326d707bdfcd04fc939b78e7c9060dd674d9a8be536f2ae4";

/// Returned by `run_bounded` when the engine had to be killed. Any gate that
/// sees this has measured a hang, never an answer.
const HUNG: &str = "<HARNESS: engine had to be killed>";

// ── harness ─────────────────────────────────────────────────────────────

fn fresh_dir(tag: &str) -> PathBuf {
    let mut d = std::env::temp_dir();
    d.push(format!(
        "nlang-oodp-{}-{}-{}",
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
        .env("OO_IDENTITY", dir.join("identity-for-tests"));
    c
}

fn oo(dir: &Path, args: &[&str]) -> String {
    let out = oo_cmd(dir).args(args).output().unwrap();
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout).trim(),
        String::from_utf8_lossy(&out.stderr).trim()
    )
}

/// Runs `oo` with a deadline. Returns [`HUNG`] rather than blocking the suite,
/// so "the engine never came back" is a measurable outcome instead of a
/// wedged test run.
fn run_bounded(dir: &Path, args: &[&str], secs: u64) -> String {
    let mut child = oo_cmd(dir)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(secs);
    loop {
        match child.try_wait().unwrap() {
            Some(_) => break,
            None if Instant::now() >= deadline => {
                child.kill().ok();
                child.wait().ok();
                return HUNG.to_string();
            }
            None => std::thread::sleep(Duration::from_millis(50)),
        }
    }
    let out = child.wait_with_output().unwrap();
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout).trim(),
        String::from_utf8_lossy(&out.stderr).trim()
    )
}

fn write(dir: &Path, name: &str, src: &str) {
    fs::write(dir.join(name), src).unwrap();
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn object_path(dir: &Path, caid: &str) -> PathBuf {
    let d = caid.rsplit(':').next().unwrap();
    dir.join(".oo")
        .join("objects")
        .join("sha256")
        .join(&d[..2])
        .join(&d[2..])
}

/// Stores `expr` in `dir` and returns its CAID.
fn store(dir: &Path, expr: &str) -> String {
    write(
        dir,
        "i.n",
        &format!("id: ~%Discovery./identify_and_store {expr}\n"),
    );
    let caid = oo(dir, &["run", "i.n", "--observe", "id"])
        .trim()
        .trim_start_matches('"')
        .split('"')
        .next()
        .unwrap()
        .to_string();
    assert!(caid.starts_with("hash:sha256:"), "store() got {caid:?}");
    caid
}

/// A running node.
///
/// The launcher tries `node serve` then `serve` — see the header note. This
/// tolerance exists so the wire gates work on both sides of the rename; R5 is
/// the gate that decides the spelling.
struct Node {
    child: Child,
    port: u16,
}

impl Drop for Node {
    fn drop(&mut self) {
        self.child.kill().ok();
        self.child.wait().ok();
    }
}

fn serve(dir: &Path) -> Node {
    for args in [
        vec!["node", "serve", "--port"],
        vec!["serve", "--port"],
    ] {
        let port = free_port();
        let p = port.to_string();
        let mut a = args.clone();
        a.push(&p);
        let child = oo_cmd(dir)
            .args(&a)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut node = Node { child, port };
        for _ in 0..40 {
            std::thread::sleep(Duration::from_millis(100));
            if node.child.try_wait().unwrap().is_some() {
                break; // this spelling is not accepted; try the next
            }
            if TcpStream::connect(("127.0.0.1", port)).is_ok() {
                return node;
            }
        }
        node.child.kill().ok();
    }
    panic!("neither `oo node serve` nor `oo serve` came up");
}

/// Sends raw bytes to a node and reads the whole reply, bounded.
fn ask_raw(port: u16, payload: &str) -> Vec<u8> {
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    s.write_all(payload.as_bytes()).unwrap();
    if !payload.ends_with('\n') {
        s.write_all(b"\n").unwrap();
    }
    s.flush().unwrap();
    let mut buf = Vec::new();
    s.read_to_end(&mut buf).ok();
    buf
}

/// A peer that accepts a connection and never says anything, holding it open.
/// Returns the port; the listener thread is detached and dies with the test.
fn spawn_silent_peer() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        let mut held = Vec::new();
        for stream in listener.incoming() {
            let Ok(s) = stream else { continue };
            held.push(s); // never read, never write, never close
        }
    });
    port
}

/// A peer that answers every request with the same bytes, whatever was asked.
fn spawn_liar(payload: Vec<u8>) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut s) = stream else { continue };
            let Ok(c) = s.try_clone() else { continue };
            let mut line = String::new();
            let _ = BufReader::new(c).read_line(&mut line);
            let _ = s.write_all(&payload);
            let _ = s.flush();
            let _ = s.shutdown(std::net::Shutdown::Write);
        }
    });
    port
}

/// Client-visible result of fetching `caid` from `addr`, bounded.
fn fetch_from(dir: &Path, addr: &str, caid: &str, secs: u64) -> String {
    write(
        dir,
        "q.n",
        &format!(
            "p: ~%Discovery./connect(\"a\", \"{addr}\")\nout: ~%Discovery./fetch(\"a\", \"{caid}\")\n"
        ),
    );
    run_bounded(dir, &["run", "q.n", "--observe", "out"], secs)
}

/// Replaces an object's bytes with a valid encoding of a DIFFERENT value, so
/// the holder's own read fails address re-verification (v0.2.43) — the peer
/// has it, and what it has is not what it claims.
fn corrupt_object(dir: &Path, caid: &str) {
    let decoy = fresh_dir("decoy");
    let other = store(&decoy, "{ not: \"the same thing at all\" }");
    let bytes = fs::read(object_path(&decoy, &other)).unwrap();
    let p = object_path(dir, caid);
    assert!(p.exists(), "harness: nothing to corrupt at {p:?}");
    fs::write(&p, bytes).unwrap();
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES
// ─────────────────────────────────────────────────────────────────────────

/// R1 — the headline. Four peer states, four distinguishable answers.
///
/// PAIRED IN ALL FOUR DIRECTIONS on purpose: an engine that answered
/// `#not_found` to everything would pass any one-sided test. The gate asserts
/// the four results are *pairwise* different, and that none of them is the
/// harness having to kill the engine.
#[test]
fn red_four_peer_states_are_four_answers() {
    let holder = fresh_dir("r1-holder");
    let good = store(&holder, "{ pkg: \"hello\" }");
    let rotten = store(&holder, "{ pkg: \"rotten\" }");
    corrupt_object(&holder, &rotten);
    let missing = format!("hash:sha256:v1:{}", "e".repeat(64));

    let node = serve(&holder);
    let addr = format!("tcp://127.0.0.1:{}", node.port);
    let client = fresh_dir("r1-client");

    let has = fetch_from(&client, &addr, &good, 30);
    let lacks = fetch_from(&client, &addr, &missing, 30);
    let corrupt = fetch_from(&client, &addr, &rotten, 30);
    let silent = fetch_from(
        &client,
        &format!("tcp://127.0.0.1:{}", spawn_silent_peer()),
        &good,
        30,
    );

    // Anti-vacuity: the working case must actually work, or "all different"
    // could be satisfied by four different flavours of failure.
    assert!(
        has.contains("hello") && !has.contains("_|_"),
        "harness: the honest fetch did not return the object: {has:?}"
    );

    assert_ne!(
        HUNG, silent,
        "a peer that accepts and never answers still hangs the engine"
    );

    let outcomes = [
        ("has it", &has),
        ("lacks it", &lacks),
        ("has it, corrupt", &corrupt),
        ("never answers", &silent),
    ];
    for (i, (na, a)) in outcomes.iter().enumerate() {
        for (nb, b) in outcomes.iter().skip(i + 1) {
            assert_ne!(
                a, b,
                "`{na}` and `{nb}` are indistinguishable to the client: {a:?}"
            );
        }
    }
}

/// R2 — the node accepts the specified request envelope.
#[test]
fn red_node_accepts_the_request_envelope() {
    let holder = fresh_dir("r2");
    let caid = store(&holder, "{ pkg: \"enveloped\" }");
    let node = serve(&holder);

    let reply = ask_raw(
        node.port,
        &format!("{{{{ %op: #fetch, %hash: \"{caid}\" }}}}"),
    );
    assert!(
        !reply.is_empty(),
        "a well-formed #fetch envelope was answered with silence"
    );
    let text = String::from_utf8_lossy(&reply);
    assert!(
        text.contains("enveloped"),
        "the envelope did not return the object: {text}"
    );
}

/// R3 — a known op this node does not serve is ANSWERED, not ignored.
///
/// `#discover` is specified (REAL_02 §3.2) and not implemented here. A peer
/// must be able to tell "this node does not do discovery" from "this node
/// ignored me"; those are the same 0 bytes today.
#[test]
fn red_an_unserved_op_gets_an_answer() {
    let holder = fresh_dir("r3");
    let caid = store(&holder, "{ pkg: \"x\" }");
    let node = serve(&holder);

    let reply = ask_raw(
        node.port,
        &format!("{{{{ %op: #discover, %hash: \"{caid}\" }}}}"),
    );
    assert!(
        !reply.is_empty(),
        "a specified-but-unserved op was answered with silence"
    );
}

/// R4 — the node emits the specified response envelope.
///
/// Distinct from R2: accepting the request and emitting a reply that *says what
/// kind of reply it is* are two things, and after R2 they can fail
/// independently — a node can take the envelope and still answer with a bare
/// value.
///
/// DEPENDENT RED, disclosed: until R2 is green this fails at "answered with
/// silence" rather than at the missing fields, because the baseline node reads
/// the envelope as a CAID it does not have. Same shape as v0.2.45's R5a, and
/// annotated rather than restructured for the same reason: the alternative is
/// to assert envelope fields on a reply to the OLD protocol, which would smuggle
/// in a design constraint (that old-protocol requests get an envelope back
/// rather than a clean close) that the work order deliberately leaves open.
#[test]
fn red_node_emits_the_response_envelope() {
    let holder = fresh_dir("r4");
    let caid = store(&holder, "{ pkg: \"enveloped\" }");
    let node = serve(&holder);

    let reply = ask_raw(
        node.port,
        &format!("{{{{ %op: #fetch, %hash: \"{caid}\" }}}}"),
    );
    let text = String::from_utf8_lossy(&reply).to_string();
    assert!(
        !text.is_empty(),
        "a #fetch envelope for an object the node HOLDS was answered with \
         silence, so there is no response envelope to inspect (see R2)"
    );
    for field in ["%status", "%source", "%hops"] {
        assert!(
            text.contains(field),
            "the response envelope has no `{field}`: {text}"
        );
    }
    assert!(
        text.contains("#success"),
        "a successful fetch must say so: {text}"
    );
}

/// R5 — the node command is `oo node serve`.
///
/// `node` is not a coinage: REAL_01 §1.2 already calls this a 宇宙節點
/// (Universe Node). The noun was in the spec all along and never had a CLI
/// spelling. PAIRED: the old spelling must be gone, or the rename has not
/// happened — it has only been aliased.
#[test]
fn red_the_node_command_is_oo_node_serve() {
    let d = fresh_dir("r5");
    let port = free_port();

    let mut child = oo_cmd(&d)
        .args(["node", "serve", "--port", &port.to_string()])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut up = false;
    for _ in 0..40 {
        std::thread::sleep(Duration::from_millis(100));
        if child.try_wait().unwrap().is_some() {
            break;
        }
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            up = true;
            break;
        }
    }
    child.kill().ok();
    child.wait().ok();
    assert!(up, "`oo node serve` did not come up");

    // PAIR: the old spelling is retired, not aliased.
    let old = oo(&d, &["serve", "--port", &free_port().to_string()]);
    assert!(
        old.contains("unrecognized") || old.contains("error"),
        "`oo serve` still works, so this is an alias and not a rename: {old:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// PINS — green at baseline, must stay green
// ─────────────────────────────────────────────────────────────────────────

/// P1 — two engines still federate. The capability this arc is refining must
/// survive being refined.
#[test]
fn pin_two_engines_federate_end_to_end() {
    let holder = fresh_dir("p1-holder");
    let caid = store(&holder, "{ pkg: \"federated\" }");
    let node = serve(&holder);
    let client = fresh_dir("p1-client");

    let got = fetch_from(
        &client,
        &format!("tcp://127.0.0.1:{}", node.port),
        &caid,
        30,
    );
    assert!(
        got.contains("federated") && !got.contains("_|_"),
        "two engines no longer federate: {got:?}"
    );
}

/// P2 — a peer's answer is checked, not trusted.
///
/// The liar answers every request with some other object's bytes. The client
/// must never hand those back, whatever the peer claims about them. Written to
/// hold on both sides of the envelope change: pre-arc the liar is refused by
/// address re-verification (v0.2.44), post-arc it is refused for that or for
/// speaking the old protocol — either way the decoy must not surface.
#[test]
fn pin_a_lying_peer_is_still_refused() {
    let decoy_dir = fresh_dir("p2-decoy");
    let decoy = store(&decoy_dir, "{ pkg: \"DECOY-PAYLOAD\" }");
    let bytes = fs::read(object_path(&decoy_dir, &decoy)).unwrap();
    assert!(
        String::from_utf8_lossy(&bytes).contains("DECOY-PAYLOAD"),
        "harness: the decoy bytes are not what we think"
    );

    let wanted = format!("hash:sha256:v1:{}", "a".repeat(64));
    let client = fresh_dir("p2-client");
    let got = fetch_from(
        &client,
        &format!("tcp://127.0.0.1:{}", spawn_liar(bytes)),
        &wanted,
        30,
    );
    assert_ne!(HUNG, got, "the liar hung the engine");
    assert!(
        !got.contains("DECOY-PAYLOAD"),
        "a peer's bytes were handed back for a CAID they do not hash to: {got:?}"
    );
}

/// P8 — a silent peer names a PEER timeout, not the computation one.
///
/// ACCEPTOR REPAIR pin. The delivery reused `BottomCause::Timeout`, which
/// pre-exists for a computation that outran `%timeout` (`observation.rs:63`).
/// ERROR_CODES gives `#timeout` the remedy 「請優化性能、減少嵌套,或放寬時間
/// 限制」 — advice that, for a peer holding a socket open and saying nothing,
/// points the reader at their own code. An arc whose thesis is that four
/// situations must be separable cannot ship a fifth that is not.
#[test]
fn pin_a_silent_peer_names_a_peer_timeout() {
    let client = fresh_dir("p8");
    let got = fetch_from(
        &client,
        &format!("tcp://127.0.0.1:{}", spawn_silent_peer()),
        &format!("hash:sha256:v1:{}", "a".repeat(64)),
        30,
    );
    assert_ne!(HUNG, got, "a silent peer hung the engine");
    assert!(
        got.contains("peer_timeout"),
        "a silent peer must name a peer timeout: {got:?}"
    );
}

/// P3 — an ordinary value's address does not move. Nothing on this path
/// touches how values are addressed.
#[test]
fn pin_ordinary_value_caids_do_not_move() {
    let d = fresh_dir("p3");
    assert_eq!(
        store(&d, "{ hello: \"world\" }"),
        GOLDEN_VALUE_CAID,
        "an ordinary value's address moved"
    );
}

/// P4 — the universe root stays deterministic across workspaces.
#[test]
fn pin_root_caid_stays_deterministic() {
    let digests: std::collections::BTreeSet<String> = (0..3)
        .map(|i| {
            let d = fresh_dir(&format!("p4-{i}"));
            write(&d, "s.n", PLAIN_SRC);
            oo(&d, &["evolve", "s.n"]);
            assert!(
                oo(&d, &["commit", "-m", "x"]).contains("Commit successful"),
                "harness: commit"
            );
            let commit = oo(&d, &["log"])
                .lines()
                .find_map(|l| l.strip_prefix("commit ").map(|s| s.trim().to_string()))
                .unwrap();
            let j: serde_json::Value =
                serde_json::from_slice(&fs::read(object_path(&d, &commit)).unwrap()).unwrap();
            let dg = &j["root"]["digest"];
            if let Some(s) = dg.as_str() {
                s.to_string()
            } else if let Some(a) = dg.as_array() {
                a.iter()
                    .map(|b| format!("{:02x}", b.as_u64().expect("digest byte")))
                    .collect::<String>()
            } else {
                panic!("no usable digest: {}", j["root"]);
            }
        })
        .collect();
    assert_eq!(digests.len(), 1, "the universe root moved: {digests:#?}");
}

/// P5 — a LOCAL peer (a directory, not a socket) still resolves. The wire is
/// not the only way `disc.fetch` reaches an object.
#[test]
fn pin_local_peer_still_resolves() {
    let holder = fresh_dir("p5-holder");
    let caid = store(&holder, "{ pkg: \"local\" }");
    let client = fresh_dir("p5-client");
    write(
        &client,
        "q.n",
        &format!(
            "p: ~%Discovery./connect(\"a\", \"{}\")\nout: ~%Discovery./fetch(\"a\", \"{caid}\")\n",
            holder.display()
        ),
    );
    let got = run_bounded(&client, &["run", "q.n", "--observe", "out"], 30);
    assert!(
        got.contains("local") && !got.contains("_|_"),
        "a local peer no longer resolves: {got:?}"
    );
}

/// P6 — connecting to a store directory is still refused (SPEC_08 §6.3).
#[test]
fn pin_connect_still_refuses_the_store_boundary() {
    let holder = fresh_dir("p6-holder");
    store(&holder, "{ pkg: \"x\" }");
    let client = fresh_dir("p6-client");
    write(
        &client,
        "q.n",
        &format!(
            "out: ~%Discovery./connect(\"a\", \"{}\")\n",
            holder.join(".oo").display()
        ),
    );
    let got = run_bounded(&client, &["run", "q.n", "--observe", "out"], 30);
    assert!(
        got.contains("store_boundary"),
        "the store boundary stopped refusing peer connections: {got:?}"
    );
}

/// P7 — local LADD is untouched. `advertise` and `find` run entirely inside
/// one process today; this arc does not put them on the wire, and must not
/// break them on the way past.
#[test]
fn pin_local_ladd_still_works() {
    let d = fresh_dir("p7");
    write(
        &d,
        "q.n",
        "a: ~%Discovery./advertise { pkg: \"ladd\", version: 1 }\n\
         out: ~%Discovery./find { pkg: \"ladd\" }\n",
    );
    let adv = run_bounded(&d, &["run", "q.n", "--observe", "a"], 30);
    assert!(
        !adv.contains("_|_") && adv.contains("true"),
        "~%Discovery./advertise regressed: {adv:?}"
    );
    let found = run_bounded(&d, &["run", "q.n", "--observe", "out"], 30);
    assert_ne!(HUNG, found, "~%Discovery./find hung");
}
