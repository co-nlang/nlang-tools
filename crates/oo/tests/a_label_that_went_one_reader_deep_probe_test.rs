// A label that went one reader deep.
// Order: nlang-tools/docs/a_label_that_went_one_reader_deep_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-129. No new ruling: the fields of
// one write are a set (SYNTAX_03 §2 #1), so the order they are written in
// does not change what is stored; Q-128 M2 already says a name that arrives
// later gives its label to the coordinates that read it. A coordinate that
// reads one of those is a reader too, however many hops away.
//
// ── What is wrong (v0.127.0) ────────────────────────────────────────────
//
//   `a: [b]`, `b: [c]`, `c: [d]`, `d: [p]`, `p: ~%Env.get …` in one evolve:
//   written reader first, `a` and `b` keep no `#io` (`c`, `d` do) and the
//   commit stores another root than the same fields written name first;
//   three orders, three roots. A label goes one reader per restamp: the
//   restamp reads each value's label in the working set as it was before
//   the restamp (`predict_effect` reads the stored label). And a record's
//   own label (`ComboVal.effect`) is never restamped at all, so a record
//   that reads a record is two roots at two hops already.
//
// The other half: what was right stays as it was — one hop and two hops,
// a function chain, a pure name written last, every root of the name
// written first; and the cost of a landing stays what Q-128 made it.
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


const P: &str = "p: ~%Env.get \"Q129_UNSET\"";

/// The roots of the same fields written five ways, each in a workspace of
/// its own: every line reversed (the name first, each reader after what it
/// reads); the readers first; the readers from their second line, the
/// name, then the first; the readers in one file and the name in another,
/// one evolve; the readers in one evolve and the name in the next.
fn five_ways(readers: &str) -> Vec<(String, String)> {
    let lines: Vec<&str> = readers.lines().collect();
    let mut rev: Vec<&str> = lines.clone();
    rev.push(P);
    rev.reverse();
    let rotated = {
        let mut v: Vec<&str> = lines[1..].to_vec();
        v.push(P);
        v.push(lines[0]);
        v
    };
    let ways: Vec<(&str, Vec<Vec<String>>)> = vec![
        ("every line reversed", vec![vec![rev.join("\n")]]),
        ("the readers first", vec![vec![format!("{readers}\n{P}")]]),
        ("rotated", vec![vec![rotated.join("\n")]]),
        ("two files, one evolve", vec![vec![readers.to_string(), P.to_string()]]),
        ("two evolves", vec![vec![readers.to_string()], vec![P.to_string()]]),
    ];
    let mut out = Vec::new();
    for (label, calls) in ways {
        let w = Ws::new("ways");
        let mut rcs = Vec::new();
        for (ci, files) in calls.iter().enumerate() {
            let mut args = vec!["evolve".to_string()];
            for (i, f) in files.iter().enumerate() {
                let name = format!("c{ci}_{i}.n");
                fs::write(w.ws.join(&name), format!("{f}\n")).unwrap();
                args.push(name);
            }
            let a: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            rcs.push(format!("e{}", w.oo(&a).1));
        }
        rcs.push(format!("c{}", w.oo(&["commit", "-m", "a"]).1));
        out.push((label.to_string(), format!("{} {}", rcs.join(" "), w.root())));
    }
    out
}

