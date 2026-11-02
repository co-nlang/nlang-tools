// A list slow to keep.
// Order: nlang-tools/docs/a_list_slow_to_keep_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-116.  Keeping a value costs
// what the value is long: a commit of four times the elements takes less
// than eight times as long, and a hundred thousand are kept — never a
// commit that ends the process or does not return.
//
// ── What is wrong (v0.116.0) ────────────────────────────────────────────
//
//   Committing a list of 2500 / 5000 / 10000 / 20000 elements takes
//   0.27 / 0.74 / 3.39 / 18.05 s, while its evolve takes 0.10 to 0.64 s;
//   a hundred thousand from JSON do not commit in 90 s (and end the
//   process under 1 GB). `5 |> [$, …]` of 20000 terms evolves in 17.7 s.
//   The commit's pass that re-keys bodies, and the binding of open terms,
//   remove and re-insert every key of a map in turn: each removal shifts
//   the rest, N^2 for N keys.
//
// Every `oo` here runs under a 2 GB address-space cap and a kill inside
// the probe (60 s, 30 s for a scale step); timings are compared within
// one cell, N against 4N. Memory per element is large but linear (an
// integer in a list costs about 6 KB to evolve) and is not this order.
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
        let scratch = nlang_interpreter::ScratchDir::new(&format!("slow-keep-{tag}"));
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

/// Write `src` in a new workspace: evolve and commit, each timed.
/// "e<rc> c<rc>" and the commit's seconds (and the evolve's).
fn keep(src: &str, kill: Duration) -> (String, f64, f64) {
    let w = Ws::new("keep");
    fs::write(w.ws.join("f.n"), src).unwrap();
    let (re, te) = timed(&w, &["evolve", "f.n"], kill);
    if re != 0 {
        return (format!("e{re}"), te, 0.0);
    }
    let (rc, tc) = timed(&w, &["commit", "-m", "a"], kill);
    (format!("e{re} c{rc}"), te, tc)
}

fn list_of(n: usize, item: impl Fn(usize) -> String) -> String {
    (0..n).map(item).collect::<Vec<_>>().join(", ")
}

/// One shape at N and at 4N: both land, the larger within the kill, and
/// four times the elements costs less than eight times the time (a
/// linear step is about four, a quadratic one sixteen). `phase` picks
/// which step is weighed: the commit, or the evolve and commit together.
/// Timed cells run one at a time: tests in this file run in parallel, and
/// a step weighed beside another is not weighed alone.
static WEIGH: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn scale(bad: &mut Vec<String>, label: &str, src: &dyn Fn(usize) -> String, n: usize, both: bool) {
    let _alone = WEIGH.lock().unwrap_or_else(|e| e.into_inner());
    let kill = Duration::from_secs(30);
    let (a, ae, ac) = keep(&src(n), kill);
    let (b, be, bc) = keep(&src(4 * n), kill);
    let (t1, t4) = if both { (ae + ac, be + bc) } else { (ac, bc) };
    let ok = a == "e0 c0" && b == "e0 c0" && t4 < 8.0 * t1 + 0.5;
    let got = format!("{a} {t1:.2}s / {b} {t4:.2}s (x{:.1})", t4 / t1.max(0.001));
    check(bad, &format!("{label}, N={n} and {}", 4 * n), if ok { "linear".into() } else { got }, "linear");
}

/// r1 — Keeping a list costs what the list is long. A commit of four times
/// the elements takes less than eight times as long, for a list of one
/// value, of distinct integers, of strings, of lists, a list in a field,
/// a list read from JSON.
#[test]
fn r1_a_list_is_kept_in_its_length() {
    let mut bad = Vec::new();
    let shapes: Vec<(&str, Box<dyn Fn(usize) -> String>, usize)> = vec![
        ("list of 1", Box::new(|n| format!("r: [{}]", list_of(n, |_| "1".into()))), 10000),
        ("list of i", Box::new(|n| format!("r: [{}]", list_of(n, |i| i.to_string()))), 10000),
        ("list of strings", Box::new(|n| format!("r: [{}]", list_of(n, |i| format!("\"s{i}\"")))), 10000),
        ("list of lists", Box::new(|n| format!("r: [{}]", list_of(n, |_| "[1]".into()))), 2500),
        ("a list in a field", Box::new(|n| format!("r: {{ xs: [{}], name: \"n\" }}", list_of(n, |i| i.to_string()))), 10000),
        ("a list read from JSON", Box::new(|n| format!("r: ~%Json./parse (\"\"\"[{}]\"\"\")", list_of(n, |_| "1".into()))), 10000),
    ];
    let k = shapes.len();
    for (label, src, n) in &shapes {
        scale(&mut bad, label, src.as_ref(), *n, false);
    }
    verdict(bad, k);
}

/// r2 — A value with open terms binds them in its length: `5 |> [$, …]`
/// evolved and committed, four times the terms in less than eight times
/// the time.
#[test]
fn r2_open_terms_are_bound_in_their_length() {
    let mut bad = Vec::new();
    let src = |n: usize| format!("r: 5 |> [{}]", list_of(n, |_| "$".into()));
    scale(&mut bad, "5 |> [$, …]", &src, 5000, true);
    let src = |n: usize| format!("r: 5 |> [{}]", list_of(n, |i| format!("$ + {i}")));
    scale(&mut bad, "5 |> [$ + i, …]", &src, 5000, true);
    verdict(bad, 2);
}

