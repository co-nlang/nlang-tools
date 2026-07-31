// 歸屬聲明 / affiliation claim — #3c-a (2026-07-30).
// Pre-committed by work order: docs/affiliation_claim_handover.md
//
// ── What this arc is ─────────────────────────────────────────────────────
//
// An operator runs several machines. Each is an independent stranger to
// everyone else: REAL_02 §4.1.1 gives every workspace its own node key, and
// REAL_02 §220 forbids that key from being the operator's. Today there is no
// way for the operator to say "these are mine", and no way to take it back.
//
// An affiliation claim is a signed statement by an operator that a node is
// theirs. It rides inside the advert body, so the node signs it too.
//
// ── The six rulings this file encodes (approved 2026-07-30) ──────────────
//
// 1. NAME. "歸屬聲明 / affiliation claim". Not 委任 (ORDER_01 §7.4 already
//    means delegating administrative authority over a subpath) and not 背書
//    (SPEC_13 §179 already means a governance authority vouching for the
//    correctness of a CAID). REAL_01 §7.6 says a word that spans two
//    questions makes the spec contradict itself a few months later.
//    "聲明" is load-bearing: §7.6.1 — a claim obliges the RECEIVER to judge.
//
// 2. WHAT IT BUYS. Cross-machine trust aggregation and revocation. NOT Sybil
//    resistance (§7.6 row 5 — that needs an external physical anchor). An
//    attacker simply does not sign. The mechanism constrains only those who
//    opt in, so what it buys is Sybil LEGIBILITY for the honest operator who
//    chooses to disclose.
//
// 3. ADDITIVE ONLY. A claim is degree ≥1 (asserted, disc 025). It must never
//    cause anything to be accepted that would otherwise be refused. CAID
//    verification stays unconditional. A broken claim does NOT reject the
//    advert — the node's own signature is still valid, so who it is was never
//    in doubt; only the affiliation is unproven.
//
// 4. REVOCATION = SHORT LIFETIME + RENEWAL, no revocation list. OODP has no
//    distribution channel for one, and ORDER_01 §7.4's own instinct is auto-
//    expiry. Prior art, and the reason this is not merely convenient: the
//    CA/Browser Forum has been cutting maximum certificate lifetimes for years
//    (825 → 398 days, and further) precisely BECAUSE revocation does not work
//    in practice — CRL/OCSP are unreliable and fail soft. People who HAVE a
//    revocation channel are moving to short lifetimes anyway.
//
// 5. THREE PATHS, ALL OR NOTHING. A claim arrives three ways: direct
//    #advertise, relayed inside a #discover answer, and loaded from
//    `.oo/peers/directory` at startup. Verifying on some and not others
//    recreates exactly the defect v0.2.54's acceptance repaired: a signature
//    record nobody checks is not a signature record.
//
// 6. TRUST ROOT IS A THIRD LIST, IN THE ASSERTION LAYER. SPEC_13 §4.1.2
//    obligation #3 already rules this: "信任配置不是宇宙內容 … 建築師白名單、
//    黑名單、對等點清單屬斷言層, 經帶外通道供給". So it lives beside
//    `~/.oo/authorized_keys`, never in `~%Official`, and it is distinct from
//    `architect_registry` (governance) and REAL_02 §6.2's root-of-trust
//    (package blacklists, still unimplemented). Three lists is the honest
//    answer because they answer three different questions — but it must be
//    written down, not drifted into. THAT LIST IS #3c-b, NOT THIS ARC.
//
// ── Measured at reconnaissance, 2026-07-30, on v0.3.0 ────────────────────
//
//   * A nested unknown combo in the advert body is accepted (#success), and
//     so is one whose inner signature actually computes. `required` in
//     oodp.rs is a presence check, not an exhaustive one, and unknown fields
//     flow into the body CAID that both sides sign. THIS ARC IS INCREMENTAL,
//     measured live after #3b moved the CAID — not inferred.
//   * The claim already travels: a #discover answer carries `%ad` verbatim,
//     operator signature and all. And it is already persisted: the durable
//     record stores `ad_source`, the verbatim `%ad`. Propagation and
//     durability cost this arc nothing — v0.2.54 already built them.
//   * Therefore the verified operator is a DERIVED fact and, per v0.2.54's
//     ruling that derived state is rebuilt at load rather than stored, IT IS
//     NOT PERSISTED. The durable format does not change at all. Adding a key
//     would be tolerated anyway (`decode_record_line` reads key by key) —
//     that is why P6 is a real pin and not a countdown timer.
//   * Cost: +265 bytes per advert (462 → 727, +57%). MAX_DISCOVER_PEERS = 8
//     binds long before the 64 KiB response budget, so the budget is not a
//     constraint. v0.2.54's directory measurement moves: 150 adverts,
//     131,568 B → ≈206,000 B, still far under R6's 1 MB bound.
//
// ── Why `oo node peers` is in scope and is not scope creep ───────────────
//
// `oo node` today is serve / id / advertise / discover / find-node. There is
// no way to observe what the node believes about a peer. Without a surface,
// every red here would be red only because a command is missing, and the
// calibration would be theatre. The satisfiability check on the work order
// (standing rule) is what put this in: an arc whose result cannot be observed
// is not deliverable. R1 and R2 deliberately do NOT use it, so that the reds
// are not a monoculture that a stub could turn green all at once.
//
// ── The wire shape is pinned literally, and that is deliberate ───────────
//
// The standing rule is "pin the property, not one spelling" (P4 of the
// persistence arc, which pinned `.oo/routing/` and was satisfied by renaming
// a file). A WIRE format is the exception: interoperability is a claim about
// bytes, and a second implementation reading the spec must produce the same
// ones. So the field names, the domain string and the signed payload below
// are normative, and the probe asserts them literally.

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use nlang_interpreter::value::Identity;
use ring::signature::{self as rsig, Ed25519KeyPair, KeyPair, UnparsedPublicKey};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

