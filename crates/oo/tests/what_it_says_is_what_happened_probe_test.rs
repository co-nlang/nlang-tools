// What it says is what happened.
// Order: nlang-tools/docs/what_it_says_is_what_happened_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-060.  Ruling: meta/oo/STATUS.md D77.
//
// ── What this arc is ─────────────────────────────────────────────────────
//
// Q-042, Q-044 and Q-055 each fixed a class of report that was not true or not
// in the engine's own words; each time the class was wider than its seeds.
// Recon 2026-09-27 on the v0.61.0 tag build (injection matrix: every durable
// path unreadable / unwritable × 17 local commands, plus the network paths)
// found thirteen members in four shapes:
//
//   host representation   identity key, node key, discovery.n unreadable →
//                          `os error 13`; advertise to a closed port → `os
//                          error 111`; serve on a busy port → `os error 98`;
//                          `atomic_write temp create …` (a function name);
//                          discover / find-node print a Rust `{:?}` (`Conflict`)
//   a reason that did not  format unwritable → commit says "working set
//   happen                 unreadable" while status reads it; a sequential
//                          commit after `evolve v: _` says "consumed by a
//                          concurrent commit" (SPEC_10 §4.1.3 says an honest
//                          empty working set keeps its original answer);
//                          a refused connection is called a conflict
//   surfaces disagree      evolve with savepoints unwritable: rc=1, but the
//                          injection landed and status shows it; node discover
//                          with its peer directory unwritable: rc=0 and
//                          "accepted=1", and the directory is empty
//   a field never recorded inspect prints `parent: (none)` for every commit
//   shown as fact          (Commit.parent is unset by design, D18/D52)
//
// D77 (user, 2026-09-27): a connection that could not be established is
// `#peer_unreachable` — a fifth row in REAL_02 §3.2.2, beside #peer_timeout.
//
// ── Probe integrity ──────────────────────────────────────────────────────
//
// Wording-free except where the spec fixes a spelling (`#peer_unreachable`,
// a registry tag) and where a spelling is the defect (`os error`, `atomic_write`,
// `Conflict`, `working set unreadable`, `concurrent`, `parent: (none)`).
// Every injection has a control in the same test. Network tests start a real
// `oo node serve` and check that it answers before injecting anything.
//
// The delivery may NOT edit this file. If a pin is wrong, say so in the
// report. `rustfmt` must not touch it. Run as a non-root user (permission
// injections are meaningless to root; the tests say VOID if so).
//
// Baseline measured 2026-09-27 on dev 73d3fcb / oo v0.61.0: see the order.

use std::fs;
use std::net::{TcpListener, TcpStream};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

struct Ws {
    _scratch: nlang_interpreter::ScratchDir,
    root: PathBuf,
    ws: PathBuf,
}

impl Ws {
    fn new(tag: &str) -> Self {
        let scratch = nlang_interpreter::ScratchDir::new(&format!("says-{tag}"));
        let root = scratch.path().to_path_buf();
        let ws = root.join("ws");
        fs::create_dir_all(&ws).unwrap();
        Ws { _scratch: scratch, root, ws }
    }

    fn cmd(&self, args: &[&str]) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_oo"));
        c.args(args)
            .current_dir(&self.ws)
            .env("OO_IDENTITY", self.root.join("identity"))
            .env("OO_NODE_HOME", self.root.join("node-home"));
        c
    }

    fn oo(&self, args: &[&str]) -> (String, i32) {
        let o = self.cmd(args).output().expect("oo runs");
        (
            format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)),
            o.status.code().unwrap_or(-1),
        )
    }

    fn ok(&self, args: &[&str]) -> String {
        let (o, rc) = self.oo(args);
        assert_eq!(rc, 0, "setup `oo {}`: {o}", args.join(" "));
        o
    }

    fn write(&self, name: &str, text: &str) {
        fs::write(self.ws.join(name), text).unwrap();
    }

    fn committed(&self, name: &str, text: &str) {
        self.write(name, text);
        self.ok(&["evolve", name]);
        self.ok(&["commit", "-m", name]);
    }

    fn head(&self) -> String {
        fs::read_to_string(self.ws.join(".oo/HEAD")).unwrap().trim().to_string()
    }

    fn staged_has(&self, needle: &str) -> bool {
        self.oo(&["status"]).0.contains(needle)
    }
}

