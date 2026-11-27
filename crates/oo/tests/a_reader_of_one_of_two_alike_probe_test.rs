// Q-143 probe: a reader of one of two records written alike reads that one.
//
// Q-126 (SPEC_10 §2.3.1, clause 2 and 3): in one observation a field written
// once is one value, its effect done once; two records written alike are two,
// each doing its own. D129 (clause 4): in `oo evolve` a definition that reads
// a lazy field doing an effect when forced stays its recipe; D131: at commit a
// cell whose effect all comes from reading other coordinates stores its
// syntax. v0.142.0 stores `k: q.r1.a` as the recipe of the field it reads,
// `g "x"`, closing over frames that say nothing of which of two alike records
// it was; Q-142 claims a frame only when one record reads as it. So with two
// alike, `k` does the effect on its own, and the commit stores its answer.
//
// v0.142.0 (since v0.125.0, Inbox I-324): `q: { r1: { a: g "x" },
// r2: { a: g "x" } }`, `k: q.r1.a` commits 3 bytes (2 is right); two alike in
// a list, two alike on the root with the reader in a second evolve, through a
// function: 3 or 4. `e: k = q.r1.a` with `random` stores #false.
//
// r1 — each effect is done once: two alike records, a reader of one, commit
//      2 bytes (one record: 1); also with the readers in a second evolve, or
//      in a later commit; g written later or pinned, as v0.142.0.
// r2 — one value: with `random`, `k = q.r1.a` is #true; `k = q.r2.a` stays
//      #false (two alike are two).
// g1 — roots and readings of these shapes, as the reference stores them: a
//      reader of a lazy effect field stores its own syntax (`k: q.r1.a`,
//      `k: [f 1, f 2]`), not the field's answer. Pure shapes as v0.142.0.

use std::fs;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const PURE: &str = "g: (s -> s)";
const GA: &str = "g: (s -> ~%Io./append_file (\"a.log\", s))";
const TWO: &str = "q: { r1: { a: g \"x\" }, r2: { a: g \"x\" } }";

// r1: (label, written with g, bytes a commit must append)
const ONCE: &[(&str, &str, usize)] = &[
    ("one record, a reader (as Q-142)", "q: { r: { a: g \"x\" } }\nk: q.r.a", 1),
    ("two alike, a reader of one", "q: { r1: { a: g \"x\" }, r2: { a: g \"x\" } }\nk: q.r1.a", 2),
    ("two alike, a reader of each", "q: { r1: { a: g \"x\" }, r2: { a: g \"x\" } }\nk1: q.r1.a\nk2: q.r2.a", 2),
    ("two alike, two readers of one", "q: { r1: { a: g \"x\" }, r2: { a: g \"x\" } }\nk1: q.r1.a\nk2: q.r1.a", 2),
    ("two alike three deep", "q: { r1: { s: { a: g \"x\" } }, r2: { s: { a: g \"x\" } } }\nk: q.r1.s.a", 2),
    ("two alike, an alias, then its field", "q: { r1: { a: g \"x\" }, r2: { a: g \"x\" } }\nk: q.r1\nm: k.a", 2),
    ("two alike in a list", "q: { r: [{ a: g \"x\" }, { a: g \"x\" }] }\nk: q.r.1.a", 2),
    ("two alike in a tuple", "q: { r: ({ a: g \"x\" }, { a: g \"x\" }) }\nk: q.r.1.a", 2),
    ("two alike on the root", "q1: { r: { a: g \"x\" } }\nq2: { r: { a: g \"x\" } }\nk: q1.r.a", 2),
    ("two alike, a function reading one, applied twice", "q: { r1: { a: g \"x\" }, r2: { a: g \"x\" } }\nf: (x -> q.r1.a)\nk: [f 1, f 2]", 2),
    ("two alike a call makes", "mk: (s -> { a: g s })\nq: { r1: mk \"x\", r2: mk \"x\" }\nk: q.r1.a", 2),
    ("two alike, the reader in a record", "q: { r1: { a: g \"x\" }, r2: { a: g \"x\" } }\ns: { k: q.r1.a }", 2),
    ("two alike, the reader in a list", "q: { r1: { a: g \"x\" }, r2: { a: g \"x\" } }\nk: [q.r1.a, q.r1.a]", 2),
    ("two alike, a call returns a union holding one", "q: { r1: { a: g \"x\" }, r2: { a: g \"x\" } }\nf: (x -> q.r1 | 5)\nk: f 1\nm: k.a", 2),
];

