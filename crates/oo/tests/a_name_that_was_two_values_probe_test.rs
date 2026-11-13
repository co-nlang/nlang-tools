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

// ── r6 (R-1) ── a name for a record is one record ────────────────────────
//
// A name whose value is a record, read twice in one observation, is one
// record: its fields are observed once. The record may be written in place,
// nested in another, made in a call, or reached through another name. Two
// applications are still two (D109): `r: f 1` and `s: f 1` are two records,
// and `(f 1).a` read twice applies `f` twice.
//
// v0.125.0 and the first reference: `{ r: { a: ~%Math./random () }, e: r.a
// = r.a }.e` is `#false`; committing `q: { r: { a: h "Q126" }, m: [r.a,
// r.a] }` appends three bytes.

#[test]
fn r6_a_name_for_a_record_is_one_record() {
    let mut bad = Vec::new();
    let mut run: Vec<Box<dyn Fn() -> Vec<String> + Send + Sync>> = vec![
        kept("r6-nested", "q: { r: { a: h \"Q126\" }, m: [r.a, r.a] }", "_.q.m 1", "\"one\""),
        kept("r6-nested-arrow", "q: { r: { a: (s -> h s) \"Q126\" }, m: [r.a, r.a] }", "_.q.m 0", "\"one\""),
        kept("r6-made-in-call", "q: { f: (x -> { a: h x }), r: f \"Q126\", m: [r.a, r.a] }", "_.q.m 1", "\"one\""),
        kept("r6-top-call", "f: x -> { a: h x }\nr: f \"Q126\"\nm: [r.a, r.a]", "_.m 1", "\"one\""),
        // The reader of §6 of the order: a top-level alias of a nested field
        // that reads its sibling.
        kept("r6-alias-nested", "k: q.r.b\nq: { r: { a: (s -> h s) \"Q126\", b: a } }", "_.k", "\"one\""),
        one_value("r6-random-nested", "q: { r: { a: ~%Math./random () }, e: r.a = r.a }", &[("_.q.e", "#true")]),
        one_value("r6-random-arrow", "q: { r: { a: (s -> ~%Math./random ()) 1 }, e: r.a = r.a }", &[("_.q.e", "#true")]),
        one_value("r6-random-call", "q: { f: (x -> { a: ~%Math./random () }), r: f 1, e: r.a = r.a }", &[("_.q.e", "#true")]),
        one_value(
            "r6-random-alias",
            "k: q.r.b\nq: { r: { a: (s -> ~%Math./random ()) 1, b: a } }",
            &[("_.k = _.q.r.b", "#true"), ("_.q.r.a = _.q.r.b", "#true")],
        ),
    ];
    // In one `oo eval`, with no history.
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        let w = Ws::new("r6-eval");
        for (e, want) in [
            ("{ r: { a: ~%Math./random () }, e: r.a = r.a }.e", "#true"),
            ("{ r: { a: ~%Math./random () }, e: r = r }.e", "#true"),
            ("{ r: { a: ~%Math./random (), b: a }, e: r.a = r.b }.e", "#true"),
            ("{ r: { a: ~%Math./random () }, s: r, e: r.a = s.a }.e", "#true"),
            ("{ r: { s: { a: ~%Math./random () } }, e: r.s.a = r.s.a }.e", "#true"),
            ("{ r: [{ a: ~%Math./random () }], e: (r 0).a = (r 0).a }.e", "#true"),
            ("{ f: x -> { a: ~%Math./random () }, r: f 1, e: r.a = r.a }.e", "#true"),
            ("{ r: 1 |> (x -> { a: ~%Math./random () }), e: r.a = r.a }.e", "#true"),
            ("{ g: x -> { r: { a: ~%Math./random () }, e: r.a = r.a }, k: (g 1).e }.k", "#true"),
            // (the effect notes dropped leave a space before each comma)
            ("~%List./map ((x -> { r: { a: ~%Math./random () }, e: r.a = r.a }.e), [1, 2, 3])", "[#true , #true , #true ]"),
        ] {
            check(&mut bad, &format!("r6-eval: {e}"), w.eval_in("two", e), want);
        }
        bad
    }));
    // Bytes one `oo eval` appends.
    for (i, (e, want)) in [
        (format!("{{ r: {{ a: {A} }}, m: [r.a, r.a] }}.m"), "1"),
        (format!("{{ f: (x -> {{ a: {A} }}), r: f 1, m: [r.a, r.a] }}.m"), "1"),
        (format!("{{ r: (x -> {{ a: {A} }}) 1, m: [r.a, r.a] }}.m"), "1"),
        // The other half (D109): two applications are two records.
        (format!("{{ f: (x -> {{ a: {A} }}), r: f 1, s: f 1, m: [r.a, s.a, r.a] }}.m"), "2"),
        (format!("{{ f: (x -> {{ a: {A} }}), m: [(f 1).a, (f 1).a] }}.m"), "2"),
        (format!("{{ f: (x -> {{ g: (y -> {A}) }}), r: f 1, m: [r.g 1, r.g 1] }}.m"), "2"),
        ("{ q: { a: ~%Io./append_file (\"a.log\", $) }, m: [\"Z\" |> q.a, \"Z\" |> q.a] }.m".to_string(), "2"),
        (format!("{{ f: (x -> {{ a: {A} }}), m: ~%List./map ((x -> (f x).a), [1, 2, 3]) }}.m"), "3"),
    ]
    .into_iter()
    .enumerate()
    {
        run.push(Box::new(move || {
            let mut bad = Vec::new();
            check(&mut bad, &format!("r6: {e}"), appended(&format!("r6-{i}"), &e), want);
            bad
        }));
    }
    // The other half at commit: two applications at the top level are two
    // records; reads do no more.
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        let w = Ws::new("r6-two-apps");
        check(&mut bad, "r6 two apps: write", w.write(&format!("{H}\nf: x -> {{ a: h \"Q126\" }}\nm: [(f 1).a, (f 1).a]\n")), "e0 landed");
        check(&mut bad, "r6 two apps: bytes after commit", w.bytes().to_string(), "2");
        check(&mut bad, "r6 two apps: _.m 1", w.eval_in("two", "_.m 1"), "\"one\"");
        check(&mut bad, "r6 two apps: bytes after a read", w.bytes().to_string(), "2");
        bad
    }));
    // The other half: two writes that meet in one evolve; a nested record
    // read by name after the meet reads the met record.
    for (i, (s1, s2, rd, want)) in [
        ("q: { a: @int, r: { x: a }, e: r.x }", "q: { a: 1 }", "_.q.e", "1"),
        ("q: { r: { x: 1 }, e: r }", "q: { r: { y: 2 } }", "_.q.e.y", "2"),
        ("q: { r: { x: 1 } }\nk: q.r", "q: { r: { y: 2 } }", "_.k.y", "2"),
    ]
    .into_iter()
    .enumerate()
    {
        run.push(Box::new(move || {
            let mut bad = Vec::new();
            let w = Ws::new(&format!("r6-meet-{i}"));
            let f1 = w.file(&format!("{s1}\n"));
            let f2 = w.file(&format!("{s2}\n"));
            let (o, rc) = w.oo("one", &["evolve", &f1, &f2]);
            check(&mut bad, &format!("r6 meet {i}: evolve"), if rc == 0 { "e0".into() } else { squash(&o) }, "e0");
            let (o, _) = w.oo("one", &["commit", "-m", "c"]);
            check(&mut bad, &format!("r6 meet {i}: commit"), said(&o), "landed");
            check(&mut bad, &format!("r6 meet {i}: {rd}"), w.eval_in("two", rd), want);
            bad
        }));
    }
    let n = cells(&mut bad, run);
    verdict(bad, n);
}

