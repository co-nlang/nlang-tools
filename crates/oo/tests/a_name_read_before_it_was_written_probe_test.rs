// A name read before it was written.
// Order: nlang-tools/docs/a_name_read_before_it_was_written_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-128. No new ruling: the fields of
// one write are a set (SYNTAX_03 §2 #1), and writing a coordinate costs that
// coordinate (Q-116, Q-117). A coordinate that reads a name the write makes
// later, and the coordinate that makes it, cost what they are, as they do
// when the name comes first.
//
// ── What is wrong (v0.126.0) ────────────────────────────────────────────
//
//   `k_i: p + 1` for i < N and `p: 1` written last: N = 500 / 1000 / 2000
//   evolve in 4.2 / 12.4 / 45.6 s; with `p` first, 0.57 / 1.06 / 2.11 s.
//   Q-117 adds a coordinate in place only when every name it reads is
//   already there and nothing staged reads it; a forward reading, and the
//   name it waits for, take the whole meet and restamp the working set.
//
// The other half: the readers of a name that arrives later read the label
// it gives them, as when it comes first; roots at v0.126.0 do not move.
//
// Every `oo` here runs under a 2 GB address-space cap and a kill inside
// the probe (60 s, 30 s for a scale step); timed cells run one at a time.
//
// The delivery may NOT edit this file. `rustfmt` must not touch it.

#![allow(dead_code)]

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const KILL_AFTER: Duration = Duration::from_secs(60);

struct Ws {
    _scratch: nlang_interpreter::ScratchDir,
    root: PathBuf,
    ws: PathBuf,
}

impl Ws {
    fn new(tag: &str) -> Self {
        let scratch = nlang_interpreter::ScratchDir::new(&format!("read-first-{tag}"));
        let root = scratch.path().to_path_buf();
        let ws = root.join("ws");
        fs::create_dir_all(&ws).unwrap();
        Ws { _scratch: scratch, root, ws }
    }

