// Q-139 probe: a written effect is done once.
//
// Q-126 (SPEC_10 §2.3.1, D131): an effect is done by an observation, never
// by bookkeeping, and a definition predicted pure that does one when it is
// observed keeps its answer. Two paths in v0.138.0 do it more than once:
//
// *   the commit asks each predicted-pure field whether working it out does
//     an effect (Q-138) and, when it did, works it out again to store that
//     value — doing the effect a second time. Commit `k: g "x"` before `g`
//     is written, then write `g` as an append and commit: 2 bytes
//     (v0.137.0: 1);
// *   judging whether a pin is already held (D84 (i), Q-123) observes the
//     merged root as the commit would — at every load: after a pin evolve
//     of that `g`, each `oo status` appends (v0.137.0 once, v0.138.0
//     twice) and the pin commit appends 2 (v0.137.0) or 4 (v0.138.0) more.
//     v0.122.0, before the judgement: status 0, commit 1.
//
// The judgement is also where a ring through a call goes back to 2^L: its
// second projection reads the records the first one kept as one value
// (Q-126), from another root, and no node is met again. A call ring
// committed, then `x` pinned: status 8.6 s at L=8, no return at L=16
// (v0.138.0); v0.137.0 no return at L=8.
//
// r1 — its function written later or pinned, the commit does each written
//      effect once and status does none (eight shapes).
// r2 — a pin beside a ring through a call (L=64) is judged and committed
//      within its bound.
// g1 — roots and readings do not change, as v0.137.0: written at once, its
//      function written later, pinned.
//
// Not here: a reader of a stored recipe whose answer is first observed at
// this commit does the effect again (`k: g "x"`, `a: { p: k, q: k }`,
// `g` written later: 3 bytes; written at once: 1) — v0.122.0 the same (Inbox
// I-322). Those shapes are in g1 for their roots only.

use std::fs;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const PURE: &str = "g: (s -> s)";
const GA: &str = "g: (s -> ~%Io./append_file (\"a.log\", s))";

// (label, readers, a reading, written effects)
const SHAPES: &[(&str, &str, &str, usize)] = &[
    ("one", "k: g \"x\"", "_.k", 1),
    ("a record twice in a list", "a: { p: g \"x\" }\nb: [a, a]", "_.b.1.p", 1),
    ("a ring, two ways", "n0: { e: g \"x\", w0: n1, w1: n1 }\nn1: { w0: n2, w1: n2 }\nn2: { w0: n0, w1: n0 }", "_.n1.w1.w0.e", 1),
    ("through id", "id: (x -> x)\nk: id (g \"x\")", "_.k", 1),
    ("two cells alike", "k1: g \"x\"\nk2: g \"x\"", "_.k2", 2),
    ("a list", "l: [g \"x\", g \"y\"]", "_.l.1", 2),
    ("a call makes a record, twice", "mk: (s -> { e: g s })\nk: mk \"x\"\nk2: mk \"x\"", "_.k2.e", 2),
    ("a record, a field read", "k: { a: g \"x\", b: g \"y\" }\nr: k.a", "_.r", 2),
];

// g1 only (I-322: their counts are not this card's).
const READERS: &[(&str, &str, &str)] = &[
    ("two readers of one", "k: g \"x\"\na: { p: k, q: k }", "_.a.q"),
    ("a call makes a record, an alias", "mk: (s -> { e: g s })\nk: mk \"x\"\nk2: k", "_.k2.e"),
    ("a chain of calls", "id: (x -> [x])\nk0: id k1\nk1: id k2\nk2: g \"x\"", "_.k0"),
];