// ── r7 ── an answer for an input is that application's; a name read in a
//          function's body is the name ──

/// Appends one byte to `a.log`, then gives back `s`.
const G: &str = "(s -> ~%Io./append_file (\"a.log\", \"Z\") |> (_ -> s))";

#[test]
fn r7_an_answer_belongs_to_the_input_it_read() {
    let mut bad = Vec::new();
    let mut run: Vec<Box<dyn Fn() -> Vec<String> + Send + Sync>> = vec![
        // A name read in a function's body, the function applied twice.
        kept("r7-body", "q: { a: h \"Q126\", f: (x -> a), m: [f 1, f 2] }", "_.q.m 1", "\"one\""),
        kept("r7-body-and-direct", "q: { a: h \"Q126\", f: (x -> a), m: [a, f 1] }", "_.q.m 1", "\"one\""),
        one_value("r7-random-body", "q: { a: ~%Math./random (), f: (x -> a), e: f 1 = f 2 }", &[("_.q.e", "#true")]),
    ];
    // An open term read under two inputs: each input its own answer.
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        let w = Ws::new("r7-eval");
        for (e, want) in [
            (format!("{{ g: {G}, q: {{ a: g $ }}, m: [\"x\" |> q.a, \"y\" |> q.a] }}.m"), "[\"x\", \"y\"]"),
            (format!("{{ g: {G}, a: g $, m: [\"x\" |> a, \"y\" |> a] }}.m"), "[\"x\", \"y\"]"),
            (format!("{{ g: {G}, a: g $, b: a, m: [\"x\" |> (_ -> b), \"y\" |> (_ -> b)] }}.m"), "[\"x\" , \"y\" ]"),
            (format!("{{ g: {G}, q: {{ a: g $, b: a }}, m: [\"x\" |> (_ -> q.b), \"y\" |> (_ -> q.b)] }}.m"), "[\"x\" , \"y\" ]"),
            (format!("{{ g: {G}, r: {{ s: {{ a: g $ }} }}, m: [\"x\" |> (_ -> r.s.a), \"y\" |> (_ -> r.s.a)] }}.m"), "[\"x\" , \"y\" ]"),
            (format!("{{ g: {G}, q: {{ a: g $ }}, f: (s -> q.a), m: [f \"x\", f \"y\"] }}.m"), "[\"x\" , \"y\" ]"),
            (format!("{{ g: {G}, a: g $, f: (s -> a), m: [f \"x\", f \"y\"] }}.m"), "[\"x\" , \"y\" ]"),
            (format!("{{ g: {G}, p: $, a: g p, m: [\"x\" |> (_ -> a), \"y\" |> (_ -> a)] }}.m"), "[\"x\" , \"y\" ]"),
            (format!("{{ g: {G}, p: $, a: g p, m: [\"x\" |> (_ -> p), \"x\" |> (_ -> a), \"y\" |> (_ -> a)] }}.m"), "[\"x\", \"x\" , \"y\" ]"),
            (format!("{{ g: {G}, q: {{ a: g $ }}, k: \"x\" |> (_ -> q), m: [k.a, \"y\" |> (_ -> q.a)] }}.m"), "[\"x\" , \"y\" ]"),
            (format!("{{ g: {G}, a: g $, m: \"x\" |> (_ -> [a, \"y\" |> (_ -> a), a]) }}.m"), "[\"x\" , \"y\" , \"x\" ]"),
            // Two records a call made, each reading the open name under its own input.
            (format!("{{ g: {G}, a: g $, mk: (y -> {{ b: a }}), r: mk \"x\", s: mk \"y\", m: [r.b, s.b] }}.m"), "[\"x\" , \"y\" ]"),
            // A record read with no input, then bound; the name still reads its own.
            (format!("{{ g: {G}, q: {{ a: g $ }}, w: q = q, k: \"y\" |> q, m: [w, \"x\" |> (_ -> [k.a, q.a]), \"z\" |> (_ -> q.a)] }}.m"), "[#true, [\"y\" , \"x\" ], \"z\" ]"),
            // Names read in a body, with no effect: the values.
            ("{ a: 10, f: x -> a + x, m: [f 1, f 2] }.m".to_string(), "[11, 12]"),
            ("{ a: ~%Math./random (), f: x -> a, g: x -> a, e: f 1 = g 2 }.e".to_string(), "#true"),
            ("{ a: ~%Math./random (), f: x -> a, e: a = f 1 }.e".to_string(), "#true"),
        ] {
            check(&mut bad, &format!("r7-eval: {e}"), w.eval_in("two", &e), want);
        }
        bad
    }));
    // Bytes one `oo eval` appends.
    for (i, (e, want)) in [
        // A name read in a body is one name, however often the body runs.
        (format!("{{ a: {A}, f: (x -> a), m: [f 1, f 2] }}.m"), "1"),
        (format!("{{ a: {A}, f: ((x @int) -> a), m: [f 1, f 2] }}.m"), "1"),
        (format!("{{ a: {A}, m: ~%List./map ((x -> a), [1, 2, 3]) }}.m"), "1"),
        (format!("{{ a: {A}, f: (x -> (y -> a)), m: [(f 1) 2, (f 3) 4] }}.m"), "1"),
        (format!("{{ a: {A}, q: {{ f: (x -> a), m: [f 1, f 2] }} }}.q.m"), "1"),
        (format!("{{ a: {A}, f: (x -> a), m: [1 |> f, 2 |> f] }}.m"), "1"),
        (format!("{{ mk: (y -> {{ a: {A}, g: (x -> a) }}), r: mk 1, m: [r.g 1, r.g 2] }}.m"), "1"),
        (format!("{{ r: {{ a: {A} }}, f: (x -> [r.a, r.a]), m: f 1 }}.m"), "1"),
        // The other half (D109): what a body does, each application does.
        (format!("{{ f: (x -> {A}), m: [f 1, f 2] }}.m"), "2"),
        (format!("{{ f: (x -> {{ b: {A}, c: [b, b] }}.c), m: [f 1, f 2] }}.m"), "2"),
        (format!("{{ mk: (y -> {{ a: {A}, g: (x -> a) }}), r: mk 1, s: mk 2, m: [r.g 1, s.g 1, r.g 2] }}.m"), "2"),
        // An open term: each application binds it, and does it.
        ("{ a: ~%Io./append_file (\"a.log\", $), m: [\"Z\" |> a, \"Z\" |> a] }.m".to_string(), "2"),
        ("{ a: ~%Io./append_file (\"a.log\", $), f: (x -> (\"Z\" |> a)), m: [f 1, f 2] }.m".to_string(), "2"),
        ("{ a: ~%Io./append_file (\"a.log\", $), b: a, m: [\"Z\" |> (_ -> b), \"Z\" |> (_ -> b)] }.m".to_string(), "2"),
        // A field carrying its input is one field of its record.
        (format!("{{ mk: (y -> {{ a: ~%Io./append_file (\"a.log\", $), b: a }}), r: mk \"Z\", s: mk \"Z\", m: [r.b, s.b, r.a, s.a] }}.m"), "2"),
        (format!("{{ g: {G}, q: {{ a: g $ }}, w: q = q, k: \"y\" |> q, m: [w, k.a, k.a] }}.m"), "1"),
        ("{ q: { a: ~%Io./append_file (\"a.log\", $) }, m: \"Z\" |> (_ -> [q.a, q.a]) }.m".to_string(), "1"),
        // One application reads it twice: once.
        ("{ a: ~%Io./append_file (\"a.log\", $), m: \"Z\" |> (_ -> [a, a]) }.m".to_string(), "1"),
    ]
    .into_iter()
    .enumerate()
    {
        run.push(Box::new(move || {
            let mut bad = Vec::new();
            check(&mut bad, &format!("r7: {e}"), appended(&format!("r7-{i}"), &e), want);
            bad
        }));
    }
    // A record holding an open term stays a recipe in the history; read
    // under two inputs there, each input has its own answer.
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        let w = Ws::new("r7-commit");
        check(&mut bad, "r7 commit: write", w.write(&format!("q: {{ g: {G}, t: {{ a: g $ }}, m: [\"x\" |> t.a, \"y\" |> t.a] }}\n")), "e0 landed");
        check(&mut bad, "r7 commit: _.q.m", w.eval_in("two", "_.q.m"), "[\"x\", \"y\"]");
        check(&mut bad, "r7 commit: _.q.m, read again", w.eval_in("two", "_.q.m"), "[\"x\", \"y\"]");
        bad
    }));
    let n = cells(&mut bad, run);
    verdict(bad, n);
}

