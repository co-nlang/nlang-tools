// A text read back short.
// Order: nlang-tools/docs/a_text_read_back_short_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-114.  What a serializer writes,
// the same module's `parse` reads back as itself (SPEC_09 §5.1.2 #1), at
// every depth `stringify` writes. A text deeper than a value may be
// (`max_unification_depth`) is the horizon `#max_depth_exceeded` (SPEC_08
// §3.2.2 #1 and #3), the depth `stringify` stops at — not `#none`, which
// says "not JSON", not a conflict, which says "not TOML", and never a
// crash, however deep.
//
// ── What is wrong (v0.114.0) ────────────────────────────────────────────
//
//   `~%Json./parse` of 128 or more levels answers `#none`, the answer for
//   a text that is not JSON; `~%Toml./parse` of 80 or more levels answers
//   ⊥ `#conflict` "TOML parse error". A 200-level record `stringify` writes
//   (1201 characters) does not read back. The crates' own limits (128 and
//   80), not the engine's.
//
// Every `oo` here runs under a 1 GB address-space cap and a 10 s kill.
// r4 writes its texts to files: a million levels do not fit an argument.
//
// The delivery may NOT edit this file. `rustfmt` must not touch it.


#![allow(dead_code)]

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const KILL_AFTER: Duration = Duration::from_secs(10);

struct Ws {
    _scratch: nlang_interpreter::ScratchDir,
    root: PathBuf,
    ws: PathBuf,
}

impl Ws {
    fn new(tag: &str) -> Self {
        let scratch = nlang_interpreter::ScratchDir::new(&format!("read-back-{tag}"));
        let root = scratch.path().to_path_buf();
        let ws = root.join("ws");
        fs::create_dir_all(&ws).unwrap();
        Ws { _scratch: scratch, root, ws }
    }

