// A name that arrived late.
// Order: nlang-tools/docs/a_name_that_arrived_late_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-122. No ruling: SPEC_10 §2.3.1
// (D110 款) "存語法不得讓衝突晚到：與既有定義相撞的寫入，照舊在演化時以兩邊的
// 答案判、拒絕"; §2.2.1 (Staged_old ⊓ Definition = ⊥ ⟹ the evolve does not
// happen, the report names the leaf); D56 (a ⊥ that enters a commit is
// reported).
//
// ── What is wrong (v0.122.0) ────────────────────────────────────────────
//
//   `r: 7` and `r: n + 1` committed (the second is kept as its syntax, D110),
//   then `n: 5`: evolve takes it, the commit stores `r` as ⊥. The other way
//   round (`n: 5`, `r: 7`, then `r: n + 1`) the last write is refused. The
//   same for an alias, `_.x`, a name read through another name, a type or a
//   range refined, a field that reads its record's name, a record field and
//   a list element that read the root, an arrow's body, a comparison — in
//   every split (commits, evolves, one file). And a ⊥ a write makes itself
//   in a list element (`n: 5`, `l: [n & 7]`), or late in a record or a list,
//   enters history without a report.
//
// The other half: a name that agrees is taken; a recipe that arrives last is
// refused as before; one definition that reads ⊥ by itself is the writer's
// (kept, reported — D56); a ⊥ already there does not refuse a later write;
// evolve runs no effect; small roots do not move.
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
        let scratch = nlang_interpreter::ScratchDir::new(&format!("name-arrived-late-{tag}"));
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



/// (shape, the writes before the name, the late name, the name that agrees,
/// where the ⊥ falls, what to read)
fn shapes() -> Vec<(&'static str, Vec<&'static str>, &'static str, &'static str, &'static str, &'static str)> {
    vec![
        ("two definitions", vec!["r: 7", "r: n + 1"], "n: 5", "n: 6", "r", "_.r"),
        ("alias", vec!["b: a", "b: 3"], "a: 4", "a: 3", "b", "_.b"),
        ("absolute path", vec!["r: 7", "r: _.x + 1"], "x: 5", "x: 6", "r", "_.r"),
        ("through a name", vec!["r: 7", "r: m + 1", "m: n"], "n: 5", "n: 6", "r", "_.r"),
        ("type refined", vec!["n: @int", "r: 7", "r: n + 1"], "n: 5", "n: 6", "r", "_.r"),
        ("range refined", vec!["n: 1..10", "r: 7", "r: n + 1"], "n: 5", "n: 6", "r", "_.r"),
        ("a field reading its record", vec!["h: { k: @int, r: k + 1 }", "h: { r: 7 }"], "h: { k: 5 }", "h: { k: 6 }", "h.r", "_.h.r"),
        ("a record field reading the root", vec!["h: { r: n + 1 }", "h: { r: 7 }"], "n: 5", "n: 6", "h.r", "_.h.r"),
        ("a list element", vec!["l: [n]", "l: [7]"], "n: 5", "n: 7", "l.0", "_.l.0"),
        ("an arrow's body", vec!["g: (x -> n)", "r: g 0", "r: 7"], "n: 5", "n: 7", "r", "_.r"),
        ("a comparison", vec!["r: n = 5", "r: #false"], "n: 5", "n: 6", "r", "_.r"),
        ("one line &", vec!["r: 7 & (n + 1)"], "n: 5", "n: 6", "r", "_.r"),
        ("one recipe", vec!["r: n & 7"], "n: 5", "n: 7", "r", "_.r"),
        ("a root spread lands the name", vec!["...c", "r: 7", "r: x + 1"], "c: { x: 5 }", "c: { x: 6 }", "r", "_.r"),
        ("the name comes in a root spread", vec!["h: { q: n + 1 }", "h: { q: 7 }"], "d: { n: 5 }\n...d", "d: { n: 6 }\n...d", "h.q", "_.h.q"),
    ]
}

/// One definition per write: a shape whose recipe is one line reads ⊥ by
/// itself when the name is in the same file — that is the writer's (r3).
fn one_definition(shape: &str) -> bool {
    shape == "one line &" || shape == "one recipe"
}

// ── r1 ── a name that arrives late and makes a definition read ⊥ is refused ──

