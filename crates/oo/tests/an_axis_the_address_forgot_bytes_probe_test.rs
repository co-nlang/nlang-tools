// An axis the address forgot — the bytes an epoch-1 store receives.
// Order: nlang-tools/docs/an_axis_the_address_forgot_handover.md (§9, R-1)
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-083.  Ruling: D106; REAL_02 §5.1.1
// (a store declaring the older form receives that form).
//
// Separate from the CLI probe because it reads objects with the frozen
// encoding (`bn_serial::with_legacy_identity`, named by the order), which
// does not exist in v0.84.0 — this file does not compile there; the CLI
// probe must, so its guards can be calibrated on v0.84.0.
//
// The delivery may NOT edit this file or the fixtures. `rustfmt` must not
// touch it.

#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

struct Ws {
    _scratch: nlang_interpreter::ScratchDir,
    root: PathBuf,
    ws: PathBuf,
}

impl Ws {
    fn new(tag: &str) -> Self {
        let scratch = nlang_interpreter::ScratchDir::new(&format!("axis-forgot-bytes-{tag}"));
        let root = scratch.path().to_path_buf();
        let ws = root.join("ws");
        fs::create_dir_all(&ws).unwrap();
        Ws { _scratch: scratch, root, ws }
    }

    /// A workspace holding a copy of the epoch-1 store.
    fn epoch1(tag: &str) -> Self {
        let w = Ws::new(tag);
        copy_dir(&fixture(), &w.ws.join(".oo"));
        w
    }

    /// Run `oo`, killing it after 60 s.
    fn oo(&self, args: &[&str]) -> (String, i32) {
        let mut child = Command::new(env!("CARGO_BIN_EXE_oo"))
            .args(args)
            .current_dir(&self.ws)
            .env("OO_IDENTITY", self.root.join("identity"))
            .env("OO_NODE_HOME", self.root.join("node-home"))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("oo runs");
        let start = Instant::now();
        loop {
            if child.try_wait().expect("wait").is_some() {
                break;
            }
            if start.elapsed() > Duration::from_secs(60) {
                let _ = child.kill();
                let _ = child.wait();
                return (format!("TIMEOUT after 60 s: oo {}", args.join(" ")), -1);
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let o = child.wait_with_output().expect("output");
        (
            format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)).trim().to_string(),
            o.status.code().unwrap_or(-1),
        )
    }

    /// `oo eval`, whitespace collapsed to single spaces.
    fn eval(&self, e: &str) -> String {
        squash(&self.oo(&["eval", e]).0)
    }

    /// Evolve then commit `src`. Both must succeed: a reading taken after a
    /// failed commit is a reading of the previous universe.
    fn commit(&self, src: &str) {
        fs::write(self.ws.join("a.n"), src).unwrap();
        let (o, rc) = self.oo(&["evolve", "a.n"]);
        assert_eq!(rc, 0, "evolve {src:?}: {o}");
        let (o, rc) = self.oo(&["commit", "-m", "a"]);
        assert_eq!(rc, 0, "commit {src:?}: {o}");
    }

    fn head(&self) -> String {
        fs::read_to_string(self.ws.join(".oo").join("HEAD")).unwrap_or_default().trim().to_string()
    }

    /// The root digest of HEAD.
    fn root(&self) -> String {
        let (o, _) = self.oo(&["inspect", &self.head()]);
        o.lines()
            .find(|l| l.contains("root:"))
            .and_then(|l| l.rsplit(':').next())
            .unwrap_or("")
            .trim()
            .to_string()
    }

    fn objects_format(&self) -> String {
        fs::read_to_string(self.ws.join(".oo").join("objects.format")).unwrap_or_default().trim().to_string()
    }

    /// The on-disk object for a 64-hex digest.
    fn object_path(&self, digest: &str) -> PathBuf {
        self.ws.join(".oo/objects/sha256").join(&digest[..2]).join(&digest[2..])
    }
}

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/epoch1_repo/oo_dir")
}

fn tables_fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/epoch1_tables_repo/oo_dir")
}

fn read_decl(w: &Ws, name: &str) -> String {
    fs::read_to_string(w.ws.join(".oo").join(name)).unwrap_or_default().trim().to_string()
}


fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let dst = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &dst);
        } else {
            fs::copy(entry.path(), &dst).unwrap();
        }
    }
}

fn squash(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}


/// Every object in the store, as (64-hex name, text).
fn objects(w: &Ws) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let base = w.ws.join(".oo/objects/sha256");
    for d in fs::read_dir(&base).unwrap() {
        let d = d.unwrap();
        for f in fs::read_dir(d.path()).unwrap() {
            let f = f.unwrap();
            let name = format!("{}{}", d.file_name().to_string_lossy(), f.file_name().to_string_lossy());
            out.push((name, fs::read_to_string(f.path()).unwrap_or_default()));
        }
    }
    out.sort();
    out
}

/// The address the frozen epoch-1 encoding gives an object's text.
fn epoch1_address(text: &str) -> Option<String> {
    use nlang_interpreter::bn_serial::with_legacy_identity;
    use nlang_interpreter::store_codec::{commit_document_address, decode_value};
    with_legacy_identity(|| {
        if text.starts_with("#nlang/store commit") {
            commit_document_address(text).ok().map(|h| hex::encode(h.digest))
        } else {
            decode_value(text).ok().map(|v| hex::encode(v.content_hash().digest))
        }
    })
}

/// Branch and body names written into a stored value: `"{<64 hex>}"`.
fn digest_names(text: &str) -> std::collections::BTreeSet<String> {
    let b = text.as_bytes();
    let mut out = std::collections::BTreeSet::new();
    for i in 0..b.len().saturating_sub(66) {
        if b[i] == b'{' && b[i + 65] == b'}' && b[i + 1..i + 65].iter().all(|c| c.is_ascii_hexdigit()) {
            out.insert(text[i..i + 66].to_string());
        }
    }
    out
}

/// A store declaring encoding=5 receives epoch-1 bytes: after a write, every
/// object in it is still at its epoch-1 address, and the names written into
/// a table are the names `oo v0.84.0` wrote. Controls: the fixture itself,
/// untouched, passes the same checks.
#[test]
fn r10_an_epoch_1_store_receives_epoch_1_bytes() {
    let w = Ws::new("r10");
    copy_dir(&tables_fixture(), &w.ws.join(".oo"));
    let check = |when: &str| {
        let mut wrong = Vec::new();
        for (name, text) in objects(&w) {
            match epoch1_address(&text) {
                Some(a) if a == name => {}
                other => wrong.push(format!("  {name}: epoch-1 address {other:?}")),
            }
        }
        assert!(wrong.is_empty(), "{when}: objects not at their epoch-1 address:\n{}", wrong.join("\n"));
    };
    check("the fixture itself");
    let old_root = w.root();
    let old_names = digest_names(&fs::read_to_string(w.object_path(&old_root)).unwrap());
    assert!(old_names.len() >= 4, "control: the fixture root names its branches: {old_names:?}");
    // `k` and `b` are carried over unchanged; only `y` is new.
    w.commit("y: 1\n");
    check("after a commit");
    let new_root = w.root();
    assert_ne!(new_root, old_root);
    let new_names = digest_names(&fs::read_to_string(w.object_path(&new_root)).unwrap());
    let lost: Vec<_> = old_names.difference(&new_names).collect();
    assert!(lost.is_empty(), "the unchanged tables were rewritten with other names: {lost:?}");
}

