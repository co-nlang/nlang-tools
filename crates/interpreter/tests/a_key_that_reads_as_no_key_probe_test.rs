// A key that reads as no key.
// Recon: in-conversation, 2026-09-22 (acceptor). Queue: WORK_QUEUE Q-054.
// Ruling: STATUS.md O91. Order: docs/a_key_that_reads_as_no_key_handover.md
//
// ── What this arc is ─────────────────────────────────────────────────────
//
// `node_id_if_present()` and the two loads in `Ouroboros::init` read the node
// key with `Identity::load(..).ok()`. Three states collapse into two:
//
//     absent                      -> None
//     present but unreadable      -> None      <- the defect
//     present and readable        -> Some(id)
//
// Downstream, `peers::load` reads `None` as "no node key yet" and
// `restore_asserted = this_node_id.map(|id| id == owner).unwrap_or(false)`,
// so an unreadable key does not merely reorder anything -- it discards the
// observer half of EVERY record (addr cleared, provenance -> Unknown,
// admission_seq -> 0) and leaves the routing index unseeded and empty.
//
// ── Why this is not just "a different order" ─────────────────────────────
//
// `local_seat_rank` salts the tie-break with the reader's OWN node id. Its
// purpose is recorded in `crates/oo/tests/seat_order_probe_test.rs`: a
// victim-independent tie-break lets an attacker grind one key (SPEC_15 §7.1
// prices minting at 3,500 keys/s) and "hold a seat on every node that ever
// hears you in the same second as someone else". With the reader's id
// missing, the hash is sha256("oodp-seat-order:v1:" + "|" + peer_id) --
// **a function of the peer's identity alone.**
//
// That is REAL_02's C′ from the seat_order arc, which was declared a MUST NOT
// with no probe **because it was judged unreachable once arrival order was
// total**. It is reachable: an unreadable identity sets admission_seq to 0 for
// every record, so it manufactures the tie it then breaks by peer id alone.
//
// ── Measured, 2026-09-22, oo v0.56.0 ────────────────────────────────────
//
//   * the write window is real and open: 200 real mints, a 0-byte visible
//     window observed 120/200, partial writes 0/200 (never a torn key)
//   * `oo status` is indifferent to all four key states (good / 0 B / 20 B
//     garbage / absent): same rc, same output; while `oo node id` on the same
//     garbage key refuses rc=1 "not a valid PKCS#8". The refusal exists; the
//     admission path does not ask for it.
//   * one reader waits (`load_or_mint` -> `load_after_race`), three do not
//     (lib.rs:948, 957, 1117), and 948/957 load the same key twice.
//
// ── The invariants (not the mechanism) ───────────────────────────────────
//
//  I1 (丁) No ordinary read path may conclude that this workspace has no node
//     identity while a complete identity exists on disk within the bounded
//     window already ruled at Q-051. That invariant was accepted there in its
//     strong form -- "wait out ANY transient incomplete identity file" -- and
//     is applied at one of four sites.
//  I2 (甲) When an identity file exists and is still unreadable after that
//     wait, the outcome MUST NOT be observationally identical to having no
//     identity at all. Refusing is one way; this file does not pick one.
//  I3 A permanently invalid key MUST still be refused, and its bytes left
//     unchanged (Q-051 G2/D2). Waiting must not become accepting.
//  I4 No read path may mint.
//
// ── NOT probed, so silence is not mistaken for coverage ──────────────────
//
//   * the cost of I1 at the three sites (up to 3 x 100ms per invocation on a
//     permanently corrupt key) is NOT measured here. The order asks for it.
//   * `resolve_node_home()` failures (relative OO_NODE_HOME, no HOME) are a
//     second, non-race route into the same None. Not probed: they need an
//     env-mutating test, and this file is deliberately env-stable.
//   * nothing here reaches the network path.
//   * ⚠ DISCLOSED HAZARD: `node_key_path` reads `OO_NODE_HOME` and there is
//     no parameter to override it, so this file sets that variable once,
//     through a `OnceLock`, before any key path is derived. `set_var` in a
//     multi-threaded test binary can in principle race a concurrent
//     `getenv`, whose worst case is one test falling back to `~/.oo/nodes`
//     and leaving a stray key in the operator's home. Measured over a
//     whole-workspace run x3: `~/.oo/nodes` held 297 files before and 297
//     after. Not proof of absence -- disclosed so it is not a surprise.
//
// Baseline measured 2026-09-22 on dev cbfd95d / oo v0.56.0: 5 green, 4 red
// (r5 added at acceptance: red on that baseline too -> 5 green, 5 red).
// If a pin here is wrong, say so in the report -- do not edit it.

