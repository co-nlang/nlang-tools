// A key that was not finished being written.
// Recon: nlang-tools/docs/a_key_that_was_not_finished_being_written_recon.md
// Order: nlang-tools/docs/a_key_that_was_not_finished_being_written_handover.md
//
// -- What this arc is ----------------------------------------------------
//
// `create_new` claims the identity path in one syscall and the private key
// bytes are written after it returns, so between those two moments the file
// exists and is not yet a key. A process that lost the `create_new` race is
// protected -- `load_after_race` retries for a bounded 100ms. A process that
// never raced is not: `load_or_mint` opens with
//
//     if path.exists() { return Self::load(path); }
//
// one attempt, no retry. The earlier repair guarded the racer and left the
// bystander, and the bystander is the one that fails.
//
// -- What is NOT the defect, stated because the acceptor got it wrong once -
//
// A permanently invalid key file MUST keep failing. The acceptor's first
// "mechanism confirmed" measurement truncated a key to 0 bytes, saw the
// expected error text, and recorded that as the defect. It is not: nothing
// is going to finish writing that file, so retrying forever would still be
// an error, and a repair that made that case pass would be worse than the
// bug. G2 below pins that.
//
// The defect is only the TRANSIENT case: incomplete now, complete in
// microseconds. R1 constructs exactly that and nothing else.
//
// NOT PROBED, so silence is not mistaken for coverage:
//   * Whether the node key path (`resolve_node_home`) has the same shape.
//   * Whether 100 x 1ms is enough under whole-tree load.
//   * `pin_concurrent_first_mint_yields_one_key` belongs to another arc and
//     is NOT touched here. It tests this property and goes red about one run
//     in three; R1 makes the same property deterministic. It should stop
//     flaking once this is fixed, but that is an inference, not a claim of
//     this arc -- its randomness has a second possible source.

use std::path::{Path, PathBuf};
use std::process::Command;

fn scratch(tag: &str) -> nlang_interpreter::ScratchDir {
    nlang_interpreter::ScratchDir::new(&format!("keywrite-{tag}"))
}

