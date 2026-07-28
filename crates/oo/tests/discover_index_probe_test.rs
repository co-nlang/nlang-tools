// The directory that answers (2026-07-28, pre-committed by work order:
// docs/discover_index_handover.md).
//
// ── The headline, measured on v0.2.50 ────────────────────────────────────
//
// One CAID, one machine A, three lines:
//
//     control 1   A serves it over the wire            → %result present   PASS
//     measure     B holds A's verified signed advert   → ⊥ #conflict       FAIL
//     control 2   B with an explicit ./connect to A    → { found_via: … }  PASS
//
// The advertise arc landed a directory that is signed, verified, addressed —
// and inert. REAL_02 §4.2.6 declares that scope, so v0.2.50 is conformant;
// this arc is what gives the directory a job. It does not make the directory a
// fetch source (that is the routing arc, and it is a consent question). It
// makes the directory *answerable*: a node that knows where A is can say so.
//
// ── §4.2.5 cannot survive discovery ──────────────────────────────────────
// §4.2.5 buys anti-reflection and anti-amplification structurally:
//
//     "一份廣告只能描述你已經在跟他講話的那一台:一個有效簽章永遠無法指名
//      第三方,反射與放大向量在結構上關閉,不靠檢查。"
//
// `#discover` **is** the operation of naming a third party. It reopens exactly
// the vector §4.2.5 closed, and no arrangement of this protocol can close it
// structurally again. What survives is a split *inside each relayed record*:
//
//   node_id, public_key,     SELF-AUTHENTICATING — inside the signature.
//   services, listen_port,   Whoever handed them to you is irrelevant.
//   ts, ttl
//
//   observed_host            ASSERTED — the relayer's observation. Never
//                            signed, because §4.2.5's whole point is that the
//                            host is observed rather than claimed, and an
//                            observation cannot travel.
//   %hops                    ASSERTED — the relayer's count.
//
// So the two strata of discussion 025 run *through the record*: a
// self-authenticating object wrapped in asserted pointers. R2 pins the first
// half; R10 and R7 pin the second, including the part that does not work.
//
// ── `ttl` is a lattice quantity this layer cannot compute ───────────────
// It has no unit because it never had one to have. §4.1's first routing
// filter is MASA_overlap = MASA_Q ⊓ MASA_Ni — **every hop is a meet** — and
// meet descends the order (A ⊓ B ⊑ A). §3.1's mass is Tr(P_C), an integer:
// the rank of a projection. A quantity that is an integer, descends under
// meet, and is naturally bounded above is exactly `ttl: @int & ..15`.
//
//     Nothing decrements it. The mathematics decrements it.
//
// And because each node recomputes it from content-addressed data, the
// original `ttl` is SELF-AUTHENTICATING — degree 0 in the sense of discussion
// 025, trusting no relayer. A hop count is the degenerate shadow left when
// the distance cannot be computed: an asserted pointer, degree ≥1, on the
// wrong side of the seam. It cannot be computed here for the reason M5
// records — d_L lives in `gbb_registry`, `ttl` lives in `peer_adverts`, and
// the two maps have never met. **The field kept the name of the mechanism and
// lost the mechanism**, which is discussion 026's disease, third instance in
// this arc.
//
// The monotonicity was also abandoned on purpose: §4.3 step 3 sends a query
// with no common MASA to a random jump and §4.4 makes those jumps mandatory,
// but a node with no common MASA is one §4.1 already filtered to W = 0. The
// jump leaves the descending chain by construction, and §7.3 says why — it is
// the Semantic Eclipse defence. A self-authenticating monotone budget and the
// ability to escape a captured neighbourhood cannot both be had; the spec
// chose escape and then needed a non-monotone net, which is what
// MAX_ROUTING_HOPS = 16 is.
//
// So these probes claim only what this layer can hold up: `ttl` is signed,
// range-checked, never modified, and `ttl: 0` means "do not relay me" —
// meaningful under either reading. `%hops` is emitted because §3.2 documents
// it and an operator reading a log wants it, and it is **not a gate**:
// comparing a relay count against a rank bound would look like a defence
// while being a category error, and it is unverifiable besides. R7 pins that
// limitation, because a gate that only records successes teaches the next
// reader the wrong thing.
//
// ── Explicitly NOT this arc ──────────────────────────────────────────────
// Kademlia (REAL_02 §4.1): no k-buckets, no XOR distance, no FIND_NODE, no
// `.oo/routing/buckets.dat`. Multi-hop relay: this arc serves `%hops: 1` only.
// Fetch sources: `~%Discovery./fetch` keeps exactly today's source set, so the
// measurement at the top of this file **stays red after this arc**, on purpose.
//
// Also not this arc, and deliberately so: the advertisement declares no
// lifetime. With `ttl` spent on propagation depth, `ts` is the only time in
// the record, and the receiver applies its own staleness bound (SPEC_13 §6.1.1
// permits a local availability policy; it forbids one that switches off
// verification). Adding a lifetime field is a spec change and it is not the
// delivery's to make.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::fs;

use ring::signature::{Ed25519KeyPair, KeyPair};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

/// Must match `oodp::ADVERT_DOMAIN`. A signature over an advertisement CAID
/// must not be replayable as a signature over anything else that hashes values.
const ADVERT_DOMAIN: &str = "oodp-advert:v1:";

/// Work order §3.6.
const MAX_DISCOVER_PEERS: usize = 8;

// ── harness ─────────────────────────────────────────────────────────────

