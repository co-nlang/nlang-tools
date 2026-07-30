// Durable peer directory (2026-07-29, pre-committed by work order:
// docs/advert_persistence_handover.md).
//
// ── Why this file exists at all ──────────────────────────────────────────
//
// The `kademlia_table` arc's delivery persisted the routing state and
// acceptance reverted it. The pin it left behind says why in its own failure
// message: durable OODP state "carries GC, migration, a REAL_02 §5.1 clause,
// and the fact that incumbent-first stops being reset by a restart. Arriving
// as a side effect is how persistence arrives unaudited."
//
// So this arc is that pin's expiry date, and `p4_nothing_persisted` is listed
// in the work order as SCHEDULED TO CHANGE rather than quietly edited. A pin
// whose content is "X has not happened yet" is a countdown timer.
//
// ── The three probes that are not here, and why ──────────────────────────
//
// Calibration removed three gates that would have passed at v0.2.53 for the
// wrong reason. Recording them is cheaper than re-deriving why they are gone:
//
//   * "cumulative writes < 1 MB" — at baseline the engine writes ZERO bytes,
//     so the bound passes trivially. R6 therefore asserts writes are
//     non-zero AND bounded; the first clause is what makes it red today.
//     (Standing rule: a comparison must first prove both sides non-empty.)
//   * "`.oo/format` is not bumped" — nothing changes it today, so it is an
//     invariant, not a target. It is P7.
//   * "a previous release still opens this store" — the file does not exist
//     at baseline, so there is nothing to hand the old binary. Cross-version
//     is an acceptance measurement (§7 of the order), and the invariant that
//     makes it possible — an unknown `.oo/` entry does not break this engine —
//     is P6, which is green today and must stay green.
//
// ── An honest limitation of this harness ─────────────────────────────────
//
// `caid_of` signs through `~%Discovery./identify`, which is what the engine's
// verifier uses. That is faithful to the protocol and it is also why these
// probes cannot see the ledger item measured on 2026-07-29: `./identify`
// returns the CAID of the argument pack `apply_morphism` builds, not of the
// value. Probe and engine go through the same call, so they agree, and a
// second implementation reading REAL_02 §4.2 literally would not. That is
// the delegation arc's problem; it is named here so nobody reads these greens
// as covering it.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::fs;

use nlang_interpreter::value::Identity;
use ring::signature::{Ed25519KeyPair, KeyPair};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

const ADVERT_DOMAIN: &str = "oodp-advert:v1:";
const ID_BYTES: usize = 20;

/// Work order §3.1. The declared home of the durable directory.
const PEERS_DIR: &str = "peers";
const PEERS_FILE: &str = "directory";

// ── harness ─────────────────────────────────────────────────────────────

fn fresh_dir(tag: &str) -> PathBuf {
    let mut d = std::env::temp_dir();
    d.push(format!(
        "nlang-peers-{}-{}-{}",
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

fn oo(dir: &Path, args: &[&str]) -> String {
    let out = oo_cmd(dir).args(args).output().unwrap();
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout).trim(),
        String::from_utf8_lossy(&out.stderr).trim()
    )
}

fn write(dir: &Path, name: &str, src: &str) {
    fs::write(dir.join(name), src).unwrap();
}

fn init(dir: &Path) {
    oo(dir, &["run", "--help"]);
    write(dir, "seed.n", "seed: { ok: #true }\n");
    oo(dir, &["run", "seed.n"]);
}

fn peers_path(dir: &Path) -> PathBuf {
    dir.join(".oo").join(PEERS_DIR).join(PEERS_FILE)
}

fn first_string(out: &str) -> String {
    let s = out
        .split_once('"')
        .unwrap_or_else(|| panic!("no string atom in {out:?}"))
        .1;
    s.split('"').next().unwrap().to_string()
}

fn caid_of(dir: &Path, expr: &str) -> String {
    let out = oo(dir, &["eval", &format!("~%Discovery./identify {expr}")]);
    let caid = first_string(&out);
    assert!(caid.starts_with("hash:sha256:"), "caid_of() got {caid:?}");
    caid
}

fn object_count(dir: &Path) -> usize {
    fn walk(p: &Path, n: &mut usize) {
        if let Ok(rd) = fs::read_dir(p) {
            for e in rd.flatten() {
                let path = e.path();
                if path.is_dir() { walk(&path, n); } else { *n += 1; }
            }
        }
    }
    let mut n = 0;
    walk(&dir.join(".oo").join("objects"), &mut n);
    n
}

fn now_secs() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64
}

fn copy_tree(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).unwrap();
    for e in fs::read_dir(src).unwrap().flatten() {
        let from = e.path();
        let to = dst.join(e.file_name());
        if from.is_dir() {
            copy_tree(&from, &to);
        } else {
            fs::copy(&from, &to).ok();
        }
    }
}

// ── ids, probe side ─────────────────────────────────────────────────────

fn routing_id(public_key: &[u8]) -> [u8; ID_BYTES] {
    let caid = Identity { public_key: public_key.to_vec(), private_key: Vec::new() }
        .node_id_caid();
    let d = caid.digest;
    let mut out = [0u8; ID_BYTES];
    out.copy_from_slice(&d[..ID_BYTES]);
    out
}

fn routing_id_of_caid(caid: &str) -> [u8; ID_BYTES] {
    let bytes = hex::decode(caid.rsplit(':').next().unwrap()).expect("digest is not hex");
    let mut out = [0u8; ID_BYTES];
    out.copy_from_slice(&bytes[..ID_BYTES]);
    out
}

fn xor(a: &[u8; ID_BYTES], b: &[u8; ID_BYTES]) -> [u8; ID_BYTES] {
    let mut o = [0u8; ID_BYTES];
    for i in 0..ID_BYTES { o[i] = a[i] ^ b[i]; }
    o
}

