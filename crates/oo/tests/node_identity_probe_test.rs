// The node introduces itself by its port number (2026-07-27, pre-committed by
// work order: docs/node_identity_handover.md).
//
// ── The headline, measured on v0.2.48 ────────────────────────────────────
//
//   let source_id = format!("node:{}", port);        // main.rs:235
//
// Two nodes on port 19831 on different machines are the same `%source`. One
// node restarted on another port is a different one. `%from` does not exist at
// all, so a request carries no notion of who is asking.
//
// `ladd.rs`'s `node_caid` is a misnomer — `disc.advertise` fills it with
// `arg.content_hash()`, the CAID of the advertised VALUE. Nothing in the engine
// holds a node's own identity.
//
// ── Why a node identity cannot be derived from the universe ──────────────
// Two fresh workspaces, same source, measured on v0.2.48:
//
//   a root: 5a2ec0a175ec4f089c6c5d9f6d939507…
//   b root: 5a2ec0a175ec4f089c6c5d9f6d939507…
//
// v0.2.45 made that determinism a virtue, and it disqualifies the universe as a
// node address: every node serving the same universe would occupy one DHT slot.
//
//   The property that makes a universe federatable is exactly the property
//   that disqualifies it as a node address. Content addressing answers *what*;
//   a node address answers *which of the many holders*.
//
// So the identity is a keypair. REAL_02 §4.2's `signature: b""` needs one
// anyway, and §4.1's 「節點 ID = CAID 的內容指紋前 160 bit」 never said what
// that CAID addresses — this arc answers: the node's public key.
//
// ── Where it lives, and why the path is part of it ───────────────────────
// Not in `.oo/`: v0.2.46 ruled a secret must not live inside a shareable
// artifact, and `.oo/objects` is the thing built to be served and copied.
//
// Path-derived, because **the engine cannot tell a moved workspace from a
// copied one** — it sees only that the path changed. Making the path part of
// the identity makes a copy structurally a different node: no detection, no
// heuristic, nobody to ask. `mv` costs a new identity; with no DHT yet that
// costs nothing.
//
// ── `%from` is a claim ───────────────────────────────────────────────────
// It is unsigned and any peer can put anything there. P5 forges it and requires
// the answer to be unchanged. Shipping it is for observability and for the
// advertise/discover arc; it is not authentication and nothing may depend on it.

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

const HUNG: &str = "<HARNESS: engine had to be killed>";
const GOLDEN_VALUE_CAID: &str = "hash:sha256:v2:_:gICS1LCf09bLAQD//5HUsJ/T1ssBAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA:681781ef857ac859326d707bdfcd04fc939b78e7c9060dd674d9a8be536f2ae4";

// ── harness ─────────────────────────────────────────────────────────────

fn fresh_dir(tag: &str) -> nlang_interpreter::ScratchDir {
    nlang_interpreter::ScratchDir::new(&format!("nid-{tag}"))
}

/// One operator home shared by a test, so the node keys of several workspaces
/// land in the same place — that is the arrangement the arc is about, and it
/// must never be the developer's real `~/.oo`.
fn fresh_home(tag: &str) -> nlang_interpreter::ScratchDir {
    fresh_dir(&format!("home-{tag}"))
}

fn oo_cmd(dir: &Path, home: &Path) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_oo"));
    c.current_dir(dir)
        .env("OO_NODE_HOME", home)
        .env("OO_IDENTITY", home.join("identity-for-tests"));
    c
}

fn oo(dir: &Path, home: &Path, args: &[&str]) -> String {
    let out = oo_cmd(dir, home).args(args).output().unwrap();
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout).trim(),
        String::from_utf8_lossy(&out.stderr).trim()
    )
}

fn run_bounded(dir: &Path, home: &Path, args: &[&str], secs: u64) -> String {
    let mut child = oo_cmd(dir, home)
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

fn store(dir: &Path, home: &Path, expr: &str) -> String {
    write(
        dir,
        "i.n",
        &format!("id: ~%Discovery./identify_and_store {expr}\n"),
    );
    let caid = oo(dir, home, &["run", "i.n", "--observe", "id"])
        .trim()
        .trim_start_matches('"')
        .split('"')
        .next()
        .unwrap()
        .to_string();
    assert!(caid.starts_with("hash:sha256:"), "store() got {caid:?}");
    caid
}

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

fn serve(dir: &Path, home: &Path) -> Node {
    let port = free_port();
    let child = oo_cmd(dir, home)
        .args(["node", "serve", "--port", &port.to_string()])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut node = Node { child, port };
    for _ in 0..40 {
        std::thread::sleep(Duration::from_millis(100));
        if node.child.try_wait().unwrap().is_some() {
            panic!("`oo node serve` exited immediately");
        }
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return node;
        }
    }
    panic!("`oo node serve` never came up");
}

