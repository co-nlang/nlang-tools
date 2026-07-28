// The table that can be held accountable (2026-07-28, pre-committed by work
// order: docs/kademlia_table_handover.md).
//
// ── Why this arc was nearly refused ──────────────────────────────────────
//
// k-buckets are unfalsifiable on two or three local nodes. A probe cannot
// tell a real bucket structure from a flat list, because on a small network
// they behave identically. Shipping structure that no measurement can hold
// accountable is precisely what the acceptance protocol exists to refuse, and
// this arc was very close to being deferred on those grounds.
//
// What changed the answer is a measurement:
//
//     20,000 synthetic identities, 5.59 s, 0.280 ms/key
//     deepest populated bucket ...... 15
//     buckets overflowing k=20 ...... 10
//     distribution .................. [9944, 5044, 2601, 1136, 647, 303, …]
//
// `nlang-interpreter` is a normal dependency of `oo`, so a probe can mint
// identities in-process and compute their ids with **the engine's own**
// `Identity::node_id_caid()` — not a reimplementation of it. Ten buckets
// overflow in six seconds, and the correct answer to `closest(target, k)` can
// be brute-forced over the whole inserted set and compared.
//
// So the structure is falsifiable here after all, and R5 is the probe this
// arc exists for:
//
//     a flat list with correct sorting passes R1 vacuously and fails R2;
//     a real table that searches only the target's own bucket passes R2
//     and fails R5.
//
// Neither can be satisfied by a structure that is not the specified one.
//
// ── The limit, stated rather than papered over ───────────────────────────
// Random sampling cannot reach buckets ≥16 (probability 2⁻¹⁶), so table
// *population* at depth is untestable here. The *query* path is not: `%target`
// is an arbitrary 160-bit value, so R5 builds deep targets by flipping low
// bits of a known id. Coverage claims must keep those two apart.
//
// ── What the cost of an identity buys ────────────────────────────────────
// node_id = sha256(public key), and a key costs 0.280 ms. Taking one slot in
// a given victim's bucket i costs 2^(i+1) keys in expectation: bucket 10 is
// about eleven seconds for all twenty slots; bucket 20 is 3.3 hours; bucket
// 30 is 140 days. That is Kademlia's standing assumption — ids are hard to
// choose — meeting a machine that makes 3,500 of them a second.
//
// It is not a defect introduced here, and it is not fixable here either:
// ORDER_00 §1.1 already argues no internal mechanism supplies Sybil
// resistance. What incumbent-first (R3) buys is narrower and worth being
// exact about: grinding buys nothing against a bucket that is already full,
// so the attacker must be **early**, not merely rich. That is a race
// condition, not a defence, and R3 pins the policy rather than the safety.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::fs;

use nlang_interpreter::value::Identity;
use ring::signature::{Ed25519KeyPair, KeyPair};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

const ADVERT_DOMAIN: &str = "oodp-advert:v1:";
/// Work order §3.1.
const K: usize = 20;
/// Work order §3.3.
const MAX_FIND_NODE_PEERS: usize = 20;
const MAX_REPLY_BYTES: usize = 64 * 1024;
/// A node id is the first 20 bytes of the CAID digest (§3.1 / M2).
const ID_BYTES: usize = 20;

// ── harness ─────────────────────────────────────────────────────────────