/// The readers of a late name, however many hops away: a list, a record, a
/// list and a record in turn, a local coordinate, a type and a rule in the
/// middle, sums, `_.` paths, unions, a path into a record.
const CELLS: [(&str, &str); 13] = [
    ("a: [b]\nb: [c]\nc: [p]", "4b37801ae93f6a98600142e5ab68f0ddec29f67a034c5ecd85c0c21233f79383"),
    ("a: [b]\nb: [c]\nc: [d]\nd: [p]", "6f02383e0134fb1794e2efe64fac3dcaee447d0c96bf3116e33a48185f418587"),
    ("a: { x: b }\nb: { y: p }", "66eea80e16a2a83e322277196a7e43b9ed0ae43313740ae4eafdba7c4fae37f9"),
    ("a: { x: b }\nb: { y: c }\nc: { z: p }", "a771c43e5cb2956be924491f097a44f75b94b4dd0d3c90e33be4212dfb197481"),
    ("a: { x: b }\nb: [c]\nc: { z: p }", "77b284a533fe32aacb96fd745aafc0e1731a0358a543b23005c1477f5d7eecb8"),
    ("a: [b]\nb: { y: c }\nc: [p]", "3c95263fcdc5fb562be74f94ea704585c19bd887c86c848ed6a09732479ba7f0"),
    ("m: [~a]\n~a: [b]\nb: [~c]\n~c: [p]", "9d6ed5acc777df85fea10c46a4eb2813fddd4c4a6debcccacd1e984ae90aa2a9"),
    ("a: [@T]\n@T: [c]\nc: [p]", "7cd6116691a8095698438a784b3903b5fa29dc7540c0330738da84dded243cb2"),
    ("a: [/r]\n/r: [c]\nc: [p]", "03ada6df286d104f026891a1b98c7bf024bf09bfec00be55ad647348686d80d1"),
    ("a: b + \"\"\nb: c + \"\"\nc: [p]", "8b2cc1ba1f40d7b923bcca1445314adf9473e380004bbfc0eabcf8690f40be01"),
    ("a: [_.b]\nb: [_.c]\nc: [_.p]", "4b37801ae93f6a98600142e5ab68f0ddec29f67a034c5ecd85c0c21233f79383"),
    ("a: [b | 1]\nb: [c | 1]\nc: [p | 1]", "3024ccd89ce677a680e5cadbb2f367d963c8789f332ac4fe961f78636893c068"),
    ("a: [b.v]\nb: { v: c }\nc: [p]", "843323d0736a957ef26239baadc7722807d548edf74586af6702a80ce2bb012c"),
];

/// r1 — Every reader of a name that arrives later has its label, however
/// many hops away: the same fields written five ways (reversed, readers
/// first, rotated, two files, two evolves) store one root, the root of the
/// name written first (v0.127.0's). The reader four hops away reads `#io`;
/// a pin that makes the name pure takes the label from every reader again;
/// a chain of sixty, written reader first, stores the root of the same
/// chain written name first.
#[test]
fn r1_every_reader_of_a_late_name_has_its_label() {
    let mut bad = Vec::new();
    let mut n = 0;
    for (readers, want) in CELLS {
        for (label, got) in five_ways(readers) {
            n += 1;
            let rcs = if label == "two evolves" { "e0 e0 c0" } else { "e0 c0" };
            check(&mut bad, &format!("{readers:?}, {label}"), got, &format!("{rcs} {want}"));
        }
    }
    let (_, _, v) = staged_and_root(&[&format!("a: [b]\nb: [c]\nc: [d]\nd: [p]\n{P}")], &["(_.a).%effect", "(_.b).%effect"]);
    check(&mut bad, "four hops, the reader first: the labels of a and b", v.join(" | "), "#io | #io");
    n += 1;
    // A pin that makes the name pure: the readers lose the label it took
    // away, however far (the root of `p: 7` written with them).
    {
        let w = Ws::new("pin");
        fs::write(w.ws.join("f.n"), format!("a: [b]\nb: [c]\nc: [p]\n{P}\n")).unwrap();
        fs::write(w.ws.join("g.n"), "p: 7\n").unwrap();
        let e1 = w.oo(&["evolve", "f.n"]).1;
        let e2 = w.oo(&["evolve", "--pin", "--grant", "pin", "g.n"]).1;
        let c = w.oo(&["commit", "-m", "a", "--grant", "pin"]).1;
        check(&mut bad, "three hops, the readers first, then a pin makes the name pure: root",
            format!("e{e1} p{e2} c{c} {}", w.root()),
            "e0 p0 c0 39fb4282b904c85ac8bc8a640ed7747b72c963eb8986d4b2d81d859c1702568a");
        n += 1;
    }
    // Sixty hops, under the depth of an evaluation.
    let chain: Vec<String> = (0..60).map(|i| format!("k{i}: [k{}]", i + 1)).chain(std::iter::once(format!("k60: [p]"))).collect();
    let fwd = format!("{}\n{P}", chain.join("\n"));
    let rev = {
        let mut v: Vec<String> = chain.clone();
        v.push(P.to_string());
        v.reverse();
        v.join("\n")
    };
    let (_, r_fwd, _) = staged_and_root(&[&fwd], &[]);
    let (_, r_rev, _) = staged_and_root(&[&rev], &[]);
    check(&mut bad, "sixty hops: the reader first and the name first", (r_fwd == r_rev && r_fwd.len() == 64).to_string(), "true");
    n += 1;
    verdict(bad, n);
}