fn fresh_dir(tag: &str) -> PathBuf {
    let mut d = std::env::temp_dir();
    d.push(format!(
        "nlang-discover-{}-{}-{}",
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

fn head_commit(dir: &Path) -> String {
    let c = oo(dir, &["log"])
        .lines()
        .find_map(|l| l.strip_prefix("commit ").map(|s| s.trim().to_string()))
        .unwrap_or_default();
    assert!(c.starts_with("hash:sha256:"), "no HEAD commit in {dir:?}");
    c
}

fn object_path(dir: &Path, caid: &str) -> PathBuf {
    let d = caid.rsplit(':').next().unwrap();
    dir.join(".oo")
        .join("objects")
        .join("sha256")
        .join(&d[..2])
        .join(&d[2..])
}

/// Hex digest of the universe root this repository's HEAD points at.
///
/// **Not the commit CAID** — commits carry timestamps and parents and are
/// *supposed* to differ between repositories. The obligation of SPEC_13 §4.1.2
/// #3 is about the root the commit points at. Anything unusable panics rather
/// than degrading to a value that could compare equal to another failure.
fn root_digest(dir: &Path) -> String {
    let p = object_path(dir, &head_commit(dir));
    let commit: serde_json::Value =
        serde_json::from_slice(&fs::read(&p).unwrap_or_else(|e| panic!("{p:?}: {e}"))).unwrap();
    let dg = commit["root"]["digest"].clone();
    let hex = if let Some(s) = dg.as_str() {
        s.to_string()
    } else if let Some(a) = dg.as_array() {
        a.iter()
            .map(|b| format!("{:02x}", b.as_u64().expect("digest byte")))
            .collect()
    } else {
        panic!("commit root has no usable `digest` field: {}", commit["root"]);
    };
    assert!(
        hex.len() == 64 && hex.chars().all(|c| c.is_ascii_hexdigit()),
        "root digest is not a 64-hex string: {hex:?}"
    );
    hex
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
    public_key_hex: String,
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
            public_key_hex: nk.public_key_hex.clone(),
            services: vec![],
            listen_port,
            capacity: 10,
            ts: now_secs(),
            ttl: 15,
        }
    }

    fn serving(mut self, caid: &str) -> Self {
        self.services.push(caid.to_string());
        self
    }

    fn ttl(mut self, ttl: i64) -> Self {
        self.ttl = ttl;
        self
    }

    /// The signed body: the advertisement **before** `signature` is added.
    fn body(&self) -> String {
        let services = self
            .services
            .iter()
            .map(|s| format!("\"{s}\""))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "{{{{ node_id: \"{}\", public_key: \"{}\", services: [{services}], \
             listen_port: {}, capacity: {}, ts: {}, ttl: {} }}}}",
            self.node_id, self.public_key_hex, self.listen_port, self.capacity, self.ts, self.ttl
        )
    }

    /// Full advertisement, signed by `signer` over the CAID of `body()`.
    fn signed(&self, caid_dir: &Path, signer: &Ed25519KeyPair) -> String {
        let body = self.body();
        let caid = caid_of(caid_dir, &body);
        let payload = format!("{ADVERT_DOMAIN}{caid}");
        let sig = hex::encode(signer.sign(payload.as_bytes()).as_ref());
        let inner = body.trim_start_matches("{{").trim_end_matches("}}").trim();
        format!("{{{{ {inner}, signature: \"{sig}\" }}}}")
    }
}

fn advert_request(from: &str, ad: &str) -> String {
    format!("{{{{ %op: #advertise, %from: \"{from}\", %ad: {ad} }}}}\n")
}

fn discover_request(from: &str, target: &str) -> String {
    format!("{{{{ %op: #discover, %from: \"{from}\", %target: \"{target}\" }}}}\n")
}

// ── relayed-record surgery (probe side only) ────────────────────────────

/// Value of a `key: "…"` field in an n/ advertisement source.
fn field_str(ad_src: &str, key: &str) -> String {
    let needle = format!("{key}: \"");
    let i = ad_src
        .find(&needle)
        .unwrap_or_else(|| panic!("no `{key}` in {ad_src}"));
    let rest = &ad_src[i + needle.len()..];
    rest.split('"').next().unwrap().to_string()
}

/// The advertisement with its `signature` field removed — what §4.2.1 says the
/// signature commits to. The probe reconstructs it from the source text alone,
/// which is the whole point: a relayed record must be verifiable from the
/// packet and nothing else.
fn body_without_signature(ad_src: &str) -> String {
    let i = ad_src
        .find(", signature:")
        .unwrap_or_else(|| panic!("no signature field in {ad_src}"));
    format!("{} }}}}", &ad_src[..i])
}

/// Replaces one `key: "…"` field's value, leaving everything else byte-identical.
fn replace_field(ad_src: &str, key: &str, new_value: &str) -> String {
    let old = field_str(ad_src, key);
    ad_src.replace(
        &format!("{key}: \"{old}\""),
        &format!("{key}: \"{new_value}\""),
    )
}