use nlang_interpreter::peers;
use nlang_interpreter::value::Identity;
use nlang_interpreter::{Ouroboros, PeerAdvert};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// One shared node home for the whole binary. Key paths are derived from the
/// workspace path, so distinct workspaces get distinct keys without any test
/// mutating the environment after start-up.
fn node_home() -> &'static PathBuf {
    static HOME: OnceLock<PathBuf> = OnceLock::new();
    HOME.get_or_init(|| {
        let p = std::env::temp_dir().join(format!("nk-probe-home-{}", std::process::id()));
        std::fs::create_dir_all(&p).expect("node home");
        std::env::set_var("OO_NODE_HOME", &p);
        p
    })
}

fn workspace(tag: &str) -> nlang_interpreter::ScratchDir {
    node_home();
    nlang_interpreter::ScratchDir::new(&format!("reads-as-no-key-{tag}"))
}

fn key_path(ws: &Path) -> PathBuf {
    Identity::node_key_path(ws).expect("node key path")
}

/// A real key, minted through the engine's own API.
fn mint(ws: &Path) -> String {
    let p = key_path(ws);
    std::fs::create_dir_all(p.parent().expect("parent")).expect("mkdir");
    let id = Identity::load_or_mint(&p).expect("mint");
    id.node_id_caid().to_string()
}

/// `n` records that all claim the same `received_at` second. `admission_seq`
/// is present and distinct, so arrival order IS total here: any tie in these
/// fixtures is one the engine manufactured, not one the fixture supplied.
fn write_directory(ws: &Path, owner: &str, n: usize) {
    let path = peers::directory_path(ws);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    let mut text = format!("# {} node_id={owner}\n", peers::FORMAT_TAG);
    for i in 0..n {
        let tag = format!("{:x}", 0x2000 + i);
        let node_id = format!("hash:sha256:v1:{tag:0>64}");
        text.push_str(&format!(
            "{{\"ad\":\"{{{{ node_id: \\\"{node_id}\\\" }}}}\",\"node_id\":\"{node_id}\",\
             \"public_key\":\"{:0>64}\",\"services\":[],\"listen_port\":9000,\"capacity\":10,\
             \"ts\":1700000000,\"ttl\":86400,\"observed_host\":\"127.0.0.1\",\"hops\":0,\
             \"addr\":\"127.0.0.1:9000\",\"provenance\":\"direct\",\"received_at\":1700000000,\
             \"admission_seq\":{}}}\n",
            tag,
            i + 1
        ));
    }
    std::fs::write(&path, text).expect("write directory");
}

/// Load the durable directory the way the engine does, with an explicitly
/// supplied identity. `peers::load` does not verify signatures -- that is a
/// separate step -- so a directory can be written by hand and handed to the
/// real parser, the real comparator and the real routing index.
fn loaded(ws: &Path, id: Option<&str>) -> (Vec<PeerAdvert>, usize) {
    let (by_id, routing, _, _) = peers::load(ws, id);
    (by_id.into_values().collect(), routing.total())
}

/// Node ids in admission order, under the identity `id`.
fn admission_order(records: &[PeerAdvert], id: Option<&str>) -> Vec<String> {
    let mut v = records.to_vec();
    v.sort_by(|a, b| peers::cmp_admission_order(a, b, id));
    v.into_iter().map(|a| a.node_id).collect()
}

/// The `admission_seq` every record came back with. All zeros means the
/// engine discarded the total order the fixture supplied.
fn seqs(records: &[PeerAdvert]) -> Vec<u64> {
    let mut v: Vec<u64> = records.iter().map(|a| a.admission_seq).collect();
    v.sort_unstable();
    v
}

/// What an ordinary open concludes about this workspace's identity. This is
/// the reader under repair; everything else here is downstream of it.
fn identity_as_read(ws: &Path) -> Option<String> {
    Ouroboros::init(ws).expect("init").node_id_if_present()
}

