// A pin HEAD said it already held.
// Order: nlang-tools/docs/a_pin_head_said_it_held_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-123. No ruling: D84 (i) "內容已在
// `HEAD` 根裡（不移動位置，D80 ②）的注入不是提議"; SPEC_08 §6.2 `#pin`
// 「直接覆蓋節點值，跳過收斂邏輯」; SPEC_10 §2.2.3 (the pin's unit is the
// coordinate); D33 (a ⊥ is the operator's to resolve).
//
// ── What is wrong (v0.123.0) ────────────────────────────────────────────
//
//   `r: 5 & 7` committed (a ⊥ the write made itself, kept and reported —
//   D56), then `evolve --pin --grant pin` `r: 3`: evolve takes it, `status`
//   says the universe is static, `commit --grant pin` answers `already in
//   HEAD`, and `r` stays ⊥. The member file stays. The same for a record
//   field, a list element, a recipe stored as ⊥; and for a pin that widens
//   (`r: 3` pinned to `@int` or `1..10`) or drops a field (`{ a: 1, b: 2 }`
//   pinned to `{ a: 1 }`). "HEAD holds it" is judged by the meet, and the
//   meet of a pin's value with ⊥, with a narrower value, or with a record
//   that has more fields is the root. An ordinary write over ⊥ is refused
//   (monotone), so a pin is the operator's only way that keeps history.
//
// The other half: a pin HEAD really holds (the same value) answers `already
// in HEAD` as before; a pin that narrows lands as before; an ordinary write
// over ⊥ is still refused; a pin's commit still needs the capability; an
// ordinary write HEAD holds is held; small roots do not move.
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
        let scratch = nlang_interpreter::ScratchDir::new(&format!("pin-head-held-{tag}"));
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

/// (label, committed one by one, the pin, what to read, what it reads after)
fn moves() -> Vec<(&'static str, Vec<&'static str>, &'static str, &'static str, &'static str)> {
    vec![
        ("over ⊥", vec!["r: 5 & 7"], "r: 3", "_.r", "3"),
        ("over a record field that is ⊥", vec!["h: { a: 1, r: 5 & 7 }"], "h: { a: 1, r: 3 }", "_.h", "{ a: 1 r: 3 }"),
        ("over a list element that is ⊥", vec!["l: [5 & 7, 1]"], "l: [3, 1]", "_.l", "[3, 1]"),
        ("over a recipe stored as ⊥", vec!["n: 5\nr: n & 7"], "r: 3", "_.r", "3"),
        ("over ⊥ committed after another field", vec!["a: 1", "r: 5 & 7"], "r: 3", "_.r", "3"),
        ("wider: a type", vec!["r: 3"], "r: @int", "_.r", INT),
        ("wider: a range", vec!["r: 3"], "r: 1..10", "_.r", "1..10"),
        // AMENDED 2026-11-05 for Q-124 (D128): a pin writes the leaves of a record, so it cannot drop
        // a field ({ a: 1 } over { a: 1, b: 2 } is held); the record cells widen a field instead.
        ("a field of a record wider", vec!["r: { a: 1, b: 2 }"], "r: { a: 1, b: @int }", "_.r", "{ a: 1 b: {{ %kind: #type %name: \"int\" }} }"),
        ("a field of a nested record wider", vec!["h: { k: { a: 1, b: 2 }, z: 0 }"], "h: { k: { a: 1, b: @int }, z: 0 }", "_.h.k", "{ a: 1 b: {{ %kind: #type %name: \"int\" }} }"),
    ]
}

// ── r1 ── a pin that moves a coordinate lands, wherever the meet would not move it ──