/// r2 — Every landing is the meet's: each cell of r1, the readers first,
/// with every in-place landing checked against the meet and its restamp
/// (`OO_Q117_SHADOW`; no `Q-117 shadow`, and landings were checked).
#[test]
fn r2_every_landing_is_the_meets() {
    let mut bad = Vec::new();
    for (readers, _) in CELLS {
        let (got, n) = shadowed(&format!("{readers}\n{P}"));
        check(&mut bad, &format!("{readers:?}, the readers first: every landing is the meet's"), got, "e0");
        check(&mut bad, &format!("{readers:?}, the readers first: landings checked"), (n > 0).to_string(), "true");
    }
    verdict(bad, CELLS.len() * 2);
}

/// r3 — The cost of a late name stays what Q-128 made it: four times the
/// readers in less than eight times the time — three hops, each reader its
/// own chain; three hops, every reader through one chain; records three
/// hops deep; the readers in one evolve and the name in the next.
#[test]
fn r3_a_label_that_goes_on_costs_its_readers() {
    let mut bad = Vec::new();
    let shapes: Vec<(&str, Box<dyn Fn(usize) -> Vec<String>>)> = vec![
        ("three hops, each reader its own chain", Box::new(|n| vec![lines(n, |i| format!("a{i}: [b{i}]\nb{i}: [c{i}]\nc{i}: [p]")) + P + "\n"])),
        ("three hops, every reader through one chain", Box::new(|n| vec![lines(n, |i| format!("a{i}: [b]")) + "b: [c]\nc: [p]\n" + P + "\n"])),
        ("records three hops deep", Box::new(|n| vec![lines(n, |i| format!("a{i}: {{ x: b{i} }}\nb{i}: {{ y: c }}")) + "c: { z: p }\n" + P + "\n"])),
    ];
    for (label, files) in &shapes {
        scale(&mut bad, label, files.as_ref(), 500);
    }
    let calls = |n: usize| vec![vec![lines(n, |i| format!("a{i}: [b{i}]\nb{i}: [c{i}]\nc{i}: [p]"))], vec![format!("{P}\n")]];
    scale_calls(&mut bad, "the readers in one evolve, the name in the next", &calls, 500);
    verdict(bad, shapes.len() + 1);
}