/// r3 — A hundred thousand elements are kept and read back: no commit ends
/// the process or runs past its kill, through a coordinate, three ways.
#[test]
fn r3_a_hundred_thousand_are_kept() {
    let mut bad = Vec::new();
    let mut k = 0;
    let big = format!("r: ~%Json./parse (\"\"\"[{}]\"\"\")", list_of(100000, |_| "1".into()));
    let lit = format!("r: [{}]", list_of(50000, |i| i.to_string()));
    for (src, q, want) in [(&big, "~%List./len (_.r)", "100000"), (&lit, "~%List./at (49999, _.r)", "49999")] {
        let _alone = WEIGH.lock().unwrap_or_else(|e| e.into_inner());
        let (got, roots) = three_ways(src, "s: 1", q);
        for (way, g) in ["one file", "two evolves", "two commits"].iter().zip(got) {
            let rc = if *way == "two evolves" { "e0 e0 c0" } else if *way == "one file" { "e0 c0" } else { "e0 c0 e0 c0" };
            check(&mut bad, &format!("{} …, {way}", &src[..24]), g, &format!("{rc} | {want}"));
            k += 1;
        }
        let mut r = roots.clone();
        r.sort();
        r.dedup();
        check(&mut bad, &format!("{} …: one root", &src[..24]), r.len().to_string(), "1");
        k += 1;
    }
    verdict(bad, k);
}

/// g1 — What is kept is what was kept: roots of single commits at v0.116.0
/// do not move — lists, nested, a list in a field, open terms, a table, a
/// branch with two bodies, fields that read their siblings, strings.
#[test]
// AMENDED 2026-11-02 for Q-118 (D126): a closure frame enters identity by its digest; roots that hold one moved.
fn g1_roots_that_were_kept_do_not_move() {
    let cells: [(&[&str], &str); 12] = [
        (&["r: [1, 2, 3]"], "42d52b1d8be2c4ee717a18effdfa0b14b046d0d2885794d9b4754b3d3e6aa924"),
        (&["r: [[1], [2, [3]]]"], "985fa22997296374e033e0d02d53fd3cabefa472ceb70e36108e3374314ed83c"),
        (&["r: { xs: [1, 2], name: \"n\" }"], "857a7c1341539c7e2cd4fbcef0a7c3fdfaee23cb89bf8b3c51502bec5b3e327d"),
        (&["r: [{ a: 1 }, { a: 2 }]"], "9d1ef85f7877142be5fd59fa845bc7866f89c2ce8e439c362daea4997e7adf94"),
        (&["r: 5 |> [$, $ + 1]"], "a96de7c276c784b283f548ebef2faee79b442b77d371c2d912e356b61e95e324"),
        (&["o: { a: $ + 1 }"], "2f03b548731f804e5121c8bea53e71e81bb211a310714733fb05fa36177cb9bb"),
        (&["t: { @int: 0, 1: \"a\" }"], "ad136515ecb828dbf509917ff97cb426a82cec4fd9e5476ff239587e7782f6b4"),
        (&["t: { @int: (n -> n + 1) }", "t: { @int: (n -> n * 2) }"], "74dc95fe41bf528fea6195cdcdd1fa900813cd88ea0e7179562f97b6c83da1d7"),
        (&["r: { a: 1, b: a + 1 }"], "299eb715b7bb1be5e00195cf0c297be160358c23546fb7c7f3d4db95b9f83681"),
        (&["c: { a: c.b, b: 1 }"], "4e7080708e5a119ca9a513b7a12217125b56f21b80b5f1589d44ec029e8c5549"),
        (&["r: \"a long string\""], "fd022d418e8797ff79d2cae5249fedfeeeaef8ab017970bf3abddd70d09c8092"),
        (&["r: { z: 1, a: 2, m: [3, 1] }"], "a5eae034fae5cad0786c7d13e498ca484475979080592fb07a9949eb03ddf192"),
    ];
    let mut bad = Vec::new();
    let mut roots = vec![String::new(); cells.len()];
    std::thread::scope(|s| {
        for (r, (srcs, _)) in roots.iter_mut().zip(&cells) {
            s.spawn(move || {
                let w = Ws::new("g1");
                let got = w.writes(srcs);
                *r = if got.starts_with("e0") && got.ends_with("c0") { w.root() } else { format!("not committed: {got}") };
            });
        }
    });
    for ((srcs, want), got) in cells.iter().zip(roots) {
        check(&mut bad, &srcs.join(" / "), got, want);
    }
    verdict(bad, cells.len());
}

/// g2 — What reads right reads the same: the kept values, read back.
#[test]
fn g2_what_was_kept_reads_the_same() {
    let cells: [(&str, &str, &str); 8] = [
        ("r: [3, 1, 2]", "_.r", "[3, 1, 2]"),
        ("r: [[1], [2, [3]]]", "_.r", "[[1], [2, [3]]]"),
        ("r: { xs: [1, 2], name: \"n\" }", "_.r.xs", "[1, 2]"),
        ("r: 5 |> [$, $ + 1]", "_.r", "5"),
        ("o: { a: $ + 1 }", "4 |> _.o", "4"),
        ("r: { a: 1, b: a + 1 }", "_.r.b", "2"),
        ("c: { a: c.b, b: 1 }", "_.c.a", "1"),
        ("r: { z: 1, a: 2, m: [3, 1] }", "_.r", "{ a: 2 m: [3, 1] z: 1 }"),
    ];
    let mut bad = Vec::new();
    for (src, q, want) in cells {
        let w = Ws::new("g2");
        let got = w.writes(&[src]);
        let v = short(&w.eval(q));
        check(&mut bad, &format!("{src} → {q}"), format!("{got} | {v}"), &format!("e0 c0 | {want}"));
    }
    verdict(bad, cells.len());
}