// ── normative constants (work order §2) ─────────────────────────────────

/// Existing advert domain. Unchanged by this arc.
const ADVERT_DOMAIN: &str = "oodp-advert:v1:";

/// New. The third signature domain in the engine, after `oodp-advert:v1:`
/// (node key) and `refine:` (operator key, authority.rs). Carries `:v1:`
/// because `refine:` not carrying one is a defect we do not repeat.
const AFFILIATION_DOMAIN: &str = "oodp-affiliation:v1:";

/// Ruling 4 has no teeth if any expiry is accepted, so there is a ceiling.
/// 30 days: long enough that renewal is a monthly chore, short enough that a
/// compromised node ages out without a revocation channel. Same style as
/// `STALE_SKEW_SECS` / `DISCOVER_STALE_SECS` in oodp.rs.
const MAX_AFFILIATION_LIFETIME_SECS: i64 = 30 * 24 * 3600;

/// The signed payload. Binds the claim to BOTH the node and the expiry:
///   * without `node_id`, the claim is transferable to any node (R6)
///   * without `expires`, the holder extends it themselves (R5)
fn affiliation_payload(node_id: &str, expires: i64) -> String {
    format!("{AFFILIATION_DOMAIN}{node_id}:{expires}")
}

// ── harness ─────────────────────────────────────────────────────────────