    /// Run `oo` capped at 2 GB, killed after 60 s: (output, rc). A run
    /// that is killed or dies without a word says so.
    fn oo(&self, args: &[&str]) -> (String, i32) {
        let mut child = Command::new("sh")
            .arg("-c")
            .arg("ulimit -v 2000000; exec \"$0\" \"$@\"")
            .arg(env!("CARGO_BIN_EXE_oo"))
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
            if start.elapsed() > KILL_AFTER {
                let _ = child.kill();
                let _ = child.wait();
                return ("DID NOT RETURN".into(), -1);
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let o = child.wait_with_output().expect("output");
        let text = format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
        let rc = o.status.code().unwrap_or(-1);
        if text.contains("panicked") {
            return ("PANIC".into(), rc);
        }
        if text.contains("stack overflow") || text.contains("memory allocation") || (rc != 0 && text.trim().is_empty()) {
            return ("CRASHED".into(), rc);
        }
        (text.trim().to_string(), rc)
    }

    /// Evolve `src` from a new file: the exit code, or the crash.
    fn evolve(&self, src: &str) -> String {
        let n = fs::read_dir(&self.ws).map(|d| d.count()).unwrap_or(0);
        let f = format!("f{n}.n");
        fs::write(self.ws.join(&f), src).unwrap();
        let (o, rc) = self.oo(&["evolve", &f]);
        if o == "DID NOT RETURN" || o == "CRASHED" || o == "PANIC" { o } else { format!("e{rc}") }
    }

    /// Evolve each source as its own file, then commit.
    fn writes(&self, srcs: &[&str]) -> String {
        let mut out: Vec<String> = srcs.iter().map(|s| self.evolve(s)).collect();
        let (o, rc) = self.oo(&["commit", "-m", "a"]);
        out.push(if o == "DID NOT RETURN" || o == "CRASHED" || o == "PANIC" { o } else { format!("c{rc}") });
        out.join(" ")
    }

    /// `oo eval`, the trailing effect note dropped, whitespace collapsed.
    fn eval(&self, e: &str) -> String {
        let s = self.oo(&["eval", e]).0;
        let s = match s.find(";; %effect") {
            Some(i) => s[..i].trim().to_string(),
            None => s,
        };
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

/// A ⊥ or a horizon reduced to its cause.
fn short(got: &str) -> String {
    for cause in ["#conflict", "#missing_key", "#no_matching_branch", "#fuel_exhausted", "#max_depth_exceeded"] {
        if (got.starts_with("_|_") || got.starts_with("#blur")) && got.contains(cause) {
            return format!("{} {cause}", got.split_whitespace().next().unwrap_or(""));
        }
    }
    got.into()
}

/// `oo eval` from an empty directory, the effect note dropped, whitespace
/// collapsed, not cut short.
fn eval_full(e: &str) -> String {
    Ws::new("eval").eval(e)
}

fn eval_all(srcs: &[String]) -> Vec<String> {
    let mut out = vec![String::new(); srcs.len()];
    let per = srcs.len().div_ceil(8).max(1);
    std::thread::scope(|s| {
        for (o, i) in out.chunks_mut(per).zip(srcs.chunks(per)) {
            s.spawn(move || {
                for (o, e) in o.iter_mut().zip(i) {
                    *o = eval_full(e);
                }
            });
        }
    });
    out
}

fn table(bad: &mut Vec<String>, cells: &[(&str, &str)]) {
    let srcs: Vec<String> = cells.iter().map(|c| c.0.to_string()).collect();
    for ((e, want), got) in cells.iter().zip(eval_all(&srcs)) {
        check(bad, e, short(&got), want);
    }
}

fn three_ways(w1: &str, w2: &str, q: &str) -> (Vec<String>, Vec<String>) {
    let mut got = Vec::new();
    let mut roots = Vec::new();
    for ways in [vec![vec![format!("{w1}\n{w2}")]], vec![vec![w1.to_string(), w2.to_string()]], vec![vec![w1.to_string()], vec![w2.to_string()]]] {
        let w = Ws::new("three");
        let mut rc = Vec::new();
        for commit in &ways {
            let srcs: Vec<&str> = commit.iter().map(|s| s.as_str()).collect();
            rc.push(w.writes(&srcs));
        }
        got.push(format!("{} | {}", rc.join(" "), short(&w.eval(q))));
        roots.push(w.root());
    }
    (got, roots)
}

fn root_of(src: &str) -> String {
    let w = Ws::new("root");
    let got = w.writes(&[src]);
    if got != "e0 c0" {
        return format!("not committed: {got}");
    }
    w.root()
}


/// One `oo` capped at 2 GB, killed after `kill`: (rc, seconds). Killed or
/// crashed runs answer rc -1.
fn timed(w: &Ws, args: &[&str], kill: Duration) -> (i32, f64) {
    let mut child = Command::new("sh")
        .arg("-c")
        .arg("ulimit -v 2000000; exec \"$0\" \"$@\"")
        .arg(env!("CARGO_BIN_EXE_oo"))
        .args(args)
        .current_dir(&w.ws)
        .env("OO_IDENTITY", w.root.join("identity"))
        .env("OO_NODE_HOME", w.root.join("node-home"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("oo runs");
    let start = Instant::now();
    loop {
        if let Some(st) = child.try_wait().expect("wait") {
            return (st.code().unwrap_or(-1), start.elapsed().as_secs_f64());
        }
        if start.elapsed() > kill {
            let _ = child.kill();
            let _ = child.wait();
            return (-1, start.elapsed().as_secs_f64());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn list_of(n: usize, item: impl Fn(usize) -> String) -> String {
    (0..n).map(item).collect::<Vec<_>>().join(", ")
}

/// Timed cells run one at a time: tests in this file run in parallel, and
/// a step weighed beside another is not weighed alone.
static WEIGH: std::sync::Mutex<()> = std::sync::Mutex::new(());


fn lines(n: usize, line: impl Fn(usize) -> String) -> String {
    (0..n).map(line).collect::<Vec<_>>().join("\n") + "\n"
}

/// Write `files` (one evolve over all of them, after `before` was committed
/// in the same workspace), then commit: "e<rc> c<rc>" and the seconds of
/// the evolve and the commit over `files`.
fn write(before: Option<&str>, files: &[String], kill: Duration) -> (String, f64) {
    let w = Ws::new("write");
    if let Some(b) = before {
        fs::write(w.ws.join("before.n"), b).unwrap();
        let (r1, _) = timed(&w, &["evolve", "before.n"], Duration::from_secs(120));
        let (r2, _) = timed(&w, &["commit", "-m", "before"], Duration::from_secs(120));
        if r1 != 0 || r2 != 0 {
            return (format!("before e{r1} c{r2}"), 0.0);
        }
    }
    let mut args = vec!["evolve".to_string()];
    for (i, f) in files.iter().enumerate() {
        let name = format!("f{i}.n");
        fs::write(w.ws.join(&name), f).unwrap();
        args.push(name);
    }
    let a: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let (re, te) = timed(&w, &a, kill);
    if re != 0 {
        return (format!("e{re}"), te);
    }
    let (rc, tc) = timed(&w, &["commit", "-m", "a"], kill);
    (format!("e{re} c{rc}"), te + tc)
}

/// One shape at N and at 4N coordinates: both land, the larger within the
/// kill, and four times the coordinates costs less than eight times the
/// time (a linear step is about four, a quadratic one sixteen).
fn scale(bad: &mut Vec<String>, label: &str, files: &dyn Fn(usize) -> Vec<String>, n: usize) {
    let _alone = WEIGH.lock().unwrap_or_else(|e| e.into_inner());
    let kill = Duration::from_secs(30);
    let (a, t1) = write(None, &files(n), kill);
    let (b, t4) = write(None, &files(4 * n), kill);
    let ok = a == "e0 c0" && b == "e0 c0" && t4 < 8.0 * t1 + 0.5;
    let got = format!("{a} {t1:.2}s / {b} {t4:.2}s (x{:.1})", t4 / t1.max(0.001));
    check(bad, &format!("{label}, N={n} and {}", 4 * n), if ok { "linear".into() } else { got }, "linear");
}

/// Evolve each group of files as one call, then commit; "e<rc>… c<rc>" and
/// the seconds of all of it.
fn write_calls(calls: &[Vec<String>], kill: Duration) -> (String, f64) {
    let w = Ws::new("calls");
    let mut rcs = Vec::new();
    let mut t = 0.0;
    for (ci, files) in calls.iter().enumerate() {
        let mut args = vec!["evolve".to_string()];
        for (i, f) in files.iter().enumerate() {
            let name = format!("c{ci}_{i}.n");
            fs::write(w.ws.join(&name), f).unwrap();
            args.push(name);
        }
        let a: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let (re, te) = timed(&w, &a, kill);
        t += te;
        rcs.push(format!("e{re}"));
        if re != 0 {
            return (rcs.join(" "), t);
        }
    }
    let (rc, tc) = timed(&w, &["commit", "-m", "a"], kill);
    rcs.push(format!("c{rc}"));
    (rcs.join(" "), t + tc)
}

fn scale_calls(bad: &mut Vec<String>, label: &str, calls: &dyn Fn(usize) -> Vec<Vec<String>>, n: usize) {
    let _alone = WEIGH.lock().unwrap_or_else(|e| e.into_inner());
    let kill = Duration::from_secs(30);
    let (a, t1) = write_calls(&calls(n), kill);
    let (b, t4) = write_calls(&calls(4 * n), kill);
    let want: String = (0..calls(1).len()).map(|_| "e0 ").collect::<String>() + "c0";
    let ok = a == want && b == want && t4 < 8.0 * t1 + 0.5;
    let got = format!("{a} {t1:.2}s / {b} {t4:.2}s (x{:.1})", t4 / t1.max(0.001));
    check(bad, &format!("{label}, N={n} and {}", 4 * n), if ok { "linear".into() } else { got }, "linear");
}

/// One shape at N and 4N with its own kill.
fn scale_at(bad: &mut Vec<String>, label: &str, files: &dyn Fn(usize) -> Vec<String>, n: usize, kill: Duration) {
    let _alone = WEIGH.lock().unwrap_or_else(|e| e.into_inner());
    let (a, t1) = write(None, &files(n), kill);
    let (b, t4) = write(None, &files(4 * n), kill);
    let ok = a == "e0 c0" && b == "e0 c0" && t4 < 8.0 * t1 + 0.5;
    let got = format!("{a} {t1:.2}s / {b} {t4:.2}s (x{:.1})", t4 / t1.max(0.001));
    check(bad, &format!("{label}, N={n} and {}", 4 * n), if ok { "linear".into() } else { got }, "linear");
}

/// r1 — A coordinate that reads a name written after it, and the name,
/// cost what they are: four times the coordinates in less than eight times
/// the time — one name read by all, written last; records, lists and local
/// coordinates that read it; each coordinate reading its own name written
/// later; the readers in one file and the name in another; the readers in
/// one evolve and the name in the next; coordinates that observe it; each
/// coordinate its own name again at 2000 and 8000.
#[test]
fn r1_a_name_written_after_its_readers() {
    let mut bad = Vec::new();
    let shapes: Vec<(&str, Box<dyn Fn(usize) -> Vec<String>>)> = vec![
        ("one name read by all, written last", Box::new(|n| vec![lines(n, |i| format!("k{i}: p + {i}")) + "p: 1\n"])),
        ("records that read it", Box::new(|n| vec![lines(n, |i| format!("k{i}: {{ a: p.v + {i}, b: {i} }}")) + "p: { v: 1 }\n"])),
        ("lists that read it", Box::new(|n| vec![lines(n, |i| format!("k{i}: [p, {i}]")) + "p: 1\n"])),
        ("local coordinates that read it", Box::new(|n| vec![lines(n, |i| format!("~k{i}: ~p + {i}")) + "~p: 1\n"])),
        ("each its own name, written later", Box::new(|n| vec![lines(n, |i| format!("k{i}: j{i} + 1")) + &lines(n, |i| format!("j{i}: {i}"))])),
        ("the readers in one file, the name in another", Box::new(|n| vec![lines(n, |i| format!("k{i}: p + {i}")), format!("p: 1\n{}", lines(n, |i| format!("j{i}: {i}")))])),
        ("coordinates that observe it", Box::new(|n| vec![lines(n, |i| format!("k{i}: ~%Env.get p")) + "p: \"Q128_UNSET\"\n"])),
    ];
    let k = shapes.len() + 1;
    for (label, files) in &shapes {
        scale(&mut bad, label, files.as_ref(), 500);
    }
    let calls = |n: usize| vec![vec![lines(n, |i| format!("k{i}: p + {i}"))], vec!["p: 1\n".to_string()]];
    scale_calls(&mut bad, "the readers in one evolve, the name in the next", &calls, 500);
    // The same at 2000 and 8000: a step that copies the working set once
    // per name shows only past a few thousand.
    let own = |n: usize| vec![lines(n, |i| format!("k{i}: j{i} + 1")) + &lines(n, |i| format!("j{i}: {i}"))];
    scale_at(&mut bad, "each its own name, written later", &own, 2000, Duration::from_secs(60));
    verdict(bad, k + 1);
}

/// Evolve and commit each group of files in turn: rcs, root, readings.
fn reading(groups: &[&[&str]], qs: &[&str]) -> (String, String, Vec<String>) {
    let w = Ws::new("read");
    let mut rcs = Vec::new();
    for (gi, g) in groups.iter().enumerate() {
        let mut args = vec!["evolve".to_string()];
        for (i, src) in g.iter().enumerate() {
            let name = format!("f{gi}_{i}.n");
            fs::write(w.ws.join(&name), format!("{src}\n")).unwrap();
            args.push(name);
        }
        let a: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        rcs.push(format!("e{}", w.oo(&a).1));
        rcs.push(format!("c{}", w.oo(&["commit", "-m", "a"]).1));
    }
    (rcs.join(" "), w.root(), qs.iter().map(|q| short(&w.eval(q))).collect())
}

/// What `status` prints, the root after a commit, and readings.
fn staged_and_root(calls: &[&str], qs: &[&str]) -> (String, String, Vec<String>) {
    let w = Ws::new("label");
    let mut rcs = Vec::new();
    for c in calls {
        rcs.push(w.evolve(&format!("{c}\n")));
    }
    let st = squash(&w.oo(&["status"]).0);
    let (_, rc) = w.oo(&["commit", "-m", "a"]);
    rcs.push(format!("c{rc}"));
    (format!("{} | {st}", rcs.join(" ")), w.root(), qs.iter().map(|q| short(&w.eval(q))).collect())
}

/// r2 — The readers of a name that arrives later read the label it brings,
/// as when it comes first: an observed coordinate read by a list, a record
/// and a local coordinate, written before them, after them, and in an
/// evolve after theirs; a reader of a reader written between them (it
/// reads the label the first reader got when the name arrived); a type, a
/// rule and a meta field that read it, each read by a coordinate. What
/// is staged in each order is
/// what v0.126.0
/// stages (the readers keep their recipe and carry `#io`); the root and the
/// readings are the same in every order.
#[test]
fn r2_a_late_name_gives_its_readers_its_label() {
    let mut bad = Vec::new();
    let readers = "a: [p, 1]\nb: { q: p }\nc: [p, \"x\"]\n~l: [p]";
    let p = "p: ~%Env.get \"Q128_UNSET\"";
    let qs = ["(_.a).%effect", "(_.b.q).%effect", "(_.c).%effect"];
    let staged = |got: &str| {
        let s = got.split(" | ").nth(1).unwrap_or("");
        let from = s.find("Staged changes:").unwrap_or(0);
        let to = s.find("Total Logical").unwrap_or(s.len());
        s[from..to].trim().to_string()
    };
    let (first, root_first, v_first) = staged_and_root(&[&format!("{p}\n{readers}")], &qs);
    check(&mut bad, "the name first: readings", v_first.join(" | "), "#io | #io | #io");
    check(&mut bad, "the name first: what is staged", staged(&first),
        "Staged changes: { a: [#none ;; %effect: #io, 1] b: { q: p ;; %effect: #io } ;; %effect: #io c: [#none ;; %effect: #io, \"x\"] p: #none ;; %effect: #io ~l: [#none ;; %effect: #io] } ;; %effect: #io");
    for (label, calls) in [
        ("the readers first", vec![format!("{readers}\n{p}")]),
        ("the readers in an evolve before", vec![readers.to_string(), p.to_string()]),
    ] {
        let c: Vec<&str> = calls.iter().map(|s| s.as_str()).collect();
        let (got, root, v) = staged_and_root(&c, &qs);
        check(&mut bad, &format!("{label}: what is staged"), staged(&got),
            "Staged changes: { a: [p, 1] ;; %effect: #io b: { q: p ;; %effect: #io } c: [p, \"x\"] ;; %effect: #io p: #none ;; %effect: #io ~l: [p] ;; %effect: #io } ;; %effect: #io");
        check(&mut bad, &format!("{label}: root"), root, &root_first);
        check(&mut bad, &format!("{label}: readings"), v.join(" | "), &v_first.join(" | "));
    }
    // A reader of a reader, written between them in the same evolve: it
    // reads the label the first reader got when the name arrived.
    let (got, root, v) = staged_and_root(&[&format!("a: [p]\nm: [a]\n{p}")], &["(_.m).%effect"]);
    check(&mut bad, "a reader of a reader: what is staged", staged(&got),
        "Staged changes: { a: [p] ;; %effect: #io m: [a] ;; %effect: #io p: #none ;; %effect: #io } ;; %effect: #io");
    check(&mut bad, "a reader of a reader: root", root, "13419f971787792aa5c81eb51b2cccdd1fa472bb4756cc6f46c974d93c63991c");
    check(&mut bad, "a reader of a reader: reading", v.join(" | "), "#io");
    // Readers that are not coordinates — a type, a rule, a meta field —
    // and a coordinate that reads them, written between them and the name.
    for (src, want_staged, want_root) in [
        ("@T: [p]\nm: [@T]", "Staged changes: { @T: [p] ;; %effect: #io m: [@T] ;; %effect: #io p: #none ;; %effect: #io } ;; %effect: #io", "a542fcf3aa3879b240ce1a2731a647bc0d34ab2601d340b1513bb24857fef232"),
        ("@T: { x: p }\nm: [@T]", "Staged changes: { @T: { x: p ;; %effect: #io } m: [@T] p: #none ;; %effect: #io } ;; %effect: #io", "1fcb801b569e433555b6b191767460a2e0ed71bc9a1a702967d7b286b7f407de"),
        ("/r: [p]\nm: [/r]", "Staged changes: { /r: [p] ;; %effect: #io m: [/r] ;; %effect: #io p: #none ;; %effect: #io } ;; %effect: #io", "6be88ea1e0163cad9d1bc32aae9cf6363f68cf16a6cae4622d8e0925c1ea2f5c"),
        ("%m: [p]\nz: [%m]", "Staged changes: { %m: [p] ;; %effect: #io p: #none ;; %effect: #io z: [%m] ;; %effect: #io } ;; %effect: #io", "e40c2dbca1be781ec77cb3faf37cc5af798c51ed5a70c71266414b953cae3248"),
    ] {
        let (got, root, _) = staged_and_root(&[&format!("{src}\n{p}")], &[]);
        check(&mut bad, &format!("{src:?}, the name after: what is staged"), staged(&got), want_staged);
        check(&mut bad, &format!("{src:?}, the name after: root"), root, want_root);
    }
    // A pure name arriving later leaves its readers pure.
    let (got, _, v) = staged_and_root(&["a: [p, 1]\nb: { q: p }\np: 7"], &["(_.a).%effect", "_.b.q"]);
    check(&mut bad, "a pure name, written last: what is staged", staged(&got), "Staged changes: { a: [p, 1] b: { q: p } p: 7 }");
    check(&mut bad, "a pure name, written last: readings", v.join(" | "), "#pure | 7");
    verdict(bad, 21);
}

/// g1 — What was written is what was written: roots and readings at
/// v0.126.0 do not move — a name read before it, by a record, a list, a
/// local coordinate and a path; a chain of ten written forward; the name in
/// the next evolve; over a committed root; the name rewritten in a later
/// evolve (refused); a cycle; a function written after its caller; the name
/// written twice (refused); the name written as `_`; what the session
/// stores when a `_` is written before a forward reading.
#[test]
fn g1_what_was_written_stays_written() {
    let chain = (0..10).map(|i| format!("k{i}: k{} + 1", i + 1)).collect::<Vec<_>>().join("\n") + "\nk10: 0";
    let cases: Vec<(&str, Vec<&str>, Vec<&str>, &str)> = vec![
        ("forward one", vec!["k: p + 1\np: 1"], vec!["_.k"], "e0 c0 | 52526f246bf4b7655d714b34ce1b5748a4e1f222acb93aca733db9762c3f023f | 2"),
        ("fan", vec!["a: [p, 1]\nb: { q: p.v }\n~l: p.v\nc: ~l + 1\np: { v: 2 }"], vec!["_.a", "_.b.q", "_.c"], "e0 c0 | 84253e8ccd8ee60c64d14990c7e3727784d99e8e6d561774ccef05dbb680fbf7 | [{ v: 2 }, 1] || 2 || 3"),
        ("chain of ten", vec![chain.as_str()], vec!["_.k0"], "e0 c0 | bf4b5960d07a07e32bf28c320632e37ac44685a3bc017685065eb1f148916465 | 10"),
        ("two calls", vec!["k: p + 1", "p: 1"], vec!["_.k"], "e0 e0 c0 | 52526f246bf4b7655d714b34ce1b5748a4e1f222acb93aca733db9762c3f023f | 2"),
        ("rewritten in a call", vec!["k: p + 1\np: 1", "p: 2"], vec!["_.k", "_.p"], "e0 e1 c0 | 52526f246bf4b7655d714b34ce1b5748a4e1f222acb93aca733db9762c3f023f | 2 || 1"),
        ("a cycle", vec!["a: b + 1\nb: c\nc: a"], vec!["_.a"], "e0 c0 | c110efdc5fa9a85faf55bbc8db5a28316f498ea6aaa57f711d6117f5c63db97f | _|_ ;; %cause: #divergent"),
        ("a path before it", vec!["k: q.a.b\nq: { a: { b: 5 } }"], vec!["_.k"], "e0 c0 | a9c82c712c699112548558e852e3fcc365c7ed7117c3ab1ccf9755a7e3110546 | 5"),
        ("a local before it", vec!["k: ~p * 2\n~p: 5"], vec!["_.k"], "e0 c0 | 07ac3985d3da3ed8d2e7623e3dcfb255638d7a61413734c40fe2effb1364f950 | 10"),
        ("a function after", vec!["k: f 2\nf: (x -> x + 1)"], vec!["_.k"], "e0 c0 | d75bba9d2517778e7072805e81bedac305c532ccdd6ba623ad16f7a16517c649 | 3"),
        ("the name twice", vec!["k: p + 1\np: 1\np: 2"], vec!["_.k"], "e1 c1 |  | _"),
        ("the name as _", vec!["k: p\np: _\nz: 1"], vec!["_.k"], "e0 c0 | bd094a9a0f89eef71e3cc1011aa66030b1b92998e9cb265d72a9dcab08a17394 | _"),
    ];
    let mut bad = Vec::new();
    for (name, calls, qs, want) in &cases {
        let (rc, root, vals) = staged_and_root(calls, qs);
        let rc = rc.split(" | ").next().unwrap_or("").to_string();
        check(&mut bad, name, format!("{rc} | {root} | {}", vals.join(" || ")), want);
    }
    // Over a committed root.
    let (rc, root, vals) = reading(&[&["p: 1"], &["k: p + 1\nq: r\nr: 3"]], &["_.k", "_.q"]);
    check(&mut bad, "over a root", format!("{rc} | {root} | {}", vals.join(" || ")),
        "e0 c0 e0 c0 | 383bda8684c126cda01c8245a83ecd8097b1cb54d88cc3fde5e619ac4a490992 | 2 || 3");
    // A `_` written before a forward reading: what the session stores
    // (the injection's body; its id is new each time).
    let w = Ws::new("stored");
    let _ = w.evolve("t: _\nout: t.%cause\nu: 1\n");
    let dir = w.ws.join(".oo").join("injections");
    let body: Vec<String> = fs::read_dir(&dir)
        .map(|d| {
            d.filter_map(|e| e.ok())
                .map(|e| fs::read_to_string(e.path()).unwrap_or_default())
                .map(|t| squash(&t.lines().filter(|l| !l.starts_with("id:")).collect::<Vec<_>>().join("\n")))
                .collect()
        })
        .unwrap_or_default();
    check(&mut bad, "a `_` before a forward reading: what the session stores", body.join(" || "),
        "#nlang/store injection pin_coords: [] absorbs: {} effect_tags: 0 { out: { ~%__nlang_thunk: #true ~%__nlang_expr: t.%cause ~%__nlang_closure: [] } u: 1 }");
    verdict(bad, cases.len() + 2);
}

/// Evolve `src` in a new workspace with every in-place landing checked
/// against the meet and its restamp (`OO_Q117_SHADOW`): the exit code or
/// `PANIC`, and how many landings were checked.
fn shadowed(src: &str) -> (String, usize) {
    let w = Ws::new("shadow");
    let hits = w.root.join("hits");
    fs::write(&hits, "").unwrap();
    fs::write(w.ws.join("f.n"), format!("{src}\n")).unwrap();
    let o = Command::new("sh")
        .arg("-c")
        .arg("ulimit -v 2000000; exec timeout -s KILL 60 \"$0\" \"$@\"")
        .arg(env!("CARGO_BIN_EXE_oo"))
        .args(["evolve", "f.n"])
        .current_dir(&w.ws)
        .env("OO_IDENTITY", w.root.join("identity"))
        .env("OO_NODE_HOME", w.root.join("node-home"))
        .env("OO_Q117_SHADOW", &hits)
        .stdin(Stdio::null())
        .output()
        .expect("oo runs");
    let text = format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
    let n = fs::read_to_string(&hits).unwrap_or_default().lines().count();
    let rc = o.status.code().unwrap_or(-1);
    (if text.contains("Q-117 shadow") { "PANIC".into() } else { format!("e{rc}") }, n)
}

/// r3 — A coordinate that reads the name through `_.` is one of its
/// readers (M2): `_.p` names the root's `p` whatever a record or an arrow
/// around the path binds. The reader of a reader in the same evolve, the
/// reader first or the name first, gives one root — the root the bare
/// spelling gives (`[p]`, `[p | 1]`, `[{ y: p }]`, `@T: [p]`), or, where a
/// record of the reader binds `p` itself, the root of the name first.
/// Every in-place landing is the meet's, `_.` read in a list, a union, a
/// record, a record that binds `p` itself, a sum, a path into the name,
/// and on the type axis.
#[test]
fn r3_a_reader_through_the_root_is_a_reader() {
    let mut bad = Vec::new();
    let p = "p: ~%Env.get \"Q128_UNSET\"";
    let cells = [
        ("a: [_.p]\nm: [a]", "13419f971787792aa5c81eb51b2cccdd1fa472bb4756cc6f46c974d93c63991c"),
        ("a: [_.p | 1]\nm: [a]", "1b3454a3c10f9064b573689ecccf5d1d2ec5b6a5ea66bc929adc31260d93a992"),
        ("a: [{ y: _.p }]\nm: [a]", "84fd03746a5dce2de57c1f7227cebafe1b083e9448acc0c5ef7fbf008aa11c07"),
        ("@T: [_.p]\nm: [@T]", "a542fcf3aa3879b240ce1a2731a647bc0d34ab2601d340b1513bb24857fef232"),
        ("a: [{ p: 2, y: _.p }]\nm: [a]", "bddee15f6f09db4213ad05f71803e549105b2417fde6a336a3202473208f8200"),
    ];
    for (src, want) in cells {
        for (label, text) in [("the reader first", format!("{src}\n{p}")), ("the name first", format!("{p}\n{src}"))] {
            let (_, root, _) = staged_and_root(&[&text], &[]);
            check(&mut bad, &format!("{src:?}, {label}: root"), root, want);
        }
    }
    let shadow = [
        "a: [_.p]\nm: [a]",
        "a: [_.p | 1]\nm: [a]",
        "a: [{ y: _.p }]\nm: [a]",
        "@T: [_.p]\nm: [@T]",
        "a: [{ p: 2, y: _.p }]\nm: [a]",
        "a: { x: _.p }\nm: [a]",
        "a: { p: 2, x: _.p }\nm: [a]",
        "a: { p: 2, x: [_.p] }\nm: [a]",
        "a: [_.p + \"\"]\nm: [a]",
        "a: [_.p.v]\nm: [a]",
    ];
    for src in shadow {
        let (got, n) = shadowed(&format!("{src}\n{p}"));
        check(&mut bad, &format!("{src:?}, the reader first: every landing is the meet's"), got, "e0");
        check(&mut bad, &format!("{src:?}, the reader first: landings checked"), (n > 0).to_string(), "true");
    }
    verdict(bad, cells.len() * 2 + shadow.len() * 2);
}
