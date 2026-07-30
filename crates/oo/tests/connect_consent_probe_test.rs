// 撥號需要同意 / dialling needs consent — the gate (2026-07-30).
// Pre-committed by work order: docs/connect_consent_handover.md
//
// ── The defect ───────────────────────────────────────────────────────────
//
// REAL_02 §4.2.6 says the fetch source set "只由操作者顯式建立(`./connect`)"
// — is built only by the operator, explicitly. That sentence describes an
// intention the mechanism does not enforce. `~%Discovery./connect` is a plain
// language-layer builtin carrying `EffectTag::IO` and nothing else.
//
// Measured on v0.5.0: an ordinary program, no `--grant`, no `--privileged`,
// added a remote source and the engine then dialled it.
//
//     added: ~%Discovery./connect ("stranger", "tcp://192.0.2.9:8080")
//       → #true  ;; %effect: #io
//     a following ~%Discovery./fetch → 5.05 s, ⊥ #peer_timeout
//
// The 5.05 s proves the dial happened. So any program — including one fetched
// from a peer and evaluated — can make this engine connect out to an address
// of its choosing and tell that address which CAID it wants.
//
// ── Why this is the arc that had to come first ───────────────────────────
//
// §4.2.6 also defers a ruling: whether a *discovered* peer may become a fetch
// source, which it calls a consent question (同意權問題) to be settled on its
// own rather than as a side effect of discovery. That ruling cannot be made
// while nothing gates who may add a source at all. The question "under what
// consent may discovery add a source" presupposes that consent exists.
//
// It does not, and the containment is accidental: the source set lives in
// process memory, so a program's `./connect` dies with the process. Nobody
// chose that. It is the same species as the `#discover` hash seed the sampling
// arc replaced — an accidental property standing in for a decision.
//
// ── What is actually at risk, and what is not ────────────────────────────
//
// Not integrity. SPEC_13 §6.1.1 and REAL_03 §6.6: every source is a read path,
// bytes are re-addressed and compared, a failing source is skipped and the scan
// **must** continue. A malicious source cannot hand you a wrong answer.
//
// What a source gets is (a) the right to spend your time — 5 s per silent
// member, per fetch, sequentially, and §6.1.1 forbids bailing out early — and
// (b) knowledge of which CAID you want. (a) is cappable and a silent member is
// detectable. (b) is neither capped nor revocable.
//
// So the axis is not security against convenience. It is **who may spend your
// time and who may learn what you are looking for**, which is why §4.2.6 called
// it consent rather than trust.
//
// ── Scope: the remote form only (G1) ────────────────────────────────────
//
// `./connect` has two forms. The local one opens an `ObjectStore` at a path;
// it dials nobody and tells nobody, so neither cost applies, and SPEC_08 §6.3
// already governs that filesystem boundary (`crosses_store_boundary` refuses a
// store directory). The gate is on `tcp://` only. **P1** holds the local form
// open; that is a ruling, not an omission.
//
// ── Numbers the gates are set from (measured on v0.5.0) ─────────────────
//
//   fetch with no source at all ....... 0.040 s (×3, stable)
//   fetch with one blackholed source .. 5.05 s   (linear: 3 → 15.09 s)
//   R4's threshold is 2 s: 50× above the floor, 2.5× below the dial.
//
// Existing refusal shape, for R1/R2 to assert against:
//
//   ⊥ (%cause: #privileged_required)
//     ;; runPure requires effect_override grant (CLI --privileged / --grant)
//
// i.e. the cause is `#privileged_required` and the detail names the capability.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use std::fs;

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

/// The capability this arc introduces. A CLI flag spelling is an interface, so
/// it is pinned literally for the same reason a wire format is: a second
/// implementation reading the spec must accept the same word.
const GRANT: &str = "connect";

