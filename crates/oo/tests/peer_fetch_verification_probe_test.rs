// Peer-fetch address verification — a socket is a path (2026-07-26,
// pre-committed by work order: docs/peer_fetch_verification_handover.md).
//
// ── The headline, measured verbatim on v0.2.43 ───────────────────────────
//
// A peer that answers every request with the same 61 bytes:
//
//     {"Atom":[{"Str":"ATTACKER_CONTROLLED_NEVER_EXISTED"},0,null]}
//
// A program that asks for an address which has never existed:
//
//     conn: ~%Discovery./connect { 0: "Hostile", 1: "tcp://127.0.0.1:9934" }
//     got:  ~%Discovery./fetch   { 0: "Hostile", 1: "hash:sha256:v1:0000…0" }
//
//     $ oo run probe2.n --observe got
//     "ATTACKER_CONTROLLED_NEVER_EXISTED"
//
// `remote_fetch` (lib.rs:2249) opens a socket, writes the requested CAID,
// reads bytes, `serde_json::from_slice`, `Ok(val)`. The requested hash is
// used to ASK and never again. Any CAID can be made to resolve to any
// content by any peer you have connected to.
//
// v0.2.43 hardened the local store — the one place where you at least own
// the bytes — and left the network path, where you own nothing, entirely
// unverified. That release's story commit was "A path is not an identity".
// A socket is a path.
//
// ── Why this is a compliance gap, not a spec change ──────────────────────
// REAL_03 §6.6 was committed the day before this arc opened. 條款一 already
// says 「以 CAID 取得內容的**每一條路徑**」 and the section's own
// implementation note names 「對等取用」 as one of the hot paths. REAL_02
// §3.1 asserts the property as fact:
//
//     來源不影響收斂結果——相同的 hash 無論從哪裡取得,內容必然相同。
//
// The engine makes that sentence false. Nothing new has to be written down;
// what is written down has to be implemented.
//
// One floor under another: SPEC_13 §7.2 mandates a defence against the
// Semantic Eclipse Attack in which the engine deliberately fetches from
// nodes OUTSIDE its trust lattice (隨機跳出, 1/64). That only hardens
// anything if out-of-lattice bytes authenticate themselves. Without §6.6 on
// the network path it is not a defence, it is a channel.
//
// ── Ruling Q1: 對等 at degree 0, 偏序 at degree ≥1 ───────────────────────
// SPEC_13 §7.1's trust poset resolves 別名衝突 — which CAID a name collapses
// to. That is degree ≥1: authority and meaning. Once a CAID is in hand, who
// hands you the bytes is irrelevant, because degree-0 verification is
// self-authenticating: the least-trusted peer returning correctly-addressed
// bytes gives you an object byte-identical to the most-trusted peer's.
//
// So a mismatching peer is SKIPPED and the sweep CONTINUES (aborting would
// hand one malicious node a denial capability over every CAID), the verdict
// is never silently dropped (條款四), and `⊥ #caid_mismatch` is the result
// only when no source produced bytes that verify.
//
// R6 is that ruling in executable form. `Ouroboros::peers` is a `HashMap`
// (lib.rs:302) iterated in an order that differs per process. Today two
// peers holding different content for one CAID give a NONDETERMINISTIC
// result. Verification is what makes an unordered peer set safe: afterwards
// only correctly-addressed bytes survive, and those are identical by
// definition.
//
// ── Measured before writing (先量後寫) ───────────────────────────────────
// D1 network path unverified ......... measured twice (fabricated bytes under
//                                      a never-existing CAID; a real object
//                                      under a neighbouring CAID)
// D2 `NDP Miss` for a corrupt object . measured verbatim
// D3 corrupt ≡ absent ≡ ⊥ #conflict .. measured, character-for-character
// D4 shadow scan truncates silently .. PAIRED DISCRIMINATOR, 2 → 1 after
//                                      editing three bytes of one commit
// D5 shadow report swallows its read . read from code, same discard
//
// ── Calibration (the recurring failure, five arcs running) ───────────────
// Every red gate below asserts FIRST that the operation actually happened —
// the fake peer logged the request, the tampered object is on disk and still
// decodable, the untampered shadow report is non-empty — and only THEN the
// invariant. The defect this suite's predecessors kept hitting is a gate that
// goes red because the operation never ran: a ⊥ payload that was never
// applied (v0.2.42), a probe that measured the seeded universe instead of the
// run's delta (v0.2.43), and — during THIS arc's own measurement — a fetch
// that returned ⊥ because the hostile server had failed to start, which reads
// exactly like verification working. That one was a vacuous GREEN.
//
// R6 is the one gate with a probabilistic baseline: two liars and one honest
// peer, first-success-wins over `HashMap` order, 10 runs. Probability of a
// vacuous baseline pass ≈ 3⁻¹⁰. Deterministic after the fix.