/// A table with a default branch: `t 0` is 0, anything else does the body.
const T: &str = "@{ 0 }: 0, _";

#[test]
fn r8_a_table_is_a_function_too() {
    let mut bad = Vec::new();
    let mut run: Vec<Box<dyn Fn() -> Vec<String> + Send + Sync>> = vec![
        // A name read in a branch's body, the branch chosen twice, and read directly.
        kept("r8-branch", "q: { a: h \"Q126\", t: { @{ 0 }: 0, _: a }, m: [t 1, t 2, a] }", "_.q.m 1", "\"one\""),
        one_value("r8-random-branch", "q: { a: ~%Math./random (), t: { @{ 0 }: 0, _: a }, e: [t 1 = t 2, t 1 = a] }", &[("_.q.e", "[#true , #true ]")]),
    ];
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        let w = Ws::new("r8-eval");
        let r = "~%Math./random ()";
        for (e, want) in [
            // The name a branch reads is the name: against a direct read, another
            // branch, another table, an arrow, a pipe, a table made in a body.
            (format!("{{ a: {r}, t: {{ {T}: a }}, e: [t 1 = t 2, t 1 = a, a = t 1] }}.e"), "[#true , #true , #true ]"),
            (format!("{{ a: {r}, t: {{ @{{ 0 }}: a, _: a }}, e: [t 0 = t 1, t 0 = a, t 1 = a] }}.e"), "[#true , #true , #true ]"),
            (format!("{{ a: {r}, t: {{ {T}: a }}, f: x -> a, e: [t 1 = f 1, f 1 = t 1] }}.e"), "[#true , #true ]"),
            (format!("{{ a: {r}, t: {{ {T}: a }}, u: {{ {T}: a }}, e: t 1 = u 1 }}.e"), "#true"),
            (format!("{{ a: {r}, b: a, t: {{ {T}: b }}, e: [t 1 = a, t 1 = b] }}.e"), "[#true , #true ]"),
            (format!("{{ a: {r}, t: {{ {T}: a }}, e: [(1 |> t) = (2 |> t), (1 |> t) = a] }}.e"), "[#true , #true ]"),
            (format!("{{ a: {r}, f: x -> {{ {T}: a }} x, e: [f 1 = f 2, f 1 = a] }}.e"), "[#true , #true ]"),
            // A record a call made: its table reads its own field; two calls, two records.
            (format!("{{ mk: x -> {{ a: {r}, t: {{ {T}: a }} }}, r: mk 1, s: mk 1, e: [r.t 1 = r.t 2, r.t 1 = r.a, r.t 1 = s.t 1] }}.e"), "[#true , #true , #false ]"),
            // A branch read by navigation is a coordinate: read twice, one value.
            (format!("{{ t: {{ @{{ 0 }}: 0, k: {r} }}, e: t.k = t.k }}.e"), "#true"),
            (format!("{{ a: {r}, t: {{ @{{ 0 }}: 0, k: a }}, e: [t.k = a, t.k = t.k] }}.e"), "[#true , #true ]"),
            // An open name read in a branch: each input its own answer.
            (format!("{{ g: {G}, a: g $, t: {{ {T}: a }}, m: [t \"x\", t \"y\"] }}.m"), "[\"x\" , \"y\" ]"),
            // The other half: what a branch's body makes is made each time.
            (format!("{{ t: {{ {T}: {r} }}, e: t 1 = t 1 }}.e"), "#false"),
            (format!("{{ t: {{ {T}: {{ a: {r}, e: a = a }} }}, e: [(t 1).e, (t 1).a = (t 1).a] }}.e"), "[#true , #false ]"),
            // No effect: the values.
            (format!("{{ a: 5, t: {{ {T}: a + $ }}, m: [t 1, t 2, t 1, t 0] }}.m"), "[6, 7, 6, 0]"),
            (format!("{{ a: $ + 1, t: {{ {T}: a }}, m: [t 1, t 5, t 1] }}.m"), "[2, 6, 2]"),
            (format!("{{ mk: x -> {{ v: x, t: {{ {T}: v + $ }} }}, r: mk 1, s: mk 2, m: [r.t 10, s.t 10, r.t 20] }}.m"), "[11, 12, 21]"),
            (format!("{{ a: $ * 2, t: {{ {T}: a + $ }}, m: [10 |> (_ -> t 1), 20 |> (_ -> t 1)] }}.m"), "[3, 3]"),
            (format!("{{ f: {{ {T}: $ + f ($ - 1) }}, m: [f 3, f 4, f 3] }}.m"), "[6, 10, 6]"),
        ] {
            check(&mut bad, &format!("r8-eval: {e}"), w.eval_in("two", &e), want);
        }
        bad
    }));
    // Bytes one `oo eval` appends.
    for (i, (e, want)) in [
        (format!("{{ a: {A}, t: {{ {T}: a }}, m: [t 1, t 2, a] }}.m"), "1"),
        (format!("{{ a: {A}, t: {{ @{{ 0 }}: a, _: a }}, m: [t 0, t 1] }}.m"), "1"),
        (format!("{{ a: {A}, t: {{ {T}: a }}, f: (x -> a), m: [f 1, t 1, f 2] }}.m"), "1"),
        (format!("{{ g: {G}, a: g 1, f: {{ {T}: a + f ($ - 1) }}, m: f 3 }}.m"), "1"),
        (format!("{{ g: {G}, a: g 1, t: {{ {T}: a }}, u: {{ {T}: a + t $ }}, m: u 1 }}.m"), "1"),
        (format!("{{ g: {G}, mk: (x -> {{ a: g x, t: {{ {T}: a }} }}), r: mk \"x\", s: mk \"y\", m: [r.t 1, s.t 1, r.t 2, r.a] }}.m"), "2"),
        // Navigation reads the coordinate once; applying the branch does its body.
        (format!("{{ g: {G}, t: {{ @{{ 0 }}: 0, k: g \"x\" }}, m: [t.k, t.k] }}.m"), "1"),
        (format!("{{ g: {G}, t: {{ @{{ 0 }}: 0, k: g \"x\" }}, m: [t.k, t \"k\", t \"k\"] }}.m"), "3"),
        (format!("{{ g: {G}, a: g \"x\", t: {{ @{{ 0 }}: 0, k: a }}, m: [t.k, t \"k\", a] }}.m"), "1"),
        // An open name read in a branch: each application does it.
        (format!("{{ g: {G}, a: g $, t: {{ {T}: a }}, m: [t \"x\", t \"y\"] }}.m"), "2"),
        (format!("{{ g: {G}, a: g $, t: {{ {T}: a }}, m: [t \"x\", t \"x\"] }}.m"), "2"),
        // The other half (D109): what a branch's body does, each choice does.
        (format!("{{ g: {G}, t: {{ {T}: g $ }}, m: [t \"x\", t \"y\", t \"x\"] }}.m"), "3"),
        (format!("{{ g: {G}, t: {{ {T}: {{ a: g \"x\" }} }}, m: [(t 1).a, (t 1).a] }}.m"), "2"),
        (format!("{{ g: {G}, a: g \"o\", t: {{ {T}: [a, g $] }}, m: [t \"x\", t \"y\"] }}.m"), "3"),
        (format!("{{ g: {G}, f: {{ {T}: (g 1) + f ($ - 1) }}, m: f 3 }}.m"), "3"),
        (format!("{{ g: {G}, t: {{ {T}: g \"x\" }}, m: [t 1, t 1] }}.m"), "2"),
    ]
    .into_iter()
    .enumerate()
    {
        run.push(Box::new(move || {
            let mut bad = Vec::new();
            check(&mut bad, &format!("r8: {e}"), appended(&format!("r8-{i}"), &e), want);
            bad
        }));
    }
    let n = cells(&mut bad, run);
    verdict(bad, n);
}

