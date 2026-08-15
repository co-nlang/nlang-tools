// Direct observation provenance (2026-07-31, pre-committed by work order:
// docs/direct_observation_provenance_handover.md).
//
// This probe deliberately does not add an admission policy. It asks a narrower
// question first: can a receiver distinguish the host it observed on the
// connection carrying #advertise from a host asserted by a #discover relay?
//
// The signed advertisement is kept byte-for-byte. The expected `provenance`
// field is receiver-local durable metadata; it must not enter the signed body
// or the #discover response. The reds are ignored until the engine delivery;
// the controls at the top are live now and make every absence assertion below
// non-vacuous.

mod common;

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ring::signature::{Ed25519KeyPair, KeyPair};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);
const ADVERT_DOMAIN: &str = "oodp-advert:v1:";
const PEERS_FILE: &str = "directory";

// ── harness ──────────────────────────────────────────────────────────────

fn fresh_dir(tag: &str) -> nlang_interpreter::ScratchDir {
    nlang_interpreter::ScratchDir::new(&format!("direct-provenance-{tag}"))
}

fn oo_cmd(dir: &Path) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_oo"));
    c.current_dir(dir)
        .env("OO_IDENTITY", dir.join("identity-for-tests"))
        .env("OO_NODE_HOME", dir.join("node-home-for-tests"));
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

fn write_source(dir: &Path, name: &str, src: &str) {
    fs::write(dir.join(name), src).unwrap();
}

fn init(dir: &Path) {
    oo(dir, &["run", "--help"]);
    write_source(dir, "seed.n", "seed: { ok: #true }\n");
    oo(dir, &["run", "seed.n"]);
}

fn first_string(out: &str) -> String {
    let s = out
        .split_once('"')
        .unwrap_or_else(|| panic!("no string atom in {out:?}"))
        .1;
    s.split('"').next().unwrap().to_string()
}

fn store(dir: &Path, expr: &str) -> String {
    write_source(
        dir,
        "service.n",
        &format!("id: ~%Discovery./identify_and_store {expr}\n"),
    );
    let caid = first_string(&oo(dir, &["run", "service.n", "--observe", "id"]));
    assert!(caid.starts_with("hash:sha256:"), "store() got {caid:?}");
    caid
}

fn caid_of(dir: &Path, expr: &str) -> String {
    let caid = first_string(&oo(
        dir,
        &["eval", &format!("~%Discovery./identify {expr}")],
    ));
    assert!(caid.starts_with("hash:sha256:"), "caid_of() got {caid:?}");
    caid
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

fn copy_tree(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).unwrap();
    for entry in fs::read_dir(src).unwrap().flatten() {
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            copy_tree(&from, &to);
        } else {
            fs::copy(&from, &to).unwrap();
        }
    }
}

fn peers_path(dir: &Path) -> PathBuf {
    dir.join(".oo").join("peers").join(PEERS_FILE)
}

// ── node and signed-advert fixtures ─────────────────────────────────────

struct NodeKey {
    node_id: String,
    key_pair: Ed25519KeyPair,
    public_key_hex: String,
}

fn node_key(dir: &Path) -> NodeKey {
    let out = oo(dir, &["node", "id"]);
    let node_id = out
        .lines()
        .find(|line| line.starts_with("hash:"))
        .unwrap_or_else(|| panic!("`oo node id` printed no CAID: {out:?}"))
        .trim()
        .to_string();
    let path = out
        .lines()
        .find_map(|line| line.strip_prefix("path:"))
        .unwrap_or_else(|| panic!("`oo node id` printed no key path: {out:?}"))
        .trim()
        .to_string();
    let pkcs8 = fs::read(&path).unwrap_or_else(|e| panic!("read node key {path}: {e}"));
    let key_pair = Ed25519KeyPair::from_pkcs8(&pkcs8)
        .unwrap_or_else(|e| panic!("node key at {path} is not Ed25519: {e:?}"));
    NodeKey {
        node_id,
        public_key_hex: hex::encode(key_pair.public_key().as_ref()),
        key_pair,
    }
}