mod common;

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

const ZERO_CAID: &str =
    "hash:sha256:v1:0000000000000000000000000000000000000000000000000000000000000000";

// ── harness ─────────────────────────────────────────────────────────────

fn fresh_dir() -> nlang_interpreter::ScratchDir {
    nlang_interpreter::ScratchDir::new("peerfetch")
}

fn oo_raw(dir: &Path, args: &[&str]) -> (String, String) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_oo"));
    for a in args {
        cmd.arg(a);
    }
    let out = cmd
        .current_dir(dir)
        .env("OO_IDENTITY", dir.join("identity-for-tests"))
        .env("OO_NODE_HOME", dir.join("node-home-for-tests"))
        .output()
        .unwrap();
    (
        String::from_utf8_lossy(&out.stdout).trim().to_string(),
        String::from_utf8_lossy(&out.stderr).trim().to_string(),
    )
}

fn oo(dir: &Path, args: &[&str]) -> String {
    let (o, e) = oo_raw(dir, args);
    format!("{o}{e}")
}

fn write(dir: &Path, name: &str, src: &str) {
    fs::write(dir.join(name), src).unwrap();
}

/// Runs one n/ source and observes one coordinate.
fn run_observe(dir: &Path, src: &str, coord: &str) -> String {
    write(dir, "p.n", src);
    // connect_consent §5.1: this suite's programs dial tcp:// sources; grant
    // models an operator who consented (more faithful, not weaker).
    oo(
        dir,
        &["run", "p.n", "--observe", coord, "--grant", "connect"],
    )
}

/// `.oo/objects/sha256/<aa>/<rest>` for the digest of a CAID. The digest is
/// the last colon-separated field in both v1 and v2 spellings.
fn digest_of(caid: &str) -> String {
    caid.rsplit(':').next().unwrap().to_string()
}

fn object_path(dir: &Path, caid: &str) -> PathBuf {
    let d = digest_of(caid);
    dir.join(".oo")
        .join("objects")
        .join("sha256")
        .join(&d[..2])
        .join(&d[2..])
}

/// Stores `value_src` in `dir`'s object store and returns its CAID.
/// `identify_and_store` persists under `oo run` even after v0.2.43 removed
/// the automatic store-put loop — verified before this file was written.
fn store_value(dir: &Path, value_src: &str) -> String {
    let out = run_observe(
        dir,
        &format!("id: ~%Discovery./identify_and_store {value_src}\n"),
        "id",
    );
    let caid = out
        .trim()
        .trim_start_matches('"')
        .split('"')
        .next()
        .unwrap()
        .to_string();
    assert!(
        caid.starts_with("hash:sha256:"),
        "store_value did not yield a CAID; got {out:?}"
    );
    assert!(
        object_path(dir, &caid).exists(),
        "store_value did not persist an object for {caid}"
    );
    caid
}

/// Flips the first hex digit of a CAID's digest — a syntactically valid
/// address that is not this object's.
fn neighbouring_caid(caid: &str) -> String {
    let d = digest_of(caid);
    let first = if d.starts_with('a') { 'b' } else { 'a' };
    let mutated = format!("{first}{}", &d[1..]);
    caid.rsplit_once(':').unwrap().0.to_string() + ":" + &mutated
}

// ── fake peers ──────────────────────────────────────────────────────────

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

/// Pull a CAID out of a bare line or an OODP request envelope.
fn caid_from_request(req: &str) -> String {
    let t = req.trim();
    if t.starts_with("hash:") {
        return t.to_string();
    }
    if let Some(i) = t.find("hash:sha256:") {
        let rest = &t[i..];
        let end = rest
            .find(|c: char| c == '"' || c.is_whitespace() || c == '}')
            .unwrap_or(rest.len());
        return rest[..end].to_string();
    }
    t.to_string()
}

