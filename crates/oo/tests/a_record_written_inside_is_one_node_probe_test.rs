// Q-142 probe: a record written inside a record is one node, however it is
// reached.
//
// Q-126 (SPEC_10 §2.3.1, clause 2): in one observation a field written once
// is one value, and its effect is done once. Evolve stores a definition that
// reads a nested field, `k: q.r.a`, as the recipe of that field, closing over
// the frames it was written in: `q`'s and `r`'s. Read back for a commit, `q`'s
// frame is the node of the record `q` read back; `r`'s is no read-back
// record's, since `q` holds `r` as the recipe that makes it. So `k` reads `a`
// in a frame with no node, and the commit's walk of `q` makes `r` again, a new
// node, and reads `a` there: the effect is done twice, and a `random` there
// is two values.
//
// v0.141.0 (since v0.125.0, Inbox I-021): `q: { r: { a: g "x" } }`,
// `k: q.r.a` commits 2 bytes; three deep, through an alias, in a function,
// a sibling read, a record in a list or a tuple, a record a call makes: 2;
// `e: k = q.r.a` with `random` stores #false.
//
// r1 — written at once and committed, each nested field's effect is done
//      once: 1 byte (two different records in a list: 2); its function
//      written later or pinned, still once (as in v0.141.0); the same with its
//      readers written in a second evolve (v0.141.0: 2, through an alias 3,
//      one deep too — the reader closes over its own copy of `q`'s frame).
// r2 — one value: with `random`, `k = q.r.a` is #true (also with the
//      readers in a second evolve); two records written
//      alike in two fields stay two coordinates: `k: q.r1.a`, `k = q.r2.a`
//      is #false.
// g1 — roots and readings do not change, as v0.141.0: pure shapes, effect
//      shapes written at once, their function written later or pinned.
//
// Not here: two records written alike in two fields of one record (or two
// alike elements of one list, or two alike records on the root, the reader
// in a second evolve), read through one of them, still do the
// effect three times where two is right: the stored recipe does not say
// which of the two it read.

use std::fs;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const PURE: &str = "g: (s -> s)";
const GA: &str = "g: (s -> ~%Io./append_file (\"a.log\", s))";

// r1: (label, written with g, bytes a commit must append)
const ONCE: &[(&str, &str, usize)] = &[
    ("a field of a nested record, by its path", "q: { r: { a: g \"x\" } }\nk: q.r.a", 1),
    ("three deep", "q: { r: { s: { a: g \"x\" } } }\nk: q.r.s.a", 1),
    ("an alias of the nested record, then its field", "q: { r: { a: g \"x\" } }\nk: q.r\nm: k.a", 1),
    ("two readers by the path", "q: { r: { a: g \"x\" } }\nk: q.r.a\nm: q.r.a", 1),
    ("a function reading the path, applied twice", "q: { r: { a: g \"x\" } }\nf: (x -> q.r.a)\nk: [f 1, f 2]", 1),
    ("a sibling that reads it", "q: { r: { a: g \"x\", b: a } }\nk: q.r.b", 1),
    ("a sibling three deep", "q: { r: { s: { a: g \"x\", b: a } } }\nk: q.r.s.b", 1),
    ("beside a private field", "q: { r: { ~p: 1, a: g \"x\" } }\nk: q.r.a", 1),
    ("a record in a list", "q: { r: [{ a: g \"x\" }] }\nk: q.r.0.a", 1),
    ("a record in a tuple", "q: { r: ({ a: g \"x\" }, 1) }\nk: q.r.0.a", 1),
    ("a record in a record in a list", "q: { r: [{ s: { a: g \"x\" } }] }\nk: q.r.0.s.a", 1),
    ("a nested record a call makes", "mk: (s -> { r: { a: g s } })\nq: mk \"x\"\nk: q.r.a", 1),
    ("two different records in a list (2)", "q: { r: [{ a: g \"x\" }, { a: g \"y\" }] }\nk: q.r.1.a", 2),
];

// r1: (label, the record, its readers in a second evolve, bytes a commit must append)
const SPLIT: &[(&str, &str, &str, usize)] = &[
    ("a field one deep", "q: { a: g \"x\" }", "k: q.a", 1),
    ("a field of a nested record", "q: { r: { a: g \"x\" } }", "k: q.r.a", 1),
    ("three deep", "q: { r: { s: { a: g \"x\" } } }", "k: q.r.s.a", 1),
    ("an alias of the nested record, then its field", "q: { r: { a: g \"x\" } }", "k: q.r\nm: k.a", 1),
    ("a record in a list", "q: { r: [{ a: g \"x\" }] }", "k: q.r.0.a", 1),
    ("a union holding the record, an alias, then its field", "q: { r: { a: g \"x\" } | 5 }", "k: q.r\nm: k.a", 1),
];

