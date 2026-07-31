// A proof that does not name the prover (2026-07-27, pre-committed by work
// order: docs/advertise_wire_handover.md).
//
// ── The headline, measured on v0.2.49 ────────────────────────────────────
//
// A node cannot serve its own public key:
//
//     $ oo node id
//     hash:sha256:v2:_:…:8a2add2b…
//     $ printf '{{ %op: #fetch, %hash: "hash:…8a2add2b…" }}\n' | nc node 19551
//     {"%status":"#not_found", …}
//
// REAL_02 §7.2 obligation #1 says an engine receiving a ServiceAdvertisement
// MUST "verify `signature` matches the public key corresponding to `node_id`".
// §4.2 and §7.1 give that packet `node_id: @caid` and `signature: b""` and no
// public key; Ed25519 does not verify against a hash; and the measurement
// above shows there is no fetch route to the key either.
//
//     §7.2 obligation #1 has never been satisfiable. An unsatisfiable check is
//     not a stricter check — it is no check.
//
// The same shape as the v0.2.46 whitelist that no value could satisfy.
//
// ── Where the key comes from ─────────────────────────────────────────────
// Inline, in the advertisement, with the receiver recomputing CAID(key) and
// comparing it to the claimed `node_id`. Not by fetching: the reason you are
// verifying an advertisement is to learn where and who peers are, so a route
// that presupposes a reachable peer is a bootstrap loop.
//
// The operator path already worked this way and the wire simply had not
// followed: `AuthorityInfo` carries `signer_pubkey_hex` — the key itself, not
// its CAID (`authority.rs`).
//
// The advertisement therefore splits exactly along the two strata of
// discussion 025:
//
//   public_key   self-authenticating OBJECT — whoever handed it to you is
//                irrelevant; if it hashes to node_id it IS that key
//   services     an ASSERTION — the signature makes a liar attributable, it
//                does not make the list true
//
// Merciless at degree 0 is what funds permissiveness at degree ≥1. R8 pins the
// second half: advertising a CAID the node does not hold is `#success`, and any
// engine that "verifies" the services list is claiming something it cannot know.
//
// ── Host observed, port claimed (work order Q1) ──────────────────────────
// v0.2.49 replaced `source_id = format!("node:{}", port)` with the node id.
// That was right — a port is not an identity — but the port had been doing two
// jobs, and fixing the identity job left the *location* job with no home: after
// v0.2.49 there is no address anywhere in REAL_02.
//
// The address is reassembled from two sources of different quality:
//
//   host   OBSERVED, from the connection carrying the advertisement
//   port   CLAIMED, inside the signature (a listening port cannot be observed;
//          the source port is ephemeral and is not it)
//
// So an advertisement can only ever describe the machine you are already
// talking to. A signed claim can never name a third party, and the reflection
// vector closes structurally rather than by a check. R7 measures both halves.
//
// ── Why `%reason` and not four statuses (work order Q3) ──────────────────
// Rejection has at least four causes. v0.2.48's finding was that collapsing
// four situations into one silence is the defect; collapsing four rejections
// into one `#conflict` is that same defect one level down. `%status:
// #rejected` keeps the envelope's status set stable across ops and `%reason`
// carries the discrimination. R2/R3/R4/R5 are pairwise, not merely "not
// #success" — the named-parameter arc's lesson: a gate that cannot tell two
// failures apart passes for the wrong reason.
//
// ── Explicitly NOT this arc ──────────────────────────────────────────────
// GPP (REAL_02 §7 / APP_02 §6). The measurement that opened this arc also
// found that REAL_02 §7's "身分" and APP_02 §6's are different questions —
// §6's circuit takes `fingerprint_commitment, n, k` as public inputs and names
// no prover, so what it proves is custody, and a proof bound to nothing about
// the prover is transferable. That reorganisation spans four documents and is
// deferred to its own discussion. Nothing here may be read as settling it.
//
// The peer directory this arc writes is not read by any fetch path. That is
// deliberate and declared, not implied: routing is the discover arc.

use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use ring::signature::{Ed25519KeyPair, KeyPair};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

