// Q-140 probe: a field read back is one value.
//
// Q-126 (SPEC_10 §2.3.1, clause 2): in one observation a written field is
// one value — whoever reads it, however many times, its effect is done once.
// A commit observes the root it stores. When that root is HEAD's read back
// from the store with a later write over it, a field of the root closes over
// no frame: what forcing it did is kept nowhere (Q-126 keeps it on the node
// the field was written on), and the coordinate cache keeps no value that did
// an effect. Each reader forces it again and does its effect again.
//
// `k: g "x"` and `a: { p: k, q: k }` committed, then `g` written as an
// append (plainly, or by a pin) and committed: 3 bytes in v0.139.0 (written
// at once: 1); `l: [k, k, k]`: 4. v0.122.0 the same (Inbox I-322).
//
// r1 — readers of a field of a root read back, its function written later or
//      pinned: each written effect is done once, status does none (eleven
//      shapes; two fields written alike are two).
// r2 — keeping a field of the root costs what the field does: a chain of
//      lists committed at N and 4N in less than six times the time.
// r3 — only a commit's projection keys a field on the root: the readers of a
//      name in one evolve, the name in the next, that evolve in less than
//      three times the commit's time.
// g1 — roots and readings do not change, as v0.139.0: written at once, its
//      function written later, pinned.
//
// Not here: a field of a nested record reached through the commit's walk and
// through a reader's path is two records when written at once
// (`q: { r: { a: g "x" } }`, `k: q.r.a`: 2 bytes; Inbox I-021).

use std::fs;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const PURE: &str = "g: (s -> s)";
const GA: &str = "g: (s -> ~%Io./append_file (\"a.log\", s))";

// (label, readers, a reading, written effects)
const SHAPES: &[(&str, &str, &str, usize)] = &[
    ("two readers of one", "k: g \"x\"\na: { p: k, q: k }", "_.a.q", 1),
    ("a call makes a record, an alias", "mk: (s -> { e: g s })\nk: mk \"x\"\nk2: k", "_.k2.e", 1),
    ("a chain of calls", "id: (x -> [x])\nk0: id k1\nk1: id k2\nk2: g \"x\"", "_.k0", 1),
    ("three times in a list", "k: g \"x\"\nl: [k, k, k]", "_.l.2", 1),
    ("a reader of a reader", "k: g \"x\"\nm: k\nn: m", "_.n", 1),
    ("read in a function body", "k: g \"x\"\nf: (x -> k)\nm: [f 1, f 2]", "_.m.1", 1),
    ("a record's field read twice", "k: { e: g \"x\" }\nm: k.e\nn: k.e", "_.n", 1),
    ("two cells alike, each read", "k1: g \"x\"\nk2: g \"x\"\na: [k1, k2, k1]", "_.a.2", 2),
    ("a record a call makes, read twice", "mk: (s -> { e: g s })\nk: mk \"x\"\na: k.e\nb: k.e", "_.b", 1),
    ("read through a nested path", "k: g \"x\"\nq: { r: { a: k } }\nm: q.r.a", "_.m", 1),
    ("a record and its field, both read", "k: g \"x\"\na: { p: k }\nb: a.p\nc: [a, b]", "_.c.1", 1),
];