struct Advert {
    node_id: String,
    public_key_hex: String,
    service: String,
    listen_port: u16,
    ts: i64,
}

impl Advert {
    fn body(&self) -> String {
        format!(
            "{{{{ node_id: \"{}\", public_key: \"{}\", services: [\"{}\"], \
             listen_port: {}, capacity: 10, ts: {}, ttl: 15 }}}}",
            self.node_id, self.public_key_hex, self.service, self.listen_port, self.ts
        )
    }

    fn signed(&self, caid_dir: &Path, signer: &Ed25519KeyPair) -> String {
        let body = self.body();
        let body_caid = caid_of(caid_dir, &body);
        let sig = hex::encode(
            signer
                .sign(format!("{ADVERT_DOMAIN}{body_caid}").as_bytes())
                .as_ref(),
        );
        let inner = body.trim_start_matches("{{").trim_end_matches("}}").trim();
        format!("{{{{ {inner}, signature: \"{sig}\" }}}}")
    }
}

fn advert_for(source: &Path, key: &NodeKey, service: &str, listen_port: u16, ts: i64) -> String {
    Advert {
        node_id: key.node_id.clone(),
        public_key_hex: key.public_key_hex.clone(),
        service: service.to_string(),
        listen_port,
        ts,
    }
    .signed(source, &key.key_pair)
}

fn advert_request(from: &str, ad: &str) -> String {
    format!("{{{{ %op: #advertise, %from: \"{from}\", %ad: {ad} }}}}\n")
}

fn discover_request(from: &str, target: &str) -> String {
    format!("{{{{ %op: #discover, %from: \"{from}\", %target: \"{target}\" }}}}\n")
}

fn status_of(reply: &str) -> String {
    serde_json::from_str::<serde_json::Value>(reply.trim())
        .ok()
        .and_then(|v| {
            v.get("%status")
                .or_else(|| v.get("status"))
                .and_then(|s| s.as_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "<none>".to_string())
        .trim_start_matches('#')
        .to_string()
}

fn peer_entries(reply: &str) -> Vec<(String, String)> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(reply.trim()) else {
        return vec![];
    };
    let Some(entries) = value.get("%peers").and_then(|v| v.as_array()) else {
        return vec![];
    };
    entries
        .iter()
        .filter_map(|entry| {
            Some((
                entry.get("%ad")?.as_str()?.to_string(),
                entry
                    .get("%observed_host")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
            ))
        })
        .collect()
}

struct Node {
    child: Child,
    port: u16,
    log: PathBuf,
}

impl Drop for Node {
    fn drop(&mut self) {
        self.child.kill().ok();
        self.child.wait().ok();
    }
}

impl Node {
    fn addr(&self) -> String {
        format!("127.0.0.1:{}", self.port)
    }

    fn stop(mut self) {
        self.child.kill().ok();
        self.child.wait().ok();
    }

    fn log(&self) -> String {
        fs::read_to_string(&self.log).unwrap_or_default()
    }
}

fn serve(dir: &Path) -> Node {
    let served = common::serve(oo_cmd(dir), dir.join("serve.log"));
    Node { child: served.child, port: served.port, log: served.log }
}

fn ask_raw(port: u16, payload: &str) -> String {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    stream.write_all(payload.as_bytes()).unwrap();
    if !payload.ends_with('\n') {
        stream.write_all(b"\n").unwrap();
    }
    stream.flush().unwrap();
    stream.shutdown(std::net::Shutdown::Write).ok();
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).unwrap();
    String::from_utf8_lossy(&buf).to_string()
}

fn advertise_direct(node: &Node, key: &NodeKey, ad: &str) {
    let reply = ask_raw(node.port, &advert_request(&key.node_id, ad));
    assert_eq!(
        status_of(&reply),
        "success",
        "direct advertisement failed: {reply}"
    );
}

