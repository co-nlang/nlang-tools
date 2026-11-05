// A pin that erased its neighbours.
// Order: nlang-tools/docs/a_pin_that_erased_its_neighbours_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-124. Ruling D128 (甲, 2026-11-05):
// a pin overwrites only the leaves its value writes; every other coordinate
// under them — the siblings — meets as without `--pin`. SPEC_10 §2.2.3
// 「吸收……以座標為單位……不得抹去未被本次 `pin` 指名的座標」; §2.2.1 a
// coordinate is down to the leaf; SYNTAX_03 §2 #1 「賦值是把約束綁到座標」,
// #3 `a.b.c: v` ≡ `{a: {b: {c: v}}}` (so both spellings answer the same).
//
// ── What is wrong (v0.123.0 + Q-123) ────────────────────────────────────
//
//   `h: { k: 5, r: 6 }` committed, then `evolve --pin --grant pin` `h.k: 7`:
//   `h` becomes `{ k: 7 }` and `h.r` is gone; without `--pin` a write beside
//   `h.r` keeps it. The pin's unit is the top-level field `h`, so every
//   coordinate under `h` it did not name is erased: two levels down
//   (`a.b.c: 1` drops `a.b.d` and `a.e`), a recipe beside it, a list's other
//   elements (`l.1: 9` makes `l` the record `{ 1: 9 }`). Since Q-123 a pin
//   of the value HEAD holds (`h.k: 5`) lands too and erases `h.r`. Two pins
//   one after the other (`h.k: 7`, then `h.r: 9`) leave `{ r: 9 }`: the
//   second absorbs the whole of `h` from the first. Inside one pin file the
//   path keys replace each other in source order: `h.k: 7` and `h.r: 8`
//   give `{ r: 8 }` while `h: { k: 7, r: 8 }` gives both, and `h.k: 7` with
//   `h.k: 8` gives `8` where the same file without `--pin` is refused.
//
// The other half: what a pin writes at a leaf still overwrites (a scalar
// over a record, a record over a scalar, a type, a ⊥ repaired); a pin of
// what HEAD holds is held, through any spelling; the definitions a pin does
// not write are judged as before (Q-122); ordinary writes are unchanged; a
// pin's commit still needs the capability; small roots do not move.
//
// Every `oo` here runs under a 3 GB address-space cap and a kill inside
// the probe (90 s).
//
// The delivery may NOT edit this file. `rustfmt` must not touch it.

#![allow(dead_code)]

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const KILL_AFTER: Duration = Duration::from_secs(90);

struct Ws {
    _scratch: nlang_interpreter::ScratchDir,
    root: PathBuf,
    ws: PathBuf,
}

impl Ws {
    fn new(tag: &str) -> Self {
        let scratch = nlang_interpreter::ScratchDir::new(&format!("pin-leaves-{tag}"));
        let root = scratch.path().to_path_buf();
        let ws = root.join("ws");
        fs::create_dir_all(&ws).unwrap();
        Ws { _scratch: scratch, root, ws }
    }

