// A private field that was never committed — R-1: the refine shadow scan.
// Order: nlang-tools/docs/a_private_never_committed_handover.md §9
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-079.  Ruling: meta/oo/STATUS.md D103.
// Spec: REAL_03 §6 (`~` axes are axes of the value), SPEC_13 §5.2 #6.
//
// ── What R-1 is ──────────────────────────────────────────────────────────
//
// A refine lists the historical commits whose root holds a source CAID as a
// field (`Shadow: N historical commit(s) will be semantically updated`). The
// scan walked the public axis only. Before Q-079 no root held a `~` field of
// its own; from Q-079 one does, and the scan did not see it. Measured on the
// delivery (dev 4d177ae), source = the CAID of `{ z: 7 }`:
//   * `p: { z: 7 }`             Shadow: 1 historical commit(s)
//   * `~p: { z: 7 }` + `k: 1`   no Shadow line
// The delivery reported this itself (§8.4 Q1 #7) and left it. Repaired by the
// acceptor while no recorded `shadow` list can move.
//
// The delivery may NOT edit this file. `rustfmt` must not touch it.

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};

struct Ws {
    _scratch: nlang_interpreter::ScratchDir,
    root: PathBuf,
    ws: PathBuf,
}

impl Ws {
    fn new(tag: &str) -> Self {
        let scratch = nlang_interpreter::ScratchDir::new(&format!("private-never-committed-r1-{tag}"));
        let root = scratch.path().to_path_buf();
        let ws = root.join("ws");
        fs::create_dir_all(&ws).unwrap();
        Ws { _scratch: scratch, root, ws }
    }

    fn oo(&self, args: &[&str]) -> (String, i32) {
        let o = Command::new(env!("CARGO_BIN_EXE_oo"))
            .args(args)
            .current_dir(&self.ws)
            .env("OO_IDENTITY", self.root.join("identity"))
            .env("OO_NODE_HOME", self.root.join("node-home"))
            .stdin(Stdio::null())
            .output()
            .expect("oo runs");
        (
            format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)).trim().to_string(),
            o.status.code().unwrap_or(-1),
        )
    }

    fn commit(&self, src: &str) {
        fs::write(self.ws.join("a.n"), src).unwrap();
        let (o, rc) = self.oo(&["evolve", "a.n"]);
        assert_eq!(rc, 0, "evolve {src:?}: {o}");
        let (o, rc) = self.oo(&["commit", "-m", "a"]);
        assert_eq!(rc, 0, "commit {src:?}: {o}");
    }

    /// The CAID of `{ z: 7 }` in this universe's store.
    fn caid_z7(&self) -> String {
        fs::write(self.ws.join("s.n"), "x: ~%Engine./save { z: 7 }\n").unwrap();
        let (o, rc) = self.oo(&["run", "--observe", "x", "s.n"]);
        assert_eq!(rc, 0, "save: {o}");
        let caid = o.split('"').nth(1).unwrap_or("").to_string();
        assert!(caid.starts_with("hash:"), "no CAID in {o:?}");
        caid
    }

    fn head(&self) -> String {
        fs::read_to_string(self.ws.join(".oo").join("HEAD")).unwrap().trim().to_string()
    }

    /// Refine `{ z: 7 }` onto itself; the shadow lines it prints.
    fn shadow(&self) -> String {
        let c = self.caid_z7();
        let (o, rc) = self.oo(&["refine", "--source", &c, "--target", &c, "-m", "r"]);
        assert_eq!(rc, 0, "refine: {o}");
        o.lines()
            .skip_while(|l| !l.starts_with("Shadow:"))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// The scan's control: a public field holding the source is listed.
#[test]
fn s1_public_field_is_listed() {
    let w = Ws::new("s1");
    w.commit("p: { z: 7 }\n");
    let digest = w.head().rsplit(':').next().unwrap().to_string();
    let s = w.shadow();
    assert!(
        s.starts_with("Shadow: 1 historical commit(s)") && s.contains(&digest),
        "a root field holding the source: its commit is listed\n  got: {s:?}"
    );
}

/// R-1: a root `~` field holding the source is listed the same way.
#[test]
fn r1_root_private_field_is_listed() {
    let w = Ws::new("r1");
    w.commit("~p: { z: 7 }\nk: 1\n");
    let digest = w.head().rsplit(':').next().unwrap().to_string();
    let s = w.shadow();
    assert!(
        s.starts_with("Shadow: 1 historical commit(s)") && s.contains(&digest),
        "a root `~` field holding the source: its commit is listed (REAL_03 §6)\n  got: {s:?}"
    );
}

/// No false positive: a root `~` field holding something else is not listed.
#[test]
fn s2_other_private_field_is_not_listed() {
    let w = Ws::new("s2");
    w.commit("~p: { z: 8 }\nk: 1\n");
    let s = w.shadow();
    assert!(s.is_empty(), "nothing holds the source: no shadow\n  got: {s:?}");
}