fn oo_discover(dir: &Path, relayer: &str, target: &str) -> String {
    oo(
        dir,
        &["node", "discover", "--to", relayer, "--target", target],
    )
}

// ── fake relayer ─────────────────────────────────────────────────────────

struct Relayer {
    port: u16,
    asked: Arc<Mutex<Vec<String>>>,
}

impl Relayer {
    fn addr(&self) -> String {
        format!("127.0.0.1:{}", self.port)
    }

    fn was_contacted(&self) -> bool {
        !self.asked.lock().unwrap().is_empty()
    }
}

fn spawn_relayer(source_id: &str, hops: i64, ad: &str, host: &str) -> Relayer {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let asked = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&asked);
    let reply = serde_json::json!({
        "%status": "#success",
        "%source": source_id,
        "%hops": hops,
        "%peers": [{"%ad": ad, "%observed_host": host}],
    })
    .to_string();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let Ok(clone) = stream.try_clone() else {
                continue;
            };
            let mut line = String::new();
            if BufReader::new(clone).read_line(&mut line).is_err() {
                continue;
            }
            seen.lock().unwrap().push(line);
            let _ = stream.write_all(reply.as_bytes());
            let _ = stream.write_all(b"\n");
            let _ = stream.flush();
            let _ = stream.shutdown(std::net::Shutdown::Write);
        }
    });
    Relayer { port, asked }
}

// ── durable-record inspection ────────────────────────────────────────────

#[derive(Debug)]
struct Record {
    node_id: String,
    ad: String,
    observed_host: String,
    addr: String,
    hops: i64,
    provenance: Option<String>,
}

fn records(dir: &Path) -> Vec<Record> {
    let text = fs::read_to_string(peers_path(dir))
        .unwrap_or_else(|e| panic!("durable peer directory missing: {e}"));
    text.lines()
        .skip(1)
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| {
            let value = serde_json::from_str::<serde_json::Value>(line).ok()?;
            Some(Record {
                node_id: value.get("node_id")?.as_str()?.to_string(),
                ad: value.get("ad")?.as_str()?.to_string(),
                observed_host: value
                    .get("observed_host")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                addr: value
                    .get("addr")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                hops: value.get("hops").and_then(|v| v.as_i64()).unwrap_or(0),
                provenance: value
                    .get("provenance")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
            })
        })
        .collect()
}

fn latest_record(dir: &Path, node_id: &str) -> Record {
    records(dir)
        .into_iter()
        .filter(|r| r.node_id == node_id)
        .last()
        .unwrap_or_else(|| panic!("no durable record for {node_id}"))
}

fn assert_provenance(record: &Record, expected: &str) {
    assert_eq!(
        record.provenance.as_deref(),
        Some(expected),
        "record has no explicit {expected} provenance: {record:?}"
    );
}

fn remove_field_from_records(dir: &Path, field: &str) {
    let path = peers_path(dir);
    let text = fs::read_to_string(&path).unwrap();
    let mut lines = Vec::new();
    for (index, line) in text.lines().enumerate() {
        if index == 0 || line.trim().is_empty() {
            lines.push(line.to_string());
            continue;
        }
        let mut value: serde_json::Value = serde_json::from_str(line).unwrap();
        value.as_object_mut().unwrap().remove(field);
        lines.push(serde_json::to_string(&value).unwrap());
    }
    fs::write(path, lines.join("\n") + "\n").unwrap();
}

// ════════════════════════════════════════════════════════════════════════
//  CONTROLS — live before and after delivery
// ════════════════════════════════════════════════════════════════════════