// ── r9 ── two calls are two records; a function observed reads its node ──

#[test]
fn r9_a_function_observed_reads_its_node() {
    let mut bad = Vec::new();
    let mut run: Vec<Box<dyn Fn() -> Vec<String> + Send + Sync>> = vec![
        // A function stored beside the name it reads: the commit does the
        // effect once, and from history the function reads the field.
        kept("r9-stored-arrow", "q: { r: { a: h \"Q126\", f: (y -> a) } }", "[_.q.r.a, _.q.r.f 1]", "[\"one\" , \"one\" ]"),
        kept("r9-stored-table", "q: { r: { a: h \"Q126\", t: { @{ 0 }: 0, _: a } } }", "[_.q.r.a, _.q.r.t 1, _.q.r.t 2]", "[\"one\" , \"one\" , \"one\" ]"),
        // ... with the function that does the effect written beside them.
        kept(
            "r9-stored-beside",
            "q: { r: { g: (s -> ~%Io./append_file (\"a.log\", \"Z\") |> (_ -> ~%Env.get s)), a: g \"Q126\", f: (y -> a) } }",
            "[_.q.r.a, _.q.r.f 1]",
            "[\"one\" , \"one\" ]",
        ),
        one_value(
            "r9-random-nested",
            "q: { r: { a: ~%Math./random (), f: (y -> a), t: { @{ 0 }: 0, _: a }, u: { @{ 0 }: 0, k: a } } }",
            &[("[_.q.r.a = _.q.r.f 1, _.q.r.a = _.q.r.t 1, _.q.r.t 1 = _.q.r.t 2, _.q.r.u.k = _.q.r.a]", "[#true , #true , #true , #true ]")],
        ),
        one_value(
            "r9-random-flat",
            "q: { a: ~%Math./random (), t: { @{ 0 }: 0, _: a }, u: { @{ 0 }: 0, k: a } }",
            &[("[_.q.t 1 = _.q.a, _.q.t 1 = _.q.t 2, _.q.u.k = _.q.a]", "[#true , #true , #true ]")],
        ),
    ];
    // The other half: a stored function still follows a later write of the
    // name it reads, and a later write beside an answered name keeps it.
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        let w = Ws::new("r9-later");
        check(&mut bad, "r9-later: write", w.write("q: { r: { a: @int, f: (y -> a), t: { @{ 0 }: 0, _: a } } }\n"), "e0 landed");
        check(&mut bad, "r9-later: second write", w.write("q: { r: { a: 5 } }\n"), "e0 landed");
        check(&mut bad, "r9-later: read", w.eval_in("two", "[_.q.r.a, _.q.r.f 1, _.q.r.t 1]"), "[5, 5, 5]");
        let w = Ws::new("r9-beside");
        check(&mut bad, "r9-beside: write", w.write(&format!("{H}\nq: {{ r: {{ a: h \"Q126\", f: (y -> a) }} }}\n")), "e0 landed");
        check(&mut bad, "r9-beside: second write", w.write("q: { r: { b: 2 } }\n"), "e0 landed");
        check(&mut bad, "r9-beside: bytes", w.bytes().to_string(), "1");
        check(&mut bad, "r9-beside: read", w.eval_in("two", "[_.q.r.a, _.q.r.f 1, _.q.r.b]"), "[\"one\" , \"one\" , 2 ]");
        // A function printed from history: the reading does no effect.
        let w = Ws::new("r9-printed");
        check(&mut bad, "r9-printed: write", w.write(&format!("q: {{ g: {G}, a: g \"x\", f: (y -> a) }}\n")), "e0 landed");
        check(&mut bad, "r9-printed: bytes after commit", w.bytes().to_string(), "1");
        let _ = w.eval_in("two", "_.q.f");
        check(&mut bad, "r9-printed: bytes after printing f", w.bytes().to_string(), "1");
        bad
    }));
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        let w = Ws::new("r9-eval");
        let r = "~%Math./random ()";
        for (e, want) in [
            // One name met twice is one record.
            (format!("{{ r: {{ p: {r} }}, e: [(r & {{ q: 1 }}).p = (r & {{ q: 1 }}).p, (r & {{ q: 1 }}).p = r.p] }}.e"), "[#true , #true ]"),
            (format!("{{ f: (y -> {{ p: {r} }}), r: f 1, e: [(r & {{ q: 1 }}).p = (r & {{ q: 1 }}).p, (r & {{ q: 1 }}).p = r.p] }}.e"), "[#true , #true ]"),
            // No effect: the values.
            ("{ f: (y -> { p: y + 1 }), m: [f 1 & { q: 1 }, f 2 & { q: 1 }, f 1 & { q: 1 }] }.m".to_string(), "[{ p: 2 q: 1 }, { p: 3 q: 1 }, { p: 2 q: 1 }]"),
        ] {
            check(&mut bad, &format!("r9-eval: {e}"), w.eval_in("two", &e), want);
        }
        // Meeting again what was met costs what it cost: a 200 field record
        // a call made, met sixteen times, is still a value.
        let fields = (1..=200).map(|i| format!("k{i}: {i}")).collect::<Vec<_>>().join(", ");
        let list = (1..=16).map(|i| i.to_string()).collect::<Vec<_>>().join(", ");
        let want = format!("[{}]", (2..=17).map(|i| i.to_string()).collect::<Vec<_>>().join(", "));
        let e = format!("{{ f: (y -> {{ {fields} }}), m: ~%List./map ((x -> (f 1 & {{ q: 1 }}).k1 + x), [{list}]) }}.m");
        check(&mut bad, "r9-eval: a record met sixteen times", w.eval_in("two", &e), &want);
        bad
    }));
    // Bytes one `oo eval` appends.
    for (i, (e, want)) in [
        // Two calls are two records, met or not (D109).
        (format!("{{ g: {G}, f: (y -> {{ p: g \"P\" }}), m: [f 1 & {{ q: 1 }}, f 1 & {{ q: 1 }}] }}.m"), "2"),
        (format!("{{ g: {G}, t: {{ @{{ 1 }}: {{ p: g \"P\" }} }} & {{ @{{ 1 }}: {{ q: g \"Q\" }} }}, m: [t 1, t 1] }}.m"), "4"),
        (format!("{{ g: {G}, t: {{ @{{ 1 }}: {{ p: g \"P\" }} }} & {{ @{{ 1 }}: {{ q: g \"Q\" }} }}, m: [(t 1).p, (t 1).q, (t 1).p] }}.m"), "3"),
        (format!("{{ g: {G}, t: (y -> {{ p: g \"P\" }}) & (y -> {{ q: g \"Q\" }}), m: [t 1, t 1] }}.m"), "4"),
        (format!("{{ g: {G}, t: {{ @{{ 1 }}: {{ p: g \"P\" }} }} & {{ @{{ 1 }}: {{ q: g \"Q\" }} }}, m: [t 1] }}.m"), "2"),
        (format!("{{ g: {G}, f: (y -> {{ p: g \"P\" }}), h: (y -> {{ q: g \"Q\" }}), m: [(f 1) & (h 1), (f 1) & (h 1)] }}.m"), "4"),
        (format!("{{ t: {{ @{{ 1 }}: {{ p: {A} }} }} & {{ @{{ 1 }}: {{ q: {A} }} }}, m: [t 1, t 1] }}.m"), "4"),
        // The other half: one name met twice is one record.
        (format!("{{ g: {G}, r: {{ p: g \"P\" }}, m: [r & {{ q: 1 }}, r & {{ q: 1 }}, r.p] }}.m"), "1"),
        (format!("{{ g: {G}, f: (y -> {{ p: g \"P\" }}), r: f 1, m: [r & {{ q: 1 }}, r & {{ q: 1 }}, r.p] }}.m"), "1"),
        (format!("{{ g: {G}, f: (y -> {{ p: g \"P\" }}), s: f 1 & {{ q: 1 }}, m: [s, s, s.p] }}.m"), "1"),
        // A function observed reads the names it closes over on their node.
        (format!("{{ r: {{ a: {A}, f: (y -> a) }} }}.r"), "1"),
        (format!("{{ r: {{ a: {A}, t: {{ {T}: a }} }} }}.r"), "1"),
        (format!("{{ a: {A}, f: (y -> a) }}.f"), "1"),
        (format!("{{ a: {A}, t: {{ {T}: a }} }}.t"), "1"),
        (format!("{{ r: {{ a: {A}, f: (y -> a) }}, m: [r, r.f 1, r.a] }}.m"), "1"),
        (format!("{{ r: {{ a: {A}, f: (y -> z -> a) }}, m: [r, (r.f 1) 2] }}.m"), "1"),
        // The other half: observing a function does not do its body.
        (format!("{{ r: {{ f: (y -> {A}) }} }}.r"), "0"),
        (format!("{{ r: {{ t: {{ {T}: {A} }} }} }}.r"), "0"),
    ]
    .into_iter()
    .enumerate()
    {
        run.push(Box::new(move || {
            let mut bad = Vec::new();
            check(&mut bad, &format!("r9: {e}"), appended(&format!("r9-{i}"), &e), want);
            bad
        }));
    }
    let n = cells(&mut bad, run);
    verdict(bad, n);
}

