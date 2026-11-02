// A coordinate that redoes the rest.
// Order: nlang-tools/docs/a_coordinate_that_redoes_the_rest_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-117.  Writing a coordinate costs
// that coordinate: a file of four times the coordinates evolves and commits
// in less than eight times as long, whatever the coordinates hold, and a
// coordinate over a large root does not cost what the root holds.
//
// ── What is wrong (v0.117.0) ────────────────────────────────────────────
//
//   A file of 250 / 500 / 1000 / 2000 coordinates `k_i: i` evolves in
//   1.34 / 3.75 / 11.26 / 38.65 s (commit 0.07 to 0.16 s); two files of
//   500 take what one of 1000 takes. Each coordinate copies the working
//   set and meets it whole, twice (the working set and the session's
//   delta), restamps every thunk in it, and copies the committed root.
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
        let scratch = nlang_interpreter::ScratchDir::new(&format!("redo-rest-{tag}"));
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

/// r1 — Writing a file of coordinates costs what the file is long: four
/// times the coordinates in less than eight times the time — integers,
/// strings, expressions, records whose fields read each other, lists,
/// local coordinates, coordinates that all read one, two files at once,
/// coordinates that each hold forty elements.
#[test]
fn r1_coordinates_are_written_in_their_number() {
    let mut bad = Vec::new();
    let shapes: Vec<(&str, Box<dyn Fn(usize) -> Vec<String>>)> = vec![
        ("integers", Box::new(|n| vec![lines(n, |i| format!("k{i}: {i}"))])),
        ("strings", Box::new(|n| vec![lines(n, |i| format!("k{i}: \"s{i}\""))])),
        ("expressions", Box::new(|n| vec![lines(n, |i| format!("k{i}: {i} + 1"))])),
        ("records", Box::new(|n| vec![lines(n, |i| format!("k{i}: {{ a: {i}, b: a + 1 }}"))])),
        ("lists", Box::new(|n| vec![lines(n, |i| format!("k{i}: [{i}, {i}]"))])),
        ("local coordinates", Box::new(|n| vec![lines(n, |i| format!("~k{i}: {i}"))])),
        ("all read one", Box::new(|n| vec![format!("k0: 0\n{}", lines(n - 1, |i| format!("k{}: k0 + {i}", i + 1)))])),
        ("two files", Box::new(|n| vec![lines(n / 2, |i| format!("a{i}: {i}")), lines(n / 2, |i| format!("b{i}: a{i} + 1"))])),
    ];
    let k = shapes.len() + 1;
    for (label, files) in &shapes {
        scale(&mut bad, label, files.as_ref(), 500);
    }
    // Wide values: what a coordinate copies grows with what is staged.
    let wide = |n: usize| vec![lines(n, |i| format!("k{i}: [{}]", list_of(40, |_| i.to_string())))];
    scale(&mut bad, "lists of forty", &wide, 500);
    verdict(bad, k);
}

/// r2 — A coordinate does not cost what the root holds: 250 new
/// coordinates over a root of a 32000-element list take, beyond writing
/// one, less than twice what they take over an empty root.
#[test]
fn r2_a_coordinate_does_not_cost_the_root() {
    let _alone = WEIGH.lock().unwrap_or_else(|e| e.into_inner());
    let mut bad = Vec::new();
    let kill = Duration::from_secs(60);
    let big = format!("big: [{}]\n", list_of(32000, |i| i.to_string()));
    let new = |n: usize| vec![lines(n, |i| format!("k{i}: {i}"))];
    let (a1, e1) = write(None, &new(1), kill);
    let (a2, e250) = write(None, &new(250), kill);
    let (b1, r1) = write(Some(&big), &new(1), kill);
    let (b2, r250) = write(Some(&big), &new(250), kill);
    let empty = (e250 - e1).max(0.05);
    let over = r250 - r1;
    let ok = [&a1, &a2, &b1, &b2].iter().all(|s| s.as_str() == "e0 c0") && over < 2.0 * empty + 0.3;
    let got = format!("{a1} {a2} {b1} {b2}: over the root {over:.2}s, over nothing {empty:.2}s");
    check(&mut bad, "250 new coordinates over a 32000-element root", if ok { "flat".into() } else { got }, "flat");
    verdict(bad, 1);
}

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