fn fresh_dir(tag: &str) -> PathBuf {
    let mut d = std::env::temp_dir();
    d.push(format!(
        "nlang-kad-{}-{}-{}",
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

// ── ids and distance, probe side ────────────────────────────────────────

/// The 160-bit routing id of a node: first 20 bytes of its CAID digest.
///
/// Computed with the engine's own `node_id_caid`, deliberately — a probe that
/// reimplemented the CAID encoding would be testing its own copy of the rule.
fn routing_id(public_key: &[u8]) -> [u8; ID_BYTES] {
    let caid = Identity {
        public_key: public_key.to_vec(),
        private_key: Vec::new(),
    }
    .node_id_caid();
    let d = caid.digest;
    assert!(
        d.len() >= ID_BYTES,
        "digest is {} bytes, cannot take a 160-bit prefix",
        d.len()
    );
    let mut out = [0u8; ID_BYTES];
    out.copy_from_slice(&d[..ID_BYTES]);
    out
}

/// Routing id parsed out of a `node_id` CAID string (last colon-separated field).
fn routing_id_of_caid(caid: &str) -> [u8; ID_BYTES] {
    let hex_digest = caid.rsplit(':').next().unwrap();
    let bytes = hex::decode(hex_digest).expect("digest is not hex");
    let mut out = [0u8; ID_BYTES];
    out.copy_from_slice(&bytes[..ID_BYTES]);
    out
}

fn xor(a: &[u8; ID_BYTES], b: &[u8; ID_BYTES]) -> [u8; ID_BYTES] {
    let mut o = [0u8; ID_BYTES];
    for i in 0..ID_BYTES { o[i] = a[i] ^ b[i]; }
    o
}

/// Bucket index = number of leading ZERO bits of the XOR (§3.1).
/// 160 means the two ids are equal.
fn bucket_index(self_id: &[u8; ID_BYTES], peer: &[u8; ID_BYTES]) -> usize {
    let x = xor(self_id, peer);
    let mut n = 0;
    for b in x.iter() {
        if *b == 0 { n += 8; } else { n += b.leading_zeros() as usize; break; }
    }
    n.min(ID_BYTES * 8)
}

/// XOR distance as a comparable big-endian value.
fn distance(a: &[u8; ID_BYTES], b: &[u8; ID_BYTES]) -> [u8; ID_BYTES] {
    xor(a, b)
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
    fn body(&self, services: &[String], ts: i64, ttl: i64) -> String {
        let svc = services.iter().map(|s| format!("\"{s}\"")).collect::<Vec<_>>().join(", ");
        format!(
            "{{{{ node_id: \"{}\", public_key: \"{}\", services: [{svc}], \
             listen_port: {}, capacity: 10, ts: {ts}, ttl: 15 }}}}",
            self.node_id, self.public_key_hex, self.listen_port
        )
        .replace("ttl: 15", &format!("ttl: {ttl}"))
    }

    fn signed_advert(&self, caid_dir: &Path) -> String {
        let body = self.body(&[], now_secs(), 15);
        let caid = caid_of(caid_dir, &body);
        let sig = hex::encode(self.key_pair.sign(format!("{ADVERT_DOMAIN}{caid}").as_bytes()).as_ref());
        let inner = body.trim_start_matches("{{").trim_end_matches("}}").trim();
        format!("{{{{ {inner}, signature: \"{sig}\" }}}}")
    }

    fn advertise_request(&self, caid_dir: &Path) -> String {
        format!(
            "{{{{ %op: #advertise, %from: \"{}\", %ad: {} }}}}\n",
            self.node_id,
            self.signed_advert(caid_dir)
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
}

/// Ports above 21000 — earlier arcs leave stray listeners in the 19000s.
fn free_port() -> u16 {
    for _ in 0..64 {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let p = l.local_addr().unwrap().port();
        if p > 21000 { return p; }
    }
    panic!("no free port above 21000");
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
    let mut node = Node { child, port, log };
    for _ in 0..40 {
        std::thread::sleep(Duration::from_millis(100));
        if node.child.try_wait().unwrap().is_some() {
            panic!("`oo node serve` exited: {}", node.log());
        }
        if TcpStream::connect(("127.0.0.1", port)).is_ok() { return node; }
    }
    panic!("`oo node serve` never came up: {}", node.log());
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

/// `%ad` sources out of a `%peers` array.
fn peer_ads(reply: &str) -> Vec<String> {
    let Ok(j) = serde_json::from_str::<serde_json::Value>(reply.trim()) else { return vec![] };
    let Some(arr) = j.get("%peers").and_then(|v| v.as_array()) else { return vec![] };
    arr.iter()
        .filter_map(|e| e.get("%ad").and_then(|v| v.as_str()).map(str::to_string))
        .collect()
}

/// `node_id` field out of an advertisement source.
fn ad_node_id(ad: &str) -> String {
    let needle = "node_id: \"";
    let i = ad.find(needle).unwrap_or_else(|| panic!("no node_id in {ad}"));
    ad[i + needle.len()..].split('"').next().unwrap().to_string()
}

/// The routing ids named by a `#find_node` answer, in the order returned.
fn answer_ids(reply: &str) -> Vec<[u8; ID_BYTES]> {
    peer_ads(reply).iter().map(|a| routing_id_of_caid(&ad_node_id(a))).collect()
}

/// `oo node routing` → (bucket occupancies, total, dropped_full).
fn routing_dump(dir: &Path) -> (std::collections::BTreeMap<usize, usize>, usize, usize) {
    let out = oo(dir, &["node", "routing"]);
    let mut buckets = std::collections::BTreeMap::new();
    let (mut total, mut dropped) = (usize::MAX, usize::MAX);
    for line in out.lines() {
        let l = line.trim();
        if let Some(rest) = l.strip_prefix("bucket ") {
            if let Some((i, n)) = rest.split_once(':') {
                if let (Ok(i), Ok(n)) = (i.trim().parse(), n.trim().parse()) {
                    buckets.insert(i, n);
                }
            }
        } else if let Some(v) = l.strip_prefix("total:") {
            total = v.trim().parse().unwrap_or(usize::MAX);
        } else if let Some(v) = l.strip_prefix("dropped_full:") {
            dropped = v.trim().parse().unwrap_or(usize::MAX);
        }
    }
    assert!(
        total != usize::MAX && dropped != usize::MAX,
        "`oo node routing` did not print `total:` and `dropped_full:` — probes \
         cannot assert about a structure they cannot see: {out:?}"
    );
    (buckets, total, dropped)
}

/// This node's own routing id, from `oo node id`.
fn self_id(dir: &Path) -> [u8; ID_BYTES] {
    let out = oo(dir, &["node", "id"]);
    let caid = out.lines().find(|l| l.starts_with("hash:"))
        .unwrap_or_else(|| panic!("`oo node id` printed no CAID: {out:?}"));
    routing_id_of_caid(caid.trim())
}

// ── the specified policy, simulated probe-side ──────────────────────────

/// What the table must contain after `offered` are presented in order, under
/// §3.1 (160 buckets, k per bucket) and R-d (incumbent-first).
///
/// This is the *specification* restated, not a copy of the implementation —
/// R2 and R3 exist to check that the implementation agrees with it, and R5
/// uses it only so that "the correct answer" is well defined.
fn simulate_table<'a>(
    self_id: &[u8; ID_BYTES],
    offered: &'a [SynthPeer],
) -> Vec<&'a SynthPeer> {
    let mut buckets: std::collections::BTreeMap<usize, Vec<&SynthPeer>> = Default::default();
    for p in offered {
        let b = bucket_index(self_id, &p.id);
        if b >= ID_BYTES * 8 { continue; } // self — never stored (§3.5)
        let slot = buckets.entry(b).or_default();
        if slot.iter().any(|q| q.node_id == p.node_id) { continue; } // refresh
        if slot.len() < K { slot.push(p); }
    }
    buckets.into_values().flatten().collect()
}

/// The k ids closest to `target`, by brute force over `held`.
fn brute_force_closest(
    target: &[u8; ID_BYTES],
    held: &[&SynthPeer],
    k: usize,
) -> Vec<[u8; ID_BYTES]> {
    let mut v: Vec<[u8; ID_BYTES]> = held.iter().map(|p| p.id).collect();
    // Ascending by distance; ties broken by the id itself so the order is total.
    v.sort_by(|a, b| distance(a, target).cmp(&distance(b, target)).then(a.cmp(b)));
    v.truncate(k);
    v
}

// ── fixtures ────────────────────────────────────────────────────────────

struct Fixture {
    dir: PathBuf,
    caid_dir: PathBuf,
    node: Node,
    self_id: [u8; ID_BYTES],
    peers: Vec<SynthPeer>,
}

/// Buckets an offered set must populate before it can discriminate a table
/// from a list. With 220 random ids bucket 5 is empty about 3% of the time,
/// and a gate that fails three runs in a hundred for a reason unrelated to
/// the code is worse than no gate — so the set is drawn until it qualifies
/// rather than asserted about after the fact.
const MIN_BUCKETS_COVERED: usize = 6;

/// Serves a node and offers it `n` freshly minted peers, in order.
///
/// Peers are minted **before** any are sent, so the draw can be extended
/// until it covers enough buckets without the server seeing a different
/// number of advertisements than the probe accounts for.
fn fixture(tag: &str, n: usize) -> Fixture {
    let dir = fresh_dir(&format!("{tag}-srv"));
    let caid_dir = fresh_dir(&format!("{tag}-caid"));
    init(&dir);
    init(&caid_dir);
    let node = serve(&dir);
    let sid = self_id(&dir);
    let rng = ring::rand::SystemRandom::new();

    let mut peers: Vec<SynthPeer> = Vec::with_capacity(n);
    let covered = |ps: &[SynthPeer]| -> usize {
        ps.iter()
            .map(|p| bucket_index(&sid, &p.id))
            .collect::<std::collections::BTreeSet<_>>()
            .len()
    };
    let mut minted = 0usize;
    while peers.len() < n || covered(&peers) < MIN_BUCKETS_COVERED {
        peers.push(mint_peer(&rng, 21000 + (minted as u16 % 4000)));
        minted += 1;
        assert!(
            minted < n * 10,
            "HARNESS: {minted} ids still cover only {} buckets — the id space \
             is not behaving like sha256 output",
            covered(&peers)
        );
    }

    for (i, p) in peers.iter().enumerate() {
        let r = ask_raw(node.port, &p.advertise_request(&caid_dir));
        assert_eq!(
            status_of(&r),
            "success",
            "LIVENESS: synthetic advertisement {i} was not accepted, so nothing \
             below measures a routing table: {r}"
        );
    }
    Fixture { dir, caid_dir, node, self_id: sid, peers }
}

// ════════════════════════════════════════════════════════════════════════
//  RED — must fail on v0.2.51, for the reason stated
// ════════════════════════════════════════════════════════════════════════

/// R1 — every peer lands in the bucket its XOR says it should.
///
/// Baseline: `oo node routing` does not exist.
#[test]
#[ignore]
fn r1_bucket_index_is_the_leading_zero_count() {
    let f = fixture("r1", 220);
    let (buckets, total, _) = routing_dump(&f.dir);

    let mut expected: std::collections::BTreeMap<usize, usize> = Default::default();
    for p in &f.peers {
        let b = bucket_index(&f.self_id, &p.id);
        let e = expected.entry(b).or_default();
        if *e < K { *e += 1; }
    }
    assert!(
        expected.len() >= MIN_BUCKETS_COVERED,
        "HARNESS: only {} buckets populated — the fixture guarantees at least \
         {MIN_BUCKETS_COVERED}",
        expected.len()
    );
    assert_eq!(buckets, expected, "bucket occupancy disagrees with XOR");
    assert_eq!(total, expected.values().sum::<usize>(), "total disagrees");
}

/// R2 — a bucket holds exactly k, however many are offered.
///
/// About half of all random ids land in bucket 0, so 220 offers put well over
/// a hundred candidates there. A flat list keeps them all.
///
/// Baseline: `oo node routing` does not exist.
#[test]
#[ignore]
fn r2_a_bucket_holds_exactly_k() {
    let f = fixture("r2", 220);
    let offered_b0 = f.peers.iter().filter(|p| bucket_index(&f.self_id, &p.id) == 0).count();
    assert!(
        offered_b0 > K + 30,
        "HARNESS: only {offered_b0} candidates for bucket 0, need well over {K}"
    );
    let (buckets, _, dropped) = routing_dump(&f.dir);
    assert_eq!(buckets.get(&0), Some(&K), "bucket 0 does not hold exactly {K}");
    assert_eq!(
        dropped,
        f.peers.len() - buckets.values().sum::<usize>(),
        "dropped_full does not account for every peer that did not fit"
    );
}

/// R3 — incumbent-first, both halves.
///
/// The first k offered into a bucket are kept; every later one is refused.
/// Presence is read through the protocol: a peer in the table is returned by
/// `#find_node` aimed at its own id.
///
/// Baseline: `#find_node` is not implemented.
#[test]
#[ignore]
fn r3_incumbent_first_keeps_the_early_ones() {
    let f = fixture("r3", 220);
    let b0: Vec<&SynthPeer> = f.peers.iter()
        .filter(|p| bucket_index(&f.self_id, &p.id) == 0).collect();
    assert!(b0.len() > K + 10, "HARNESS: too few bucket-0 candidates: {}", b0.len());

    let present = |p: &SynthPeer| {
        let r = ask_raw(f.node.port, &find_node_request("x", &hex::encode(p.id)));
        answer_ids(&r).iter().any(|id| *id == p.id)
    };

    for (i, p) in b0.iter().take(K).enumerate() {
        assert!(present(p), "early peer {i} of bucket 0 was not kept");
    }
    for (i, p) in b0.iter().skip(K).take(10).enumerate() {
        assert!(
            !present(p),
            "late peer {} of bucket 0 was admitted — incumbent-first means the \
             attacker must be EARLY, and this lets them be merely persistent",
            i + K
        );
    }
}

/// R4 — a peer already present refreshes; it consumes no slot and evicts nobody.
///
/// Baseline: `oo node routing` does not exist.
#[test]
#[ignore]
fn r4_readvertising_refreshes_rather_than_competes() {
    let f = fixture("r4", 60);
    let (before, total_before, dropped_before) = routing_dump(&f.dir);

    for p in f.peers.iter().take(10) {
        let r = ask_raw(f.node.port, &p.advertise_request(&f.caid_dir));
        assert_eq!(status_of(&r), "success", "re-advertisement refused: {r}");
    }

    let (after, total_after, dropped_after) = routing_dump(&f.dir);
    assert_eq!(before, after, "re-advertising changed bucket occupancy");
    assert_eq!(total_before, total_after, "re-advertising changed the total");
    assert_eq!(
        dropped_before, dropped_after,
        "a re-advertisement was counted as a peer that did not fit"
    );
}

/// R5 — **the probe this arc exists for.**
///
/// `closest(target, k)` must equal brute force over everything the table
/// holds, for targets at every depth. A table that searches only the bucket
/// the target falls into passes R1 and R2 and fails here, because the k
/// nearest ids to an arbitrary target are spread across several buckets.
///
/// Deep targets are built by flipping low bits of a known id — the query path
/// reaches depths that random population never will.
///
/// Baseline: `#find_node` is not implemented.
#[test]
#[ignore]
fn r5_closest_equals_brute_force_at_every_depth() {
    let f = fixture("r5", 220);
    let held = simulate_table(&f.self_id, &f.peers);
    assert!(held.len() > K, "HARNESS: table too small to discriminate: {}", held.len());

    let mut targets: Vec<[u8; ID_BYTES]> = Vec::new();
    // Shallow: random.
    let rng = ring::rand::SystemRandom::new();
    for i in 0..10 { targets.push(mint_peer(&rng, 21000 + i).id); }
    // Deep: a held peer's id with progressively higher bits flipped, so the
    // target sits at distance 2^b from a peer the table certainly has.
    for b in [0usize, 1, 3, 7, 15, 31, 63, 100, 140, 159] {
        let mut t = held[0].id;
        t[(159 - b) / 8] ^= 1 << (b % 8);
        targets.push(t);
    }

    let mut worst_rank_disagreement = 0usize;
    for t in &targets {
        let expect = brute_force_closest(t, &held, MAX_FIND_NODE_PEERS);
        let got = answer_ids(&ask_raw(f.node.port, &find_node_request("x", &hex::encode(t))));
        assert_eq!(
            got.len(),
            expect.len(),
            "answer length differs for target {}",
            hex::encode(t)
        );
        for (i, (g, e)) in got.iter().zip(expect.iter()).enumerate() {
            if g != e { worst_rank_disagreement = worst_rank_disagreement.max(i + 1); }
        }
        assert_eq!(
            got,
            expect,
            "closest() disagrees with brute force at target {} — first \
             disagreement at rank {worst_rank_disagreement}",
            hex::encode(t)
        );
    }
}

/// R6 — self is neither stored nor returned.
///
/// Baseline: `#find_node` is not implemented.
#[test]
#[ignore]
fn r6_self_is_never_in_the_table() {
    let f = fixture("r6", 40);
    let me = hex::encode(f.self_id);
    let r = ask_raw(f.node.port, &find_node_request("x", &me));
    assert_eq!(status_of(&r), "success", "{r}");
    assert!(
        !answer_ids(&r).iter().any(|id| *id == f.self_id),
        "the node returned itself as one of the nodes closest to itself: {r}"
    );
    let (buckets, _, _) = routing_dump(&f.dir);
    assert!(
        !buckets.contains_key(&160),
        "a bucket at index 160 exists — that index means `equal to self`"
    );
}

/// R7 — a relayed record from `#find_node` verifies from the packet alone.
///
/// Baseline: `#find_node` is not implemented.
#[test]
#[ignore]
fn r7_relayed_records_verify_from_the_packet_alone() {
    let f = fixture("r7", 30);
    let target = hex::encode(f.peers[0].id);
    let r = ask_raw(f.node.port, &find_node_request("x", &target));
    let ads = peer_ads(&r);
    assert!(!ads.is_empty(), "no records to verify: {r}");

    for ad in &ads {
        let pk_hex = {
            let n = "public_key: \"";
            let i = ad.find(n).unwrap();
            ad[i + n.len()..].split('"').next().unwrap().to_string()
        };
        let sig_hex = {
            let n = "signature: \"";
            let i = ad.find(n).unwrap();
            ad[i + n.len()..].split('"').next().unwrap().to_string()
        };
        let body = {
            let i = ad.find(", signature:").unwrap();
            format!("{} }}}}", &ad[..i])
        };
        let caid = caid_of(&f.caid_dir, &body);
        let payload = format!("{ADVERT_DOMAIN}{caid}");
        ring::signature::UnparsedPublicKey::new(
            &ring::signature::ED25519,
            &hex::decode(&pk_hex).unwrap(),
        )
        .verify(payload.as_bytes(), &hex::decode(&sig_hex).unwrap())
        .unwrap_or_else(|_| panic!("relayed record does not verify: {ad}"));
    }
}

/// R8 — `%target` is a 160-bit id and nothing else. A CAID is refused.
///
/// Letting the op accept a CAID is how "who is near this id" and "who serves
/// this CAID" get confused into one question again.
///
/// Baseline: every `#find_node` is `#not_implemented`, so the two cases are
/// indistinguishable.
#[test]
#[ignore]
fn r8_target_is_an_id_not_a_caid() {
    let f = fixture("r8", 10);
    let good = hex::encode(f.peers[0].id);
    assert_eq!(
        status_of(&ask_raw(f.node.port, &find_node_request("x", &good))),
        "success",
        "LIVENESS: a well-formed target was not answered"
    );
    for bad in [
        f.peers[0].node_id.clone(),          // a CAID
        good.to_uppercase(),                 // uppercase
        good[..38].to_string(),              // too short
        format!("{good}00"),                 // too long
        "zz".repeat(20),                     // not hex
        String::new(),                       // absent
    ] {
        assert_eq!(
            status_of(&ask_raw(f.node.port, &find_node_request("x", &bad))),
            "conflict",
            "malformed %target accepted: {bad:?}"
        );
    }
}

/// R9 — `%from` never enters the table.
///
/// Standard Kademlia learns a node from any message it sends, because it has
/// nothing better. Ours has signed advertisements, and `%from` is an unsigned
/// claim — learning from it would hand an attacker arbitrary insertions.
///
/// Baseline: `oo node routing` does not exist.
#[test]
#[ignore]
fn r9_the_table_learns_only_from_signed_advertisements() {
    let f = fixture("r9", 40);
    let (before, total_before, _) = routing_dump(&f.dir);

    let rng = ring::rand::SystemRandom::new();
    let stranger = mint_peer(&rng, 29999);
    for _ in 0..5 {
        ask_raw(
            f.node.port,
            &find_node_request(&stranger.node_id, &hex::encode(stranger.id)),
        );
    }

    let (after, total_after, _) = routing_dump(&f.dir);
    assert_eq!(total_before, total_after, "the table grew from an unsigned %from");
    assert_eq!(before, after, "bucket occupancy changed from an unsigned %from");
    let r = ask_raw(f.node.port, &find_node_request("x", &hex::encode(stranger.id)));
    assert!(
        !answer_ids(&r).iter().any(|id| *id == stranger.id),
        "a node that only ever *asked* is now being advertised to others: {r}"
    );
}

/// R10 — the answer does not depend on who asks.
///
/// Baseline: three identical `#not_implemented` replies, which is why the
/// liveness assertion is what makes this red.
#[test]
#[ignore]
fn r10_the_answer_does_not_depend_on_who_asks() {
    let f = fixture("r10", 40);
    let target = hex::encode(f.peers[0].id);
    let mut answers = Vec::new();
    for from in ["", &f.peers[1].node_id, "hash:sha256:v1:not-a-node"] {
        let r = ask_raw(f.node.port, &find_node_request(from, &target));
        let ids = answer_ids(&r);
        assert!(
            !ids.is_empty(),
            "LIVENESS: %from={from:?} produced no peers, so 'identical' would \
             mean 'identically empty': {r}"
        );
        answers.push(ids);
    }
    for i in 1..answers.len() {
        assert_eq!(answers[0], answers[i], "the answer changed with %from");
    }
}

/// R11 — a `#find_node` reply whose `%ad` computes.
///
/// The standing rule from v0.2.50, at this arc's remote-input entry point: an
/// adversarial case must include a payload that computes, not only payloads of
/// the wrong shape. A good record travels in the same reply, so a client that
/// simply never processes anything cannot pass by doing nothing.
///
/// Baseline: `oo node find-node` does not exist.
#[test]
#[ignore]
fn r11_a_relayed_body_that_computes_is_refused_before_it_runs() {
    let dir = fresh_dir("r11-client");
    let caid_dir = fresh_dir("r11-caid");
    init(&dir);
    init(&caid_dir);

    let rng = ring::rand::SystemRandom::new();
    let good_peer = mint_peer(&rng, 21500);
    let good = good_peer.signed_advert(&caid_dir);

    let loot = dir.join("pwned-by-find-node.txt");
    assert!(!loot.exists(), "probe error: loot path already exists");
    let payload = format!(
        "~%Io./write_file(\"{}\", \"owned via #find_node\")",
        loot.display()
    );

    let reply = serde_json::json!({
        "%status": "#success", "%source": "x", "%hops": 1,
        "%peers": [
            {"%ad": good, "%observed_host": "198.51.100.1"},
            {"%ad": payload, "%observed_host": "198.51.100.2"},
        ]
    })
    .to_string();

    let relay = spawn_relayer(reply);
    let out = oo(&dir, &["node", "find-node", "--to", &relay.addr(),
                         "--target", &hex::encode(good_peer.id)]);

    assert!(!loot.exists(), "a relayed body was EVALUATED: {} exists\n{out}", loot.display());
    assert!(!relay.asked().is_empty(), "LIVENESS: the client never connected: {out}");
    assert!(
        out.contains(&good_peer.node_id),
        "LIVENESS: the good record in the same reply was not accepted, so the \
         absence of the file proves nothing: {out}"
    );
}

/// R12 — both budgets: at most k entries out, and an oversized reply in is named.
///
/// The v0.2.51 acceptance repair established that a budget only the honest
/// side keeps is not a budget. It applies here unchanged.
///
/// Baseline: `#find_node` is not implemented; `oo node find-node` does not exist.
#[test]
#[ignore]
fn r12_both_budgets_are_kept() {
    let f = fixture("r12", 220);
    let r = ask_raw(f.node.port, &find_node_request("x", &hex::encode(f.peers[0].id)));
    let n = answer_ids(&r).len();
    assert!(n > 0, "LIVENESS: 220 advertisements landed and none came back: {r}");
    assert!(n <= MAX_FIND_NODE_PEERS, "answer carries {n} peers, cap is {MAX_FIND_NODE_PEERS}");
    assert!(r.len() <= MAX_REPLY_BYTES, "answer is {} bytes, budget is {MAX_REPLY_BYTES}", r.len());

    // …and the client refuses an oversized reply by name rather than
    // processing a truncated prefix.
    let dir = fresh_dir("r12-client");
    init(&dir);
    let flood = spawn_flooder(MAX_REPLY_BYTES * 4);
    let out = oo(&dir, &["node", "find-node", "--to", &flood.addr(),
                         "--target", &"a".repeat(40)]);
    assert!(
        out.contains("oversize"),
        "an oversized reply was not named — a truncated prefix must never be \
         processed as though it were a short answer: {out}"
    );
}

// ── fake peers for the client-side reds ─────────────────────────────────

struct Relayer { port: u16, asked: Arc<Mutex<Vec<String>>> }

impl Relayer {
    fn addr(&self) -> String { format!("127.0.0.1:{}", self.port) }
    fn asked(&self) -> Vec<String> { self.asked.lock().unwrap().clone() }
}

fn spawn_relayer(reply: String) -> Relayer {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let asked = Arc::new(Mutex::new(Vec::new()));
    let log = Arc::clone(&asked);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut s) = stream else { continue };
            let Ok(c) = s.try_clone() else { continue };
            let mut line = String::new();
            if std::io::BufRead::read_line(&mut std::io::BufReader::new(c), &mut line).is_err() {
                continue;
            }
            log.lock().unwrap().push(line.trim().to_string());
            let _ = s.write_all(reply.as_bytes());
            let _ = s.write_all(b"\n");
            let _ = s.flush();
            let _ = s.shutdown(std::net::Shutdown::Write);
        }
    });
    Relayer { port, asked }
}