    /// Run `oo` capped at 3 GB, killed after 90 s: (output, rc). A run
    /// that is killed or dies without a word says so.
    fn oo(&self, args: &[&str]) -> (String, i32) {
        let mut child = Command::new("sh")
            .arg("-c")
            .arg("ulimit -v 3000000; exec \"$0\" \"$@\"")
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
        // Drain both pipes while waiting: a root printed past the pipe's
        // buffer would otherwise block the child until the kill.
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

    /// Evolve `src` from a new file: (output, rc).
    fn evolve(&self, src: &str) -> (String, i32) {
        let n = fs::read_dir(&self.ws).map(|d| d.count()).unwrap_or(0);
        let f = format!("f{n}.n");
        fs::write(self.ws.join(&f), src).unwrap();
        self.oo(&["evolve", &f])
    }

    /// `oo commit`: (what it says before `Commit successful`, rc).
    fn commit(&self) -> (String, i32) {
        let (o, rc) = self.oo(&["commit", "-m", "a"]);
        let said = o.split("Commit successful").next().unwrap_or("").trim().to_string();
        (squash(&said), rc)
    }

    /// Evolve then commit `src`; "e0 c0" when both succeed.
    fn write(&self, src: &str) -> String {
        let (eo, er) = self.evolve(src);
        if eo == "DID NOT RETURN" || eo == "CRASHED" || eo == "PANIC" {
            return eo;
        }
        let (co, cr) = self.commit();
        if co == "DID NOT RETURN" || co == "CRASHED" || co == "PANIC" {
            return co;
        }
        format!("e{er} c{cr}")
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

    /// The committed root as `oo inspect` prints it.
    fn stored(&self) -> String {
        let head = fs::read_to_string(self.ws.join(".oo").join("HEAD")).unwrap_or_default();
        if head.trim().is_empty() {
            return String::new();
        }
        let (o, _) = self.oo(&["inspect", head.trim()]);
        let caid = o.lines().find(|l| l.starts_with("root:")).and_then(|l| l.split_whitespace().last()).unwrap_or("").to_string();
        self.oo(&["inspect", &caid]).0
    }

    /// The committed root object's bytes, as the store holds them.
    fn root_object(&self) -> String {
        let r = self.root();
        if r.len() < 3 {
            return String::new();
        }
        let p = self.ws.join(".oo").join("objects").join("sha256").join(&r[..2]).join(&r[2..]);
        fs::read_to_string(p).unwrap_or_default()
    }

    /// How many fields the committed root stores as a fuel horizon.
    fn stored_out_of_fuel(&self) -> usize {
        self.stored().matches("#fuel_exhausted").count()
    }

    /// The first line the committed root prints for `name`, cut to 12 bytes.
    fn stored_form(&self, name: &str) -> String {
        let s = self.stored();
        let key = format!("  {name}:");
        s.lines().find(|l| l.starts_with(&key)).map(|l| l.trim().chars().take(12).collect()).unwrap_or_default()
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
    for cause in ["#conflict", "#fuel_exhausted", "#max_depth_exceeded"] {
        if (got.starts_with("_|_") || got.starts_with("#blur")) && got.contains(cause) {
            return format!("{} {cause}", got.split_whitespace().next().unwrap_or(""));
        }
    }
    got.into()
}

fn fields(n: usize, f: impl Fn(usize) -> String) -> String {
    (0..n).map(f).collect::<Vec<_>>().join(", ")
}

/// `{ k0: 0, …, k{n-1}: n-1 }`
fn rec(n: usize) -> String {
    format!("{{ {} }}", fields(n, |i| format!("k{i}: {i}")))
}

const FUEL: &str = "#blur #fuel_exhausted";

/// Run each cell in its own workspace, four at a time; a cell pushes what
/// is wrong.
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




impl Ws {
    /// `oo evolve --pin --grant pin` from a new file: (output, rc).
    fn pin(&self, src: &str) -> (String, i32) {
        let n = fs::read_dir(&self.ws).map(|d| d.count()).unwrap_or(0);
        let f = format!("f{n}.n");
        fs::write(self.ws.join(&f), src).unwrap();
        self.oo(&["evolve", "--pin", "--grant", "pin", &f])
    }

    /// `oo commit --grant pin`: (output, rc).
    fn commit_pin(&self) -> (String, i32) {
        let (o, rc) = self.oo(&["commit", "-m", "p", "--grant", "pin"]);
        (squash(&o), rc)
    }

    /// What `oo status` says, squashed, the standard-root line dropped.
    fn status(&self) -> String {
        let (o, _) = self.oo(&["status"]);
        squash(&o.lines().filter(|l| !l.starts_with("Standard root")).collect::<Vec<_>>().join(" "))
    }

    /// Working-set member files on disk.
    fn members(&self) -> usize {
        fs::read_dir(self.ws.join(".oo").join("injections")).map(|d| d.count()).unwrap_or(0)
    }
}

/// What a commit said, reduced: "landed", "held", or the first 80 bytes.
fn said(o: &str) -> String {
    if o.contains("Commit successful") {
        "landed".into()
    } else if o.contains("already in HEAD") {
        "held".into()
    } else {
        o.chars().take(80).collect()
    }
}

const INT: &str = "{{ %kind: #type %name: \"int\" }}";


impl Ws {
    /// `oo evolve --pin --grant pin` of several new files in one command.
    fn pin_files(&self, srcs: &[&str]) -> (String, i32) {
        let mut names = Vec::new();
        for s in srcs {
            let n = fs::read_dir(&self.ws).map(|d| d.count()).unwrap_or(0);
            let f = format!("f{n}.n");
            fs::write(self.ws.join(&f), s).unwrap();
            names.push(f);
        }
        let mut args = vec!["evolve", "--pin", "--grant", "pin"];
        args.extend(names.iter().map(|s| s.as_str()));
        self.oo(&args)
    }
}

/// A pin lands: rc 0 at evolve, `status` lists it, the commit lands, the
/// member is collected. Pushes what is wrong.
fn lands(bad: &mut Vec<String>, w: &Ws, label: &str, pinned: &str) {
    let (o, rc) = w.pin(pinned);
    check(bad, &format!("{label}: pin"), format!("rc={rc}"), "rc=0");
    if rc != 0 {
        bad.push(format!("    (said {})", o.chars().take(100).collect::<String>()));
    }
    check(bad, &format!("{label}: status lists it"), w.status().contains("Staged changes").to_string(), "true");
    let (o, rc) = w.commit_pin();
    check(bad, &format!("{label}: commit"), format!("rc={rc} {}", said(&o)), "rc=0 landed");
    check(bad, &format!("{label}: the member is collected"), w.members().to_string(), "0");
}

const INT_IN: &str = "{{ %kind: #type %name: \"int\" }}";

// ── r1 ── a pin overwrites the leaves it writes; the siblings stay ──

#[test]
fn r1_a_pin_writes_only_its_leaves() {
    let mut bad = Vec::new();
    let mut run: Vec<Box<dyn Fn() -> Vec<String> + Send + Sync>> = Vec::new();
    // (label, committed, the pin, what to read, what it reads after)
    let rows: Vec<(&'static str, &'static str, &'static str, &'static str, String)> = vec![
        ("a path key", "h: { k: 5, r: 6 }", "h.k: 7", "_.h", "{ k: 7 r: 6 }".into()),
        ("the nested spelling", "h: { k: 5, r: 6 }", "h: { k: 7 }", "_.h", "{ k: 7 r: 6 }".into()),
        ("two levels down", "a: { b: { c: 0, d: 1 }, e: 2 }", "a.b.c: 1", "_.a", "{ b: { c: 1 d: 1 } e: 2 }".into()),
        ("two levels down, nested", "a: { b: { c: 0, d: 1 }, e: 2 }", "a: { b: { c: 1 } }", "_.a", "{ b: { c: 1 d: 1 } e: 2 }".into()),
        ("a sibling recipe reads the pinned leaf", "h: { k: @int, r: k + 1 }", "h.k: 5", "_.h", "{ k: 5 r: 6 }".into()),
        ("a ⊥ leaf beside a field", "h: { k: 5, r: 5 & 7 }", "h.r: 3", "_.h", "{ k: 5 r: 3 }".into()),
        ("a list element", "l: [1, 2, 3]", "l.1: 9", "_.l", "[1, 9, 3]".into()),
        ("a list written by its elements", "l: [1, 2, 3]", "l: [7]", "_.l", "[7, 2, 3]".into()),
        ("a type at a leaf", "h: { k: 5, r: 6 }", "h.k: @int", "_.h", format!("{{ k: {INT_IN} r: 6 }}")),
        ("a leaf widened inside a record", "r: { a: 1, b: 2 }", "r: { b: @int }", "_.r", format!("{{ a: 1 b: {INT_IN} }}")),
    ];
    for (label, committed, pinned, q, want) in rows {
        run.push(Box::new(move || {
            let mut bad = Vec::new();
            let w = Ws::new("r1");
            check(&mut bad, &format!("{label}: setup"), w.write(committed), "e0 c0");
            lands(&mut bad, &w, label, pinned);
            check(&mut bad, &format!("{label}: {q}"), short(&w.eval(q)), &want);
            bad
        }));
    }
    // The two spellings land one root, and at one level it is the root of
    // writing the result down.
    for (label, committed, a, b, written) in [
        ("one level", "h: { k: 5, r: 6 }", "h.k: 7", "h: { k: 7 }", Some("h: { k: 7, r: 6 }")),
        ("two levels", "a: { b: { c: 0, d: 1 }, e: 2 }", "a.b.c: 1", "a: { b: { c: 1 } }", None),
    ] {
        run.push(Box::new(move || {
            let mut bad = Vec::new();
            let (wa, wb) = (Ws::new("r1s"), Ws::new("r1s"));
            for (w, p) in [(&wa, a), (&wb, b)] {
                let _ = w.write(committed);
                let _ = w.pin(p);
                let _ = w.commit_pin();
            }
            check(&mut bad, &format!("{label}: `{a}` and `{b}` land one root"), wa.root(), &wb.root());
            if let Some(src) = written {
                let wc = Ws::new("r1s");
                let _ = wc.write(src);
                check(&mut bad, &format!("{label}: the root of writing `{src}`"), wa.root(), &wc.root());
            }
            bad
        }));
    }
    let n = cells(&mut bad, run);
    verdict(bad, n);
}

// ── r2 ── one pin write is a set: its fields meet, then it overwrites ──

#[test]
fn r2_one_pin_write_is_a_set() {
    let mut bad = Vec::new();
    let mut run: Vec<Box<dyn Fn() -> Vec<String> + Send + Sync>> = Vec::new();
    // (label, the pin, what to read, what it reads after)
    let rows: Vec<(&'static str, &'static str, &'static str, &'static str)> = vec![
        ("two path keys", "h.k: 7\nh.r: 8", "_.h", "{ k: 7 r: 8 }"),
        ("two path keys, the other order", "h.r: 8\nh.k: 7", "_.h", "{ k: 7 r: 8 }"),
        ("a path key twice with one value", "h.k: 7\nh.r: 8\nh.k: 7", "_.h", "{ k: 7 r: 8 }"),
        ("two path keys at two depths", "a.b: 1\na.c.d: 2", "_.a", "{ b: 1 c: { d: 2 } }"),
        ("one leaf twice, the narrower first", "h.k: 5\nh.k: @int", "_.h", "{ k: 5 }"),
        ("one leaf twice, the wider first", "h.k: @int\nh.k: 5", "_.h", "{ k: 5 }"),
    ];
    for (label, pinned, q, want) in rows {
        run.push(Box::new(move || {
            let mut bad = Vec::new();
            let w = Ws::new("r2");
            let _ = w.write("x: 0");
            lands(&mut bad, &w, label, pinned);
            check(&mut bad, &format!("{label}: {q}"), short(&w.eval(q)), want);
            bad
        }));
    }
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        let (wa, wb) = (Ws::new("r2s"), Ws::new("r2s"));
        for (w, p) in [(&wa, "h.k: 7\nh.r: 8"), (&wb, "h: { k: 7, r: 8 }")] {
            let _ = w.write("x: 0");
            let _ = w.pin(p);
            let _ = w.commit_pin();
        }
        check(&mut bad, "two path keys and the nested spelling land one root", wa.root(), &wb.root());
        bad
    }));
    // Two definitions of one coordinate in one pin write are refused, as
    // the same write without `--pin` is (SPEC_10 §2.3.1: two fields of one
    // write at one coordinate are two definitions). One command is one
    // member, its files too.
    for (label, srcs, at) in [
        ("one leaf twice", vec!["h.k: 7\nh.k: 8"], "#conflict at h.k"),
        ("one coordinate twice", vec!["r: 7\nr: 8"], "#conflict at r"),
        ("one coordinate in two files of one command", vec!["r: 3", "r: 4"], "#conflict at r"),
    ] {
        run.push(Box::new(move || {
            let mut bad = Vec::new();
            let w = Ws::new("r2r");
            let _ = w.write("x: 0");
            let before = w.root();
            let (o, rc) = w.pin_files(&srcs);
            check(&mut bad, &format!("{label}: refused"), format!("rc={rc} {}", o.contains(at)), "rc=1 true");
            check(&mut bad, &format!("{label}: no member"), w.members().to_string(), "0");
            check(&mut bad, &format!("{label}: root"), w.root(), &before);
            bad
        }));
    }
    let n = cells(&mut bad, run);
    verdict(bad, n);
}