/// C0 — both entry paths are live, and both leave a real durable record.
#[test]
fn c0_direct_and_relayed_advertisements_are_both_live() {
    let source = fresh_dir("c0-source");
    let relay = fresh_dir("c0-relay");
    let direct_receiver = fresh_dir("c0-direct");
    let relayed_receiver = fresh_dir("c0-relayed");
    init(&source);
    init(&relay);
    init(&direct_receiver);
    init(&relayed_receiver);

    let key = node_key(&source);
    let service = store(&source, "{ provenance_control: true }");
    let ad = advert_for(&source, &key, &service, 24100, now_secs());

    let direct_node = serve(&direct_receiver);
    advertise_direct(&direct_node, &key, &ad);
    assert!(
        !records(&direct_receiver).is_empty(),
        "direct acceptance produced no durable record"
    );
    direct_node.stop();

    let relay_node = serve(&relay);
    advertise_direct(&relay_node, &key, &ad);
    let out = oo_discover(&relayed_receiver, &relay_node.addr(), &service);
    assert!(out.contains("#success"), "relayed discover failed: {out}");
    assert!(
        out.contains(&key.node_id),
        "relayed discover returned no source advertisement: {out}"
    );
    assert!(
        !records(&relayed_receiver).is_empty(),
        "relayed acceptance produced no durable record"
    );
}

/// C1 — existing restart and copy semantics are live before provenance is
/// delivered: a same-owner restart restores the observed host, while a copy
/// retains the signed record but not the observer-local host.
#[test]
fn c1_restart_and_copy_still_discriminate_the_observer_half() {
    let source = fresh_dir("c1-source");
    let receiver = fresh_dir("c1-receiver");
    init(&source);
    init(&receiver);

    let key = node_key(&source);
    let service = store(&source, "{ provenance_restart_copy: true }");
    let ad = advert_for(&source, &key, &service, 24101, now_secs());

    let node = serve(&receiver);
    advertise_direct(&node, &key, &ad);
    node.stop();

    let restarted = serve(&receiver);
    let before_copy = peer_entries(&ask_raw(
        restarted.port,
        &discover_request("control", &service),
    ));
    assert_eq!(
        before_copy.len(),
        1,
        "restart lost the signed advertisement"
    );
    assert_eq!(
        before_copy[0].1, "127.0.0.1",
        "same-owner restart lost its observed host"
    );
    restarted.stop();

    let copy = fresh_dir("c1-copy");
    copy_tree(&receiver, &copy);
    let copied = serve(&copy);
    let after_copy = peer_entries(&ask_raw(
        copied.port,
        &discover_request("control", &service),
    ));
    assert_eq!(after_copy.len(), 1, "copy lost the signed advertisement");
    assert_eq!(
        after_copy[0].1, "",
        "copy claimed an observation it never made: {after_copy:?}"
    );
}

// ════════════════════════════════════════════════════════════════════════
//  REDS — remove #[ignore] only when the delivery accepts this work order
// ════════════════════════════════════════════════════════════════════════

/// R1 — a direct #advertise receipt is marked direct in durable local state.
#[test]
fn r1_direct_receipt_records_direct_provenance() {
    let source = fresh_dir("r1-source");
    let receiver = fresh_dir("r1-receiver");
    init(&source);
    init(&receiver);
    let key = node_key(&source);
    let service = store(&source, "{ provenance_direct: true }");
    let ad = advert_for(&source, &key, &service, 24200, now_secs());

    let node = serve(&receiver);
    advertise_direct(&node, &key, &ad);
    let record = latest_record(&receiver, &key.node_id);
    assert_eq!(record.ad, ad, "the durable record changed the signed bytes");
    assert_eq!(record.observed_host, "127.0.0.1");
    assert_eq!(record.addr, "127.0.0.1:24200");
    assert_eq!(record.hops, 0);
    assert_provenance(&record, "direct");
}

