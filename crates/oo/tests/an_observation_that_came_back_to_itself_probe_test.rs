// An observation that came back to itself.
// Order: nlang-tools/docs/an_observation_that_came_back_to_itself_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-131 (no new ruling): a cycle with
// a hop that is not a pure reference diverges (SPEC_12 §1.1 #2, §2.2); a
// cell that observes an effect stores what it observed (D110, D46 ②); a
// record's lazy field is observed at commit (D129).
//
// ── What is wrong (v0.130.0) ────────────────────────────────────────────
//
//   A cell with an effect on a cycle reads itself while it is observed by
//   evaluating its recipe once more: `a: [E, a]` does E twice and stores
//   `[#true, [#true, ⊥]]` (v0.129.0 once, `[#true, _]`). Where the cut falls
//   depends on which cell was observed first, so the orders of one write
//   store up to four roots and do the effects two to four times. A record
//   that comes back to itself with an effect on it (`a: { e: E, x: b }`,
//   `b: { y: a }`) is taken by evolve, and every commit does the effect and
//   then refuses: `refusing to store a root this engine cannot read back`.
//
// The other half: what does not come back to an effect cell keeps its root.
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
const P: &str = "p: ~%Env.get \"Q131_UNSET\"";
const E: &str = "~%Io./append_file (\"a.log\", \"x\")";
const F: &str = "~%Io./append_file (\"a.log\", \"y\")";

struct Ws {
    _scratch: nlang_interpreter::ScratchDir,
    root: PathBuf,
    ws: PathBuf,
}

impl Ws {
    fn new(tag: &str) -> Self {
        let scratch = nlang_interpreter::ScratchDir::new(&format!("came-back-{tag}"));
        let root = scratch.path().to_path_buf();
        let ws = root.join("ws");
        fs::create_dir_all(&ws).unwrap();
        Ws { _scratch: scratch, root, ws }
    }