    /// Run `oo` capped at 1 GB, killed after 10 s: (output, rc). A run
    /// that is killed or dies without a word says so.
    fn oo(&self, args: &[&str]) -> (String, i32) {
        let mut child = Command::new("sh")
            .arg("-c")
            .arg("ulimit -v 1000000; exec \"$0\" \"$@\"")
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

/// Split at `sep` outside brackets and strings.
fn split_top(s: &str, sep: &str) -> Vec<String> {
    let (mut out, mut cur, mut depth, mut q, mut i) = (Vec::new(), String::new(), 0i32, false, 0);
    let b = s.as_bytes();
    while i < s.len() {
        let ch = b[i] as char;
        if q {
            cur.push(ch);
            q = !(ch == '"' && (i == 0 || b[i - 1] != b'\\'));
            i += 1;
            continue;
        }
        match ch {
            '"' => q = true,
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            _ => {}
        }
        if depth == 0 && s[i..].starts_with(sep) {
            out.push(cur.trim().to_string());
            cur.clear();
            i += sep.len();
            continue;
        }
        cur.push(ch);
        i += 1;
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}

/// A union's members in one order, each once.
fn members(s: &str) -> String {
    let mut m = split_top(s, " | ");
    m.sort();
    m.dedup();
    m.join(" | ")
}

fn rec(d: usize) -> String {
    format!("{}1{}", "{ a: ".repeat(d), " }".repeat(d))
}

fn lst(d: usize) -> String {
    format!("{}1{}", "[".repeat(d), "]".repeat(d))
}

/// `d` tables, a table at the bottom (TOML's root is a table).
fn tab(d: usize) -> String {
    format!("{}{{ v: 1 }}{}", "{ a: ".repeat(d), " }".repeat(d))
}

fn varr(d: usize) -> String {
    format!("{{ v: {} }}", lst(d))
}

fn back(m: &str, v: &str) -> String {
    format!("(~%{m}./parse (~%{m}./stringify ({v}))) = {v}")
}

/// r1 — What `stringify` writes, `parse` reads back as itself, at every
/// depth: records, lists, tables, mixed shapes (a deep table beside a deep
/// array is as deep as the deeper one); and at the deepest level
/// `stringify` writes, found by writing deeper until it answers the
/// horizon — one call deep, where it stands in the round trip (the depth
/// budget counts the calls a value is written under).
#[test]
fn r1_what_is_written_reads_back() {
    let mut bad = Vec::new();
    let mut cells: Vec<(String, String)> = Vec::new();
    for d in [1, 80, 127, 128, 129, 200, 250] {
        cells.push((format!("json record {d}"), back("Json", &rec(d))));
        cells.push((format!("json list {d}"), back("Json", &lst(d))));
    }
    for d in [1, 79, 80, 81, 127, 200, 240] {
        cells.push((format!("toml tables {d}"), back("Toml", &tab(d))));
        cells.push((format!("toml array {d}"), back("Toml", &varr(d))));
    }
    let mixed = format!("{{ x: {}, y: {{ z: {} }} }}", rec(200), lst(210));
    cells.push(("json mixed 200 + 210".into(), back("Json", &mixed)));
    cells.push(("toml mixed 200 + 210".into(), back("Toml", &mixed)));
    let nested = format!("{{ x: {}{{ v: {} }}{} }}", "{ a: ".repeat(120), lst(120), " }".repeat(120));
    cells.push(("toml 120 tables over a 120 array".into(), back("Toml", &nested)));
    // A number's point is not a level.
    let floats = vec!["1.5"; 200].join(", ");
    cells.push(("toml array 240 of 200 floats".into(), back("Toml", &format!("{{ v: {}{floats}{} }}", "[".repeat(240), "]".repeat(240)))));
    let mixed_floats = format!("{{ x: {}{{ v: [1.5, 2.5, 3.5] }}{}, y: {{ z: {}{floats}{} }} }}", "{ a: ".repeat(200), " }".repeat(200), "[".repeat(210), "]".repeat(210));
    cells.push(("toml mixed 200 + 210, floats".into(), back("Toml", &mixed_floats)));
    let srcs: Vec<String> = cells.iter().map(|c| c.1.clone()).collect();
    for ((label, _), got) in cells.iter().zip(eval_all(&srcs)) {
        check(&mut bad, label, short(&got), "#true");
    }
    let mut n = cells.len();
    let shapes: [(&str, fn(usize) -> String); 3] = [("Json", lst), ("Json", rec), ("Toml", varr)];
    for (m, mk) in shapes {
        let mut deepest = None;
        for d in 240..300 {
            let w = eval_full(&format!("~%Str./len (~%Str./trim (~%{m}./stringify ({})))", mk(d)));
            if w.parse::<usize>().is_err() {
                break;
            }
            deepest = Some(d);
        }
        match deepest {
            Some(d) => check(&mut bad, &format!("{m}: the deepest stringify writes ({d}) reads back"), short(&eval_full(&back(m, &mk(d)))), "#true"),
            None => check(&mut bad, &format!("{m}: stringify writes 240 levels"), "no".into(), "yes"),
        }
        n += 1;
    }
    verdict(bad, n);
}

/// r2 — A text deeper than a value may be is the horizon
/// `#max_depth_exceeded`: lists and records in JSON; arrays, inline tables,
/// a dotted table header and a dotted key in TOML; three hundred levels to
/// fifty thousand. Not `#none`, not a conflict, not a crash.
#[test]
fn r2_a_text_too_deep_is_the_horizon() {
    let mut bad = Vec::new();
    let mut cells: Vec<(String, String)> = Vec::new();
    for d in [300, 401, 1000, 50000] {
        cells.push((format!("json list {d}"), format!("~%Json./parse (\"\"\"{}{}\"\"\")", "[".repeat(d), "]".repeat(d))));
    }
    for d in [300, 1000, 15000] {
        cells.push((format!("json record {d}"), format!("~%Json./parse (\"\"\"{}1{}\"\"\")", "{\"a\":".repeat(d), "}".repeat(d))));
    }
    cells.push(("json list 300 in a record".into(), format!("~%Json./parse (\"\"\"{{\"a\": [1, {}{}]}}\"\"\")", "[".repeat(300), "]".repeat(300))));
    for d in [300, 1000, 10000] {
        cells.push((format!("toml array {d}"), format!("~%Toml./parse (\"\"\"a = {}1{}\"\"\")", "[".repeat(d), "]".repeat(d))));
        cells.push((format!("toml inline tables {d}"), format!("~%Toml./parse (\"\"\"a = {}1{}\"\"\")", "{ b = ".repeat(d), " }".repeat(d))));
        cells.push((format!("toml header {d}"), format!("~%Toml./parse (\"\"\"[{}]\nv = 1\"\"\")", vec!["a"; d].join("."))));
        cells.push((format!("toml dotted key {d}"), format!("~%Toml./parse (\"\"\"{} = 1\"\"\")", vec!["a"; d].join("."))));
    }
    // A key's parts are levels however they are spelled.
    cells.push(("toml numeric dotted key 10000".into(), format!("~%Toml./parse (\"\"\"{} = 1\"\"\")", vec!["1"; 10000].join("."))));
    cells.push(("toml numeric header 10000".into(), format!("~%Toml./parse (\"\"\"[{}]\nv = 1\"\"\")", vec!["1"; 10000].join("."))));
    cells.push(("toml quoted dotted key 5000".into(), format!("~%Toml./parse (\"\"\"{} = 1\"\"\")", vec!["\"b\""; 5000].join("."))));
    cells.push(("toml inline table dotted key 10000".into(), format!("~%Toml./parse (\"\"\"a = {{ {} = 1 }}\"\"\")", vec!["b"; 10000].join("."))));
    cells.push(("toml array of an inline table, dotted 5000".into(), format!("~%Toml./parse (\"\"\"a = [1.5, {{ {} = 1 }}]\"\"\")", vec!["b"; 5000].join("."))));
    let srcs: Vec<String> = cells.iter().map(|c| c.1.clone()).collect();
    for ((label, _), got) in cells.iter().zip(eval_all(&srcs)) {
        check(&mut bad, label, short(&got), "#blur #max_depth_exceeded");
    }
    verdict(bad, cells.len());
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


/// r3 — Through a coordinate the same: one file, two evolves, two commits
/// read back the deep record as itself, one root; a deep text stored is
/// the horizon.
#[test]
fn r3_three_ways_one_reading_one_root() {
    let mut bad = Vec::new();
    let mut n = 0;
    let x = format!("x: {}", rec(200));
    let t = format!("t: {{ x: {}{{ v: {} }}{} }}", "{ a: ".repeat(120), lst(120), " }".repeat(120));
    let deep = format!("r: ~%Json./parse (\"\"\"{}{}\"\"\")", "[".repeat(300), "]".repeat(300));
    for (w1, w2, want) in [
        (x.as_str(), "r: (~%Json./parse (~%Json./stringify (x))) = x", "#true"),
        ("r: (~%Json./parse (~%Json./stringify (x))) = x", x.as_str(), "#true"),
        (t.as_str(), "r: (~%Toml./parse (~%Toml./stringify (t))) = t", "#true"),
        (deep.as_str(), "s: 1", "#blur #max_depth_exceeded"),
    ] {
        let (got, roots) = three_ways(w1, w2, "_.r");
        let label = format!("{} / {}", &w1[..w1.len().min(40)], &w2[..w2.len().min(40)]);
        for (way, g) in ["one file", "two evolves", "two commits"].iter().zip(got) {
            let rc = if *way == "two evolves" { "e0 e0 c0" } else if *way == "one file" { "e0 c0" } else { "e0 c0 e0 c0" };
            check(&mut bad, &format!("{label}, {way}"), g, &format!("{rc} | {want}"));
            n += 1;
        }
        let mut r = roots.clone();
        r.sort();
        r.dedup();
        check(&mut bad, &format!("{label}: one root"), r.len().to_string(), "1");
        n += 1;
    }
    verdict(bad, n);
}

/// g1 — What reads right today reads the same: a text that is not JSON is
/// `#none`, one that is not TOML a conflict; shallow texts; brackets, dots
/// and braces inside strings and comments are not depth; 127 levels.
#[test]
fn g1_what_was_read_right_is_read_the_same() {
    let mut bad = Vec::new();
    let s1000 = |c: &str| c.repeat(1000);
    let cells: Vec<(String, String)> = vec![
        ("~%Json./parse (\"\"\"{\"\"\")".into(), "#none".into()),
        ("~%Json./parse (\"\"\"[1, 2\"\"\")".into(), "#none".into()),
        ("~%Json./parse (\"\"\"[1] x\"\"\")".into(), "#none".into()),
        ("~%Json./parse (\"\"\"null\"\"\")".into(), "#none".into()),
        ("~%Toml./parse (\"\"\"a = \"\"\")".into(), "_|_ #conflict".into()),
        ("~%Toml./parse (\"\"\"a = [1\"\"\")".into(), "_|_ #conflict".into()),
        ("~%Json./parse (\"\"\"{\"a\": [1, {\"b\": \"x\"}], \"c\": 1.5}\"\"\")".into(), "{ a: [1, { b: \"x\" }] c: 1.5 }".into()),
        ("~%Toml./parse (\"\"\"[t]\nb = [1, [2]]\nc = { d = \"x\" }\"\"\")".into(), "{ t: { b: [1, [2]] c: { d: \"x\" } } }".into()),
        ("~%Toml./parse (\"\"\"a.b.c = 1\"\"\")".into(), "{ a: { b: { c: 1 } } }".into()),
        ("~%Json./parse (\"\"\"[1, 2, 3]\"\"\") = [1, 2, 3]".into(), "#true".into()),
        (format!("~%Str./len (~%List./at (0, ~%Json./parse (\"\"\"[\"{}\"]\"\"\")))", s1000("[")), "1000".into()),
        (format!("~%Str./len (~%List./at (0, ~%Json./parse (\"\"\"[\"\\\"{}\"]\"\"\")))", s1000("{")), "1001".into()),
        (format!("~%Str./len ((~%Toml./parse (\"\"\"a = \"{}\"\n\"\"\")).a)", s1000("[")), "1000".into()),
        (format!("~%Str./len ((~%Toml./parse (\"\"\"a = \"{}\"\n\"\"\")).a)", s1000(".")), "1000".into()),
        (format!("~%Str./len ((~%Toml./parse (\"\"\"a = '''\n{}'''\n\"\"\")).a)", s1000("[")), "1000".into()),
        (format!("(~%Toml./parse (\"\"\"a = 1 # {}\n\"\"\")).a", s1000("{")), "1".into()),
        (format!("~%List./len (~%Json./parse (\"\"\"{}{}\"\"\"))", "[".repeat(127), "]".repeat(127)), "1".into()),
        // Wide, not deep: a thousand siblings are one level.
        (format!("~%List./len (~%Json./parse (\"\"\"[{}]\"\"\"))", vec!["[1]"; 1000].join(", ")), "1000".into()),
        (format!("~%List./len ((~%Toml./parse (\"\"\"a = [{}]\"\"\")).a)", vec!["[1]"; 1000].join(", ")), "1000".into()),
        (format!("~%List./len (~%Reflection./keys (~%Toml./parse (\"\"\"{}\"\"\")))", (0..300).map(|i| format!("[t{i}]\nv = 1\n")).collect::<String>()), "300".into()),
    ];
    let srcs: Vec<String> = cells.iter().map(|c| c.0.clone()).collect();
    for ((e, want), got) in cells.iter().zip(eval_all(&srcs)) {
        check(&mut bad, &e[..e.len().min(90)], short(&got), want);
    }
    verdict(bad, cells.len());
}

fn root_of(src: &str) -> String {
    let w = Ws::new("root");
    let got = w.writes(&[src]);
    if got != "e0 c0" {
        return format!("not committed: {got}");
    }
    w.root()
}


/// g2 — Roots of single commits that read right at v0.114.0 do not move.
#[test]
fn g2_roots_that_read_right_do_not_move() {
    let mut bad = Vec::new();
    let deep100 = format!("r: ~%Json./parse (\"\"\"{}{}\"\"\")", "[".repeat(100), "]".repeat(100));
    let cases = [
        ("r: ~%Json./parse (\"\"\"[[1], {\"a\": 2}]\"\"\")".to_string(), "bdaacbd1f3857d746c8eaccd92c8e45f9aee4486fb756ca0e4dc56602e86dd79"),
        ("r: ~%Toml./parse (\"\"\"[t]\nb = [1, [2]]\"\"\")".to_string(), "c0fd230f4dd166d1217679c07fcede28fbab9ea9b2e976b7c6d1e539e8e985b6"),
        ("r: ~%Json./parse (\"\"\"{\"\"\")".to_string(), "0c8242bb8570326595b0b1dda5f1c8ac3b4e58d5f7b08c75ed1d59bf73935978"),
        (deep100, "352bbb92f1e4f0b147a047c3d2b58e563aeefdbf89a59c0594aecb7141529623"),
        ("r: ~%Toml./parse (\"\"\"a.b.c = 1\"\"\")".to_string(), "d5c0fd6859e4b5e8aa002f32631c4a53bd235c820bc0e3c2d621ddeac871825d"),
        ("r: ~%Json./stringify (~%Json./parse (\"\"\"[[1], {\"a\": 2}]\"\"\"))".to_string(), "6f8569af46be09a5adae97fac4dc993c492218da718f6c772c1fca1dffe1268f"),
    ];
    for (src, root) in &cases {
        check(&mut bad, &src[..src.len().min(60)], root_of(src), root);
    }
    verdict(bad, cases.len());
}

/// r4 — However deep: a text written in a file, read in a coordinate, a
/// million levels of JSON or a hundred thousand of TOML (deep enough that a
/// parser without its own limit runs out of native stack) is the horizon,
/// evolved and committed, never a crash.
#[test]
fn r4_however_deep_it_is_the_horizon() {
    let mut bad = Vec::new();
    let keys = |part: &str, n: usize| vec![part; n].join(".");
    let cells = [
        ("json list 1000000", format!("r: ~%Json./parse (\"\"\"{}{}\"\"\")", "[".repeat(1_000_000), "]".repeat(1_000_000))),
        ("json record 300000", format!("r: ~%Json./parse (\"\"\"{}1{}\"\"\")", "{\"a\":".repeat(300_000), "}".repeat(300_000))),
        ("toml array 100000", format!("r: ~%Toml./parse (\"\"\"a = {}1{}\"\"\")", "[".repeat(100_000), "]".repeat(100_000))),
        ("toml inline tables 100000", format!("r: ~%Toml./parse (\"\"\"a = {}1{}\"\"\")", "{ b = ".repeat(100_000), " }".repeat(100_000))),
        ("toml dotted key 100000", format!("r: ~%Toml./parse (\"\"\"{} = 1\"\"\")", keys("a", 100_000))),
        ("toml header 100000", format!("r: ~%Toml./parse (\"\"\"[{}]\nv = 1\"\"\")", keys("a", 100_000))),
        ("toml numeric dotted key 100000", format!("r: ~%Toml./parse (\"\"\"{} = 1\"\"\")", keys("1", 100_000))),
        ("toml quoted dotted key 50000", format!("r: ~%Toml./parse (\"\"\"{} = 1\"\"\")", keys("\"b\"", 50_000))),
        ("toml inline table dotted key 100000", format!("r: ~%Toml./parse (\"\"\"a = {{ {} = 1 }}\"\"\")", keys("b", 100_000))),
    ];
    std::thread::scope(|s| {
        let hs: Vec<_> = cells
            .iter()
            .map(|(label, src)| {
                s.spawn(move || {
                    let w = Ws::new("deep");
                    (label, format!("{} | {}", w.writes(&[src.as_str()]), short(&w.eval("_.r"))))
                })
            })
            .collect();
        for h in hs {
            let (label, got) = h.join().unwrap();
            check(&mut bad, label, got, "e0 c0 | #blur #max_depth_exceeded");
        }
    });
    verdict(bad, cells.len());
}
