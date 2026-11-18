// A horizon that paid for the whole working set.
// Order: nlang-tools/docs/a_horizon_that_paid_for_the_working_set_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-130. No new ruling: a write
// costs what its size costs (Q-116, Q-117, Q-128 — one invariant); a
// horizon is a snapshot (D127), so what it reads does not change when a
// coordinate arrives.
//
// ── What is wrong (v0.128.0) ────────────────────────────────────────────
//
//   `/f` recursing past the depth, `h: /f 5000`, and N fields `k_i: h + i`
//   in one evolve: N=500 / 1000 / 2000 evolve 9.53 / 34.60 / 142.02 s — the
//   square of the write. The same fields over `h: /f 50` (a recipe, not a
//   horizon): 5.99 / 11.36 / 22.35 s. Each coordinate that lands as a
//   horizon goes the meet's way: Q-117's in-place landing takes atoms,
//   recipes, records and unions of them, not a horizon, so each one meets
//   the whole working set and restamps all of it. A chain past the depth
//   written name first lands every link as a horizon (D130): N=800 21.36 s,
//   written reader first 11.70 s.
//
// The other half: every landing is the meet's (shadowed), and the roots
// and readings stay what v0.128.0 stores.
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
        let scratch = nlang_interpreter::ScratchDir::new(&format!("one-reader-deep-{tag}"));
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



/// Recursion past the depth of an evaluation: `h` lands as a horizon
/// (`#max_depth_exceeded`), and so does everything that reads it.
const DEEP: &str = "/f: (x -> x == 0 ? 0 : /f (x - 1))\nh: /f 5000\n";
/// A horizon of fuel (`#fuel_exhausted`), read by nothing in the universe.
const FUEL: &str = "~%List./sum (~%List./range (0, 1000000))";

/// r1 — A write whose coordinates land as horizons costs what it costs to
/// write them, however many: four times the coordinates, less than eight
/// times the seconds. Readers of a horizon, the horizon itself, local
/// coordinates reading one, a chain past the depth written name first, and two
/// evolves. (A horizon of fuel is in r2 and g1: each costs its own fuel.)
#[test]
fn r1_a_horizon_lands_at_the_cost_of_its_write() {
    let mut bad = Vec::new();
    scale(&mut bad, "each reads a horizon", &|n| vec![format!("{DEEP}{}", lines(n, |i| format!("k{i}: h + {i}")))], 500);
    scale(&mut bad, "each is the horizon", &|n| vec![format!("{DEEP}{}", lines(n, |i| format!("k{i}: h")))], 500);
    scale(&mut bad, "each a local reading a horizon", &|n| vec![format!("{DEEP}{}m: [~k0]\n", lines(n, |i| format!("~k{i}: h + {i}")))], 500);
    scale(&mut bad, "a chain past the depth, the name first", &|n| {
        vec![format!("k{n}: 0\n{}", lines(n, |i| { let j = n - 1 - i; format!("k{j}: k{} + 1", j + 1) }))]
    }, 500);
    scale_calls(&mut bad, "the horizon in one evolve, its readers in the next", &|n| {
        vec![vec![DEEP.to_string()], vec![lines(n, |i| format!("k{i}: h + {i}"))]]
    }, 500);
    verdict(bad, 5);
}