/// Serves `answer(requested_caid)` — `None` means "nothing", the wire form of
/// absence. Detached; dies with the test process.
/// Accepts bare CAID or OODP `{{ %op: #fetch, %hash: "…" }}` request lines.
fn spawn_peer<F>(answer: F) -> FakePeer
where
    F: Fn(&str) -> Option<Vec<u8>> + Send + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let asked = Arc::new(Mutex::new(Vec::new()));
    let log = Arc::clone(&asked);
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
            let req = line.trim().to_string();
            let caid = caid_from_request(&req);
            log.lock().unwrap().push(caid.clone());
            if let Some(bytes) = answer(&caid) {
                let _ = stream.write_all(&bytes);
                let _ = stream.flush();
            }
            let _ = stream.shutdown(std::net::Shutdown::Write);
        }
    });
    FakePeer { port, asked }
}

/// Answers every request with the same bytes, whatever was asked.
fn spawn_liar(payload: Vec<u8>) -> FakePeer {
    spawn_peer(move |_| Some(payload.clone()))
}

/// Answers only for objects it actually holds, with the bytes on disk. The
/// stored file is pretty-printed and `oo serve` sends a compact encoding;
/// both decode to the same `Value`, so this is an honest peer.
fn spawn_honest(dir: PathBuf) -> FakePeer {
    spawn_peer(move |req| fs::read(object_path(&dir, req)).ok())
}

/// Runs `oo node serve` against `dir`, sends one OODP `#fetch` envelope,
/// returns (response body, the server's own console output).
fn ndp_ask(dir: &Path, caid: &str) -> (String, String) {
    // Isolate node-key mint from the developer's real ~/.oo.
    let node_home = dir.join("node-home-for-tests");
    let mut command = Command::new(env!("CARGO_BIN_EXE_oo"));
    command
        .current_dir(dir)
        .env("OO_NODE_HOME", &node_home)
        .env("OO_IDENTITY", dir.join("identity-for-tests"));
    let mut node = common::serve(command, dir.join("serve.log"));
    let mut stream = TcpStream::connect(("127.0.0.1", node.port)).unwrap();
    // Bare CAID retired (node_identity D5); use the envelope form.
    let req = format!("{{{{ %op: #fetch, %hash: \"{caid}\" }}}}\n");
    stream.write_all(req.as_bytes()).unwrap();
    stream.flush().unwrap();
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).ok();
    drop(stream);

    std::thread::sleep(std::time::Duration::from_millis(200));
    node.child.kill().ok();
    node.child.wait().ok();
    (
        String::from_utf8_lossy(&buf).into_owned(),
        common::read_log(&node.log),
    )
}

// ── tamper ──────────────────────────────────────────────────────────────

/// Rewrites bytes in place, preserving length and JSON validity, so the
/// object stays DECODABLE and only its address is wrong. That distinction is
/// the whole of REAL_03 §6.6 條款三: `#caid_mismatch` (bytes lying) is not
/// `#object_undecodable` (integrity undecidable).
fn tamper(path: &Path, from: &str, to: &str) {
    assert_eq!(from.len(), to.len(), "tamper must preserve length");
    let bytes = fs::read(path).unwrap();
    let text = String::from_utf8(bytes).expect("object is not UTF-8");
    assert_eq!(
        text.matches(from).count(),
        1,
        "tamper marker {from:?} must occur exactly once in {path:?}"
    );
    let tampered = text.replace(from, to);
    nlang_interpreter::store_codec::object_json_view(tampered.as_bytes()).expect(
        "tampered object must remain decodable — otherwise this probe \
                 measures #object_undecodable rather than #caid_mismatch",
    );
    fs::write(path, tampered).unwrap();
}

