// A port you did not bind (Q-026, pre-committed by work order:
// docs/a_port_you_did_not_bind_handover.md).
//
// ── The claim ────────────────────────────────────────────────────────────
//
// A node must report the port it actually bound, not the port it was asked
// for.
//
// Measured 2026-08-15 on v0.23.0: `oo node serve --port 0` binds fine — the
// OS picks one — and the banner says
//     n/ OODP node serving at port 0 (node hash:sha256:v2:…)
// while `/proc/<pid>/fd` → `/proc/net/tcp` shows it listening on 40707. The
// one line that tells you where a node is, is false exactly when the OS chose
// the port. Same family as `.oo/format`: a marker reporting something other
// than the thing it measures.
//
// ── Why this is not only cosmetic ────────────────────────────────────────
//
// Because the banner cannot be trusted for the number, fourteen test files
// each guess a port instead, with their own copy of a `free_port()` that asks
// `127.0.0.1:0` and then hands a bare number to a child that binds
// `0.0.0.0:{port}`. Those are not the same question — measured, holding
// `127.0.0.1:P` makes `0.0.0.0:P` EADDRINUSE — so the collision partner need
// not be another test at all. That is why it has never reproduced when a
// suite is run alone, and why it has now aborted two full workspace runs, in
// two different files.
//
// ── Probe integrity ──────────────────────────────────────────────────────
//
// Reds are `#[ignore]`d and MUST be red at the baseline for the reason stated
// in each. The delivery may remove `#[ignore]` and nothing else in this file.
// C0 runs first: an assertion about a banner is vacuous if no node ever came
// up.

use std::io::Read;
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

const BANNER: &str = "n/ OODP node serving at port ";

fn fresh(tag: &str) -> nlang_interpreter::ScratchDir {
    let d = nlang_interpreter::ScratchDir::new(&format!("notbound-{tag}"));
    let _ = Command::new(env!("CARGO_BIN_EXE_oo"))
        .arg("init")
        .current_dir(&*d)
        .env("OO_IDENTITY", d.join("identity-for-tests"))
        .env("OO_NODE_HOME", d.join("node-home-for-tests"))
        .output();
    d
}

/// A port to ask for, obtained by asking **the same question the child will
/// ask** — `0.0.0.0:0`, not `127.0.0.1:0`. Only used by C0, which needs an
/// explicit port to check the banner against. After §3.1 no test needs this.
fn a_port_to_ask_for() -> u16 {
    let l = TcpListener::bind("0.0.0.0:0").unwrap();
    l.local_addr().unwrap().port()
}

struct Node {
    child: Child,
    log: std::path::PathBuf,
}

impl Node {
    fn log(&self) -> String {
        std::fs::read_to_string(&self.log).unwrap_or_default()
    }
    /// The port named on the banner, once it appears. `None` if no banner
    /// arrives within the window.
    fn banner_port(&self) -> Option<u16> {
        for _ in 0..60 {
            let text = self.log();
            if let Some(i) = text.find(BANNER) {
                let digits: String = text[i + BANNER.len()..]
                    .chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect();
                if !digits.is_empty() {
                    return digits.parse().ok();
                }
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        None
    }
    fn stop(mut self) {
        self.child.kill().ok();
        self.child.wait().ok();
    }
}

fn serve(dir: &Path, port: u16) -> Node {
    let log = dir.join(format!("serve-{port}.log"));
    let f = std::fs::File::create(&log).unwrap();
    let child = Command::new(env!("CARGO_BIN_EXE_oo"))
        .args(["node", "serve", "--port", &port.to_string()])
        .current_dir(dir)
        .env("OO_IDENTITY", dir.join("identity-for-tests"))
        .env("OO_NODE_HOME", dir.join("node-home-for-tests"))
        .stdout(Stdio::from(f.try_clone().unwrap()))
        .stderr(Stdio::from(f))
        .spawn()
        .unwrap();
    Node { child, log }
}

/// Does something answer OODP on this port? A bare TCP connect is enough —
/// the claim under test is "the number names a socket that exists".
fn reachable(port: u16) -> bool {
    for _ in 0..40 {
        if let Ok(mut s) = TcpStream::connect(("127.0.0.1", port)) {
            let _ = s.set_read_timeout(Some(Duration::from_millis(200)));
            let mut b = [0u8; 1];
            let _ = s.read(&mut b);
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    false
}

// ── C0 — control, runs first ─────────────────────────────────────────────

#[test]
fn c0_an_explicit_port_is_named_and_reachable() {
    let d = fresh("c0");
    let asked = a_port_to_ask_for();
    let node = serve(&d, asked);

    let named = node.banner_port();
    let log = node.log();
    let up = named.map(reachable).unwrap_or(false);
    node.stop();

    assert_eq!(
        named,
        Some(asked),
        "control failed: asked for port {asked}, banner said {named:?}. \
         Every probe in this file is vacuous until this passes.\n{log}"
    );
    assert!(up, "control failed: port {asked} was named but nothing answered.\n{log}");
}

// ── P1 — red: the banner must not name a port nobody bound ───────────────
// Baseline red: verbatim `n/ OODP node serving at port 0`.

#[test]
fn p1_port_zero_is_not_what_gets_reported() {
    let d = fresh("p1");
    let node = serve(&d, 0);
    let named = node.banner_port();
    let log = node.log();
    node.stop();

    assert!(
        named.is_some(),
        "no banner at all — the node did not start.\n{log}"
    );
    assert_ne!(
        named,
        Some(0),
        "the banner reports the port it was ASKED for, not the one it bound. \
         `0` is a request, never an address.\n{log}"
    );
}

// ── P2 — red: and the number it reports must be the real one ─────────────
// P1 alone is satisfied by printing any number at all. P2 pins it to the
// socket that actually exists. Standing rule: a red asserting a
// non-existence must assert an existence in the same run.

#[test]
fn p2_the_reported_port_is_the_one_you_can_reach() {
    let d = fresh("p2");
    let node = serve(&d, 0);
    let named = node.banner_port();
    let log = node.log();
    let up = named.map(reachable).unwrap_or(false);
    node.stop();

    let named = named.expect("no banner at all — the node did not start");
    assert!(
        up,
        "the banner named port {named}, but nothing answers there. \
         A reported port that cannot be reached is worse than no report.\n{log}"
    );
}