/// Sends `total` bytes of a well-formed prefix that never ends, without ever
/// pausing — so a read timeout, which answers a stall, never fires.
fn spawn_flooder(total: usize) -> Relayer {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let asked = Arc::new(Mutex::new(Vec::new()));
    let log = Arc::clone(&asked);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut s) = stream else { continue };
            let Ok(c) = s.try_clone() else { continue };
            let mut line = String::new();
            let _ = std::io::BufRead::read_line(&mut std::io::BufReader::new(c), &mut line);
            log.lock().unwrap().push(line.trim().to_string());
            let _ = s.write_all(br##"{"%status":"#success","%source":"x","%hops":1,"%peers":["##);
            let chunk = format!(r#"{{"%ad":"{}","%observed_host":"1.2.3.4"}},"#, "A".repeat(4096));
            let mut sent = 0usize;
            while sent < total {
                if s.write_all(chunk.as_bytes()).is_err() { break; }
                sent += chunk.len();
            }
            let _ = s.shutdown(std::net::Shutdown::Write);
        }
    });
    Relayer { port, asked }
}

// ════════════════════════════════════════════════════════════════════════
//  PINS — green on v0.2.51, must stay green
// ════════════════════════════════════════════════════════════════════════

/// P1 — `#discover` still answers from the service index, unchanged.
#[test]
fn p1_discover_untouched() {
    let dir = fresh_dir("p1-srv");
    let caid_dir = fresh_dir("p1-caid");
    init(&dir);
    init(&caid_dir);
    let node = serve(&dir);
    let target = caid_of(&caid_dir, "{{ p1: 1 }}");

    let rng = ring::rand::SystemRandom::new();
    let p = mint_peer(&rng, 21777);
    let body = p.body(&[target.clone()], now_secs(), 15);
    let caid = caid_of(&caid_dir, &body);
    let sig = hex::encode(p.key_pair.sign(format!("{ADVERT_DOMAIN}{caid}").as_bytes()).as_ref());
    let inner = body.trim_start_matches("{{").trim_end_matches("}}").trim();
    let ad = format!("{{{{ {inner}, signature: \"{sig}\" }}}}");
    let r = ask_raw(node.port, &format!(
        "{{{{ %op: #advertise, %from: \"{}\", %ad: {ad} }}}}\n", p.node_id));
    assert_eq!(status_of(&r), "success", "{r}");

    let d = ask_raw(node.port, &format!(
        "{{{{ %op: #discover, %from: \"x\", %target: \"{target}\" }}}}\n"));
    assert_eq!(status_of(&d), "success", "{d}");
    assert!(
        peer_ads(&d).iter().any(|a| ad_node_id(a) == p.node_id),
        "the service index stopped answering: {d}"
    );
}