/// Verifies a relayed advertisement **using only what is in it**: recompute the
/// CAID of the body without `signature`, prefix the domain, check Ed25519
/// against the embedded `public_key`. This is what a receiver that has never
/// spoken to the advertiser has to be able to do.
fn verify_relayed(caid_dir: &Path, ad_src: &str) -> Result<(), String> {
    let pk_hex = field_str(ad_src, "public_key");
    let sig_hex = field_str(ad_src, "signature");
    let pk = hex::decode(&pk_hex).map_err(|e| format!("public_key not hex: {e}"))?;
    let sig = hex::decode(&sig_hex).map_err(|e| format!("signature not hex: {e}"))?;
    let caid = caid_of(caid_dir, &body_without_signature(ad_src));
    let payload = format!("{ADVERT_DOMAIN}{caid}");
    ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, &pk)
        .verify(payload.as_bytes(), &sig)
        .map_err(|_| "signature does not verify".to_string())
}

// ── running node ────────────────────────────────────────────────────────

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
    fn addr(&self) -> String {
        format!("127.0.0.1:{}", self.port)
    }
}

/// Ports above 21000: earlier arcs' probe runs leave `oo node serve` processes
/// listening in the 19000s on developer machines, and a probe that connects to
/// a "free" port can hit one (work order §8 item 8).
fn free_port() -> u16 {
    for _ in 0..64 {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let p = l.local_addr().unwrap().port();
        if p > 21000 {
            return p;
        }
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

fn field_of(reply: &str, key: &str) -> Option<String> {
    let j: serde_json::Value = serde_json::from_str(reply.trim()).ok()?;
    let v = j.get(key).or_else(|| j.get(key.trim_start_matches('%')))?;
    Some(v.as_str()?.trim().trim_start_matches('#').to_string())
}

fn status_of(reply: &str) -> String {
    field_of(reply, "%status").unwrap_or_else(|| "<none>".into())
}

fn hops_of(reply: &str) -> i64 {
    serde_json::from_str::<serde_json::Value>(reply.trim())
        .ok()
        .and_then(|j| j.get("%hops").and_then(|v| v.as_i64()))
        .unwrap_or(-1)
}

/// One entry of `%peers`. `ad` is the advertisement as n/ source, byte-for-byte
/// as the advertiser wrote it (work order §3.3).
#[derive(Clone, Debug)]
struct Relayed {
    ad: String,
    observed_host: String,
}

fn peers_of(reply: &str) -> Vec<Relayed> {
    let Ok(j) = serde_json::from_str::<serde_json::Value>(reply.trim()) else {
        return vec![];
    };
    let Some(arr) = j.get("%peers").and_then(|v| v.as_array()) else {
        return vec![];
    };
    arr.iter()
        .map(|e| Relayed {
            ad: e
                .get("%ad")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            observed_host: e
                .get("%observed_host")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
        })
        .collect()
}

// ── fake relayer ────────────────────────────────────────────────────────

/// A node that answers `#discover` with whatever the probe wants — including
/// answers a correct engine would never produce. This is the *querying* side's
/// adversary: everything a real relayer sends is remote input, and a reply to
/// a request you initiated is exactly the input people forget to distrust.
struct Relayer {
    port: u16,
    asked: Arc<Mutex<Vec<String>>>,
}

impl Relayer {
    fn addr(&self) -> String {
        format!("127.0.0.1:{}", self.port)
    }
    fn asked(&self) -> Vec<String> {
        self.asked.lock().unwrap().clone()
    }
}

fn spawn_relayer(reply: String) -> Relayer {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let asked = Arc::new(Mutex::new(Vec::new()));
    let log = Arc::clone(&asked);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let Ok(clone) = stream.try_clone() else { continue };
            let mut line = String::new();
            if BufReader::new(clone).read_line(&mut line).is_err() {
                continue;
            }
            log.lock().unwrap().push(line.trim().to_string());
            let _ = stream.write_all(reply.as_bytes());
            let _ = stream.write_all(b"\n");
            let _ = stream.flush();
            let _ = stream.shutdown(std::net::Shutdown::Write);
        }
    });
    Relayer { port, asked }
}

/// Builds a `#discover` reply carrying `entries` as (advert source, host).
fn relay_reply(source_id: &str, hops: i64, entries: &[(String, String)]) -> String {
    let peers: Vec<serde_json::Value> = entries
        .iter()
        .map(|(ad, host)| serde_json::json!({ "%ad": ad, "%observed_host": host }))
        .collect();
    serde_json::json!({
        "%status": "#success",
        "%source": source_id,
        "%hops": hops,
        "%peers": peers,
    })
    .to_string()
}

/// `oo node discover --to … --target …` on the querying side.
fn oo_discover(dir: &Path, to: &str, target: &str) -> String {
    oo(dir, &["node", "discover", "--to", to, "--target", target])
}

// ── workspaces ──────────────────────────────────────────────────────────

/// A: advertises. B: the index that answers. C: the querier.
struct Trio {
    a: PathBuf,
    b: PathBuf,
    c: PathBuf,
}

fn trio(tag: &str) -> Trio {
    let a = fresh_dir(&format!("{tag}-a"));
    let b = fresh_dir(&format!("{tag}-b"));
    let c = fresh_dir(&format!("{tag}-c"));
    init(&a);
    init(&b);
    init(&c);
    Trio { a, b, c }
}

/// A advertises `caid` to the node at `port`, using the real CLI. Asserts it
/// landed — a discover probe whose advertise silently failed measures nothing.
fn advertise(from: &Path, port: u16, caid: &str, listen_port: u16) {
    let out = oo(
        from,
        &[
            "node",
            "advertise",
            "--to",
            &format!("127.0.0.1:{port}"),
            "--service",
            caid,
            "--listen-port",
            &listen_port.to_string(),
        ],
    );
    assert!(
        out.contains("#success"),
        "LIVENESS: the advertisement this probe depends on did not land: {out}"
    );
}

