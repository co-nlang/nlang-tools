// A table that answered what it did not read.
// Order: nlang-tools/docs/a_table_that_answered_what_it_did_not_read_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-119.  A table tests each branch by
// meeting the argument with its pattern; a test that is not a conflict was
// counted as a match. A test that ran out — a pattern that is a horizon, or a
// meet that ran out of fuel or depth — is not a conflict, so its branch was
// taken and its body answered, as though the table had read it. SPEC_08
// §3.2.2 #1: a horizon a value context meets passes out as it is.
//
// ── What is wrong (v0.119.0) ────────────────────────────────────────────
//
//   `({ /f: …, t: { @{ /f 5000 }: 1, @{ 5 }: 2, _: 7 } }).t 5` ⟹ `1`: the
//   pattern `/f 5000` is `#blur #max_depth_exceeded`; its branch is taken,
//   the branch that does match is lost, and no horizon is left in the
//   answer. A committed table of 2000 rows `t: { @{ 0 }: 0 * 2, …, _: 0 }`
//   read from the root: `_.t 2000` ⟹ `0 | 528 | 1656 | 2538 | 3434` (the
//   answer is `0`); 4000 rows: `_.t 5` ⟹ `602 | 4920 | 6964 | 7670 | #blur`,
//   which does not hold `10`. The same table evaluated directly answers
//   `#blur`.
//
// The other half: a table whose tests are all decided answers what it
// answered, and its root does not move. A horizon the argument carries where
// the pattern does not look is the argument's (§3.2.2 #2): the test decides. Which of two patterns is finer is
// the table's own bookkeeping and spends no fuel (SPEC_04 §6.5; D113), so a
// table of 60 overlapping ranges still answers.
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

struct Ws {
    _scratch: nlang_interpreter::ScratchDir,
    root: PathBuf,
    ws: PathBuf,
}

