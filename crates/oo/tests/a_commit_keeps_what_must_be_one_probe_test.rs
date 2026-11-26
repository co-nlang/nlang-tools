// Q-141 probe: a commit keeps what must be one value.
//
// Q-140 keyed a field of a root read back from the store on that root, so a
// commit keeps what the field gave (Q-126): the answer of an effect, and a
// record, so that the name is one record whoever reads it. A record with
// nothing lazy in it reads the same however often it is read; keeping it
// buys no sameness. But a commit kept every one of them at once, each in
// full (values share no structure, Inbox I-044): a chain of lists
// `k_i: [k_{i+1}]` holds, at link i, a list expanded to the depth limit.
//
// v0.140.0: the commit's peak 730 MB at N=1000 (v0.139.0 116), 1501 at 2000
// (140); at N=4000 under 3 GB the commit is refused where v0.139.0 stores it
// in 81 s and 183 MB (Inbox I-323).
//
// r1 — a commit's memory grows with what it writes: a chain of lists
//      committed at N and 4N, the commit's peak under x1.6.
// r2 — what must be one value still is: a record a call makes, held below
//      the top of what a field gives (in a list, a tuple, a nested record,
//      a union),
//      read by two readers, its function written later or pinned: the effect
//      is done once, status does none.
// g1 — roots and readings do not change, as v0.140.0: written at once, its
//      function written later, pinned.
//
// Not here: values share no structure (I-044) — so a chain of lists commits
// in the time v0.139.0 took (v0.140.0 was twice as fast by holding every
// link); a list's elements are expanded to the depth limit (I-051).

use std::fs;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const PURE: &str = "g: (s -> s)";
const GA: &str = "g: (s -> ~%Io./append_file (\"a.log\", s))";

// (label, readers, a reading)
const SHAPES: &[(&str, &str, &str)] = &[
    ("a list holding a record a call makes, an alias", "mk: (s -> { e: g s })\nk: [mk \"x\"]\nk2: k", "_.k2.0.e"),
    ("a list holding a record a call makes, read twice", "mk: (s -> { e: g s })\nk: [mk \"x\"]\na: k.0.e\nb: k.0.e", "_.b"),
    ("a list in a list holding a record, an alias", "mk: (s -> { e: g s })\nk: [[mk \"x\"]]\nk2: k", "_.k2.0.0.e"),
    ("a tuple holding a record a call makes, an alias", "mk: (s -> { e: g s })\nk: ([mk \"x\"], 1)\nk2: k", "_.k2.0.0.e"),
    ("a call makes a nested record, an alias", "mk: (s -> { r: { e: g s } })\nk: mk \"x\"\nk2: k", "_.k2.r.e"),
    ("a list holding a written record, an alias", "k: [{ e: g \"x\" }]\nk2: k", "_.k2.0.e"),
    ("a union holding a list holding a record, an alias", "mk: (s -> { e: g s })\nk: [mk \"x\"] | 5\nk2: k", "_.k2"),
];

