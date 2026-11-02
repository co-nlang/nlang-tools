// A record carried in every field.
// Order: nlang-tools/docs/a_record_carried_in_every_field_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-118 (D126).  A field of a record
// that is not forced yet is a thunk whose closure frame is the record. Its
// identity wrote that frame out in full, so a record of N fields hashed N
// frames of N fields. D126: a frame enters a thunk's identity by its digest.
// Writing a record of N fields costs N; a store declaring encoding 10 or less
// keeps the v0.118.0 rule, and its roots do not move.
//
// ── What is wrong (v0.118.0) ────────────────────────────────────────────
//
//   `h: { k0: 1, …, k1999: 1 }` evolves in 7.65 s and commits in 13.86 s;
//   2000 fields that read one field, or an outer coordinate, end the commit
//   (signal 6, 4 GB). 98% of an evolve is the identity of the unforced
//   record; the commit also copies each shared frame once per field
//   (`Arc::make_mut`), and reads the record back comparing each field's
//   frame to it, field by field.
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
        let scratch = nlang_interpreter::ScratchDir::new(&format!("every-field-{tag}"));
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


/// A workspace whose `.oo/` is a copy of a store a real `oo v0.118.0`
/// wrote (tests/fixtures/v118_framed_repo; its README says how).
fn fixture_ws(tag: &str) -> Ws {
    let w = Ws::new(tag);
    copy_fixture(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/v118_framed_repo/oo_dir"),
        &w.ws.join(".oo"),
    );
    w
}

fn copy_fixture(from: &std::path::Path, to: &std::path::Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            copy_fixture(&p, &to.join(e.file_name()));
        } else {
            fs::copy(&p, to.join(e.file_name())).unwrap();
        }
    }
}

fn declared(w: &Ws) -> String {
    fs::read_to_string(w.ws.join(".oo").join("objects.format")).unwrap_or_default().trim().to_string()
}

fn record(n: usize, field: impl Fn(usize) -> String) -> String {
    list_of(n, field)
}

/// r1 — Writing a record of N fields costs what the record holds: four
/// times the fields in less than eight times the time, the larger within
/// 30 s — fields written as values, fields that read one field, fields that
/// read an outer coordinate, fields that are records reading the outer
/// one, local fields read by data fields, a list of records whose fields
/// read each other, a table of N rows.
#[test]
fn r1_a_record_is_written_in_its_fields() {
    let mut bad = Vec::new();
    let shapes: Vec<(&str, Box<dyn Fn(usize) -> Vec<String>>)> = vec![
        ("values", Box::new(|n| vec![format!("h: {{ {} }}\n", record(n, |i| format!("k{i}: 1")))])),
        ("all read one", Box::new(|n| vec![format!("h: {{ k0: 1, {} }}\n", record(n - 1, |i| format!("k{}: k0 + {i}", i + 1)))])),
        ("an outer coordinate", Box::new(|n| vec![format!("a: 5\nh: {{ {} }}\n", record(n, |i| format!("k{i}: a")))])),
        ("records reading the outer", Box::new(|n| vec![format!("h: {{ a: 1, {} }}\n", record(n, |i| format!("k{i}: {{ b: a + {i} }}")))])),
        ("local fields", Box::new(|n| vec![format!("h: {{ {} }}\n", record(n, |i| format!("~p{i}: {i}, k{i}: ~p{i} + 1")))])),
        ("a list of records", Box::new(|n| vec![format!("l: [{}]\n", record(n, |i| format!("{{ a: {i}, b: a + 1 }}")))])),
        ("table rows", Box::new(|n| vec![format!("t: {{ {}, _: 0 }}\n", record(n, |i| format!("@{{ {i} }}: {i} * 2")))])),
    ];
    let k = shapes.len();
    for (label, files) in &shapes {
        scale(&mut bad, label, files.as_ref(), 1000);
    }
    verdict(bad, k);
}