// r2: (label, written, the reading, what it reads)
const ONE: &[(&str, &str, &str, &str)] = &[
    ("random: k = q.r.a", "q: { r: { a: ~%Math./random () } }\nk: q.r.a\ne: k = q.r.a", "_.e", "#true ;; %effect: #nondet"),
    ("random three deep", "q: { r: { s: { a: ~%Math./random () } } }\nk: q.r.s.a\ne: k = q.r.s.a", "_.e", "#true ;; %effect: #nondet"),
    ("random through an alias", "q: { r: { a: ~%Math./random () } }\nk: q.r\nm: k.a\ne: m = q.r.a", "_.e", "#true ;; %effect: #nondet"),
    ("random in a list", "q: { r: [{ a: ~%Math./random () }] }\nk: q.r.0.a\ne: k = q.r.0.a", "_.e", "#true ;; %effect: #nondet"),
    ("two alike in two fields stay two: k: q.r1.a, k = q.r2.a",
     "q: { r1: { a: ~%Math./random () }, r2: { a: ~%Math./random () } }\nk: q.r1.a\ne: k = q.r2.a", "_.e", "#false ;; %effect: #nondet"),
];

// g1: (label, written, a reading); pure, or with g
const SHAPES: &[(&str, &str, &str)] = &[
    ("pure: a field of a nested record", "q: { r: { a: 1 + 1 } }\nk: q.r.a", "_.k"),
    ("pure: a sibling", "q: { r: { a: 1 + 1, b: a } }\nk: q.r.b\nm: q.r", "_.m"),
    ("pure: in a list", "q: { r: [{ a: 1 + 1 }] }\nk: q.r.0.a", "_.k"),
    ("g: a field of a nested record", "q: { r: { a: g \"x\" } }\nk: q.r.a", "_.k"),
    ("g: an alias, then its field", "q: { r: { a: g \"x\" } }\nk: q.r\nm: k.a", "_.m"),
    ("g: a function", "q: { r: { a: g \"x\" } }\nf: (x -> q.r.a)\nk: [f 1, f 2]", "_.k"),
    ("g: in a list", "q: { r: [{ a: g \"x\" }] }\nk: q.r.0.a", "_.k"),
    ("g: a call makes it", "mk: (s -> { r: { a: g s } })\nq: mk \"x\"\nk: q.r.a", "_.k"),
];