// ── r3 ── a later pin takes only the leaves it writes from what it absorbs ──

#[test]
fn r3_a_later_pin_takes_only_its_leaves() {
    let mut bad = Vec::new();
    let mut run: Vec<Box<dyn Fn() -> Vec<String> + Send + Sync>> = Vec::new();
    // (label, committed, steps: ('e' evolve | 'p' pin, src), read, want)
    let rows: Vec<(&'static str, &'static str, Vec<(char, &'static str)>, &'static str, &'static str)> = vec![
        ("two pins, two leaves", "h: { k: 5, r: 6 }", vec![('p', "h.k: 7"), ('p', "h.r: 9")], "_.h", "{ k: 7 r: 9 }"),
        ("two pins, one leaf", "h: { k: 5, r: 6 }", vec![('p', "h.k: 7"), ('p', "h.k: 8")], "_.h", "{ k: 8 r: 6 }"),
        ("a pin, then a write beside it", "h: { k: 5, r: 6 }", vec![('p', "h.k: 7"), ('e', "h.z: 1")], "_.h", "{ k: 7 r: 6 z: 1 }"),
        ("a write, then a pin beside it", "h: { k: 5, r: 6 }", vec![('e', "h.z: 1"), ('p', "h.k: 7")], "_.h", "{ k: 7 r: 6 z: 1 }"),
        ("a write at the leaf, then a pin there", "h: { k: 5, r: 6 }", vec![('e', "h.k: 5"), ('p', "h.k: 7")], "_.h", "{ k: 7 r: 6 }"),
        ("deeper after shallower", "h: { k: { a: 0, b: 2 }, r: 6 }", vec![('p', "h.k: 3"), ('p', "h.k: { a: 1 }")], "_.h", "{ k: { a: 1 } r: 6 }"),
        ("shallower after deeper", "h: { k: { a: 0, b: 2 }, r: 6 }", vec![('p', "h.k: { a: 1 }"), ('p', "h.k: 3")], "_.h", "{ k: 3 r: 6 }"),
    ];
    for (label, committed, steps, q, want) in rows {
        run.push(Box::new(move || {
            let mut bad = Vec::new();
            let w = Ws::new("r3");
            let _ = w.write(committed);
            for (k, s) in &steps {
                let (o, rc) = if *k == 'p' { w.pin(s) } else { w.evolve(s) };
                check(&mut bad, &format!("{label}: {k} {s}"), format!("rc={rc}"), "rc=0");
                if rc != 0 {
                    bad.push(format!("    (said {})", o.chars().take(100).collect::<String>()));
                }
            }
            let (o, rc) = w.commit_pin();
            check(&mut bad, &format!("{label}: commit"), format!("rc={rc} {}", said(&o)), "rc=0 landed");
            check(&mut bad, &format!("{label}: {q}"), short(&w.eval(q)), want);
            bad
        }));
    }
    let n = cells(&mut bad, run);
    verdict(bad, n);
}

// ── r4 ── the siblings a pin keeps are judged as any definition (Q-122) ──

#[test]
fn r4_the_siblings_a_pin_keeps_are_judged() {
    let mut bad = Vec::new();
    let mut run: Vec<Box<dyn Fn() -> Vec<String> + Send + Sync>> = Vec::new();
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        let w = Ws::new("r4");
        let _ = w.write("h: { k: 5, r: k & 5 }");
        let before = w.root();
        let (o, rc) = w.pin("h.k: 7");
        check(&mut bad, "a sibling read as ⊥: refused at h.r", format!("rc={rc} {}", o.contains("#conflict at h.r")), "rc=1 true");
        check(&mut bad, "a sibling read as ⊥: no member", w.members().to_string(), "0");
        check(&mut bad, "a sibling read as ⊥: root", w.root(), &before);
        check(&mut bad, "a sibling read as ⊥: _.h", short(&w.eval("_.h")), "{ k: 5 r: 5 }");
        bad
    }));
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        let w = Ws::new("r4");
        let _ = w.write("h: { k: 5, r: k + 1 }");
        lands(&mut bad, &w, "a sibling that reads it", "h.k: 7");
        check(&mut bad, "a sibling that reads it: _.h", short(&w.eval("_.h")), "{ k: 7 r: 8 }");
        bad
    }));
    let n = cells(&mut bad, run);
    verdict(bad, n);
}