/// g1 — What was written is what was written: roots and readings of mixed
/// files at v0.117.0 do not move — a forward reading, a coordinate that is
/// `_`, the same coordinate twice, a spread among coordinates, two files,
/// coordinates over a root and reading it, a self reading, a local read.
#[test]
// AMENDED 2026-11-02 for Q-118 (D126): a closure frame enters identity by its digest; roots that hold one moved.
fn g1_what_was_written_stays_written() {
    let twenty = lines(20, |i| format!("k{i}: {i}"));
    let ten = lines(10, |i| format!("k{i}: {i}"));
    let tenj = lines(10, |i| format!("j{i}: k{i} + 1"));
    let thirty = lines(30, |i| format!("b{i}: {i}"));
    let tenk = lines(10, |i| format!("k{i}: b{i} * 2"));
    let cases: Vec<(&str, Vec<Vec<&str>>, Vec<&str>, &str)> = vec![
        ("ints", vec![vec![twenty.as_str()]], vec!["_.k19"], "e0 c0 | cbd672d177922cab314b9b663afd043abdc8e6db1c9db580ffe1cfafce44b0bd | 19"),
        ("mixed", vec![vec!["a: 1\nb: \"s\"\nc: a + 1\nd: { x: 1, y: x + 1 }\ne: [1, 2]\n~f: 3\ng: ~f + 1\nh: 1 | 2\ni: #t"]], vec!["_.c", "_.d.y", "_.g", "_.h"], "e0 c0 | 342f4cebf74594942fe31ec1cdb75a1cd262a4b3884c6d5521259771e60f86b5 | 2 || 2 || 4 || 1 | 2"),
        ("forward", vec![vec!["a: z + 1\nb: 2\nz: 1"]], vec!["_.a"], "e0 c0 | 54d24d18bc02d2069de14e431a220281780a96b48b7e3a6ffbc59db6f543ea8a | 2"),
        ("top coordinate", vec![vec!["t: _\nout: t.%cause\nu: 1"]], vec!["_.out", "_.u"], "e0 c0 | 77473d9854d3065e392537d34367706bf5de7535352f65578926220f43684594 | _ || 1"),
        ("same twice", vec![vec!["x: 1\nx: 1\ny: 2"]], vec!["_.x", "_.y"], "e0 c0 | 04518d1b662974ac206331e7af92641252c96f541b6f8866b4dfa8f4a15678eb | 1 || 2"),
        ("spread among", vec![vec!["a: 1\n...{ s: 2 }\nb: a + 1"]], vec!["_.s", "_.b"], "e0 c0 | 2f8641deb930a3b9499f3d3e4535742c6a4b8b723bdae87d53724d156457819c | 2 || 2"),
        ("two files", vec![vec![ten.as_str(), tenj.as_str()]], vec!["_.j9"], "e0 c0 | ae2309c7c12615bb3a6d24c15255fe8bb52353c99dc52140475f96845cb151e9 | 10"),
        ("over a root", vec![vec![thirty.as_str()], vec![tenk.as_str()]], vec!["_.k9", "_.b29"], "e0 c0 e0 c0 | e1256490aaac5b3414bc3dbcfe239e234df5e7b991f535df6c812dc50b7d94a7 | 18 || 29"),
        ("rewrite a root coordinate", vec![vec!["a: 1\nb: 2"], vec!["a: 1\nc: a + b"]], vec!["_.c"], "e0 c0 e0 c0 | 65a1fca2c7c9088666765da032f0770968719a78474f8f5e392a5a1c33f67631 | 3"),
        ("self reading", vec![vec!["k: k + 1"]], vec!["_.k"], "e0 c0 | 114f0042c0b8893ac6091046966abc22545b816efda125d9aaa3a466bd9b909a | _|_ ;; %cause: #divergent"),
        ("local read", vec![vec!["~p: 5\nq: ~p * 2"]], vec!["_.q"], "e0 c0 | 9c63517a0034ad2bf61c3c0a6118d0dba6ab0978b1104f4c20cd7382d14991d0 | 10"),
    ];
    let mut bad = Vec::new();
    for (name, groups, qs, want) in &cases {
        let g: Vec<&[&str]> = groups.iter().map(|v| v.as_slice()).collect();
        let (rc, root, vals) = reading(&g, qs);
        check(&mut bad, name, format!("{rc} | {root} | {}", vals.join(" || ")), want);
    }
    verdict(bad, cases.len());
}

/// g2 — What was refused is refused, and an effect is kept: two values for
/// one coordinate, a `~%Config` among coordinates; a coordinate read beside
/// a nondeterministic one.
#[test]
fn g2_what_was_refused_is_refused() {
    let mut bad = Vec::new();
    let (rc, _, vals) = reading(&[&["x: 1\nx: 2"]], &["_.x"]);
    check(&mut bad, "x: 1 / x: 2", format!("{rc} | {}", vals.join(" || ")), "e1 c1 | _");
    let (rc, _, vals) = reading(&[&["a: 1\n~%Config: { fuel: 100000 }\nb: a + 1"]], &["_.b"]);
    check(&mut bad, "a config among coordinates", format!("{rc} | {}", vals.join(" || ")), "e1 c1 | _");
    let (rc, _, vals) = reading(&[&["r: ~%Math./random ()\ns: 1"]], &["_.s", "(_.r).%effect"]);
    check(&mut bad, "beside a random coordinate", format!("{rc} | {}", vals.join(" || ")), "e0 c0 | 1 || #nondet");
    verdict(bad, 3);
}