// g1: what v0.139.0 answers (measured on its tag build).
const WANT_G1: &[(&str, &str)] = &[
    ("two readers of one, g at once", r#"0 0 | 35817a6f50a073bb | _.a.q=#true ;; %effect: #io"#),
    ("two readers of one, g later", r#"0 0 0 0 0 0 | 94899767c1759760 | _.a.q=#true ;; %effect: #io"#),
    ("two readers of one, g pinned", r#"0 0 0 0 0 0 | 94899767c1759760 | _.a.q=#true ;; %effect: #io"#),
    ("a call makes a record, an alias, g at once", r#"0 0 | b79341b15a08dae2 | _.k2.e=#true ;; %effect: #io"#),
    ("a call makes a record, an alias, g later", r#"0 0 0 0 0 0 | 6e94881c96847376 | _.k2.e=#true ;; %effect: #io"#),
    ("a call makes a record, an alias, g pinned", r#"0 0 0 0 0 0 | 6e94881c96847376 | _.k2.e=#true ;; %effect: #io"#),
    ("a chain of calls, g at once", r#"0 0 | fc010c5995233738 | _.k0=[[#true ;; %effect: #io]]"#),
    ("a chain of calls, g later", r#"0 0 0 0 0 0 | fc010c5995233738 | _.k0=[[#true ;; %effect: #io]]"#),
    ("a chain of calls, g pinned", r#"0 0 0 0 0 0 | fc010c5995233738 | _.k0=[[#true ;; %effect: #io]]"#),
    ("three times in a list, g at once", r#"0 0 | 1cdc0eeba2931818 | _.l.2=#true ;; %effect: #io"#),
    ("three times in a list, g later", r#"0 0 0 0 0 0 | 00a1ad1598f2c365 | _.l.2=#true ;; %effect: #io"#),
    ("three times in a list, g pinned", r#"0 0 0 0 0 0 | 00a1ad1598f2c365 | _.l.2=#true ;; %effect: #io"#),
    ("a reader of a reader, g at once", r#"0 0 | 022a4adfe8c25c6e | _.n=#true ;; %effect: #io"#),
    ("a reader of a reader, g later", r#"0 0 0 0 0 0 | 5eee3542cd665722 | _.n=#true ;; %effect: #io"#),
    ("a reader of a reader, g pinned", r#"0 0 0 0 0 0 | 5eee3542cd665722 | _.n=#true ;; %effect: #io"#),
    ("read in a function body, g at once", r#"0 0 | 3ff048d414000cc2 | _.m.1=#true ;; %effect: #io"#),
    ("read in a function body, g later", r#"0 0 0 0 0 0 | d8b700454e48221d | _.m.1=#true ;; %effect: #io"#),
    ("read in a function body, g pinned", r#"0 0 0 0 0 0 | d8b700454e48221d | _.m.1=#true ;; %effect: #io"#),
    ("a record's field read twice, g at once", r#"0 0 | 15c376d54efec05e | _.n=#true ;; %effect: #io"#),
    ("a record's field read twice, g later", r#"0 0 0 0 0 0 | 91b6bf83c89b7174 | _.n=#true ;; %effect: #io"#),
    ("a record's field read twice, g pinned", r#"0 0 0 0 0 0 | 91b6bf83c89b7174 | _.n=#true ;; %effect: #io"#),
    ("two cells alike, each read, g at once", r#"0 0 | 95f325883cda8373 | _.a.2=#true ;; %effect: #io"#),
    ("two cells alike, each read, g later", r#"0 0 0 0 0 0 | 7a461b37610047b4 | _.a.2=#true ;; %effect: #io"#),
    ("two cells alike, each read, g pinned", r#"0 0 0 0 0 0 | 7a461b37610047b4 | _.a.2=#true ;; %effect: #io"#),
    ("a record a call makes, read twice, g at once", r#"0 0 | cdab78d485467882 | _.b=#true ;; %effect: #io"#),
    ("a record a call makes, read twice, g later", r#"0 0 0 0 0 0 | f28d852977108566 | _.b=#true ;; %effect: #io"#),
    ("a record a call makes, read twice, g pinned", r#"0 0 0 0 0 0 | f28d852977108566 | _.b=#true ;; %effect: #io"#),
    ("read through a nested path, g at once", r#"0 0 | 4c76f329315b3ff7 | _.m=#true ;; %effect: #io"#),
    ("read through a nested path, g later", r#"0 0 0 0 0 0 | cc2b05028595df23 | _.m=#true ;; %effect: #io"#),
    ("read through a nested path, g pinned", r#"0 0 0 0 0 0 | cc2b05028595df23 | _.m=#true ;; %effect: #io"#),
    ("a record and its field, both read, g at once", r#"0 0 | bbdf3a1b61b6b0cb | _.c.1=#true ;; %effect: #io"#),
    ("a record and its field, both read, g later", r#"0 0 0 0 0 0 | 8fd6e2f8ca956c81 | _.c.1=#true ;; %effect: #io"#),
    ("a record and its field, both read, g pinned", r#"0 0 0 0 0 0 | 8fd6e2f8ca956c81 | _.c.1=#true ;; %effect: #io"#),
];

struct Ws {
    _scratch: nlang_interpreter::ScratchDir,
    root: std::path::PathBuf,
    ws: std::path::PathBuf,
}

impl Ws {
    fn new(tag: &str) -> Self {
        let scratch = nlang_interpreter::ScratchDir::new(&format!("read-back-{tag}"));
        let root = scratch.path().to_path_buf();
        let ws = root.join("ws");
        fs::create_dir_all(&ws).unwrap();
        Ws { _scratch: scratch, root, ws }
    }

    /// Run `oo` capped at 3 GB, killed after `kill`: (output, rc, seconds).
    /// The output goes to a file, not a pipe.
    fn oo(&self, args: &[&str], kill: Duration) -> (String, i32, f64) {
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
                return ("DID NOT RETURN".into(), -1, start.elapsed().as_secs_f64());
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let secs = start.elapsed().as_secs_f64();
        let status = child.wait().expect("wait");
        let text = String::from_utf8_lossy(&fs::read(&out_path).unwrap_or_default()).to_string();
        let rc = status.code().unwrap_or(-1);
        if text.contains("panicked") {
            return ("PANIC".into(), rc, secs);
        }
        if text.contains("stack overflow") || text.contains("memory allocation") || (rc != 0 && text.trim().is_empty()) {
            return ("CRASHED".into(), rc, secs);
        }
        (text, rc, secs)
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
        let (o, _, _) = self.oo(&["inspect", head.trim()], Duration::from_secs(60));
        o.lines()
            .find(|l| l.contains("root:"))
            .and_then(|l| l.split_whitespace().last())
            .map(|r| r[r.len().saturating_sub(16)..].to_string())
            .unwrap_or_else(|| "-".into())
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

// ── r1 ── readers of a field read back, each written effect once ──

#[test]
fn r1_a_field_read_back_is_one_value_to_its_readers() {
    let mut bad = Vec::new();
    let mut n = 0;
    for (label, readers, _, written) in SHAPES {
        for pin in [false, true] {
            n += 1;
            let how = if pin { "pinned" } else { "written later" };
            let w = Ws::new(&format!("r1-{n}"));
            let (rcs, st, done) = w.rewrite(readers, pin);
            check(&mut bad, &format!("{label}, g {how}: rc, bytes after two statuses, after the commit"),
                format!("{rcs} | {st} | {done}"), &format!("0 0 0 0 0 0 | 0 | {written}"));
        }
    }
    report(bad, n);
}

// ── r2 ── keeping a field of the root costs what the field does ──

fn list_chain_commit(n: usize) -> (i32, f64) {
    let w = Ws::new(&format!("r2-{n}"));
    let mut src: String = (0..n).map(|i| format!("k{i}: [k{}]\n", i + 1)).collect();
    src.push_str(&format!("k{n}: 7"));
    w.file("f.n", &src);
    let e = w.oo(&["evolve", "f.n"], Duration::from_secs(120));
    if e.1 != 0 {
        return (e.1, e.2);
    }
    let c = w.oo(&["commit", "-m", "a"], Duration::from_secs(120));
    (c.1, c.2)
}

#[test]
fn r2_a_chain_of_lists_commits_in_its_length() {
    // v0.139.0: 3.9 s at 250, 16.4 at 1000 (x4.2); the reference 2.0 / 7.7
    // (x3.9); a form of it that also took a thunk whose one frame was the
    // root for a field put under the root: 2.5 / 20.6 (x8.2).
    let (rc1, t1) = list_chain_commit(250);
    let (rc4, t4) = list_chain_commit(1000);
    let ok = rc1 == 0 && rc4 == 0 && t4 < 6.0 * t1 + 0.5;
    let got = format!("rc {rc1} {t1:.2}s / rc {rc4} {t4:.2}s (x{:.1})", t4 / t1.max(0.001));
    report(if ok { Vec::new() } else { vec![format!("a chain of lists, N=250 and 1000: want rc 0 and under x6\n      got:  {got}")] }, 1);
}

// ── r3 ── a write does not pay for what the commit keeps ──

#[test]
fn r3_a_write_does_not_key_fields_on_the_root() {
    // v0.139.0 and the reference: the name's evolve about the commit's time
    // (1.63 / 1.46 s at N=2000); keyed on the root in evolve too: 7.9 / 1.3.
    let n = 2000;
    let w = Ws::new("r3");
    let readers: String = (0..n).map(|i| format!("a{i}: [b{i}]\nb{i}: [c{i}]\nc{i}: [p]\n")).collect();
    w.file("f.n", &readers);
    w.file("p.n", "p: ~%Env.get \"Q140_UNSET\"");
    let e1 = w.oo(&["evolve", "f.n"], Duration::from_secs(120));
    let e2 = w.oo(&["evolve", "p.n"], Duration::from_secs(120));
    let c = w.oo(&["commit", "-m", "a"], Duration::from_secs(120));
    let ok = e1.1 == 0 && e2.1 == 0 && c.1 == 0 && e2.2 < 3.0 * c.2 + 0.5;
    let got = format!("rc {} {} {}; the name's evolve {:.2}s, the commit {:.2}s (x{:.1})", e1.1, e2.1, c.1, e2.2, c.2, e2.2 / c.2.max(0.001));
    report(if ok { Vec::new() } else { vec![format!("readers then the name, N={n}: want rc 0 and under x3\n      got:  {got}")] }, 1);
}

// ── g1 ── roots and readings, as v0.139.0 ──

fn g1_lines() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for (n, (label, readers, q, _)) in SHAPES.iter().enumerate() {
        for how in ["at once", "later", "pinned"] {
            let w = Ws::new(&format!("g1-{n}-{}", how.len()));
            let rcs = if how == "at once" {
                w.file("f.n", &format!("{GA}\n{readers}"));
                format!("{} {}", w.run(&["evolve", "f.n"]), w.run(&["commit", "-m", "a"]))
            } else {
                w.rewrite(readers, how == "pinned").0
            };
            out.push((format!("{label}, g {how}"), format!("{rcs} | {} | {q}={}", w.root_tail(), w.read(q))));
        }
    }
    out
}

#[test]
fn g1_what_is_stored_and_read_does_not_change() {
    let got = g1_lines();
    if std::env::var_os("Q140_PRINT").is_some() {
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