fn fresh_dir(tag: &str) -> PathBuf {
    let mut d = std::env::temp_dir();
    d.push(format!(
        "nlang-affil-{}-{}-{}",
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

/// `oo node id` prints the bare CAID on line one and `path: …` on line two —
/// not a quoted atom. Calibration caught `first_string` choking on it.
fn node_id_of(dir: &Path) -> String {
    let out = oo(dir, &["node", "id"]);
    out.split_whitespace()
        .find(|t| t.starts_with("hash:sha256:"))
        .unwrap_or_else(|| panic!("`oo node id` printed no CAID: {out}"))
        .to_string()
}

fn identity_path(dir: &Path) -> PathBuf {
    dir.join("identity-for-tests")
}

fn peers_file(dir: &Path) -> PathBuf {
    dir.join(".oo").join("peers").join("directory")
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

/// Signs the way the engine verifies: through `~%Discovery./identify`.
fn caid_of(dir: &Path, expr: &str) -> String {
    let out = oo(dir, &["eval", &format!("~%Discovery./identify {expr}")]);
    let caid = first_string(&out);
    assert!(caid.starts_with("hash:sha256:"), "caid_of() got {caid:?}");
    caid
}

/// `run` alone writes no objects — measured at calibration. A store only has
/// content after `evolve` + `commit`, and pins that compare object counts or
/// universe roots need one, or they compare nothing to nothing.
fn commit_something(dir: &Path, tag: &str) -> String {
    let f = format!("{tag}.n");
    fs::write(dir.join(&f), format!("{tag}: {{ anchor: 1 }}\n")).unwrap();
    oo(dir, &["run", &f]);
    oo(dir, &["evolve", &f]);
    oo(dir, &["commit", "-m", tag])
}

fn object_count(dir: &Path) -> usize {
    fn walk(p: &Path, n: &mut usize) {
        if let Ok(rd) = fs::read_dir(p) {
            for e in rd.flatten() {
                let path = e.path();
                if path.is_dir() {
                    walk(&path, n);
                } else {
                    *n += 1;
                }
            }
        }
    }
    let mut n = 0;
    walk(&dir.join(".oo").join("objects"), &mut n);
    n
}

fn free_port() -> u16 {
    for _ in 0..64 {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let p = l.local_addr().unwrap().port();
        if p > 23000 {
            return p;
        }
    }
    panic!("no free port above 23000");
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

fn json_of(reply: &str) -> Option<serde_json::Value> {
    serde_json::from_str(reply.trim()).ok()
}

fn status_of(reply: &str) -> String {
    json_of(reply)
        .and_then(|j| {
            j.get("%status")
                .and_then(|v| v.as_str())
                .map(str::to_string)
        })
        .map(|s| s.trim().trim_start_matches('#').to_string())
        .unwrap_or_else(|| format!("<no %status in {}>", reply.trim()))
}

/// A stub peer that accepts one line, answers `#success`, and hands the raw
/// request back. Lets R2 watch what `oo node advertise` actually puts on the
/// wire without depending on any new observation command.
fn stub_peer() -> (u16, mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for stream in listener.incoming().take(8) {
            let Ok(mut s) = stream else { continue };
            let mut line = String::new();
            let mut r = BufReader::new(s.try_clone().unwrap());
            if r.read_line(&mut line).is_err() {
                continue;
            }
            let _ = s.write_all(b"{\"%status\": \"#success\", \"%hops\": 0}\n");
            let _ = s.flush();
            let _ = tx.send(line);
        }
    });
    (port, rx)
}

// ── keys ────────────────────────────────────────────────────────────────

struct Key {
    kp: Ed25519KeyPair,
    pk_hex: String,
    node_id: String,
}

fn mint_key(rng: &ring::rand::SystemRandom) -> Key {
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

fn verify_ed25519(pk_hex: &str, payload: &str, sig_hex: &str) -> bool {
    let (Ok(pk), Ok(sig)) = (hex::decode(pk_hex), hex::decode(sig_hex)) else {
        return false;
    };
    UnparsedPublicKey::new(&rsig::ED25519, pk)
        .verify(payload.as_bytes(), &sig)
        .is_ok()
}

// ── advert construction ─────────────────────────────────────────────────

/// The normative claim shape. `expires` is seconds since the epoch.
fn claim_block(operator: &Key, node_id: &str, expires: i64) -> String {
    let sig = hex::encode(
        operator
            .kp
            .sign(affiliation_payload(node_id, expires).as_bytes())
            .as_ref(),
    );
    format!(
        ", affiliation: {{{{ operator_key: \"{}\", signature: \"{}\", expires: {} }}}}",
        operator.pk_hex, sig, expires
    )
}

struct Advert {
    node: Key,
    port: u16,
}

impl Advert {
    fn new(node: Key, port: u16) -> Self {
        Advert { node, port }
    }

    fn body(&self, svc: &[&str], ts: i64, extra: &str) -> String {
        let s = svc
            .iter()
            .map(|x| format!("\"{x}\""))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "{{{{ node_id: \"{}\", public_key: \"{}\", services: [{s}], \
             listen_port: {}, capacity: 10, ts: {ts}, ttl: 15{extra} }}}}",
            self.node.node_id, self.node.pk_hex, self.port
        )
    }

    /// Node-signs the body, claim included. The node always signs whatever
    /// affiliation block is present — that is what makes a broken claim the
    /// node's own doing rather than a third party's (ruling 3).
    fn signed(&self, caid_dir: &Path, svc: &[&str], extra: &str) -> String {
        let body = self.body(svc, now_secs(), extra);
        let caid = caid_of(caid_dir, &body);
        let sig = hex::encode(
            self.node
                .kp
                .sign(format!("{ADVERT_DOMAIN}{caid}").as_bytes())
                .as_ref(),
        );
        let inner = body.trim_start_matches("{{").trim_end_matches("}}").trim();
        format!("{{{{ {inner}, signature: \"{sig}\" }}}}")
    }

    fn request(&self, caid_dir: &Path, svc: &[&str], extra: &str) -> String {
        format!(
            "{{{{ %op: #advertise, %from: \"{}\", %ad: {} }}}}\n",
            self.node.node_id,
            self.signed(caid_dir, svc, extra)
        )
    }
}

fn service_caid(dir: &Path, tag: &str) -> String {
    caid_of(dir, &format!("{{{{ svc: \"{tag}\" }}}}"))
}

// ── the observation surface (delivered by this arc) ─────────────────────
//
// `oo node peers` must report, for each known peer, its node id and either
// the operator key of a VERIFIED affiliation or nothing. The probe reads it
// as lines and looks for the two hex strings; it deliberately does not pin a
// column layout, only that both facts are recoverable and that an unverified
// claim contributes NO operator string anywhere in the output.

fn peers_output(dir: &Path) -> String {
    oo(dir, &["node", "peers"])
}

fn peers_lists_node(out: &str, node_id: &str) -> bool {
    // node ids are long; match on the digest tail, which is unambiguous.
    let tail = node_id.rsplit(':').next().unwrap();
    out.contains(tail)
}

fn peers_shows_operator(out: &str, operator_pk_hex: &str) -> bool {
    out.contains(operator_pk_hex)
}

// ════════════════════════════════════════════════════════════════════════
// CONTROLS — green at baseline AND after. If one of these is red, every
// verdict below is void. Standing rule: a whole-set-scanning probe leads
// with a control, because a silently broken harness makes every red pass.
// ════════════════════════════════════════════════════════════════════════

/// C1 — the fixture is armed: the probe can mint an operator key, sign a
/// claim over the normative payload, and verify it. A counter-case must fail.
#[test]
fn c1_the_claim_fixture_actually_signs_and_verifies() {
    let rng = ring::rand::SystemRandom::new();
    let op = mint_key(&rng);
    let node = mint_key(&rng);
    let expires = now_secs() + 3600;

    let payload = affiliation_payload(&node.node_id, expires);
    let sig = hex::encode(op.kp.sign(payload.as_bytes()).as_ref());

    assert!(
        verify_ed25519(&op.pk_hex, &payload, &sig),
        "harness cannot verify a signature it just made"
    );
    // Counter-case: the same signature over a different node must NOT verify,
    // otherwise R6 would pass for free.
    let other = mint_key(&rng);
    assert!(
        !verify_ed25519(
            &op.pk_hex,
            &affiliation_payload(&other.node_id, expires),
            &sig
        ),
        "a claim for one node verified for another — R6 would be vacuous"
    );
    // Counter-case: same node, different expiry, otherwise R5 would be vacuous.
    assert!(
        !verify_ed25519(
            &op.pk_hex,
            &affiliation_payload(&node.node_id, expires + 1),
            &sig
        ),
        "a claim verified under a different expiry — R5 would be vacuous"
    );
}

/// C2 — the harness reaches a live engine: a plain advert is accepted.
#[test]
fn c2_a_plain_advert_is_accepted() {
    let rng = ring::rand::SystemRandom::new();
    let dir = fresh_dir("c2");
    init(&dir);
    let node = serve(&dir);
    let ad = Advert::new(mint_key(&rng), 24001);
    let r = ask_raw(node.port, &ad.request(&dir, &[], ""));
    assert_eq!(status_of(&r), "success", "control advert refused: {r}");
}

// ════════════════════════════════════════════════════════════════════════
// REDS — `#[ignore]` until delivery. Delivery removes ONLY the attribute.
// ════════════════════════════════════════════════════════════════════════

/// R1 — minting exists, and the key it signs with is the key `oo identity`
/// reports. REAL_01 §7.5.2 (可宣告性): reporting X while signing with Y makes
/// the operator publish a key that never signs, and the failure surfaces much
/// later as "signer not in the registry", pointing at the wrong cause.
///
/// Does not use `oo node peers` — so the reds are not a monoculture.
#[test]
fn r1_minting_signs_with_the_key_that_oo_identity_reports() {
    let dir = fresh_dir("r1");
    init(&dir);

    let node_id = node_id_of(&dir);
    let ident = oo(&dir, &["identity"]);
    let operator_pk = ident
        .split_whitespace()
        .find(|t| t.len() == 64 && t.chars().all(|c| c.is_ascii_hexdigit()))
        .unwrap_or_else(|| panic!("`oo identity` printed no 64-hex key: {ident}"))
        .to_string();

    let out = oo(&dir, &["node", "affiliate"]);
    let sig = out
        .split_whitespace()
        .find(|t| t.len() == 128 && t.chars().all(|c| c.is_ascii_hexdigit()))
        .unwrap_or_else(|| panic!("`oo node affiliate` printed no 128-hex signature: {out}"))
        .to_string();
    let expires: i64 = out
        .split_whitespace()
        .filter_map(|t| {
            t.trim_matches(|c: char| !c.is_ascii_digit())
                .parse::<i64>()
                .ok()
        })
        .find(|n| *n > now_secs() && *n <= now_secs() + MAX_AFFILIATION_LIFETIME_SECS)
        .unwrap_or_else(|| panic!("`oo node affiliate` printed no plausible expiry: {out}"));

    assert!(
        verify_ed25519(&operator_pk, &affiliation_payload(&node_id, expires), &sig),
        "the minted claim does not verify against the key `oo identity` reports\n\
         operator={operator_pk}\n node={node_id}\n expires={expires}\n out={out}"
    );
}

/// R2 — the claim rides the advert. Watched on the wire through a stub peer,
/// so this red is independent of the new observation command too.
#[test]
fn r2_the_claim_rides_the_advert() {
    let dir = fresh_dir("r2");
    init(&dir);
    oo(&dir, &["node", "affiliate"]);

    let (port, rx) = stub_peer();
    let out = oo(
        &dir,
        &["node", "advertise", "--to", &format!("127.0.0.1:{port}")],
    );
    let seen = rx
        .recv_timeout(Duration::from_secs(10))
        .unwrap_or_else(|e| panic!("stub peer saw no request ({e:?}); cli said: {out}"));

    assert!(
        seen.contains("affiliation") && seen.contains("operator_key"),
        "the advert carried no affiliation block after minting one:\n{seen}"
    );
}

/// R3 — the direct #advertise path verifies a good claim and the node can say
/// so. The differential is what makes this real: two peers, one with a claim
/// that computes and one with none, BOTH accepted and BOTH listed, and the
/// operator string present for exactly one of them. A `peers` command that
/// prints nothing, or that prints the operator unconditionally, fails.
#[test]
fn r3_a_verified_claim_is_reported_and_an_absent_one_is_not() {
    let rng = ring::rand::SystemRandom::new();
    let dir = fresh_dir("r3");
    init(&dir);
    let node = serve(&dir);
    let op = mint_key(&rng);

    let with = Advert::new(mint_key(&rng), 24101);
    let claim = claim_block(&op, &with.node.node_id, now_secs() + 3600);
    assert_eq!(
        status_of(&ask_raw(node.port, &with.request(&dir, &[], &claim))),
        "success"
    );

    let without = Advert::new(mint_key(&rng), 24102);
    assert_eq!(
        status_of(&ask_raw(node.port, &without.request(&dir, &[], ""))),
        "success"
    );

    let out = peers_output(&dir);
    assert!(
        peers_lists_node(&out, &with.node.node_id) && peers_lists_node(&out, &without.node.node_id),
        "both peers must be listed before the differential means anything:\n{out}"
    );
    assert!(
        peers_shows_operator(&out, &op.pk_hex),
        "a claim that verifies was not reported:\n{out}"
    );
    // Exactly one occurrence: the unaffiliated peer must not borrow it.
    assert_eq!(
        out.matches(&op.pk_hex).count(),
        1,
        "the operator key appears more than once — an unaffiliated peer got it:\n{out}"
    );
}

/// R4 — a claim whose signature does not compute is not reported, and the
/// advert is still accepted (ruling 3: additive only, never subtractive).
#[test]
fn r4_a_claim_that_does_not_compute_is_dropped_but_the_advert_stands() {
    let rng = ring::rand::SystemRandom::new();
    let dir = fresh_dir("r4");
    init(&dir);
    let node = serve(&dir);
    let op = mint_key(&rng);

    let ad = Advert::new(mint_key(&rng), 24201);
    let good = claim_block(&op, &ad.node.node_id, now_secs() + 3600);
    // Flip one hex digit of the operator signature; everything else is intact.
    let bad = {
        let i = good.find("signature: \"").unwrap() + "signature: \"".len();
        let mut s: Vec<char> = good.chars().collect();
        s[i] = if s[i] == 'a' { 'b' } else { 'a' };
        s.into_iter().collect::<String>()
    };
    assert_ne!(good, bad, "harness failed to corrupt the signature");

    let r = ask_raw(node.port, &ad.request(&dir, &[], &bad));
    assert_eq!(
        status_of(&r),
        "success",
        "a broken claim must not reject the advert: {r}"
    );

    let out = peers_output(&dir);
    assert!(
        peers_lists_node(&out, &ad.node.node_id),
        "peer vanished:\n{out}"
    );
    assert!(
        !peers_shows_operator(&out, &op.pk_hex),
        "an unverifiable claim was reported as an affiliation:\n{out}"
    );
}

/// R5 — `expires` is inside the signed payload, so the holder cannot extend
/// its own claim. The operator signature here is GENUINE; only the expiry
/// printed in the block was moved. An implementation that signs `node_id`
/// alone accepts this and fails the probe.
///
/// CALIBRATION, 2026-07-30: this probe was written with the absence assertion
/// alone and it was GREEN at baseline — `oo node peers` does not exist, its
/// output is a CLI error, the error does not contain the operator key, and so
/// "the affiliation was not reported" held for a reason that has nothing to do
/// with expiry. The in-window control below is what makes the absence mean
/// something. Standing rule, learned again: every red that asserts an absence
/// must assert a presence in the same run.
#[test]
fn r5_a_claim_cannot_be_extended_by_its_holder() {
    let rng = ring::rand::SystemRandom::new();
    let dir = fresh_dir("r5");
    init(&dir);
    let node = serve(&dir);

    // Control: an untouched claim from a different operator, which must be
    // reported, or the assertion below only proves nothing is ever reported.
    let ctl_op = mint_key(&rng);
    let ctl = Advert::new(mint_key(&rng), 24302);
    let ctl_claim = claim_block(&ctl_op, &ctl.node.node_id, now_secs() + 600);
    assert_eq!(
        status_of(&ask_raw(node.port, &ctl.request(&dir, &[], &ctl_claim))),
        "success"
    );

    let op = mint_key(&rng);
    let ad = Advert::new(mint_key(&rng), 24301);
    let real_expiry = now_secs() + 600;
    let block = claim_block(&op, &ad.node.node_id, real_expiry);
    let stretched = block.replace(
        &format!("expires: {real_expiry}"),
        &format!("expires: {}", real_expiry + 86_400),
    );
    assert_ne!(block, stretched, "harness failed to move the expiry");

    let r = ask_raw(node.port, &ad.request(&dir, &[], &stretched));
    assert_eq!(
        status_of(&r),
        "success",
        "ruling 3: the advert still stands: {r}"
    );

    let out = peers_output(&dir);
    assert!(
        peers_shows_operator(&out, &ctl_op.pk_hex),
        "the untouched control claim was not reported, so the absence below \
         proves only that nothing is ever reported:\n{out}"
    );
    assert!(
        !peers_shows_operator(&out, &op.pk_hex),
        "an extended expiry was accepted — `expires` is not in the signed \
         payload:\n{out}"
    );
}

/// R6 — a claim is not transferable. This is the adversarial case with a
/// payload that COMPUTES (standing rule for remote-input entry points): the
/// operator signature is genuine, issued by a real operator, for a real node
/// — just not this one. Node B signs its own body correctly around it.
#[test]
fn r6_another_nodes_claim_does_not_transfer() {
    let rng = ring::rand::SystemRandom::new();
    let dir = fresh_dir("r6");
    init(&dir);
    let node = serve(&dir);
    let op = mint_key(&rng);

    let a = Advert::new(mint_key(&rng), 24401);
    let b = Advert::new(mint_key(&rng), 24402);
    let a_claim = claim_block(&op, &a.node.node_id, now_secs() + 3600);

    // A's claim really is good — establish that first, or R6 proves nothing.
    assert_eq!(
        status_of(&ask_raw(node.port, &a.request(&dir, &[], &a_claim))),
        "success"
    );
    let out = peers_output(&dir);
    assert!(
        peers_shows_operator(&out, &op.pk_hex),
        "A's claim did not verify, so B's rejection would prove nothing:\n{out}"
    );

    // Now B wears it.
    let r = ask_raw(node.port, &b.request(&dir, &[], &a_claim));
    assert_eq!(
        status_of(&r),
        "success",
        "ruling 3: the advert still stands: {r}"
    );

    let out = peers_output(&dir);
    assert_eq!(
        out.matches(&op.pk_hex).count(),
        1,
        "the operator key is reported twice — B inherited A's affiliation:\n{out}"
    );
}

/// R7 — an expired claim is not reported, and one beyond the maximum lifetime
/// is not reported either. Without the ceiling, ruling 4 buys nothing: an
/// operator could issue a hundred-year claim and call it short-lived.
#[test]
fn r7_expiry_is_enforced_at_both_ends() {
    let rng = ring::rand::SystemRandom::new();
    let dir = fresh_dir("r7");
    init(&dir);
    let node = serve(&dir);

    let past_op = mint_key(&rng);
    let past = Advert::new(mint_key(&rng), 24501);
    let expired = claim_block(&past_op, &past.node.node_id, now_secs() - 60);
    assert_eq!(
        status_of(&ask_raw(node.port, &past.request(&dir, &[], &expired))),
        "success"
    );

    let far_op = mint_key(&rng);
    let far = Advert::new(mint_key(&rng), 24502);
    let too_long = claim_block(
        &far_op,
        &far.node.node_id,
        now_secs() + MAX_AFFILIATION_LIFETIME_SECS + 86_400,
    );
    assert_eq!(
        status_of(&ask_raw(node.port, &far.request(&dir, &[], &too_long))),
        "success"
    );

    // A control claim inside the window, so "reports nothing ever" cannot pass.
    let ok_op = mint_key(&rng);
    let ok = Advert::new(mint_key(&rng), 24503);
    let good = claim_block(&ok_op, &ok.node.node_id, now_secs() + 3600);
    assert_eq!(
        status_of(&ask_raw(node.port, &ok.request(&dir, &[], &good))),
        "success"
    );

    let out = peers_output(&dir);
    assert!(
        peers_shows_operator(&out, &ok_op.pk_hex),
        "the in-window control claim was not reported, so the two refusals below \
         prove only that nothing is ever reported:\n{out}"
    );
    assert!(
        !peers_shows_operator(&out, &past_op.pk_hex),
        "an expired claim was reported:\n{out}"
    );
    assert!(
        !peers_shows_operator(&out, &far_op.pk_hex),
        "a claim beyond MAX_AFFILIATION_LIFETIME_SECS was reported:\n{out}"
    );
}

/// R8 — path two of three: a claim relayed inside a #discover answer is
/// verified by the receiver. B learns about the peer from A, never directly.
#[test]
fn r8_the_relayed_path_verifies_the_claim() {
    let rng = ring::rand::SystemRandom::new();
    let a_dir = fresh_dir("r8a");
    let b_dir = fresh_dir("r8b");
    init(&a_dir);
    init(&b_dir);
    let a = serve(&a_dir);

    let op = mint_key(&rng);
    let svc = service_caid(&a_dir, "r8");
    let ad = Advert::new(mint_key(&rng), 24601);
    let claim = claim_block(&op, &ad.node.node_id, now_secs() + 3600);
    assert_eq!(
        status_of(&ask_raw(
            a.port,
            &ad.request(&a_dir, &[svc.as_str()], &claim)
        )),
        "success"
    );

    // B asks A. The answer carries `%ad` verbatim, claim included (measured).
    let out = oo(
        &b_dir,
        &[
            "node",
            "discover",
            "--to",
            &format!("127.0.0.1:{}", a.port),
            "--target",
            &svc,
        ],
    );
    // A CLI error is also non-empty. Assert the query actually ran, or B's
    // empty directory below would be explained by the query never happening.
    assert!(
        !out.trim().is_empty() && !out.contains("error:") && !out.contains("Usage:"),
        "`oo node discover` did not run: {out}"
    );

    let peers = peers_output(&b_dir);
    assert!(
        peers_lists_node(&peers, &ad.node.node_id),
        "B did not record the relayed peer at all:\n{peers}\ndiscover said: {out}"
    );
    assert!(
        peers_shows_operator(&peers, &op.pk_hex),
        "B recorded the peer but never verified the relayed claim — path two of \
         three is unchecked, which is the v0.2.54 defect:\n{peers}"
    );
}

/// R9 — path three of three: affiliation is re-derived when the durable
/// directory is loaded, and re-derived means *re-judged*, not replayed.
///
/// REWRITTEN AT ACCEPTANCE, 2026-07-30. The original tampered with the stored
/// operator signature and required the peer to stay listed. That was a probe
/// built on a false model: **the claim lives inside the node-signed body**, so
/// changing one byte of it breaks the node signature too. There is no such
/// thing as a claim-only tamper, and the state the probe demanded cannot
/// arise. The delivery satisfied it by weakening `verify_loaded` from "drop
/// unverifiable records" to "keep the row, clear services" — after which 50
/// fabricated rows appended to `.oo/peers/directory` were all listed by
/// `oo node peers` and all survived on disk. `.oo/` is writable by any n/
/// program (SPEC_08 §6.3). Both the policy and this probe are repaired.
///
/// Tampering is already owned by `advert_persistence`'s
/// `p8_a_tampered_stored_signature_is_not_served`, and with the revert the
/// whole record is dropped there, as it was at v0.2.54.
///
/// What is left is the one thing only the load path can get wrong, and it is
/// genuinely time-dependent: **a claim that was valid when it arrived and has
/// expired by the time the directory is read**. Nothing about the record
/// changes; only `now` does. A load path that stored the verdict, or that
/// replayed the accept-time answer, reports a stale affiliation and fails.
#[test]
fn r9_an_affiliation_valid_at_receipt_expires_by_load() {
    let rng = ring::rand::SystemRandom::new();
    let dir = fresh_dir("r9");
    init(&dir);

    // Peer A: a long-lived claim. It must survive the restart, or the absence
    // asserted of B below proves only that nothing is ever reported.
    let long_op = mint_key(&rng);
    let long = Advert::new(mint_key(&rng), 24701);
    let long_claim = claim_block(&long_op, &long.node.node_id, now_secs() + 3600);

    // Peer B: a claim that dies while the directory sits on disk.
    let short_op = mint_key(&rng);
    let short = Advert::new(mint_key(&rng), 24702);
    let deadline = now_secs() + 6;
    let short_claim = claim_block(&short_op, &short.node.node_id, deadline);

    let node = serve(&dir);
    assert_eq!(
        status_of(&ask_raw(node.port, &long.request(&dir, &[], &long_claim))),
        "success"
    );
    assert_eq!(
        status_of(&ask_raw(node.port, &short.request(&dir, &[], &short_claim))),
        "success"
    );

    // Both are live right now — assert it before the clock moves, so a slow
    // machine fails loudly here instead of passing vacuously later.
    assert!(
        now_secs() < deadline,
        "the harness took longer than the claim lived"
    );
    let live = peers_output(&dir);
    assert!(
        peers_shows_operator(&live, &short_op.pk_hex),
        "the short-lived claim was never honoured, so its later absence would \
         mean nothing:\n{live}"
    );
    node.stop();

    while now_secs() <= deadline {
        std::thread::sleep(Duration::from_millis(200));
    }
    assert!(
        now_secs() > deadline,
        "harness: the deadline has not passed"
    );

    // Fresh process, same bytes on disk, later clock.
    let out = peers_output(&dir);
    assert!(
        peers_lists_node(&out, &long.node.node_id) && peers_lists_node(&out, &short.node.node_id),
        "a peer disappeared across the restart — an expiring claim is additive \
         and must not subtract the peer (ruling 3):\n{out}"
    );
    assert!(
        peers_shows_operator(&out, &long_op.pk_hex),
        "the long-lived affiliation did not survive the restart, so path three \
         is not running at all:\n{out}"
    );
    assert!(
        !peers_shows_operator(&out, &short_op.pk_hex),
        "an expired affiliation was still reported — the load path replayed a \
         verdict instead of re-judging it:\n{out}"
    );
}

/// R10 — the operator private key is needed to MINT and never to SERVE. This
/// is the single property that keeps REAL_02 §220 intact: if serving needed
/// the operator key, an affiliated node would be a machine holding the
/// operator's signing power, and copying the workspace would copy it.
#[test]
fn r10_serving_an_affiliation_never_needs_the_operator_key() {
    let dir = fresh_dir("r10");
    init(&dir);
    oo(&dir, &["node", "affiliate"]);

    let ident = identity_path(&dir);
    assert!(
        ident.exists(),
        "minting did not create an operator key, so removing it proves nothing"
    );
    fs::remove_file(&ident).unwrap();

    let (port, rx) = stub_peer();
    let out = oo(
        &dir,
        &["node", "advertise", "--to", &format!("127.0.0.1:{port}")],
    );
    let seen = rx
        .recv_timeout(Duration::from_secs(10))
        .unwrap_or_else(|e| panic!("stub peer saw no request ({e:?}); cli said: {out}"));
    assert!(
        seen.contains("affiliation"),
        "the node could not serve its own claim without the operator key present:\n{seen}"
    );
    assert!(
        !ident.exists(),
        "serving re-minted the operator key — §7.5.2 forbids minting outside an \
         actual signing need, and serving is not one"
    );
}

// ════════════════════════════════════════════════════════════════════════
// PINS — green now, must stay green. These are the things this arc must not
// break while adding a field to a signed body.
// ════════════════════════════════════════════════════════════════════════

/// P1 — the feature is optional: an advert with no affiliation is accepted
/// and recorded exactly as before.
#[test]
fn p1_an_advert_without_a_claim_is_still_ordinary() {
    let rng = ring::rand::SystemRandom::new();
    let dir = fresh_dir("p1");
    init(&dir);
    let node = serve(&dir);
    let ad = Advert::new(mint_key(&rng), 24801);
    assert_eq!(
        status_of(&ask_raw(node.port, &ad.request(&dir, &[], ""))),
        "success"
    );
    assert!(
        fs::read_to_string(peers_file(&dir))
            .unwrap_or_default()
            .contains(ad.node.node_id.rsplit(':').next().unwrap()),
        "an unaffiliated peer stopped being recorded"
    );
}

/// P2 — a broken claim does not reject the advert (ruling 3).
///
/// Green today and green after, for two DIFFERENT reasons, which is the whole
/// point of pinning it: today nothing looks at the block, so of course it
/// passes; afterwards the engine looks, decides the affiliation is unproven,
/// and declines to punish the node for it. The pin is what stops the second
/// reason from quietly becoming "reject it".
#[test]
fn p2_a_broken_claim_does_not_reject_the_advert() {
    let rng = ring::rand::SystemRandom::new();
    let dir = fresh_dir("p2");
    init(&dir);
    let node = serve(&dir);
    let ad = Advert::new(mint_key(&rng), 24901);
    let junk = ", affiliation: {{ operator_key: \"00\", signature: \"00\", expires: 1 }}";
    let r = ask_raw(node.port, &ad.request(&dir, &[], junk));
    assert_eq!(
        status_of(&r),
        "success",
        "a broken claim rejected the advert: {r}"
    );
}

/// P3 — the node signature still governs. A body mutated after signing is
/// refused whether or not it carries a claim. The claim must never become a
/// path around the check that was already there.
#[test]
fn p3_the_node_signature_still_governs() {
    let rng = ring::rand::SystemRandom::new();
    let dir = fresh_dir("p3");
    init(&dir);
    let node = serve(&dir);
    let op = mint_key(&rng);
    let ad = Advert::new(mint_key(&rng), 25001);
    let claim = claim_block(&op, &ad.node.node_id, now_secs() + 3600);
    let mutated = ad.signed(&dir, &[], &claim).replace("ttl: 15", "ttl: 14");
    let r = ask_raw(
        node.port,
        &format!(
            "{{{{ %op: #advertise, %from: \"{}\", %ad: {} }}}}\n",
            ad.node.node_id, mutated
        ),
    );
    assert_eq!(
        status_of(&r),
        "rejected",
        "a mutated body was accepted: {r}"
    );
}

/// P4 — advertising still writes no objects. SPEC_13 §4.1.2 obligation #3:
/// engine-local, non-deterministic state must not be minted into the universe,
/// and an operator key is exactly that.
#[test]
fn p4_advertising_writes_no_objects() {
    let rng = ring::rand::SystemRandom::new();
    let dir = fresh_dir("p4");
    init(&dir);
    // Calibration measured that `run` alone writes nothing: objects appear at
    // `evolve` + `commit`. Without this the baseline is zero, and "the count
    // did not change" would hold even if the walker were broken.
    commit_something(&dir, "p4anchor");
    let node = serve(&dir);
    let before = object_count(&dir);
    assert!(before > 0, "harness: a committed store has objects");
    let op = mint_key(&rng);
    let ad = Advert::new(mint_key(&rng), 25101);
    let claim = claim_block(&op, &ad.node.node_id, now_secs() + 3600);
    assert_eq!(
        status_of(&ask_raw(node.port, &ad.request(&dir, &[], &claim))),
        "success"
    );
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(
        object_count(&dir),
        before,
        "receiving a claim minted objects"
    );
}

/// P5 — the universe root does not move when a claim is received.
#[test]
fn p5_the_universe_root_is_unmoved_by_a_claim() {
    let rng = ring::rand::SystemRandom::new();
    let dir = fresh_dir("p5");
    init(&dir);
    let root = commit_something(&dir, "p5anchor");
    let before = oo(&dir, &["status"]);
    assert!(
        !before.trim().is_empty() && object_count(&dir) > 0,
        "harness: nothing was committed, so an unmoved root proves nothing \
         (status={before:?}, commit said {root:?})"
    );

    let node = serve(&dir);
    let op = mint_key(&rng);
    let ad = Advert::new(mint_key(&rng), 25201);
    let claim = claim_block(&op, &ad.node.node_id, now_secs() + 3600);
    ask_raw(node.port, &ad.request(&dir, &[], &claim));
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(
        oo(&dir, &["status"]),
        before,
        "a received claim moved the universe root"
    );
}

/// P6 — the store format marker is not bumped. Measured at reconnaissance:
/// the verified operator is a DERIVED fact, rebuilt at load from the verbatim
/// `%ad` the record already holds, so nothing new is persisted. Should the
/// delivery decide to store the verdict anyway, this pin is what makes that a
/// decision rather than a side effect.
#[test]
fn p6_the_store_format_marker_is_not_bumped() {
    let rng = ring::rand::SystemRandom::new();
    let dir = fresh_dir("p6");
    init(&dir);
    let fmt = dir.join(".oo").join("format");
    let before = fs::read_to_string(&fmt).unwrap_or_default();
    assert!(
        !before.trim().is_empty(),
        "harness: `.oo/format` is empty or absent"
    );

    let node = serve(&dir);
    let op = mint_key(&rng);
    let ad = Advert::new(mint_key(&rng), 25301);
    let claim = claim_block(&op, &ad.node.node_id, now_secs() + 3600);
    ask_raw(node.port, &ad.request(&dir, &[], &claim));
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(
        fs::read_to_string(&fmt).unwrap_or_default(),
        before,
        "`.oo/format` was bumped"
    );
}

/// P7 — the operator private key never enters the workspace store. REAL_01
/// §7.5.1: secrets must not live in the thing that exists in order to be
/// copied. Minting a claim is the first time this arc touches that key, so
/// this is the moment the property is at risk.
#[test]
fn p7_the_operator_private_key_never_enters_the_store() {
    let dir = fresh_dir("p7");
    init(&dir);
    oo(&dir, &["node", "affiliate"]);
    let ident = identity_path(&dir);
    if !ident.exists() {
        // Baseline: nothing minted it yet. The scan below still has to run —
        // it is the scan, not the key, that this pin protects.
        assert!(true);
    }
    let secret = fs::read(&ident).unwrap_or_default();

    let mut checked = 0usize;
    fn walk(p: &Path, secret: &[u8], checked: &mut usize) {
        let Ok(rd) = fs::read_dir(p) else { return };
        for e in rd.flatten() {
            let path = e.path();
            if path.is_dir() {
                walk(&path, secret, checked);
            } else {
                *checked += 1;
                if secret.len() >= 32 {
                    let bytes = fs::read(&path).unwrap_or_default();
                    assert!(
                        !bytes.windows(secret.len()).any(|w| w == secret),
                        "the operator private key was found inside {}",
                        path.display()
                    );
                }
            }
        }
    }
    walk(&dir.join(".oo"), &secret, &mut checked);
    assert!(
        checked > 0,
        "the scan visited no files — a silent walker passes everything"
    );
}

/// P8 — ordinary work still does not mint a node key. REAL_01 §7.5.4: reading
/// at open is allowed, minting is not. Adding a CLI that signs must not turn
/// `oo status` into a key-minting operation.
#[test]
fn p8_ordinary_work_does_not_mint_a_node_key() {
    let dir = fresh_dir("p8");
    init(&dir);
    let home = dir.join("node-home-for-tests");
    let count = |p: &Path| -> usize {
        fn walk(p: &Path, n: &mut usize) {
            if let Ok(rd) = fs::read_dir(p) {
                for e in rd.flatten() {
                    let path = e.path();
                    if path.is_dir() {
                        walk(&path, n);
                    } else {
                        *n += 1;
                    }
                }
            }
        }
        let mut n = 0;
        walk(p, &mut n);
        n
    };
    let before = count(&home);
    oo(&dir, &["status"]);
    oo(&dir, &["log"]);
    fs::write(dir.join("w.n"), "w: { x: 2 }\n").unwrap();
    oo(&dir, &["run", "w.n"]);
    assert_eq!(
        count(&home),
        before,
        "ordinary work minted something in the node key home"
    );
}
