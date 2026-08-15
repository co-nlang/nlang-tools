// 哪八個 / which eight — #3c-b1 (2026-07-30).
// Pre-committed by work order: docs/discover_sampling_handover.md
//
// ── The defect ───────────────────────────────────────────────────────────
//
// REAL_02 §4.3.5 caps a `#discover` answer at 8 peers. It never says WHICH
// eight. The engine filled that gap with `for adv in dir.values()` over a
// `HashMap`, so the answer is a function of the per-process hash seed.
//
// Measured at reconnaissance, 20 peers all serving one service:
//
//   answer size ................. 8
//   same answer twice in a row .. YES
//   same answer after restart ... no (2 of 8 overlap)
//
// So it is neither deterministic nor sampled. It is one fixed arbitrary
// permutation per process — which takes the drawbacks of both. The answer
// cannot be accounted for, AND retrying does not help: a client handed eight
// Sybil nodes gets the same eight until the node restarts.
//
// ── Why the fix is sampling and not an ordering ──────────────────────────
//
// The obvious repair is to sort by something. It is the wrong repair, and the
// cost is computable. Any deterministic asker-independent order f(peer,target)
// can be ground: to land 8 identities above N honest ones takes on the order
// of N attempts, and SPEC_15 §7.1 measured key minting at 3,500/second — under
// a second for N=1000. What is bought is a **permanent** seat, because the
// durable peer directory keeps it (§7.1's 2026-07-29 note).
//
// **A deterministic rule is a rule the attacker computes offline too.** The
// defender computes it once per query; the attacker computes it once per
// target and keeps the winning keys forever. That asymmetry is the whole of
// it. Today's nondeterminism is the only thing standing in the way — and it
// is ACCIDENTAL, a side effect of a hash seed nobody chose. An accidental
// defence is not a defence: the first person to fix the flakiness removes it
// without knowing what it was.
//
// Keying the order to the asker (rendezvous hashing) is already forbidden:
// REAL_02 §3.2's 2026-07-28 `#discover` ruling says `%from` is a claim and
// **no decision may depend on it**, because making the answer depend on who
// asks "buys no security and manufactures a partition surface".
//
// ── Why this is not n/ giving up on determinism ──────────────────────────
//
// n/ is a deterministic project — CAID, observation, authority. Sybil cost
// shaping wants the opposite. The tension dissolves once you notice that
// §4.3.5's cap is **already** a truncation: the answer was never the whole
// directory. Presenting one arbitrary truncation with the stability that
// makes it look canonical is the counterfeit. Saying "this is a sample" is
// the honest form, not the dishonest one.
//
// And it is only cost shaping. ORDER_00 §1.1 and SPEC_15 §7.1's own closing
// line stand: no internal mechanism supplies Sybil resistance, the anchor is
// external. This arc claims nothing more.
//
// ── Why `#find_node` must stay deterministic (P7) ────────────────────────
//
// `find_node` sorts by XOR to the target and must keep doing so — Kademlia
// convergence depends on it. The asymmetry has a reason, and the reason is
// the line the whole arc turns on: **`find_node`'s answer is checkable by
// the asker** (they can compute the distances themselves and see that the
// peers really are near), while `#discover`'s is not — a hit means only
// "someone claims to serve this" (§4.3.2). Determinism is right where the
// answer can be checked and counterfeit where it cannot.
//
// ── The numbers these gates are set from (simulated, 20k trials) ─────────
//
//   N=20, k=8:  P(two draws give the same set)      = 1/C(20,8) = 7.9e-6
//               P(4 draws all identical to the 1st) = 5e-16
//               queries to cover all 20: median 7, p99.9 20, max 26
//   N=12, k=8:  P(two draws identical) = 2.0e-3  ← why N is 20 and not 12
//
// R2's cap of 200 is set from the analytic tail, not from that max: the
// chance a given candidate is missed in M draws is (1-8/20)^M, so a union
// bound over 20 candidates at M=200 is ~1e-40. Standing rule, learned on the
// kademlia arc: when an assertion about a draw becomes a loop, the loop's
// guard is a new number and needs its own measurement. This is that
// measurement, and the guard is deliberately not tied to N.

use std::collections::HashSet;
mod common;

use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use nlang_interpreter::value::Identity;
use ring::signature::{Ed25519KeyPair, KeyPair};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

const ADVERT_DOMAIN: &str = "oodp-advert:v1:";
/// REAL_02 §4.3.5.
const MAX_DISCOVER_PEERS: usize = 8;
/// Fixture size. Not 12 — see the header: at 12 two draws collide once in 500.
const N_PEERS: usize = 20;
/// R1: four queries. P(union still 8 under uniform sampling) ≈ 5e-16.
const R1_QUERIES: usize = 4;
/// R2: coverage guard, set from the analytic tail (~1e-40), not from N.
const R2_MAX_QUERIES: usize = 200;