    /// Run `oo` capped at 2 GB, killed after `kill`: (output, rc, seconds).
    fn oo_for(&self, args: &[&str], kill: Duration) -> (String, i32, f64) {
        let mut child = Command::new("sh")
            .arg("-c")
            .arg("ulimit -v 2000000; exec \"$0\" \"$@\"")
            .arg(env!("CARGO_BIN_EXE_oo"))
            .args(args)
            .current_dir(&self.ws)
            .env("OO_IDENTITY", self.root.join("identity"))
            .env("OO_NODE_HOME", self.root.join("node-home"))
            .env_remove("Q131_UNSET")
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
            if start.elapsed() > kill {
                let _ = child.kill();
                let _ = child.wait();
                return ("DID NOT RETURN".into(), -1, start.elapsed().as_secs_f64());
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let secs = start.elapsed().as_secs_f64();
        let o = child.wait_with_output().expect("output");
        let text = format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
        let rc = o.status.code().unwrap_or(-1);
        if text.contains("panicked") {
            return ("PANIC".into(), rc, secs);
        }
        if text.contains("stack overflow") || text.contains("memory allocation") || (rc != 0 && text.trim().is_empty()) {
            return ("CRASHED".into(), rc, secs);
        }
        (text.trim().to_string(), rc, secs)
    }

    fn oo(&self, args: &[&str]) -> (String, i32) {
        let (o, rc, _) = self.oo_for(args, KILL_AFTER);
        (o, rc)
    }

    fn file(&self, src: &str) -> String {
        let n = fs::read_dir(&self.ws).map(|d| d.count()).unwrap_or(0);
        let f = format!("f{n}.n");
        fs::write(self.ws.join(&f), format!("{src}\n")).unwrap();
        f
    }

    /// Evolve `src` from a new file: the exit code, or the crash.
    fn evolve(&self, src: &str) -> String {
        let f = self.file(src);
        let (o, rc) = self.oo(&["evolve", &f]);
        if o == "DID NOT RETURN" || o == "CRASHED" || o == "PANIC" { o } else { format!("e{rc}") }
    }

    fn commit(&self) -> String {
        let (o, rc) = self.oo(&["commit", "-m", "a", "--grant", "pin", "--grant", "effect_override"]);
        if o == "DID NOT RETURN" || o == "CRASHED" || o == "PANIC" { o } else { format!("c{rc}") }
    }

    fn eval(&self, e: &str) -> String {
        let s = self.oo(&["eval", e]).0;
        squash(&s)
    }

    fn root(&self) -> String {
        let head = fs::read_to_string(self.ws.join(".oo").join("HEAD")).unwrap_or_default();
        if head.trim().is_empty() {
            return String::new();
        }
        let (o, _) = self.oo(&["inspect", head.trim()]);
        o.lines()
            .find(|l| l.contains("root:"))
            .and_then(|l| l.rsplit(':').next())
            .unwrap_or("")
            .trim()
            .to_string()
    }

    /// Bytes the effect has written so far.
    fn done(&self) -> u64 {
        fs::metadata(self.ws.join("a.log")).map(|m| m.len()).unwrap_or(0)
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

/// One write of `src` (its lines in the given order), committed: the
/// exit codes and the root.
fn written(src: &str) -> String {
    let w = Ws::new("written");
    let e = w.evolve(src);
    let c = w.commit();
    format!("{e} {c} {}", w.root())
}

fn reversed(src: &str) -> String {
    src.lines().rev().collect::<Vec<_>>().join("\n")
}

/// Every order of the lines of `src`.
fn orders(src: &str) -> Vec<String> {
    fn go(rest: Vec<&str>, at: Vec<&str>, out: &mut Vec<String>) {
        if rest.is_empty() {
            out.push(at.join("\n"));
            return;
        }
        for i in 0..rest.len() {
            let mut r = rest.clone();
            let x = r.remove(i);
            let mut a = at.clone();
            a.push(x);
            go(r, a, out);
        }
    }
    let mut out = Vec::new();
    go(src.lines().collect(), Vec::new(), &mut out);
    out
}

/// One write of `src`, committed, then `reads` read: the exit codes, the
/// root, the bytes the effects wrote after evolve / commit / the reads, and
/// what was read.
fn wrote_all(src: &str, reads: &[&str]) -> (String, String, Vec<String>) {
    let w = Ws::new("wrote");
    let e = w.evolve(src);
    let after_e = w.done();
    let c = w.commit();
    let after_c = w.done();
    let vs: Vec<String> = reads.iter().map(|q| w.eval(q)).collect();
    let after_r = w.done();
    (format!("{e} {c} {}", w.root()), format!("{after_e}/{after_c}/{after_r}"), vs)
}

/// The lists that come back to a cell with an effect of its own, and the
/// bytes its effects write (one per effect written).
const LISTS: [(&str, &str); 7] = [
    ("a: [%E%, a]", "1/1/1"),
    ("a: [%E%, b]\nb: [a]", "1/1/1"),
    ("a: [%E%, b]\nb: [%F%, a]", "2/2/2"),
    ("a: [%E%, b]\nb: [c]\nc: [a]", "1/1/1"),
    ("a: [%E%, b]\nb: [a]\nm: [b]", "1/1/1"),
    ("a: [%E%, _.b]\nb: [_.a]", "1/1/1"),
    ("a: [%E%, b.0]\nb: [a]", "1/1/1"),
];

fn spelled(src: &str) -> String {
    src.replace("%E%", E).replace("%F%", F)
}

/// r1 — An effect on a cycle is done once: written in any order, each effect
/// written writes once at evolve, and not again at commit or when read. (A
/// cell observed reads itself as the cycle, SPEC_12 §2.2; v0.130.0 evaluated
/// its own recipe once more.)
#[test]
fn r1_an_effect_on_a_cycle_is_done_once() {
    let mut bad = Vec::new();
    let mut n = 0;
    for (shape, want) in LISTS {
        for src in orders(&spelled(shape)) {
            let (_, bytes, _) = wrote_all(&src, &["_.a", "_.b", "_.m"]);
            check(&mut bad, &format!("{src:?}: bytes after evolve/commit/reads"), bytes, want);
            n += 1;
        }
    }
    verdict(bad, n);
}

/// r2 — One write, one root, on a cycle through an effect cell: every order
/// of the lines stores the same root (the cut is where the cycle comes back
/// to the cell observed, not where the order happened to start).
#[test]
fn r2_one_root_in_every_order() {
    let mut bad = Vec::new();
    let mut n = 0;
    let shapes: Vec<String> = LISTS
        .iter()
        .map(|(s, _)| spelled(s))
        .chain(
            [
                "a: [~%Env.get \"Q131_UNSET\", b]\nb: [a]",
                "a: [~%Env.get b]\nb: [a]",
                "a: [~%Env.get b]\nb: [~%Env.get a]",
            ]
            .map(String::from),
        )
        .collect();
    for shape in shapes {
        let os = orders(&shape);
        let first = wrote_all(&os[0], &[]).0;
        for src in &os[1..] {
            check(&mut bad, &format!("{src:?} against {:?}", os[0]), wrote_all(src, &[]).0, &first);
            n += 1;
        }
    }
    verdict(bad, n);
}

/// r3 — Where the cycle comes back, the answer holds what the pure cycle
/// holds there: `a: [E, a]` reads `[#true, ⊥ #divergent]` as `a: [a]` reads
/// `[⊥ #divergent]`; `a: [E, b]`, `b: [a]` reads `[#true, [⊥ #divergent]]`
/// as `a: [b]`, `b: [a]` reads `[[⊥ #divergent]]`; `b` keeps the answer of
/// `a` it read. The `_.` spelling stores one root in every order and reads
/// what the bare spelling reads. (Q-133 pre-revision, D131: `b` reads `a`
/// and stores its recipe, which keeps its spelling — the root is no longer
/// the bare spelling's.)
#[test]
fn r3_the_cycle_point_is_what_the_pure_cycle_holds() {
    let mut bad = Vec::new();
    let mut n = 0;
    let io = "#true ;; %effect: #io";
    let cut = "_|_ ;; %cause: #divergent";
    for src in orders(&spelled("a: [%E%, a]")) {
        let (_, _, v) = wrote_all(&src, &["_.a"]);
        n += 1;
        check(&mut bad, "a: [E, a]: _.a", v.join(" | "), &format!("[{io}, {cut}]"));
    }
    let (_, _, v) = wrote_all("a: [a]", &["_.a"]);
    check(&mut bad, "a: [a] (pure): _.a", v.join(" | "), &format!("[{cut}]"));
    for src in orders(&spelled("a: [%E%, b]\nb: [a]")) {
        let (_, _, v) = wrote_all(&src, &["_.a", "_.b"]);
        n += 1;
        check(&mut bad, &format!("{src:?}: _.a | _.b"), v.join(" | "), &format!("[{io}, [{cut}]] | [[{io}, [{cut}]]]"));
    }
    let (_, _, v) = wrote_all("a: [b]\nb: [a]", &["_.a"]);
    check(&mut bad, "a: [b], b: [a] (pure): _.a", v.join(" | "), &format!("[[{cut}]]"));
    let bare = wrote_all(&spelled("a: [%E%, b]\nb: [a]"), &["_.a", "_.b"]).2.join(" | ");
    let rooted = orders(&spelled("a: [%E%, _.b]\nb: [_.a]"));
    let first = wrote_all(&rooted[0], &[]).0;
    for src in &rooted {
        n += 2;
        let (root, _, v) = wrote_all(src, &["_.a", "_.b"]);
        check(&mut bad, &format!("{src:?}: one root in every order"), root, &first);
        check(&mut bad, &format!("{src:?}: reads what the bare spelling reads"), v.join(" | "), &bare);
    }
    verdict(bad, n + 2);
}

/// r4 — A record that comes back to itself with an effect on it is committed:
/// in every order the evolve the write was taken by is followed by a commit
/// that lands, the effect is done once, every order stores one root, and the
/// way round the cycle reads the answer the effect gave. (v0.130.0 refused
/// every such commit after doing the effect: `refusing to store a root this
/// engine cannot read back`.)
#[test]
fn r4_a_record_that_comes_back_is_committed() {
    let mut bad = Vec::new();
    let mut n = 0;
    for (shape, round, direct, want) in [
        ("a: { e: %E%, x: b }\nb: { y: a }", "_.a.x.y.e", "_.a.e", "0/1/1"),
        ("a: { e: %E%, x: a }", "_.a.x.e", "_.a.e", "0/1/1"),
        ("a: { x: b }\nb: { y: a, e: %E% }", "_.a.x.e", "_.b.e", "0/1/1"),
        ("a: { e: %E%, x: b }\nb: [a]", "_.b.0.e", "_.a.e", "0/1/1"),
        ("a: { e: %E%, x: [b] }\nb: [a]", "_.a.x.0.0.e", "_.a.e", "0/1/1"),
        ("a: { x: b }\nb: { y: a }\nc: [a, %E%]", "_.c.1", "_.c.1", "1/1/1"),
    ] {
        let os = orders(&spelled(shape));
        let mut first: Option<String> = None;
        for src in &os {
            let (root, bytes, v) = wrote_all(src, &[round, direct]);
            let codes: String = root.split(' ').take(2).collect::<Vec<_>>().join(" ");
            check(&mut bad, &format!("{src:?}: evolve, commit"), codes, "e0 c0");
            check(&mut bad, &format!("{src:?}: bytes after evolve/commit/reads"), bytes, want);
            check(&mut bad, &format!("{src:?}: {round} reads {direct}"), format!("{}", v.len() == 2 && v[0] == v[1] && !v[0].is_empty() && v[0] != "_"), "true");
            n += 3;
            match &first {
                None => first = Some(root),
                Some(f) => {
                    check(&mut bad, &format!("{src:?}: the root of {:?}", os[0]), root, f);
                    n += 1;
                }
            }
        }
    }
    verdict(bad, n);
}

/// g1 — What does not come back to an effect cell keeps its root: pure
/// cycles (lists, records, arithmetic, an alias cycle, a list of itself), a
/// cycle beside an IO name it does not observe, a cycle broken by a path, an
/// IO record outside the cycle, a cycle whose name was pinned pure — both
/// orders, the roots v0.130.0 stores.
#[test]
fn g1_what_does_not_come_back_keeps_its_root() {
    let mut bad = Vec::new();
    let mut n = 0;
    for (shape, want) in [
        ("a: [b]\nb: [a]", "75359a00cb04c651ffc237d8a38fbea41e76c021c368b564d5629e95e6468b78"),
        ("a: b + 1\nb: a + 1", "9d10e2b35aef49a12b7b413a186b63987db644fba1ce3846319849d6d0c0edf8"),
        ("a: { x: b }\nb: { y: a }", "628ca75d9dd3e671ab984df798f6eafa258cbda5f39040d17073280a674c5d20"),
        ("a: [b, 1]\nb: [a, 2]", "041689feed8a897be3ddaf2cc061bca309ee2fdefb1231bec56f3b3234b5a9b1"),
        ("a: [a]\nm: [a]", "a4990a40edda0ff3967fea5a28e68dddff6c8d16bb2ce3e2a1d741058a29575e"),
        ("a: b\nb: a", "9e5fdfb1d7250293edf11ad0ae7c61198d460fed18c0bbb411cc21103fe217ee"),
        ("a: [b, p]\nb: [a]\np: @P@", "072f5bdc25a7cd9c22f2eccc88ffd7c7eb61407dbdacf0fbae27d81ee11cc4c5"),
        ("a: { x: b.y, q: p }\nb: { y: 1, z: a }\np: @P@", "70447fb35d3006467861276807687dc6dcf7fef749d6f6cf7514b5098b793b42"),
        ("a: { x: b }\nb: { y: a }\nm: { e: p }\np: @P@", "67f4dd3dbe2fafb9e179c104411f08c1caed5de8e930d2e2b34397791201deae"),
        ("a: { x: b, q: p }\nb: { y: a }\np: 7", "b2a9697915ee3cd0de09e37dcabdc9b92e1bf6e0392eff84a1a8e033500949b3"),
    ] {
        let shape = shape.replace("p: @P@", P);
        for src in [shape.clone(), reversed(&shape)] {
            check(&mut bad, &format!("{src:?}"), written(&src), &format!("e0 c0 {want}"));
            n += 1;
        }
    }
    verdict(bad, n);
}