// g1: what v0.140.0 answers (measured on its tag build).
const WANT_G1: &[(&str, &str)] = &[
    ("a list holding a record a call makes, an alias, g at once", r#"0 0 | 27fd10dbdbab31c3 | _.k2.0.e=#true ;; %effect: #io"#),
    ("a list holding a record a call makes, an alias, g later", r#"0 0 0 0 0 0 | d0f1f853f0adf854 | _.k2.0.e=#true ;; %effect: #io"#),
    ("a list holding a record a call makes, an alias, g pinned", r#"0 0 0 0 0 0 | d0f1f853f0adf854 | _.k2.0.e=#true ;; %effect: #io"#),
    ("a list holding a record a call makes, read twice, g at once", r#"0 0 | 44727608f4e8a302 | _.b=#true ;; %effect: #io"#),
    ("a list holding a record a call makes, read twice, g later", r#"0 0 0 0 0 0 | 1ab80b791197c16e | _.b=#true ;; %effect: #io"#),
    ("a list holding a record a call makes, read twice, g pinned", r#"0 0 0 0 0 0 | 1ab80b791197c16e | _.b=#true ;; %effect: #io"#),
    ("a list in a list holding a record, an alias, g at once", r#"0 0 | 3a44734e30e76c9b | _.k2.0.0.e=#true ;; %effect: #io"#),
    ("a list in a list holding a record, an alias, g later", r#"0 0 0 0 0 0 | a21458571cd3b3f9 | _.k2.0.0.e=#true ;; %effect: #io"#),
    ("a list in a list holding a record, an alias, g pinned", r#"0 0 0 0 0 0 | a21458571cd3b3f9 | _.k2.0.0.e=#true ;; %effect: #io"#),
    ("a tuple holding a record a call makes, an alias, g at once", r#"0 0 | ebe53a2ce473f0f2 | _.k2.0.0.e=#true ;; %effect: #io"#),
    ("a tuple holding a record a call makes, an alias, g later", r#"0 0 0 0 0 0 | fc0323b2d9bb5a9e | _.k2.0.0.e=#true ;; %effect: #io"#),
    ("a tuple holding a record a call makes, an alias, g pinned", r#"0 0 0 0 0 0 | fc0323b2d9bb5a9e | _.k2.0.0.e=#true ;; %effect: #io"#),
    ("a call makes a nested record, an alias, g at once", r#"0 0 | 1682f0f271ed38fc | _.k2.r.e=#true ;; %effect: #io"#),
    ("a call makes a nested record, an alias, g later", r#"0 0 0 0 0 0 | 5f94740aa66bd983 | _.k2.r.e=#true ;; %effect: #io"#),
    ("a call makes a nested record, an alias, g pinned", r#"0 0 0 0 0 0 | 5f94740aa66bd983 | _.k2.r.e=#true ;; %effect: #io"#),
    ("a list holding a written record, an alias, g at once", r#"0 0 | 6ac9c4d9c4661292 | _.k2.0.e=#true ;; %effect: #io"#),
    ("a list holding a written record, an alias, g later", r#"0 0 0 0 0 0 | 1101681aaa49500b | _.k2.0.e=#true ;; %effect: #io"#),
    ("a list holding a written record, an alias, g pinned", r#"0 0 0 0 0 0 | 1101681aaa49500b | _.k2.0.e=#true ;; %effect: #io"#),
    ("a union holding a list holding a record, an alias, g at once", r#"0 0 | ae295ecf2ff9feec | _.k2=5 ;; %effect: #io | [{ %effect: #io e: #true ;; %effect: #io } ;; %effect: #io]"#),
    ("a union holding a list holding a record, an alias, g later", r#"0 0 0 0 0 0 | 519695d7f7a5c1e2 | _.k2=5 ;; %effect: #io | [{ %effect: #io e: #true ;; %effect: #io } ;; %effect: #io]"#),
    ("a union holding a list holding a record, an alias, g pinned", r#"0 0 0 0 0 0 | 519695d7f7a5c1e2 | _.k2=5 ;; %effect: #io | [{ %effect: #io e: #true ;; %effect: #io } ;; %effect: #io]"#),
    ("a chain of lists, N=50", r#"0 0 | 53db557c9a8c1bb0 | _.k45=[[[[[7]]]]]"#),
];

struct Ws {
    _scratch: nlang_interpreter::ScratchDir,
    root: std::path::PathBuf,
    ws: std::path::PathBuf,
}

/// Peak resident memory of a running process, in KB (Linux; 0 elsewhere).
fn peak_kb(pid: u32) -> u64 {
    fs::read_to_string(format!("/proc/{pid}/status"))
        .ok()
        .and_then(|s| s.lines().find(|l| l.starts_with("VmHWM:")).and_then(|l| l.split_whitespace().nth(1)).and_then(|v| v.parse().ok()))
        .unwrap_or(0)
}

impl Ws {
    fn new(tag: &str) -> Self {
        let scratch = nlang_interpreter::ScratchDir::new(&format!("keeps-{tag}"));
        let root = scratch.path().to_path_buf();
        let ws = root.join("ws");
        fs::create_dir_all(&ws).unwrap();
        Ws { _scratch: scratch, root, ws }
    }

    /// Run `oo` capped at 3 GB, killed after `kill`: (output, rc, seconds,
    /// peak KB). The output goes to a file, not a pipe. The peak is read
    /// while it runs, every 10 ms.
    fn oo(&self, args: &[&str], kill: Duration) -> (String, i32, f64, u64) {
        let out_path = self.root.join("out.txt");
        let out = fs::File::create(&out_path).expect("output file");
        let err = out.try_clone().expect("output file");
        let mut cmd = Command::new("sh");
        cmd.arg("-c")
            .arg("ulimit -v 3000000; exec \"$0\" \"$@\"")
            .arg(env!("CARGO_BIN_EXE_oo"))
            .args(args)
            .current_dir(&self.ws)
            .env("OO_IDENTITY", self.root.join("identity"))
            .env("OO_NODE_HOME", self.root.join("node-home"))
            .env_remove("OO_Q117_SHADOW")
            .stdin(Stdio::null())
            .stdout(Stdio::from(out))
            .stderr(Stdio::from(err));
        let mut child = cmd.spawn().expect("oo runs");
        let pid = child.id();
        let start = Instant::now();
        let mut peak = 0;
        loop {
            peak = peak.max(peak_kb(pid));
            if child.try_wait().expect("wait").is_some() {
                break;
            }
            if start.elapsed() > kill {
                let _ = child.kill();
                let _ = child.wait();
                return ("DID NOT RETURN".into(), -1, start.elapsed().as_secs_f64(), peak);
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let secs = start.elapsed().as_secs_f64();
        let status = child.wait().expect("wait");
        let text = String::from_utf8_lossy(&fs::read(&out_path).unwrap_or_default()).to_string();
        let rc = status.code().unwrap_or(-1);
        if text.contains("panicked") {
            return ("PANIC".into(), rc, secs, peak);
        }
        if text.contains("stack overflow") || text.contains("memory allocation") || (rc != 0 && text.trim().is_empty()) {
            return ("CRASHED".into(), rc, secs, peak);
        }
        (text, rc, secs, peak)
    }

    fn run(&self, args: &[&str]) -> i32 {
        self.oo(args, Duration::from_secs(60)).1
    }

    fn file(&self, name: &str, src: &str) {
        fs::write(self.ws.join(name), format!("{src}\n")).unwrap();
    }

    /// Bytes an effect appended to `a.log`.
    fn bytes(&self) -> usize {
        fs::read(self.ws.join("a.log")).map(|b| b.len()).unwrap_or(0)
    }

    fn read(&self, q: &str) -> String {
        squash(&self.oo(&["eval", "--", q], Duration::from_secs(60)).0)
    }

    /// The committed root's address, last 16 hex digits.
    fn root_tail(&self) -> String {
        let head = fs::read_to_string(self.ws.join(".oo").join("HEAD")).unwrap_or_default();
        let o = self.oo(&["inspect", head.trim()], Duration::from_secs(60)).0;
        o.lines()
            .find(|l| l.contains("root:"))
            .and_then(|l| l.split_whitespace().last())
            .map(|r| r[r.len().saturating_sub(16)..].to_string())
            .unwrap_or_else(|| "-".into())
    }

    /// Commit the readers — without `g`, or with `g` pure to be pinned —
    /// then write `g` as an append, plainly or by a pin; status twice;
    /// commit. (rc of each step, bytes after the two statuses, after the
    /// commit.)
    fn rewrite(&self, readers: &str, pin: bool) -> (String, usize, usize) {
        self.file("f.n", &if pin { format!("{PURE}\n{readers}") } else { readers.to_string() });
        self.file("g.n", GA);
        let mut rcs = vec![self.run(&["evolve", "f.n"]), self.run(&["commit", "-m", "a"])];
        rcs.push(if pin { self.run(&["evolve", "--pin", "--grant", "pin", "g.n"]) } else { self.run(&["evolve", "g.n"]) });
        rcs.push(self.run(&["status"]));
        rcs.push(self.run(&["status"]));
        let after_status = self.bytes();
        rcs.push(if pin { self.run(&["commit", "-m", "b", "--grant", "pin"]) } else { self.run(&["commit", "-m", "b"]) });
        let rcs = rcs.iter().map(|r| r.to_string()).collect::<Vec<_>>().join(" ");
        (rcs, after_status, self.bytes())
    }
}

fn squash(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn check(bad: &mut Vec<String>, label: &str, got: String, want: &str) {
    if got != want {
        bad.push(format!("{label}\n      got:  {got}\n      want: {want}"));
    }
}

fn report(bad: Vec<String>, of: usize) {
    assert!(bad.is_empty(), "{} of {} wrong:\n{}", bad.len(), of, bad.join("\n"));
}

// ── r1 ── a commit's memory grows with what it writes ──

/// A chain of lists of length `n`, evolved, then committed: (rc, seconds, peak MB) of the commit.
fn list_chain_commit(n: usize) -> (i32, f64, u64) {
    let w = Ws::new(&format!("r1-{n}"));
    let mut src: String = (0..n).map(|i| format!("k{i}: [k{}]\n", i + 1)).collect();
    src.push_str(&format!("k{n}: 7"));
    w.file("f.n", &src);
    let e = w.oo(&["evolve", "f.n"], Duration::from_secs(120));
    if e.1 != 0 {
        return (e.1, e.2, e.3 / 1024);
    }
    let c = w.oo(&["commit", "-m", "a"], Duration::from_secs(120));
    (c.1, c.2, c.3 / 1024)
}

#[test]
fn r1_a_commit_holds_what_it_writes() {
    let (rc1, t1, m1) = list_chain_commit(250);
    let (rc4, t4, m4) = list_chain_commit(1000);
    let ok = rc1 == 0 && rc4 == 0 && m1 > 0 && (m4 as f64) < 1.6 * m1 as f64;
    let got = format!("rc {rc1} {t1:.2}s {m1} MB / rc {rc4} {t4:.2}s {m4} MB (x{:.2})", m4 as f64 / m1.max(1) as f64);
    report(if ok { Vec::new() } else { vec![format!("a chain of lists, N=250 and 1000: want rc 0 and the commit's peak under x1.6\n      got:  {got}")] }, 1);
}

// ── r2 ── what must be one value still is ──

#[test]
fn r2_a_record_held_below_the_top_is_one_record() {
    let mut bad = Vec::new();
    let mut n = 0;
    for (label, readers, _) in SHAPES {
        for pin in [false, true] {
            n += 1;
            let how = if pin { "pinned" } else { "written later" };
            let w = Ws::new(&format!("r2-{n}"));
            let (rcs, st, done) = w.rewrite(readers, pin);
            check(&mut bad, &format!("{label}, g {how}: rc, bytes after two statuses, after the commit"),
                format!("{rcs} | {st} | {done}"), "0 0 0 0 0 0 | 0 | 1");
        }
    }
    report(bad, n);
}

// ── g1 ── roots and readings, as v0.140.0 ──

fn g1_lines() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for (n, (label, readers, q)) in SHAPES.iter().enumerate() {
        for how in ["at once", "later", "pinned"] {
            let w = Ws::new(&format!("g1-{n}-{}", how.len()));
            let rcs = if how == "at once" {
                w.file("f.n", &format!("{GA}\n{readers}"));
                format!("{} {}", w.run(&["evolve", "f.n"]), w.run(&["commit", "-m", "a"]))
            } else {
                w.rewrite(readers, how == "pinned").0
            };
            out.push((format!("{label}, g {how}"), format!("{rcs} | {} | {q}={}", w.root_tail(), w.read(q))));
        }
    }
    let w = Ws::new("g1-chain");
    let mut src: String = (0..50).map(|i| format!("k{i}: [k{}]\n", i + 1)).collect();
    src.push_str("k50: 7");
    w.file("f.n", &src);
    let rcs = format!("{} {}", w.run(&["evolve", "f.n"]), w.run(&["commit", "-m", "a"]));
    out.push(("a chain of lists, N=50".into(), format!("{rcs} | {} | _.k45={}", w.root_tail(), w.read("_.k45"))));
    out
}

#[test]
fn g1_what_is_stored_and_read_does_not_change() {
    let got = g1_lines();
    if std::env::var_os("Q141_PRINT").is_some() {
        for (l, g) in &got {
            println!("G1\t{l}\t{g}");
        }
    }
    let mut bad = Vec::new();
    for (label, g) in &got {
        match WANT_G1.iter().find(|(l, _)| l == label) {
            Some((_, want)) => check(&mut bad, label, g.clone(), want),
            None => bad.push(format!("{label}: no expectation\n      got:  {g}")),
        }
    }
    report(bad, got.len());
}