// ── harness ─────────────────────────────────────────────────────────────

fn fresh_dir(tag: &str) -> nlang_interpreter::ScratchDir {
    nlang_interpreter::ScratchDir::new(&format!("sampling-{tag}"))
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

fn service_caid(dir: &Path, tag: &str) -> String {
    caid_of(dir, &format!("{{{{ svc: \"{tag}\" }}}}"))
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
    fn stop(mut self) {
        self.child.kill().ok();
        self.child.wait().ok();
    }
}

fn serve(dir: &Path) -> Node {
    let served = common::serve(oo_cmd(dir), dir.join("serve.log"));
    Node { child: served.child, port: served.port, log: served.log }
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
        .map(|s| s.trim().trim_start_matches('#').to_string())
        .unwrap_or_else(|| format!("<no %status in {}>", reply.trim()))
}

/// `node_id`s named in a `#discover` answer, read out of the verbatim `%ad`.
fn answer_ids(reply: &str) -> Vec<String> {
    let Ok(j) = serde_json::from_str::<serde_json::Value>(reply.trim()) else {
        return vec![];
    };
    let Some(arr) = j.get("%peers").and_then(|v| v.as_array()) else {
        return vec![];
    };
    arr.iter()
        .filter_map(|e| {
            let ad = e.get("%ad").and_then(|v| v.as_str())?;
            Some(
                ad.split("node_id: \"")
                    .nth(1)?
                    .split('"')
                    .next()?
                    .to_string(),
            )
        })
        .collect()
}

// ── fixture ─────────────────────────────────────────────────────────────

struct Peer {
    kp: Ed25519KeyPair,
    pk_hex: String,
    node_id: String,
    port: u16,
}

fn mint(rng: &ring::rand::SystemRandom, port: u16) -> Peer {
    let pkcs8 = Ed25519KeyPair::generate_pkcs8(rng).unwrap();
    let kp = Ed25519KeyPair::from_pkcs8(pkcs8.as_ref()).unwrap();
    let pk = kp.public_key().as_ref().to_vec();
    let node_id = Identity {
        public_key: pk.clone(),
        private_key: Vec::new(),
    }
    .node_id_caid()
    .to_string();
    Peer {
        pk_hex: hex::encode(&pk),
        node_id,
        kp,
        port,
    }
}

impl Peer {
    fn request(&self, dir: &Path, svc: &[&str], capacity: i64, ttl: i64, ts: i64) -> String {
        let s = svc
            .iter()
            .map(|x| format!("\"{x}\""))
            .collect::<Vec<_>>()
            .join(", ");
        let body = format!(
            "{{{{ node_id: \"{}\", public_key: \"{}\", services: [{s}], \
             listen_port: {}, capacity: {capacity}, ts: {ts}, ttl: {ttl} }}}}",
            self.node_id, self.pk_hex, self.port
        );
        let caid = caid_of(dir, &body);
        let sig = hex::encode(
            self.kp
                .sign(format!("{ADVERT_DOMAIN}{caid}").as_bytes())
                .as_ref(),
        );
        let inner = body.trim_start_matches("{{").trim_end_matches("}}").trim();
        format!(
            "{{{{ %op: #advertise, %from: \"{}\", %ad: {{{{ {inner}, signature: \"{sig}\" }}}} }}}}\n",
            self.node_id
        )
    }
}

/// `n` peers all advertising `svc`. Returns their node ids.
fn populate(node: &Node, dir: &Path, svc: &str, n: usize, base_port: u16) -> Vec<String> {
    let rng = ring::rand::SystemRandom::new();
    let mut ids = Vec::new();
    for i in 0..n {
        let p = mint(&rng, base_port + i as u16);
        let r = ask_raw(node.port, &p.request(dir, &[svc], 10, 15, now_secs()));
        assert_eq!(status_of(&r), "success", "fixture advert {i} refused: {r}");
        ids.push(p.node_id);
    }
    ids
}

fn discover(port: u16, asker: &str, target: &str) -> String {
    ask_raw(
        port,
        &format!("{{{{ %op: #discover, %from: \"{asker}\", %target: \"{target}\" }}}}\n"),
    )
}

// ════════════════════════════════════════════════════════════════════════
// CONTROLS — green before and after.
// ════════════════════════════════════════════════════════════════════════

/// C1 — the harness reaches a live index: one advertiser, one hit.
#[test]
fn c1_the_index_answers_at_all() {
    let rng = ring::rand::SystemRandom::new();
    let dir = fresh_dir("c1");
    init(&dir);
    let svc = service_caid(&dir, "c1");
    let node = serve(&dir);
    let p = mint(&rng, 33001);
    assert_eq!(
        status_of(&ask_raw(
            node.port,
            &p.request(&dir, &[&svc], 10, 15, now_secs())
        )),
        "success"
    );
    let ids = answer_ids(&discover(node.port, "x", &svc));
    assert_eq!(
        ids,
        vec![p.node_id],
        "one advertiser must produce exactly one hit: {ids:?}"
    );
}

/// C2 — the fixture really does exceed the cap, or every gate below is empty.
#[test]
fn c2_the_fixture_exceeds_the_cap() {
    let dir = fresh_dir("c2");
    init(&dir);
    let svc = service_caid(&dir, "c2");
    let node = serve(&dir);
    let ids = populate(&node, &dir, &svc, N_PEERS, 33100);
    assert_eq!(ids.len(), N_PEERS);
    assert!(
        N_PEERS > MAX_DISCOVER_PEERS,
        "fixture must exceed the cap to test selection"
    );
    let a = answer_ids(&discover(node.port, "x", &svc));
    assert_eq!(
        a.len(),
        MAX_DISCOVER_PEERS,
        "with {N_PEERS} candidates the answer must be capped at {MAX_DISCOVER_PEERS}: {a:?}"
    );
}

// ════════════════════════════════════════════════════════════════════════
// REDS — `#[ignore]` until delivery. Delivery removes ONLY the attribute.
// ════════════════════════════════════════════════════════════════════════

/// R1 — retrying can surface a peer the first answer did not name, **without
/// restarting the node**. This is the property the whole arc buys: a client
/// handed eight Sybils must be able to route around them by asking again.
///
/// Today the answer is one fixed permutation per process, so the union over
/// any number of queries is exactly 8, and the only way to see anything else
/// is to restart the server. Under uniform sampling the union exceeds 8 after
/// four queries with probability 1 − 5e-16.
#[test]
fn r1_asking_again_can_return_someone_else() {
    let dir = fresh_dir("r1");
    init(&dir);
    let svc = service_caid(&dir, "r1");
    let node = serve(&dir);
    let ids = populate(&node, &dir, &svc, N_PEERS, 33200);

    let mut union: HashSet<String> = HashSet::new();
    let mut first: Option<Vec<String>> = None;
    for q in 0..R1_QUERIES {
        let a = answer_ids(&discover(node.port, "x", &svc));
        assert_eq!(
            a.len(),
            MAX_DISCOVER_PEERS,
            "query {q} did not return a full answer, so the union below means nothing: {a:?}"
        );
        if first.is_none() {
            first = Some(a.clone());
        }
        union.extend(a);
    }
    // Every id returned must be one we actually advertised — otherwise the
    // union could grow by invention rather than by sampling.
    for id in &union {
        assert!(
            ids.contains(id),
            "answer named a peer that was never advertised: {id}"
        );
    }
    assert!(
        union.len() > MAX_DISCOVER_PEERS,
        "{R1_QUERIES} queries to one running node returned the same {} peers every \
         time (union {}). The answer is a fixed permutation, so a client cannot \
         retry past a bad set without restarting the server.\nfirst answer: {:?}",
        MAX_DISCOVER_PEERS,
        union.len(),
        first.unwrap()
    );
}

/// R2 — no eligible candidate is permanently unreachable. A peer that passes
/// every §4.3.2 filter must be capable of appearing in *some* answer; today
/// the twelve outside the process permutation never appear at all.
///
/// The guard is 200 queries, from the analytic tail (a given candidate is
/// missed in M draws with probability (1−8/20)^M; union bound over 20 at
/// M=200 is ~1e-40). It is deliberately not a multiple of N: the kademlia
/// arc shipped a loop whose guard was tied to an unrelated n and flaked 5.3%
/// of the time.
#[test]
fn r2_every_eligible_peer_can_be_reached() {
    let dir = fresh_dir("r2");
    init(&dir);
    let svc = service_caid(&dir, "r2");
    let node = serve(&dir);
    let ids = populate(&node, &dir, &svc, N_PEERS, 33300);
    let all: HashSet<String> = ids.iter().cloned().collect();

    let mut seen: HashSet<String> = HashSet::new();
    let mut queries = 0usize;
    while seen.len() < all.len() && queries < R2_MAX_QUERIES {
        queries += 1;
        seen.extend(answer_ids(&discover(node.port, "x", &svc)));
    }
    let missing: Vec<&String> = all.difference(&seen).collect();
    assert!(
        missing.is_empty(),
        "after {queries} queries only {} of {} eligible peers had ever been \
         named; {} can never be discovered from this node while it stays up: {:?}",
        seen.len(),
        all.len(),
        missing.len(),
        missing
    );
}

// ════════════════════════════════════════════════════════════════════════
// PINS — green now, must stay green.
// ════════════════════════════════════════════════════════════════════════

/// P1 — when everything fits, sampling changes nothing: all candidates come
/// back, every time. Selection may only ever be about the overflow.
#[test]
fn p1_under_the_cap_the_answer_is_everyone_every_time() {
    let dir = fresh_dir("p1");
    init(&dir);
    let svc = service_caid(&dir, "p1");
    let node = serve(&dir);
    let ids = populate(&node, &dir, &svc, MAX_DISCOVER_PEERS - 3, 33400);
    let all: HashSet<String> = ids.iter().cloned().collect();
    for q in 0..6 {
        let a: HashSet<String> = answer_ids(&discover(node.port, "x", &svc))
            .into_iter()
            .collect();
        assert_eq!(
            a, all,
            "query {q} dropped or invented a peer when all of them fit"
        );
    }
}

/// P2 — `capacity` is not a weight. §4.2.4 makes it an unverifiable claim, so
/// letting it bias selection would move the incentive to lie from ordering
/// into sampling.
///
/// This is a PIN and not a red on purpose: today's fixed permutation is
/// already unrelated to capacity, so it passes now for a reason that has
/// nothing to do with the arc. Written as a red it would have been green at
/// baseline about 99.96% of the time — the trap the affiliation arc's R5 fell
/// into, caught here at design time instead.
#[test]
fn p2_capacity_does_not_bias_selection() {
    let rng = ring::rand::SystemRandom::new();
    let dir = fresh_dir("p2");
    init(&dir);
    let svc = service_caid(&dir, "p2");
    let node = serve(&dir);

    let mut humble: HashSet<String> = HashSet::new();
    for i in 0..10u16 {
        let p = mint(&rng, 33500 + i);
        assert_eq!(
            status_of(&ask_raw(
                node.port,
                &p.request(&dir, &[&svc], 1, 15, now_secs())
            )),
            "success"
        );
        humble.insert(p.node_id);
    }
    for i in 0..10u16 {
        let p = mint(&rng, 33600 + i);
        let r = ask_raw(
            node.port,
            &p.request(&dir, &[&svc], 1_000_000, 15, now_secs()),
        );
        assert_eq!(status_of(&r), "success");
    }

    // Under any capacity-blind rule a humble peer shows up almost at once.
    // P(8 draws all from the boastful ten) = C(10,8)/C(20,8) = 3.6e-4 per
    // query, so over eight queries a false red is ~1e-27.
    let mut saw_humble = false;
    for _ in 0..8 {
        if answer_ids(&discover(node.port, "x", &svc))
            .iter()
            .any(|id| humble.contains(id))
        {
            saw_humble = true;
            break;
        }
    }
    assert!(
        saw_humble,
        "eight answers named only peers claiming capacity 1,000,000 — selection \
         is weighted by a number §4.2.4 says nobody can verify"
    );
}

/// P3 — exclusion still happens before selection (§4.3.2). `ttl == 0` means
/// "do not relay me" and a stale advert is out; neither may ever appear.
///
/// Sampling makes this pin *stronger* than it could be before: absence can now
/// be confirmed across many draws rather than inferred from one.
#[test]
fn p3_excluded_peers_never_appear_however_often_you_ask() {
    let rng = ring::rand::SystemRandom::new();
    let dir = fresh_dir("p3");
    init(&dir);
    let svc = service_caid(&dir, "p3");
    let node = serve(&dir);

    let eligible = populate(&node, &dir, &svc, 6, 33700);
    assert!(
        !eligible.is_empty(),
        "harness: no eligible peers, absence proves nothing"
    );

    let no_relay = mint(&rng, 33800);
    assert_eq!(
        status_of(&ask_raw(
            node.port,
            &no_relay.request(&dir, &[&svc], 10, 0, now_secs())
        )),
        "success",
        "a ttl=0 advert must still be accepted; it is relaying that is refused"
    );

    let mut seen: HashSet<String> = HashSet::new();
    for _ in 0..40 {
        seen.extend(answer_ids(&discover(node.port, "x", &svc)));
    }
    assert!(
        seen.iter().any(|id| eligible.contains(id)),
        "40 queries named nobody at all, so the absence below proves nothing"
    );
    assert!(
        !seen.contains(&no_relay.node_id),
        "a ttl=0 advert was relayed; exclusion must precede selection (§4.3.2)"
    );
}

/// P4 — the cap itself is unchanged (§4.3.5), however many candidates there are.
#[test]
fn p4_the_cap_holds() {
    let dir = fresh_dir("p4");
    init(&dir);
    let svc = service_caid(&dir, "p4");
    let node = serve(&dir);
    populate(&node, &dir, &svc, N_PEERS, 33900);
    for q in 0..5 {
        let a = answer_ids(&discover(node.port, "x", &svc));
        assert!(
            a.len() <= MAX_DISCOVER_PEERS,
            "query {q} returned {} peers, cap is {MAX_DISCOVER_PEERS}",
            a.len()
        );
    }
}

/// P5 — an unknown target is still `#success` with an empty list, not
/// `#not_found` (§4.3.3). Selection must not turn "nobody" into an error.
#[test]
fn p5_no_hits_is_still_success_with_an_empty_list() {
    let dir = fresh_dir("p5");
    init(&dir);
    let known = service_caid(&dir, "p5-known");
    let unknown = service_caid(&dir, "p5-unknown");
    let node = serve(&dir);
    populate(&node, &dir, &known, 3, 34000);
    let r = discover(node.port, "x", &unknown);
    assert_eq!(
        status_of(&r),
        "success",
        "an unknown target must not be an error: {r}"
    );
    assert!(
        answer_ids(&r).is_empty(),
        "an unknown target named peers: {r}"
    );
}

/// P6 — `%from` still decides nothing (REAL_02 §3.2, 2026-07-28 ruling). Two
/// different askers must not receive systematically different answers; making
/// the answer depend on who asks was ruled out as a partition surface that
/// buys nothing, and sampling must not smuggle it back in.
///
/// Stated as coverage rather than equality, because under sampling two answers
/// differ anyway: what must hold is that neither asker is confined to a subset.
#[test]
fn p6_the_asker_does_not_change_what_is_reachable() {
    let dir = fresh_dir("p6");
    init(&dir);
    let svc = service_caid(&dir, "p6");
    let node = serve(&dir);
    let ids = populate(&node, &dir, &svc, N_PEERS, 34100);
    let all: HashSet<String> = ids.iter().cloned().collect();

    let mut reach = |asker: &str| -> HashSet<String> {
        let mut seen = HashSet::new();
        for _ in 0..R2_MAX_QUERIES {
            seen.extend(answer_ids(&discover(node.port, asker, &svc)));
            if seen.len() == all.len() {
                break;
            }
        }
        seen
    };
    let a = reach("hash:sha256:v2:_:aaaa:1111");
    let b = reach("hash:sha256:v2:_:bbbb:2222");
    assert!(
        !a.is_empty() && !b.is_empty(),
        "harness: neither asker saw anything"
    );
    assert_eq!(
        a, b,
        "two askers reach different peer sets — the answer depends on `%from`, \
         which §3.2's 2026-07-28 ruling forbids"
    );
}

/// P7 — `#find_node` stays deterministic. Kademlia convergence depends on the
/// XOR order, and unlike `#discover` its answer is **checkable by the asker**:
/// they can compute the distances and see the peers really are nearest. That
/// is the line — determinism belongs where the answer can be checked. This pin
/// exists so nobody unifies the two ops on the grounds that they look alike.
#[test]
fn p7_find_node_is_still_deterministic() {
    let dir = fresh_dir("p7");
    init(&dir);
    let svc = service_caid(&dir, "p7");
    let node = serve(&dir);
    populate(&node, &dir, &svc, N_PEERS, 34200);

    let target = "b".repeat(40);
    let q = format!("{{{{ %op: #find_node, %from: \"x\", %target: \"{target}\" }}}}\n");
    let first = ask_raw(node.port, &q);
    assert_eq!(
        status_of(&first),
        "success",
        "find_node did not answer: {first}"
    );
    // Calibration: `find_node` shares `encode_discover_response` with
    // `#discover`, so it is the same `%peers: [{%ad, …}]` shape. The first
    // version of this pin invented a second parser and its own guard caught it.
    let base = answer_ids(&first);
    assert!(
        !base.is_empty(),
        "harness: find_node named nobody, so equality proves nothing"
    );
    for q_i in 0..6 {
        assert_eq!(
            answer_ids(&ask_raw(node.port, &q)),
            base,
            "find_node answer {q_i} differed — its order is load bearing for lookup \
             convergence and must not be sampled"
        );
    }
}