/// Domain separation: a signature over an advertisement CAID must not be
/// replayable as a signature over anything else that hashes values (`refine:`
/// in `authority.rs` is separated the same way).
const ADVERT_DOMAIN: &str = "oodp-advert:v1:";

/// Returned by `run_bounded` when the engine had to be killed.
const HUNG: &str = "<HARNESS: engine had to be killed>";

// ── harness ─────────────────────────────────────────────────────────────

fn fresh_dir(tag: &str) -> PathBuf {
    let mut d = std::env::temp_dir();
    d.push(format!(
        "nlang-advert-{}-{}-{}",
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

fn init(dir: &Path) {
    oo(dir, &["run", "--help"]);
    write(dir, "seed.n", "seed: { ok: #true }\n");
    oo(dir, &["run", "seed.n"]);
}

/// First string atom in an `oo eval` / `oo run --observe` reply.
fn first_string(out: &str) -> String {
    let s = out
        .split_once('"')
        .unwrap_or_else(|| panic!("no string atom in {out:?}"))
        .1;
    s.split('"').next().unwrap().to_string()
}

/// Stores `expr` in `dir` and returns its CAID.
fn store(dir: &Path, expr: &str) -> String {
    write(
        dir,
        "i.n",
        &format!("id: ~%Discovery./identify_and_store {expr}\n"),
    );
    let caid = first_string(&oo(dir, &["run", "i.n", "--observe", "id"]));
    assert!(caid.starts_with("hash:sha256:"), "store() got {caid:?}");
    caid
}

/// CAID of `expr` **without** storing it — the engine's own canonical encoding,
/// which is what the signature commits to.
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

// ── node identity, probe side ───────────────────────────────────────────

/// This workspace's node identity as the probe needs it: the id the engine
/// publishes, and the private key it signs with.
struct NodeKey {
    node_id: String,
    key_pair: Ed25519KeyPair,
    public_key_hex: String,
}

fn node_key(dir: &Path) -> NodeKey {
    let out = oo(dir, &["node", "id"]);
    let node_id = out
        .lines()
        .find(|l| l.starts_with("hash:"))
        .unwrap_or_else(|| panic!("`oo node id` printed no CAID: {out:?}"))
        .trim()
        .to_string();
    let path = out
        .lines()
        .find_map(|l| l.strip_prefix("path:"))
        .unwrap_or_else(|| panic!("`oo node id` printed no path: {out:?}"))
        .trim()
        .to_string();
    let pkcs8 = fs::read(&path).unwrap_or_else(|e| panic!("read node key {path}: {e}"));
    let key_pair = Ed25519KeyPair::from_pkcs8(&pkcs8)
        .unwrap_or_else(|e| panic!("node key at {path} is not PKCS#8 Ed25519: {e:?}"));
    let public_key_hex = hex::encode(key_pair.public_key().as_ref());
    NodeKey {
        node_id,
        key_pair,
        public_key_hex,
    }
}

// ── advertisement construction ──────────────────────────────────────────

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

/// A ServiceAdvertisement under construction. Every field is settable so the
/// probe can express packets the engine would never emit.
struct Advert {
    node_id: String,
    public_key_hex: Option<String>,
    services: Vec<String>,
    listen_port: u16,
    capacity: i64,
    ts: i64,
    ttl: i64,
}

impl Advert {
    fn new(nk: &NodeKey, listen_port: u16) -> Self {
        Advert {
            node_id: nk.node_id.clone(),
            public_key_hex: Some(nk.public_key_hex.clone()),
            services: vec![],
            listen_port,
            capacity: 10,
            ts: now_secs(),
            ttl: 15,
        }
    }

    /// The signed body: the advertisement **before** `signature` is added.
    fn body(&self) -> String {
        let services = self
            .services
            .iter()
            .map(|s| format!("\"{s}\""))
            .collect::<Vec<_>>()
            .join(", ");
        let mut fields = vec![format!("node_id: \"{}\"", self.node_id)];
        if let Some(ref pk) = self.public_key_hex {
            fields.push(format!("public_key: \"{pk}\""));
        }
        fields.push(format!("services: [{services}]"));
        fields.push(format!("listen_port: {}", self.listen_port));
        fields.push(format!("capacity: {}", self.capacity));
        fields.push(format!("ts: {}", self.ts));
        fields.push(format!("ttl: {}", self.ttl));
        format!("{{{{ {} }}}}", fields.join(", "))
    }

    /// Full advertisement, signed by `signer` over the CAID of `body()`.
    ///
    /// `caid_dir` is only a workspace to ask for the canonical CAID; it never
    /// affects what is signed.
    fn signed(&self, caid_dir: &Path, signer: &Ed25519KeyPair) -> String {
        let body = self.body();
        let caid = caid_of(caid_dir, &body);
        let payload = format!("{ADVERT_DOMAIN}{caid}");
        let sig = hex::encode(signer.sign(payload.as_bytes()).as_ref());
        let inner = body.trim_start_matches("{{").trim_end_matches("}}").trim();
        format!("{{{{ {inner}, signature: \"{sig}\" }}}}")
    }
}

/// Wrap an advertisement in the request envelope.
fn advert_request(from: &str, ad: &str) -> String {
    format!("{{{{ %op: #advertise, %from: \"{from}\", %ad: {ad} }}}}\n")
}

// ── running node ────────────────────────────────────────────────────────

/// A node, with its log on disk so gates can read it while it runs.
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

    /// Waits until `pred` sees the log, or gives up. Returns the final log.
    fn log_until(&self, pred: impl Fn(&str) -> bool) -> String {
        for _ in 0..40 {
            let l = self.log();
            if pred(&l) {
                return l;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        self.log()
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
    let mut node = Node { child, port, log };
    for _ in 0..40 {
        std::thread::sleep(Duration::from_millis(100));
        if node.child.try_wait().unwrap().is_some() {
            panic!("`oo node serve` exited: {}", node.log());
        }
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return node;
        }
    }
    panic!("`oo node serve` never came up: {}", node.log());
}

/// Sends raw bytes and reads the whole reply. Also returns the source port the
/// probe connected FROM, which R7 needs: it is ephemeral and is not the
/// advertised listening port.
fn ask_raw_from(port: u16, payload: &str) -> (String, u16) {
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    let src = s.local_addr().unwrap().port();
    s.write_all(payload.as_bytes()).unwrap();
    if !payload.ends_with('\n') {
        s.write_all(b"\n").unwrap();
    }
    s.flush().unwrap();
    s.shutdown(std::net::Shutdown::Write).ok();
    let mut buf = Vec::new();
    s.read_to_end(&mut buf).ok();
    (String::from_utf8_lossy(&buf).to_string(), src)
}

fn ask_raw(port: u16, payload: &str) -> String {
    ask_raw_from(port, payload).0
}

/// `%status` of a reply, without the `#`. `<none>` when the peer said nothing —
/// silence must never be indistinguishable from an answer (REAL_02 §3.2).
fn status_of(reply: &str) -> String {
    field_of(reply, "%status").unwrap_or_else(|| "<none>".into())
}

fn reason_of(reply: &str) -> String {
    field_of(reply, "%reason").unwrap_or_else(|| "<absent>".into())
}

fn field_of(reply: &str, key: &str) -> Option<String> {
    let j: serde_json::Value = serde_json::from_str(reply.trim()).ok()?;
    let v = j.get(key).or_else(|| j.get(key.trim_start_matches('%')))?;
    Some(v.as_str()?.trim().trim_start_matches('#').to_string())
}

/// A pair of workspaces: `a` serves, `b` advertises to it.
struct Pair {
    a: PathBuf,
    b: PathBuf,
}

fn pair(tag: &str) -> Pair {
    let a = fresh_dir(&format!("{tag}-a"));
    let b = fresh_dir(&format!("{tag}-b"));
    init(&a);
    init(&b);
    Pair { a, b }
}

// ════════════════════════════════════════════════════════════════════════
//  RED — must fail on v0.2.49, for the reason stated
// ════════════════════════════════════════════════════════════════════════

/// R1 — a well-formed signed advertisement is accepted.
///
/// Baseline: `#advertise` answers `#not_implemented` (REAL_02 §3.2 requires an
/// explicit status, and v0.2.48 gave it one, but never an implementation).
#[test]
fn r1_valid_advertisement_accepted() {
    let p = pair("r1");
    let node = serve(&p.a);
    let nk = node_key(&p.b);
    let ad = Advert::new(&nk, 8080);

    let reply = ask_raw(
        node.port,
        &advert_request(&nk.node_id, &ad.signed(&p.b, &nk.key_pair)),
    );

    assert_eq!(
        status_of(&reply),
        "success",
        "a correctly signed advertisement must be accepted; got {reply}"
    );
}

/// R2 — a body altered after signing is `#bad_signature`.
///
/// Pairwise: the SAME advertisement unaltered must be `#success`, so the gate
/// measures the alteration and not some unrelated refusal.
#[test]
fn r2_tampered_body_is_bad_signature() {
    let p = pair("r2");
    let node = serve(&p.a);
    let nk = node_key(&p.b);
    let ad = Advert::new(&nk, 8080);
    let good = ad.signed(&p.b, &nk.key_pair);

    let clean = ask_raw(node.port, &advert_request(&nk.node_id, &good));
    assert_eq!(status_of(&clean), "success", "control: {clean}");

    // Same signature, one field moved.
    let tampered = good.replace("capacity: 10", "capacity: 11");
    assert_ne!(tampered, good, "harness: tamper did not apply");
    let reply = ask_raw(node.port, &advert_request(&nk.node_id, &tampered));

    assert_eq!(status_of(&reply), "rejected", "got {reply}");
    assert_eq!(
        reason_of(&reply),
        "bad_signature",
        "an altered body must name the signature, not something else; got {reply}"
    );
}

/// R3 — a **valid** signature under an identity that is not the claimed one is
/// `#identity_mismatch`, not `#bad_signature`.
///
/// This is the gate the whole arc turns on. The signature verifies against the
/// inline key; what fails is that CAID(inline key) ≠ the claimed `node_id`. An
/// engine that only checks "does the signature verify against the key in the
/// packet" passes every forgery, because the forger supplies both.
#[test]
fn r3_wrong_identity_is_not_bad_signature() {
    let p = pair("r3");
    let node = serve(&p.a);
    let mine = node_key(&p.b);
    let other = node_key(&p.a); // a different workspace ⇒ a different key

    let mut ad = Advert::new(&mine, 8080);
    ad.node_id = other.node_id.clone(); // claim to be A …
                                        // … while carrying B's key and signing with B's key: internally consistent
                                        // except for the one binding that matters.
    let packet = ad.signed(&p.b, &mine.key_pair);
    let reply = ask_raw(node.port, &advert_request(&other.node_id, &packet));

    assert_eq!(status_of(&reply), "rejected", "got {reply}");
    assert_eq!(
        reason_of(&reply),
        "identity_mismatch",
        "the signature is valid — what fails is CAID(public_key) ≠ node_id; got {reply}"
    );
}

/// R4 — a stale timestamp is `#stale`, and freshness is the only difference.
#[test]
fn r4_stale_timestamp_pairwise() {
    let p = pair("r4");
    let node = serve(&p.a);
    let nk = node_key(&p.b);

    let mut fresh = Advert::new(&nk, 8080);
    fresh.ts = now_secs();
    let ok = ask_raw(
        node.port,
        &advert_request(&nk.node_id, &fresh.signed(&p.b, &nk.key_pair)),
    );
    assert_eq!(status_of(&ok), "success", "control: {ok}");

    let mut old = Advert::new(&nk, 8080);
    old.ts = now_secs() - 3600;
    let reply = ask_raw(
        node.port,
        &advert_request(&nk.node_id, &old.signed(&p.b, &nk.key_pair)),
    );

    assert_eq!(status_of(&reply), "rejected", "got {reply}");
    assert_eq!(
        reason_of(&reply),
        "stale",
        "an hour-old advertisement must name the clock; got {reply}"
    );
}

/// R5 — a malformed advertisement is `#malformed`, distinguishable from the
/// three substantive refusals.
#[test]
fn r5_malformed_is_its_own_reason() {
    let p = pair("r5");
    let node = serve(&p.a);
    let nk = node_key(&p.b);

    // `%ad` absent entirely.
    let no_ad = format!("{{{{ %op: #advertise, %from: \"{}\" }}}}\n", nk.node_id);
    let r = ask_raw(node.port, &no_ad);
    assert_eq!(status_of(&r), "rejected", "got {r}");
    assert_eq!(reason_of(&r), "malformed", "missing %ad: {r}");

    // `%ad` present but not a combo.
    let scalar = format!(
        "{{{{ %op: #advertise, %from: \"{}\", %ad: 7 }}}}\n",
        nk.node_id
    );
    let r = ask_raw(node.port, &scalar);
    assert_eq!(status_of(&r), "rejected", "got {r}");
    assert_eq!(reason_of(&r), "malformed", "scalar %ad: {r}");

    // `%ad` a combo, but no signature at all.
    let unsigned = Advert::new(&nk, 8080).body();
    let r = ask_raw(node.port, &advert_request(&nk.node_id, &unsigned));
    assert_eq!(status_of(&r), "rejected", "got {r}");
    assert_eq!(
        reason_of(&r),
        "malformed",
        "an unsigned advertisement is missing a required field, not a bad signature: {r}"
    );
}

/// R6 — the inline public key is load-bearing, and no other route may stand in
/// for it.
///
/// A valid advertisement is sent first, so the node has certainly seen this
/// key. A second advertisement, identical but for the missing `public_key`, is
/// still refused. Remembering the key from earlier, deriving it, or fetching it
/// would each turn `node_id` back into an unverifiable claim.
#[test]
fn r6_no_fallback_route_to_the_key() {
    let p = pair("r6");
    let node = serve(&p.a);
    let nk = node_key(&p.b);

    let seen = Advert::new(&nk, 8080);
    let ok = ask_raw(
        node.port,
        &advert_request(&nk.node_id, &seen.signed(&p.b, &nk.key_pair)),
    );
    assert_eq!(status_of(&ok), "success", "control: {ok}");

    let mut keyless = Advert::new(&nk, 8080);
    keyless.public_key_hex = None;
    let reply = ask_raw(
        node.port,
        &advert_request(&nk.node_id, &keyless.signed(&p.b, &nk.key_pair)),
    );

    assert_eq!(
        status_of(&reply),
        "rejected",
        "the key must travel in every advertisement, not once; got {reply}"
    );
    assert_eq!(reason_of(&reply), "malformed", "got {reply}");
}

/// R7 — host observed, port claimed.
///
/// Two halves, both required:
///   (a) the recorded address uses the port from the signed advertisement;
///   (b) it does NOT use the ephemeral source port the packet arrived on.
///
/// (b) is what makes (a) mean anything: an engine that recorded the peer's
/// source address would also "contain the address" for most of a test run.
#[test]
fn r7_host_observed_port_claimed() {
    let p = pair("r7");
    let node = serve(&p.a);
    let nk = node_key(&p.b);
    let claimed = free_port(); // nobody is listening there; the claim stands alone
    let ad = Advert::new(&nk, claimed);

    let (reply, source_port) = ask_raw_from(
        node.port,
        &advert_request(&nk.node_id, &ad.signed(&p.b, &nk.key_pair)),
    );
    assert_eq!(status_of(&reply), "success", "control: {reply}");
    assert_ne!(
        source_port, claimed,
        "harness: the ephemeral port collided with the claim; rerun"
    );

    let want = format!("addr=127.0.0.1:{claimed}");
    let log = node.log_until(|l| l.contains(&want));
    let recorded: Vec<&str> = log
        .lines()
        .filter(|l| l.starts_with("OODP Advert:"))
        .collect();
    assert!(
        !recorded.is_empty(),
        "no `OODP Advert:` line recording the accepted peer; log:\n{log}"
    );
    let recorded = recorded.join("\n");
    assert!(
        recorded.contains(&want),
        "the peer address must be observed-host + claimed-port ({want}); recorded:\n{recorded}"
    );
    // Scoped to the record line on purpose: logging the connection's source
    // address elsewhere is fine, believing it is the peer's address is not.
    assert!(
        !recorded.contains(&format!("127.0.0.1:{source_port}")),
        "the ephemeral source port {source_port} was recorded as the peer address; \
         recorded:\n{recorded}"
    );
}

/// R8 — `services` is a claim and is never verified.
///
/// The advertisement names a CAID that exists nowhere. It must be accepted:
/// the receiving node cannot know what a peer holds, and an engine that
/// answered `#not_found` here would be asserting knowledge it does not have.
/// The claim costs the liar a wasted round trip later, when `#fetch` returns a
/// self-authenticating object or does not.
#[test]
fn r8_services_is_a_claim_not_a_fact() {
    let p = pair("r8");
    let node = serve(&p.a);
    let nk = node_key(&p.b);

    // A real CAID of a value neither node stores.
    let absent = caid_of(&p.b, "{{ nobody: \"has this\" }}");
    let mut ad = Advert::new(&nk, 8080);
    ad.services = vec![absent.clone()];

    let reply = ask_raw(
        node.port,
        &advert_request(&nk.node_id, &ad.signed(&p.b, &nk.key_pair)),
    );

    assert_eq!(
        status_of(&reply),
        "success",
        "a service list is unverifiable; refusing it claims knowledge the node \
         does not have. got {reply}"
    );
}

/// R9 — the envelope and the payload must agree about who is speaking.
///
/// The advertisement is entirely valid; only `%from` disagrees. A packet that
/// says two different things about its sender must not be resolved by the
/// receiver silently picking one.
#[test]
fn r9_envelope_and_payload_must_agree() {
    let p = pair("r9");
    let node = serve(&p.a);
    let mine = node_key(&p.b);
    let other = node_key(&p.a);

    let ad = Advert::new(&mine, 8080);
    let packet = ad.signed(&p.b, &mine.key_pair);

    let ok = ask_raw(node.port, &advert_request(&mine.node_id, &packet));
    assert_eq!(status_of(&ok), "success", "control: {ok}");

    let reply = ask_raw(node.port, &advert_request(&other.node_id, &packet));
    assert_eq!(status_of(&reply), "rejected", "got {reply}");
    assert_eq!(
        reason_of(&reply),
        "identity_mismatch",
        "%from ≠ %ad.node_id must be refused, not silently resolved; got {reply}"
    );
}

// ════════════════════════════════════════════════════════════════════════
//  PIN — green on v0.2.49 and must stay green
// ════════════════════════════════════════════════════════════════════════

/// P1 — `#fetch` still works, and absence is still distinguishable from it.
#[test]
fn p1_fetch_unbroken() {
    let p = pair("p1");
    let caid = store(&p.a, "{{ kept: \"here\" }}");
    let node = serve(&p.a);
    let nk = node_key(&p.b);

    let hit = ask_raw(
        node.port,
        &format!(
            "{{{{ %op: #fetch, %hash: \"{caid}\", %from: \"{}\" }}}}\n",
            nk.node_id
        ),
    );
    assert_eq!(status_of(&hit), "success", "{hit}");

    let missing = caid_of(&p.b, "{{ not: \"stored\" }}");
    let miss = ask_raw(
        node.port,
        &format!(
            "{{{{ %op: #fetch, %hash: \"{missing}\", %from: \"{}\" }}}}\n",
            nk.node_id
        ),
    );
    assert_eq!(status_of(&miss), "not_found", "{miss}");
}

/// P2 — `%from` stays a claim on `#fetch`.
///
/// This arc makes `%from` load-bearing for `#advertise`. It must not become
/// load-bearing for `#fetch`: objects self-authenticate, so serving cannot
/// depend on who asks (REAL_02 §3.2).
#[test]
fn p2_from_is_still_a_claim_on_fetch() {
    let p = pair("p2");
    let caid = store(&p.a, "{{ served: \"regardless\" }}");
    let node = serve(&p.a);
    let real = node_key(&p.b).node_id;

    let mut seen = vec![];
    for from in [
        real.as_str(),
        "hash:sha256:v1:0000000000000000000000000000000000000000000000000000000000000000",
        "not-a-caid",
        "",
    ] {
        let r = ask_raw(
            node.port,
            &format!("{{{{ %op: #fetch, %hash: \"{caid}\", %from: \"{from}\" }}}}\n"),
        );
        seen.push(status_of(&r));
    }
    // Omitted entirely.
    let r = ask_raw(
        node.port,
        &format!("{{{{ %op: #fetch, %hash: \"{caid}\" }}}}\n"),
    );
    seen.push(status_of(&r));

    assert!(
        seen.iter().all(|s| s == "success"),
        "serving must not depend on %from; got {seen:?}"
    );
}

/// P3 — `#discover` is the service index (discover_index arc). Missing
/// `%target` is `#conflict`; a well-formed query is `#success` (possibly empty
/// `%peers`), never `#not_implemented`.
#[test]
fn p3_discover_is_served() {
    let p = pair("p3");
    let node = serve(&p.a);
    let caid = caid_of(&p.b, "{{ q: 1 }}");
    // Wrong field: `%hash` is for `#fetch`; discover requires `%target`.
    let r = ask_raw(
        node.port,
        &format!("{{{{ %op: #discover, %hash: \"{caid}\" }}}}\n"),
    );
    assert_eq!(status_of(&r), "conflict", "missing %target: {r}");
    let r = ask_raw(
        node.port,
        &format!("{{{{ %op: #discover, %from: \"x\", %target: \"{caid}\" }}}}\n"),
    );
    assert_eq!(
        status_of(&r),
        "success",
        "a well-formed #discover must be answered: {r}"
    );
}

/// P4 — unknown ops say `#not_implemented` (wire_says_why); the retired bare-
/// CAID form and garbage lines stay `#conflict`. None is answered with silence.
#[test]
fn p4_unknown_and_retired_forms() {
    let p = pair("p4");
    let caid = store(&p.a, "{{ v: 1 }}");
    let node = serve(&p.a);

    let unknown = ask_raw(node.port, "{{ %op: #teleport, %hash: \"x\" }}\n");
    // SCHEDULED CHANGE (wire_says_why §6.1): unknown op is not #conflict.
    assert_eq!(status_of(&unknown), "not_implemented", "{unknown}");

    let bare = ask_raw(node.port, &format!("{caid}\n"));
    assert_eq!(
        status_of(&bare),
        "conflict",
        "bare CAID stays retired: {bare}"
    );

    let garbage = ask_raw(node.port, "not a packet at all\n");
    assert_eq!(status_of(&garbage), "conflict", "{garbage}");
}

/// P5 — the language-layer LADD is a different thing and stays untouched.
///
/// `~%Discovery./advertise` writes the in-process GBB registry; the wire
/// `#advertise` is a packet between engines. Conflating them would let a remote
/// packet steer local routing.
///
/// Measured baseline, and pinned in BOTH directions: advertising a value and
/// then asking for it by the same key burns the hop budget and eclipses.
///
///     a: ~%Discovery./advertise { pkg: "ladd", version: 1 }   →  #true
///     f: ~%Discovery./find      { pkg: "ladd" }               →  _|_ #semantic_eclipse
///
/// That verdict is **not endorsed here**. It is pre-existing (`disc.find` only
/// resolves via the explicit-`target` direct-lookup path today) and it is on
/// the ledger. The pin exists so this arc changes it in neither direction — a
/// wire arc that silently "fixed" local routing on the way past would be just
/// as much a scope leak as one that broke it.
#[test]
fn p5_local_ladd_untouched() {
    let p = pair("p5");
    write(
        &p.b,
        "l.n",
        "a: ~%Discovery./advertise { pkg: \"ladd\", version: 1 }\n\
         f: ~%Discovery./find { pkg: \"ladd\" }\n",
    );
    let adv = run_bounded(&p.b, &["run", "l.n", "--observe", "a"], 30);
    assert!(
        !adv.contains("_|_") && adv.contains("true"),
        "local advertise regressed: {adv:?}"
    );
    let found = run_bounded(&p.b, &["run", "l.n", "--observe", "f"], 30);
    assert_ne!(HUNG, found, "~%Discovery./find hung");
    assert!(
        found.contains("semantic_eclipse"),
        "local find's verdict moved; this arc must not touch it either way: {found:?}"
    );
}

/// P6 — the node private key is still refused at the language boundary
/// (v0.2.49's repair; REAL_01 §7.5.3).
#[test]
fn p6_node_key_still_behind_the_boundary() {
    let p = pair("p6");
    let out = oo(&p.b, &["node", "id"]);
    let path = out
        .lines()
        .find_map(|l| l.strip_prefix("path:"))
        .unwrap()
        .trim()
        .to_string();
    let esc = path.replace('\\', "\\\\");
    write(&p.b, "read.n", &format!("out: ~%Io./read_file \"{esc}\"\n"));
    let got = run_bounded(&p.b, &["run", "read.n", "--observe", "out"], 30);
    assert!(
        got.contains("store_boundary") || got.contains("_|_"),
        "the node key must be refused, not merely undecodable: {got:?}"
    );
}

/// P7 — an advertisement writes nothing to the object store.
///
/// Non-vacuous by construction: the store is proved non-empty first, so this
/// is not two empty sets being compared (the v0.2.44 lesson).
#[test]
fn p7_advertising_stores_nothing() {
    let p = pair("p7");
    store(&p.a, "{{ pre: \"existing\" }}");
    let before = object_count(&p.a);
    assert!(
        before > 0,
        "harness: nothing in the store to compare against"
    );

    let node = serve(&p.a);
    let nk = node_key(&p.b);
    let ad = Advert::new(&nk, 8080);
    ask_raw(
        node.port,
        &advert_request(&nk.node_id, &ad.signed(&p.b, &nk.key_pair)),
    );
    std::thread::sleep(Duration::from_millis(300));

    assert_eq!(
        object_count(&p.a),
        before,
        "an unsolicited packet must not add objects to the receiver's store"
    );
}

/// P8 — advertising does not move the universe root.
///
/// The peer directory is engine-local state. SPEC_13 §4.1.2 obligation #1 keeps
/// engine-local state out of the universe, which is exactly what made the node
/// id a keypair rather than a content address (REAL_02 §4.1.1).
#[test]
fn p8_universe_root_unmoved() {
    let p = pair("p8");
    write(&p.a, "u.n", "v: { anchor: 1 }\n");
    oo(&p.a, &["run", "u.n"]);
    oo(&p.a, &["commit", "-m", "anchor"]);
    let before = oo(&p.a, &["status"]);
    assert!(
        !before.trim().is_empty(),
        "harness: `oo status` said nothing"
    );

    let node = serve(&p.a);
    let nk = node_key(&p.b);
    let ad = Advert::new(&nk, 8080);
    ask_raw(
        node.port,
        &advert_request(&nk.node_id, &ad.signed(&p.b, &nk.key_pair)),
    );
    std::thread::sleep(Duration::from_millis(300));

    assert_eq!(
        oo(&p.a, &["status"]),
        before,
        "advertising changed the workspace's own state"
    );
}

/// P9 — the node id is stable, and advertising does not change it.
#[test]
fn p9_node_id_stable() {
    let p = pair("p9");
    let first = node_key(&p.b).node_id;
    let again = node_key(&p.b).node_id;
    assert_eq!(first, again, "node id is not stable across calls");
    assert!(first.starts_with("hash:sha256:"), "{first}");

    let node = serve(&p.a);
    let nk = node_key(&p.b);
    let ad = Advert::new(&nk, 8080);
    ask_raw(
        node.port,
        &advert_request(&nk.node_id, &ad.signed(&p.b, &nk.key_pair)),
    );
    assert_eq!(
        node_key(&p.b).node_id,
        first,
        "advertising rotated the node id"
    );
}

/// P10 — the operator key and the node key stay different keys
/// (REAL_01 §7.5.4 / REAL_02 §4.1.1: who authorises vs which one is serving).
#[test]
fn p10_two_keys_stay_two() {
    let p = pair("p10");
    let operator = oo(&p.b, &["identity"]);
    let op_hex = operator
        .lines()
        .find(|l| l.len() == 64 && l.chars().all(|c| c.is_ascii_hexdigit()))
        .unwrap_or_else(|| panic!("`oo identity` printed no key: {operator:?}"))
        .to_string();
    let node_hex = node_key(&p.b).public_key_hex;
    assert_ne!(
        op_hex, node_hex,
        "the operator key and the node key must not be the same key"
    );
}