// g1: what v0.141.0 answers (measured on its tag build).
const WANT_G1: &[(&str, &str)] = &[
    ("pure: a field of a nested record, at once", r#"0 0 | f96a1bf6adb9311b | _.k=2"#),
    ("pure: a sibling, at once", r#"0 0 | a605e69e16063913 | _.m={ a: 2 b: 2 }"#),
    ("pure: in a list, at once", r#"0 0 | 4f74a148b1a2e462 | _.k=2"#),
    ("g: a field of a nested record, at once", r#"0 0 | da6d2e33e8aef155 | _.k=#true ;; %effect: #io"#),
    ("g: a field of a nested record, later", r#"0 0 0 0 0 0 | a764a170f1c491d1 | _.k=#true ;; %effect: #io"#),
    ("g: a field of a nested record, pinned", r#"0 0 0 0 0 0 | a764a170f1c491d1 | _.k=#true ;; %effect: #io"#),
    ("g: an alias, then its field, at once", r#"0 0 | d82f6f7bce0e34c9 | _.m=#true ;; %effect: #io"#),
    ("g: an alias, then its field, later", r#"0 0 0 0 0 0 | a28ead8f555c3509 | _.m=#true ;; %effect: #io"#),
    ("g: an alias, then its field, pinned", r#"0 0 0 0 0 0 | a28ead8f555c3509 | _.m=#true ;; %effect: #io"#),
    ("g: a function, at once", r#"0 0 | 573c31849b2bc28b | _.k=[#true ;; %effect: #io, #true ;; %effect: #io]"#),
    ("g: a function, later", r#"0 0 0 0 0 0 | 6041495d1d90cbf2 | _.k=[#true ;; %effect: #io, #true ;; %effect: #io]"#),
    ("g: a function, pinned", r#"0 0 0 0 0 0 | 6041495d1d90cbf2 | _.k=[#true ;; %effect: #io, #true ;; %effect: #io]"#),
    ("g: in a list, at once", r#"0 0 | 3e4d4096698915aa | _.k=#true ;; %effect: #io"#),
    ("g: in a list, later", r#"0 0 0 0 0 0 | 00c6311cedb0bc89 | _.k=#true ;; %effect: #io"#),
    ("g: in a list, pinned", r#"0 0 0 0 0 0 | 00c6311cedb0bc89 | _.k=#true ;; %effect: #io"#),
    ("g: a call makes it, at once", r#"0 0 | 205efc8ccab05a5a | _.k=#true ;; %effect: #io"#),
    ("g: a call makes it, later", r#"0 0 0 0 0 0 | 5bd5faae4d6b4d01 | _.k=#true ;; %effect: #io"#),
    ("g: a call makes it, pinned", r#"0 0 0 0 0 0 | 5bd5faae4d6b4d01 | _.k=#true ;; %effect: #io"#),
];

struct Ws {
    _scratch: nlang_interpreter::ScratchDir,
    root: std::path::PathBuf,
    ws: std::path::PathBuf,
}

impl Ws {
    fn new(tag: &str) -> Self {
        let scratch = nlang_interpreter::ScratchDir::new(&format!("inside-{tag}"));
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

// ── r1 ── each nested field's effect is done once ──

#[test]
fn r1_a_nested_field_does_its_effect_once() {
    let mut bad = Vec::new();
    let mut n = 0;
    for (i, (label, src, want)) in ONCE.iter().enumerate() {
        n += 1;
        let w = Ws::new(&format!("r1-{i}"));
        let (rcs, e, c) = w.at_once(&format!("{GA}\n{src}"));
        check(&mut bad, &format!("{label}: rc of evolve and commit | bytes after the evolve, after the commit"),
            format!("{rcs} | {e} {c}"), &format!("0 0 | 0 {want}"));
    }
    // Its function written later or pinned (v0.141.0 does these once already).
    for (i, readers) in ["q: { r: { a: g \"x\" } }\nk: q.r.a", "q: { r: { a: g \"x\" } }\nk: q.r\nm: k.a", "q: { r: [{ a: g \"x\" }] }\nk: q.r.0.a"].iter().enumerate() {
        for pin in [false, true] {
            n += 1;
            let w = Ws::new(&format!("r1-late-{i}-{pin}"));
            let (rcs, st, done) = w.rewrite(readers, pin);
            check(&mut bad, &format!("{readers:?}, g {}: rc, bytes after two statuses, after the commit", if pin { "pinned" } else { "written later" }),
                format!("{rcs} | {st} | {done}"), "0 0 0 0 0 0 | 0 | 1");
        }
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
    report(bad, n);
}

// ── r2 ── one value ──

#[test]
fn r2_a_nested_field_is_one_value() {
    let mut bad = Vec::new();
    for (i, (label, src, q, want)) in ONE.iter().enumerate() {
        let w = Ws::new(&format!("r2-{i}"));
        let (rcs, _, _) = w.at_once(src);
        check(&mut bad, &format!("{label}: rc | {q}"), format!("{rcs} | {}", w.read(q)), &format!("0 0 | {want}"));
    }
    // Its readers in a second evolve.
    let split: &[(&str, &str, &str, &str)] = &[
        ("random: k = q.r.a", "q: { r: { a: ~%Math./random () } }", "k: q.r.a\ne: k = q.r.a", "#true ;; %effect: #nondet"),
        ("two alike on the root stay two: k: q2.r.a, k = q1.r.a",
         "q1: { r: { a: ~%Math./random () } }\nq2: { r: { a: ~%Math./random () } }", "k: q2.r.a\ne: k = q1.r.a", "#false ;; %effect: #nondet"),
    ];
    for (i, (label, first, readers, want)) in split.iter().enumerate() {
        let w = Ws::new(&format!("r2-split-{i}"));
        w.file("f1.n", first);
        w.file("f2.n", readers);
        let rcs = format!("{} {} {}", w.run(&["evolve", "f1.n"]), w.run(&["evolve", "f2.n"]), w.run(&["commit", "-m", "a"]));
        check(&mut bad, &format!("{label}, its readers in a second evolve: rc | _.e"), format!("{rcs} | {}", w.read("_.e")), &format!("0 0 0 | {want}"));
    }
    report(bad, ONE.len() + split.len());
}

// ── g1 ── roots and readings, as v0.141.0 ──

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
fn g1_what_is_stored_and_read_does_not_change() {
    let got = g1_lines();
    if std::env::var_os("Q142_PRINT").is_some() {
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