/// g1 — What was right stays: one hop, two hops, a record one hop, a
/// function chain, a cocoon in a chain — the readers first and the name first, the roots of
/// v0.127.0; a pure name written last leaves every reader pure, in both
/// orders; a pure chain of ten (Q-128 g1).
#[test]
fn g1_what_was_right_stays() {
    let mut bad = Vec::new();
    let cases = [
        ("a: [p]", P, "96ed881ea6245a1be0ac7d7ff6fe7452c30d02f80023e8a24d01eb93613a8056", "#io"),
        ("a: [b]\nb: [p]", P, "ecb464236a861ebb4f85453381e0e481f1764c0beb2c193d9ee449a791c63dc3", "#io"),
        ("a: { x: p }", P, "67a2e235685479f6cc9c7721d5746fc05c25572d2b73bac3ba56f9e29d5dcf9b", "#io"),
        ("a: (x -> b x)\nb: (x -> c x)", "c: (x -> ~%Env.get x)", "6fe052ffc699496a34638095ef7b56fd0bb0a0d05d73faf7a0b07ee3d15f5332", "#io"),
        ("a: [b]\nb: [c]\nc: [p]", "p: 7", "39fb4282b904c85ac8bc8a640ed7747b72c963eb8986d4b2d81d859c1702568a", "#pure"),
        // A cocoon in the chain keeps its own label (GUIDE_03 §4.2.1 Shield).
        ("a: [b]\nb: {{ v: c }}\nc: [p]", P, "e17bcda4e4c2808e4b163b0240ae24cf70e5ebee3b3c9f7ee4fed9dd79968c30", "#io"),
        ("a: [b]\nb: { w: {{ v: c }} }\nc: [p]", P, "a710eafec10b97943b00ec2ce432a24189e42f2e51f6e54cd66ccafc12d5a1b3", "#io"),
    ];
    for (readers, name, root, label) in cases {
        let rev = {
            let mut v: Vec<&str> = readers.lines().collect();
            v.push(name);
            v.reverse();
            v.join("\n")
        };
        for (way, text) in [("the readers first", format!("{readers}\n{name}")), ("every line reversed", rev)] {
            let (_, r, v) = staged_and_root(&[&text], &["(_.a).%effect"]);
            check(&mut bad, &format!("{readers:?} + {name:?}, {way}"), format!("{r} {}", v.join("")), &format!("{root} {label}"));
        }
    }
    let chain = (0..10).map(|i| format!("k{i}: k{} + 1", i + 1)).collect::<Vec<_>>().join("\n") + "\nk10: 0";
    let (_, r, v) = staged_and_root(&[&chain], &["_.k0"]);
    check(&mut bad, "a pure chain of ten", format!("{r} {}", v.join("")), "bf4b5960d07a07e32bf28c320632e37ac44685a3bc017685065eb1f148916465 10");
    verdict(bad, cases.len() * 2 + 1);
}

/// R-1 — each shape with `p: 7` written first: its root (v0.127.0's).
const PINS: [(&str, &str); 17] = [
    ("a: [p]", "9311baeaae5c22b5ed8816894b0cd2a2d9caaa384f8713ac4136d3f0e74cf1bb"),
    ("a: { x: p }", "72d75ffd0655cc8a2e7dbc1e15657dc34fd8a187979a881dab84a7c8aca47362"),
    ("a: [b]\nb: [c]\nc: [p]", "39fb4282b904c85ac8bc8a640ed7747b72c963eb8986d4b2d81d859c1702568a"),
    ("a: [b]\nb: [c]\nc: [d]\nd: [p]", "c692eb0d3d2b8d4e64ccfa05e2a46b222b13d2b5d987731753cdb62cf506afc6"),
    ("a: { x: b }\nb: { y: p }", "2ca5dae66cb23bb80de8556929166278d4cddd22243d99cd714db6cf115a8ced"),
    ("a: { x: b }\nb: { y: c }\nc: { z: p }", "86e94568b771add118eb18a31343b410c9d66370963d5ceb941f1f1a8fede3b0"),
    ("a: { x: b }\nb: [c]\nc: { z: p }", "643c90bc42abd640ed836cb22fe2d31d50596a3dbc8d1b26559e71ef8c40c252"),
    ("a: [b]\nb: { y: c }\nc: [p]", "c41d847fce03beecbd7fb90583f0b6e1c38fb3e93c3dbbee765b4eb08b1e1d2f"),
    ("~a: [b]\nb: [~c]\n~c: [p]\nm: [~a]", "c41b1c1f433e2c7fa24a6ecf179e89b30992b2913ba0cce96a43c33d487a3976"),
    ("a: [@T]\n@T: [c]\nc: [p]", "faaeecb489160cd35cb1e18561f9c231442bf261aa378fd039666074cf0af4d1"),
    ("a: [/r]\n/r: [c]\nc: [p]", "eca126577d15c94b48cddcbbb07f55b94db7edf696ffa3031fcef5f5b10c7095"),
    ("a: [_.b]\nb: [_.c]\nc: [_.p]", "4e86ac9263f241c71bb4daafa3f3b9804abc0e00af99db432003159ccdb314fa"),
    ("a: [b | 1]\nb: [c | 1]\nc: [p | 1]", "22afa8c8ef4115b351201b80e349d470337219791151d7db13738fcd6791e592"),
    ("a: [b.v]\nb: { v: c }\nc: [p]", "aaaba44d29c52c4b0f34da7dec4154be3c329c084ae5400b1cc43ee40d2d3333"),
    ("a: { x: { y: p } }", "a1d29c50f5ddd910dc8f501727ae6df5227c1116ab9176fd4660c14263547236"),
    ("a: [{ x: p }]", "7ab7424e83d33dffceaf412f87247a0878306ca0c7fd0066f3dcb9fa0cdb477c"),
    ("m: [@T]\n@T: { x: p }", "70123300f51b41977a2840cc53158a08611ac624873a6d5f9aa1ed67ebe06ae9"),
];

