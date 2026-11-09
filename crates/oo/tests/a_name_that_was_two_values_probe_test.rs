// A name that was two values.
// Order: nlang-tools/docs/a_name_that_was_two_values_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-126. No new ruling: the commit
// keeps what an effect cell observed (SPEC_10 §2.3.1 — the criterion is
// `%effect`, and `%effect` is the effective tag, O61); once an observation
// collapses, its active tags solidify (SPEC_08 §4.2.4); a name read in one
// observation is read as it collapsed (SPEC_04 §6) — a coordinate is one
// value, so `a = a` is `#true` (SPEC_06). D129 (乙, 2026-11-09): the working
// set is lazy; a definition that reads a lazy field which does an effect is
// itself left as its recipe at `oo evolve` and observed at commit, with
// that field — one value, one effect.
//
// ── What is wrong (v0.125.0) ────────────────────────────────────────────
//
//   History: a record field whose definition does an effect but is
//   predicted pure (an arrow applied in place, the record's own function,
//   an alias of an effect field, a builtin applying a function) is stored
//   as its recipe. `h` appends one byte and reads Q126. Commit
//   `q: { a: (s -> h s) "Q126" }` with Q126=one: three bytes; every later
//   `oo eval _.q.a` appends one more and reads the Q126 of that process.
//   `q: { a: ~%Math./random (), e: a = a }` committed: `_.q.e` is `#false`
//   forever. `k: q.a` beside `q: { a: h "Q126" }`: two bytes at commit.
//
//   One observation: `{ a: (x -> ~%Math./random ()) 1, b: a = a }.b` is
//   `#false`; `{ f: (x -> append), a: f 1 }.a` appends three bytes;
//   `append |> (_ -> _)` two.
//
// The other half: two coordinates written alike are two effects; what does
// not read an effect is stored as before (a pure recipe reading HEAD stays
// a recipe, D110); roots of top-level effect cells and of record fields
// that v0.125.0 already captured do not move.
//
// Every `oo` here runs under a 3 GB address-space cap and a kill inside
// the probe (90 s). The probe sets Q126 for every `oo` it runs.
//
// The delivery may NOT edit this file. `rustfmt` must not touch it.

#![allow(dead_code)]

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const KILL_AFTER: Duration = Duration::from_secs(90);

/// Appends one byte to `a.log`, then reads the environment variable named `s`.
const H: &str = "h: (s -> ~%Io./append_file (\"a.log\", \"Z\") |> (_ -> ~%Env.get s))";
/// Appends one byte to `a.log`.
const A: &str = "~%Io./append_file (\"a.log\", \"Z\")";

struct Ws {
    _scratch: nlang_interpreter::ScratchDir,
    root: PathBuf,
    ws: PathBuf,
}

impl Ws {
    fn new(tag: &str) -> Self {
        let scratch = nlang_interpreter::ScratchDir::new(&format!("twovalues-{tag}"));
        let root = scratch.path().to_path_buf();
        let ws = root.join("ws");
        fs::create_dir_all(&ws).unwrap();
        fs::write(ws.join("a.log"), "").unwrap();
        Ws { _scratch: scratch, root, ws }
    }