// r1: (label, the records, their readers in a second evolve, bytes a commit must append)
const SPLIT: &[(&str, &str, &str, usize)] = &[
    ("two alike in two fields", "q: { r1: { a: g \"x\" }, r2: { a: g \"x\" } }", "k: q.r1.a", 2),
    ("two alike in a list", "q: { r: [{ a: g \"x\" }, { a: g \"x\" }] }", "k: q.r.1.a", 2),
    ("two alike on the root", "q1: { r: { a: g \"x\" } }\nq2: { r: { a: g \"x\" } }", "k: q1.r.a", 2),
    ("two alike, an alias, then its field", "q: { r1: { a: g \"x\" }, r2: { a: g \"x\" } }", "k: q.r1\nm: k.a", 2),
];

// r2: (label, written, the reading, what it reads)
const ONE: &[(&str, &str, &str, &str)] = &[
    ("random, two alike: k = q.r1.a",
     "q: { r1: { a: ~%Math./random () }, r2: { a: ~%Math./random () } }\nk: q.r1.a\ne: k = q.r1.a", "_.e", "#true ;; %effect: #nondet"),
    ("random, two alike stay two: k = q.r2.a",
     "q: { r1: { a: ~%Math./random () }, r2: { a: ~%Math./random () } }\nk: q.r1.a\ne: k = q.r2.a", "_.e", "#false ;; %effect: #nondet"),
    ("random, two alike in a list: k = q.r.1.a",
     "q: { r: [{ a: ~%Math./random () }, { a: ~%Math./random () }] }\nk: q.r.1.a\ne: k = q.r.1.a", "_.e", "#true ;; %effect: #nondet"),
    ("random, two alike, through a function: f 1 = q.r1.a",
     "q: { r1: { a: ~%Math./random () }, r2: { a: ~%Math./random () } }\nf: (x -> q.r1.a)\ne: f 1 = q.r1.a", "_.e", "#true ;; %effect: #nondet"),
];

// g1: (label, written, a reading); pure, or with g
const SHAPES: &[(&str, &str, &str)] = &[
    ("pure: two alike, a reader of one", "q: { r1: { a: 1 + 1 }, r2: { a: 1 + 1 } }\nk: q.r1.a", "_.k"),
    ("pure: a function reading one", "q: { r1: { a: 1 + 1 }, r2: { a: 1 + 1 } }\nf: (x -> q.r1.a)\nk: [f 1, f 2]", "_.k"),
    ("g: two alike, a reader of one", "q: { r1: { a: g \"x\" }, r2: { a: g \"x\" } }\nk: q.r1.a", "_.k"),
    ("g: two alike in a list", "q: { r: [{ a: g \"x\" }, { a: g \"x\" }] }\nk: q.r.1.a", "_.k"),
    ("g: a function reading one", "q: { r1: { a: g \"x\" }, r2: { a: g \"x\" } }\nf: (x -> q.r1.a)\nk: [f 1, f 2]", "_.k"),
    ("g: two alike a call makes", "mk: (s -> { a: g s })\nq: { r1: mk \"x\", r2: mk \"x\" }\nk: q.r1.a", "_.k"),
];