/// R2 — a #discover receipt is explicitly relayed, even though this process
/// opened a TCP connection to the relayer.
#[test]
fn r2_relayed_receipt_records_relayed_provenance() {
    let source = fresh_dir("r2-source");
    let relay = fresh_dir("r2-relay");
    let receiver = fresh_dir("r2-receiver");
    init(&source);
    init(&relay);
    init(&receiver);
    let key = node_key(&source);
    let service = store(&source, "{ provenance_relayed: true }");
    let ad = advert_for(&source, &key, &service, 24201, now_secs());

    let relay_node = serve(&relay);
    advertise_direct(&relay_node, &key, &ad);
    let out = oo_discover(&receiver, &relay_node.addr(), &service);
    assert!(out.contains("#success"), "relay path failed: {out}");
    let record = latest_record(&receiver, &key.node_id);
    assert_eq!(record.ad, ad, "relay changed the signed bytes");
    assert_provenance(&record, "relayed");
    assert_ne!(record.provenance.as_deref(), Some("direct"));
}

/// R3 — for one exact signed advertisement, direct wins over relayed in both
/// arrival orders. The relayer's host must not replace the direct observation.
#[test]
fn r3_direct_is_authoritative_for_the_same_signed_advertisement() {
    let source = fresh_dir("r3-source");
    let direct_first = fresh_dir("r3-direct-first");
    let relay_first = fresh_dir("r3-relay-first");
    init(&source);
    init(&direct_first);
    init(&relay_first);
    let key = node_key(&source);
    let service = store(&source, "{ provenance_order: true }");
    let ad = advert_for(&source, &key, &service, 24202, now_secs());

    let node = serve(&direct_first);
    advertise_direct(&node, &key, &ad);
    let relayer = spawn_relayer(&key.node_id, 1, &ad, "198.51.100.20");
    let out = oo_discover(&direct_first, &relayer.addr(), &service);
    assert!(
        relayer.was_contacted(),
        "direct-first relayer was not contacted"
    );
    assert!(out.contains("#success"), "direct-first relay failed: {out}");
    let direct_first_record = latest_record(&direct_first, &key.node_id);

    let relayer = spawn_relayer(&key.node_id, 1, &ad, "198.51.100.21");
    let out = oo_discover(&relay_first, &relayer.addr(), &service);
    assert!(out.contains("#success"), "relay-first relay failed: {out}");
    let node = serve(&relay_first);
    advertise_direct(&node, &key, &ad);
    let relay_first_record = latest_record(&relay_first, &key.node_id);

    // Both arrival orders were exercised before either negative assertion. A
    // failure below therefore names provenance/merge semantics, not a setup
    // path that was never run.
    assert_provenance(&direct_first_record, "direct");
    assert_eq!(direct_first_record.observed_host, "127.0.0.1");
    assert_provenance(&relay_first_record, "direct");
    assert_eq!(relay_first_record.observed_host, "127.0.0.1");
}

/// R4 — a newer signed advertisement from the same node does not inherit the
/// direct status of an older one. Provenance follows the exact ad identity.
#[test]
fn r4_a_different_signed_ad_does_not_inherit_direct_provenance() {
    let source = fresh_dir("r4-source");
    let receiver = fresh_dir("r4-receiver");
    init(&source);
    init(&receiver);
    let key = node_key(&source);
    let service = store(&source, "{ provenance_exact_ad: true }");
    let t0 = now_secs();
    let old_ad = advert_for(&source, &key, &service, 24203, t0);
    let new_ad = advert_for(&source, &key, &service, 24204, t0 + 1);

    let node = serve(&receiver);
    advertise_direct(&node, &key, &old_ad);
    let relayer = spawn_relayer(&key.node_id, 1, &new_ad, "198.51.100.22");
    let out = oo_discover(&receiver, &relayer.addr(), &service);
    assert!(out.contains("#success"), "new relayed ad failed: {out}");
    let record = latest_record(&receiver, &key.node_id);
    assert_eq!(
        record.ad, new_ad,
        "the newer signed ad did not replace the old one"
    );
    assert_eq!(record.observed_host, "198.51.100.22");
    assert_provenance(&record, "relayed");
}