// ── r5 ── a pin of what HEAD holds is held, through any spelling (Q-123) ──

#[test]
fn r5_a_pin_of_what_head_holds_is_held_in_any_spelling() {
    let mut bad = Vec::new();
    let mut run: Vec<Box<dyn Fn() -> Vec<String> + Send + Sync>> = Vec::new();
    for (label, committed, pinned, q) in [
        ("a path key", "h: { k: 5, r: 6 }", "h.k: 5", "_.h"),
        ("fewer fields", "h: { k: 5, r: 6 }", "h: { k: 5 }", "_.h"),
        ("an empty record", "h: { k: 5, r: 6 }", "h: {}", "_.h"),
        ("two levels down", "a: { b: { c: 0, d: 1 }, e: 2 }", "a.b.c: 0", "_.a"),
        ("fewer elements", "l: [1, 2, 3]", "l: [1]", "_.l"),
    ] {
        run.push(Box::new(move || {
            let mut bad = Vec::new();
            let w = Ws::new("r5");
            let _ = w.write(committed);
            let (before, read) = (w.root(), w.eval(q));
            let (_, rc) = w.pin(pinned);
            check(&mut bad, &format!("{label}: pin"), format!("rc={rc}"), "rc=0");
            check(&mut bad, &format!("{label}: status is quiet"), w.status().contains("Staged changes").to_string(), "false");
            let (o, rc) = w.commit_pin();
            check(&mut bad, &format!("{label}: commit"), format!("rc={rc} {}", said(&o)), "rc=1 held");
            check(&mut bad, &format!("{label}: root"), w.root(), &before);
            check(&mut bad, &format!("{label}: {q}"), w.eval(q), &read);
            bad
        }));
    }
    let n = cells(&mut bad, run);
    verdict(bad, n);
}