fn chmod(p: &Path, mode: u32) {
    fs::set_permissions(p, fs::Permissions::from_mode(mode)).unwrap();
}

/// Permission injections are meaningless as root.
fn require_not_root(p: &Path) {
    chmod(p, 0o000);
    let readable = fs::read(p).is_ok() || fs::read_dir(p).is_ok();
    chmod(p, 0o700);
    if readable {
        panic!("VOID READING: permission injection has no effect (running as root?)");
    }
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

/// A real `oo node serve` in its own workspace; killed on drop.
struct Server {
    child: Child,
    port: u16,
    _ws: Ws,
}

impl Server {
    fn start(tag: &str) -> Self {
        let ws = Ws::new(tag);
        let port = free_port();
        let child = ws
            .cmd(&["node", "serve", "--port", &port.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("serve spawns");
        let t = Instant::now();
        while TcpStream::connect(("127.0.0.1", port)).is_err() {
            if t.elapsed() > Duration::from_secs(10) {
                panic!("VOID READING: `oo node serve` never listened on {port}");
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        Server { child, port, _ws: ws }
    }

    fn addr(&self) -> String {
        format!("127.0.0.1:{}", self.port)
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// A port nothing listens on (bound, then released).
fn closed_addr() -> String {
    format!("127.0.0.1:{}", free_port())
}

fn service_caid(w: &Ws) -> String {
    let o = w.ok(&["eval", "~%Discovery./identify 1"]);
    let at = o.find("hash:sha256:").unwrap_or_else(|| panic!("VOID READING: no CAID: {o}"));
    o[at..].split(|c: char| c == '"' || c.is_whitespace()).next().unwrap().to_string()
}

const TARGET40: &str = "0123456789abcdef0123456789abcdef01234567";

// ── Controls ─────────────────────────────────────────────────────────────

/// The healthy paths answer: commit, identity, node id, status.
#[test]
fn c1_the_healthy_paths_answer() {
    let w = Ws::new("c1");
    w.committed("a.n", "a: 1\n");
    w.ok(&["identity"]);
    w.ok(&["node", "id"]);
    w.ok(&["status"]);
}

/// A live peer answers discover, and the discovering workspace records it.
#[test]
fn c2_a_live_discover_is_recorded() {
    let s = Server::start("c2s");
    let b = Ws::new("c2b");
    let svc = service_caid(&b);
    let (o, rc) = b.oo(&["node", "advertise", "--to", &s.addr(), "--service", &svc, "--listen-port", "9"]);
    assert!(rc == 0 && o.contains("#success"), "VOID READING: advertise did not land: {o}");
    let c = Ws::new("c2c");
    let o = c.ok(&["node", "discover", "--to", &s.addr(), "--target", &svc]);
    assert!(o.contains("accepted=1"), "VOID READING: discover saw no peer: {o}");
    let dir = fs::read_to_string(c.ws.join(".oo/peers/directory")).unwrap_or_default();
    assert!(!dir.trim().is_empty(), "a healthy discover did not record its peer");
}

// ── Guards ───────────────────────────────────────────────────────────────

/// A workspace that never had anything to commit keeps its answer
/// (SPEC_10 §4.1.3 last clause): rc≠0 and no mention of concurrency.
#[test]
fn g1_nothing_to_commit_is_unchanged() {
    let w = Ws::new("g1");
    let (o, rc) = w.oo(&["commit", "-m", "x"]);
    assert!(rc != 0 && !o.contains("concurrent"), "{o}");
}

// ── Reds: host representation ────────────────────────────────────────────

fn no_host(o: &str) -> bool {
    !o.contains("os error") && !o.contains("Os {") && !o.contains("kind:")
}

/// The operator identity file cannot be read. Baseline: `… read failed:
/// Permission denied (os error 13)`.
#[test]
fn r1_an_unreadable_identity_is_named_in_engine_words() {
    let w = Ws::new("r1");
    w.ok(&["identity"]);
    let id = w.root.join("identity");
    require_not_root(&id);
    chmod(&id, 0o000);
    let (o, rc) = w.oo(&["identity"]);
    chmod(&id, 0o600);
    assert!(rc != 0, "an unreadable identity was not refused: {o}");
    assert!(no_host(&o), "`oo identity` answers in host words: {o}");
}

/// `refine --sign` with the identity unreadable. Baseline: `Signing failed:
/// … (os error 13)`.
#[test]
fn r2_signing_with_an_unreadable_identity_is_named_in_engine_words() {
    let w = Ws::new("r2");
    w.ok(&["identity"]);
    w.committed("a.n", "a: 1\n");
    let ins = w.ok(&["inspect", &w.head()]);
    let root = ins.lines().find_map(|l| l.strip_prefix("root:")).unwrap().trim().to_string();
    let id = w.root.join("identity");
    require_not_root(&id);
    chmod(&id, 0o000);
    let (o, rc) = w.oo(&["refine", "--source", &root, "--target", &root, "-m", "r", "--sign"]);
    chmod(&id, 0o600);
    assert!(rc != 0, "signing with an unreadable identity was not refused: {o}");
    assert!(no_host(&o), "`oo refine --sign` answers in host words: {o}");
}

/// The node key cannot be read. Baseline: `(os error 13)`.
#[test]
fn r3_an_unreadable_node_key_is_named_in_engine_words() {
    let w = Ws::new("r3");
    w.ok(&["node", "id"]);
    let home = w.root.join("node-home");
    require_not_root(&home);
    chmod(&home, 0o000);
    let (o, rc) = w.oo(&["node", "id"]);
    chmod(&home, 0o700);
    assert!(rc != 0, "an unreadable node key was not refused: {o}");
    assert!(no_host(&o), "`oo node id` answers in host words: {o}");
}

/// `.oo/discovery.n` cannot be read. Baseline: `(os error 13)`.
#[test]
fn r4_an_unreadable_discovery_file_is_named_in_engine_words() {
    let w = Ws::new("r4");
    w.committed("a.n", "a: 1\n");
    let key = w.ok(&["identity"]).lines().next().unwrap().trim().to_string();
    w.ok(&["node", "trust", "add", &key]);
    let p = w.ws.join(".oo/discovery.n");
    if !p.exists() {
        panic!("VOID READING: `oo node trust add` wrote no discovery.n");
    }
    w.ok(&["status"]);
    require_not_root(&p);
    chmod(&p, 0o000);
    let (o, rc) = w.oo(&["status"]);
    chmod(&p, 0o600);
    assert!(rc != 0, "an unreadable discovery.n was not refused: {o}");
    assert!(no_host(&o), "`oo status` answers in host words: {o}");
}

/// `serve` on a port already taken. Baseline: `Address already in use (os
/// error 98)`.
#[test]
fn r5_a_busy_port_is_named_in_engine_words() {
    let w = Ws::new("r5");
    let held = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = held.local_addr().unwrap().port().to_string();
    let (o, rc) = w.oo(&["node", "serve", "--port", &port]);
    drop(held);
    assert!(rc != 0, "serve on a busy port did not fail: {o}");
    assert!(no_host(&o), "`oo node serve` answers in host words: {o}");
}

// ── Reds: a connection that was never made (D77) ────────────────────────

/// Baseline: `Connection refused (os error 111)`.
#[test]
fn r6_advertise_to_nobody_is_peer_unreachable() {
    let w = Ws::new("r6");
    let svc = service_caid(&w);
    let (o, rc) = w.oo(&["node", "advertise", "--to", &closed_addr(), "--service", &svc]);
    assert!(rc != 0, "{o}");
    assert!(no_host(&o) && o.contains("#peer_unreachable"), "advertise to a closed port: {o}");
}

/// Baseline: `discover transport: Conflict`.
#[test]
fn r7_discover_from_nobody_is_peer_unreachable() {
    let w = Ws::new("r7");
    let svc = service_caid(&w);
    let (o, rc) = w.oo(&["node", "discover", "--to", &closed_addr(), "--target", &svc]);
    assert!(rc != 0, "{o}");
    assert!(
        o.contains("#peer_unreachable") && !o.to_lowercase().contains("conflict"),
        "discover from a closed port: {o}"
    );
}

/// Baseline: `find-node transport: Conflict`.
#[test]
fn r8_find_node_from_nobody_is_peer_unreachable() {
    let w = Ws::new("r8");
    let (o, rc) = w.oo(&["node", "find-node", "--to", &closed_addr(), "--target", TARGET40]);
    assert!(rc != 0, "{o}");
    assert!(
        o.contains("#peer_unreachable") && !o.to_lowercase().contains("conflict"),
        "find-node from a closed port: {o}"
    );
}

// ── Reds: a reason that did not happen ───────────────────────────────────

/// `.oo/format` read-only. `status` reads the working set in the same state
/// (control), so "working set unreadable" is not what happened.
/// Baseline: commit rc=1, `Error: working set unreadable`.
#[test]
fn r9_a_lock_that_cannot_be_taken_is_not_an_unreadable_working_set() {
    let w = Ws::new("r9");
    w.committed("a.n", "a: 1\n");
    w.write("b.n", "b: 2\n");
    w.ok(&["evolve", "b.n"]);
    let f = w.ws.join(".oo/format");
    chmod(&f, 0o400);
    let (st, strc) = w.oo(&["status"]);
    if strc != 0 || !st.contains("b: 2") {
        chmod(&f, 0o600);
        panic!("VOID READING: status could not read the working set either: {st}");
    }
    let (o, _) = w.oo(&["commit", "-m", "x"]);
    chmod(&f, 0o600);
    assert!(!o.contains("working set unreadable"), "a readable working set was reported unreadable: {o}");
}

/// A sequential `evolve v: _` then `commit`: nothing concurrent happened.
/// SPEC_10 §4.1.3: an honest empty working set keeps its original answer —
/// the one a workspace that never evolved anything gets (control).
/// Baseline: `working set consumed by a concurrent commit`.
#[test]
fn r10_a_sequential_commit_is_not_told_it_was_raced() {
    let never = Ws::new("r10a");
    never.write("e.n", ""); // AMENDED 2026-09-30 for Q-064 (D82): only `evolve` creates a universe
    let _ = never.oo(&["evolve", "e.n"]);
    let (want, want_rc) = never.oo(&["commit", "-m", "x"]);
    let w = Ws::new("r10b");
    w.write("p.n", "v: _\n");
    // Whatever evolve says about `v: _` (it adds nothing) is not what is
    // measured here; only the commit that follows it.
    let _ = w.oo(&["evolve", "p.n"]);
    let (o, rc) = w.oo(&["commit", "-m", "x"]);
    assert!(!o.contains("concurrent"), "a sequential commit was told it was raced: {o}");
    assert_eq!((o.trim(), rc), (want.trim(), want_rc), "an empty working set must answer as one that never had anything");
}

// ── Reds: every surface says the same thing ─────────────────────────────

/// `.oo/savepoints` unwritable during evolve. Text/exit code and the landed
/// state must agree, and no engine function name reaches the operator.
/// Baseline: rc=1 `atomic_write temp create …`, and `d: 4` is staged.
#[test]
fn r11_a_failed_evolve_did_not_land() {
    let w = Ws::new("r11");
    w.committed("a.n", "a: 1\n");
    w.write("d.n", "d: 4\n");
    let sp = w.ws.join(".oo/savepoints");
    require_not_root(&sp);
    chmod(&sp, 0o500);
    let (o, rc) = w.oo(&["evolve", "d.n"]);
    chmod(&sp, 0o700);
    let landed = w.staged_has("d: 4");
    assert_eq!(rc == 0, landed, "evolve said rc={rc} and the injection landed={landed}: {o}");
    assert!(!o.contains("atomic_write"), "an engine function name reached the operator: {o}");
}

/// `.oo/injections` unwritable: no function name either.
/// Baseline: `atomic_write temp create …`.
#[test]
fn r12_no_function_name_on_an_unwritable_working_set() {
    let w = Ws::new("r12");
    w.committed("a.n", "a: 1\n");
    w.write("d.n", "d: 4\n");
    let inj = w.ws.join(".oo/injections");
    require_not_root(&inj);
    chmod(&inj, 0o500);
    let (o, rc) = w.oo(&["evolve", "d.n"]);
    chmod(&inj, 0o700);
    assert!(rc != 0, "{o}");
    assert!(!o.contains("atomic_write") && no_host(&o), "{o}");
}

/// The discovering workspace cannot write its peer directory. If discover
/// says rc=0, the peer it reports as accepted must be in the directory.
/// Baseline: rc=0, `accepted=1`, directory empty.
#[test]
fn r13_an_accepted_peer_is_recorded_or_the_command_says_it_was_not() {
    let s = Server::start("r13s");
    let b = Ws::new("r13b");
    let svc = service_caid(&b);
    let (o, rc) = b.oo(&["node", "advertise", "--to", &s.addr(), "--service", &svc, "--listen-port", "9"]);
    assert!(rc == 0 && o.contains("#success"), "VOID READING: advertise did not land: {o}");
    let c = Ws::new("r13c");
    c.write("seed.n", "seed: 0\n"); // AMENDED 2026-09-30 for Q-064 (D82): only `evolve` creates a universe
    c.ok(&["evolve", "seed.n"]);
    c.ok(&["status"]);
    let peers = c.ws.join(".oo/peers");
    fs::create_dir_all(&peers).unwrap();
    require_not_root(&peers);
    chmod(&peers, 0o500);
    let (o, rc) = c.oo(&["node", "discover", "--to", &s.addr(), "--target", &svc]);
    chmod(&peers, 0o700);
    let dir = fs::read_to_string(peers.join("directory")).unwrap_or_default();
    if rc == 0 {
        assert!(!dir.trim().is_empty(), "discover succeeded and recorded nothing: {o}");
    } else {
        assert!(no_host(&o), "{o}");
    }
}

// ── Reds: a field never recorded, shown as fact ──────────────────────────

/// A commit with an ancestor. Commit.parent is unset by design (D18/D52);
/// `inspect` must not present that absence as "no parent".
/// Baseline: `parent: (none)`.
#[test]
fn r14_inspect_does_not_say_a_commit_has_no_parent_when_it_has_one() {
    let w = Ws::new("r14");
    w.committed("a.n", "a: 1\n");
    w.committed("b.n", "b: 2\n");
    let log = w.ok(&["log"]);
    if log.matches("commit hash:").count() < 2 {
        panic!("VOID READING: expected two commits: {log}");
    }
    let o = w.ok(&["inspect", &w.head()]);
    assert!(!o.contains("parent: (none)"), "a commit with an ancestor is shown as having none: {o}");
}

// ── Added at acceptance (2026-09-27), repair round R-1 ────────────────────

/// Accepts every connection and closes it at once, without a byte.
struct HangUp {
    port: u16,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    th: Option<std::thread::JoinHandle<()>>,
}

impl HangUp {
    fn start() -> Self {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        l.set_nonblocking(true).unwrap();
        let port = l.local_addr().unwrap().port();
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let s2 = stop.clone();
        let th = std::thread::spawn(move || {
            while !s2.load(std::sync::atomic::Ordering::Relaxed) {
                match l.accept() {
                    Ok((c, _)) => drop(c),
                    Err(_) => std::thread::sleep(Duration::from_millis(10)),
                }
            }
        });
        HangUp { port, stop, th: Some(th) }
    }

    fn addr(&self) -> String {
        format!("127.0.0.1:{}", self.port)
    }
}

impl Drop for HangUp {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        if let Some(t) = self.th.take() {
            let _ = t.join();
        }
    }
}

fn object_count(w: &Ws) -> usize {
    fn walk(p: &Path) -> usize {
        fs::read_dir(p).map(|d| d.flatten().map(|e| if e.path().is_dir() { walk(&e.path()) } else { 1 }).sum()).unwrap_or(0)
    }
    walk(&w.ws.join(".oo/objects"))
}

/// gc with a root source it cannot read. Found at acceptance by adding a
/// before/after state check to the recon matrix (the recon classified text
/// and exit codes only, and missed it): with `.oo/HEAD` unreadable, gc says
/// "0 reachable", rc=0, and deletes every object; after the permission is
/// restored `log` answers `CAID not found`. v0.55.0 and v0.61.0 alike.
/// REAL_03 §6.6: a store that cannot be opened must not be treated as empty.
/// Baseline: red on v0.61.0 and on the delivery 694a264.
#[test]
fn r15_gc_does_not_collect_what_it_could_not_read_head() {
    let w = Ws::new("r15");
    w.committed("a.n", "a: 1\n");
    w.committed("b.n", "b: 2\n");
    let before = object_count(&w);
    let head = w.ws.join(".oo/HEAD");
    require_not_root(&head);
    chmod(&head, 0o000);
    let (o, _) = w.oo(&["gc", "--grant", "gc"]);
    chmod(&head, 0o600);
    assert_eq!(object_count(&w), before, "gc deleted objects while HEAD was unreadable: {o}");
    let (l, rc) = w.oo(&["log"]);
    assert_eq!(rc, 0, "history is gone after gc: {l}");
}

/// The same with `.oo/savepoints` unreadable (3 of 6 objects deleted in the
/// recon, `log` broken after). Baseline: red on v0.61.0 and 694a264.
#[test]
fn r16_gc_does_not_collect_what_it_could_not_read_savepoints() {
    let w = Ws::new("r16");
    w.committed("a.n", "a: 1\n");
    w.committed("b.n", "b: 2\n");
    let before = object_count(&w);
    let sp = w.ws.join(".oo/savepoints");
    require_not_root(&sp);
    chmod(&sp, 0o000);
    let (o, _) = w.oo(&["gc", "--grant", "gc"]);
    chmod(&sp, 0o700);
    assert_eq!(object_count(&w), before, "gc deleted objects while savepoints were unreadable: {o}");
    let (l, rc) = w.oo(&["log"]);
    assert_eq!(rc, 0, "history is gone after gc: {l}");
}

/// D78: connected, then the peer hung up before answering ⟹ `#peer_closed`.
/// On 694a264 the same event is `#peer_unreachable` (discover, find-node),
/// `#peer_timeout` (fetch), and rc=0 with an empty line (advertise).
#[test]
fn r17_discover_from_a_peer_that_hangs_up_is_peer_closed() {
    let h = HangUp::start();
    let w = Ws::new("r17");
    let svc = service_caid(&w);
    let (o, rc) = w.oo(&["node", "discover", "--to", &h.addr(), "--target", &svc]);
    assert!(rc != 0 && o.contains("#peer_closed"), "rc={rc}: {o}");
}

#[test]
fn r18_find_node_from_a_peer_that_hangs_up_is_peer_closed() {
    let h = HangUp::start();
    let w = Ws::new("r18");
    let (o, rc) = w.oo(&["node", "find-node", "--to", &h.addr(), "--target", TARGET40]);
    assert!(rc != 0 && o.contains("#peer_closed"), "rc={rc}: {o}");
}

#[test]
fn r19_fetch_from_a_peer_that_hangs_up_is_peer_closed() {
    let h = HangUp::start();
    let w = Ws::new("r19");
    let caid = service_caid(&w);
    w.write(
        "p.n",
        &format!(
            "conn: ~%Discovery./connect {{{{ 0: \"A\", 1: \"tcp://{}\" }}}}\ngot: ~%Discovery./fetch {{{{ 0: \"A\", 1: \"{caid}\" }}}}\n",
            h.addr()
        ),
    );
    let (o, _) = w.oo(&["run", "p.n", "--observe", "got", "--grant", "connect"]);
    assert!(o.contains("#peer_closed"), "{o}");
}

#[test]
fn r20_advertise_to_a_peer_that_hangs_up_is_peer_closed() {
    let h = HangUp::start();
    let w = Ws::new("r20");
    let svc = service_caid(&w);
    let (o, rc) = w.oo(&["node", "advertise", "--to", &h.addr(), "--service", &svc]);
    assert!(rc != 0 && o.contains("#peer_closed"), "advertise to a peer that hung up: rc={rc} [{o}]");
}