// ════════════════════════════════════════════════════════════════════════
//  RED — must fail on v0.2.50, for the reason stated
// ════════════════════════════════════════════════════════════════════════

/// R1 — an index answers with the peer that advertised the target.
///
/// Baseline: `#discover` answers `#not_implemented` (`oodp.rs:328`), so
/// `%peers` is absent and the list is empty.
#[test]
fn r1_index_answers_with_the_advertiser() {
    let t = trio("r1");
    let b = serve(&t.b);
    let caid = store(&t.a, "{ treasure: \"R1\" }");
    advertise(&t.a, b.port, &caid, 21730);

    let r = ask_raw(b.port, &discover_request("whoever", &caid));
    assert_eq!(status_of(&r), "success", "discover reply: {r}");

    let peers = peers_of(&r);
    assert_eq!(peers.len(), 1, "expected exactly the advertiser: {r}");

    let a_id = node_key(&t.a).node_id;
    assert!(
        peers[0].ad.contains(&a_id),
        "the relayed record does not name A ({a_id}): {r}"
    );
    assert!(
        peers[0].ad.contains(&caid),
        "the relayed record does not carry the advertised service: {r}"
    );
}

/// R2 — the relayed record is verifiable from the packet alone.
///
/// The check uses nothing but the bytes that came back: the embedded
/// `public_key`, the CAID of the body without `signature`, the domain prefix.
/// That is what a party which has never spoken to A must be able to do, and it
/// is only possible if the relay is verbatim.
///
/// Baseline: nothing to verify — `#not_implemented`.
#[test]
fn r2_relayed_record_verifies_from_the_packet_alone() {
    let t = trio("r2");
    let b = serve(&t.b);
    let caid = store(&t.a, "{ treasure: \"R2\" }");
    advertise(&t.a, b.port, &caid, 21731);

    let r = ask_raw(b.port, &discover_request("whoever", &caid));
    let peers = peers_of(&r);
    assert_eq!(peers.len(), 1, "no record to verify: {r}");
    assert_eq!(
        peers[0].observed_host, "127.0.0.1",
        "the host must be the one the index OBSERVED on A's connection, not \
         anything A said (§4.2.5): {r}"
    );
    assert_eq!(hops_of(&r), 1, "a relayed record has travelled one hop: {r}");

    // Verified with packet contents only — `t.c` is merely a workspace to ask
    // for the canonical CAID encoding, and holds nothing about A.
    verify_relayed(&t.c, &peers[0].ad)
        .unwrap_or_else(|e| panic!("relayed record does not verify ({e}): {}", peers[0].ad));

    // …and it really is A's record, not a well-formed record about nobody.
    let a = node_key(&t.a);
    assert_eq!(
        field_str(&peers[0].ad, "node_id"),
        a.node_id,
        "relayed node_id is not A's"
    );
    assert_eq!(
        field_str(&peers[0].ad, "public_key"),
        a.public_key_hex,
        "relayed public_key is not A's"
    );
}

/// R3 — a tampered signature drops **that entry**, and the good entry in the
/// same response survives (work order R-e).
///
/// Pairwise on purpose: an engine that rejects the whole response passes the
/// first half and fails the second, and an engine that verifies nothing passes
/// the second and fails the first.
///
/// Baseline: `oo node discover` does not exist.
#[test]
fn r3_tampered_signature_drops_only_itself() {
    let t = trio("r3");
    let caid = store(&t.a, "{ treasure: \"R3\" }");
    let a = node_key(&t.a);
    let d = node_key(&t.b); // stands in for a second advertiser

    let good = Advert::new(&a, 21732).serving(&caid).signed(&t.a, &a.key_pair);
    let honest = Advert::new(&d, 21733).serving(&caid).signed(&t.b, &d.key_pair);
    // Flip one hex digit of the signature: still well-formed, no longer valid.
    let sig = field_str(&honest, "signature");
    let flipped = format!(
        "{}{}",
        if sig.starts_with('a') { 'b' } else { 'a' },
        &sig[1..]
    );
    let bad = replace_field(&honest, "signature", &flipped);

    let relayer = spawn_relayer(relay_reply(
        &d.node_id,
        1,
        &[
            (good, "198.51.100.1".into()),
            (bad, "198.51.100.2".into()),
        ],
    ));

    let out = oo_discover(&t.c, &relayer.addr(), &caid);
    assert!(
        out.contains(&a.node_id),
        "the good record was not accepted: {out}"
    );
    assert!(
        !out.contains(&d.node_id),
        "a record with an invalid signature was accepted: {out}"
    );
}

/// R4 — `node_id ≠ CAID(public_key)` drops that entry, good entry survives.
///
/// The forged record is signed correctly **by the key it carries** — a forger
/// supplies both — and differs only in claiming to be someone else. §4.2.2's
/// step 2 exists for exactly this, and it must apply to relayed records too.
///
/// Baseline: `oo node discover` does not exist.
#[test]
fn r4_identity_mismatch_drops_only_itself() {
    let t = trio("r4");
    let caid = store(&t.a, "{ treasure: \"R4\" }");
    let a = node_key(&t.a);
    let d = node_key(&t.b);

    let good = Advert::new(&a, 21734).serving(&caid).signed(&t.a, &a.key_pair);

    // D's key, D's signature — but claiming A's node_id. Nothing about the
    // signature is wrong; the binding is.
    let mut forged = Advert::new(&d, 21735).serving(&caid);
    forged.node_id = a.node_id.clone();
    let forged = forged.signed(&t.b, &d.key_pair);
    assert!(
        verify_relayed(&t.c, &forged).is_ok(),
        "probe error: the forgery must carry a VALID signature, or R4 measures R3"
    );

    let relayer = spawn_relayer(relay_reply(
        &d.node_id,
        1,
        &[
            (good, "198.51.100.1".into()),
            (forged, "198.51.100.9".into()),
        ],
    ));

    let out = oo_discover(&t.c, &relayer.addr(), &caid);
    assert!(
        out.contains(&a.node_id),
        "the good record was not accepted: {out}"
    );
    assert!(
        !out.contains("198.51.100.9"),
        "a record whose node_id does not hash from its public_key was accepted: {out}"
    );
}