#[test]
fn r1_a_late_name_that_makes_a_definition_read_bottom_is_refused() {
    let mut bad = Vec::new();
    let mut run: Vec<Box<dyn Fn() -> Vec<String> + Send + Sync>> = Vec::new();
    for (shape, before, late, _ok, at, q) in shapes() {
        for split in ["commits", "evolves", "one file"] {
            if split == "one file" && one_definition(shape) {
                continue;
            }
            let before: Vec<String> = before.iter().map(|s| s.to_string()).collect();
            run.push(Box::new(move || {
                let mut bad = Vec::new();
                let w = Ws::new("r1");
                let tag = format!("{shape}, {split}");
                let (o, rc) = match split {
                    "commits" => {
                        for s in &before {
                            check(&mut bad, &format!("{tag}: {s}"), w.write(s), "e0 c0");
                        }
                        w.evolve(late)
                    }
                    "evolves" => {
                        for s in &before {
                            check(&mut bad, &format!("{tag}: {s}"), format!("e{}", w.evolve(s).1), "e0");
                        }
                        w.evolve(late)
                    }
                    _ => w.evolve(&format!("{}\n{late}", before.join("\n"))),
                };
                check(&mut bad, &format!("{tag}: {late} refused"), format!("rc={rc}"), "rc=1");
                check(&mut bad, &format!("{tag}: named at the leaf"), o.contains(&format!("#conflict at {at}")).to_string(), "true");
                if rc != 1 || !o.contains(&format!("#conflict at {at}")) {
                    bad.push(format!("    (said {})", o.chars().take(120).collect::<String>()));
                }
                let _ = w.commit();
                let r = short(&w.eval(q));
                check(&mut bad, &format!("{tag}: {q} is not ⊥ ({r})"), r.starts_with("_|_").to_string(), "false");
                bad
            }));
        }
    }
    let n = cells(&mut bad, run);
    verdict(bad, n);
}

// ── r2 ── a ⊥ a write makes itself is kept and reported, in every shape ──

#[test]
fn r2_a_bottom_the_write_makes_itself_is_reported() {
    let mut bad = Vec::new();
    let mut run: Vec<Box<dyn Fn() -> Vec<String> + Send + Sync>> = Vec::new();
    for (label, src, at, q) in [
        ("top", "n: 5\nr: n & 7", "r", "_.r"),
        ("a record field reading the root", "n: 5\nh: { r: n & 7 }", "h.r", "_.h.r"),
        ("a field reading its record", "h: { k: 5, r: k & 7 }", "h.r", "_.h.r"),
        ("a list element reading a name", "n: 5\nl: [n & 7]", "l.0", "_.l.0"),
        ("a record literal", "h: { r: 5 & 7 }", "h.r", "_.h.r"),
        ("a list literal", "l: [5 & 7]", "l.0", "_.l.0"),
        ("one line &, the name in the file", "r: 7 & (n + 1)\nn: 5", "r", "_.r"),
        ("one recipe, the name in the file", "r: n & 7\nn: 5", "r", "_.r"),
    ] {
        run.push(Box::new(move || {
            let mut bad = Vec::new();
            let w = Ws::new("r2");
            let (eo, er) = w.evolve(src);
            check(&mut bad, &format!("{label}: evolve"), format!("rc={er}"), "rc=0");
            let (co, cr) = w.commit();
            check(&mut bad, &format!("{label}: commit"), format!("rc={cr}"), "rc=0");
            check(&mut bad, &format!("{label}: reported at {at}"), co.contains(&format!("#conflict at {at}")).to_string(), "true");
            if !co.contains(&format!("#conflict at {at}")) {
                bad.push(format!("    (evolve said {}; commit said {})", eo.chars().take(80).collect::<String>(), co.chars().take(120).collect::<String>()));
            }
            check(&mut bad, &format!("{label}: {q}"), short(&w.eval(q)), "_|_ #conflict");
            bad
        }));
    }
    let n = cells(&mut bad, run);
    verdict(bad, n);
}

// ── g1 ── a name that agrees is taken; a recipe that arrives last is refused as before ──

#[test]
fn g1_a_name_that_agrees_is_taken_and_a_late_recipe_refused() {
    let mut bad = Vec::new();
    let mut run: Vec<Box<dyn Fn() -> Vec<String> + Send + Sync>> = Vec::new();
    for (shape, before, late, ok, at, q) in shapes() {
        let before: Vec<String> = before.iter().map(|s| s.to_string()).collect();
        run.push(Box::new(move || {
            let mut bad = Vec::new();
            let w = Ws::new("g1-ok");
            for s in &before {
                let _ = w.write(s);
            }
            check(&mut bad, &format!("{shape}: {ok}"), w.write(ok), "e0 c0");
            check(&mut bad, &format!("{shape}: {q} after {ok}"), short(&w.eval(q)).starts_with("_|_").to_string(), "false");
            // One definition, or a last write that is itself a late name
            // (`m: n` after `r: m + 1`): r1's, not "as before".
            if one_definition(shape) || shape == "through a name" {
                return bad;
            }
            // The recipe last: the name first, then the writes; the last one
            // that collides is refused at the leaf.
            let w = Ws::new("g1-recipe");
            check(&mut bad, &format!("{shape}: {late} first"), w.write(late), "e0 c0");
            let mut said = Vec::new();
            for s in &before {
                let (o, rc) = w.evolve(s);
                if rc == 0 {
                    let _ = w.commit();
                }
                said.push((rc, o));
            }
            let refused = said.iter().any(|(rc, o)| *rc == 1 && o.contains(&format!("#conflict at {at}")));
            check(&mut bad, &format!("{shape}: a recipe that arrives last is refused at {at}"), refused.to_string(), "true");
            bad
        }));
    }
    let n = cells(&mut bad, run);
    verdict(bad, n);
}

