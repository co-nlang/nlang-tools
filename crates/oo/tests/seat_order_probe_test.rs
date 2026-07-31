// 在席是誰 / which three hold the seats — REAL_02 §4.2.6.3 (2026-07-31).
// Pre-committed by work order: docs/seat_order_handover.md
//
// ── The defect ───────────────────────────────────────────────────────────
//
// §4.2.6.2 caps automatic fetch sources at three and says overflow keeps the
// incumbents. Inside one process that is arrival order. Across a restart it is
// not: the set is rebuilt from the durable directory, sorted by `received_at`
// and then by `node_id`.
//
// `received_at` is stored with **one-second resolution**. Everything that
// arrives inside the same second therefore ties, and the tie is broken by
// `node_id` ascending — a hash of the peer's public key.
//
// Measured 2026-07-31, five adverts accepted in one second (their durable
// `received_at` values byte-identical):
//
//   seats went to the three lowest node_id, not to the first three to arrive
//   the same three every time across five restarts
//
// So the rebuild is **deterministic**; it is deterministically *wrong*. (The
// first draft of §4.2.6.3 claimed the opposite — that restarts could disagree
// — from reading the code instead of running it. The clause now records the
// correction.)
//
// ── Why a hash order here is worse than a hash order in the routing table ──
//
// `node_id = sha256(public key)` and SPEC_15 §7.1 prices minting at a measured
// 3,500 keys/second. The routing table also sorts by identity, but its bucket
// index is an XOR against **the victim's own id**, so grinding buys a seat at
// one victim. Raw `node_id` ascending is **victim-independent**: the smallest
// id is smallest for everyone.
//
//   Grind once, hold a seat on every node that ever hears you in the same
//   second as someone else.
//
// This is §4.3.5.1's line — "a deterministic rule is a rule the attacker
// computes offline too" — reappearing one layer down.
//
// ── Why the answer is not sampling ───────────────────────────────────────
//
// §4.3.5.1 answered the same shape (a cap that never said which) with a
// declared uniform sample. That answer does not transfer. A discovery reply is
// not a verdict (§4.3.2) and re-drawing costs only latency; an automatic
// source spends the operator's time and tells someone what they are looking
// for (§4.2.6.1). Re-drawing every restart would unseat honest sources for no
// reason and hand an attacker a fresh chance at every boot instead of none.
//
// ── What is being asked for ──────────────────────────────────────────────
//
// B′: the durable record must make arrival order **total**, and the rebuild
//     must follow it. Finer timestamps, an admission sequence number, or
//     anything else — the probe pins the property, not the spelling.
// C′: even so, a tie must not be broken by the peer's identity alone. That is
//     a MUST NOT rather than a check: once B′ holds it is unreachable, and a
//     rule that guards an unreachable state is a red line, not a gate. It has
//     no probe here on purpose, and saying so is cheaper than pretending.