#[test]
fn r1_a_pin_that_moves_a_coordinate_lands() {
    let mut bad = Vec::new();
    let mut run: Vec<Box<dyn Fn() -> Vec<String> + Send + Sync>> = Vec::new();
    for (label, before, pinned, q, want) in moves() {
        run.push(Box::new(move || {
            let mut bad = Vec::new();
            let w = Ws::new("r1");
            for s in &before {
                let _ = w.write(s);
            }
            let (o, rc) = w.pin(pinned);
            check(&mut bad, &format!("{label}: pin"), format!("rc={rc}"), "rc=0");
            if rc != 0 {
                bad.push(format!("    (said {})", o.chars().take(100).collect::<String>()));
            }
            check(&mut bad, &format!("{label}: status lists it"), w.status().contains("Staged changes").to_string(), "true");
            let (o, rc) = w.commit_pin();
            check(&mut bad, &format!("{label}: commit"), format!("rc={rc} {}", said(&o)), "rc=0 landed");
            check(&mut bad, &format!("{label}: {q}"), short(&w.eval(q)), want);
            check(&mut bad, &format!("{label}: the member is collected"), w.members().to_string(), "0");
            bad
        }));
    }
    let n = cells(&mut bad, run);
    verdict(bad, n);
}

// ── r2 ── the same beside other members ──

#[test]
fn r2_a_pin_beside_other_members_lands() {
    let mut bad = Vec::new();
    let mut run: Vec<Box<dyn Fn() -> Vec<String> + Send + Sync>> = Vec::new();
    // (label, committed, steps: ('e' evolve | 'p' pin, src), reads)
    let rows: Vec<(&'static str, &'static str, Vec<(char, &'static str)>, Vec<(&'static str, &'static str)>)> = vec![
        ("an ordinary write first", "r: 5 & 7", vec![('e', "s: 2"), ('p', "r: 3")], vec![("_.r", "3"), ("_.s", "2")]),
        ("an ordinary write after", "r: 5 & 7", vec![('p', "r: 3"), ('e', "s: 2")], vec![("_.r", "3"), ("_.s", "2")]),
        ("two pins at one coordinate", "r: 3", vec![('p', "r: @int"), ('p', "r: 1..10")], vec![("_.r", "1..10")]),
        // AMENDED 2026-11-05 for Q-124 (D128): widen a field of `s` instead of dropping one.
        ("two pins, two coordinates", "r: 5 & 7\ns: { a: 1, b: 2 }", vec![('p', "r: 3"), ('p', "s: { a: 1, b: @int }")], vec![("_.r", "3"), ("_.s", "{ a: 1 b: {{ %kind: #type %name: \"int\" }} }")]),
        ("one pin, a held and a moved coordinate", "r: 3\ns: 5 & 7", vec![('p', "r: 3\ns: 4")], vec![("_.r", "3"), ("_.s", "4")]),
    ];
    for (label, committed, steps, reads) in rows {
        run.push(Box::new(move || {
            let mut bad = Vec::new();
            let w = Ws::new("r2");
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
            for (q, want) in &reads {
                check(&mut bad, &format!("{label}: {q}"), short(&w.eval(q)), want);
            }
            check(&mut bad, &format!("{label}: members collected"), w.members().to_string(), "0");
            bad
        }));
    }
    // A pin that lands, then a wider pin, then the same again (held).
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        let w = Ws::new("r2-again");
        let _ = w.write("r: 3");
        let _ = w.pin("r: 4");
        let (o, rc) = w.commit_pin();
        check(&mut bad, "pin r: 4 over r: 3", format!("rc={rc} {}", said(&o)), "rc=0 landed");
        let _ = w.pin("r: @int");
        let (o, rc) = w.commit_pin();
        check(&mut bad, "then pin r: @int", format!("rc={rc} {}", said(&o)), "rc=0 landed");
        check(&mut bad, "then pin r: @int: _.r", short(&w.eval("_.r")), INT);
        let _ = w.pin("r: @int");
        let (o, rc) = w.commit_pin();
        check(&mut bad, "pin r: @int again", format!("rc={rc} {}", said(&o)), "rc=1 held");
        bad
    }));
    let n = cells(&mut bad, run);
    verdict(bad, n);
}

