// A reader that kept an answer it could read again.
// Order: nlang-tools/docs/a_reader_that_kept_an_answer_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-133. Ruling D131 (乙,
// 2026-11-21): a cell whose effect comes only from the coordinates of the
// root it reads stores its recipe; a cell whose own expression does an
// effect stores its answer. That is D46 read as written: the cells it reads
// store their answers, so reading it again is reproducible (D46 ①); only a
// cell that does the effect itself is not (D46 ②).
//
// ── What is wrong (v0.132.0) ────────────────────────────────────────────
//
//   Since v0.128.0 (Q-129) a reader of an effect cell carries its label, and
//   the commit stores an answer for every labelled cell. An IO chain
//   `k_i: [k_{i+1}]`, `k_N: ~%Env.get p` stores, for every link, an answer
//   cut at the depth horizon (128 levels, `#blur` inside); the commit holds
//   about ten copies of each level. N=1000 commits in 2.9 GB, N=2000 in
//   6 GB (v0.127.0: 96 MB, 116 MB), and under a 3 GB cap the commit aborts.
//
// ── What is right ────────────────────────────────────────────────────────
//
//   A reader that reads coordinates of the root, and does no effect of its
//   own, keeps its recipe: `a: [p]` is stored as `[p]`. It reads what it
//   read before (p's answer), and the effect is done once.
//
// The other half: a cell that does an effect itself (a call to an effect
// morphism, an application of a function with an effect), or reads a
// sibling in its own record (the record's frame holds that sibling's recipe,
// not its answer), stores its answer, as before.
//
// Every `oo` here runs under a 2 GB address-space cap and a kill inside
// the probe (60 s).
//
// The delivery may NOT edit this file. `rustfmt` must not touch it.

#![allow(dead_code)]

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const KILL_AFTER: Duration = Duration::from_secs(60);
const E: &str = "~%Io./append_file (\"a.log\", \"x\")";
const P: &str = "p: ~%Io./append_file (\"a.log\", \"x\")";

struct Ws {
    _scratch: nlang_interpreter::ScratchDir,
    root: PathBuf,
    ws: PathBuf,
}

impl Ws {
    fn new(tag: &str) -> Self {
        let scratch = nlang_interpreter::ScratchDir::new(&format!("kept-answer-{tag}"));
        let root = scratch.path().to_path_buf();
        let ws = root.join("ws");
        fs::create_dir_all(&ws).unwrap();
        Ws { _scratch: scratch, root, ws }
    }