/// Sends raw bytes to a node and reads the whole reply, bounded.
fn ask_raw(port: u16, payload: &str) -> String {
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    s.write_all(payload.as_bytes()).unwrap();
    if !payload.ends_with('\n') {
        s.write_all(b"\n").unwrap();
    }
    s.flush().unwrap();
    let mut buf = Vec::new();
    s.read_to_end(&mut buf).ok();
    String::from_utf8_lossy(&buf).into_owned()
}

/// A peer that records the raw request line it was sent and answers nothing
/// useful — used to read what the CLIENT puts on the wire.
struct Recorder {
    port: u16,
    seen: Arc<Mutex<Vec<String>>>,
}

fn spawn_recorder() -> Recorder {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = Arc::clone(&seen);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut s) = stream else { continue };
            let Ok(c) = s.try_clone() else { continue };
            let mut line = String::new();
            let _ = BufReader::new(c).read_line(&mut line);
            log.lock().unwrap().push(line.trim().to_string());
            let _ = s.write_all(b"");
            let _ = s.shutdown(std::net::Shutdown::Write);
        }
    });
    Recorder { port, seen }
}

fn fetch_from(dir: &Path, home: &Path, addr: &str, caid: &str, secs: u64) -> String {
    write(
        dir,
        "q.n",
        &format!(
            "p: ~%Discovery./connect(\"a\", \"{addr}\")\nout: ~%Discovery./fetch(\"a\", \"{caid}\")\n"
        ),
    );
    // connect_consent §5.1: remote ./connect needs --grant connect.
    run_bounded(
        dir,
        home,
        &["run", "q.n", "--observe", "out", "--grant", "connect"],
        secs,
    )
}

/// The 64-hex id `oo node id` prints.
fn node_id(dir: &Path, home: &Path) -> String {
    let out = oo(dir, home, &["node", "id"]);
    out.split(|c: char| !c.is_ascii_hexdigit())
        .find(|w| w.len() >= 64)
        .unwrap_or_else(|| panic!("`oo node id` printed no id: {out}"))
        .to_string()
}