/// R5 — a relayed advertisement whose body **computes**.
///
/// The v0.2.50 lesson, applied to the new entry point. That arc's own gate sent
/// `%ad: 7` — a scalar that evaluates harmlessly — and the real defect (the
/// engine evaluated the body before any of the five checks, giving any
/// unauthenticated peer an arbitrary effect) walked straight past it.
///
///     An adversarial case at a remote-input entry point must include a
///     payload that computes, not only payloads of the wrong shape.
///
/// A good record travels in the same response, so a delivery that simply never
/// processes anything cannot pass this by doing nothing.
///
/// Baseline: `oo node discover` does not exist, so the good record is not
/// accepted either — red on the liveness half.
#[test]
fn r5_a_relayed_body_that_computes_is_refused_before_it_runs() {
    let t = trio("r5");
    let caid = store(&t.a, "{ treasure: \"R5\" }");
    let a = node_key(&t.a);

    let good = Advert::new(&a, 21736).serving(&caid).signed(&t.a, &a.key_pair);

    let loot = t.c.join("pwned-by-discover.txt");
    assert!(!loot.exists(), "probe error: loot path already exists");
    let payload = format!(
        "~%Io./write_file(\"{}\", \"owned via #discover\")",
        loot.display()
    );

    let relayer = spawn_relayer(relay_reply(
        &a.node_id,
        1,
        &[
            (good, "198.51.100.1".into()),
            (payload, "198.51.100.3".into()),
        ],
    ));

    let out = oo_discover(&t.c, &relayer.addr(), &caid);

    assert!(
        !loot.exists(),
        "a relayed advertisement body was EVALUATED: {} exists after {out}",
        loot.display()
    );
    // Two independent liveness witnesses. Without them, "the file is absent"
    // is exactly the vacuous pass this arc's own standing rule exists to stop:
    // a querier that never connects, or never parses, also writes no file.
    assert!(
        !relayer.asked().is_empty(),
        "LIVENESS: the querier never contacted the relayer, so the absence of \
         the file proves nothing: {out}"
    );
    assert!(
        out.contains(&a.node_id),
        "LIVENESS: the good record in the same response was not accepted, so \
         the absence of the file proves nothing: {out}"
    );
}

/// R6 — `ttl: 0` means "do not relay me", and `ttl: 1` from the same directory
/// is still relayed.
///
/// Both advertisements are valid, both are accepted by `#advertise`, both name
/// the same service. Only the budget differs. Pairwise so that a delivery
/// which relays nothing, or ignores `ttl` entirely, fails.
///
/// Baseline: `#not_implemented` — neither is relayed, so the second half fails.
#[test]
fn r6_ttl_zero_is_not_relayed_and_ttl_one_is() {
    let t = trio("r6");
    let b = serve(&t.b);
    let caid = store(&t.a, "{ treasure: \"R6\" }");
    let a = node_key(&t.a);
    let quiet = node_key(&t.c);

    let relayable = Advert::new(&a, 21737)
        .serving(&caid)
        .ttl(1)
        .signed(&t.a, &a.key_pair);
    let no_relay = Advert::new(&quiet, 21738)
        .serving(&caid)
        .ttl(0)
        .signed(&t.c, &quiet.key_pair);

    for (who, ad) in [(&a, &relayable), (&quiet, &no_relay)] {
        let r = ask_raw(b.port, &advert_request(&who.node_id, ad));
        assert_eq!(
            status_of(&r),
            "success",
            "LIVENESS: both advertisements must be ACCEPTED — ttl 0 is a valid \
             budget, not a malformed record: {r}"
        );
    }

    let r = ask_raw(b.port, &discover_request("whoever", &caid));
    let ads: Vec<String> = peers_of(&r).into_iter().map(|p| p.ad).collect();
    let joined = ads.join("\n");

    assert!(
        joined.contains(&a.node_id),
        "the ttl:1 record was not relayed: {r}"
    );
    assert!(
        !joined.contains(&quiet.node_id),
        "a ttl:0 record was relayed — `do not relay me` is being ignored: {r}"
    );
}