/// One evolve over `pre` committed, then `src`, every in-place landing
/// checked against the meet (`OO_Q117_SHADOW`): the exit code or `PANIC`,
/// and how many were checked.
fn shadowed_over(pre: &str, src: &str) -> (String, usize) {
    let w = Ws::new("shadow-over");
    let _ = w.writes(&[pre]);
    let hits = w.root.join("hits");
    fs::write(&hits, "").unwrap();
    fs::write(w.ws.join("g.n"), format!("{src}\n")).unwrap();
    let o = Command::new("sh")
        .arg("-c")
        .arg("ulimit -v 2000000; exec timeout -s KILL 60 \"$0\" \"$@\"")
        .arg(env!("CARGO_BIN_EXE_oo"))
        .args(["evolve", "g.n"])
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

/// r2 — Every landing is the meet's: each shape below, with every in-place
/// landing checked against the meet and its restamp; at least one is
/// checked, and none differs.
#[test]
fn r2_every_landing_of_a_horizon_is_the_meets() {
    let mut bad = Vec::new();
    let cells: [&str; 12] = [
        "/f: (x -> x == 0 ? 0 : /f (x - 1))\nh: /f 5000\nk: h + 1",
        "/f: (x -> x == 0 ? 0 : /f (x - 1))\nh: /f 5000\nk: { v: h }\nm: [k]",
        "/f: (x -> x == 0 ? 0 : /f (x - 1))\nh: /f 5000\n~k: h + 1\nm: [~k]",
        "/f: (x -> x == 0 ? 0 : /f (x - 1))\nk: h + 1\nh: /f 5000",
        "/f: (x -> x == 0 ? 0 : /f (x - 1))\nt: _\nh: /f 5000\nk: h + 1",
        "/f: (x -> x == 0 ? 0 : /f (x - 1))\nh: /f 5000\nk: [h, h]\nu: (k, h)",
        "g: ~%List./sum (~%List./range (0, 1000000))\nk: g + 1",
        "/f: (x -> x == 0 ? 0 : /f (x - 1))\nh: /f 5000\nk: h | 1",
        "/f: (x -> x == 0 ? 0 : /f (x - 1))\nh: /f 5000\nk: h & 1",
        // Beside a horizon, values that are not horizons and that the meet
        // does not leave as they are: a ⊥ inside a union, `_`, a reference.
        "/f: (x -> x == 0 ? 0 : /f (x - 1))\nh: /f 5000\nk: ({ a: 1, b: 2 } & { a: 2 }) | { a: 2, b: 2 }",
        "/f: (x -> x == 0 ? 0 : /f (x - 1))\nh: /f 5000\nt: _\nk: t.%cause",
        "/f: (x -> x == 0 ? 0 : /f (x - 1))\nh: /f 5000\nr: <<h>>\nk: [r]",
    ];
    for c in cells {
        let (rc, n) = shadowed(c);
        check(&mut bad, &format!("{c:?}: every landing is the meet's"), rc, "e0");
        check(&mut bad, &format!("{c:?}: landings checked"), (n > 0).to_string(), "true");
    }
    // Over a committed root: a horizon where HEAD holds a value (D127: both
    // are kept), and a value where HEAD holds a horizon.
    for (pre, src) in [
        ("/f: (x -> x == 0 ? 0 : /f (x - 1))\nr: 7", "r: /f 5000\nk: 1"),
        ("/f: (x -> x == 0 ? 0 : /f (x - 1))\nr: /f 5000", "r: 7\nk: 1"),
        ("/f: (x -> x == 0 ? 0 : /f (x - 1))\nh: /f 5000", "k: h + 1\nm: [k]"),
    ] {
        let (rc, n) = shadowed_over(pre, src);
        check(&mut bad, &format!("over {pre:?}, {src:?}: every landing is the meet's"), rc, "e0");
        check(&mut bad, &format!("over {pre:?}, {src:?}: landings checked"), (n > 0).to_string(), "true");
    }
    verdict(bad, 30);
}

/// g1 — What was right stays: the roots and readings of the shapes above,
/// at three coordinates, as v0.128.0 stores them, in the order v0.128.0
/// stores one root for (D130: past a horizon the order may matter).
#[test]
fn g1_what_was_right_stays() {
    let mut bad = Vec::new();
    let deep = DEEP;
    let fuel = format!("k0: {FUEL} + 0\nk1: {FUEL} + 1\nk2: {FUEL} + 2");
    let chain = "k130: 0\nk129: k130 + 1\nk128: k129 + 1\nk127: k128 + 1\nk126: k127 + 1\nk125: k126 + 1\nk124: k125 + 1\nk123: k124 + 1\nk122: k123 + 1\nk121: k122 + 1\nk120: k121 + 1\nk119: k120 + 1\nk118: k119 + 1\nk117: k118 + 1\nk116: k117 + 1\nk115: k116 + 1\nk114: k115 + 1\nk113: k114 + 1\nk112: k113 + 1\nk111: k112 + 1\nk110: k111 + 1\nk109: k110 + 1\nk108: k109 + 1\nk107: k108 + 1\nk106: k107 + 1\nk105: k106 + 1\nk104: k105 + 1\nk103: k104 + 1\nk102: k103 + 1\nk101: k102 + 1\nk100: k101 + 1\nk99: k100 + 1\nk98: k99 + 1\nk97: k98 + 1\nk96: k97 + 1\nk95: k96 + 1\nk94: k95 + 1\nk93: k94 + 1\nk92: k93 + 1\nk91: k92 + 1\nk90: k91 + 1\nk89: k90 + 1\nk88: k89 + 1\nk87: k88 + 1\nk86: k87 + 1\nk85: k86 + 1\nk84: k85 + 1\nk83: k84 + 1\nk82: k83 + 1\nk81: k82 + 1\nk80: k81 + 1\nk79: k80 + 1\nk78: k79 + 1\nk77: k78 + 1\nk76: k77 + 1\nk75: k76 + 1\nk74: k75 + 1\nk73: k74 + 1\nk72: k73 + 1\nk71: k72 + 1\nk70: k71 + 1\nk69: k70 + 1\nk68: k69 + 1\nk67: k68 + 1\nk66: k67 + 1\nk65: k66 + 1\nk64: k65 + 1\nk63: k64 + 1\nk62: k63 + 1\nk61: k62 + 1\nk60: k61 + 1\nk59: k60 + 1\nk58: k59 + 1\nk57: k58 + 1\nk56: k57 + 1\nk55: k56 + 1\nk54: k55 + 1\nk53: k54 + 1\nk52: k53 + 1\nk51: k52 + 1\nk50: k51 + 1\nk49: k50 + 1\nk48: k49 + 1\nk47: k48 + 1\nk46: k47 + 1\nk45: k46 + 1\nk44: k45 + 1\nk43: k44 + 1\nk42: k43 + 1\nk41: k42 + 1\nk40: k41 + 1\nk39: k40 + 1\nk38: k39 + 1\nk37: k38 + 1\nk36: k37 + 1\nk35: k36 + 1\nk34: k35 + 1\nk33: k34 + 1\nk32: k33 + 1\nk31: k32 + 1\nk30: k31 + 1\nk29: k30 + 1\nk28: k29 + 1\nk27: k28 + 1\nk26: k27 + 1\nk25: k26 + 1\nk24: k25 + 1\nk23: k24 + 1\nk22: k23 + 1\nk21: k22 + 1\nk20: k21 + 1\nk19: k20 + 1\nk18: k19 + 1\nk17: k18 + 1\nk16: k17 + 1\nk15: k16 + 1\nk14: k15 + 1\nk13: k14 + 1\nk12: k13 + 1\nk11: k12 + 1\nk10: k11 + 1\nk9: k10 + 1\nk8: k9 + 1\nk7: k8 + 1\nk6: k7 + 1\nk5: k6 + 1\nk4: k5 + 1\nk3: k4 + 1\nk2: k3 + 1\nk1: k2 + 1\nk0: k1 + 1";
    let cells: Vec<(Vec<String>, Vec<&str>, &str)> = vec![
        (vec![format!("{deep}k0: h + 0\nk1: h + 1\nk2: h + 2")], vec!["_.k2"], "a4da0d72f53c582c2352280b118e60561bea6e2b8e923d2681fd881832f9276c #blur #max_depth_exceeded"),
        // Q-127 pre-revision (2026-11-18): v0.128.0 stored `415f3955…` readers
        // first (D130 lets it depend on the order); a write that lands in the
        // order its reads settle stores the name-first root in both.
        (vec![format!("k0: h + 0\nk1: h + 1\nk2: h + 2\n{deep}")], vec!["_.k2"], "a4da0d72f53c582c2352280b118e60561bea6e2b8e923d2681fd881832f9276c #blur #max_depth_exceeded"),
        (vec![format!("{deep}k0: {{ v: h, i: 0 }}\nk1: {{ v: h, i: 1 }}")], vec!["_.k1.i", "_.k1.v"], "92cba6dcf2bef080bf1055031445ef0d0bc608f6e9af5cd1be371ff08c7425d9 1 | #blur #max_depth_exceeded"),
        (vec![format!("{deep}~k0: h + 0\nm: [~k0]")], vec!["_.m.0"], "28e1a1319ab3f524bf4ebf0b46c3ce01474fab8561562da481ee215fcec14f13 #blur #max_depth_exceeded"),
        (vec![chain.to_string()], vec!["_.k0", "_.k129"], "e55bac15beecddece5c5e5f38728c0cb7de8f7e9311062028e25de4857506968 #blur #max_depth_exceeded | 1"),
        (vec![fuel], vec!["_.k1"], "1f94b45d1d4090f590214c401645509fa00c8cece0c307384cb5fed370855009 #blur #fuel_exhausted"),
        (vec![deep.to_string(), "k0: h + 0\nk1: h + 1".to_string()], vec!["_.k1"], "7fc08ed74a0ca3f20b0d8162478e2d59259596a783d28f9ff940e3c9b9210f68 #blur #max_depth_exceeded"),
        (vec![format!("{deep}k: [h, h]\nu: (k, h)\nj: h | 1")], vec!["_.u.1"], "71f5c97d313341f82ff26572bc96403a12b6cb81fd9d2e989cd9fd7075974c4f #blur #max_depth_exceeded"),
    ];
    let mut n = 0;
    for (calls, qs, want) in &cells {
        let c: Vec<&str> = calls.iter().map(|s| s.as_str()).collect();
        let (_, r, v) = staged_and_root(&c, qs);
        check(&mut bad, &format!("{calls:?}"), format!("{r} {}", v.join(" | ")), want);
        n += 1;
    }
    // Over a committed root (D127): a horizon where HEAD holds a value, a
    // value where HEAD holds a horizon, a reader of a horizon HEAD holds.
    for (pre, src, qs, want) in [
        ("/f: (x -> x == 0 ? 0 : /f (x - 1))\nr: 7", "r: /f 5000\nk: 1", ["_.r", "_.k"], "e0 c0 e0 c0 d54c5ebfdb3e2ca984c127ef98368ed482dcc19d38082c548a7c211246c0d999 #blur #max_depth_exceeded | 1"),
        ("/f: (x -> x == 0 ? 0 : /f (x - 1))\nr: /f 5000", "r: 7\nk: 1", ["_.r", "_.k"], "e0 c0 e0 c0 d54c5ebfdb3e2ca984c127ef98368ed482dcc19d38082c548a7c211246c0d999 #blur #max_depth_exceeded | 1"),
        ("/f: (x -> x == 0 ? 0 : /f (x - 1))\nh: /f 5000", "k: h + 1\nm: [k]", ["_.k", "_.m.0"], "e0 c0 e0 c0 ca87f139c4c0558f5c13cdfd9d314bee6cf433818e0530cef3dd9adedad1b2a8 #blur #max_depth_exceeded | #blur #max_depth_exceeded"),
    ] {
        let w = Ws::new("over");
        let a = w.writes(&[pre]);
        let b = w.writes(&[src]);
        let v: Vec<String> = qs.iter().map(|q| short(&w.eval(q))).collect();
        check(&mut bad, &format!("over {pre:?}: {src:?}"), format!("{a} {b} {} {}", w.root(), v.join(" | ")), want);
        n += 1;
    }
    verdict(bad, n);
}