impl Ws {
    fn new(tag: &str) -> Self {
        let scratch = nlang_interpreter::ScratchDir::new(&format!("did-not-read-{tag}"));
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


const F: &str = "/f: (x -> x == 0 ? 0 : /f (x - 1))";
const H: &str = "/f 5000";
const DEPTH: &str = "#blur #max_depth_exceeded";
const FUEL: &str = "#blur #fuel_exhausted";
/// An argument whose field `a` is already a horizon (the meet forces it).
const FORCED: &str = "({ a: H, b: 5 } & { a: _, b: 5 })";

/// `({ /f: …, r: <body> }).r` — `H` inside is a horizon.
fn w(body: &str) -> String {
    format!("({{ {F}, r: {} }}).r", body.replace("H", H))
}

fn rows(n: usize) -> String {
    let rows: Vec<String> = (0..n).map(|i| format!("@{{ {i} }}: {i} * 2")).collect();
    format!("{{ {}, _: 0 }}", rows.join(", "))
}

fn committed(src: &str) -> Ws {
    let w = Ws::new("table");
    let got = w.writes(&[src]);
    assert_eq!(got, "e0 c0", "the table is written: {src:.80}");
    w
}

/// A table whose test cannot be decided answers that horizon, whichever way
/// it is reached.
#[test]
fn r1_an_undecided_test_is_not_a_match() {
    let cells: Vec<(String, &str)> = vec![
        (w("{ @{ H }: 1, _: 7 } 5"), DEPTH),
        (w("{ @{ H }: 1, @{ 5 }: 2, _: 7 } 5"), DEPTH),
        (w("{ @{ H }: 1, @{ 6 }: 2 } 5"), DEPTH),
        (w("({ @{ H }: 1 } & { @{ 5 }: 2 }) 5"), DEPTH),
        (format!("({{ {F}, /g: {{ @{{ {H} }}: 1, @{{ 5 }}: 2 }}, r: /g 5 }}).r"), DEPTH),
        (w("(x @{ H } -> 1) 5"), DEPTH),
        (w("{ @{ H }: 1, @{ 5 }: 2, _: 7 } (5 | 6)"), DEPTH),
        (w("{ @{ { a: H } }: 1, _: 7 } { a: 5 }"), DEPTH),
        (w("{ @{ @int & (H) }: 1, _: 7 } 5"), DEPTH),
        (w("{ @{ { a: @int } }: 1, _: 7 } { a: H }"), DEPTH),
        (w(&format!("{{ @{{ {{ a: @int }} }}: 1, _: 7 }} {FORCED}")), DEPTH),
        (w("{ @{ { ~p: @int, b: 5 } }: 1, _: 7 } ({ ~p: H, b: 5 } & { ~p: _, b: 5 })"), DEPTH),
        (format!("({{ {F}, t: {{ @{{ {H} }}: 1, @{{ 5 }}: 2 }} }}).t 5"), DEPTH),
    ];
    let mut bad = Vec::new();
    let refs: Vec<(&str, &str)> = cells.iter().map(|(e, w)| (e.as_str(), *w)).collect();
    table(&mut bad, &refs);
    verdict(bad, cells.len());
}

/// A committed table that runs out of fuel while it is read answers the true
/// value or the horizon, never a value no branch it read gives.
#[test]
fn r2_a_large_table_read_from_the_root() {
    let mut bad = Vec::new();
    let mut n_cells = 0;
    for n in [2000usize, 4000] {
        let w = committed(&format!("t: {}\n", rows(n)));
        for (q, truth) in [
            ("_.t 5".to_string(), "10".to_string()),
            ("_.t 0".to_string(), "0".to_string()),
            (format!("_.t {n}"), "0".to_string()),
            (format!("_.t {}", n - 1), format!("{}", 2 * (n - 1))),
        ] {
            n_cells += 1;
            let got = short(&w.eval(&q));
            if got != truth && got != FUEL {
                bad.push(format!("  {n} rows {q}\n    want {truth} or {FUEL}\n    got  {}", got.chars().take(200).collect::<String>()));
            }
        }
    }
    verdict(bad, n_cells);
}

/// The same table read from the root and evaluated directly answers the same.
#[test]
fn r3_two_paths_one_answer() {
    let mut bad = Vec::new();
    let w = committed(&format!("t: {}\n", rows(2000)));
    let qs = ["5", "0", "2000", "1999"];
    for a in qs {
        let root = short(&w.eval(&format!("_.t {a}")));
        let direct = short(&eval_full(&format!("({}) {a}", rows(2000))));
        check(&mut bad, &format!("2000 rows at {a}: root vs direct"), root, &direct);
    }
    verdict(bad, qs.len());
}

/// The other half: tests that are all decided answer what they answered.
#[test]
fn g1_a_decided_table_answers_as_before() {
    let mut bad = Vec::new();
    let cells: Vec<(String, &str)> = vec![
        (w("{ @{ /f 3 }: 1, _: 7 } 5"), "7"),
        ("({ @{ 5 }: 2, _: 7 }) 5".to_string(), "2"),
        ("({ @{ 5 }: 2 }) 6".to_string(), "_|_ #no_matching_branch"),
        ("({ @{ 0.. }: 0, @{ 10.. }: 1, @{ 20.. }: 2 }) 15".to_string(), "1"),
        ("({ @{ @int }: $ + 1 } & { @{ @int }: $ * 2 }) 5".to_string(), "_|_ #conflict"),
        (w("{ @{ 5 }: H, _: 7 } 5"), DEPTH),
        (w("{ @{ 5 }: H, _: 7 } 6"), "7"),
        (w("{ @{ @int }: 1, _: 7 } (H)"), DEPTH),
        (w("{ @{ 5 }: 1, _: 7 } (H)"), DEPTH),
        (w("{ _: 7 } { a: H }"), "7"),
        (w("{ @{ { b: 5 } }: 1, _: 7 } { a: H, b: 5 }"), "1"),
        (w(&format!("{{ @{{ {{ b: 5 }} }}: 1, _: 7 }} {FORCED}")), "1"),
        (w(&format!("{{ _: 7 }} {FORCED}")), "7"),
        (w(&format!("{{ @{{ {{ a: _, b: 5 }} }}: 1, _: 7 }} {FORCED}")), "1"),
        (w("{ _: 7 } (H, 5)"), "7"),
        (w("{ @{ { c: H } }: 1, _: 7 } { b: 5 }"), "1"),
    ];
    let refs: Vec<(&str, &str)> = cells.iter().map(|(e, w)| (e.as_str(), *w)).collect();
    table(&mut bad, &refs);
    let mut n = cells.len();
    // Sixty overlapping ranges: which is finer is decided without fuel.
    let ranges: Vec<String> = (0..60).map(|i| format!("@{{ {i}.. }}: {i}")).collect();
    let t = committed(&format!("t: {{ {} }}\n", ranges.join(", ")));
    check(&mut bad, "60 overlapping ranges at 1000", short(&t.eval("_.t 1000")), "59");
    n += 1;
    let t = committed(&format!("t: {}\n", rows(1900)));
    for (q, want) in [("_.t 1899", "3798"), ("_.t 5", "10"), ("_.t 1900", "0")] {
        check(&mut bad, &format!("1900 rows {q}"), short(&t.eval(q)), want);
        n += 1;
    }
    verdict(bad, n);
}

/// The other half: no stored table moves.
#[test]
fn g2_roots_do_not_move() {
    let mut bad = Vec::new();
    let cells: Vec<(String, &str)> = vec![
        ("t: { @{ 5 }: 2, _: 7 }\n".to_string(), "f20c6ac367df86e0078db46a6614707af5199c25d52c646033b99a09a5ae130a"),
        ("t: { @{ @int }: $ + 1, @{ @str }: 0 }\n".to_string(), "f79632267352dca07155eeeef4514b0ee702c1fcb405c189e684b411751bf182"),
        ("t: { @{ 0.. }: 0, @{ 10.. }: 1, @{ 20.. }: 2 }\n".to_string(), "57b30a0f5d6c6312f76c2aff11e6fb8c8308d1c8d5d0bb650e9e2ce71a937b79"),
        ("/g: { @{ 5 }: 2, _: 7 }\nr: /g 5\n".to_string(), "1319a6e310a7de8838d8f459607858772b81e2611cd644f50c6a322e82965cfd"),
        (format!("t: {}\n", rows(1900)), "ab297d47f55c1b19548980f17bf941a0e80dd870e4a0a2985d78012c242f8fa4"),
    ];
    for (src, want) in &cells {
        let w = committed(src);
        check(&mut bad, &src.chars().take(60).collect::<String>(), w.root(), want);
    }
    verdict(bad, cells.len());
}