/// R5 — a second relay carrying the exact signed body remains relayed. The
/// claimed hop count never upgrades a relay assertion to a local observation.
#[test]
fn r5_a_forwarded_relayed_observation_stays_relayed() {
    let source = fresh_dir("r5-source");
    let receiver = fresh_dir("r5-receiver");
    init(&source);
    init(&receiver);
    let key = node_key(&source);
    let service = store(&source, "{ provenance_forwarded: true }");
    let ad = advert_for(&source, &key, &service, 24205, now_secs());

    let first = spawn_relayer(&key.node_id, 1, &ad, "198.51.100.23");
    let out = oo_discover(&receiver, &first.addr(), &service);
    assert!(out.contains("#success"), "first relay failed: {out}");

    let second = spawn_relayer(&key.node_id, 2, &ad, "198.51.100.24");
    let out = oo_discover(&receiver, &second.addr(), &service);
    assert!(second.was_contacted(), "second relay was not contacted");
    assert!(out.contains("#success"), "forwarded relay failed: {out}");
    let record = latest_record(&receiver, &key.node_id);
    assert_provenance(&record, "relayed");
    assert_eq!(record.observed_host, "198.51.100.24");
    assert_eq!(record.hops, 2);
}

/// R6 — `%hops: 0` and `%hops: 1` are both relay assertions. Neither is a
/// shortcut to direct provenance.
#[test]
fn r6_zero_and_one_claimed_hops_are_both_relayed() {
    let source = fresh_dir("r6-source");
    let zero = fresh_dir("r6-zero");
    let one = fresh_dir("r6-one");
    init(&source);
    init(&zero);
    init(&one);
    let key = node_key(&source);
    let service = store(&source, "{ provenance_hops: true }");
    let ad = advert_for(&source, &key, &service, 24206, now_secs());

    let mut records_by_hops = Vec::new();
    for (receiver, hops, host) in [
        (&zero, 0_i64, "198.51.100.25"),
        (&one, 1_i64, "198.51.100.26"),
    ] {
        let relayer = spawn_relayer(&key.node_id, hops, &ad, host);
        let out = oo_discover(receiver, &relayer.addr(), &service);
        assert!(out.contains("#success"), "hops={hops} relay failed: {out}");
        records_by_hops.push((latest_record(receiver, &key.node_id), host, hops));
    }

    // Exercise both claims before asserting either one. `%hops` is remote
    // input in both cases; neither value may be used as a directness shortcut.
    for (record, host, hops) in records_by_hops {
        assert_provenance(&record, "relayed");
        assert_eq!(record.observed_host, host);
        assert_eq!(record.hops, hops);
    }
}

/// R7 — a same-owner restart preserves provenance. The direct case is paired
/// with a post-restart relay so a loader that silently defaults to relayed or
/// unknown cannot pass by merely preserving bytes on disk.
#[test]
fn r7_restart_preserves_direct_and_relayed_provenance() {
    let source = fresh_dir("r7-source");
    let direct = fresh_dir("r7-direct");
    let relayed = fresh_dir("r7-relayed");
    init(&source);
    init(&direct);
    init(&relayed);
    let key = node_key(&source);
    let service = store(&source, "{ provenance_restart: true }");
    let ad = advert_for(&source, &key, &service, 24207, now_secs());

    let node = serve(&direct);
    advertise_direct(&node, &key, &ad);
    node.stop();
    let restarted = serve(&direct);
    let observed = peer_entries(&ask_raw(
        restarted.port,
        &discover_request("restart", &service),
    ));
    assert_eq!(observed.len(), 1, "restart lost the direct record");
    assert_eq!(observed[0].1, "127.0.0.1");
    restarted.stop();

    let relayer = spawn_relayer(&key.node_id, 1, &ad, "198.51.100.27");
    let out = oo_discover(&direct, &relayer.addr(), &service);
    assert!(out.contains("#success"), "post-restart relay failed: {out}");
    let direct_record = latest_record(&direct, &key.node_id);

    let relayer = spawn_relayer(&key.node_id, 1, &ad, "198.51.100.28");
    let out = oo_discover(&relayed, &relayer.addr(), &service);
    assert!(
        out.contains("#success"),
        "initial relayed receipt failed: {out}"
    );
    let relayed_record = latest_record(&relayed, &key.node_id);

    // The same-owner restart was exercised before these assertions. The
    // relayed fixture is checked from the durable record because the current
    // CLI's discover writer intentionally stores a sparse, non-service index
    // row (ts=0/services=[]), which is not a provenance fact.
    assert_provenance(&direct_record, "direct");
    assert_eq!(direct_record.observed_host, "127.0.0.1");
    assert_provenance(&relayed_record, "relayed");
    assert_eq!(relayed_record.observed_host, "198.51.100.28");
}