/// `oo identity` with the identity file at `key`, run from `dir`.
fn identity(dir: &Path, key: &Path) -> (String, i32) {
    let o = Command::new(env!("CARGO_BIN_EXE_oo"))
        .arg("identity")
        .current_dir(dir)
        .env("OO_IDENTITY", key)
        .env("OO_NODE_HOME", dir.join("node-home-for-tests"))
        .output()
        .expect("oo runs");
    (
        format!(
            "{}{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        ),
        o.status.code().unwrap_or(-1),
    )
}

fn hex64(out: &str) -> Option<String> {
    out.split_whitespace()
        .find(|t| t.len() == 64 && t.chars().all(|c| c.is_ascii_hexdigit()))
        .map(|s| s.to_string())
}

/// Mint a real key once and hand back its bytes, to use as the material a
/// late writer finishes with.
fn material(dir: &Path) -> (PathBuf, Vec<u8>, String) {
    let key = dir.join("material-key");
    let (out, rc) = identity(dir, &key);
    assert_eq!(rc, 0, "REACH: minting the material key: {out}");
    let hex = hex64(&out).expect("REACH: a 64-hex key in the output");
    let bytes = std::fs::read(&key).expect("REACH: the material key is on disk");
    assert!(
        !bytes.is_empty(),
        "REACH: the material key has bytes ({} B)",
        bytes.len()
    );
    (key, bytes, hex)
}

// ---------------------------------------------------------------------
// G1. The control. A complete key loads, and the key reported is the key
// in the file. If this goes red nothing else here means anything.
// ---------------------------------------------------------------------
#[test]
fn g1_a_complete_key_loads_and_is_the_key_reported() {
    let s = scratch("g1");
    let d = s.path();
    let (_, bytes, hex) = material(d);

    let key = d.join("k1");
    std::fs::write(&key, &bytes).expect("write k1");
    let (out, rc) = identity(d, &key);
    assert_eq!(rc, 0, "CONTROL: a complete key must load: {out}");
    assert_eq!(
        hex64(&out).as_deref(),
        Some(hex.as_str()),
        "CONTROL: the reported key must be the one in the file: {out}"
    );
}

// ---------------------------------------------------------------------
// G2. RED LINE. A permanently invalid key must stay refused, by name,
// with the file left alone. This is the assertion a careless "just retry"
// or "mint a fresh one on failure" repair breaks, and breaking it is
// worse than the bug this arc fixes.
// ---------------------------------------------------------------------
#[test]
fn g2_a_permanently_invalid_key_is_still_refused() {
    let s = scratch("g2");
    let d = s.path();

    for (name, contents) in [("empty", Vec::new()), ("truncated", vec![0x30u8; 20])] {
        let key = d.join(format!("bad-{name}"));
        std::fs::write(&key, &contents).expect("write bad key");
        let before = std::fs::read(&key).expect("read back");

        let (out, rc) = identity(d, &key);
        assert_ne!(
            rc, 0,
            "a {name} key file must be refused, not accepted: {out}"
        );
        assert!(
            out.contains("not a valid PKCS#8"),
            "the refusal must name what is wrong with it: {out}"
        );
        assert_eq!(
            std::fs::read(&key).expect("read after"),
            before,
            "D2: the key file must be left exactly as it was ({name})"
        );
    }
}

// ---------------------------------------------------------------------
// R1. The defect. A key file that is incomplete NOW and complete a
// moment later must be waited out, not reported as corrupt.
//
// This is the `path.exists()` branch of `load_or_mint`: the file is there,
// so it never enters the race and never reaches the bounded retry that
// `load_after_race` already has. 20ms against a 100ms budget.
// ---------------------------------------------------------------------
#[test]
fn r1_a_key_still_being_written_is_waited_for() {
    let s = scratch("r1");
    let d = s.path();
    let (_, bytes, hex) = material(d);

    let key = d.join("k-transient");
    // The window: the name is claimed, the bytes are not there yet.
    std::fs::write(&key, b"").expect("claim the name");

    let late = key.clone();
    let payload = bytes.clone();
    let writer = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(&late, &payload).expect("the late writer finishes");
    });

    let (out, rc) = identity(d, &key);
    writer.join().expect("writer thread");

    assert_eq!(
        rc, 0,
        "a key that was mid-write when we looked was reported as corrupt. \
         `load_after_race` already waits up to 100ms for exactly this, but \
         only on the branch that lost the create_new race; a process that \
         found the file already present takes `Self::load` once: {out}"
    );
    assert_eq!(
        hex64(&out).as_deref(),
        Some(hex.as_str()),
        "having waited, the key reported must be the one that landed -- not \
         a freshly minted one: {out}"
    );
}

// ---------------------------------------------------------------------
// G3. RED LINE (green today) -- the property the earlier repair bought,
// restated so this arc cannot sell it back: whatever happens, a process
// must never report a key that is not the key in the file.
//
// It is green today because the engine refuses and prints nothing. A
// repair that answered the window by minting its own key instead of
// waiting would make R1 green and this red, which is the whole point of
// keeping it.
// ---------------------------------------------------------------------
#[test]
fn g3_no_process_reports_a_key_that_is_not_on_disk() {
    let s = scratch("r2");
    let d = s.path();
    let (_, bytes, hex) = material(d);

    let key = d.join("k-witness");
    std::fs::write(&key, b"").expect("claim the name");
    let late = key.clone();
    let payload = bytes.clone();
    let writer = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(&late, &payload).expect("the late writer finishes");
    });

    let (out, rc) = identity(d, &key);
    writer.join().expect("writer thread");

    let on_disk = std::fs::read(&key).expect("read the key back");
    assert_eq!(on_disk, bytes, "REACH: the late writer's bytes are the ones there");

    if rc == 0 {
        assert_eq!(
            hex64(&out).as_deref(),
            Some(hex.as_str()),
            "a process reported a key that is not the one in the file: {out}"
        );
    } else {
        assert!(
            hex64(&out).is_none(),
            "a refusal must not also print a key: {out}"
        );
    }
}