/// R-1 — records with a field that carries `#io` itself, beside a field that
/// reads `p`: the root once a pin made `p` pure, and `(_.a).%effect`.
/// Q-127 pre-revision (2026-11-18): the roots first pinned here were
/// v0.128.0's after the pin (`6f625b81…`, `17fe1f60…`, `39c6fbaf…`), not the
/// roots of `p: 7` written with the readers — the acceptor pinned the
/// defect. These are the pure roots (v0.128.0 and v0.129.0 store them
/// written pure); a cocoon's own label is `#pure` (L2-84).
const KEPT: [(&str, &str, &str); 3] = [
    ("k: {{ v: ~%Env.get \"Q129_K\" }}\na: { ...k\n y: p }", "821a21be04fb786afcd7ab9dfd11a53e4cfcb943a1838ac8d5b5cb2aadd0bc66", "#pure"),
    ("k: { v: ~%Env.get \"Q129_K\" }\na: { ...k\n y: p }", "55adb533df5c9f257fda3d2ffdf4700903e64e6d92960de306d40fcdbb68ca39", "#io"),
    ("a: { v: ~%Env.get \"Q129_K\"\n y: p }", "9d7737187c7d9ef49901986563bf6f124ace50819c32a03101a96f36cf67d93d", "#io"),
];

/// `readers` and the name that does IO, the readers first (in one evolve, or
/// the name in a second), then a pin that makes the name `7`, then a commit:
/// the exit codes, the root, and `(_.a).%effect`.
fn pinned_pure(readers: &str, two: bool) -> (String, String) {
    let w = Ws::new("pinned");
    let mut rcs = Vec::new();
    if two {
        rcs.push(w.evolve(&format!("{readers}\n")));
        rcs.push(w.evolve(&format!("{P}\n")));
    } else {
        rcs.push(w.evolve(&format!("{readers}\n{P}\n")));
    }
    fs::write(w.ws.join("pin.n"), "p: 7\n").unwrap();
    rcs.push(format!("p{}", w.oo(&["evolve", "--pin", "--grant", "pin", "pin.n"]).1));
    rcs.push(format!("c{}", w.oo(&["commit", "-m", "a", "--grant", "pin"]).1));
    (format!("{} {}", rcs.join(" "), w.root()), w.eval("(_.a).%effect"))
}

/// r4 (R-1) — A pin that makes the name pure takes its label back from
/// every reader, and from every record that had it from a reader: the
/// readers written first, the name pinned to `7`, store the root of `p: 7`
/// written with them. A record keeps the label a field of its own still
/// carries.
#[test]
fn r4_a_pin_that_makes_the_name_pure_takes_the_label_back() {
    let mut bad = Vec::new();
    let mut n = 0;
    for (readers, want) in PINS {
        for two in [false, true] {
            n += 1;
            let rcs = if two { "e0 e0 p0 c0" } else { "e0 p0 c0" };
            check(&mut bad, &format!("{readers:?}, the name in {} evolve", if two { "a second" } else { "the same" }),
                pinned_pure(readers, two).0, &format!("{rcs} {want}"));
        }
    }
    for (readers, want, eff) in KEPT {
        n += 1;
        let (r, e) = pinned_pure(readers, false);
        check(&mut bad, &format!("{readers:?}: a field of its own still carries it"), format!("{r} {e}"), &format!("e0 p0 c0 {want} {eff}"));
    }
    verdict(bad, n);
}