/// R7 — the relay bound binds the honest index, and **not the wire**.
///
/// Two halves, and the second one is the point:
///
///   * an honest index will not emit a record whose signed `ttl` is 0;
///   * a dishonest relayer hands you that same record anyway and is believed,
///     because nothing in the packet says where it came from. `%hops` is the
///     relayer's own number, outside the signature, with nothing to check it
///     against — and it is not a quantity commensurable with `ttl` in the
///     first place (see the header).
///
/// This probe exists so that no later reader mistakes the bound for a defence.
/// The receiver's protection at this layer is its own budget, not the sender's.
///
/// Baseline: neither half runs — `#not_implemented`, and no `oo node discover`.
#[test]
fn r7_the_relay_bound_binds_the_honest_index_only() {
    let t = trio("r7");
    let caid = store(&t.a, "{ treasure: \"R7\" }");
    let a = node_key(&t.a);

    // Half 1 — honest index refuses to emit a ttl:0 record.
    let b = serve(&t.b);
    let spent = Advert::new(&a, 21739)
        .serving(&caid)
        .ttl(0)
        .signed(&t.a, &a.key_pair);
    let r = ask_raw(b.port, &advert_request(&a.node_id, &spent));
    assert_eq!(status_of(&r), "success", "advertise must accept ttl:0: {r}");
    let r = ask_raw(b.port, &discover_request("whoever", &caid));
    assert_eq!(status_of(&r), "success", "discover reply: {r}");
    assert!(
        peers_of(&r).is_empty(),
        "an honest index relayed a record marked `do not relay me`: {r}"
    );

    // Half 2 — a relayer that hands you the very same record is believed. The
    // record is genuine and its signature is valid; what cannot be checked is
    // who passed it on, and `%hops` is that relayer's own number.
    let relayer = spawn_relayer(relay_reply(&a.node_id, 0, &[(spent, "198.51.100.7".into())]));
    let out = oo_discover(&t.c, &relayer.addr(), &caid);
    assert!(
        out.contains(&a.node_id),
        "R7's second half records a LIMITATION, not a defence: a relayer that \
         passes on a ttl:0 record must be believed, because nothing in the \
         packet can contradict it, and %hops is not a quantity commensurable \
         with ttl anyway. If this now fails, the engine has acquired a check \
         it cannot actually perform — report it, do not 'fix' the probe: {out}"
    );
}

/// R8 — `ttl` outside `0..=15` is refused at the advertise entry point.
///
/// §4.2 spells the bound (`@int & ..15`) and v0.2.50 never checks it
/// (`field_as_i64(&cv, "ttl").unwrap_or(0)`). Harmless while nothing relays;
/// once relaying exists, an unbounded budget is an unbounded flood.
///
/// Baseline: accepted — `#success` for every value.
#[test]
fn r8_ttl_range_is_enforced() {
    let t = trio("r8");
    let b = serve(&t.b);
    let a = node_key(&t.a);

    for bad in [-1_i64, 16, 999_999_999] {
        let ad = Advert::new(&a, 21740).ttl(bad).signed(&t.a, &a.key_pair);
        let r = ask_raw(b.port, &advert_request(&a.node_id, &ad));
        assert_eq!(
            status_of(&r),
            "rejected",
            "ttl {bad} is outside §4.2's `..15` and must be refused: {r}"
        );
        assert_eq!(
            field_of(&r, "%reason").unwrap_or_else(|| "<absent>".into()),
            "malformed",
            "an out-of-range ttl is a malformed record, not a signature or \
             identity failure: {r}"
        );
    }

    // …and the boundary values are still accepted.
    for ok in [0_i64, 15] {
        let ad = Advert::new(&a, 21741).ttl(ok).signed(&t.a, &a.key_pair);
        let r = ask_raw(b.port, &advert_request(&a.node_id, &ad));
        assert_eq!(status_of(&r), "success", "ttl {ok} is in range: {r}");
    }
}

/// R9 — the amplification bound holds.
///
/// §4.2.5 closed reflection and amplification *structurally*. `#discover`
/// reopens them by construction, so the close has to be a budget instead. A
/// directory of twelve matching peers must not become a twelve-peer answer to
/// a one-line question.
///
/// Baseline: `#not_implemented`, so nothing is emitted and the lower bound
/// fails.
#[test]
fn r9_the_response_is_capped() {
    let t = trio("r9");
    let b = serve(&t.b);
    let caid = store(&t.a, "{ treasure: \"R9\" }");

    // Twelve distinct advertisers, all claiming the same service.
    let mut advertisers = Vec::new();
    for i in 0..12 {
        let d = fresh_dir(&format!("r9-peer{i}"));
        init(&d);
        let k = node_key(&d);
        let ad = Advert::new(&k, 21750 + i as u16)
            .serving(&caid)
            .signed(&d, &k.key_pair);
        let r = ask_raw(b.port, &advert_request(&k.node_id, &ad));
        assert_eq!(status_of(&r), "success", "advertiser {i} not accepted: {r}");
        advertisers.push(k.node_id);
    }

    let req = discover_request("whoever", &caid);
    let r = ask_raw(b.port, &req);
    let peers = peers_of(&r);

    assert!(
        !peers.is_empty(),
        "LIVENESS: twelve advertisements landed and none came back: {r}"
    );
    assert!(
        peers.len() <= MAX_DISCOVER_PEERS,
        "response carries {} peers, cap is {MAX_DISCOVER_PEERS}: {r}",
        peers.len()
    );

    // The number the work order asks the delivery to report.
    let ratio = r.len() as f64 / req.len() as f64;
    assert!(
        ratio < 64.0,
        "amplification ratio {ratio:.1}× (response {} bytes / request {} bytes) \
         — §4.2.5's structural bound was replaced by a budget, and the budget \
         has to actually bound something",
        r.len(),
        req.len()
    );
}