/// r2 — A store this engine creates declares encoding 11, and a root that
/// holds an unforced field has the D126 address: the frame by its digest.
/// The readings are v0.118.0's.
#[test]
fn r2_a_frame_enters_by_its_digest() {
    let mut bad = Vec::new();
    let cells: [(&str, &str, &str, &str, &str); 7] = [
        ("a field reads a field", "h: { a: 1, b: a + 1 }\n", "_.h.b", "2", "2284caf43b29be83c7b90aee891418b43fcd91b8495819d95f70ff43f54a0e5e"),
        ("a field reads an outer coordinate", "a0: 5\no: { k: a0, m: k + 1 }\n", "_.o.m", "6", "fc03e3e585364246b608cb2c26e9db32c94c09d9e5d9ace096769edb49f18800"),
        ("a nested record reads the outer", "g: { a: 2, n: { c: a * 10 } }\n", "_.g.n.c", "20", "a1e136e8b87b76e53a4f17c4e0c6032f22b845fd1810980e1add98eb46608e4e"),
        ("a list of records", "l: [{ a: 3, b: a + 1 }, { a: 4, b: a + 1 }]\n", "_.l.1.b", "5", "00310e1e6e2b23ff09c61ecfb833e8e1f51b654a1352f5de6172b9fcf22578bb"),
        ("a table body that needs its input", "t: { @{ @int }: $ + 1, _: 0 }\n", "_.t 4", "5", "a62063f562592ec83a118eca735c5e73a10a18c802a471647159ec89309316d6"),
        ("a local field read", "h: { ~p: 3, q: p * 2 }\n", "_.h.q", "6", "69826ee06463463325cb4dd5c65e4a0901e67bc94f6499f6442a8d9ceb93fd07"),
        ("an arrow reading a field", "h: { k: 2, f: (x -> x + k) }\n", "_.h.f 1", "3", "923979413249900d2d5c71694a8ba989f98637f2b3bedceb4d68988f5323e930"),
    ];
    let w = Ws::new("decl");
    check(&mut bad, "new store: commit", w.writes(&["x: 0\n"]), "e0 c0");
    check(&mut bad, "new store declares", declared(&w), "encoding=11");
    for (label, src, q, reading, root) in cells {
        let w = Ws::new("r2");
        check(&mut bad, &format!("{label}: commit"), w.writes(&[src]), "e0 c0");
        check(&mut bad, &format!("{label}: {q}"), w.eval(q), reading);
        check(&mut bad, &format!("{label}: root"), w.root(), root);
    }
    verdict(bad, 2 + cells.len() * 3);
}

/// r3 — A store `oo v0.118.0` wrote (encoding 10) keeps the v0.118.0 rule:
/// it reads as it did, a later write lands on the root v0.118.0 itself
/// writes there (measured with the v0.118.0 binary on a copy of the
/// fixture), and the declaration does not move. `oo migrate` advances it
/// to 11 and says whom that locks out; after it, the old readings hold and
/// a write takes the new rule.
#[test]
fn r3_an_old_store_keeps_its_rule_until_migrate() {
    let mut bad = Vec::new();
    let w = fixture_ws("old");
    for (q, want) in [("_.h.b", "2"), ("_.g.n.c", "20"), ("_.o.m", "6"), ("_.t 4", "5"), ("_.l.1.b", "5"), ("_.hf.w", "2")] {
        check(&mut bad, &format!("as v0.118.0 committed it: {q}"), w.eval(q), want);
    }
    check(&mut bad, "a later write", w.writes(&["z: 3\nh2: { a: 7, b: a + z }\nh: { c: b * 2 }\n"]), "e0 c0");
    check(&mut bad, "the root v0.118.0 writes", w.root(), "45f1d436d364f9237184252c7b667c51cf6ab422c9a00c12d0418c2db421cae9");
    check(&mut bad, "still declares", declared(&w), "encoding=10");
    for (q, want) in [("_.h.c", "4"), ("_.h2.b", "10"), ("_.t 9", "10")] {
        check(&mut bad, &format!("after the write: {q}"), w.eval(q), want);
    }
    let (o, rc) = w.oo(&["migrate", "--grant", "migrate"]);
    check(&mut bad, "migrate", format!("rc={rc}"), "rc=0");
    check(&mut bad, "migrate names whom it locks out", o.contains("unopenable by oo v0.101.0 through v0.118.0").to_string(), "true");
    check(&mut bad, "migrated declares", declared(&w), "encoding=11");
    for (q, want) in [("_.h.c", "4"), ("_.h.b", "2"), ("_.g.n.c", "20"), ("_.hf.w", "2")] {
        check(&mut bad, &format!("after migrate: {q}"), w.eval(q), want);
    }
    check(&mut bad, "a write after migrate", w.writes(&["m: { a: 1, b: a + 2 }\n"]), "e0 c0");
    check(&mut bad, "after migrate: _.m.b", w.eval("_.m.b"), "3");
    check(&mut bad, "the new rule's root", w.root(), "337399c9305686b77fb2b9e00c57b54fab8fc5416334ec8ca0173d358c79bbd3");
    verdict(bad, 22);
}