/// Copies a workspace to a NEW path, the way `cp -r` would.
fn copy_workspace(src: &Path, tag: &str) -> nlang_interpreter::ScratchDir {
    let dst = fresh_dir(tag);
    fn rec(a: &Path, b: &Path) {
        fs::create_dir_all(b).unwrap();
        for e in fs::read_dir(a).unwrap().flatten() {
            let (p, q) = (e.path(), b.join(e.file_name()));
            if p.is_dir() {
                rec(&p, &q);
            } else {
                fs::copy(&p, &q).unwrap();
            }
        }
    }
    rec(src, &dst);
    dst
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES
// ─────────────────────────────────────────────────────────────────────────

/// R1 — the headline, PAIRED BOTH WAYS.
///
/// A copy at a different path is a different node; the same workspace across
/// processes is the same node. An engine that minted a fresh key on every call
/// would pass the first half; one that returned a constant would pass the
/// second. Only both together say anything.
#[test]
fn red_a_copied_workspace_is_a_different_node() {
    let home = fresh_home("r1");
    let a = fresh_dir("r1-a");
    write(&a, "s.n", "v: { pkg: \"same\" }\n");
    oo(&a, &home, &["evolve", "s.n"]);
    oo(&a, &home, &["commit", "-m", "x"]);

    let first = node_id(&a, &home);
    assert_eq!(
        first.len(),
        64,
        "a node id must be a 64-hex digest: {first}"
    );

    // Same workspace, a second process.
    let again = node_id(&a, &home);
    assert_eq!(
        first, again,
        "the same workspace reported two different node identities"
    );

    // Byte-identical copy at another path.
    let b = copy_workspace(&a, "r1-b");
    let copied = node_id(&b, &home);
    assert_ne!(
        first, copied,
        "a copied workspace kept the original's node identity: two nodes now \
         claim one id"
    );
}

/// R2 — the key is outside the workspace.
///
/// v0.2.46's rule, applied to the other key: `.oo/objects` is built to be
/// served and copied, so nothing secret may sit in that tree.
#[test]
fn red_the_node_key_lives_outside_the_workspace() {
    let home = fresh_home("r2");
    let d = fresh_dir("r2");
    write(&d, "s.n", "v: 1\n");
    oo(&d, &home, &["evolve", "s.n"]);
    oo(&d, &home, &["commit", "-m", "x"]);
    let id = node_id(&d, &home);
    assert_eq!(id.len(), 64, "harness: no node id");

    // Something under the operator home now holds a key.
    let mut found = Vec::new();
    fn walk(p: &Path, out: &mut Vec<PathBuf>) {
        if let Ok(rd) = fs::read_dir(p) {
            for e in rd.flatten() {
                let q = e.path();
                if q.is_dir() {
                    walk(&q, out)
                } else {
                    out.push(q)
                }
            }
        }
    }
    walk(&home, &mut found);
    assert!(
        !found.is_empty(),
        "no key material was persisted under the operator home"
    );

    // And nothing in `.oo/` matches any of it.
    let mut store_files = Vec::new();
    walk(&d.join(".oo"), &mut store_files);
    for k in &found {
        let bytes = fs::read(k).unwrap_or_default();
        if bytes.is_empty() {
            continue;
        }
        for s in &store_files {
            assert_ne!(
                fs::read(s).unwrap_or_default(),
                bytes,
                "key material found inside .oo/: {s:?}"
            );
        }
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for k in &found {
            let m = fs::metadata(k).unwrap().permissions().mode() & 0o777;
            assert_eq!(m, 0o600, "node key {k:?} is not 0600: {m:o}");
        }
    }
}

/// R3 — `%source` is the node, not the port.
///
/// PAIRED: two ports on one workspace must give the SAME `%source`, and two
/// workspaces must give different ones. Today it is `node:<port>`, which gets
/// both halves exactly backwards.
#[test]
fn red_source_identifies_the_node_not_the_port() {
    let home = fresh_home("r3");
    let a = fresh_dir("r3-a");
    let caid = store(&a, &home, "{ pkg: \"src\" }");
    let b = fresh_dir("r3-b");
    store(&b, &home, "{ pkg: \"src\" }");

    let ask = |n: &Node| {
        let r = ask_raw(n.port, &format!("{{{{ %op: #fetch, %hash: \"{caid}\" }}}}"));
        let i = r.find("\"%source\"").unwrap_or_else(|| {
            panic!("no %source in the reply: {r}");
        });
        r[i..].chars().take(90).collect::<String>()
    };

    let (s1, s2) = {
        let n1 = serve(&a, &home);
        let one = ask(&n1);
        drop(n1);
        let n2 = serve(&a, &home);
        let two = ask(&n2);
        (one, two)
    };
    assert_eq!(
        s1, s2,
        "one workspace served on two ports reported two different sources"
    );

    let nb = serve(&b, &home);
    let sb = ask(&nb);
    assert_ne!(s1, sb, "two different workspaces reported the same source");
}

/// R4 — the request carries `%from`.
#[test]
fn red_requests_carry_from() {
    let home = fresh_home("r4");
    let client = fresh_dir("r4");
    let rec = spawn_recorder();
    let _ = fetch_from(
        &client,
        &home,
        &format!("tcp://127.0.0.1:{}", rec.port),
        &format!("hash:sha256:v1:{}", "a".repeat(64)),
        30,
    );
    let seen = rec.seen.lock().unwrap().clone();
    assert!(!seen.is_empty(), "harness: the client never sent anything");
    assert!(
        seen[0].contains("%from"),
        "the request carries no %from: {:?}",
        seen[0]
    );

    // And what it carries is this workspace's node id, not a placeholder.
    let id = node_id(&client, &home);
    assert!(
        seen[0].contains(&id),
        "%from is not this node's id ({id}): {:?}",
        seen[0]
    );
}

/// R5 — the transition-period bare-CAID request is retired.
///
/// v0.2.48 accepted it as a **declared, dated** compatibility surface and the
/// spec put its removal in this arc. PAIRED: the envelope form still works, so
/// a pass cannot come from the node having stopped answering altogether.
#[test]
fn red_the_legacy_bare_caid_request_is_retired() {
    let home = fresh_home("r5");
    let d = fresh_dir("r5");
    let caid = store(&d, &home, "{ pkg: \"legacy\" }");
    let node = serve(&d, &home);

    // CONTROL: the envelope form works.
    let ok = ask_raw(
        node.port,
        &format!("{{{{ %op: #fetch, %hash: \"{caid}\" }}}}"),
    );
    assert!(
        ok.contains("legacy"),
        "control: the envelope form stopped working: {ok}"
    );

    let legacy = ask_raw(node.port, &caid);
    assert!(
        !legacy.contains("legacy"),
        "the retired bare-CAID form is still served: {legacy}"
    );
    assert!(
        !legacy.is_empty(),
        "a retired request form must still be answered, not met with silence"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// PINS — green at baseline, must stay green
// ─────────────────────────────────────────────────────────────────────────

/// P1 — a forged `%from` changes nothing.
///
/// `%from` is unsigned; any peer can put anything there. If any outcome moved
/// with it, the field would have become authentication by accident. Green at
/// baseline because the field does not exist yet; it is exactly what an
/// implementation that started keying decisions on `%from` would break.
#[test]
fn pin_a_forged_from_changes_nothing() {
    let home = fresh_home("p1");
    let d = fresh_dir("p1");
    let caid = store(&d, &home, "{ pkg: \"claim\" }");
    let node = serve(&d, &home);

    let honest = ask_raw(
        node.port,
        &format!("{{{{ %op: #fetch, %hash: \"{caid}\" }}}}"),
    );
    let forged = ask_raw(
        node.port,
        &format!(
            "{{{{ %op: #fetch, %hash: \"{caid}\", %from: \"hash:sha256:v1:{}\" }}}}",
            "f".repeat(64)
        ),
    );
    assert!(honest.contains("claim"), "harness: the honest ask failed");
    assert_eq!(
        honest, forged,
        "the answer moved with a value any peer can invent"
    );
}

/// P2 — the operator key is not the node key.
///
/// Two questions, two keys: who authorises (governance, `#refine`) and which
/// machine answered. If one file served both, a workspace copy would carry the
/// operator's signing key — the thing v0.2.46 exists to prevent.
#[test]
fn pin_operator_and_node_keys_are_not_the_same_file() {
    let home = fresh_home("p2");
    let d = fresh_dir("p2");
    write(&d, "s.n", "v: 1\n");
    oo(&d, &home, &["evolve", "s.n"]);
    oo(&d, &home, &["commit", "-m", "x"]);

    let op = oo(&d, &home, &["identity"]);
    let op_key = op
        .split(|c: char| !c.is_ascii_hexdigit())
        .find(|w| w.len() == 64)
        .unwrap_or_else(|| panic!("harness: `oo identity` printed no key: {op}"))
        .to_string();
    let op_path = home.join("identity-for-tests");
    assert!(op_path.exists(), "harness: the operator key was not minted");

    // Whatever the node key is, it must not be the operator's bytes.
    let op_bytes = fs::read(&op_path).unwrap();
    fn walk(p: &Path, out: &mut Vec<PathBuf>) {
        if let Ok(rd) = fs::read_dir(p) {
            for e in rd.flatten() {
                let q = e.path();
                if q.is_dir() {
                    walk(&q, out)
                } else {
                    out.push(q)
                }
            }
        }
    }
    let mut all = Vec::new();
    walk(&home, &mut all);
    for f in all {
        if f == op_path {
            continue;
        }
        assert_ne!(
            fs::read(&f).unwrap_or_default(),
            op_bytes,
            "the operator's private key was reused as another key: {f:?}"
        );
    }
    assert_eq!(op_key.len(), 64);
}

/// P8 — the node key is behind the language-layer boundary, and not by luck.
///
/// ACCEPTOR REPAIR pin. REAL_01 §7.5.3 already requires a private key to be
/// inside SPEC_08 §6.3's boundary and that **the protection must not depend on
/// the path happening to contain a store-directory component**. Measured on the
/// delivered build: `~%Io./read_file` on a node key answered `#none` —
/// *permitted*, and unreadable only because PKCS#8 DER is not valid UTF-8.
/// Protection by coincidence, one byte-reading builtin away from none at all.
///
/// The CONTROL is the point: a file outside the directory stays readable, so
/// this pins a boundary rather than a blanket refusal.
#[test]
fn pin_the_node_key_is_refused_to_the_language_layer() {
    let home = fresh_home("p8");
    let d = fresh_dir("p8");
    write(&d, "s.n", "v: 1\n");
    let id = node_id(&d, &home);
    assert_eq!(id.len(), 64.max(id.len()), "harness: no node id");

    let nodes = home.join("nodes");
    let key = fs::read_dir(&nodes)
        .unwrap_or_else(|e| panic!("harness: no nodes dir ({e})"))
        .flatten()
        .map(|e| e.path())
        .find(|p| p.is_file())
        .expect("harness: no node key was minted");

    let refused = oo(
        &d,
        &home,
        &["eval", &format!("~%Io./read_file(\"{}\")", key.display())],
    );
    assert!(
        refused.contains("store_boundary"),
        "the language layer can reach a node key: {refused}"
    );

    // CONTROL: outside the directory is still ordinary ground.
    let outside = home.join("outside.txt");
    fs::write(&outside, b"readable").unwrap();
    let ok = oo(
        &d,
        &home,
        &[
            "eval",
            &format!("~%Io./read_file(\"{}\")", outside.display()),
        ],
    );
    assert!(
        !ok.contains("store_boundary"),
        "control: the guard became a blanket refusal: {ok}"
    );
}

/// P3 — the four-way discriminator survives (v0.2.48's headline).
#[test]
fn pin_four_peer_states_stay_distinguishable() {
    let home = fresh_home("p3");
    let holder = fresh_dir("p3-holder");
    let good = store(&holder, &home, "{ pkg: \"held\" }");
    let missing = format!("hash:sha256:v1:{}", "e".repeat(64));
    let node = serve(&holder, &home);
    let addr = format!("tcp://127.0.0.1:{}", node.port);
    let client = fresh_dir("p3-client");

    let has = fetch_from(&client, &home, &addr, &good, 30);
    let lacks = fetch_from(&client, &home, &addr, &missing, 30);
    assert!(
        has.contains("held") && !has.contains("_|_"),
        "harness: the honest fetch failed: {has}"
    );
    assert!(
        lacks.contains("missing_key"),
        "absence stopped saying absence: {lacks}"
    );
    assert_ne!(has, lacks);
}

/// P4 — ordinary work mints no node key. A workspace that never touches the
/// network is not a node yet.
#[test]
fn pin_ordinary_work_mints_no_node_key() {
    let home = fresh_home("p4");
    let d = fresh_dir("p4");
    write(&d, "s.n", "v: { hello: \"world\" }\n");
    oo(&d, &home, &["run", "s.n"]);
    oo(&d, &home, &["status"]);
    oo(&d, &home, &["evolve", "s.n"]);
    assert!(
        oo(&d, &home, &["commit", "-m", "x"]).contains("Commit successful"),
        "harness: commit"
    );
    oo(&d, &home, &["log"]);

    let nodes_dir = home.join("nodes");
    assert!(
        !nodes_dir.exists() || fs::read_dir(&nodes_dir).unwrap().next().is_none(),
        "ordinary work minted a node key at {nodes_dir:?}"
    );
}

/// P5 — an ordinary value's address does not move.
#[test]
fn pin_ordinary_value_caids_do_not_move() {
    let home = fresh_home("p5");
    let d = fresh_dir("p5");
    assert_eq!(
        store(&d, &home, "{ hello: \"world\" }"),
        GOLDEN_VALUE_CAID,
        "an ordinary value's address moved"
    );
}

/// P6 — the universe root stays deterministic. It is what disqualifies the
/// universe as a node address, and it must remain true for that reason to keep
/// holding.
#[test]
fn pin_root_caid_stays_deterministic() {
    let home = fresh_home("p6");
    let digests: std::collections::BTreeSet<String> = (0..3)
        .map(|i| {
            let d = fresh_dir(&format!("p6-{i}"));
            write(&d, "s.n", "v: { hello: \"world\" }\n");
            oo(&d, &home, &["evolve", "s.n"]);
            assert!(
                oo(&d, &home, &["commit", "-m", "x"]).contains("Commit successful"),
                "harness: commit"
            );
            let commit = oo(&d, &home, &["log"])
                .lines()
                .find_map(|l| l.strip_prefix("commit ").map(|s| s.trim().to_string()))
                .unwrap();
            let h = commit.rsplit(':').next().unwrap().to_string();
            let p = d
                .join(".oo")
                .join("objects")
                .join("sha256")
                .join(&h[..2])
                .join(&h[2..]);
            let j: serde_json::Value = serde_json::from_slice(&fs::read(&p).unwrap()).unwrap();
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

/// P7 — two engines still federate.
#[test]
fn pin_two_engines_federate_end_to_end() {
    let home = fresh_home("p7");
    let holder = fresh_dir("p7-holder");
    let caid = store(&holder, &home, "{ pkg: \"federated\" }");
    let node = serve(&holder, &home);
    let client = fresh_dir("p7-client");
    let got = fetch_from(
        &client,
        &home,
        &format!("tcp://127.0.0.1:{}", node.port),
        &caid,
        30,
    );
    assert!(
        got.contains("federated") && !got.contains("_|_"),
        "two engines no longer federate: {got}"
    );
}