    /// Run `oo` with Q126=`q126`, capped at 3 GB, killed after 90 s: (output, rc).
    fn oo_in(&self, q126: &str, args: &[&str], stdin: Option<&str>) -> (String, i32) {
        let mut child = Command::new("sh")
            .arg("-c")
            .arg("ulimit -v 3000000; exec \"$0\" \"$@\"")
            .arg(env!("CARGO_BIN_EXE_oo"))
            .args(args)
            .current_dir(&self.ws)
            .env("OO_IDENTITY", self.root.join("identity"))
            .env("OO_NODE_HOME", self.root.join("node-home"))
            .env("Q126", q126)
            .stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("oo runs");
        if let Some(text) = stdin {
            use std::io::Write;
            let mut si = child.stdin.take().unwrap();
            let _ = si.write_all(text.as_bytes());
        }
        let drain = |r: Option<Box<dyn std::io::Read + Send>>| {
            std::thread::spawn(move || {
                let mut buf = Vec::new();
                if let Some(mut r) = r {
                    let _ = r.read_to_end(&mut buf);
                }
                buf
            })
        };
        let out = drain(child.stdout.take().map(|r| Box::new(r) as Box<dyn std::io::Read + Send>));
        let err = drain(child.stderr.take().map(|r| Box::new(r) as Box<dyn std::io::Read + Send>));
        let start = Instant::now();
        let status = loop {
            if let Some(st) = child.try_wait().expect("wait") {
                break st;
            }
            if start.elapsed() > KILL_AFTER {
                let _ = child.kill();
                let _ = child.wait();
                return ("DID NOT RETURN".into(), -1);
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        let (out, err) = (out.join().unwrap_or_default(), err.join().unwrap_or_default());
        let text = format!("{}{}", String::from_utf8_lossy(&out), String::from_utf8_lossy(&err));
        let rc = status.code().unwrap_or(-1);
        if text.contains("panicked") {
            return ("PANIC".into(), rc);
        }
        if text.contains("stack overflow") || text.contains("memory allocation") || (rc != 0 && text.trim().is_empty()) {
            return ("CRASHED".into(), rc);
        }
        (text.trim().to_string(), rc)
    }

    fn oo(&self, q126: &str, args: &[&str]) -> (String, i32) {
        self.oo_in(q126, args, None)
    }

    fn file(&self, src: &str) -> String {
        let n = fs::read_dir(&self.ws).map(|d| d.count()).unwrap_or(0);
        let f = format!("f{n}.n");
        fs::write(self.ws.join(&f), src).unwrap();
        f
    }

    /// Bytes appended so far.
    fn bytes(&self) -> u64 {
        fs::metadata(self.ws.join("a.log")).map(|m| m.len()).unwrap_or(0)
    }

    /// Evolve one file and commit, both with Q126=one: "e0 landed" when both did.
    fn write(&self, src: &str) -> String {
        let f = self.file(src);
        let (o, rc) = self.oo("one", &["evolve", &f]);
        let e = if rc == 0 { "e0".to_string() } else { format!("e{rc} {}", o.chars().take(60).collect::<String>()) };
        let (o, _) = self.oo("one", &["commit", "-m", "c"]);
        format!("{e} {}", said(&o))
    }

    /// `oo eval` with Q126=`q126`, the trailing effect note dropped.
    fn eval_in(&self, q126: &str, e: &str) -> String {
        let s = self.oo(q126, &["eval", "--", e]).0;
        squash(&strip_notes(&s))
    }

    /// The committed root's digest.
    fn root(&self) -> String {
        let head = fs::read_to_string(self.ws.join(".oo").join("HEAD")).unwrap_or_default();
        if head.trim().is_empty() {
            return String::new();
        }
        let (o, _) = self.oo("one", &["inspect", head.trim()]);
        o.lines()
            .find(|l| l.contains("root:"))
            .and_then(|l| l.rsplit(':').next())
            .unwrap_or("")
            .trim()
            .to_string()
    }
}

/// Every `;; %effect: …` note dropped, wherever it stands.
fn strip_notes(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find(";;") {
        out.push_str(&rest[..i]);
        let tail = &rest[i + 2..];
        // the note is `%effect: #tag` (one word after the colon)
        let t = tail.trim_start();
        let t = t.strip_prefix("%effect:").map(|x| x.trim_start()).unwrap_or(t);
        let end = t.find(|c: char| c.is_whitespace() || c == ',' || c == ']' || c == '}').unwrap_or(t.len());
        rest = &t[end..];
    }
    out.push_str(rest);
    out
}

fn said(o: &str) -> String {
    if o.contains("Commit successful") {
        "landed".into()
    } else if o.contains("already in HEAD") {
        "held".into()
    } else {
        squash(o)
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

fn cells(bad: &mut Vec<String>, run: Vec<Box<dyn Fn() -> Vec<String> + Send + Sync>>) -> usize {
    let n = run.len();
    let per = n.div_ceil(4).max(1);
    let mut out: Vec<Vec<String>> = vec![Vec::new(); n];
    std::thread::scope(|s| {
        for (o, r) in out.chunks_mut(per).zip(run.chunks(per)) {
            s.spawn(move || {
                for (o, f) in o.iter_mut().zip(r) {
                    *o = f();
                }
            });
        }
    });
    for o in out {
        bad.extend(o);
    }
    n
}

/// Commit `src` (with `h`) under Q126=one, then read `rd` twice under
/// Q126=two: one byte by the commit, none more, and the reading is what the
/// commit observed.
fn kept(label: &'static str, src: &'static str, rd: &'static str, want: &'static str) -> Box<dyn Fn() -> Vec<String> + Send + Sync> {
    Box::new(move || {
        let mut bad = Vec::new();
        let w = Ws::new(label);
        check(&mut bad, &format!("{label}: write"), w.write(&format!("{H}\n{src}\n")), "e0 landed");
        check(&mut bad, &format!("{label}: bytes after commit"), w.bytes().to_string(), "1");
        let first = w.eval_in("two", rd);
        let second = w.eval_in("two", rd);
        check(&mut bad, &format!("{label}: bytes after two reads"), w.bytes().to_string(), "1");
        check(&mut bad, &format!("{label}: {rd}, first read"), first, want);
        check(&mut bad, &format!("{label}: {rd}, second read"), second, want);
        bad
    })
}

// ── r1 ── what an effect gave is what the commit keeps, however it is written ──

#[test]
fn r1_the_commit_keeps_what_an_effect_gave() {
    let mut bad = Vec::new();
    let run: Vec<Box<dyn Fn() -> Vec<String> + Send + Sync>> = vec![
        kept("r1-arrow", "q: { a: (s -> h s) \"Q126\" }", "_.q.a", "\"one\""),
        kept("r1-own", "q: { f: (s -> h s), a: f \"Q126\" }", "_.q.a", "\"one\""),
        kept("r1-alias", "q: { a: h \"Q126\", b: a }", "_.q.b", "\"one\""),
        kept("r1-map", "q: { a: ~%List./map (h, [\"Q126\"]) }", "_.q.a 0", "\"one\""),
        kept("r1-reader", "q: { a: h \"Q126\" }\nk: q.a", "_.k", "\"one\""),
        kept("r1-reader-first", "k: q.a\nq: { a: (s -> h s) \"Q126\" }", "_.k", "\"one\""),
        kept("r1-nested", "q: { r: { a: (s -> h s) \"Q126\" } }", "_.q.r.a", "\"one\""),
        // A nested record whose field reads a sibling written after it.
        kept("r1-nested-reader", "q: { r: { b: a, a: (s -> h s) \"Q126\" } }", "_.q.r.b", "\"one\""),
        kept("r1-pipe", "q: { a: \"Q126\" |> (s -> h s) }", "_.q.a", "\"one\""),
        // A cocoon is built at once (SPEC_04 §6.5) and shields its fields'
        // tags (SPEC_08 §4.2.1): what evolve observed is still what is kept.
        kept("r1-cocoon", "q: {{ a: (s -> h s) \"Q126\" }}", "_.q.a", "\"one\""),
        // Fields written alike in one record, all read by one: one byte each.
        Box::new(|| {
            let mut bad = Vec::new();
            let w = Ws::new("r1-alike-read");
            check(&mut bad, "r1 alike read: write", w.write(&format!("{H}\nq: {{ a: h \"Q126\", b: h \"Q126\", c: h \"Q126\", all: [a, b, c] }}\n")), "e0 landed");
            check(&mut bad, "r1 alike read: bytes after commit", w.bytes().to_string(), "3");
            check(&mut bad, "r1 alike read: _.q.all 2", w.eval_in("two", "_.q.all 2"), "\"one\"");
            check(&mut bad, "r1 alike read: bytes after a read", w.bytes().to_string(), "3");
            bad
        }),
        // Two coordinates written alike: one byte each, none more on reads.
        Box::new(|| {
            let mut bad = Vec::new();
            let w = Ws::new("r1-alike");
            check(&mut bad, "r1 alike: write", w.write(&format!("{H}\nq: {{ a: (s -> h s) \"Q126\" }}\nq2: {{ a: (s -> h s) \"Q126\" }}\n")), "e0 landed");
            check(&mut bad, "r1 alike: bytes after commit", w.bytes().to_string(), "2");
            let _ = w.eval_in("two", "_.q.a");
            let _ = w.eval_in("two", "_.q2.a");
            check(&mut bad, "r1 alike: bytes after reads", w.bytes().to_string(), "2");
            bad
        }),
    ];
    let n = cells(&mut bad, run);
    verdict(bad, n);
}

/// Commit `src` under Q126=one; every `(expr, want)` read twice must agree.
fn one_value(label: &'static str, src: &'static str, reads: &'static [(&'static str, &'static str)]) -> Box<dyn Fn() -> Vec<String> + Send + Sync> {
    Box::new(move || {
        let mut bad = Vec::new();
        let w = Ws::new(label);
        check(&mut bad, &format!("{label}: write"), w.write(&format!("{src}\n")), "e0 landed");
        for (rd, want) in reads {
            let first = w.eval_in("two", rd);
            let second = w.eval_in("two", rd);
            check(&mut bad, &format!("{label}: {rd}"), first, want);
            check(&mut bad, &format!("{label}: {rd}, read again"), second, want);
        }
        bad
    })
}

// ── r2 ── one name, one value ──

#[test]
fn r2_a_name_is_one_value() {
    let mut bad = Vec::new();
    let mut run: Vec<Box<dyn Fn() -> Vec<String> + Send + Sync>> = vec![
        one_value(
            "r2-field",
            "q: { a: ~%Math./random (), b: [a, a], e: a = a }",
            &[("_.q.e", "#true"), ("_.q.b 0 = _.q.b 1", "#true"), ("_.q.a = _.q.b 0", "#true")],
        ),
        one_value(
            "r2-arrow",
            "q: { a: (x -> ~%Math./random ()) 1, b: [a, a], e: a = a }",
            &[("_.q.e", "#true"), ("_.q.b 0 = _.q.b 1", "#true"), ("_.q.a = _.q.b 0", "#true")],
        ),
        one_value("r2-before", "q: { e: a = a, a: ~%Math./random () }", &[("_.q.e", "#true")]),
        one_value(
            "r2-top",
            "q: { a: ~%Math./random () }\nm: [q.a, q.a]\ne: q.a = q.a",
            &[("_.e", "#true"), ("_.m 0 = _.m 1", "#true"), ("_.q.a = _.m 0", "#true")],
        ),
    ];
    // In one `oo eval`, with no history.
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        let w = Ws::new("r2-eval");
        for (e, want) in [
            ("{ a: (x -> ~%Math./random ()) 1, b: a = a }.b", "#true"),
            ("{ a: ~%Math./random (), b: [a, a] }.b |> (l -> l 0 = l 1)", "#true"),
            ("{ a: ~%Math./random (), b: a = a }.b", "#true"),
        ] {
            check(&mut bad, &format!("r2-eval: {e}"), w.eval_in("two", e), want);
        }
        bad
    }));
    let n = cells(&mut bad, run);
    verdict(bad, n);
}

/// One `oo eval` of `e`: the bytes it appended.
fn appended(tag: &str, e: &str) -> String {
    let w = Ws::new(tag);
    let (o, _) = w.oo("two", &["eval", "--", e]);
    if o == "PANIC" || o == "CRASHED" || o == "DID NOT RETURN" {
        return o;
    }
    w.bytes().to_string()
}

// ── r3 ── one observation does each effect it reads once ──

#[test]
fn r3_one_observation_does_an_effect_once() {
    let mut bad = Vec::new();
    let mut run: Vec<Box<dyn Fn() -> Vec<String> + Send + Sync>> = Vec::new();
    for (i, (e, want)) in [
        (format!("{{ r: (x -> {A}) 1 }}.r"), "1"),
        (format!("{{ f: (x -> {A}), a: f 1 }}.a"), "1"),
        (format!("{{ f: (x -> {A}), a: f 1, b: a }}.b"), "1"),
        (format!("{{ r: ~%List./map ((x -> {A} |> (_ -> x)), [1, 2, 3]) }}.r"), "3"),
        (format!("{{ g: (s -> {A}), q: {{ a: g 1 }}, k: q.a }}.k"), "1"),
        (format!("{A} |> (_ -> _)"), "1"),
        (format!("{{ a: (x -> {A}) 1, b: [a, a] }}.b"), "1"),
        (format!("{{ f: (x -> {A}), a: f 1, b: f 1 }}"), "2"),
        // D109: a branch body with an effect is code until chosen; chosen
        // twice, it is two applications.
        (format!("{{ t: {{ @{{ 1 }}: {A}, _: 0 }}, a: [t 1, t 1] }}.a"), "2"),
    ]
    .into_iter()
    .enumerate()
    {
        run.push(Box::new(move || {
            let mut bad = Vec::new();
            check(&mut bad, &format!("r3: {e}"), appended(&format!("r3-{i}"), &e), want);
            bad
        }));
    }
    let n = cells(&mut bad, run);
    verdict(bad, n);
}

// ── r4 ── every way an observation is made ──

#[test]
fn r4_every_way_an_observation_is_made() {
    let mut bad = Vec::new();
    let run: Vec<Box<dyn Fn() -> Vec<String> + Send + Sync>> = vec![
        // `oo test`: one program.
        Box::new(|| {
            let mut bad = Vec::new();
            let w = Ws::new("r4-test");
            let f = w.file(&format!("q: {{ a: (s -> {A}) 1 }}\ntest_k: q.a & #true\n"));
            let (o, _) = w.oo("two", &["test", &f]);
            check(&mut bad, "r4 test: bytes", w.bytes().to_string(), "1");
            check(&mut bad, "r4 test: passed", (!o.contains("FAIL")).to_string(), "true");
            bad
        }),
        // `repl`: one line.
        Box::new(|| {
            let mut bad = Vec::new();
            let w = Ws::new("r4-repl");
            let _ = w.oo_in("two", &["repl"], Some(&format!("q: {{ a: (s -> {A}) 1 }}, k: q.a\n")));
            check(&mut bad, "r4 repl: bytes", w.bytes().to_string(), "1");
            bad
        }),
        // `oo run`: one program, the reader of a lazy field observes it.
        Box::new(|| {
            let mut bad = Vec::new();
            let w = Ws::new("r4-run");
            let f = w.file(&format!("{H}\nq: {{ a: (s -> h s) \"Q126\" }}\nk: q.a + \"!\"\n"));
            let _ = w.oo("two", &["run", &f]);
            check(&mut bad, "r4 run: bytes", w.bytes().to_string(), "1");
            bad
        }),
        // evolve, status, commit, eval: once in all.
        Box::new(|| {
            let mut bad = Vec::new();
            let w = Ws::new("r4-steps");
            let f = w.file(&format!("{H}\nq: {{ a: (s -> h s) \"Q126\", b: a }}\nk: q.b\n"));
            let (_, rc) = w.oo("one", &["evolve", &f]);
            check(&mut bad, "r4 steps: evolve rc", rc.to_string(), "0");
            let _ = w.oo("one", &["status"]);
            let _ = w.oo("one", &["commit", "-m", "c"]);
            let _ = w.oo("two", &["eval", "_.k"]);
            let _ = w.oo("two", &["status"]);
            check(&mut bad, "r4 steps: bytes", w.bytes().to_string(), "1");
            check(&mut bad, "r4 steps: _.k", w.eval_in("two", "_.k"), "\"one\"");
            bad
        }),
    ];
    let n = cells(&mut bad, run);
    verdict(bad, n);
}

// ── r5 ── D129: a reader of a lazy field and the field are one value ──

/// Take the evolves (each a file), then commit: what the commit and two
/// later reads leave.
fn readers(tag: &str, evolves: &[&str], commits_between: bool) -> Ws {
    let w = Ws::new(tag);
    for (i, src) in evolves.iter().enumerate() {
        let f = w.file(&format!("{src}\n"));
        let _ = w.oo("one", &["evolve", &f]);
        if commits_between && i + 1 < evolves.len() {
            let _ = w.oo("one", &["commit", "-m", "c"]);
        }
    }
    let _ = w.oo("one", &["commit", "-m", "c"]);
    w
}

#[test]
fn r5_a_reader_of_a_lazy_field_is_that_field() {
    let mut bad = Vec::new();
    let mut run: Vec<Box<dyn Fn() -> Vec<String> + Send + Sync>> = Vec::new();
    for (tag, evolves, between, rd) in [
        ("r5-same", vec!["q: { a: (x -> ~%Math./random ()) 1 }\nk: q.a + 0"], false, "_.k = _.q.a"),
        ("r5-first", vec!["k: q.a + 0\nq: { a: (x -> ~%Math./random ()) 1 }"], false, "_.k = _.q.a"),
        ("r5-element", vec!["q: { b: [(x -> ~%Math./random ()) 1] }\nk: q.b 0"], false, "_.k = _.q.b 0"),
        ("r5-two-evolves", vec!["q: { a: (x -> ~%Math./random ()) 1 }", "k: q.a + 0"], false, "_.k = _.q.a"),
        ("r5-head", vec!["q: { a: (x -> ~%Math./random ()) 1 }", "k: q.a + 0"], true, "_.k = _.q.a"),
    ] {
        run.push(Box::new(move || {
            let mut bad = Vec::new();
            let w = readers(tag, &evolves, between);
            check(&mut bad, &format!("{tag}: {rd}"), w.eval_in("two", rd), "#true");
            bad
        }));
    }
    // Bytes: the reader is left lazy at evolve, the commit does the effect once.
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        let w = Ws::new("r5-bytes");
        let f = w.file(&format!("{H}\nq: {{ a: h \"Q126\" }}\nk: q.a + \"!\"\n"));
        let (_, rc) = w.oo("one", &["evolve", &f]);
        check(&mut bad, "r5 bytes: evolve rc", rc.to_string(), "0");
        check(&mut bad, "r5 bytes: after evolve (D129)", w.bytes().to_string(), "0");
        let _ = w.oo("one", &["commit", "-m", "c"]);
        check(&mut bad, "r5 bytes: after commit", w.bytes().to_string(), "1");
        check(&mut bad, "r5 bytes: _.k", w.eval_in("two", "_.k"), "\"one!\"");
        check(&mut bad, "r5 bytes: after reads", w.bytes().to_string(), "1");
        bad
    }));
    // The reader's own effect and the field's: two, both at commit.
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        let w = Ws::new("r5-own");
        let f = w.file(&format!("{H}\nq: {{ a: h \"Q126\" }}\nk: h \"Q126\" + q.a\n"));
        let _ = w.oo("one", &["evolve", &f]);
        check(&mut bad, "r5 own: after evolve (D129)", w.bytes().to_string(), "0");
        let _ = w.oo("one", &["commit", "-m", "c"]);
        check(&mut bad, "r5 own: after commit", w.bytes().to_string(), "2");
        check(&mut bad, "r5 own: _.k", w.eval_in("two", "_.k"), "\"oneone\"");
        check(&mut bad, "r5 own: after a read", w.bytes().to_string(), "2");
        bad
    }));
    let n = cells(&mut bad, run);
    verdict(bad, n);
}