/// P2 — the advertise ladder is intact, including that a body which computes
/// is still refused before it runs (the v0.2.50 repair).
#[test]
fn p2_advertise_ladder_intact() {
    let dir = fresh_dir("p2-srv");
    let caid_dir = fresh_dir("p2-caid");
    init(&dir);
    init(&caid_dir);
    let node = serve(&dir);
    let rng = ring::rand::SystemRandom::new();
    let p = mint_peer(&rng, 21778);

    let r = ask_raw(node.port, &p.advertise_request(&caid_dir));
    assert_eq!(status_of(&r), "success", "a valid advertisement: {r}");

    let r = ask_raw(node.port, &format!(
        "{{{{ %op: #advertise, %from: \"hash:sha256:v1:someone-else\", %ad: {} }}}}\n",
        p.signed_advert(&caid_dir)));
    assert_eq!(status_of(&r), "rejected", "{r}");
    assert_eq!(field_of(&r, "%reason").unwrap_or_default(), "identity_mismatch", "{r}");

    let loot = dir.join("p2-must-not-exist.txt");
    let bomb = format!(
        "{{{{ %op: #advertise, %from: \"{}\", %ad: ~%Io./write_file(\"{}\", \"x\") }}}}\n",
        p.node_id, loot.display());
    let r = ask_raw(node.port, &bomb);
    assert_eq!(status_of(&r), "rejected", "{r}");
    assert!(!loot.exists(), "the v0.2.50 repair regressed");
}