// ── g2 ── a ⊥ already there does not refuse a later write ──

#[test]
fn g2_a_bottom_already_there_refuses_nothing_later() {
    let mut bad = Vec::new();
    let w = Ws::new("g2-a");
    check(&mut bad, "r: 5 & 7", w.write("r: 5 & 7"), "e0 c0");
    check(&mut bad, "then n: 1", w.write("n: 1"), "e0 c0");
    check(&mut bad, "then z: 2", w.write("z: 2"), "e0 c0");
    let w = Ws::new("g2-b");
    check(&mut bad, "n: 5, r: n & 7", w.write("n: 5\nr: n & 7"), "e0 c0");
    check(&mut bad, "then z: 1", w.write("z: 1"), "e0 c0");
    check(&mut bad, "then h: { k: 1 }", w.write("h: { k: 1 }"), "e0 c0");
    // An open term reads its input where it is used: it is not a ⊥ here,
    // and the commit reports none.
    let w = Ws::new("g2-c");
    let (_, rc) = w.evolve("t: { a: $.b + 1 }\nr: 7");
    check(&mut bad, "an open term: evolve", format!("rc={rc}"), "rc=0");
    let (co, rc) = w.commit();
    check(&mut bad, "an open term: commit", format!("rc={rc}"), "rc=0");
    check(&mut bad, "an open term: nothing reported", co.contains("#no_context").to_string(), "false");
    check(&mut bad, "an open term beside a name: n: 6", w.write("n: 6"), "e0 c0");
    verdict(bad, 10);
}

// ── g3 ── a later evolve runs no effect again ──

/// The effect an evolve runs (`~%Io./write_file` runs at evolve, v0.122.0
/// too) is not run again by a later evolve that looks for what reads ⊥: the
/// file, removed after the first evolve, is not written back.
#[test]
fn g3_a_later_evolve_runs_no_effect_again() {
    let mut bad = Vec::new();
    let w = Ws::new("g3");
    let target = w.root.join("written-by-an-effect");
    let t = target.display().to_string();
    let (_, rc) = w.evolve(&format!("e: ~%Io./write_file(\"{t}\", \"x\")\nr: 7"));
    check(&mut bad, "evolve the effect", format!("rc={rc}"), "rc=0");
    check(&mut bad, "the first evolve ran it (REACH)", target.exists().to_string(), "true");
    let _ = fs::remove_file(&target);
    let (_, rc) = w.evolve("r: n + 1");
    check(&mut bad, "evolve a recipe beside it", format!("rc={rc}"), "rc=0");
    let (_, rc) = w.evolve("n: 6");
    check(&mut bad, "evolve the name", format!("rc={rc}"), "rc=0");
    check(&mut bad, "no later evolve ran the effect again", target.exists().to_string(), "false");
    verdict(bad, 5);
}

// ── g4 ── small roots do not move ──

#[test]
fn g4_the_roots_of_small_writes_do_not_move() {
    let mut bad = Vec::new();
    for (src, want) in [
        ("x: 0", "b74d775816f6b5be6cc152577b7556f6c8fdcc63433b0e31068cc2484a94ae0f"),
        ("v: 1 + 1", "df91a8ad239ea8727c3a03a71eb4b0a787312b92306db4d8b124a73faaf40199"),
        ("f: (x -> x + 1)", "258a1a08adfd0340cdbe103e4f4851bea107a67e298e72d6679badc0595bfd1f"),
        ("h: { a: 1, b: a + 1 }", "2284caf43b29be83c7b90aee891418b43fcd91b8495819d95f70ff43f54a0e5e"),
    ] {
        let w = Ws::new("g4");
        check(&mut bad, &format!("{src}: write"), w.write(src), "e0 c0");
        check(&mut bad, &format!("{src}: root"), w.root(), want);
    }
    verdict(bad, 8);
}