use std::fs;
use std::io::BufRead;
use std::io::{BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use nlang_interpreter::value::Identity;
use ring::signature::{Ed25519KeyPair, KeyPair};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

const ADVERT_DOMAIN: &str = "oodp-advert:v1:";
const AFFILIATION_DOMAIN: &str = "oodp-affiliation:v1:";
/// REAL_02 §4.2.6.2.
const AUTOMATIC_REMOTE_CAP: usize = 3;
/// Candidates in the overflow fixtures. Five is enough to make "first three"
/// and "three lowest node_id" disjoint questions while keeping the run short.
const CANDIDATES: usize = 5;

// ── harness ─────────────────────────────────────────────────────────────

fn fresh_dir(tag: &str) -> PathBuf {
    let mut d = std::env::temp_dir();
    d.push(format!(
        "nlang-seat-{}-{}-{}",
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

fn init(dir: &Path) {
    oo(dir, &["run", "--help"]);
    fs::write(dir.join("seed.n"), "seed: { ok: #true }\n").unwrap();
    oo(dir, &["run", "seed.n"]);
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
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

fn store(dir: &Path, expr: &str) -> String {
    let out = oo(
        dir,
        &["eval", &format!("~%Discovery./identify_and_store {expr}")],
    );
    first_string(&out)
}

fn peers_file(dir: &Path) -> PathBuf {
    dir.join(".oo").join("peers").join("directory")
}

fn free_port() -> u16 {
    for _ in 0..64 {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let p = l.local_addr().unwrap().port();
        if p > 35000 {
            return p;
        }
    }
    panic!("no free port above 35000");
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
    fn log(&self) -> String {
        fs::read_to_string(&self.log).unwrap_or_default()
    }
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
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return node;
        }
    }
    panic!("`oo node serve` never came up: {}", node.log());
}

fn ask_raw(port: u16, payload: &str) -> String {
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    s.write_all(payload.as_bytes()).unwrap();
    if !payload.ends_with('\n') {
        s.write_all(b"\n").unwrap();
    }
    s.flush().unwrap();
    s.shutdown(std::net::Shutdown::Write).ok();
    let mut buf = Vec::new();
    s.read_to_end(&mut buf).ok();
    String::from_utf8_lossy(&buf).to_string()
}

fn status_of(reply: &str) -> String {
    serde_json::from_str::<serde_json::Value>(reply.trim())
        .ok()
        .and_then(|j| {
            j.get("%status")
                .and_then(|v| v.as_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| format!("<no %status in {}>", reply.trim()))
        .trim_start_matches('#')
        .to_string()
}

// ── keys ────────────────────────────────────────────────────────────────

struct Key {
    kp: Ed25519KeyPair,
    pk_hex: String,
    node_id: String,
}

fn mint(rng: &ring::rand::SystemRandom) -> Key {
    let pkcs8 = Ed25519KeyPair::generate_pkcs8(rng).unwrap();
    let kp = Ed25519KeyPair::from_pkcs8(pkcs8.as_ref()).unwrap();
    let pk = kp.public_key().as_ref().to_vec();
    let node_id = Identity {
        public_key: pk.clone(),
        private_key: Vec::new(),
    }
    .node_id_caid()
    .to_string();
    Key {
        pk_hex: hex::encode(&pk),
        node_id,
        kp,
    }
}

fn affiliation_block(op: &Key, node_id: &str, expires: i64) -> String {
    let sig = hex::encode(
        op.kp
            .sign(format!("{AFFILIATION_DOMAIN}{node_id}:{expires}").as_bytes())
            .as_ref(),
    );
    format!(
        ", affiliation: {{{{ operator_key: \"{}\", signature: \"{}\", expires: {} }}}}",
        op.pk_hex, sig, expires
    )
}

/// `.oo/discovery.n` is closed data: `affiliation_roots` at the top level and
/// nothing else. Calibration caught a nested `discovery: { … }` here, which the
/// engine refuses at startup — the closure the v0.7.0 arc built, doing its job.
fn write_roots(dir: &Path, roots: &[&str]) {
    fs::create_dir_all(dir.join(".oo")).unwrap();
    let body = roots
        .iter()
        .map(|root| format!("    \"{root}\""))
        .collect::<Vec<_>>()
        .join(",\n");
    let text = if body.is_empty() {
        "affiliation_roots: []\n".to_string()
    } else {
        format!("affiliation_roots: [\n{body}\n]\n")
    };
    fs::write(dir.join(".oo").join("discovery.n"), text).unwrap();
}

// ── fake sources ────────────────────────────────────────────────────────

struct FakePeer {
    port: u16,
    asked: Arc<Mutex<usize>>,
}
impl FakePeer {
    fn hits(&self) -> usize {
        *self.asked.lock().unwrap()
    }
}

/// Answers every request with bytes that are not the object, so a scan never
/// short-circuits and every seated source is visited.
fn spawn_peer() -> FakePeer {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let asked = Arc::new(Mutex::new(0usize));
    let seen = Arc::clone(&asked);
    std::thread::spawn(move || {
        for incoming in listener.incoming() {
            let Ok(mut s) = incoming else { continue };
            let Ok(clone) = s.try_clone() else { continue };
            let mut line = String::new();
            if BufReader::new(clone).read_line(&mut line).is_err() {
                continue;
            }
            *seen.lock().unwrap() += 1;
            let _ = s.write_all(b"NOT_THE_OBJECT");
            let _ = s.flush();
            let _ = s.shutdown(std::net::Shutdown::Write);
        }
    });
    FakePeer { port, asked }
}

// ── fixture ─────────────────────────────────────────────────────────────

struct Candidate {
    key: Key,
    peer: FakePeer,
    request: String,
}

struct Seats {
    dir: PathBuf,
    object_caid: String,
    unreachable: String,
    candidates: Vec<Candidate>,
}

/// Build `n` eligible candidates and pre-render their advertise requests, so
/// the sends carry no subprocess between them and can land in one second.
fn seats(tag: &str, n: usize) -> Seats {
    let rng = ring::rand::SystemRandom::new();
    let dir = fresh_dir(tag);
    init(&dir);
    let object_caid = store(&dir, "{ seat_order: \"payload\" }");
    assert!(
        object_caid.starts_with("hash:sha256:"),
        "fixture object not stored"
    );
    let op = mint(&rng);
    write_roots(&dir, &[&op.pk_hex]);

    let mut candidates = Vec::new();
    for _ in 0..n {
        let key = mint(&rng);
        let peer = spawn_peer();
        let claim = affiliation_block(&op, &key.node_id, now_secs() + 3600);
        let body = format!(
            "{{{{ node_id: \"{}\", public_key: \"{}\", services: [\"{}\"], \
             listen_port: {}, capacity: 10, ts: {}, ttl: 15{claim} }}}}",
            key.node_id,
            key.pk_hex,
            object_caid,
            peer.port,
            now_secs()
        );
        let body_caid = caid_of(&dir, &body);
        let sig = hex::encode(
            key.kp
                .sign(format!("{ADVERT_DOMAIN}{body_caid}").as_bytes())
                .as_ref(),
        );
        let inner = body.trim_start_matches("{{").trim_end_matches("}}").trim();
        let request = format!(
            "{{{{ %op: #advertise, %from: \"{}\", %ad: {{{{ {inner}, signature: \"{sig}\" }}}} }}}}\n",
            key.node_id
        );
        candidates.push(Candidate { key, peer, request });
    }

    let unreachable = {
        let (head, digest) = object_caid.rsplit_once(':').unwrap();
        let mut c: Vec<char> = digest.chars().collect();
        let last = c.len() - 1;
        c[last] = if c[last] == 'a' { 'b' } else { 'a' };
        format!("{head}:{}", c.into_iter().collect::<String>())
    };

    Seats {
        dir,
        object_caid,
        unreachable,
        candidates,
    }
}

impl Seats {
    /// Advertise `order` (indices) back to back into one running node.
    /// Returns how many whole seconds the burst spanned.
    fn advertise(&self, order: &[usize]) -> i64 {
        self.advertise_logged(order).0
    }

    /// As [`Self::advertise`], plus the serve console output — which is how a
    /// compaction announces itself (`OODP Peers: compact …`).
    fn advertise_logged(&self, order: &[usize]) -> (i64, String) {
        let node = serve(&self.dir);
        let t0 = now_secs();
        for &i in order {
            let reply = ask_raw(node.port, &self.candidates[i].request);
            assert_eq!(
                status_of(&reply),
                "success",
                "candidate {i} refused: {reply}"
            );
        }
        let t1 = now_secs();
        let log = node.log();
        drop(node);
        (t1 - t0, log)
    }

    /// Which candidates hold a seat, read from a fresh process by scanning for
    /// an object nobody can serve so every seated source is visited.
    fn seated(&self) -> Vec<usize> {
        let before: Vec<usize> = self.candidates.iter().map(|c| c.peer.hits()).collect();
        oo(
            &self.dir,
            &[
                "eval",
                &format!("~%Discovery./fetch \"{}\"", self.unreachable),
            ],
        );
        self.candidates
            .iter()
            .enumerate()
            .filter(|(i, c)| c.peer.hits() > before[*i])
            .map(|(i, _)| i)
            .collect()
    }

    /// Durable `received_at` per candidate, last record wins.
    fn received_at(&self) -> Vec<Option<i64>> {
        let text = fs::read_to_string(peers_file(&self.dir)).unwrap_or_default();
        self.candidates
            .iter()
            .map(|c| {
                text.lines()
                    .filter(|l| l.contains(&c.key.node_id))
                    .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
                    .last()
                    .and_then(|v| v.get("received_at").and_then(|x| x.as_i64()))
            })
            .collect()
    }

    /// Candidate indices in ascending `node_id` — the order the engine falls
    /// back to today whenever `received_at` ties.
    fn by_node_id(&self) -> Vec<usize> {
        let mut v: Vec<usize> = (0..self.candidates.len()).collect();
        v.sort_by(|a, b| {
            self.candidates[*a]
                .key
                .node_id
                .cmp(&self.candidates[*b].key.node_id)
        });
        v
    }
}

// ════════════════════════════════════════════════════════════════════════
// CONTROLS
// ════════════════════════════════════════════════════════════════════════

/// C1 — the harness can see seats at all, and the cap is real.
#[test]
fn c1_the_fixture_overflows_and_seats_are_observable() {
    let f = seats("c1", CANDIDATES);
    let order: Vec<usize> = (0..CANDIDATES).collect();
    f.advertise(&order);
    let held = f.seated();
    assert_eq!(
        held.len(),
        AUTOMATIC_REMOTE_CAP,
        "expected exactly {AUTOMATIC_REMOTE_CAP} seats from {CANDIDATES} eligible \
         candidates, saw {held:?}"
    );
}

/// C2 — under the cap nothing is chosen: every eligible candidate seats.
#[test]
fn c2_under_the_cap_everyone_seats() {
    let f = seats("c2", AUTOMATIC_REMOTE_CAP);
    let order: Vec<usize> = (0..AUTOMATIC_REMOTE_CAP).collect();
    f.advertise(&order);
    assert_eq!(
        f.seated(),
        order,
        "a candidate lost its seat with room to spare"
    );
}

// ════════════════════════════════════════════════════════════════════════
// REDS — `#[ignore]` until delivery. Delivery removes ONLY the attribute.
// ════════════════════════════════════════════════════════════════════════

/// R1 — seats follow arrival order even when arrivals share a second.
///
/// The candidates are advertised in **descending `node_id`**, so today's
/// fallback (ascending `node_id`) hands the seats to the *last* three to
/// arrive. This is a construction, not a draw: it is red every run, with no
/// fixture to redraw and no probability to quote.
#[test]
#[ignore]
fn r1_same_second_arrivals_keep_arrival_order() {
    let f = seats("r1", CANDIDATES);
    let mut order = f.by_node_id();
    order.reverse();
    let span = f.advertise(&order);

    let stamps = f.received_at();
    assert!(
        stamps.iter().all(|s| s.is_some()),
        "a candidate never reached the durable directory: {stamps:?}"
    );
    assert!(
        span <= 1,
        "the burst spanned {span} whole seconds, so this run does not exercise \
         a same-second tie at all"
    );

    let expected: Vec<usize> = {
        let mut v = order[..AUTOMATIC_REMOTE_CAP].to_vec();
        v.sort();
        v
    };
    let held = f.seated();
    assert_eq!(
        held,
        expected,
        "seats did not follow arrival order.\n arrival order: {order:?}\n \
         node_id ascending: {:?}\n durable received_at: {stamps:?}\n \
         seats: {held:?}, expected the first {AUTOMATIC_REMOTE_CAP} to arrive: {expected:?}",
        f.by_node_id()
    );
}

/// R2 — arrival order survives compaction.
///
/// The durable directory is rewritten once it outgrows its live set. A rewrite
/// that keeps only what today's format holds would silently restore the
/// second-resolution tie, and R1 would still pass on a fresh file while real
/// deployments — which compact — lost the property.
///
/// **At baseline this fails at its precondition**, because the property it
/// builds on is the one R1 is about; it therefore adds nothing until R1 passes.
/// That is the honest shape for a layered property, but it means the delivery
/// must not read a green R2 as independent evidence. The compaction assertion
/// below is what it is actually for, and it requires the rewrite to have been
/// *announced* — otherwise a run where compaction never triggered would pass
/// while testing nothing.
#[test]
#[ignore]
fn r2_arrival_order_survives_compaction() {
    let f = seats("r2", CANDIDATES);
    let mut order = f.by_node_id();
    order.reverse();
    let span = f.advertise(&order);
    assert!(
        span <= 1,
        "burst spanned {span} seconds; not a same-second tie"
    );

    let expected: Vec<usize> = {
        let mut v = order[..AUTOMATIC_REMOTE_CAP].to_vec();
        v.sort();
        v
    };
    assert_eq!(
        f.seated(),
        expected,
        "precondition: seats are wrong before compaction"
    );

    // Re-advertise every candidate several times to grow the append log past
    // its live set and force a rewrite. The rewrite must be *observed*: if it
    // never happens this probe passes without testing anything.
    let before = fs::metadata(peers_file(&f.dir))
        .map(|m| m.len())
        .unwrap_or(0);
    let mut compacted = false;
    for _ in 0..6 {
        let (_, log) = f.advertise_logged(&order);
        if log.contains("OODP Peers: compact") {
            compacted = true;
        }
    }
    let after = fs::metadata(peers_file(&f.dir))
        .map(|m| m.len())
        .unwrap_or(0);
    assert!(
        before > 0 && after > 0,
        "harness: the durable directory is empty, so compaction proves nothing"
    );
    assert!(
        compacted,
        "no compaction was announced in six rounds, so this probe would pass \
         without ever rewriting the file (file {before} -> {after} bytes)"
    );

    let held = f.seated();
    assert_eq!(
        held, expected,
        "seats changed after the directory was rewritten.\n arrival order: {order:?}\n \
         seats now: {held:?}, expected {expected:?}\n file {before} -> {after} bytes"
    );
}

// ════════════════════════════════════════════════════════════════════════
// PINS — green now, must stay green.
// ════════════════════════════════════════════════════════════════════════

/// P1 — the rebuild stays deterministic. Ruling D: the answer to a cap that
/// never said which is **not** a fresh draw here, unlike §4.3.5.1. Re-drawing
/// every restart would unseat honest sources for no reason and give an
/// attacker a new chance at every boot instead of none.
#[test]
fn p1_the_same_file_seats_the_same_three() {
    let f = seats("p1", CANDIDATES);
    let order: Vec<usize> = (0..CANDIDATES).collect();
    f.advertise(&order);
    let first = f.seated();
    assert_eq!(
        first.len(),
        AUTOMATIC_REMOTE_CAP,
        "precondition: cap not reached"
    );
    for run in 0..4 {
        assert_eq!(f.seated(), first, "restart {run} chose a different three");
    }
}

/// P2 — spaced arrivals already keep their order today, and must keep it.
/// This is the half of the property that is not broken; a fix that reorders
/// everything would pass R1 and still be wrong.
#[test]
fn p2_spaced_arrivals_already_keep_their_order() {
    let f = seats("p2", CANDIDATES);
    let mut order = f.by_node_id();
    order.reverse();
    for &i in &order {
        let node = serve(&f.dir);
        assert_eq!(
            status_of(&ask_raw(node.port, &f.candidates[i].request)),
            "success"
        );
        drop(node);
        std::thread::sleep(Duration::from_millis(1100));
    }
    let expected: Vec<usize> = {
        let mut v = order[..AUTOMATIC_REMOTE_CAP].to_vec();
        v.sort();
        v
    };
    assert_eq!(
        f.seated(),
        expected,
        "arrivals a second apart no longer keep their order"
    );
}

/// P3 — the store format marker is not bumped. Whatever carries the total
/// order must be additive: `decode_record_line` reads key by key, so a new key
/// is tolerated by older readers. Re-purposing `received_at` to a different
/// unit would not be — an old engine would read milliseconds as seconds.
#[test]
fn p3_the_store_format_marker_is_not_bumped() {
    let f = seats("p3", 1);
    let fmt = f.dir.join(".oo").join("format");
    let before = fs::read_to_string(&fmt).unwrap_or_default();
    assert!(
        !before.trim().is_empty(),
        "harness: `.oo/format` is empty or absent"
    );
    f.advertise(&[0]);
    assert_eq!(
        fs::read_to_string(&fmt).unwrap_or_default(),
        before,
        "`.oo/format` was bumped"
    );
}

/// P4 — a directory written by an older engine still loads and still seats.
/// Records with only second-resolution `received_at` and no ordering key must
/// not be dropped or demoted; there are v0.9.0 workspaces on disk already.
#[test]
fn p4_a_legacy_directory_still_seats() {
    let f = seats("p4", 2);
    f.advertise(&[0, 1]);
    let path = peers_file(&f.dir);
    let text = fs::read_to_string(&path).expect("no durable directory");
    assert!(text.lines().count() >= 2, "harness: nothing was recorded");

    // Strip every key an older engine would not have written, keeping the
    // v0.9.0 shape exactly.
    let legacy: Vec<String> = text
        .lines()
        .map(|line| {
            if !line.trim_start().starts_with('{') {
                return line.to_string();
            }
            let Ok(serde_json::Value::Object(mut o)) = serde_json::from_str(line) else {
                return line.to_string();
            };
            let known = [
                "ad",
                "node_id",
                "public_key",
                "services",
                "listen_port",
                "capacity",
                "ts",
                "ttl",
                "observed_host",
                "hops",
                "received_at",
                "addr",
                "provenance",
            ];
            o.retain(|k, _| known.contains(&k.as_str()));
            serde_json::Value::Object(o).to_string()
        })
        .collect();
    fs::write(&path, legacy.join("\n") + "\n").unwrap();

    let held = f.seated();
    assert_eq!(
        held.len(),
        2,
        "a directory in the older shape lost its sources: {held:?}"
    );
}

/// P5 — eligibility is untouched. An advert with no affiliation claim still
/// never takes a seat, however early it arrives.
#[test]
fn p5_ineligible_sources_still_never_seat() {
    let rng = ring::rand::SystemRandom::new();
    let f = seats("p5", 1);
    let stranger = mint(&rng);
    let peer = spawn_peer();
    let body = format!(
        "{{{{ node_id: \"{}\", public_key: \"{}\", services: [\"{}\"], \
         listen_port: {}, capacity: 10, ts: {}, ttl: 15 }}}}",
        stranger.node_id,
        stranger.pk_hex,
        f.object_caid,
        peer.port,
        now_secs()
    );
    let body_caid = caid_of(&f.dir, &body);
    let sig = hex::encode(
        stranger
            .kp
            .sign(format!("{ADVERT_DOMAIN}{body_caid}").as_bytes())
            .as_ref(),
    );
    let inner = body.trim_start_matches("{{").trim_end_matches("}}").trim();
    let request = format!(
        "{{{{ %op: #advertise, %from: \"{}\", %ad: {{{{ {inner}, signature: \"{sig}\" }}}} }}}}\n",
        stranger.node_id
    );

    let node = serve(&f.dir);
    assert_eq!(status_of(&ask_raw(node.port, &request)), "success");
    assert_eq!(
        status_of(&ask_raw(node.port, &f.candidates[0].request)),
        "success"
    );
    drop(node);

    let before = peer.hits();
    let held = f.seated();
    assert_eq!(
        held,
        vec![0],
        "the eligible candidate lost its seat: {held:?}"
    );
    assert_eq!(
        peer.hits(),
        before,
        "an advert with no affiliation claim was dialled"
    );
}