    /// Run `oo` capped at 2 GB, killed after 60 s: (output, rc). The output
    /// goes to a file, not a pipe: a printed root can pass 64 KB, and a pipe
    /// nobody reads until the end would stop the process.
    fn oo(&self, args: &[&str]) -> (String, i32) {
        let out_path = self.root.join("out.txt");
        let out = fs::File::create(&out_path).expect("output file");
        let err = out.try_clone().expect("output file");
        let mut cmd = Command::new("sh");
        cmd.arg("-c")
            .arg("ulimit -v 2000000; exec \"$0\" \"$@\"")
            .arg(env!("CARGO_BIN_EXE_oo"))
            .args(args)
            .current_dir(&self.ws)
            .env("OO_IDENTITY", self.root.join("identity"))
            .env("OO_NODE_HOME", self.root.join("node-home"))
            .env("Q133_A", "alpha")
            .env_remove("OO_Q117_SHADOW")
            .stdin(Stdio::null())
            .stdout(Stdio::from(out))
            .stderr(Stdio::from(err));
        let mut child = cmd.spawn().expect("oo runs");
        let start = Instant::now();
        loop {
            if child.try_wait().expect("wait").is_some() {
                break;
            }
            if start.elapsed() > KILL_AFTER {
                let _ = child.kill();
                let _ = child.wait();
                return ("DID NOT RETURN".into(), -1);
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let status = child.wait().expect("wait");
        let text = String::from_utf8_lossy(&fs::read(&out_path).unwrap_or_default()).to_string();
        let rc = status.code().unwrap_or(-1);
        if text.contains("panicked") {
            return ("PANIC".into(), rc);
        }
        if text.contains("stack overflow") || text.contains("memory allocation") || (rc != 0 && text.trim().is_empty()) {
            return ("CRASHED".into(), rc);
        }
        (text.trim().to_string(), rc)
    }

    fn code(o: String, rc: i32, letter: &str) -> String {
        if o == "DID NOT RETURN" || o == "CRASHED" || o == "PANIC" { o } else { format!("{letter}{rc}") }
    }

    /// One evolve of `src`, then a commit: "e<rc> c<rc>", or the crash.
    fn write(&self, src: &str) -> String {
        fs::write(self.ws.join("f.n"), format!("{src}\n")).unwrap();
        let (o, rc) = self.oo(&["evolve", "f.n"]);
        let e = Self::code(o, rc, "e");
        let (o, rc) = self.oo(&["commit", "-m", "a"]);
        format!("{e} {}", Self::code(o, rc, "c"))
    }

    fn eval(&self, e: &str) -> String {
        squash(&self.oo(&["eval", "--", e]).0)
    }

    /// The committed root's object, printed, whitespace squashed.
    fn body(&self) -> String {
        let head = fs::read_to_string(self.ws.join(".oo").join("HEAD")).unwrap_or_default();
        let (o, _) = self.oo(&["inspect", head.trim()]);
        let root = o.lines().find(|l| l.contains("root:")).and_then(|l| l.split_whitespace().last()).unwrap_or("").to_string();
        let (b, _) = self.oo(&["inspect", &root]);
        squash(&b)
    }

    /// Bytes the effect `E` appended so far.
    fn bytes(&self) -> usize {
        fs::read(self.ws.join("a.log")).map(|b| b.len()).unwrap_or(0)
    }
}

fn squash(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn check(bad: &mut Vec<String>, label: &str, got: String, want: &str) {
    if got != want {
        bad.push(format!("  {label}\n    want {want}\n    got  {}", got.chars().take(200).collect::<String>()));
    }
}

fn verdict(bad: Vec<String>, n: usize) {
    assert!(bad.is_empty(), "{} of {} wrong:\n{}", bad.len(), n, bad.join("\n"));
}

/// The stored root holds `want` (whitespace squashed).
fn holds(bad: &mut Vec<String>, label: &str, body: &str, want: &str) {
    if !body.contains(want) {
        bad.push(format!("  {label}\n    want the root to hold {want}\n    got  {}", body.chars().take(300).collect::<String>()));
    }
}

// ── r1 ── an IO chain commits in its size ────────────────────────────────

/// r1 — An IO chain of 1000 links, as a list chain and as a record chain,
/// commits inside the 2 GB cap: every link but the last only reads the next,
/// so it keeps its recipe. The link next to the effect reads the answer.
/// (v0.132.0: the commit holds a horizon-deep answer for every link and
/// aborts on the cap; v0.127.0 committed both in about 100 MB.)
#[test]
fn r1_an_io_chain_commits_in_its_size() {
    let mut bad = Vec::new();
    let n = 1000;
    for (shape, link, near, want) in [
        ("list", "k{i}: [k{j}]", "_.k999", "[\"alpha\" ;; %effect: #io]"),
        ("record", "k{i}: {{ x: k{j} }}", "_.k999.x", "\"alpha\" ;; %effect: #io"),
    ] {
        let src = (0..n)
            .map(|i| link.replace("{i}", &i.to_string()).replace("{j}", &(i + 1).to_string()).replace("{{", "{").replace("}}", "}"))
            .collect::<Vec<_>>()
            .join("\n")
            + &format!("\nk{n}: ~%Env.get \"Q133_A\"");
        let w = Ws::new(&format!("chain-{shape}"));
        check(&mut bad, &format!("{shape} chain of {n}: evolve, commit"), w.write(&src), "e0 c0");
        check(&mut bad, &format!("{shape} chain of {n}: {near}"), w.eval(near), want);
        holds(&mut bad, &format!("{shape} chain of {n}: k0 keeps its recipe"), &w.body(),
            &if shape == "list" { "k0: [k1]".to_string() } else { "k0: { x: k1".to_string() });
    }
    verdict(bad, 6);
}

// ── r2 ── a reader of a coordinate keeps its recipe ──────────────────────

/// Readers of an effect coordinate `p` that do no effect themselves, and
/// what the stored root holds for them.
const READERS: [(&str, &[&str]); 9] = [
    ("a: [p]", &["a: [p] ;; %effect: #cached"]),
    ("a: { x: p }", &["x: p ;; %effect: #cached"]),
    ("a: p", &["a: p ;; %effect: #cached"]),
    ("a: [_.p]", &["a: [_.p] ;; %effect: #cached"]),
    ("a: [b]\nb: [p]", &["a: [b] ;; %effect: #cached", "b: [p] ;; %effect: #cached"]),
    ("~a: [p]\nm: [~a]", &["~a: [p] ;; %effect: #cached", "m: [~a] ;; %effect: #cached"]),
    ("@T: [p]\nm: [@T]", &["@T: [p] ;; %effect: #cached", "m: [@T] ;; %effect: #cached"]),
    ("a: [q.v]\nq: { v: p }", &["a: [q.v] ;; %effect: #cached", "v: p ;; %effect: #cached"]),
    ("a: [p, 1]", &["a: [p, 1] ;; %effect: #cached"]),
];

/// r2 — A reader of an effect coordinate that does no effect itself is
/// stored as its recipe, in either order with `p`; the effect is done once,
/// at commit, and reading the reader twice does it no more and reads what
/// `p` stored. (v0.132.0: the reader stores the answer it read.)
#[test]
fn r2_a_reader_of_a_coordinate_keeps_its_recipe() {
    let mut bad = Vec::new();
    let mut n = 0;
    for (readers, stored) in READERS {
        for (order, src) in [("the readers first", format!("{readers}\n{P}")), ("p first", format!("{P}\n{readers}"))] {
            let w = Ws::new("reader");
            check(&mut bad, &format!("{readers:?}, {order}: evolve, commit"), w.write(&src), "e0 c0");
            let body = w.body();
            for want in stored {
                holds(&mut bad, &format!("{readers:?}, {order}: stored"), &body, want);
                n += 1;
            }
            check(&mut bad, &format!("{readers:?}, {order}: bytes after commit"), w.bytes().to_string(), "1");
            let first = w.eval("_");
            let again = w.eval("_");
            check(&mut bad, &format!("{readers:?}, {order}: the root read twice"), again, &first);
            check(&mut bad, &format!("{readers:?}, {order}: bytes after reads"), w.bytes().to_string(), "1");
            n += 4;
        }
    }
    verdict(bad, n);
}

// ── g1 ── a cell that does the effect itself keeps its answer ────────────

/// g1 — A cell whose own expression does an effect stores its answer: a call
/// to an effect morphism, an application of a function that does one, a
/// builtin that applies such a function, an arrow applied in place beside a
/// read of an effect coordinate; and a field that reads its sibling in its
/// own record (the frame's copy of the sibling is its recipe). Each effect is
/// done once, and reading the root twice does it no more. (v0.132.0 the same.)
#[test]
fn g1_a_cell_that_does_the_effect_keeps_its_answer() {
    let mut bad = Vec::new();
    let h = format!("h: (x -> {E})");
    let cells: [(String, &str, &str); 5] = [
        (format!("a: [{E}]"), "a: [#true ;; %effect: #cached]", "1"),
        (format!("{h}\na: h 1"), "a: #true ;; %effect: #cached", "1"),
        (format!("{h}\na: ~%List./map (h, [1])"), "a: [#true ;; %effect: #cached]", "1"),
        (format!("a: [(x -> {E}) 1, p]\n{P}"), "a: [#true ;; %effect: #cached, #true ;; %effect: #cached]", "2"),
        (format!("g: {{ s: {E}, t: s }}"), "t: #true ;; %effect: #cached", "1"),
    ];
    for (src, stored, bytes) in &cells {
        let w = Ws::new("own");
        check(&mut bad, &format!("{src:?}: evolve, commit"), w.write(src), "e0 c0");
        holds(&mut bad, &format!("{src:?}: stored"), &w.body(), stored);
        check(&mut bad, &format!("{src:?}: bytes after commit"), w.bytes().to_string(), bytes);
        let _ = w.eval("_");
        let _ = w.eval("_");
        check(&mut bad, &format!("{src:?}: bytes after two reads"), w.bytes().to_string(), bytes);
    }
    verdict(bad, cells.len() * 4);
}