/// A universe whose root carries a FORCED field, three commits deep.
///
/// This matters: the shadow scan compares `fv.content_hash()` over the root's
/// fields, and a field written as a literal is stored as an UNFORCED thunk
/// carrying its source span, whose hash can never equal the forced value's
/// CAID. A field holding a builtin result is stored forced. Measured — a
/// literal-valued source never matches and the baseline shadow report is
/// empty, which would make R5 vacuous. (The thunk-at-rest finding itself is
/// out of scope and ledgered separately.)
///
/// Returns (source CAID that the shadow scan will match, commit CAIDs
/// newest-first).
fn shadow_universe(dir: &Path) -> (String, Vec<String>) {
    write(
        dir,
        "a.n",
        "payload: { hello: \"world\" }\npid: ~%Discovery./identify_and_store payload\n",
    );
    oo(dir, &["evolve", "a.n"]);
    oo(dir, &["commit", "-m", "c1"]);
    write(dir, "b.n", "marker: \"second\"\n");
    oo(dir, &["evolve", "b.n"]);
    oo(dir, &["commit", "-m", "c2"]);

    // Learn the CAID of the (forced) `pid` field in a scratch copy, so the
    // universe under test keeps exactly two commits.
    let scratch = fresh_dir();
    copy_tree(dir, &scratch);
    write(
        &scratch,
        "c.n",
        "pid2: ~%Discovery./identify_and_store pid\n",
    );
    oo(&scratch, &["evolve", "c.n"]);
    let status = oo(&scratch, &["status"]);
    let source = status
        .split("pid2: \"")
        .nth(1)
        .and_then(|s| s.split('"').next())
        .unwrap_or_default()
        .to_string();
    assert!(
        source.starts_with("hash:sha256:"),
        "shadow_universe could not learn the forced field's CAID; status was:\n{status}"
    );

    let commits: Vec<String> = oo(dir, &["log"])
        .lines()
        .filter_map(|l| l.strip_prefix("commit ").map(|s| s.trim().to_string()))
        .collect();
    assert_eq!(commits.len(), 2, "shadow_universe expects two commits");
    (source, commits)
}

fn copy_tree(from: &Path, to: &Path) {
    for entry in walk(from) {
        let rel = entry.strip_prefix(from).unwrap();
        let dst = to.join(rel);
        if entry.is_dir() {
            fs::create_dir_all(&dst).ok();
        } else {
            fs::create_dir_all(dst.parent().unwrap()).ok();
            fs::copy(&entry, &dst).unwrap();
        }
    }
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(rd) = fs::read_dir(dir) else {
        return out;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            out.push(p.clone());
            out.extend(walk(&p));
        } else {
            out.push(p);
        }
    }
    out
}

fn shadow_count(output: &str) -> Option<usize> {
    output
        .split("Shadow: ")
        .nth(1)
        .and_then(|s| s.split_whitespace().next())
        .and_then(|n| n.parse().ok())
}

// ── R1 ──────────────────────────────────────────────────────────────────