/// Hold the key empty, complete it after `WINDOW`, and run `reader` in
/// between. Returns what the reader saw.
///
/// ⚠ ACCEPTOR REPAIR, 2026-09-22, Q-054 acceptance. The first version of this
/// helper asserted `read_done < wrote_at`, meaning "the reader finished before
/// the key was completed, so it must have seen the incomplete file". That
/// guard has inverted polarity: **a reader that waits for the key necessarily
/// finishes after the writer**, so the guard voided precisely the fixed state
/// and could never go green. It was calibrated only against the broken engine,
/// where it never fires. The delivery hit it, reported `VOID READING` rather
/// than editing the probe, and was right to.
///
/// What is checked now is the only precondition that is both necessary and
/// observable from outside: the reader was invoked while the key was empty.
/// That the reader then WAITED rather than merely being slow is not provable
/// here -- `r5` proves it separately, and without `r5` these two could pass
/// for the wrong reason.
const WINDOW: u64 = 60;

fn observed_mid_write<T: std::fmt::Debug>(key: &Path, reader: impl FnOnce() -> T) -> T {
    let complete = std::fs::read(key).expect("read key");
    std::fs::write(key, b"").expect("truncate");
    assert_eq!(
        std::fs::metadata(key).expect("stat").len(),
        0,
        "precondition: the key must be empty when the reader is invoked"
    );
    let late = {
        let key = key.to_path_buf();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(WINDOW));
            std::fs::write(&key, &complete).expect("late write");
        })
    };
    let seen = reader();
    late.join().expect("late writer");
    seen
}

const N: usize = 6;

// ─────────────────────────────────────────────────────────────────────────
// CONTROLS — an assertion about degradation is vacuous if nothing loaded.
// ─────────────────────────────────────────────────────────────────────────

/// C1. The fixture reaches the target: with the owning identity supplied,
/// every record loads, the routing index is seeded with all of them, and the
/// total arrival order the fixture wrote survives the load.
#[test]
fn c1_a_readable_identity_seats_the_whole_directory() {
    let ws = workspace("c1");
    let owner = mint(ws.path());
    write_directory(ws.path(), &owner, N);

    let (records, total) = loaded(ws.path(), Some(&owner));
    assert_eq!(records.len(), N, "the measurement must reach all {N} records");
    assert_eq!(total, N, "a known identity seeds the routing index");
    assert_eq!(
        seqs(&records),
        (1..=N as u64).collect::<Vec<_>>(),
        "the fixture's total arrival order must survive the load"
    );
    assert_eq!(
        identity_as_read(ws.path()).as_deref(),
        Some(owner.as_str()),
        "control: an ordinary open reads the complete key"
    );
}

/// C2. With arrival order total the tie-break never runs, so the admission
/// order is the fixture's order. Pinned so a change to the ORDERING rule
/// cannot hide inside this arc.
#[test]
fn c2_a_total_arrival_order_decides_without_a_tie_break() {
    let ws = workspace("c2");
    let owner = mint(ws.path());
    write_directory(ws.path(), &owner, N);

    let (records, _) = loaded(ws.path(), Some(&owner));
    let expected: Vec<String> = (0..N)
        .map(|i| format!("hash:sha256:v1:{:0>64}", format!("{:x}", 0x2000 + i)))
        .collect();
    assert_eq!(admission_order(&records, Some(&owner)), expected, "admission_seq 1..N decides");
}

// ─────────────────────────────────────────────────────────────────────────
// RED — what this arc must make true.
// ─────────────────────────────────────────────────────────────────────────

/// R1 (I1). The transient case Q-051 already ruled on: the key file exists and
/// is not yet a key, and becomes one well inside the 100ms budget
/// `load_after_race` already spends at the one site that waits. An ordinary
/// open must not conclude that this workspace has no identity.
///
/// Baseline: `None` — read once, at the wrong moment.
#[test]
fn r1_a_key_being_written_does_not_read_as_no_key() {
    let ws = workspace("r1");
    let owner = mint(ws.path());
    let key = key_path(ws.path());

    let seen = observed_mid_write(&key, || identity_as_read(ws.path()));

    assert_eq!(
        seen.as_deref(),
        Some(owner.as_str()),
        "the identity is there {WINDOW}ms into a 100ms budget, and one reader \
         already waits for exactly this"
    );
}

/// R2 (I1). What that `None` costs downstream, composed through the engine's
/// own identity computation rather than asserted about a constant: the
/// observer half of every record must survive. `admission_seq` going to 0 is
/// the engine MANUFACTURING the tie it will then break by peer id alone.
///
/// Baseline: every `admission_seq` is 0 and the routing index is empty.
#[test]
fn r2_a_key_being_written_does_not_discard_the_arrival_order() {
    let ws = workspace("r2");
    let owner = mint(ws.path());
    write_directory(ws.path(), &owner, N);
    let key = key_path(ws.path());

    let as_read = observed_mid_write(&key, || identity_as_read(ws.path()));

    let (records, total) = loaded(ws.path(), as_read.as_deref());
    assert_eq!(
        seqs(&records),
        (1..=N as u64).collect::<Vec<_>>(),
        "a transiently unreadable key must not reset every admission_seq to 0"
    );
    assert_eq!(total, N, "nor empty the routing index");
}