// ── g1 ── what a pin writes at a leaf overwrites; the rest as before ──

#[test]
fn g1_a_leaf_is_overwritten_and_the_rest_as_before() {
    let mut bad = Vec::new();
    let mut run: Vec<Box<dyn Fn() -> Vec<String> + Send + Sync>> = Vec::new();
    for (label, committed, pinned, q, want) in [
        ("a scalar over a record", "h: { k: 5, r: 6 }", "h: 3", "_.h", "3"),
        ("a record over a scalar", "h: 3", "h.k: 1", "_.h", "{ k: 1 }"),
        ("a new coordinate", "x: 0", "h: { k: 1 }", "_.h", "{ k: 1 }"),
        ("an empty record, new", "x: 0", "h: {}", "_.h", "{}"),
        ("over ⊥", "r: 5 & 7", "r: 3", "_.r", "3"),
    ] {
        run.push(Box::new(move || {
            let mut bad = Vec::new();
            let w = Ws::new("g1");
            let _ = w.write(committed);
            lands(&mut bad, &w, label, pinned);
            check(&mut bad, &format!("{label}: {q}"), short(&w.eval(q)), want);
            bad
        }));
    }
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        let w = Ws::new("g1");
        let _ = w.write("h: { k: 5, r: 6 }");
        let before = w.root();
        let (o, rc) = w.evolve("h.k: 7");
        check(&mut bad, "a write without pin at a leaf: refused", format!("rc={rc} {}", o.contains("#conflict at h.k")), "rc=1 true");
        let (o, _) = w.evolve("h.k: 5");
        let _ = o;
        let (o, _) = w.commit();
        let _ = o;
        check(&mut bad, "a write without pin of what HEAD holds: root", w.root(), &before);
        check(&mut bad, "a write without pin: _.h", w.eval("_.h"), "{ k: 5 r: 6 }");
        bad
    }));
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        let w = Ws::new("g1");
        let _ = w.write("h: { k: 5, r: 6 }");
        let before = w.root();
        let _ = w.pin("h.k: 7");
        let (o, rc) = w.commit();
        check(&mut bad, "a pin's commit without the capability", format!("rc={rc} {}", o.contains("#privileged_required")), "rc=1 true");
        check(&mut bad, "a pin's commit without the capability: root", w.root(), &before);
        bad
    }));
    let n = cells(&mut bad, run);
    verdict(bad, n);
}