/// R10 — `%observed_host` is outside the signature, and the human is told.
///
/// Two halves. First, the mechanical one: rewriting the host does not disturb
/// signature verification, because the host was never signed — §4.2.5 makes
/// the host an *observation*, and an observation cannot travel. Second, the
/// one that matters at 3am: the operator-facing output says so.
///
/// Baseline: `oo node discover` does not exist.
#[test]
fn r10_observed_host_is_outside_the_signature_and_says_so() {
    let t = trio("r10");
    let caid = store(&t.a, "{ treasure: \"R10\" }");
    let a = node_key(&t.a);
    let ad = Advert::new(&a, 21742).serving(&caid).signed(&t.a, &a.key_pair);

    // The relayer names a host that has nothing to do with A. The record still
    // verifies — that IS the finding, not a bug.
    verify_relayed(&t.c, &ad).expect("probe error: the record must verify");
    let relayer = spawn_relayer(relay_reply(&a.node_id, 1, &[(ad, "203.0.113.99".into())]));

    let out = oo_discover(&t.c, &relayer.addr(), &caid);
    assert!(
        out.contains(&a.node_id),
        "LIVENESS: the record was not accepted at all: {out}"
    );
    assert!(
        out.contains("203.0.113.99"),
        "the relayer's host claim is not surfaced: {out}"
    );
    assert!(
        out.contains("21742"),
        "the signed listen_port is not surfaced: {out}"
    );
    assert!(
        out.contains("host unverified"),
        "the one place a human reads this must say the host is not verified: {out}"
    );
}

/// R11 — the answer does not depend on who asks.
///
/// REAL_02 §3.2 left `%from` on `#discover` undecided. It is decided here the
/// same way it is decided on `#fetch`: a claim. A discover answer is a
/// re-broadcast of public signed records, so making it depend on the asker
/// buys nothing and creates a partition surface.
///
/// Baseline: all three replies are `#not_implemented`, hence identical — and
/// the liveness assertion is what makes this red rather than vacuously green.
#[test]
fn r11_the_answer_does_not_depend_on_who_asks() {
    let t = trio("r11");
    let b = serve(&t.b);
    let caid = store(&t.a, "{ treasure: \"R11\" }");
    let a = node_key(&t.a);
    advertise(&t.a, b.port, &caid, 21743);

    let mut bodies = Vec::new();
    for from in ["", &a.node_id, "hash:sha256:v1:not-a-real-node"] {
        let r = ask_raw(b.port, &discover_request(from, &caid));
        assert!(
            r.contains(&a.node_id),
            "LIVENESS: %from={from:?} produced no record, so 'identical' would \
             mean 'identically empty': {r}"
        );
        // %source names the responder and is legitimately constant; strip
        // nothing else — this is a byte comparison of the answer.
        bodies.push(peers_of(&r));
    }

    for i in 1..bodies.len() {
        assert_eq!(
            format!("{:?}", bodies[0]),
            format!("{:?}", bodies[i]),
            "the discover answer changed with %from — nothing may branch on a \
             claim (REAL_02 §3.2)"
        );
    }
}

/// R12 — "nobody I know of" is an answer, and it is not "I have no index".
///
/// v0.2.48's finding was that collapsing distinguishable situations into one
/// silence is the defect. `#not_found` here would collapse *no matching peer*
/// into *no discovery service*, and a client cannot tell whether to ask
/// somebody else or to stop asking this node anything.
///
/// Baseline: `#not_implemented` for every target.
#[test]
fn r12_no_match_is_success_with_an_empty_list() {
    let t = trio("r12");
    let b = serve(&t.b);
    let known = store(&t.a, "{ treasure: \"R12-known\" }");
    let unknown = store(&t.a, "{ treasure: \"R12-unknown\" }");
    advertise(&t.a, b.port, &known, 21744);

    // LIVENESS: this index does answer, for something it knows.
    let hit = ask_raw(b.port, &discover_request("whoever", &known));
    assert!(
        !peers_of(&hit).is_empty(),
        "LIVENESS: the index answered nothing even for a service it holds: {hit}"
    );

    let miss = ask_raw(b.port, &discover_request("whoever", &unknown));
    assert_eq!(
        status_of(&miss),
        "success",
        "no matching peer is an answer, not an absence of service: {miss}"
    );
    assert!(
        peers_of(&miss).is_empty(),
        "a target nobody advertised came back with peers: {miss}"
    );
}

// ════════════════════════════════════════════════════════════════════════
//  PINS — green on v0.2.50, must stay green
// ════════════════════════════════════════════════════════════════════════

/// P1 — the advertise ladder is intact, and §3.8's new range check does not
/// refuse anything that was valid before.
///
/// The full `advertise_wire_probe_test` suite is the real pin; this is the
/// tripwire that fires inside this file.
#[test]
fn p1_advertise_ladder_intact() {
    let t = trio("p1");
    let b = serve(&t.b);
    let a = node_key(&t.a);
    let caid = store(&t.a, "{ treasure: \"P1\" }");

    let ok = Advert::new(&a, 21745).serving(&caid).signed(&t.a, &a.key_pair);
    let r = ask_raw(b.port, &advert_request(&a.node_id, &ok));
    assert_eq!(status_of(&r), "success", "a valid advertisement: {r}");

    // Envelope and payload must still agree about who is speaking.
    let r = ask_raw(b.port, &advert_request("hash:sha256:v1:somebody-else", &ok));
    assert_eq!(status_of(&r), "rejected", "{r}");
    assert_eq!(
        field_of(&r, "%reason").unwrap_or_default(),
        "identity_mismatch",
        "{r}"
    );

    // A body that computes is still refused before it runs (v0.2.50's repair).
    let loot = t.b.join("p1-must-not-exist.txt");
    let bomb = format!(
        "{{{{ %op: #advertise, %from: \"{}\", %ad: ~%Io./write_file(\"{}\", \"x\") }}}}\n",
        a.node_id,
        loot.display()
    );
    let r = ask_raw(b.port, &bomb);
    assert_eq!(status_of(&r), "rejected", "{r}");
    assert!(!loot.exists(), "the v0.2.50 repair regressed: {}", loot.display());
}