/// P3 — a filled and queried routing table never reaches the universe root.
///
/// SPEC_13 §4.1.2 #3. This arc adds engine-local state derived from *other
/// machines*; if any of it reached the root, two nodes with different peers
/// could never share a universe identity.
#[test]
fn p3_the_table_never_reaches_the_root() {
    let src = "world: {\n  greet: \"hello\"\n  n: 7\n}\n";
    let root_digest = |dir: &Path| -> String {
        let c = oo(dir, &["log"]).lines()
            .find_map(|l| l.strip_prefix("commit ").map(|s| s.trim().to_string()))
            .unwrap_or_default();
        assert!(c.starts_with("hash:sha256:"), "no HEAD commit in {dir:?}");
        let d = c.rsplit(':').next().unwrap().to_string();
        let p = dir.join(".oo").join("objects").join("sha256").join(&d[..2]).join(&d[2..]);
        let commit: serde_json::Value =
            serde_json::from_slice(&fs::read(&p).unwrap_or_else(|e| panic!("{p:?}: {e}"))).unwrap();
        let dg = commit["root"]["digest"].clone();
        let hex = if let Some(s) = dg.as_str() { s.to_string() }
            else if let Some(a) = dg.as_array() {
                a.iter().map(|b| format!("{:02x}", b.as_u64().unwrap())).collect()
            } else { panic!("no usable root digest: {}", commit["root"]) };
        assert_eq!(hex.len(), 64, "root digest is not 64 hex: {hex:?}");
        hex
    };

    let quiet = fresh_dir("p3-quiet");
    init(&quiet);
    write(&quiet, "u.n", src);
    oo(&quiet, &["evolve", "u.n"]);
    oo(&quiet, &["commit", "-m", "p3"]);

    let busy = fresh_dir("p3-busy");
    let caid_dir = fresh_dir("p3-caid");
    init(&busy);
    init(&caid_dir);
    let node = serve(&busy);
    let rng = ring::rand::SystemRandom::new();
    for i in 0..40 {
        let p = mint_peer(&rng, 21800 + i);
        let r = ask_raw(node.port, &p.advertise_request(&caid_dir));
        assert_eq!(status_of(&r), "success", "P3 setup advertisement {i}: {r}");
    }
    write(&busy, "u.n", src);
    oo(&busy, &["evolve", "u.n"]);
    oo(&busy, &["commit", "-m", "p3"]);

    assert_eq!(
        root_digest(&quiet),
        root_digest(&busy),
        "a node that has met peers committed a different universe than one \
         that has not — engine-local state reached the root"
    );
}