/// Bucket index = leading ZERO bits of the XOR. 160 means equal.
fn bucket_index(self_id: &[u8; ID_BYTES], peer: &[u8; ID_BYTES]) -> usize {
    let x = xor(self_id, peer);
    let mut n = 0;
    for b in x.iter() {
        if *b == 0 { n += 8; } else { n += b.leading_zeros() as usize; break; }
    }
    n.min(ID_BYTES * 8)
}

// ── synthetic peers ─────────────────────────────────────────────────────

struct SynthPeer {
    key_pair: Ed25519KeyPair,
    public_key_hex: String,
    node_id: String,
    id: [u8; ID_BYTES],
    listen_port: u16,
}

fn mint_peer(rng: &ring::rand::SystemRandom, listen_port: u16) -> SynthPeer {
    let pkcs8 = Ed25519KeyPair::generate_pkcs8(rng).unwrap();
    let key_pair = Ed25519KeyPair::from_pkcs8(pkcs8.as_ref()).unwrap();
    let pk = key_pair.public_key().as_ref().to_vec();
    let node_id = Identity { public_key: pk.clone(), private_key: Vec::new() }
        .node_id_caid()
        .to_string();
    SynthPeer {
        public_key_hex: hex::encode(&pk),
        id: routing_id(&pk),
        node_id,
        key_pair,
        listen_port,
    }
}

impl SynthPeer {
    fn body(&self, services: &[&str], ts: i64) -> String {
        let svc = services.iter().map(|s| format!("\"{s}\"")).collect::<Vec<_>>().join(", ");
        format!(
            "{{{{ node_id: \"{}\", public_key: \"{}\", services: [{svc}], \
             listen_port: {}, capacity: 10, ts: {ts}, ttl: 15 }}}}",
            self.node_id, self.public_key_hex, self.listen_port
        )
    }

    fn signed_advert_with(&self, caid_dir: &Path, services: &[&str], ts: i64) -> String {
        let body = self.body(services, ts);
        let caid = caid_of(caid_dir, &body);
        let sig = hex::encode(
            self.key_pair.sign(format!("{ADVERT_DOMAIN}{caid}").as_bytes()).as_ref(),
        );
        let inner = body.trim_start_matches("{{").trim_end_matches("}}").trim();
        format!("{{{{ {inner}, signature: \"{sig}\" }}}}")
    }

    fn advertise_request(&self, caid_dir: &Path) -> String {
        self.advertise_request_with(caid_dir, &[], now_secs())
    }

    fn advertise_request_with(&self, caid_dir: &Path, services: &[&str], ts: i64) -> String {
        format!(
            "{{{{ %op: #advertise, %from: \"{}\", %ad: {} }}}}\n",
            self.node_id,
            self.signed_advert_with(caid_dir, services, ts)
        )
    }
}

// ── running node ────────────────────────────────────────────────────────

struct Node { child: Child, port: u16, log: PathBuf }

impl Drop for Node {
    fn drop(&mut self) { self.child.kill().ok(); self.child.wait().ok(); }
}

impl Node {
    fn log(&self) -> String { fs::read_to_string(&self.log).unwrap_or_default() }
    fn stop(mut self) { self.child.kill().ok(); self.child.wait().ok(); }
}