// g1: what the reference answers (measured).
const WANT_G1: &[(&str, &str)] = &[
    ("pure: two alike, a reader of one, at once", r#"0 0 | 8cc8201793b1ee2d | _.k=2"#),
    ("pure: a function reading one, at once", r#"0 0 | 059a6639d4c59cff | _.k=[2, 2]"#),
    ("g: two alike, a reader of one, at once", r#"0 0 | dac23c29727590bc | _.k=#true ;; %effect: #io"#),
    ("g: two alike, a reader of one, later", r#"0 0 0 0 0 0 | f69992129e31e2f0 | _.k=#true ;; %effect: #io"#),
    ("g: two alike, a reader of one, pinned", r#"0 0 0 0 0 0 | f69992129e31e2f0 | _.k=#true ;; %effect: #io"#),
    ("g: two alike in a list, at once", r#"0 0 | 91e4424a1a11e6db | _.k=#true ;; %effect: #io"#),
    ("g: two alike in a list, later", r#"0 0 0 0 0 0 | 94d77f9758bfe69c | _.k=#true ;; %effect: #io"#),
    ("g: two alike in a list, pinned", r#"0 0 0 0 0 0 | 94d77f9758bfe69c | _.k=#true ;; %effect: #io"#),
    ("g: a function reading one, at once", r#"0 0 | d9355654f7e2abf0 | _.k=[#true ;; %effect: #io, #true ;; %effect: #io]"#),
    ("g: a function reading one, later", r#"0 0 0 0 0 0 | 7648e0cc0a974052 | _.k=[#true ;; %effect: #io, #true ;; %effect: #io]"#),
    ("g: a function reading one, pinned", r#"0 0 0 0 0 0 | 7648e0cc0a974052 | _.k=[#true ;; %effect: #io, #true ;; %effect: #io]"#),
    ("g: two alike a call makes, at once", r#"0 0 | 51502298099f0376 | _.k=#true ;; %effect: #io"#),
    ("g: two alike a call makes, later", r#"0 0 0 0 0 0 | aca6700e5e7c9fbe | _.k=#true ;; %effect: #io"#),
    ("g: two alike a call makes, pinned", r#"0 0 0 0 0 0 | aca6700e5e7c9fbe | _.k=#true ;; %effect: #io"#),
];

struct Ws {
    _scratch: nlang_interpreter::ScratchDir,
    root: std::path::PathBuf,
    ws: std::path::PathBuf,
}

impl Ws {
    fn new(tag: &str) -> Self {
        let scratch = nlang_interpreter::ScratchDir::new(&format!("which-{tag}"));
        let root = scratch.path().to_path_buf();
        let ws = root.join("ws");
        fs::create_dir_all(&ws).unwrap();
        fs::write(ws.join("a.log"), "").unwrap();
        Ws { _scratch: scratch, root, ws }
    }

    /// Run `oo` capped at 3 GB, killed after `kill`: (output, rc). The
    /// output goes to a file, not a pipe.
    fn oo(&self, args: &[&str], kill: Duration) -> (String, i32) {
        let out_path = self.root.join("out.txt");
        let out = fs::File::create(&out_path).expect("output file");
        let err = out.try_clone().expect("output file");
        let mut cmd = Command::new("sh");
        cmd.arg("-c")
            .arg("ulimit -v 3000000; exec \"$0\" \"$@\"")
            .arg(env!("CARGO_BIN_EXE_oo"))
            .args(args)
            .current_dir(&self.ws)
            .env("OO_IDENTITY", self.root.join("identity"))
            .env("OO_NODE_HOME", self.root.join("node-home"))
            .env_remove("OO_Q117_SHADOW")
            .stdin(Stdio::null())
            .stdout(Stdio::from(out))
            .stderr(Stdio::from(err));
        let mut child = cmd.spawn().expect("oo runs");
        let start = Instant::now();
        loop {
            if child.try_wait().expect("wait").is_some() {
                break;
            }
            if start.elapsed() > kill {
                let _ = child.kill();
                let _ = child.wait();
                return ("DID NOT RETURN".into(), -1);
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let status = child.wait().expect("wait");
        let text = String::from_utf8_lossy(&fs::read(&out_path).unwrap_or_default()).to_string();
        let rc = status.code().unwrap_or(-1);
        if text.contains("panicked") {
            return ("PANIC".into(), rc);
        }
        if text.contains("stack overflow") || text.contains("memory allocation") || (rc != 0 && text.trim().is_empty()) {
            return ("CRASHED".into(), rc);
        }
        (text, rc)
    }

    fn run(&self, args: &[&str]) -> i32 {
        self.oo(args, Duration::from_secs(60)).1
    }

    fn file(&self, name: &str, src: &str) {
        fs::write(self.ws.join(name), format!("{src}\n")).unwrap();
    }

    /// Bytes an effect appended to `a.log`.
    fn bytes(&self) -> usize {
        fs::read(self.ws.join("a.log")).map(|b| b.len()).unwrap_or(0)
    }

    fn read(&self, q: &str) -> String {
        squash(&self.oo(&["eval", "--", q], Duration::from_secs(60)).0)
    }

    /// The committed root's address, last 16 hex digits.
    fn root_tail(&self) -> String {
        let head = fs::read_to_string(self.ws.join(".oo").join("HEAD")).unwrap_or_default();
        let o = self.oo(&["inspect", head.trim()], Duration::from_secs(60)).0;
        o.lines()
            .find(|l| l.contains("root:"))
            .and_then(|l| l.split_whitespace().last())
            .map(|r| r[r.len().saturating_sub(16)..].to_string())
            .unwrap_or_else(|| "-".into())
    }

    /// Written at once: (rc of evolve and commit, bytes after the evolve, after the commit).
    fn at_once(&self, src: &str) -> (String, usize, usize) {
        self.file("f.n", src);
        let e = self.run(&["evolve", "f.n"]);
        let after_evolve = self.bytes();
        let c = self.run(&["commit", "-m", "a"]);
        (format!("{e} {c}"), after_evolve, self.bytes())
    }

    /// Commit the readers — without `g`, or with `g` pure to be pinned —
    /// then write `g` as an append, plainly or by a pin; status twice;
    /// commit. (rc of each step, bytes after the two statuses, after the
    /// commit.)
    fn rewrite(&self, readers: &str, pin: bool) -> (String, usize, usize) {
        self.file("f.n", &if pin { format!("{PURE}\n{readers}") } else { readers.to_string() });
        self.file("g.n", GA);
        let mut rcs = vec![self.run(&["evolve", "f.n"]), self.run(&["commit", "-m", "a"])];
        rcs.push(if pin { self.run(&["evolve", "--pin", "--grant", "pin", "g.n"]) } else { self.run(&["evolve", "g.n"]) });
        rcs.push(self.run(&["status"]));
        rcs.push(self.run(&["status"]));
        let after_status = self.bytes();
        rcs.push(if pin { self.run(&["commit", "-m", "b", "--grant", "pin"]) } else { self.run(&["commit", "-m", "b"]) });
        let rcs = rcs.iter().map(|r| r.to_string()).collect::<Vec<_>>().join(" ");
        (rcs, after_status, self.bytes())
    }
}

fn squash(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn check(bad: &mut Vec<String>, label: &str, got: String, want: &str) {
    if got != want {
        bad.push(format!("{label}\n      got:  {got}\n      want: {want}"));
    }
}

fn report(bad: Vec<String>, of: usize) {
    assert!(bad.is_empty(), "{} of {} wrong:\n{}", bad.len(), of, bad.join("\n"));
}

// ── r1 ── each effect is done once ──

#[test]
fn r1_a_reader_of_one_of_two_alike_does_its_effect_once() {
    let mut bad = Vec::new();
    let mut n = 0;
    for (i, (label, src, want)) in ONCE.iter().enumerate() {
        n += 1;
        let w = Ws::new(&format!("r1-{i}"));
        let (rcs, e, c) = w.at_once(&format!("{GA}\n{src}"));
        check(&mut bad, &format!("{label}: rc of evolve and commit | bytes after the evolve, after the commit"),
            format!("{rcs} | {e} {c}"), &format!("0 0 | 0 {want}"));
    }
    // The readers written in a second evolve.
    for (i, (label, first, readers, want)) in SPLIT.iter().enumerate() {
        n += 1;
        let w = Ws::new(&format!("r1-split-{i}"));
        w.file("f1.n", &format!("{GA}\n{first}"));
        w.file("f2.n", readers);
        let rcs = format!("{} {} {}", w.run(&["evolve", "f1.n"]), w.run(&["evolve", "f2.n"]), w.run(&["commit", "-m", "a"]));
        check(&mut bad, &format!("{label}, its readers in a second evolve: rc of the evolves and the commit | bytes after the commit"),
            format!("{rcs} | {}", w.bytes()), &format!("0 0 0 | {want}"));
    }
    // The records committed first, the reader in a later commit.
    n += 1;
    let w = Ws::new("r1-later-commit");
    w.file("f1.n", &format!("{GA}\n{TWO}"));
    w.file("f2.n", "k: q.r1.a");
    let rcs = format!("{} {} {} {}", w.run(&["evolve", "f1.n"]), w.run(&["commit", "-m", "a"]), w.run(&["evolve", "f2.n"]), w.run(&["commit", "-m", "b"]));
    check(&mut bad, "two alike committed, then a reader of one: rc of each step | bytes after the commits", format!("{rcs} | {}", w.bytes()), "0 0 0 0 | 2");
    // g written later or pinned (v0.142.0 does these as well).
    for pin in [false, true] {
        n += 1;
        let w = Ws::new(&format!("r1-late-{pin}"));
        let (rcs, st, done) = w.rewrite(&format!("{TWO}\nk: q.r1.a"), pin);
        check(&mut bad, &format!("two alike, a reader of one, g {}: rc, bytes after two statuses, after the commit", if pin { "pinned" } else { "written later" }),
            format!("{rcs} | {st} | {done}"), "0 0 0 0 0 0 | 0 | 2");
    }
    report(bad, n);
}

// ── r2 ── one value ──

#[test]
fn r2_a_reader_of_one_of_two_alike_is_that_value() {
    let mut bad = Vec::new();
    for (i, (label, src, q, want)) in ONE.iter().enumerate() {
        let w = Ws::new(&format!("r2-{i}"));
        let (rcs, _, _) = w.at_once(src);
        check(&mut bad, &format!("{label}: rc | {q}"), format!("{rcs} | {}", w.read(q)), &format!("0 0 | {want}"));
    }
    // The reader in a second evolve.
    let w = Ws::new("r2-split");
    w.file("f1.n", "q: { r1: { a: ~%Math./random () }, r2: { a: ~%Math./random () } }");
    w.file("f2.n", "k: q.r1.a\ne: k = q.r1.a");
    let rcs = format!("{} {} {}", w.run(&["evolve", "f1.n"]), w.run(&["evolve", "f2.n"]), w.run(&["commit", "-m", "a"]));
    check(&mut bad, "random, two alike, the reader in a second evolve: rc | _.e", format!("{rcs} | {}", w.read("_.e")), "0 0 0 | #true ;; %effect: #nondet");
    report(bad, ONE.len() + 1);
}

// ── g1 ── roots and readings ──

fn g1_lines() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for (n, (label, src, q)) in SHAPES.iter().enumerate() {
        let hows: &[&str] = if label.starts_with("pure") { &["at once"] } else { &["at once", "later", "pinned"] };
        for how in hows {
            let w = Ws::new(&format!("g1-{n}-{}", how.len()));
            let rcs = if *how == "at once" {
                let src = if label.starts_with("pure") { src.to_string() } else { format!("{GA}\n{src}") };
                w.at_once(&src).0
            } else {
                w.rewrite(src, *how == "pinned").0
            };
            out.push((format!("{label}, {how}"), format!("{rcs} | {} | {q}={}", w.root_tail(), w.read(q))));
        }
    }
    out
}

#[test]
fn g1_what_is_stored_and_read() {
    let got = g1_lines();
    if std::env::var_os("Q143_PRINT").is_some() {
        for (l, g) in &got {
            println!("G1\t{l}\t{g}");
        }
    }
    let mut bad = Vec::new();
    for (label, g) in &got {
        match WANT_G1.iter().find(|(l, _)| l == label) {
            Some((_, want)) => check(&mut bad, label, g.clone(), want),
            None => bad.push(format!("{label}: no expectation\n      got:  {g}")),
        }
    }
    report(bad, got.len());
}
