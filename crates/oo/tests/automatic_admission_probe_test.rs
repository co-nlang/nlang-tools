// Automatic admission + hard cap opening probe (2026-07-31).
//
// Work order: docs/automatic_admission_handover.md
//
// The controls are live at opening. The reds are deliberately ignored until
// the admission delivery. Delivery may remove only #[ignore]; it may not alter
// fixtures, assertions, or controls. This file consumes the released
// direct/relayed/unknown provenance and affiliation-root facts, but does not
// choose a hard-cap number or overflow policy.

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
const AFFILIATION_DOMAIN: &str = "oodp-affiliation:v1:";

fn fresh_dir(tag: &str) -> PathBuf {
    let mut dir = std::env::temp_dir();
    dir.push(format!(
        "nlang-automatic-admission-{tag}-{}-{}",
        std::process::id(),
        DIR_SEQ.fetch_add(1, Ordering::SeqCst)
    ));
    fs::remove_dir_all(&dir).ok();
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn oo_cmd(dir: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_oo"));
    command
        .current_dir(dir)
        .env("HOME", dir.join("home-for-tests"))
        .env("OO_IDENTITY", dir.join("identity-for-tests"))
        .env("OO_NODE_HOME", dir.join("node-home-for-tests"));
    command
}

fn oo_raw(dir: &Path, args: &[&str]) -> (bool, String) {
    let output = oo_cmd(dir).args(args).output().unwrap();
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    (output.status.success(), text.trim().to_string())
}

fn oo(dir: &Path, args: &[&str]) -> String {
    let (ok, text) = oo_raw(dir, args);
    assert!(ok, "oo {args:?} failed in {dir:?}: {text}");
    text
}

fn init(dir: &Path) {
    let (ok, text) = oo_raw(dir, &["status"]);
    assert!(ok, "opening control could not initialize {dir:?}: {text}");
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

fn first_string(output: &str) -> String {
    let s = output
        .split_once('"')
        .unwrap_or_else(|| panic!("no string atom in {output:?}"))
        .1;
    s.split('"').next().unwrap().to_string()
}

fn caid_of(dir: &Path, expr: &str) -> String {
    let caid = first_string(&oo(
        dir,
        &["eval", &format!("~%Discovery./identify {expr}")],
    ));
    assert!(
        caid.starts_with("hash:sha256:"),
        "identify returned {caid:?}"
    );
    caid
}

fn digest_of(caid: &str) -> &str {
    caid.rsplit(':').next().unwrap()
}

fn object_path(dir: &Path, caid: &str) -> PathBuf {
    let digest = digest_of(caid);
    dir.join(".oo")
        .join("objects")
        .join("sha256")
        .join(&digest[..2])
        .join(&digest[2..])
}

fn store(dir: &Path, expr: &str) -> String {
    fs::write(
        dir.join("store.n"),
        format!("id: ~%Discovery./identify_and_store {expr}\n"),
    )
    .unwrap();
    let caid = first_string(&oo(dir, &["run", "store.n", "--observe", "id"]));
    assert!(caid.starts_with("hash:sha256:"), "store returned {caid:?}");
    assert!(
        object_path(dir, &caid).exists(),
        "stored object is not on disk"
    );
    caid
}

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

struct NodeKey {
    node_id: String,
    key_pair: Ed25519KeyPair,
    public_key_hex: String,
}

fn node_key(dir: &Path) -> NodeKey {
    let output = oo(dir, &["node", "id"]);
    let node_id = output
        .lines()
        .find(|line| line.starts_with("hash:"))
        .unwrap_or_else(|| panic!("node id output has no CAID: {output:?}"))
        .trim()
        .to_string();
    let path = output
        .lines()
        .find_map(|line| line.strip_prefix("path:"))
        .unwrap_or_else(|| panic!("node id output has no key path: {output:?}"))
        .trim();
    let key_pair = Ed25519KeyPair::from_pkcs8(&fs::read(path).unwrap()).unwrap();
    NodeKey {
        node_id,
        public_key_hex: hex::encode(key_pair.public_key().as_ref()),
        key_pair,
    }
}

struct OperatorKey {
    key_pair: Ed25519KeyPair,
    public_key_hex: String,
}

fn operator_key() -> OperatorKey {
    let rng = ring::rand::SystemRandom::new();
    let pkcs8 = Ed25519KeyPair::generate_pkcs8(&rng).unwrap();
    let key_pair = Ed25519KeyPair::from_pkcs8(pkcs8.as_ref()).unwrap();
    OperatorKey {
        public_key_hex: hex::encode(key_pair.public_key().as_ref()),
        key_pair,
    }
}

fn affiliation_payload(node_id: &str, expires: i64) -> String {
    format!("{AFFILIATION_DOMAIN}{node_id}:{expires}")
}

fn affiliation_block(operator: &OperatorKey, node_id: &str, expires: i64) -> String {
    let signature = hex::encode(
        operator
            .key_pair
            .sign(affiliation_payload(node_id, expires).as_bytes())
            .as_ref(),
    );
    format!(
        ", affiliation: {{{{ operator_key: \"{}\", signature: \"{}\", expires: {} }}}}",
        operator.public_key_hex, signature, expires
    )
}

fn signed_advert(
    caid_dir: &Path,
    node: &NodeKey,
    operator: &OperatorKey,
    service: &str,
    listen_port: u16,
    expires: i64,
) -> String {
    let ts = now_secs();
    let body = format!(
        "{{{{ node_id: \"{}\", public_key: \"{}\", services: [\"{}\"], listen_port: {}, capacity: 10, ts: {}, ttl: 15{} }}}}",
        node.node_id,
        node.public_key_hex,
        service,
        listen_port,
        ts,
        affiliation_block(operator, &node.node_id, expires)
    );
    let body_caid = caid_of(caid_dir, &body);
    let signature = hex::encode(
        node.key_pair
            .sign(format!("{ADVERT_DOMAIN}{body_caid}").as_bytes())
            .as_ref(),
    );
    let inner = body.trim_start_matches("{{").trim_end_matches("}}").trim();
    format!("{{{{ {inner}, signature: \"{signature}\" }}}}")
}

fn advert_request(from: &str, ad: &str) -> String {
    format!("{{{{ %op: #advertise, %from: \"{from}\", %ad: {ad} }}}}\n")
}

fn status_of(reply: &str) -> String {
    serde_json::from_str::<serde_json::Value>(reply.trim())
        .ok()
        .and_then(|value| {
            value
                .get("%status")
                .or_else(|| value.get("status"))
                .and_then(|status| status.as_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "<none>".to_string())
        .trim_start_matches('#')
        .to_string()
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

fn free_port() -> u16 {
    for _ in 0..64 {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        if port > 22000 {
            return port;
        }
    }
    panic!("no free port above 22000");
}

fn serve(dir: &Path) -> Node {
    let port = free_port();
    let log = dir.join(format!("automatic-serve-{port}.log"));
    let file = fs::File::create(&log).unwrap();
    let child = oo_cmd(dir)
        .args(["node", "serve", "--port", &port.to_string()])
        .stdout(Stdio::from(file.try_clone().unwrap()))
        .stderr(Stdio::from(file))
        .spawn()
        .unwrap();
    let node = Node { child, port };
    for _ in 0..50 {
        std::thread::sleep(Duration::from_millis(100));
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return node;
        }
    }
    panic!("oo node serve did not come up; see {log:?}");
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
    let mut bytes = Vec::new();
    stream.read_to_end(&mut bytes).unwrap();
    String::from_utf8_lossy(&bytes).to_string()
}

struct FakePeer {
    port: u16,
    asked: Arc<Mutex<Vec<String>>>,
}

impl FakePeer {
    fn addr(&self) -> String {
        format!("tcp://127.0.0.1:{}", self.port)
    }

    fn asked(&self) -> Vec<String> {
        self.asked.lock().unwrap().clone()
    }
}

fn caid_from_request(line: &str) -> String {
    let text = line.trim();
    if text.starts_with("hash:") {
        return text.to_string();
    }
    let start = text.find("hash:sha256:").unwrap_or(0);
    let rest = &text[start..];
    let end = rest
        .find(|c: char| c == '"' || c.is_whitespace() || c == '}')
        .unwrap_or(rest.len());
    rest[..end].to_string()
}

fn spawn_peer(payload: Vec<u8>) -> FakePeer {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let asked = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&asked);
    std::thread::spawn(move || {
        for incoming in listener.incoming() {
            let Ok(mut stream) = incoming else { continue };
            let Ok(clone) = stream.try_clone() else {
                continue;
            };
            let mut line = String::new();
            if BufReader::new(clone).read_line(&mut line).is_err() {
                continue;
            }
            seen.lock().unwrap().push(caid_from_request(&line));
            let _ = stream.write_all(&payload);
            let _ = stream.flush();
            let _ = stream.shutdown(std::net::Shutdown::Write);
        }
    });
    FakePeer { port, asked }
}

struct Relayer {
    port: u16,
    asked: Arc<Mutex<Vec<String>>>,
}

impl Relayer {
    fn addr(&self) -> String {
        format!("127.0.0.1:{}", self.port)
    }

    fn contacted(&self) -> bool {
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
        for incoming in listener.incoming() {
            let Ok(mut stream) = incoming else { continue };
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

fn discover(dir: &Path, relayer: &Relayer, target: &str) -> String {
    oo(
        dir,
        &[
            "node",
            "discover",
            "--to",
            &relayer.addr(),
            "--target",
            target,
        ],
    )
}

fn peers_path(dir: &Path) -> PathBuf {
    dir.join(".oo").join("peers").join("directory")
}

fn latest_record(dir: &Path, node_id: &str) -> serde_json::Value {
    let text = fs::read_to_string(peers_path(dir)).unwrap();
    text.lines()
        .skip(1)
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|value| value.get("node_id").and_then(|v| v.as_str()) == Some(node_id))
        .last()
        .unwrap_or_else(|| panic!("no durable record for {node_id}"))
}

fn provenance_of(record: &serde_json::Value) -> Option<String> {
    record
        .get("provenance")
        .and_then(|value| value.as_str())
        .map(str::to_string)
}

fn rewrite_provenance(dir: &Path, node_id: &str, value: Option<&str>) {
    let path = peers_path(dir);
    let text = fs::read_to_string(&path).unwrap();
    let mut lines = Vec::new();
    for (index, line) in text.lines().enumerate() {
        if index == 0 || line.trim().is_empty() {
            lines.push(line.to_string());
            continue;
        }
        let mut json: serde_json::Value = serde_json::from_str(line).unwrap();
        if json.get("node_id").and_then(|v| v.as_str()) == Some(node_id) {
            let object = json.as_object_mut().unwrap();
            match value {
                Some(provenance) => {
                    object.insert(
                        "provenance".to_string(),
                        serde_json::Value::String(provenance.to_string()),
                    );
                }
                None => {
                    object.remove("provenance");
                }
            }
        }
        lines.push(serde_json::to_string(&json).unwrap());
    }
    fs::write(path, lines.join("\n") + "\n").unwrap();
}

fn fetch_unnamed(dir: &Path, caid: &str) -> String {
    fs::write(
        dir.join("fetch.n"),
        format!("got: ~%Discovery./fetch {{{{ 0: \"{caid}\" }}}}\n"),
    )
    .unwrap();
    let (_, output) = oo_raw(dir, &["run", "fetch.n", "--observe", "got"]);
    output
}

fn fetch_named(dir: &Path, name: &str, addr: &str, caid: &str) -> String {
    fs::write(
        dir.join("manual-fetch.n"),
        format!(
            "conn: ~%Discovery./connect {{{{ 0: \"{name}\", 1: \"{addr}\" }}}}\n\
             got: ~%Discovery./fetch {{{{ 0: \"{name}\", 1: \"{caid}\" }}}}\n"
        ),
    )
    .unwrap();
    let (_, output) = oo_raw(
        dir,
        &[
            "run",
            "manual-fetch.n",
            "--observe",
            "got",
            "--grant",
            "connect",
        ],
    );
    output
}

struct Fixture {
    receiver_dir: PathBuf,
    node: NodeKey,
    operator: OperatorKey,
    object_caid: String,
    payload: Vec<u8>,
    fake: FakePeer,
    ad: String,
    receiver: Option<Node>,
}

impl Fixture {
    fn direct(&mut self) {
        let receiver = serve(&self.receiver_dir);
        let reply = ask_raw(receiver.port, &advert_request(&self.node.node_id, &self.ad));
        assert_eq!(
            status_of(&reply),
            "success",
            "direct advert fixture was not accepted: {reply}"
        );
        self.receiver = Some(receiver);
    }

    fn stop_receiver(&mut self) {
        drop(self.receiver.take());
    }

    fn relay(&self, hops: i64, ad: &str) -> Relayer {
        let relayer = spawn_relayer(&self.node.node_id, hops, ad, "127.0.0.1");
        let output = discover(&self.receiver_dir, &relayer, &self.object_caid);
        assert!(
            output.contains("#success") || output.contains("success"),
            "relay fixture failed: {output}"
        );
        assert!(relayer.contacted(), "relay fixture was never contacted");
        relayer
    }
}

fn fixture(tag: &str, rooted: bool, expires: i64) -> Fixture {
    let vault = fresh_dir(&format!("{tag}-vault"));
    init(&vault);
    let object_caid = store(&vault, "{ automatic_admission: \"LIVE_PAYLOAD\" }");
    let payload = fs::read(object_path(&vault, &object_caid)).unwrap();

    let source_dir = fresh_dir(&format!("{tag}-source"));
    let node = node_key(&source_dir);
    let operator = operator_key();
    let root_key = if rooted {
        operator.public_key_hex.clone()
    } else {
        operator_key().public_key_hex
    };

    let receiver_dir = fresh_dir(&format!("{tag}-receiver"));
    write_roots(&receiver_dir, &[&root_key]);
    let fake = spawn_peer(payload.clone());
    let ad = signed_advert(
        &receiver_dir,
        &node,
        &operator,
        &object_caid,
        fake.port,
        expires,
    );

    Fixture {
        receiver_dir,
        node,
        operator,
        object_caid,
        payload,
        fake,
        ad,
        receiver: None,
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// CONTROLS — establish live, non-empty fixtures before any absence assertion.
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn c0_direct_rooted_signed_fixture_is_live() {
    let mut fixture = fixture("c0", true, now_secs() + 3600);
    fixture.direct();
    fixture.stop_receiver();

    let record = latest_record(&fixture.receiver_dir, &fixture.node.node_id);
    assert_eq!(provenance_of(&record).as_deref(), Some("direct"));
    assert_eq!(
        record.get("ad").and_then(|value| value.as_str()),
        Some(fixture.ad.as_str())
    );
    assert!(fixture
        .receiver_dir
        .join(".oo")
        .join("discovery.n")
        .exists());

    // The automatic source is not needed to prove that the payload and socket
    // are live. Manual connect is the existing consent path and is used only as
    // this control's non-empty computing payload.
    let output = fetch_named(
        &fixture.receiver_dir,
        "manual-control",
        &fixture.fake.addr(),
        &fixture.object_caid,
    );
    assert!(
        fixture
            .fake
            .asked()
            .iter()
            .any(|asked| asked == &fixture.object_caid),
        "control fake peer was never asked: {:?}",
        fixture.fake.asked()
    );
    assert!(
        output.contains("LIVE_PAYLOAD"),
        "control did not retrieve the computing payload: {output}"
    );
}

#[test]
fn c1_relay_and_legacy_records_are_live() {
    let fixture = fixture("c1", true, now_secs() + 3600);
    let relayer = fixture.relay(0, &fixture.ad);
    let relayed = latest_record(&fixture.receiver_dir, &fixture.node.node_id);
    assert_eq!(provenance_of(&relayed).as_deref(), Some("relayed"));
    assert!(!fixture
        .fake
        .asked()
        .iter()
        .any(|asked| asked == &fixture.object_caid));

    rewrite_provenance(&fixture.receiver_dir, &fixture.node.node_id, None);
    let legacy = latest_record(&fixture.receiver_dir, &fixture.node.node_id);
    assert!(provenance_of(&legacy).is_none());
    assert_eq!(
        legacy.get("ad").and_then(|value| value.as_str()),
        Some(fixture.ad.as_str())
    );
    assert!(relayer.contacted());
}

// ═══════════════════════════════════════════════════════════════════════════
// REDS — ignored until the automatic-admission delivery.
// ═══════════════════════════════════════════════════════════════════════════

#[test]
#[ignore]
fn r1_direct_rooted_admission_inserts_without_eager_dial() {
    let mut fixture = fixture("r1", true, now_secs() + 3600);
    fixture.direct();
    fixture.stop_receiver();
    assert!(
        fixture.fake.asked().is_empty(),
        "receipt eagerly dialled source"
    );

    let output = fetch_unnamed(&fixture.receiver_dir, &fixture.object_caid);
    assert!(
        fixture
            .fake
            .asked()
            .iter()
            .any(|asked| asked == &fixture.object_caid),
        "first fetch did not dial the admitted source: {:?}",
        fixture.fake.asked()
    );
    assert!(
        output.contains("LIVE_PAYLOAD"),
        "automatic source did not return the verified payload: {output}"
    );
}

#[test]
#[ignore]
fn r2_same_owner_restart_reconstructs_eligibility() {
    let mut fixture = fixture("r2", true, now_secs() + 3600);
    fixture.direct();
    fixture.stop_receiver();

    let output = fetch_unnamed(&fixture.receiver_dir, &fixture.object_caid);
    assert!(
        fixture
            .fake
            .asked()
            .iter()
            .any(|asked| asked == &fixture.object_caid),
        "restart did not reconstruct an eligible source: {:?}",
        fixture.fake.asked()
    );
    assert!(
        output.contains("LIVE_PAYLOAD"),
        "restart fetch failed: {output}"
    );
}

#[test]
#[ignore]
fn r3_copy_clears_direct_observation_for_admission() {
    let mut fixture = fixture("r3", true, now_secs() + 3600);
    fixture.direct();
    fixture.stop_receiver();

    let copy = fresh_dir("r3-copy");
    copy_tree(&fixture.receiver_dir, &copy);
    let output = fetch_unnamed(&copy, &fixture.object_caid);
    assert!(
        fixture.fake.asked().is_empty(),
        "copied workspace dialled a source from the original observer half"
    );
    assert!(
        !output.contains("LIVE_PAYLOAD"),
        "copied workspace fetched through cleared provenance: {output}"
    );
}

#[test]
#[ignore]
fn r4_relayed_zero_hops_is_not_admitted() {
    let fixture = fixture("r4", true, now_secs() + 3600);
    let _relayer = fixture.relay(0, &fixture.ad);
    let record = latest_record(&fixture.receiver_dir, &fixture.node.node_id);
    assert_eq!(provenance_of(&record).as_deref(), Some("relayed"));

    let output = fetch_unnamed(&fixture.receiver_dir, &fixture.object_caid);
    assert!(
        fixture.fake.asked().is_empty(),
        "relayed %hops:0 record became an automatic source"
    );
    assert!(
        !output.contains("LIVE_PAYLOAD"),
        "relay assertion supplied fetch content: {output}"
    );
}

#[test]
#[ignore]
fn r5_unknown_legacy_record_is_not_admitted() {
    let mut fixture = fixture("r5", true, now_secs() + 3600);
    fixture.direct();
    fixture.stop_receiver();
    rewrite_provenance(&fixture.receiver_dir, &fixture.node.node_id, None);
    assert!(provenance_of(&latest_record(&fixture.receiver_dir, &fixture.node.node_id)).is_none());

    let output = fetch_unnamed(&fixture.receiver_dir, &fixture.object_caid);
    assert!(
        fixture.fake.asked().is_empty(),
        "legacy record defaulted to an automatic direct source"
    );
    assert!(
        !output.contains("LIVE_PAYLOAD"),
        "legacy record supplied fetch content: {output}"
    );
}

#[test]
#[ignore]
fn r6_unrooted_claim_is_not_admitted() {
    let mut fixture = fixture("r6", false, now_secs() + 3600);
    fixture.direct();
    fixture.stop_receiver();

    let output = fetch_unnamed(&fixture.receiver_dir, &fixture.object_caid);
    assert!(
        fixture.fake.asked().is_empty(),
        "unrooted affiliation claim became an automatic source"
    );
    assert!(
        !output.contains("LIVE_PAYLOAD"),
        "unrooted claim supplied fetch content: {output}"
    );
}

#[test]
#[ignore]
fn r7_expired_claim_is_not_admitted() {
    let mut fixture = fixture("r7", true, now_secs() - 1);
    fixture.direct();
    fixture.stop_receiver();

    let output = fetch_unnamed(&fixture.receiver_dir, &fixture.object_caid);
    assert!(
        fixture.fake.asked().is_empty(),
        "expired affiliation claim left an automatic source behind"
    );
    assert!(
        !output.contains("LIVE_PAYLOAD"),
        "expired claim supplied fetch content: {output}"
    );
}

#[test]
#[ignore]
fn r8_newer_relayed_ad_does_not_inherit_old_direct() {
    let mut fixture = fixture("r8", true, now_secs() + 3600);
    fixture.direct();
    fixture.stop_receiver();
    assert!(fixture.fake.asked().is_empty());

    let newer_peer = spawn_peer(fixture.payload.clone());
    let newer_ad = signed_advert(
        &fixture.receiver_dir,
        &fixture.node,
        &fixture.operator,
        &fixture.object_caid,
        newer_peer.port,
        now_secs() + 3600,
    );
    let _relayer = fixture.relay(0, &newer_ad);
    let record = latest_record(&fixture.receiver_dir, &fixture.node.node_id);
    assert_eq!(
        record.get("ad").and_then(|value| value.as_str()),
        Some(newer_ad.as_str())
    );
    assert_eq!(provenance_of(&record).as_deref(), Some("relayed"));

    let output = fetch_unnamed(&fixture.receiver_dir, &fixture.object_caid);
    assert!(
        fixture.fake.asked().is_empty() && newer_peer.asked().is_empty(),
        "a newer relayed ad inherited or created an automatic source"
    );
    assert!(
        !output.contains("LIVE_PAYLOAD"),
        "newer relayed ad supplied fetch content: {output}"
    );
}

// No cap number, cap domain, or full-cap overflow behavior is encoded here.
// Those reds become satisfiable only after the user chooses those policies.