// g1: what v0.137.0 answers (measured on its tag build).
const WANT_G1: &[(&str, &str)] = &[
    ("one, g at once", r#"0 0 | f78a6f3aa644d2ff | _.k=#true ;; %effect: #io"#),
    ("one, g later", r#"0 0 0 0 0 0 | f78a6f3aa644d2ff | _.k=#true ;; %effect: #io"#),
    ("one, g pinned", r#"0 0 0 0 0 0 | f78a6f3aa644d2ff | _.k=#true ;; %effect: #io"#),
    ("a record twice in a list, g at once", r#"0 0 | 4507ebec386c7e8a | _.b.1.p=#true ;; %effect: #io"#),
    ("a record twice in a list, g later", r#"0 0 0 0 0 0 | 38dc8ddf91080ec7 | _.b.1.p=#true ;; %effect: #io"#),
    ("a record twice in a list, g pinned", r#"0 0 0 0 0 0 | 38dc8ddf91080ec7 | _.b.1.p=#true ;; %effect: #io"#),
    ("a ring, two ways, g at once", r#"0 0 | d9b74715b69800fe | _.n1.w1.w0.e=#true ;; %effect: #io"#),
    ("a ring, two ways, g later", r#"0 0 0 0 0 0 | a73e577a96832866 | _.n1.w1.w0.e=#true ;; %effect: #io"#),
    ("a ring, two ways, g pinned", r#"0 0 0 0 0 0 | a73e577a96832866 | _.n1.w1.w0.e=#true ;; %effect: #io"#),
    ("through id, g at once", r#"0 0 | 9863eb5254028394 | _.k=#true ;; %effect: #io"#),
    ("through id, g later", r#"0 0 0 0 0 0 | 9863eb5254028394 | _.k=#true ;; %effect: #io"#),
    ("through id, g pinned", r#"0 0 0 0 0 0 | 9863eb5254028394 | _.k=#true ;; %effect: #io"#),
    ("two cells alike, g at once", r#"0 0 | 1d7756478bd32521 | _.k2=#true ;; %effect: #io"#),
    ("two cells alike, g later", r#"0 0 0 0 0 0 | 1d7756478bd32521 | _.k2=#true ;; %effect: #io"#),
    ("two cells alike, g pinned", r#"0 0 0 0 0 0 | 1d7756478bd32521 | _.k2=#true ;; %effect: #io"#),
    ("a list, g at once", r#"0 0 | e24cf16b673491aa | _.l.1=#true ;; %effect: #io"#),
    ("a list, g later", r#"0 0 0 0 0 0 | e24cf16b673491aa | _.l.1=#true ;; %effect: #io"#),
    ("a list, g pinned", r#"0 0 0 0 0 0 | e24cf16b673491aa | _.l.1=#true ;; %effect: #io"#),
    ("a call makes a record, twice, g at once", r#"0 0 | 4de87a1a4ed9fd64 | _.k2.e=#true ;; %effect: #io"#),
    ("a call makes a record, twice, g later", r#"0 0 0 0 0 0 | 6e94881c96847376 | _.k2.e=#true ;; %effect: #io"#),
    ("a call makes a record, twice, g pinned", r#"0 0 0 0 0 0 | 6e94881c96847376 | _.k2.e=#true ;; %effect: #io"#),
    ("a record, a field read, g at once", r#"0 0 | eadec372e057f02c | _.r=#true ;; %effect: #io"#),
    ("a record, a field read, g later", r#"0 0 0 0 0 0 | eadec372e057f02c | _.r=#true ;; %effect: #io"#),
    ("a record, a field read, g pinned", r#"0 0 0 0 0 0 | eadec372e057f02c | _.r=#true ;; %effect: #io"#),
    ("two readers of one, g at once", r#"0 0 | 35817a6f50a073bb | _.a.q=#true ;; %effect: #io"#),
    ("two readers of one, g later", r#"0 0 0 0 0 0 | 94899767c1759760 | _.a.q=#true ;; %effect: #io"#),
    ("two readers of one, g pinned", r#"0 0 0 0 0 0 | 94899767c1759760 | _.a.q=#true ;; %effect: #io"#),
    ("a call makes a record, an alias, g at once", r#"0 0 | b79341b15a08dae2 | _.k2.e=#true ;; %effect: #io"#),
    ("a call makes a record, an alias, g later", r#"0 0 0 0 0 0 | 6e94881c96847376 | _.k2.e=#true ;; %effect: #io"#),
    ("a call makes a record, an alias, g pinned", r#"0 0 0 0 0 0 | 6e94881c96847376 | _.k2.e=#true ;; %effect: #io"#),
    ("a chain of calls, g at once", r#"0 0 | fc010c5995233738 | _.k0=[[#true ;; %effect: #io]]"#),
    ("a chain of calls, g later", r#"0 0 0 0 0 0 | fc010c5995233738 | _.k0=[[#true ;; %effect: #io]]"#),
    ("a chain of calls, g pinned", r#"0 0 0 0 0 0 | fc010c5995233738 | _.k0=[[#true ;; %effect: #io]]"#),
];

struct Ws {
    _scratch: nlang_interpreter::ScratchDir,
    root: std::path::PathBuf,
    ws: std::path::PathBuf,
}

impl Ws {
    fn new(tag: &str) -> Self {
        let scratch = nlang_interpreter::ScratchDir::new(&format!("effect-once-{tag}"));
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

// ── r1 ── rewritten plainly or by a pin, each written effect once ──

#[test]
fn r1_a_written_effect_is_done_once_and_status_does_none() {
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

// ── r2 ── a pin beside a ring through a call ──

#[test]
fn r2_a_pin_beside_a_ring_through_a_call_is_judged_within_its_bound() {
    let mut bad = Vec::new();
    let l = 64;
    let mut src = String::from("id: (x -> x)\n");
    for i in 0..l {
        let j = (i + 1) % l;
        src.push_str(&format!("n{i}: {{ v: {i}, c0: id n{j}, c1: id n{j} }}\n"));
    }
    src.push_str("x: 0");
    let w = Ws::new("r2");
    w.file("f.n", &src);
    w.file("p.n", "x: 1");
    let e = w.oo(&["evolve", "f.n"], Duration::from_secs(60));
    let c = w.oo(&["commit", "-m", "a"], Duration::from_secs(60));
    let p = w.oo(&["evolve", "--pin", "--grant", "pin", "p.n"], Duration::from_secs(60));
    check(&mut bad, "ring L=64: evolve, commit, pin evolve rc", format!("{} {} {}", e.1, c.1, p.1), "0 0 0");
    let s = w.oo(&["status"], Duration::from_secs(60));
    let pc = w.oo(&["commit", "-m", "b", "--grant", "pin"], Duration::from_secs(60));
    // The commit of the ring itself takes 0.4 s; the judgement walks it twice.
    let ok = |r: &(String, i32, f64)| if r.1 == 0 && r.2 <= 3.0 { "ok".to_string() } else { format!("{} rc={} {:.1}s", squash(&r.0).chars().take(60).collect::<String>(), r.1, r.2) };
    check(&mut bad, "status within 3 s", ok(&s), "ok");
    check(&mut bad, "pin commit within 3 s", ok(&pc), "ok");
    check(&mut bad, "_.x, _.n0.c1.v", format!("{} | {}", w.read("_.x"), w.read("_.n0.c1.v")), "1 | 1");
    report(bad, 5);
}

// ── g1 ── roots and readings, as v0.137.0 ──

fn g1_lines() -> Vec<(String, String)> {
    let mut out = Vec::new();
    let all = SHAPES.iter().map(|(l, r, q, _)| (*l, *r, *q)).chain(READERS.iter().copied());
    for (n, (label, readers, q)) in all.enumerate() {
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
    if std::env::var_os("Q139_PRINT").is_some() {
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