/// r4 — A store `oo v0.118.0` wrote takes a table of 800 rows and reads it:
/// the read checks the root by the old rule and rewrites the keys of what
/// it holds for the rule in force, once per frame, not once per row.
#[test]
fn r4_an_old_store_reads_a_large_table() {
    let mut bad = Vec::new();
    let w = fixture_ws("big");
    let t = format!("t2: {{ {}, _: 0 }}\n", record(800, |i| format!("@{{ {i} }}: {i} * 2")));
    check(&mut bad, "800 rows", w.writes(&[&t]), "e0 c0");
    check(&mut bad, "still declares", declared(&w), "encoding=10");
    check(&mut bad, "_.t2 5", w.eval("_.t2 5"), "10");
    check(&mut bad, "_.h.b", w.eval("_.h.b"), "2");
    verdict(bad, 4);
}

/// g1 — A root that holds no unforced field does not move: values, a sum
/// the engine kept, an arrow, a record of values, a list, a table whose
/// bodies are values, a union, a range, fifty fields (v0.118.0).
#[test]
fn g1_roots_without_a_frame_do_not_move() {
    let mut bad = Vec::new();
    let fifty = format!("h: {{ {} }}\n", record(50, |i| format!("k{i}: {i}")));
    let cells: Vec<(String, &str)> = vec![
        ("x: 0\n".into(), "b74d775816f6b5be6cc152577b7556f6c8fdcc63433b0e31068cc2484a94ae0f"),
        ("v: 1 + 1\n".into(), "df91a8ad239ea8727c3a03a71eb4b0a787312b92306db4d8b124a73faaf40199"),
        ("f: (x -> x + 1)\n".into(), "258a1a08adfd0340cdbe103e4f4851bea107a67e298e72d6679badc0595bfd1f"),
        ("h: { a: 1, b: 2 }\n".into(), "3525d353584d8f2523cedad36edec0385aa208f09003f8c6db60ea5d327e0006"),
        ("l: [1, 2, 3]\n".into(), "373c70e8c0a2931942915d9c0846fdbe2de333d8bb021e72094e2a842df15034"),
        ("t: { @{ @int }: 1, _: 0 }\n".into(), "b6130220f9d82a6d1a5686b41c622853a844aa649ac056e81c4871b525c3e582"),
        ("s: \"x\"\nn: 1.5\nb: #true\n".into(), "ea02b5fa383b7db5fa6bc007b6378658b09ac4f70808d4249d25837aee6eb99a"),
        (fifty, "362d479593607119717a48b0fb9d5e0dadde1219c71a0db197c60e19156e0f7d"),
        ("u: 1 | 2\n".into(), "e27d71bd0701945f240974f803c84b9f0868939a866cb5449dfd044a65e3678a"),
        ("r: 1..3\n".into(), "06254b6688a8cb61d2baa10793b1d2ae56815c87812b535fd71ae30f78acbc6d"),
    ];
    for (src, root) in &cells {
        check(&mut bad, &format!("{}: root", src.trim().chars().take(40).collect::<String>()), root_of(src), root);
    }
    verdict(bad, cells.len());
}

/// g2 — One value, one address, under either rule: a record whose field
/// reads a field, written in one file, in two files, in two commits, lands
/// on one root and one reading; `=` answers as it did (v0.118.0).
#[test]
fn g2_one_value_one_address() {
    let mut bad = Vec::new();
    for (w1, w2, q, want) in [
        ("h: { a: 1, b: a + 1 }", "z: 9", "_.h.b", "2"),
        ("a0: 5", "o: { k: a0, m: k + 1 }", "_.o.m", "6"),
        ("g: { a: 2, n: { c: a * 10 } }", "z: 9", "_.g.n.c", "20"),
        ("l: [{ a: 3, b: a + 1 }]", "z: 9", "_.l.0.b", "4"),
    ] {
        let (got, roots) = three_ways(w1, w2, q);
        for g in &got {
            check(&mut bad, &format!("{w1} / {w2}: reading"), g.rsplit(" | ").next().unwrap_or("").to_string(), want);
        }
        check(&mut bad, &format!("{w1} / {w2}: three ways, one root"), (roots[0] == roots[1] && roots[1] == roots[2] && !roots[0].is_empty()).to_string(), "true");
    }
    table(&mut bad, &[
        ("{ a: 1, b: a + 1 } = { a: 1, b: a + 1 }", "#true"),
        ("{ a: 1, b: a + 1 } = { a: 2, b: a + 1 }", "#false"),
        ("{ a: 1, b: a + 1 }.b = 2", "#true"),
        ("({ a: 1, b: a + 1 } & { a: 1 }).b", "2"),
    ]);
    verdict(bad, 4 * 4 + 4);
}