// ── g1 ── what HEAD really holds is held; what a pin never did, it still does not ──

#[test]
fn g1_what_head_holds_is_held_and_the_rest_as_before() {
    let mut bad = Vec::new();
    let mut run: Vec<Box<dyn Fn() -> Vec<String> + Send + Sync>> = Vec::new();
    // A pin of the value HEAD holds: held, root unmoved.
    for (label, committed, pinned) in [
        ("the same value", "r: 3", "r: 3"),
        ("the same record", "h: { a: 1, b: 2 }", "h: { a: 1, b: 2 }"),
        ("the same type", "r: @int", "r: @int"),
        ("the same ⊥ (5 & 7 again)", "r: 5 & 7", "r: 5 & 7"),
        ("the same record that reads its names", "h: { a: 1, b: a + 1 }", "h: { a: 1, b: a + 1 }"),
        ("the same arrow", "f: (x -> x + 1)", "f: (x -> x + 1)"),
        ("the same table", "t: { @{ @int }: 0, _: 1 }", "t: { @{ @int }: 0, _: 1 }"),
        ("the same recipe reading the root", "n: 5\nr: n + 1", "r: n + 1"),
    ] {
        run.push(Box::new(move || {
            let mut bad = Vec::new();
            let w = Ws::new("g1-held");
            let _ = w.write(committed);
            let root = w.root();
            let (_, rc) = w.pin(pinned);
            check(&mut bad, &format!("{label}: pin"), format!("rc={rc}"), "rc=0");
            check(&mut bad, &format!("{label}: status"), w.status(), "Universe is static (no staged changes).");
            let (o, rc) = w.commit_pin();
            check(&mut bad, &format!("{label}: commit"), format!("rc={rc} {}", said(&o)), "rc=1 held");
            check(&mut bad, &format!("{label}: root"), w.root(), &root);
            bad
        }));
    }
    // A pin that narrows lands as before; the same pin again is then held.
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        let w = Ws::new("g1-narrow");
        let _ = w.write("r: 3");
        let _ = w.pin("r: 4");
        let (o, rc) = w.commit_pin();
        check(&mut bad, "pin r: 4 over r: 3", format!("rc={rc} {}", said(&o)), "rc=0 landed");
        check(&mut bad, "pin r: 4: _.r", w.eval("_.r"), "4");
        let _ = w.pin("r: 4");
        let (o, rc) = w.commit_pin();
        check(&mut bad, "pin r: 4 again", format!("rc={rc} {}", said(&o)), "rc=1 held");
        bad
    }));
    // An ordinary write over ⊥ is still refused; an ordinary write HEAD holds is held.
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        let w = Ws::new("g1-plain");
        let _ = w.write("r: 5 & 7\nq: 3");
        let root = w.root();
        let (o, rc) = w.evolve("r: 3");
        check(&mut bad, "ordinary r: 3 over ⊥: refused", format!("rc={rc} {}", o.contains("#conflict at r")), "rc=1 true");
        let (o, rc) = w.evolve("q: @int");
        check(&mut bad, "ordinary q: @int over q: 3: evolve", format!("rc={rc}"), "rc=0");
        let _ = o;
        let (o, rc) = w.commit();
        check(&mut bad, "ordinary q: @int over q: 3: held", format!("rc={rc} {}", o.contains("already in HEAD")), "rc=1 true");
        check(&mut bad, "root unmoved", w.root(), &root);
        check(&mut bad, "_.q", w.eval("_.q"), "3");
        bad
    }));
    // A pin's commit still needs the capability; nothing lands without it.
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        let w = Ws::new("g1-grant");
        let _ = w.write("r: 3");
        let root = w.root();
        let _ = w.pin("r: 4");
        let (o, rc) = w.oo(&["commit", "-m", "p"]);
        check(&mut bad, "commit without --grant pin", format!("rc={rc} {}", o.contains("#privileged_required")), "rc=1 true");
        check(&mut bad, "root unmoved", w.root(), &root);
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