/// A peer answers a never-existing address with fabricated bytes. The engine
/// must not hand them to the program as the content of that address.
#[test]
fn red_fabricated_bytes_are_not_the_requested_identity() {
    let liar =
        spawn_liar(br#"{"Atom":[{"Str":"ATTACKER_CONTROLLED_NEVER_EXISTED"},0,null]}"#.to_vec());
    let dir = fresh_dir();
    let out = run_observe(
        &dir,
        &format!(
            "conn: ~%Discovery./connect {{ 0: \"H\", 1: \"{}\" }}\n\
             got:  ~%Discovery./fetch   {{ 0: \"H\", 1: \"{ZERO_CAID}\" }}\n",
            liar.addr()
        ),
        "got",
    );

    // LIVENESS: the connect and the fetch both really happened — the peer
    // received the request. Without this the gate would also pass if the
    // program had died before reaching the network at all, which is the
    // vacuous-green that bit this arc's own measurement.
    assert!(
        liar.asked().iter().any(|q| q == ZERO_CAID),
        "peer was never asked — the probe measured nothing. asked: {:?}",
        liar.asked()
    );

    assert!(
        !out.contains("ATTACKER_CONTROLLED"),
        "fabricated bytes were returned as the content of {ZERO_CAID}: {out}"
    );
}

// ── R2 ──────────────────────────────────────────────────────────────────

/// A peer answers with a GENUINE object — just not the one asked for. The
/// bytes decode, they are a real n/ value, and they are not this identity.
#[test]
fn red_a_real_object_under_the_wrong_address_is_refused() {
    let vault = fresh_dir();
    let real = store_value(&vault, "{ marker: \"PEER_B_REAL_VALUE_R2\" }");
    let bytes = fs::read(object_path(&vault, &real)).unwrap();

    // LIVENESS: the payload really is a genuine object at `real`.
    let inspected = oo(&vault, &["inspect", &real]);
    assert!(
        inspected.contains("PEER_B_REAL_VALUE_R2"),
        "payload is not a readable object at its own address: {inspected}"
    );

    let wrong = neighbouring_caid(&real);
    assert_ne!(wrong, real);

    let liar = spawn_liar(bytes);
    let dir = fresh_dir();
    let out = run_observe(
        &dir,
        &format!(
            "conn: ~%Discovery./connect {{ 0: \"H\", 1: \"{}\" }}\n\
             got:  ~%Discovery./fetch   {{ 0: \"H\", 1: \"{wrong}\" }}\n",
            liar.addr()
        ),
        "got",
    );

    // LIVENESS
    assert!(
        liar.asked().iter().any(|q| *q == wrong),
        "peer was never asked for {wrong}; asked: {:?}",
        liar.asked()
    );

    assert!(
        !out.contains("PEER_B_REAL_VALUE_R2"),
        "object {real} was accepted as the content of {wrong}: {out}"
    );
}

// ── R3 ──────────────────────────────────────────────────────────────────

/// REAL_03 §6.6 條款三 at the language surface. Corruption and absence are
/// currently the same value, character for character.
#[test]
fn red_corrupt_and_absent_are_distinguishable_to_a_program() {
    let vault = fresh_dir();
    let caid = store_value(&vault, "{ marker: \"R3_ORIGINAL\" }");
    let dir = fresh_dir();

    let fetch_src = |target: &str| {
        format!(
            "conn: ~%Discovery./connect {{ 0: \"B\", 1: \"{}\" }}\n\
             got:  ~%Discovery./fetch   {{ 0: \"B\", 1: \"{target}\" }}\n",
            vault.display()
        )
    };

    // LIVENESS: before corruption this exact fetch returns the value, so the
    // peer wiring, the CAID and the harness are all known good.
    let healthy = run_observe(&dir, &fetch_src(&caid), "got");
    assert!(
        healthy.contains("R3_ORIGINAL"),
        "peer fetch is not live before corruption: {healthy}"
    );

    tamper(&object_path(&vault, &caid), "R3_ORIGINAL", "R3_TAMPERED");

    let corrupt = run_observe(&dir, &fetch_src(&caid), "got");
    let absent = run_observe(&dir, &fetch_src(ZERO_CAID), "got");

    assert_ne!(
        corrupt, absent,
        "a program cannot tell a lying copy from an absent one; both are {corrupt}"
    );
    // Shape, not just inequality — v0.2.43's calibration lesson: a gate that
    // only asserts "these differ" can be satisfied by the wrong difference.
    assert!(
        corrupt.contains("caid_mismatch"),
        "corruption must surface as #caid_mismatch, got {corrupt}"
    );
    assert!(
        !absent.contains("caid_mismatch"),
        "plain absence must NOT be reported as a mismatch, got {absent}"
    );
    assert!(
        !corrupt.contains("R3_TAMPERED"),
        "the lying bytes were returned to the program: {corrupt}"
    );
}

// ── R4 ──────────────────────────────────────────────────────────────────

/// REAL_03 §6.6 條款三 on the operator's console. The store is corrupt;
/// the node must name the integrity failure, not report absence, and must
/// not put the lying object bytes on the wire (OODP: `%status: #conflict`,
/// no `%result`).
#[test]
fn red_ndp_serve_does_not_report_corruption_as_absence() {
    let vault = fresh_dir();
    let caid = store_value(&vault, "{ marker: \"R4_ORIGINAL\" }");
    tamper(&object_path(&vault, &caid), "R4_ORIGINAL", "R4_TAMPERED");

    let (served, console) = ndp_ask(&vault, &caid);

    // LIVENESS: the request reached the server.
    assert!(
        console.contains("OODP Request:") && console.contains(caid.as_str()),
        "oo node serve never saw the request; console was:\n{console}"
    );
    // No lying object payload on the wire (envelope may still name the status).
    assert!(
        !served.contains("R4_TAMPERED"),
        "corrupt bytes were served to a peer: {served}"
    );
    assert!(
        served.contains("#conflict") || served.contains("conflict"),
        "corrupt store must answer with conflict status: {served}"
    );

    assert!(
        !console.contains(&format!("OODP Miss: {caid}"))
            && !console.contains(&format!("NDP Miss: {caid}")),
        "corruption reported as absence; console was:\n{console}"
    );
    assert!(
        console.to_lowercase().contains("mismatch"),
        "console must name the corruption; console was:\n{console}"
    );
}

// ── R5 ──────────────────────────────────────────────────────────────────

/// REAL_03 §6.6 條款四, and the v0.2.43 `#refine` precedent one call site
/// over: tampering buys silence in the audit report.
///
/// PAIRED DISCRIMINATOR. Two byte-identical universes; three bytes edited in
/// one commit object of the second. Measured on v0.2.43: 2 shadow-affected
/// commits become 1, in the same confident wording, with no error and no
/// warning.
#[test]
fn red_tampering_does_not_silently_shorten_the_shadow_report() {
    let clean = fresh_dir();
    let (source, commits) = shadow_universe(&clean);
    let target = store_value(&fresh_dir(), "{ hello: \"world\", extra: 1 }");

    let dirty = fresh_dir();
    copy_tree(&clean, &dirty);

    // LIVENESS: the untampered scan really reports both commits. Without this
    // the gate would pass on an empty shadow list, which is what a
    // literal-valued source silently produces.
    let baseline = oo(
        &clean,
        &["refine", "-s", &source, "-t", &target, "-m", "rf", "--sign"],
    );
    assert_eq!(
        shadow_count(&baseline),
        Some(2),
        "baseline shadow scan is not live; output was:\n{baseline}"
    );

    let oldest = commits.last().unwrap();
    tamper(&object_path(&dirty, oldest), "\"c1\"", "\"cX\"");

    let after = oo(
        &dirty,
        &["refine", "-s", &source, "-t", &target, "-m", "rf", "--sign"],
    );

    assert!(
        after.to_lowercase().contains("mismatch") || after.to_lowercase().contains("truncat"),
        "the scan was cut short by a corrupt commit and said nothing. \
         baseline reported {:?}, this run reported {:?}:\n{after}",
        shadow_count(&baseline),
        shadow_count(&after)
    );
}

// ── R6 ──────────────────────────────────────────────────────────────────

/// Ruling Q1 in executable form: verification is what makes an unordered peer
/// set safe. Two liars and one honest peer; `peers` is a `HashMap` whose
/// iteration order differs per process, and the sweep returns on first
/// success. Today the answer depends on which peer the map happens to yield
/// first. After verification only correctly-addressed bytes survive, and
/// those are identical whoever sent them (REAL_02 §3.1).
#[test]
fn red_one_honest_peer_among_liars_is_found_every_time() {
    let vault = fresh_dir();
    let caid = store_value(&vault, "{ marker: \"HONEST_VALUE_R6\" }");
    let bytes = fs::read(object_path(&vault, &caid)).unwrap();

    let honest = spawn_honest(vault.path().to_path_buf());
    let liar_a = spawn_liar(br#"{"Atom":[{"Str":"LIAR_A_R6"},0,null]}"#.to_vec());
    // A liar holding a genuine-but-different object, so the sweep cannot be
    // rescued by "only well-formed values survive".
    let decoy = fresh_dir();
    let other = store_value(&decoy, "{ marker: \"LIAR_B_R6\" }");
    let liar_b = spawn_liar(fs::read(object_path(&decoy, &other)).unwrap());

    assert_ne!(bytes, fs::read(object_path(&decoy, &other)).unwrap());

    let dir = fresh_dir();
    let src = format!(
        "c1: ~%Discovery./connect {{ 0: \"honest\", 1: \"{}\" }}\n\
         c2: ~%Discovery./connect {{ 0: \"liarA\",  1: \"{}\" }}\n\
         c3: ~%Discovery./connect {{ 0: \"liarB\",  1: \"{}\" }}\n\
         all: c1 & c2 & c3\n\
         got: ~%Discovery./fetch {{ 0: \"{caid}\" }}\n",
        honest.addr(),
        liar_a.addr(),
        liar_b.addr()
    );

    let mut results = Vec::new();
    for _ in 0..10 {
        results.push(run_observe(&dir, &src, "got"));
    }

    // LIVENESS: the sweep really visited more than the honest peer.
    let liar_traffic = liar_a.asked().len() + liar_b.asked().len();
    assert!(
        liar_traffic > 0,
        "no liar was ever consulted — the sweep did not run over the peer set"
    );
    assert!(
        !honest.asked().is_empty(),
        "the honest peer was never consulted"
    );

    let dishonest: Vec<&String> = results
        .iter()
        .filter(|r| !r.contains("HONEST_VALUE_R6"))
        .collect();
    assert!(
        dishonest.is_empty(),
        "{}/10 runs did not return the honest value; a lying peer wins whenever \
         the HashMap yields it first: {dishonest:?}",
        dishonest.len()
    );
}

// ── pins: green at baseline, must stay green ────────────────────────────

/// An honest tcp peer still answers. This is the pin that catches a fix which
/// simply stops fetching over the network.
#[test]
fn pin_honest_tcp_peer_still_serves() {
    let vault = fresh_dir();
    let caid = store_value(&vault, "{ marker: \"PIN1_HONEST\" }");
    let honest = spawn_honest(vault.path().to_path_buf());
    let dir = fresh_dir();
    let out = run_observe(
        &dir,
        &format!(
            "conn: ~%Discovery./connect {{ 0: \"H\", 1: \"{}\" }}\n\
             got:  ~%Discovery./fetch   {{ 0: \"H\", 1: \"{caid}\" }}\n",
            honest.addr()
        ),
        "got",
    );
    assert!(
        out.contains("PIN1_HONEST"),
        "an honest peer's correctly-addressed object was not returned: {out}"
    );
}

/// A local-store fetch of a valid object still works.
#[test]
fn pin_local_store_fetch_still_works() {
    let dir = fresh_dir();
    let caid = store_value(&dir, "{ marker: \"PIN2_LOCAL\" }");
    let out = run_observe(
        &dir,
        &format!("got: ~%Discovery./fetch {{ 0: \"{caid}\" }}\n"),
        "got",
    );
    assert!(out.contains("PIN2_LOCAL"), "local fetch regressed: {out}");
}

/// A genuinely absent CAID stays absent — it must not become a mismatch.
/// This is the scope fence on R3: over-reporting is as wrong as under.
#[test]
fn pin_absence_from_an_honest_peer_stays_absence() {
    let vault = fresh_dir();
    store_value(&vault, "{ marker: \"PIN3_PRESENT\" }");
    let honest = spawn_honest(vault.path().to_path_buf());
    let dir = fresh_dir();
    let out = run_observe(
        &dir,
        &format!(
            "conn: ~%Discovery./connect {{ 0: \"H\", 1: \"{}\" }}\n\
             got:  ~%Discovery./fetch   {{ 0: \"H\", 1: \"{ZERO_CAID}\" }}\n",
            honest.addr()
        ),
        "got",
    );
    assert!(out.contains("_|_"), "absence should still be ⊥: {out}");
    assert!(
        !out.contains("caid_mismatch"),
        "an object nobody holds is not a lie: {out}"
    );
}

/// v0.2.43's local read verification is unchanged.
#[test]
fn pin_inspect_still_reads_a_valid_object() {
    let dir = fresh_dir();
    let caid = store_value(&dir, "{ marker: \"PIN4_INSPECT\" }");
    let out = oo(&dir, &["inspect", &caid]);
    assert!(out.contains("PIN4_INSPECT"), "inspect regressed: {out}");
}

/// The untampered shadow report is unchanged — the D4 fix must not cost the
/// honest path its audit surface.
#[test]
fn pin_untampered_shadow_report_is_complete() {
    let clean = fresh_dir();
    let (source, _) = shadow_universe(&clean);
    let target = store_value(&fresh_dir(), "{ hello: \"world\", extra: 1 }");
    let out = oo(
        &clean,
        &["refine", "-s", &source, "-t", &target, "-m", "rf", "--sign"],
    );
    assert_eq!(
        shadow_count(&out),
        Some(2),
        "the untampered shadow scan lost commits:\n{out}"
    );
    assert!(
        !out.to_lowercase().contains("mismatch"),
        "a healthy store must not be reported as corrupt:\n{out}"
    );
}

/// OODP node still reports genuine absence as absence (`#not_found` / Miss).
#[test]
fn pin_ndp_serve_still_reports_real_absence_as_miss() {
    let vault = fresh_dir();
    store_value(&vault, "{ marker: \"PIN6\" }");
    let (served, console) = ndp_ask(&vault, ZERO_CAID);
    assert!(
        served.contains("not_found") || served.contains("#not_found"),
        "absence must say not_found on the wire: {served}"
    );
    assert!(
        !served.contains("PIN6"),
        "absence must not carry an unrelated object: {served}"
    );
    assert!(
        console.contains(&format!("OODP Miss: {ZERO_CAID}"))
            || console.contains(&format!("Miss: {ZERO_CAID}")),
        "absence lost its report:\n{console}"
    );
}