/// R3 (I2). The category, not the case: "present but unreadable" must not be
/// observationally identical to "absent". This names no mechanism — a refusal
/// from `init`, a reported degradation, anything that differs satisfies it.
/// What must stop is the silence.
///
/// Baseline: the two states are indistinguishable.
#[test]
fn r3_unreadable_is_not_the_same_observation_as_absent() {
    let unreadable = {
        let ws = workspace("r3-bad");
        let owner = mint(ws.path());
        write_directory(ws.path(), &owner, N);
        std::fs::write(key_path(ws.path()), vec![0x30u8; 20]).expect("corrupt");
        match Ouroboros::init(ws.path()) {
            Ok(e) => {
                let id = e.node_id_if_present();
                let (records, total) = loaded(ws.path(), id.as_deref());
                Some((id, total, seqs(&records)))
            }
            Err(_) => None,
        }
    };
    let absent = {
        let ws = workspace("r3-none");
        let owner = format!("hash:sha256:v1:{:0>64}", "feed");
        write_directory(ws.path(), &owner, N);
        assert!(!key_path(ws.path()).exists(), "control: no key at all");
        match Ouroboros::init(ws.path()) {
            Ok(e) => {
                let id = e.node_id_if_present();
                let (records, total) = loaded(ws.path(), id.as_deref());
                Some((id, total, seqs(&records)))
            }
            Err(_) => None,
        }
    };

    assert!(absent.is_some(), "control: a workspace with no node key must still open");
    assert_ne!(
        unreadable, absent,
        "an identity file that exists and cannot be read must not be reported \
         exactly as one that is not there"
    );
}

/// R4 (I2, and REAL_02 C′ from the seat_order arc). C′ says a tie must not be
/// broken by the peer's identity alone. It was left unprobed there because it
/// was judged unreachable once arrival order was total. An unreadable identity
/// makes it reachable on ANY directory: it zeroes every `admission_seq`, so
/// the tie is manufactured, and `local_seat_rank` then loses its salt.
///
/// Operationally: two workspaces holding DIFFERENT identities, both
/// unreadable, over the same records, must not silently agree on the order.
/// Either they refuse, or their orders differ. Agreement in silence is what
/// makes grinding one key profitable on every such node at once.
#[test]
fn r4_two_victims_do_not_agree_on_an_order_neither_could_read() {
    let mut outcomes: Vec<Option<Vec<String>>> = Vec::new();
    for tag in ["r4-a", "r4-b"] {
        let ws = workspace(tag);
        let owner = mint(ws.path()); // a distinct identity per workspace
        write_directory(ws.path(), &owner, N);
        std::fs::write(key_path(ws.path()), vec![0x30u8; 20]).expect("corrupt");
        match Ouroboros::init(ws.path()) {
            Ok(e) => {
                let id = e.node_id_if_present();
                let (records, _) = loaded(ws.path(), id.as_deref());
                outcomes.push(Some(admission_order(&records, id.as_deref())));
            }
            Err(_) => outcomes.push(None), // a refusal satisfies I2
        }
    }
    let refused = outcomes.iter().any(|o| o.is_none());
    let agreed = matches!((&outcomes[0], &outcomes[1]), (Some(a), Some(b)) if a == b);
    assert!(
        refused || !agreed,
        "two nodes that cannot read their own identity must not compute the \
         same admission order: that is C′, a tie broken by the peer's identity \
         alone. got {outcomes:?}"
    );
}