fn free_port() -> u16 {
    for _ in 0..64 {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let p = l.local_addr().unwrap().port();
        if p > 22000 { return p; }
    }
    panic!("no free port above 22000");
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
    let t0 = std::time::Instant::now();
    for _ in 0..40 {
        std::thread::sleep(Duration::from_millis(100));
        if TcpStream::connect(("127.0.0.1", port)).is_ok() { return node; }
    }
    panic!("`oo node serve` never came up after {:?}: {}", t0.elapsed(), node.log());
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

fn find_node_request(from: &str, target_hex: &str) -> String {
    format!("{{{{ %op: #find_node, %from: \"{from}\", %target: \"{target_hex}\" }}}}\n")
}

/// `%target` is a single CAID string, not a list: `#discover` asks "who serves
/// this CAID". Calibration caught the probe asking with a list of names, which
/// the engine rejects as an unparseable target — four reds were failing on a
/// precondition and testing nothing.
fn discover_request(from: &str, target: &str) -> String {
    format!("{{{{ %op: #discover, %from: \"{from}\", %target: \"{target}\" }}}}\n")
}

/// A service label. Services are CAIDs, so the probe mints one rather than
/// inventing a name the engine would refuse.
fn service_caid(caid_dir: &Path, tag: &str) -> String {
    caid_of(caid_dir, &format!("{{{{ svc: \"{tag}\" }}}}"))
}

/// `%peers` entries as (ad_source, observed_host option).
fn peer_entries(reply: &str) -> Vec<(String, Option<String>)> {
    let Ok(j) = serde_json::from_str::<serde_json::Value>(reply.trim()) else { return vec![] };
    let Some(arr) = j.get("%peers").and_then(|v| v.as_array()) else { return vec![] };
    arr.iter()
        .filter_map(|e| {
            let ad = e.get("%ad").and_then(|v| v.as_str())?.to_string();
            let host = e
                .get("%observed_host")
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .filter(|s| !s.is_empty());
            Some((ad, host))
        })
        .collect()
}

fn ad_node_id(ad: &str) -> String {
    let needle = "node_id: \"";
    let i = ad.find(needle).unwrap_or_else(|| panic!("no node_id in {ad}"));
    ad[i + needle.len()..].split('"').next().unwrap().to_string()
}

fn answer_ids(reply: &str) -> Vec<[u8; ID_BYTES]> {
    peer_entries(reply).iter().map(|(a, _)| routing_id_of_caid(&ad_node_id(a))).collect()
}

/// The serving process's own account of what it wrote durably.
///
/// Byte counts cannot be observed from outside: file size distinguishes
/// neither an append from a rewrite of the same content, nor a rewrite from a
/// compaction. The serving process is the only thing that knows, and it is
/// already this arc's observation surface for routing — so the work order
/// (§3.2) requires it to say so:
///
///   `OODP Peers: append <bytes> bytes (<live> live)`
///   `OODP Peers: compact <bytes> bytes (<live> live)`
///   `OODP Peers: loaded <n> records, skipped <k> damaged`
///
/// This is the same shape as the kademlia arc's fix: the observability the
/// order specifies needs no second process.
/// HARDENED at the discover_sampling arc's acceptance (2026-07-30).
///
/// The wait was 80 × 50 ms and, on expiry, **fell through silently** and
/// computed byte totals from a partial log — so a timeout would have surfaced
/// as a caller's assertion about bytes, naming the wrong cause.
///
/// Context, stated honestly because the obvious story turned out to be wrong.
/// One workspace run during that arc's candidate re-verification reported
/// 17 passed / 2 failed in this binary; its output was not captured and it did
/// not reproduce in ~70 attempts (26 isolated runs of this suite, 16 workspace
/// runs, 18 concurrent-suite runs, 2 forced cold rebuilds). The binary was
/// identified by arithmetic: `cargo test` runs binaries in sequence and stops
/// at the first failure, the cumulative count before this one is 1293, and
/// 1293 + 17 = 1310, the number that run reported.
///
/// **The timeout hypothesis was then measured and rejected**: the 150-line wait
/// takes **761 µs** on this machine and the others tens of microseconds, against
/// a 4 s budget — `advertise_n` already awaits each response, so the lines are
/// there before the poll begins. Node startup was likewise measured at 100 ms
/// against a 4 s budget. Neither guard was close.
///
/// The repair stands anyway, for a reason independent of that: a gate that
/// gives up quietly is wrong whether or not it has ever given up. The budget is
/// now 60 s and expiry is an **assertion**, so if this ever is the cause, the
/// next occurrence says so instead of blaming a byte count. Same for `serve`,
/// whose panic now reports how long it actually waited.
///
/// The cause remains unknown. That is recorded rather than papered over.
fn peers_writes(node: &Node, expect_lines: usize) -> (u64, u64, usize) {
    let count = |s: &str| s.lines().filter(|l| l.contains("OODP Peers: ")).count();
    let mut log = node.log();
    let started = std::time::Instant::now();
    let budget = Duration::from_secs(60);
    while count(&log) < expect_lines {
        assert!(
            started.elapsed() < budget,
            "waited {:?} for {expect_lines} `OODP Peers:` lines and saw {}. \
             This is a harness timeout, not a fact about the engine — every \
             byte total below would be computed from a partial log.",
            started.elapsed(),
            count(&log)
        );
        std::thread::sleep(Duration::from_millis(50));
        log = node.log();
    }
    let mut appended = 0u64;
    let mut compacted = 0u64;
    let mut compactions = 0usize;
    for line in log.lines() {
        let Some(rest) = line.trim().split("OODP Peers: ").nth(1) else { continue };
        let n: u64 = rest
            .split_whitespace()
            .nth(1)
            .and_then(|t| t.parse().ok())
            .unwrap_or(0);
        if rest.starts_with("append") { appended += n; }
        if rest.starts_with("compact") { compacted += n; compactions += 1; }
    }
    (appended, compacted, compactions)
}

fn loaded_report(node: &Node) -> Option<(usize, usize)> {
    for _ in 0..60 {
        for line in node.log().lines() {
            if let Some(rest) = line.trim().split("OODP Peers: loaded ").nth(1) {
                let n: usize = rest.split_whitespace().next()?.parse().ok()?;
                let k: usize = rest
                    .split("skipped ")
                    .nth(1)?
                    .split_whitespace()
                    .next()?
                    .parse()
                    .ok()?;
                return Some((n, k));
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    None
}

fn advertise_n(node: &Node, caid_dir: &Path, n: usize, base_port: u16) -> Vec<SynthPeer> {
    let rng = ring::rand::SystemRandom::new();
    let mut peers = Vec::new();
    for i in 0..n {
        let p = mint_peer(&rng, base_port + i as u16);
        let reply = ask_raw(node.port, &p.advertise_request(caid_dir));
        assert_eq!(status_of(&reply), "success", "advertise {i} was not accepted");
        peers.push(p);
    }
    peers
}


/// Records as the loader sees them: file order, last-wins per `node_id`, then
/// sorted by `received_at` then `node_id` — the order `peers::load` replays.
///
/// ACCEPTANCE REPAIR. The first version of R5 compared the reloaded answer
/// against the closest 20 of everything advertised. That is not what the
/// design says and cannot be satisfied: `insert` drops a peer when its bucket
/// already holds k=20, so with 60 random peers bucket 0 overflows and the
/// table is a strict subset of the directory. The delivery reported this
/// rather than adjusting the probe — the second time it has done so.
///
/// The comparison that does hold reads the file (data) and applies the
/// documented rule (spec), against an engine that reads the same file with
/// its own code. `closest` scans every bucket and sorts by XOR to the target,
/// so `self_id` never moves the *answer* — it moves *who is in the table*,
/// through exactly these overflow drops. Which is why R5 has to keep enough
/// peers to overflow: without a drop it would pass under a table rebuilt with
/// self_id = zeros, and that is the failure it exists for.
fn directory_replay_order(dir: &Path) -> Vec<(String, String)> {
    let text = fs::read_to_string(peers_path(dir)).expect("durable directory missing");
    let mut by_id: std::collections::HashMap<String, (i64, String)> = Default::default();
    for line in text.lines().skip(1) {
        if line.trim().is_empty() { continue; }
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else { continue };
        let Some(nid) = v.get("node_id").and_then(|x| x.as_str()) else { continue };
        let Some(pk) = v.get("public_key").and_then(|x| x.as_str()) else { continue };
        let ra = v.get("received_at").and_then(|x| x.as_i64()).unwrap_or(0);
        by_id.insert(nid.to_string(), (ra, pk.to_string()));
    }
    let mut rows: Vec<(i64, String, String)> = by_id
        .into_iter()
        .map(|(nid, (ra, pk))| (ra, nid, pk))
        .collect();
    rows.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    rows.into_iter().map(|(_, nid, pk)| (nid, pk)).collect()
}

/// Replay k-bucket insertion and return the ids that survive.
fn replay_surviving(self_id: &[u8; ID_BYTES], order: &[(String, String)]) -> Vec<[u8; ID_BYTES]> {
    let mut buckets: Vec<Vec<[u8; ID_BYTES]>> = vec![Vec::new(); ID_BYTES * 8];
    let mut out = Vec::new();
    for (_nid, pk_hex) in order {
        let Ok(pk) = hex::decode(pk_hex) else { continue };
        let rid = routing_id(&pk);
        let b = bucket_index(self_id, &rid);
        if b >= ID_BYTES * 8 { continue; }
        if buckets[b].len() >= 20 { continue; }
        buckets[b].push(rid);
        out.push(rid);
    }
    out
}

// ════════════════════════════════════════════════════════════════════════
//  CONTROL — leads the file
// ════════════════════════════════════════════════════════════════════════

/// C0 — a node that learned nothing writes nothing.
///
/// Every scan below asks "what is in the directory". A loader that silently
/// fails, or a probe harness that points at the wrong path, makes all of them
/// pass by having nothing to disagree with. This one fails in that case,
/// because it is the only test here that *wants* the file absent and would
/// still be green if the arc had never happened — so it must also prove the
/// node was alive and serving while it wrote nothing.
#[test]
fn c0_a_node_that_learned_nothing_writes_nothing() {
    let dir = fresh_dir("c0");
    init(&dir);
    let node = serve(&dir);

    // alive and answering — otherwise "no file" means "no node"
    let reply = ask_raw(node.port, &find_node_request("x", &"a".repeat(40)));
    assert_eq!(status_of(&reply), "success", "the node was not serving: {reply}");
    assert!(answer_ids(&reply).is_empty(), "a fresh node named peers it never met");

    assert!(
        !peers_path(&dir).exists(),
        "a node that accepted no advertisement wrote {}",
        peers_path(&dir).display()
    );
}

// ════════════════════════════════════════════════════════════════════════
//  REDS — remove the #[ignore] to accept
// ════════════════════════════════════════════════════════════════════════

/// R1 — the directory survives a restart of the same node.
#[test]
fn r1_restart_in_place_restores_the_directory() {
    let dir = fresh_dir("r1");
    let caid_dir = fresh_dir("r1-caid");
    init(&dir);
    init(&caid_dir);

    let node = serve(&dir);
    let peers = advertise_n(&node, &caid_dir, 30, 22100);
    node.stop();

    let node2 = serve(&dir);
    let reply = ask_raw(node2.port, &find_node_request("x", &"0".repeat(40)));
    let got = answer_ids(&reply);
    assert!(
        !got.is_empty(),
        "after a restart the node knew nobody; 30 accepted advertisements did \
         not survive: {reply}"
    );
    let known: std::collections::HashSet<_> = peers.iter().map(|p| p.id).collect();
    for id in &got {
        assert!(known.contains(id), "the reloaded table named a peer nobody advertised");
    }
    assert_eq!(got.len(), 20.min(peers.len()), "reloaded answer was short");
}

/// R2 — the file appears where the order declares it, and nowhere else.
#[test]
fn r2_the_file_appears_where_declared_and_nowhere_else() {
    let dir = fresh_dir("r2");
    let caid_dir = fresh_dir("r2-caid");
    init(&dir);
    init(&caid_dir);
    let node = serve(&dir);
    advertise_n(&node, &caid_dir, 5, 22200);
    // the write must have landed before the file is inspected
    peers_writes(&node, 5);

    assert!(peers_path(&dir).exists(), "{} was never written", peers_path(&dir).display());

    let allowed = [
        "objects", "HEAD", "staged", "architects.json",
        "pin_pending", "effect_pending", "abandoned", "format",
        PEERS_DIR,
    ];
    let mut unexpected = Vec::new();
    for e in fs::read_dir(dir.join(".oo")).unwrap().flatten() {
        let n = e.file_name().to_string_lossy().to_string();
        if !allowed.contains(&n.as_str()) { unexpected.push(n); }
    }
    assert!(unexpected.is_empty(), "undeclared durable state appeared: {unexpected:?}");
}

/// R3 — a restart in place restores this node's own observations.
///
/// Not travel: the same node remembering what it itself saw.
#[test]
fn r3_restart_in_place_restores_the_observed_host() {
    let dir = fresh_dir("r3");
    let caid_dir = fresh_dir("r3-caid");
    init(&dir);
    init(&caid_dir);

    let node = serve(&dir);
    let rng = ring::rand::SystemRandom::new();
    let p = mint_peer(&rng, 22300);
    let svc = service_caid(&caid_dir, "r3");
    let req = p.advertise_request_with(&caid_dir, &[&svc], now_secs());
    assert_eq!(status_of(&ask_raw(node.port, &req)), "success");

    let before = peer_entries(&ask_raw(node.port, &discover_request("x", &svc)));
    assert_eq!(before.len(), 1, "the advertised service was not discoverable before restart");
    assert_eq!(
        before[0].1.as_deref(),
        Some("127.0.0.1"),
        "the node did not record where it saw this peer"
    );
    node.stop();

    let node2 = serve(&dir);
    let after = peer_entries(&ask_raw(node2.port, &discover_request("x", &svc)));
    assert_eq!(after.len(), 1, "the record did not survive the restart");
    assert_eq!(
        after[0].1.as_deref(),
        Some("127.0.0.1"),
        "the same node forgot its own observation across a restart"
    );
    assert_eq!(after[0].0, before[0].0, "the verbatim %ad source was not preserved byte for byte");
}

/// R4 — a copy inherits the signed half and not the observed half.
///
/// The copy is a different node because `node_key_path` hashes the absolute
/// workspace path (v0.2.48). No second machine and no second process beyond
/// the one already serving.
#[test]
fn r4_a_copy_gets_the_signed_half_only() {
    let dir = fresh_dir("r4");
    let caid_dir = fresh_dir("r4-caid");
    init(&dir);
    init(&caid_dir);

    let node = serve(&dir);
    let rng = ring::rand::SystemRandom::new();
    let p = mint_peer(&rng, 22400);
    let svc = service_caid(&caid_dir, "r4");
    let req = p.advertise_request_with(&caid_dir, &[&svc], now_secs());
    assert_eq!(status_of(&ask_raw(node.port, &req)), "success");
    peers_writes(&node, 1);
    node.stop();

    let copy = fresh_dir("r4-copy");
    copy_tree(&dir, &copy);

    let node2 = serve(&copy);
    let entries = peer_entries(&ask_raw(node2.port, &discover_request("x", &svc)));
    assert_eq!(
        entries.len(),
        1,
        "the copy lost a signed advertisement — signed facts are true whoever \
         holds them, and relaying one is repeating what you were told"
    );
    // the verbatim signed source, not a re-serialisation: relay emits these
    // bytes and a signature does not survive being re-printed
    assert_eq!(
        ad_node_id(&entries[0].0),
        p.node_id,
        "the copy relayed a record that is not the one advertised"
    );
    assert!(
        entries[0].0.contains("signature: \""),
        "the copy relayed a body with the signature stripped — then it is not \
         a signed fact any more, it is the copy's word for it"
    );
}

/// R4b — the copy must not claim an observation it never made.
#[test]
fn r4b_a_copy_does_not_inherit_an_observation() {
    let dir = fresh_dir("r4b");
    let caid_dir = fresh_dir("r4b-caid");
    init(&dir);
    init(&caid_dir);

    let node = serve(&dir);
    let rng = ring::rand::SystemRandom::new();
    let p = mint_peer(&rng, 22450);
    let svc = service_caid(&caid_dir, "r4b");
    let req = p.advertise_request_with(&caid_dir, &[&svc], now_secs());
    assert_eq!(status_of(&ask_raw(node.port, &req)), "success");
    peers_writes(&node, 1);
    node.stop();

    let copy = fresh_dir("r4b-copy");
    copy_tree(&dir, &copy);
    let node2 = serve(&copy);
    let entries = peer_entries(&ask_raw(node2.port, &discover_request("x", &svc)));

    assert_eq!(entries.len(), 1, "the copy lost the signed record");
    assert_eq!(
        entries[0].1, None,
        "the copy asserted %observed_host {:?} for a connection it never \
         accepted. An observation cannot travel (REAL_02 §4.2.5); the signed \
         body can",
        entries[0].1
    );
}

/// R5 — the rebuilt index is the index an insertion replay would build.
///
/// Compared over the whole table, not sampled: a reload that kept only the
/// first bucket, or that lost incumbent order, differs here and nowhere else.
#[test]
fn r5_the_rebuilt_index_matches_an_insertion_replay() {
    let dir = fresh_dir("r5");
    let caid_dir = fresh_dir("r5-caid");
    init(&dir);
    init(&caid_dir);

    let node = serve(&dir);
    let peers = advertise_n(&node, &caid_dir, 60, 22500);
    node.stop();

    let node2 = serve(&dir);
    let self_id = {
        let out = oo(&dir, &["node", "id"]);
        let caid = out.split_whitespace().find(|t| t.starts_with("hash:sha256:"))
            .unwrap_or_else(|| panic!("`oo node id` gave no CAID: {out}"));
        routing_id_of_caid(caid)
    };

    // Replay the documented rebuild over the file's own contents, then take
    // the closest 20 of the SURVIVORS — not of everything advertised.
    let order = directory_replay_order(&dir);
    assert_eq!(order.len(), peers.len(), "the file lost records");
    let surviving = replay_surviving(&self_id, &order);
    assert!(
        surviving.len() < order.len(),
        "no bucket overflowed with {} peers, so this probe cannot tell a table \
         rebuilt with the right self id from one rebuilt with zeros",
        order.len()
    );
    // COUNTERFACTUAL — R5 exists to catch an index rebuilt under the wrong
    // self id, which places every peer in a different bucket and therefore
    // drops a different set once buckets overflow.
    //
    // ACCEPTANCE REPAIR, THIRD PASS, and the first two are worth keeping:
    //
    //   1. compared against the closest 20 of everything advertised — but a
    //      full bucket drops what it cannot hold, so the table is a strict
    //      subset of the directory. (Reported by the delivery, not trimmed.)
    //   2. used `self_id = zeros` as the counterfactual and asserted the
    //      answers differ. That flakes about half the time, and the reason is
    //      worth writing down: bucket 0 under self `X` holds the peers whose
    //      top bit differs from X's, and under self `0` it holds the peers
    //      whose top bit is 1. **When X's top bit is 0 those are the same
    //      set** — so an engine shipping an unseeded zero self id would be
    //      indistinguishable from a correct one on half of all nodes. That is
    //      how a bug of this class survives, and it is why the probe must not
    //      depend on the draw.
    //
    // So the counterfactual is the real self id with its top bit flipped: a
    // wrong self id, minimally wrong, and one whose bucket 0 is the exact
    // complement of the right one. Both halves overflow k=20 here, so the
    // survivor sets cannot coincide.
    let mut wrong_self = self_id;
    wrong_self[0] ^= 0x80;
    let wrong_surviving = replay_surviving(&wrong_self, &order);

    let a: std::collections::HashSet<[u8; ID_BYTES]> = surviving.iter().cloned().collect();
    let b: std::collections::HashSet<[u8; ID_BYTES]> = wrong_surviving.iter().cloned().collect();
    let mut diff: Vec<[u8; ID_BYTES]> = a.symmetric_difference(&b).cloned().collect();
    diff.sort();
    // A peer kept by one rebuild and dropped by the other is at distance zero
    // from itself, so it heads one answer and is absent from the other.
    let target = *diff.first().unwrap_or_else(|| panic!(
        "the right rebuild and a wrong-self-id rebuild kept exactly the same \
         peers, so no target can separate them — {} records, {} survivors",
        order.len(),
        surviving.len()
    ));

    let mut expect: Vec<[u8; ID_BYTES]> = surviving.clone();
    expect.sort_by_key(|id| xor(id, &target));
    expect.truncate(20);

    let mut expect_wrong: Vec<[u8; ID_BYTES]> = wrong_surviving;
    expect_wrong.sort_by_key(|id| xor(id, &target));
    expect_wrong.truncate(20);
    assert_ne!(
        expect, expect_wrong,
        "a target taken from the symmetric difference still did not separate \
         the two rebuilds"
    );

    let reply = ask_raw(node2.port, &find_node_request("x", &hex::encode(target)));
    let mut got = answer_ids(&reply);
    got.sort_by_key(|id| xor(id, &target));

    assert_eq!(
        got, expect,
        "after a reload `closest(target, k)` was not the closest 20 of the \
         records that survive an insertion replay. self={} — a table rebuilt \
         with the wrong self id renumbers every bucket and drops a different \
         set",
        hex::encode(self_id)
    );
}

/// R6 — the durable write is linear, and it happens at all.
///
/// The second clause is what makes this red today: at v0.2.53 the engine
/// writes zero bytes, and a bound alone would pass trivially.
#[test]
fn r6_writes_are_linear_not_quadratic() {
    let dir = fresh_dir("r6");
    let caid_dir = fresh_dir("r6-caid");
    init(&dir);
    init(&caid_dir);

    let node = serve(&dir);
    advertise_n(&node, &caid_dir, 150, 22600);
    let (appended, compacted, compactions) = peers_writes(&node, 150);
    let total = appended + compacted;

    println!(
        "R6: 150 adverts -> appended={appended} B, compacted={compacted} B in \
         {compactions} compactions, total={total} B ({} B/record)",
        appended / 150
    );
    assert!(
        appended > 0,
        "the node reported no durable write at all for 150 accepted \
         advertisements — a bound on zero is not a bound"
    );
    assert!(
        total < 1_000_000,
        "150 advertisements cost {total} bytes ({appended} appended, \
         {compacted} in {compactions} compactions). The design this arc \
         replaced cost 14,380,000 bytes for the same 150 by rewriting the \
         whole file on every accept"
    );
}

/// R7 — a second advertisement from the same node supersedes the first.
#[test]
fn r7_a_superseded_record_is_replaced_after_reload() {
    let dir = fresh_dir("r7");
    let caid_dir = fresh_dir("r7-caid");
    init(&dir);
    init(&caid_dir);

    let node = serve(&dir);
    let rng = ring::rand::SystemRandom::new();
    let p = mint_peer(&rng, 22700);
    let t0 = now_secs();
    let old_svc = service_caid(&caid_dir, "r7-old");
    let new_svc = service_caid(&caid_dir, "r7-new");
    assert_eq!(
        status_of(&ask_raw(node.port, &p.advertise_request_with(&caid_dir, &[&old_svc], t0))),
        "success"
    );
    assert_eq!(
        status_of(&ask_raw(node.port, &p.advertise_request_with(&caid_dir, &[&new_svc], t0 + 1))),
        "success"
    );
    peers_writes(&node, 2);
    node.stop();

    let node2 = serve(&dir);
    let old = peer_entries(&ask_raw(node2.port, &discover_request("x", &old_svc)));
    let new = peer_entries(&ask_raw(node2.port, &discover_request("x", &new_svc)));
    assert!(old.is_empty(), "the superseded advertisement came back after a reload");
    assert_eq!(new.len(), 1, "the newer advertisement did not survive");
}

/// R8 — compaction runs and shrinks the file without changing the live set.
#[test]
fn r8_compaction_triggers_and_shrinks_the_file() {
    let dir = fresh_dir("r8");
    let caid_dir = fresh_dir("r8-caid");
    init(&dir);
    init(&caid_dir);

    let node = serve(&dir);
    let rng = ring::rand::SystemRandom::new();
    let peers: Vec<SynthPeer> = (0..10).map(|i| mint_peer(&rng, 22800 + i)).collect();
    let t0 = now_secs();
    let svc = service_caid(&caid_dir, "r8");
    // 10 live peers, each re-advertising 5 times: 50 lines, 10 live -> >2x
    for round in 0..5 {
        for p in &peers {
            let req = p.advertise_request_with(&caid_dir, &[&svc], t0 + round);
            assert_eq!(status_of(&ask_raw(node.port, &req)), "success");
        }
    }
    let (_, compacted, compactions) = peers_writes(&node, 50);
    assert!(
        compactions > 0,
        "50 lines over 10 live records never crossed the 2x threshold \
         ({compacted} bytes compacted)"
    );

    let size_after = fs::metadata(peers_path(&dir)).map(|m| m.len()).unwrap_or(0);
    println!(
        "R8: 50 lines / 10 live -> {compactions} compactions, {compacted} B \
         rewritten, file now {size_after} B"
    );
    node.stop();

    let node2 = serve(&dir);
    // ACCEPTANCE REPAIR: the live count is read through `#find_node` (k=20),
    // not `#discover` — MAX_DISCOVER_PEERS is 8, so asking discover for ten
    // records asserted something the protocol forbids. The delivery reported
    // that rather than trimming the assertion to fit.
    let ids = answer_ids(&ask_raw(node2.port, &find_node_request("x", &"0".repeat(40))));
    assert_eq!(
        ids.len(),
        10,
        "compaction changed the live set: {} records survived, not 10 \
         (file is {size_after} bytes)",
        ids.len()
    );
    // and the service is still discoverable at all, capped as the protocol says
    let entries = peer_entries(&ask_raw(node2.port, &discover_request("x", &svc)));
    assert_eq!(entries.len(), 8, "discover cap moved: {}", entries.len());
}

/// R9 — one damaged line costs one record, and the loss is reported.
#[test]
fn r9_one_damaged_line_does_not_cost_the_directory() {
    let dir = fresh_dir("r9");
    let caid_dir = fresh_dir("r9-caid");
    init(&dir);
    init(&caid_dir);

    let node = serve(&dir);
    advertise_n(&node, &caid_dir, 12, 22900);
    peers_writes(&node, 12);
    node.stop();

    let path = peers_path(&dir);
    let text = fs::read_to_string(&path).expect("the durable directory was never written");
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    assert!(lines.len() >= 6, "expected a line per record, got {}", lines.len());
    let victim = lines.len() / 2;
    lines[victim] = "{ this is not a record".to_string();
    fs::write(&path, lines.join("\n") + "\n").unwrap();

    let node2 = serve(&dir);
    let reply = ask_raw(node2.port, &find_node_request("x", &"0".repeat(40)));
    let got = answer_ids(&reply);
    assert!(
        !got.is_empty(),
        "one damaged line emptied the whole directory: {reply}"
    );

    let (loaded, skipped) = loaded_report(&node2)
        .expect("the node did not report what it loaded — a silent skip is how \
                 a directory quietly becomes empty");
    assert_eq!(skipped, 1, "the node loaded {loaded} and admitted to skipping {skipped}");
}

// ════════════════════════════════════════════════════════════════════════
//  PINS — must be green before and after
// ════════════════════════════════════════════════════════════════════════

/// P1 — the ladder is unchanged and the file is downstream of it.
#[test]
fn p1_unsigned_adverts_never_reach_the_directory() {
    let dir = fresh_dir("p1");
    let caid_dir = fresh_dir("p1-caid");
    init(&dir);
    init(&caid_dir);
    let node = serve(&dir);

    let rng = ring::rand::SystemRandom::new();
    let p = mint_peer(&rng, 23000);

    // a body that computes, with a signature that is merely wrong
    let svc = service_caid(&caid_dir, "p1");
    let good = p.signed_advert_with(&caid_dir, &[&svc], now_secs());
    let bad = good.replace(
        &good.split("signature: \"").nth(1).unwrap()[..8],
        "deadbeef",
    );
    let req = format!(
        "{{{{ %op: #advertise, %from: \"{}\", %ad: {bad} }}}}\n",
        p.node_id
    );
    let status = status_of(&ask_raw(node.port, &req));
    assert_ne!(status, "success", "a tampered signature was accepted");

    let entries = peer_entries(&ask_raw(node.port, &discover_request("x", &svc)));
    assert!(entries.is_empty(), "a rejected advertisement reached the directory");
    assert!(
        !peers_path(&dir).exists(),
        "a rejected advertisement was written durably"
    );
}

/// P2 — GC does not collect the peer directory.
#[test]
fn p2_gc_does_not_touch_the_peer_directory() {
    let dir = fresh_dir("p2");
    init(&dir);
    write(&dir, "u.n", "x: { n: 1 }\n");
    oo(&dir, &["evolve", "u.n"]);
    oo(&dir, &["commit", "-m", "m"]);

    // whatever the peers path holds (or does not) must be unchanged by GC
    let before = fs::read(peers_path(&dir)).ok();
    let out = oo(&dir, &["gc", "--grant", "gc"]);
    let after = fs::read(peers_path(&dir)).ok();
    assert_eq!(
        before, after,
        "`oo gc` changed the peer directory. GC sweeps `.oo/objects/`; a \
         network cache is not an object and is not reachable from any \
         commit — collecting it would be GC deciding about state no root \
         names. Output was: {out}"
    );
}

/// P3 — advertising writes no CAS objects.
#[test]
fn p3_advertising_writes_no_objects() {
    let dir = fresh_dir("p3");
    let caid_dir = fresh_dir("p3-caid");
    init(&dir);
    init(&caid_dir);
    let node = serve(&dir);
    let before = object_count(&dir);
    advertise_n(&node, &caid_dir, 20, 23100);
    ask_raw(node.port, &find_node_request("x", &"b".repeat(40)));
    assert_eq!(
        object_count(&dir),
        before,
        "the node wrote CAS objects while learning peers — the directory is \
         durable state, not content"
    );
}

/// P4 — `#fetch` is untouched and still independent of `%from`.
#[test]
fn p4_fetch_is_untouched() {
    let dir = fresh_dir("p4");
    init(&dir);
    write(&dir, "i.n", "id: ~%Discovery./identify_and_store { p4: 1 }\n");
    let caid = first_string(&oo(&dir, &["run", "i.n", "--observe", "id"]));
    let node = serve(&dir);
    for from in ["", "hash:sha256:v1:whoever", "not-a-caid"] {
        let r = ask_raw(
            node.port,
            &format!("{{{{ %op: #fetch, %from: \"{from}\", %hash: \"{caid}\" }}}}\n"),
        );
        assert_eq!(status_of(&r), "success", "%from={from:?}: {r}");
    }
}

/// P5 — within one process, persistence changes no answer.
#[test]
fn p5_find_node_answers_are_unchanged_within_one_process() {
    let dir = fresh_dir("p5");
    let caid_dir = fresh_dir("p5-caid");
    init(&dir);
    init(&caid_dir);
    let node = serve(&dir);
    let peers = advertise_n(&node, &caid_dir, 25, 23200);

    let target = peers[3].id;
    let mut expect: Vec<[u8; ID_BYTES]> = peers.iter().map(|p| p.id).collect();
    expect.sort_by_key(|id| xor(id, &target));
    expect.truncate(20);

    let mut got = answer_ids(&ask_raw(node.port, &find_node_request("x", &hex::encode(target))));
    got.sort_by_key(|id| xor(id, &target));
    assert_eq!(got, expect, "a live node's `closest(target, k)` moved");
}

/// P6 — an unrecognised entry under `.oo/` does not break this engine.
///
/// This is the invariant that makes cross-version work in both directions,
/// and it is why `.oo/format` is not bumped by this arc (order §6.2): an
/// engine that can read a store must not refuse it. A false refusal is what
/// REAL_03 §6.6's `裁決必須為真` clause forbids.
#[test]
fn p6_an_unknown_entry_under_oo_is_tolerated() {
    let dir = fresh_dir("p6");
    init(&dir);
    fs::create_dir_all(dir.join(".oo").join("something-from-the-future")).unwrap();
    fs::write(dir.join(".oo").join("a-file-from-the-future"), b"\x00\x01not utf8").unwrap();

    write(&dir, "u.n", "x: { n: 2 }\n");
    let out = oo(&dir, &["evolve", "u.n"]);
    assert!(!out.contains("Error"), "an unknown `.oo/` entry broke evolve: {out}");
    let c = oo(&dir, &["commit", "-m", "m"]);
    assert!(c.contains("hash:sha256:"), "an unknown `.oo/` entry broke commit: {c}");
    let l = oo(&dir, &["log"]);
    assert!(l.contains("commit "), "an unknown `.oo/` entry broke log: {l}");
}

/// P7 — the store format marker is an invariant of this arc, not a target.
#[test]
fn p7_the_store_format_marker_is_not_bumped() {
    let dir = fresh_dir("p7");
    init(&dir);
    let baseline = fs::read_to_string(dir.join(".oo").join("format"))
        .expect("`.oo/format` is missing — the local_gc arc declared it");

    let caid_dir = fresh_dir("p7-caid");
    init(&caid_dir);
    let node = serve(&dir);
    advertise_n(&node, &caid_dir, 3, 23300);

    let after = fs::read_to_string(dir.join(".oo").join("format")).unwrap();
    assert_eq!(
        baseline.trim(),
        after.trim(),
        "the store format marker moved. Adding a file an older engine ignores \
         is not a layout an older engine cannot read; bumping would make \
         v0.2.53 refuse a store it can open, and a verdict must be true"
    );
}


/// P8 — a stored record is served only if its signature still verifies.
///
/// ACCEPTANCE REPAIR pin. R1's ruling is that the signed face travels because
/// it is true whoever holds it — which is a property of a signature somebody
/// checks. The loader could not check: it runs while the engine is being
/// built, and computing a body CAID needs the engine. So the check moved to
/// just after construction, and this pin is what keeps it there.
///
/// The control matters as much as the target: the same record, untampered,
/// must still be served. A "repair" that dropped everything would pass the
/// tampered half and be worthless.
#[test]
fn p8_a_tampered_stored_signature_is_not_served() {
    let dir = fresh_dir("p8");
    let caid_dir = fresh_dir("p8-caid");
    init(&dir);
    init(&caid_dir);

    let node = serve(&dir);
    let rng = ring::rand::SystemRandom::new();
    let p = mint_peer(&rng, 23400);
    let svc = service_caid(&caid_dir, "p8");
    let req = p.advertise_request_with(&caid_dir, &[&svc], now_secs());
    assert_eq!(status_of(&ask_raw(node.port, &req)), "success");
    peers_writes(&node, 1);
    node.stop();

    // CONTROL: untouched, the record survives a restart.
    let good = serve(&dir);
    assert_eq!(
        peer_entries(&ask_raw(good.port, &discover_request("x", &svc))).len(),
        1,
        "the honest record did not survive — a check that drops everything is \
         not a check"
    );
    good.stop();

    // Now corrupt the signature inside the stored record and restart.
    let path = peers_path(&dir);
    let text = fs::read_to_string(&path).unwrap();
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let needle = "signature: \\\"";
    let i = lines[1].find(needle).expect("no signature in the stored record");
    let at = i + needle.len();
    lines[1].replace_range(at..at + 16, &"0".repeat(16));
    fs::write(&path, lines.join("\n") + "\n").unwrap();

    let node2 = serve(&dir);
    let entries = peer_entries(&ask_raw(node2.port, &discover_request("x", &svc)));
    assert!(
        entries.is_empty(),
        "a record whose stored signature was forged was served anyway. `.oo/` \
         is writable by any n/ program, so an unchecked directory is a free \
         and permanent seat in this table — and SPEC_15 §7.1 prices that seat \
         in minted identities"
    );

    let mut saw = false;
    for _ in 0..60 {
        if node2.log().contains("unverifiable") { saw = true; break; }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(saw, "the node dropped a record and did not say so: {}", node2.log());
}