// ── g1 ── the rest is as before ──

#[test]
fn g1_the_rest_is_as_before() {
    let mut bad = Vec::new();
    let run: Vec<Box<dyn Fn() -> Vec<String> + Send + Sync>> = vec![
        // Two coordinates written alike are never one effect (v0.125.0 does
        // more than two; sharing them would do one).
        Box::new(|| {
            let mut bad = Vec::new();
            let w = Ws::new("g1-alike");
            check(&mut bad, "g1 alike: write", w.write(&format!("{H}\nq: {{ a: (s -> h s) \"Q126\" }}\nq2: {{ a: (s -> h s) \"Q126\" }}\n")), "e0 landed");
            check(&mut bad, "g1 alike: at least two bytes", (w.bytes() >= 2).to_string(), "true");
            bad
        }),
        Box::new(|| {
            let mut bad = Vec::new();
            let n = appended("g1-ab", &format!("{{ f: (x -> {A}), a: f 1, b: f 1 }}"));
            check(&mut bad, "g1 alike, one eval: a: f 1, b: f 1 at least two", (n.parse::<u64>().unwrap_or(0) >= 2).to_string(), "true");
            check(&mut bad, "g1 alike, one eval: [A, A]", appended("g1-list", &format!("[{A}, {A}]")), "2");
            check(&mut bad, "g1 per element", appended("g1-map", &format!("~%List./map ((x -> {A} |> (_ -> x)), [1, 2, 3])")), "3");
            bad
        }),
        // D129 reaches only a lazy field that does an effect: a reader of a
        // pure field with an effect of its own observes at evolve.
        Box::new(|| {
            let mut bad = Vec::new();
            let w = Ws::new("g1-pure-field");
            let f = w.file(&format!("{H}\nq: {{ a: \"!\" }}\nk: h \"Q126\" + q.a\n"));
            let _ = w.oo("one", &["evolve", &f]);
            check(&mut bad, "g1 pure field: after evolve", w.bytes().to_string(), "1");
            let _ = w.oo("one", &["commit", "-m", "c"]);
            check(&mut bad, "g1 pure field: after commit", w.bytes().to_string(), "1");
            check(&mut bad, "g1 pure field: _.k", w.eval_in("two", "_.k"), "\"one!\"");
            bad
        }),
        // A pure recipe reading HEAD stays a recipe (D110).
        Box::new(|| {
            let mut bad = Vec::new();
            let w = Ws::new("g1-recipe");
            check(&mut bad, "g1 recipe: write", w.write("a: 1\nb: a + 1\n"), "e0 landed");
            check(&mut bad, "g1 recipe: pin a", {
                let f = w.file("a: 5\n");
                let (_, rc) = w.oo("one", &["evolve", "--pin", "--grant", "pin", &f]);
                let (o, _) = w.oo("one", &["commit", "-m", "p", "--grant", "pin"]);
                format!("e{rc} {}", said(&o))
            }, "e0 landed");
            check(&mut bad, "g1 recipe: _.b", w.eval_in("two", "_.b"), "6");
            bad
        }),
        // Roots that do not move: a top-level effect cell, a record field
        // v0.125.0 already captured, a pure record.
        Box::new(|| {
            let mut bad = Vec::new();
            for (tag, src, want) in [
                ("g1-top", "k: h \"Q126\"", ROOT_TOP),
                ("g1-held", "q: { a: h \"Q126\" }", ROOT_HELD),
                ("g1-pure", "q: { a: 1, b: a + 1 }", ROOT_PURE),
            ] {
                let w = Ws::new(tag);
                check(&mut bad, &format!("{tag}: write"), w.write(&format!("{H}\n{src}\n")), "e0 landed");
                check(&mut bad, &format!("{tag}: root"), w.root(), want);
            }
            bad
        }),
    ];
    let n = cells(&mut bad, run);
    verdict(bad, n);
}

// v0.125.0, written with `h` above under Q126=one.
const ROOT_TOP: &str = "847de8cc37a868b9418435a481ab108bc35c5a4c609507975b4a93b83e3b44da";
const ROOT_HELD: &str = "912f740bce71f62a3d366004ddeb2b56d7d183f6a2fdb3ca531908a8abb9b0e8";
const ROOT_PURE: &str = "db1c41de6c65bd0d1720d0bd38d63c2895f066edd02f4e0d1c09584d894b4340";