// ── g2 ── small roots do not move ──

#[test]
fn g2_the_roots_of_small_writes_do_not_move() {
    let mut bad = Vec::new();
    for (src, want) in [
        ("x: 0", "b74d775816f6b5be6cc152577b7556f6c8fdcc63433b0e31068cc2484a94ae0f"),
        ("v: 1 + 1", "df91a8ad239ea8727c3a03a71eb4b0a787312b92306db4d8b124a73faaf40199"),
        ("f: (x -> x + 1)", "258a1a08adfd0340cdbe103e4f4851bea107a67e298e72d6679badc0595bfd1f"),
        ("h: { a: 1, b: a + 1 }", "2284caf43b29be83c7b90aee891418b43fcd91b8495819d95f70ff43f54a0e5e"),
    ] {
        let w = Ws::new("g2");
        check(&mut bad, &format!("{src}: write"), w.write(src), "e0 c0");
        check(&mut bad, &format!("{src}: root"), w.root(), want);
    }
    verdict(bad, 8);
}

// ── r6 ── a pin a v0.42.0 sidecar names writes its leaves too ──

fn copy_dir(from: &std::path::Path, to: &std::path::Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let p = e.path();
        if p.is_dir() {
            copy_dir(&p, &to.join(e.file_name()));
        } else {
            fs::copy(&p, to.join(e.file_name())).unwrap();
        }
    }
}

#[test]
fn r6_a_sidecar_pin_writes_its_leaves() {
    let mut bad = Vec::new();
    let w = Ws::new("r6");
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pin_sidecar_v042_repo/oo_dir");
    copy_dir(&fixture, &w.ws.join(".oo"));
    check(&mut bad, "precondition: the sidecar", fs::read_to_string(w.ws.join(".oo").join("pin_pending")).unwrap_or_default(), "[\"h\"]");
    let (o, rc) = w.commit_pin();
    check(&mut bad, "commit", format!("rc={rc} {}", said(&o)), "rc=0 landed");
    check(&mut bad, "_.h", short(&w.eval("_.h")), "{ k: 7 r: 6 }");
    verdict(bad, 3);
}