/// R8 — copying a workspace clears local provenance together with the local
/// observation. A later relay may populate relayed provenance, but cannot make
/// the copied record direct.
#[test]
fn r8_copy_clears_local_provenance_before_a_later_relay() {
    let source = fresh_dir("r8-source");
    let original = fresh_dir("r8-original");
    init(&source);
    init(&original);
    let key = node_key(&source);
    let service = store(&source, "{ provenance_copy: true }");
    let ad = advert_for(&source, &key, &service, 24208, now_secs());

    let node = serve(&original);
    advertise_direct(&node, &key, &ad);
    node.stop();

    let copy = fresh_dir("r8-copy");
    copy_tree(&original, &copy);
    let node = serve(&copy);
    let copied = peer_entries(&ask_raw(node.port, &discover_request("copy", &service)));
    assert_eq!(copied.len(), 1, "copy lost the signed advertisement");
    assert_eq!(
        copied[0].1, "",
        "copy claimed the original host: {copied:?}"
    );
    node.stop();

    let relayer = spawn_relayer(&key.node_id, 1, &ad, "198.51.100.29");
    let out = oo_discover(&copy, &relayer.addr(), &service);
    assert!(out.contains("#success"), "relay into copy failed: {out}");
    let record = latest_record(&copy, &key.node_id);
    assert_provenance(&record, "relayed");
    assert_eq!(record.observed_host, "198.51.100.29");
}

/// R9 — a legacy line without the optional field is unknown, never direct. A
/// same-ad relayed update must therefore replace its old host after reload.
#[test]
fn r9_legacy_missing_provenance_is_conservative() {
    let source = fresh_dir("r9-source");
    let receiver = fresh_dir("r9-receiver");
    init(&source);
    init(&receiver);
    let key = node_key(&source);
    let service = store(&source, "{ provenance_legacy: true }");
    let ad = advert_for(&source, &key, &service, 24209, now_secs());

    let node = serve(&receiver);
    advertise_direct(&node, &key, &ad);
    node.stop();
    let before = latest_record(&receiver, &key.node_id);
    assert_eq!(before.ad, ad, "legacy fixture lost its signed body");
    remove_field_from_records(&receiver, "provenance");

    let relayer = spawn_relayer(&key.node_id, 1, &ad, "198.51.100.30");
    let out = oo_discover(&receiver, &relayer.addr(), &service);
    assert!(out.contains("#success"), "legacy relay failed: {out}");

    // The client-side discover writer stores services=[] and ts=0 by design;
    // this gate is about the durable merge result, not about serving that
    // sparse row through the stale-filtered service index.
    let record = latest_record(&receiver, &key.node_id);
    assert_eq!(record.ad, ad, "legacy relay changed the signed bytes");
    assert_eq!(record.observed_host, "198.51.100.30");
    assert_provenance(&record, "relayed");
}