// ── r10 ── functions that read each other; a list of calls (R-5) ──

/// "returned" when an observation printed a value.
fn returned(o: String) -> String {
    if o.starts_with('{') || o.starts_with('[') {
        "returned".into()
    } else {
        o.chars().take(40).collect()
    }
}

#[test]
fn r10_functions_that_read_each_other() {
    let mut bad = Vec::new();
    let mut run: Vec<Box<dyn Fn() -> Vec<String> + Send + Sync>> = Vec::new();
    // Functions that read each other are observed to the end: printed,
    // stored under a coordinate, read back and applied. One after another:
    // an engine without an end here takes 3 GB each.
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        let w = Ws::new("r10-even-odd");
        check(&mut bad, "r10-even-odd: write", w.write("q: { even: (n -> odd n), odd: (n -> even n), z: 1 }\n"), "e0 landed");
        check(&mut bad, "r10-even-odd: _.q.z", w.eval_in("two", "_.q.z"), "1");
        check(&mut bad, "r10-even-odd: root", w.root(), "e9fa0e2529d963213e4801ebeb36a86aa52e32b698fff263c3ed3a320c4d03d6");
        let w = Ws::new("r10-two-records");
        check(&mut bad, "r10-two-records: write", w.write("q: { r: { a: 1, f: (y -> s) }, s: { b: 2, h: (y -> r) } }\n"), "e0 landed");
        check(&mut bad, "r10-two-records: applied round", w.eval_in("two", "[((_.q.r.f 1).h 1).a, (((_.q.r.f 1).h 1).f 1).b]"), "[1, 2]");
        check(&mut bad, "r10-two-records: root", w.root(), "1a648628d6f677ee3f6517defd3b1774361f33e45d26c81a2574a3b79a1a8ba4");
        check(&mut bad, "r10-two-records: printed from history", returned(w.eval_in("two", "_.q.r")), "returned");
        let w = Ws::new("r10-printed");
        for e in [
            "{ even: (n -> odd n), odd: (n -> even n) }",
            "{ r: { f: (y -> s) }, s: { h: (y -> r) } }.r",
        ] {
            check(&mut bad, &format!("r10-printed: {e}"), returned(w.eval_in("two", e)), "returned");
        }
        // Each of two records that read each other does its effect once.
        let e = format!("{{ r: {{ a: {A}, f: (y -> s.b) }}, s: {{ b: {A}, h: (y -> r.a) }} }}");
        check(&mut bad, &format!("r10: {e}"), appended("r10-mutual", &e), "2");
        // N functions written in one node close over the same N names: the
        // node is read once, not once by each. 80 functions that read each
        // other, stored (v0.125.0: 3 s; each reading the node again: 75 s).
        let ring = (1..=80).map(|i| format!("f{i}: (y -> f{} y)", i % 80 + 1)).collect::<Vec<_>>().join(", ");
        let w = Ws::new("r10-eighty");
        let start = Instant::now();
        check(&mut bad, "r10-eighty: write", w.write(&format!("q: {{ {ring}, z: 1 }}\n")), "e0 landed");
        let took = start.elapsed().as_secs();
        check(&mut bad, "r10-eighty: seconds", if took < 30 { "under 30".into() } else { took.to_string() }, "under 30");
        check(&mut bad, "r10-eighty: _.q.z", w.eval_in("two", "_.q.z"), "1");
        // ... and 160 tables whose bodies call each other (each reading the
        // node again: 45 s at 160).
        let ring = (1..=160).map(|i| format!("t{i}: {{ {T}: t{} 0 }}", i % 160 + 1)).collect::<Vec<_>>().join(", ");
        let w = Ws::new("r10-tables");
        let start = Instant::now();
        check(&mut bad, "r10-tables: write", w.write(&format!("q: {{ {ring}, z: 1 }}\n")), "e0 landed");
        let took = start.elapsed().as_secs();
        check(&mut bad, "r10-tables: seconds", if took < 30 { "under 30".into() } else { took.to_string() }, "under 30");
        check(&mut bad, "r10-tables: _.q.z", w.eval_in("two", "_.q.z"), "1");
        bad
    }));
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        let w = Ws::new("r10-eval");
        let r = "~%Math./random ()";
        for (e, want) in [
            // Applied after being printed, as deep as it is applied.
            ("{ r: { a: 1, f: (y -> r) }, m: [(r.f 1).a, ((r.f 1).f 1).a, (((r.f 1).f 1).f 1).a] }.m".to_string(), "[1, 1, 1]"),
            ("{ r: { a: 1, f: (y -> s) }, s: { b: 2, h: (y -> r) }, m: [((r.f 1).h 1).a, (((r.f 1).h 1).f 1).b] }.m".to_string(), "[1, 2]"),
            // The name a function reads through its own record is that record's.
            (format!("{{ r: {{ a: {r}, f: (y -> r.a) }}, e: [r.f 1 = r.a, r.f 1 = r.f 2] }}.e"), "[#true , #true ]"),
            // No effect, no call: lists written alike meet as they did.
            ("[[1, 2] & [@int, @int], [1, 2] & [@int, @int]]".to_string(), "[[1, 2], [1, 2]]"),
            // Two captures of one node that hold different names are two views.
            (
                "{ r: { a: 1, b: 2, f: (y -> a), g: (y -> b) } }.r".to_string(),
                "{ a: 1 b: 2 f: {{ %kind: #logic %morphism: #true %rules: {{ y: {{ %closure: {{ 0: {} 1: { a: 1 } }} %code: a }} }} }} g: {{ %kind: #logic %morphism: #true %rules: {{ y: {{ %closure: {{ 0: {} 1: { b: 2 } }} %code: b }} }} }} }",
            ),
        ] {
            check(&mut bad, &format!("r10-eval: {e}"), w.eval_in("two", &e), want);
        }
        bad
    }));
    // Bytes one `oo eval` appends.
    for (i, (e, want)) in [
        // A function that reads its own record: the record's effect, once.
        (format!("{{ r: {{ a: {A}, f: (y -> r.a) }} }}"), "1"),
        (format!("{{ r: {{ a: {A}, f: (y -> r.a) }} }}.r"), "1"),
        (format!("{{ r: {{ a: {A}, f: (y -> a) }}, s: r & {{ b: 1 }}, m: [r, s] }}.m"), "1"),
        // Two calls are two records in a list too, met or not (D109).
        (format!("{{ g: {G}, f: (y -> {{ p: g \"P\" }}), m: [[f 1] & [{{ q: 1 }}], [f 1] & [{{ q: 1 }}]] }}.m"), "2"),
        (format!("{{ g: {G}, f: (y -> {{ p: g \"P\" }}), m: [{{ l: [f 1] }} & {{ l: [{{ q: 1 }}] }}, {{ l: [f 1] }} & {{ l: [{{ q: 1 }}] }}] }}.m"), "2"),
        (format!("{{ g: {G}, f: (y -> {{ p: g \"P\" }}), m: [[f 1], [f 1]] }}.m"), "2"),
        (format!("{{ g: {G}, f: (y -> {{ p: g \"P\" }}), m: [[[f 1]] & [[{{ q: 1 }}]], [[f 1]] & [[{{ q: 1 }}]]] }}.m"), "2"),
        // The other half: one name for a list, met twice, is one list.
        (format!("{{ g: {G}, f: (y -> {{ p: g \"P\" }}), l: [f 1], m: [l & [{{ q: 1 }}], l & [{{ q: 1 }}]] }}.m"), "1"),
    ]
    .into_iter()
    .enumerate()
    {
        run.push(Box::new(move || {
            let mut bad = Vec::new();
            check(&mut bad, &format!("r10: {e}"), appended(&format!("r10-{i}"), &e), want);
            bad
        }));
    }
    let n = cells(&mut bad, run);
    verdict(bad, n);
}