/// P2 — `#fetch` is untouched: it serves, and its outcome is independent of
/// `%from`.
#[test]
fn p2_fetch_untouched() {
    let t = trio("p2");
    let caid = store(&t.a, "{ treasure: \"P2\" }");
    let a = serve(&t.a);

    let plain = ask_raw(a.port, &format!("{{{{ %op: #fetch, %hash: \"{caid}\" }}}}\n"));
    assert_eq!(status_of(&plain), "success", "{plain}");

    for from in ["", "hash:sha256:v1:whoever", "not-even-a-caid"] {
        let r = ask_raw(
            a.port,
            &format!("{{{{ %op: #fetch, %from: \"{from}\", %hash: \"{caid}\" }}}}\n"),
        );
        assert_eq!(status_of(&r), "success", "%from={from:?}: {r}");
        assert_eq!(
            hops_of(&r),
            0,
            "a direct answer is 0 hops (REAL_02 §3.2): {r}"
        );
    }
}

/// P3 — the directory does not enter the universe.
///
/// SPEC_13 §4.1.2 obligation #3: no engine-local, non-deterministic value may
/// be minted into universe content, and the test of that is that two fresh
/// workspaces evolving the same source agree on the root CAID. This arc adds
/// engine-local state that is *about other machines*; if any of it reached the
/// root, two nodes with different peers could never share a universe identity.
#[test]
fn p3_the_directory_never_reaches_the_root() {
    let src = "world: {\n  greet: \"hello\"\n  n: 7\n}\n";

    let quiet = fresh_dir("p3-quiet");
    init(&quiet);
    write(&quiet, "u.n", src);
    oo(&quiet, &["evolve", "u.n"]);
    let root_quiet = oo(&quiet, &["commit", "-m", "p3"]);

    let busy = fresh_dir("p3-busy");
    init(&busy);
    let b = serve(&busy);
    // Give it peers to know about, and questions to answer about them.
    let ad_src = fresh_dir("p3-peer");
    init(&ad_src);
    let caid = store(&ad_src, "{ treasure: \"P3\" }");
    let k = node_key(&ad_src);
    let ad = Advert::new(&k, 21746).serving(&caid).signed(&ad_src, &k.key_pair);
    let r = ask_raw(b.port, &advert_request(&k.node_id, &ad));
    assert_eq!(status_of(&r), "success", "P3 setup advertisement: {r}");
    ask_raw(b.port, &discover_request("whoever", &caid));

    write(&busy, "u.n", src);
    oo(&busy, &["evolve", "u.n"]);
    let root_busy = oo(&busy, &["commit", "-m", "p3"]);
    assert!(
        root_quiet.contains("hash:") && root_busy.contains("hash:"),
        "LIVENESS: a commit did not happen: {root_quiet} / {root_busy}"
    );

    assert_eq!(
        root_digest(&quiet),
        root_digest(&busy),
        "a node that has met peers committed a different universe than one that \
         has not — engine-local state reached the root (SPEC_13 §4.1.2 #3)"
    );
}

/// P4 — local LADD is untouched. `~%Discovery./find` against an empty registry
/// is `⊥ #missing_key` with "No matching peers found"; the wire directory is a
/// different map and must not start feeding it in this arc.
#[test]
fn p4_local_ladd_untouched() {
    let t = trio("p4");
    let b = serve(&t.b);
    let caid = store(&t.a, "{ treasure: \"P4\" }");
    advertise(&t.a, b.port, &caid, 21747);

    let out = oo(&t.b, &["eval", "~%Discovery./find {{ treasure: \"P4\" }}"]);
    assert!(
        out.contains("missing_key") && out.contains("No matching peers found"),
        "local find changed after the node accepted an advertisement — the wire \
         directory and the gravitational registry are still separate maps in \
         this arc: {out}"
    );
}

/// P5 — accepting advertisements and answering discovers writes no objects.
///
/// An unsolicited packet must never cause a store write (REAL_02 §4.2.6).
#[test]
fn p5_serving_discovery_stores_nothing() {
    let t = trio("p5");
    let b = serve(&t.b);
    let before = object_count(&t.b);

    let caid = store(&t.a, "{ treasure: \"P5\" }");
    let a = node_key(&t.a);
    let ad = Advert::new(&a, 21748).serving(&caid).signed(&t.a, &a.key_pair);
    let r = ask_raw(b.port, &advert_request(&a.node_id, &ad));
    assert_eq!(status_of(&r), "success", "{r}");
    ask_raw(b.port, &discover_request("whoever", &caid));
    ask_raw(b.port, &discover_request("whoever", "hash:sha256:v1:nothing"));

    assert_eq!(
        object_count(&t.b),
        before,
        "the index wrote objects into its store while answering questions"
    );
    assert!(
        !t.b.join(".oo").join("routing").exists(),
        "`.oo/routing/` appeared — REAL_02 §5.1 sketches it, this arc does not \
         deliver it, and an undeclared file is how a directory becomes durable \
         without anyone deciding that it should"
    );
}