/// TEST-NET-1 (RFC 5737). Reserved for documentation, so packets are dropped
/// rather than refused — which is what makes the 5 s connect timeout visible.
/// A closed local port returns instantly (measured 0.06 s) and would hide it.
const BLACKHOLE: &str = "tcp://192.0.2.9:8080";

/// Above the 0.040 s no-dial floor, well below the 5.05 s dial.
const NO_DIAL_CEILING: Duration = Duration::from_secs(2);

// ── harness ─────────────────────────────────────────────────────────────

fn fresh_dir(tag: &str) -> PathBuf {
    let mut d = std::env::temp_dir();
    d.push(format!(
        "nlang-consent-{}-{}-{}",
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

fn write_prog(dir: &Path, name: &str, src: &str) -> String {
    fs::write(dir.join(name), src).unwrap();
    name.to_string()
}

/// Run a program and observe one binding. `grants` are appended verbatim.
fn observe(dir: &Path, prog: &str, binding: &str, grants: &[&str]) -> String {
    let mut args: Vec<&str> = vec!["run", prog, "--observe", binding];
    args.extend_from_slice(grants);
    oo(dir, &args)
}

fn timed_observe(dir: &Path, prog: &str, binding: &str, grants: &[&str]) -> (String, Duration) {
    let t0 = Instant::now();
    let out = observe(dir, prog, binding, grants);
    (out, t0.elapsed())
}

fn a_caid(dir: &Path) -> String {
    let out = oo(dir, &["eval", "~%Discovery./identify {{ probe: 1 }}"]);
    out.split("hash:sha256:")
        .nth(1)
        .map(|s| format!("hash:sha256:{}", s.split('"').next().unwrap_or("")))
        .unwrap_or_else(|| panic!("no CAID in {out:?}"))
}

fn refused_for_privilege(out: &str) -> bool {
    out.contains("privileged_required")
}

fn caid_of_stored(out: &str) -> String {
    out.split("hash:sha256:")
        .nth(1)
        .map(|s| format!("hash:sha256:{}", s.split('"').next().unwrap_or("")))
        .unwrap_or_else(|| panic!("nothing stored: {out}"))
}

// ── a serving node, for P2's wire control ───────────────────────────────
//
// P2 needs to prove the object is obtainable *somehow*, or its refusal proves
// nothing. Calibration found that no cross-workspace source form works without
// a grant: `./connect` to another workspace's `.oo` is refused by SPEC_08 §6.3
// (`crosses_store_boundary`), and `tcp://` is the form this arc gates. So the
// control is a raw wire `#fetch`, which is served by the holder and needs no
// source set on the asking side at all — the same control `discover_index`
// used for the same reason.

struct Node { child: Child, port: u16 }
impl Drop for Node { fn drop(&mut self) { self.child.kill().ok(); self.child.wait().ok(); } }
impl Node { fn stop(mut self) { self.child.kill().ok(); self.child.wait().ok(); } }

fn free_port() -> u16 {
    for _ in 0..64 {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let p = l.local_addr().unwrap().port();
        if p > 25000 { return p; }
    }
    panic!("no free port above 25000");
}

fn serve(dir: &Path) -> Node {
    let port = free_port();
    let f = fs::File::create(dir.join(format!("serve-{port}.log"))).unwrap();
    let child = oo_cmd(dir)
        .args(["node", "serve", "--port", &port.to_string()])
        .stdout(Stdio::from(f.try_clone().unwrap()))
        .stderr(Stdio::from(f))
        .spawn()
        .unwrap();
    for _ in 0..40 {
        std::thread::sleep(Duration::from_millis(100));
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return Node { child, port };
        }
    }
    panic!("`oo node serve` never came up");
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

// ════════════════════════════════════════════════════════════════════════
// CONTROL — green before and after. If this is red every verdict is void.
// ════════════════════════════════════════════════════════════════════════

/// C1 — the harness can see both answers: an allowed effect returns a value,
/// and an already-gated operation returns `⊥ #privileged_required`. Without
/// both halves a red below could pass because the harness sees nothing.
#[test]
fn c1_the_harness_sees_both_allowed_and_refused() {
    let dir = fresh_dir("c1");
    init(&dir);

    // Allowed: the local form of ./connect, which this arc leaves open (G1).
    let peer = dir.join("peerstore");
    fs::create_dir_all(&peer).unwrap();
    let p = write_prog(
        &dir,
        "ok.n",
        "r: ~%Discovery./connect (\"local\", \"peerstore\")\n",
    );
    let allowed = observe(&dir, &p, "r", &[]);
    assert!(
        allowed.contains("#true"),
        "harness cannot observe an allowed effect: {allowed}"
    );

    // Refused: an operation that is already gated today.
    let g = write_prog(&dir, "gated.n", "r: ~%Effect./runPure {{ x: 1 }}\n");
    let refused = observe(&dir, &g, "r", &[]);
    assert!(
        refused_for_privilege(&refused),
        "harness cannot observe a privilege refusal: {refused}"
    );
}

// ════════════════════════════════════════════════════════════════════════
// REDS — `#[ignore]` until delivery. Delivery removes ONLY the attribute.
// ════════════════════════════════════════════════════════════════════════

/// R1 — an unprivileged program cannot add a remote fetch source.
///
/// Today it can, and the engine dials what it added. This is the sentence in
/// §4.2.6 becoming a gate instead of an intention.
#[test]
#[ignore]
fn r1_an_unprivileged_program_cannot_add_a_remote_source() {
    let dir = fresh_dir("r1");
    init(&dir);
    let p = write_prog(
        &dir,
        "c.n",
        &format!("r: ~%Discovery./connect (\"stranger\", \"{BLACKHOLE}\")\n"),
    );
    let out = observe(&dir, &p, "r", &[]);
    assert!(
        refused_for_privilege(&out),
        "a program with no grant added a remote fetch source: {out}"
    );
}

/// R2 — the refusal names the capability that is missing.
///
/// SPEC_08 §6.1.4 distinguishes "this operation is not authorised" from
/// "your coverage is insufficient"; this is the former, and a diagnostic that
/// does not say which word to pass leaves the operator guessing. The existing
/// `runPure` refusal already sets the pattern: it names `effect_override`.
#[test]
#[ignore]
fn r2_the_refusal_names_the_capability() {
    let dir = fresh_dir("r2");
    init(&dir);
    let p = write_prog(
        &dir,
        "c.n",
        &format!("r: ~%Discovery./connect (\"stranger\", \"{BLACKHOLE}\")\n"),
    );
    let out = observe(&dir, &p, "r", &[]);
    assert!(refused_for_privilege(&out), "not refused at all: {out}");
    assert!(
        out.contains(GRANT),
        "the refusal does not name `{GRANT}`, so the operator cannot act on it: {out}"
    );
}

/// R3 — with the grant, the remote source is added and is usable.
///
/// The gate must not close the legitimate path. Red today for a precise
/// reason: `--grant connect` is rejected by the flag parser, whose message
/// lists the capabilities that exist.
#[test]
#[ignore]
fn r3_with_the_grant_a_remote_source_is_added() {
    let dir = fresh_dir("r3");
    init(&dir);
    let p = write_prog(
        &dir,
        "c.n",
        &format!("r: ~%Discovery./connect (\"stranger\", \"{BLACKHOLE}\")\n"),
    );
    let out = observe(&dir, &p, "r", &["--grant", GRANT]);
    assert!(
        !out.contains("unknown grant"),
        "`--grant {GRANT}` is not a capability the CLI accepts: {out}"
    );
    assert!(
        out.contains("#true"),
        "the grant was accepted but the source was not added: {out}"
    );
}

/// R4 — a refused `./connect` must not have dialled.
///
/// Returning `⊥` while having already opened the connection would satisfy R1
/// and still hand the address the thing it wanted: a packet and a query. The
/// gate has to precede the effect, which is SPEC_08 §6.1.2's "authority must
/// be presented at the moment the privileged effect is applied", one layer
/// down and measured in seconds.
///
/// Timing, not inspection, because what must not happen is a syscall. Floor
/// 0.040 s, dial 5.05 s, threshold 2 s.
#[test]
#[ignore]
fn r4_a_refused_connect_does_not_dial() {
    let dir = fresh_dir("r4");
    init(&dir);
    let caid = a_caid(&dir);
    let p = write_prog(
        &dir,
        "cf.n",
        &format!(
            "c: ~%Discovery./connect (\"stranger\", \"{BLACKHOLE}\")\n\
             r: ~%Discovery./fetch \"{caid}\"\n"
        ),
    );
    let (out, took) = timed_observe(&dir, &p, "r", &[]);
    assert!(
        took < NO_DIAL_CEILING,
        "the program took {took:?}, so the blackholed address was dialled even \
         though adding it should have been refused (floor is 0.040 s, a dial \
         costs 5.05 s): {out}"
    );
}

/// R5 — the capability does not persist to the next invocation.
///
/// SPEC_08 §6.1.4: capabilities are presented per call and not stored, and
/// REAL_01 §7.0.1 says per-invocation presentation is *stronger* than a
/// long-lived token precisely because what is not kept cannot leak. A gate
/// that remembers is a token.
#[test]
#[ignore]
fn r5_the_grant_does_not_persist_to_the_next_run() {
    let dir = fresh_dir("r5");
    init(&dir);
    let p = write_prog(
        &dir,
        "c.n",
        &format!("r: ~%Discovery./connect (\"stranger\", \"{BLACKHOLE}\")\n"),
    );

    let granted = observe(&dir, &p, "r", &["--grant", GRANT]);
    assert!(
        granted.contains("#true"),
        "precondition: the granted run must succeed, or the second run proves \
         nothing: {granted}"
    );

    let after = observe(&dir, &p, "r", &[]);
    assert!(
        refused_for_privilege(&after),
        "a later run with no grant was allowed — the capability was stored \
         somewhere, which §6.1.2 forbids: {after}"
    );
}

// ════════════════════════════════════════════════════════════════════════
// PINS — green now, must stay green.
// ════════════════════════════════════════════════════════════════════════

/// P1 — the local form needs no grant, before and after (G1).
///
/// A local store dials nobody and discloses nothing, so neither consent cost
/// applies to it, and SPEC_08 §6.3 already governs the path it opens. Gating
/// it would be gating the wrong thing and would break three probes in two
/// untouched suites for no property gained.
#[test]
fn p1_the_local_form_needs_no_grant() {
    let dir = fresh_dir("p1");
    init(&dir);
    fs::create_dir_all(dir.join("peerstore")).unwrap();
    let p = write_prog(
        &dir,
        "l.n",
        "r: ~%Discovery./connect (\"local\", \"peerstore\")\n",
    );
    let out = observe(&dir, &p, "r", &[]);
    assert!(
        out.contains("#true"),
        "the local form of ./connect stopped working without a grant: {out}"
    );
}

/// P2 — the OODP directory is still not a fetch source.
///
/// REAL_02 §4.2.6 has forbidden this since 2026-07-28 and **nothing has ever
/// pinned it**: `discover_index`'s `p2_fetch_untouched` guards the server
/// answering `#fetch`, not the client's source set. So this MUST NOT could
/// have regressed silently at any point, and one of the things this arc buys
/// is that it no longer can.
///
/// The holder is real, serving, and its advert names the very CAID asked for.
/// The control is a raw wire `#fetch` to the holder: it proves the object
/// exists and is servable without needing any source set on the asking side,
/// so the client's failure below is about consent and not about the object.
#[test]
fn p2_the_directory_is_still_not_a_fetch_source() {
    let holder = fresh_dir("p2-holder");
    let client = fresh_dir("p2-client");
    init(&holder);
    init(&client);

    let stored = oo(
        &holder,
        &["eval", "~%Discovery./identify_and_store {{ payload: \"p2-only-here\" }}"],
    );
    let caid = caid_of_stored(&stored);

    let hnode = serve(&holder);
    let cnode = serve(&client);

    // CONTROL: the holder serves it over the wire. If this fails, everything
    // below is vacuous — calibration caught an earlier version of this pin
    // passing because *nothing* worked.
    let served = ask_raw(hnode.port, &format!("{{{{ %op: #fetch, %hash: \"{caid}\" }}}}\n"));
    assert!(
        served.contains("success"),
        "control: the holder does not serve the object, so the client's failure \
         to obtain it proves nothing: {served}"
    );

    // The client's directory learns the holder, with its real address, and the
    // advert lists the CAID as a service. This is the strongest form of "the
    // client knows where to get it" short of consenting to the address.
    let adv = oo(
        &holder,
        &[
            "node",
            "advertise",
            "--to",
            &format!("127.0.0.1:{}", cnode.port),
            "--listen-port",
            &hnode.port.to_string(),
            "--service",
            &caid,
        ],
    );
    assert!(
        adv.contains("success"),
        "control: the advertisement was not accepted, so the client's directory \
         does not know the holder and this pin tests nothing: {adv}"
    );
    let dir_listing = oo(&client, &["node", "peers"]);
    assert!(
        !dir_listing.trim().is_empty() && !dir_listing.contains("error:"),
        "control: the client's directory is unreadable: {dir_listing}"
    );
    cnode.stop();

    // The measurement: the client has the holder in its directory and has
    // never consented to its address.
    let probe = write_prog(&client, "p.n", &format!("r: ~%Discovery./fetch \"{caid}\"\n"));
    let via_directory = observe(&client, &probe, "r", &[]);
    hnode.stop();

    assert!(
        !via_directory.contains("p2-only-here"),
        "a CAID was fetched from a peer this workspace never connected to — the \
         directory became a fetch source, which REAL_02 §4.2.6 forbids until \
         that consent question is ruled on separately: {via_directory}"
    );
}

// P3 was "a failing source does not abort the scan" (SPEC_13 §6.1.1).
//
// Removed at calibration, for two reasons that both matter.
//
// It cannot be a pin here: proving it needs two client-side sources, which
// needs `tcp://`, which is the form this arc gates — so the probe could not be
// green both before and after without a grant, and a pin that needs the thing
// it is pinning is not a pin.
//
// And it already has an owner:
// `peer_fetch_verification::red_one_honest_peer_among_liars_is_found_every_time`
// pins exactly this. That suite is in the work order's scheduled-to-change list
// because its `./connect` calls are all `tcp://`, which makes the requirement
// sharp rather than looser: adding `--grant` to those call sites must not
// weaken what they assert, and §5 of the order says so explicitly.

/// P4 — `#fetch` served over the wire is untouched.
///
/// This arc changes who may *become* a source. It must not change what a node
/// answers when someone asks it for an object.
#[test]
fn p4_serving_fetch_is_untouched() {
    let dir = fresh_dir("p4");
    init(&dir);
    let stored = oo(
        &dir,
        &["eval", "~%Discovery./identify_and_store {{ payload: \"p4-served\" }}"],
    );
    assert!(
        stored.contains("hash:sha256:"),
        "precondition: nothing was stored, so serving it proves nothing: {stored}"
    );
    // Reading it back locally exercises the same store path a served #fetch
    // uses, without needing a second process.
    let caid = stored
        .split("hash:sha256:")
        .nth(1)
        .map(|s| format!("hash:sha256:{}", s.split('"').next().unwrap_or("")))
        .unwrap();
    let back = oo(&dir, &["inspect", &caid]);
    assert!(
        back.contains("p4-served"),
        "a stored object could not be read back: {back}"
    );
}