/// P4 — nothing is persisted and nothing is stored.
#[test]
fn p4_nothing_persisted() {
    let dir = fresh_dir("p4-srv");
    let caid_dir = fresh_dir("p4-caid");
    init(&dir);
    init(&caid_dir);
    let node = serve(&dir);
    let before = object_count(&dir);

    let rng = ring::rand::SystemRandom::new();
    for i in 0..30 {
        let p = mint_peer(&rng, 21900 + i);
        assert_eq!(status_of(&ask_raw(node.port, &p.advertise_request(&caid_dir))), "success");
    }
    ask_raw(node.port, &find_node_request("x", &"b".repeat(40)));

    assert_eq!(object_count(&dir), before, "the node wrote objects while routing");
    assert!(
        !dir.join(".oo").join("routing").exists(),
        "`.oo/routing/` appeared — REAL_02 §5.1 records that file as a blueprint, \
         and a directory that becomes durable without anyone deciding it should \
         is how persistence arrives unaudited"
    );
}

/// P5 — `#fetch` is untouched and still independent of `%from`.
#[test]
fn p5_fetch_untouched() {
    let dir = fresh_dir("p5-srv");
    init(&dir);
    write(&dir, "i.n", "id: ~%Discovery./identify_and_store { p5: 1 }\n");
    let caid = first_string(&oo(&dir, &["run", "i.n", "--observe", "id"]));
    let node = serve(&dir);
    for from in ["", "hash:sha256:v1:whoever", "not-a-caid"] {
        let r = ask_raw(node.port, &format!(
            "{{{{ %op: #fetch, %from: \"{from}\", %hash: \"{caid}\" }}}}\n"));
        assert_eq!(status_of(&r), "success", "%from={from:?}: {r}");
    }
}