/// R5 (I1, and what keeps `r1`/`r2` from being vacuous). `r1` and `r2` assert
/// the OUTCOME: the identity was read. They cannot tell a reader that waited
/// from a reader that happened to be slow enough to miss the window. This one
/// can, and it is the only assertion in this file that is about time.
///
/// With a key that is empty and stays empty, a reader that gives the writer
/// the already-ruled budget must spend most of it before concluding; a reader
/// that looks once concludes immediately. Measured on the baseline build:
/// `oo status` took 12–14ms for every key state, good or corrupt. A lower
/// bound on a sleep is safe to assert; an upper bound would not be.
///
/// Baseline (before the repair): concludes in ~1ms -> red.
#[test]
fn r5_a_key_that_never_completes_still_costs_the_ruled_budget() {
    let ws = workspace("r5");
    mint(ws.path());
    std::fs::write(key_path(ws.path()), b"").expect("truncate");

    let t0 = std::time::Instant::now();
    let outcome = Ouroboros::init(ws.path());
    let spent = t0.elapsed();

    assert!(
        spent >= std::time::Duration::from_millis(WINDOW),
        "a reader that concludes in {spent:?} never gave the writer the window \
         Q-051 already ruled on; r1/r2 would then be green by luck"
    );
    // Which way it concludes is I2's question, not this test's: either it
    // refuses, or it reports no identity. What is pinned here is that it
    // waited first.
    let concluded_absent = matches!(&outcome, Ok(e) if e.node_id_if_present().is_none());
    assert!(
        outcome.is_err() || concluded_absent,
        "control: a permanently empty key must not somehow yield an identity"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// RED LINES — green today; the repair must not buy I1/I2 with any of these.
// ─────────────────────────────────────────────────────────────────────────

/// G1 (I3). Waiting must not become accepting. Q-051's G2/D2 restated at the
/// library boundary: a permanently invalid key stays an error, and its bytes
/// are left exactly as they were.
#[test]
fn g1_a_permanently_invalid_key_is_still_an_error() {
    let ws = workspace("g1");
    for (name, bytes) in [("empty", Vec::new()), ("truncated", vec![0x30u8; 20])] {
        let p = ws.path().join(format!("bad-{name}"));
        std::fs::write(&p, &bytes).expect("write");
        let before = std::fs::read(&p).expect("read back");
        let err = Identity::load(&p).err();
        assert!(err.is_some(), "a {name} key must not load");
        assert!(
            format!("{:?}", err.expect("err")).contains("not a valid PKCS#8"),
            "the error must name what is wrong with it ({name})"
        );
        assert_eq!(std::fs::read(&p).expect("after"), before, "D2: bytes unchanged ({name})");
    }
}

/// G2 (I4). No read path may mint. Opening a workspace that has no node key
/// must leave it without one.
#[test]
fn g2_opening_a_workspace_does_not_mint() {
    let ws = workspace("g2");
    let owner = format!("hash:sha256:v1:{:0>64}", "beef");
    write_directory(ws.path(), &owner, N);
    let p = key_path(ws.path());
    assert!(!p.exists(), "control: no key before");
    assert_eq!(identity_as_read(ws.path()), None);
    assert!(!p.exists(), "opening a workspace must not create a node key");
}

/// G3. The property C′ buys, proven to exist so R4 reads as a LOSS of it
/// rather than as its absence: two readable identities, over the same records,
/// must break an actual tie differently. The tie is reached with the legacy
/// shape — no `admission_seq` at all, so every record ties at 0.
#[test]
fn g3_two_readable_victims_break_a_tie_differently() {
    let mut orders = Vec::new();
    for tag in ["g3-a", "g3-b"] {
        let ws = workspace(tag);
        let owner = mint(ws.path());
        let path = peers::directory_path(ws.path());
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        let mut text = format!("# {} node_id={owner}\n", peers::FORMAT_TAG);
        for i in 0..12 {
            let t = format!("{:x}", 0x3000 + i);
            let node_id = format!("hash:sha256:v1:{t:0>64}");
            text.push_str(&format!(
                "{{\"ad\":\"{{{{ node_id: \\\"{node_id}\\\" }}}}\",\"node_id\":\"{node_id}\",\
                 \"public_key\":\"{:0>64}\",\"services\":[],\"listen_port\":9000,\"capacity\":10,\
                 \"ts\":1700000000,\"ttl\":86400,\"observed_host\":\"127.0.0.1\",\"hops\":0,\
                 \"addr\":\"127.0.0.1:9000\",\"provenance\":\"direct\",\"received_at\":1700000000}}\n",
                t
            ));
        }
        std::fs::write(&path, text).expect("write");

        let (records, _) = loaded(ws.path(), Some(&owner));
        assert_eq!(records.len(), 12, "control: the fixture reached the parser");
        assert_eq!(seqs(&records), vec![0u64; 12], "control: the tie is reached");
        orders.push(admission_order(&records, Some(&owner)));
    }
    assert_ne!(
        orders[0], orders[1],
        "the salt exists so two victims order a tie differently; if this is red, \
         `local_seat_rank` lost its salt for a readable identity too"
    );
}
