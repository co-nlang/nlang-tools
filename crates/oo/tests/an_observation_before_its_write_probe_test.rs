// An observation made before its write was done.
// Order: nlang-tools/docs/an_observation_before_its_write_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-125. No new ruling: the fields of
// one write are a set (SYNTAX_03 §2 #1 「欄位順序無關」; SPEC_10 §2.2.3 「同一筆
// pin 寫入裡的欄位彼此交會、與順序無關」); an effect cell stores what it
// observed (D110, D46 ②), and the commit stores that cell's value
// (SPEC_10 §2.3.1); D110 counts `_.` as reading the universe, the same
// universe a bare name reads.
//
// ── What is wrong (v0.124.0) ────────────────────────────────────────────
//
//   Evolve evaluates a write field by field. A field whose definition
//   observes an effect (`~%Env.get`, a function that calls it) is evaluated
//   there and its answer stored. It reads the bindings visible at its line:
//   the earlier lines, the working set, HEAD — not the write it belongs to.
//   `g: (n -> ~%Env.get "HOME")`, `k: g 1` committed, then one pin write
//   `k: g 1`, `g: (n -> n)`: the commit stores `k` as the HOME value, while
//   `g: (n -> n)`, `k: g 1` is held and `k` reads 1. Plain writes too:
//   `p: "HOME" | "USER"` committed, then `k: ~%Env.get p`, `p: "HOME"` stores
//   both values; with `p` new in the write `k` stores the builtin itself and
//   the effect never runs. `_.p` at evolve reads HEAD alone: `p` written in
//   an earlier evolve of the same working set is not seen.
//
// The other half: pure fields are recipes and already order-free (roots do
// not move); a field that reads nothing this write writes is evaluated where
// it stands; a pin of what HEAD holds is held; a conflict is named at its
// coordinate; the effect runs once — not again at status, eval or commit.
//
// The probe sets Q125_A=alpha and Q125_B=beta for every `oo` it runs.
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
        let scratch = nlang_interpreter::ScratchDir::new(&format!("observe-{tag}"));
        let root = scratch.path().to_path_buf();
        let ws = root.join("ws");
        fs::create_dir_all(&ws).unwrap();
        Ws { _scratch: scratch, root, ws }
    }

    /// Run `oo` capped at 3 GB, killed after 90 s: (output, rc).
    fn oo(&self, args: &[&str]) -> (String, i32) {
        let mut child = Command::new("sh")
            .arg("-c")
            .arg("ulimit -v 3000000; exec \"$0\" \"$@\"")
            .arg(env!("CARGO_BIN_EXE_oo"))
            .args(args)
            .current_dir(&self.ws)
            .env("OO_IDENTITY", self.root.join("identity"))
            .env("OO_NODE_HOME", self.root.join("node-home"))
            .env("Q125_A", "alpha")
            .env("Q125_B", "beta")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("oo runs");
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

    fn file(&self, src: &str) -> String {
        let n = fs::read_dir(&self.ws).map(|d| d.count()).unwrap_or(0);
        let f = format!("f{n}.n");
        fs::write(self.ws.join(&f), src).unwrap();
        f
    }

    /// Evolve or pin the files `srcs` in one command: rc.
    fn step(&self, pin: bool, srcs: &[&str]) -> (String, i32) {
        let names: Vec<String> = srcs.iter().map(|s| self.file(s)).collect();
        let mut args = vec!["evolve"];
        if pin {
            args.extend(["--pin", "--grant", "pin"]);
        }
        args.extend(names.iter().map(|s| s.as_str()));
        self.oo(&args)
    }

    /// `oo commit --grant pin`: "landed", "held", or what it said.
    fn commit(&self) -> String {
        let (o, _) = self.oo(&["commit", "-m", "c", "--grant", "pin"]);
        said(&o)
    }

    /// `oo eval`, the trailing effect note dropped, whitespace collapsed.
    fn eval(&self, e: &str) -> String {
        let s = self.oo(&["eval", e]).0;
        let s = match s.find(";;") {
            Some(i) => s[..i].trim().to_string(),
            None => s,
        };
        squash(&s)
    }

    /// The committed root's digest.
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

/// One step: a pin or a plain write of one or more files in one command.
#[derive(Clone, Copy)]
enum S {
    Pin(&'static [&'static str]),
    Plain(&'static [&'static str]),
}

/// Commit `base` (if any), take `steps`, commit: (rc of each step, what the
/// commit said, the root, `read`).
fn take(tag: &str, base: &str, steps: &[S], read: &str) -> (String, String, String, String) {
    let w = Ws::new(tag);
    if !base.is_empty() {
        let (o, rc) = w.step(false, &[base]);
        let c = w.commit();
        if rc != 0 || c != "landed" {
            return (format!("SETUP {} / {c}", o.chars().take(60).collect::<String>()), String::new(), String::new(), String::new());
        }
    }
    let mut rcs = Vec::new();
    for s in steps {
        let (o, rc) = match s {
            S::Pin(f) => w.step(true, f),
            S::Plain(f) => w.step(false, f),
        };
        rcs.push(if rc == 0 { "e0".to_string() } else { format!("e{rc} {}", o.chars().take(60).collect::<String>()) });
    }
    let c = w.commit();
    (rcs.join(" "), c, w.root(), w.eval(read))
}

/// The same write taken two ways must give one root and one reading, and
/// the reading is `want`.
fn same_write(bad: &mut Vec<String>, label: &str, base: &str, a: &[S], b: &[S], read: &str, want: &str, said_want: &str) {
    let (ra, ca, roota, va) = take(&format!("{label}-a"), base, a, read);
    let (rb, cb, rootb, vb) = take(&format!("{label}-b"), base, b, read);
    let steps = a.iter().map(|_| "e0").collect::<Vec<_>>().join(" ");
    check(bad, &format!("{label}: first order, steps"), ra, &steps);
    check(bad, &format!("{label}: second order, steps"), rb, &steps);
    check(bad, &format!("{label}: first order, commit"), ca, said_want);
    check(bad, &format!("{label}: second order, commit"), cb, said_want);
    check(bad, &format!("{label}: first order, {read}"), va, want);
    check(bad, &format!("{label}: second order, {read}"), vb, want);
    if roota.len() != 64 {
        bad.push(format!("  {label}: VOID READING: no root ({roota})"));
    }
    check(bad, &format!("{label}: one root"), roota, &rootb);
}

const IO_A: &str = "g: (n -> ~%Env.get \"Q125_A\")";

// ── r1 ── a pin that rewrites a name its own observing field reads ──

#[test]
fn r1_a_pin_observes_after_the_names_it_rewrites() {
    let mut bad = Vec::new();
    let mut run: Vec<Box<dyn Fn() -> Vec<String> + Send + Sync>> = Vec::new();
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        same_write(&mut bad, "r1 working set: pin g to IO, then k, g back",
            "g: (n -> n)\nk: g 1",
            &[S::Pin(&["g: (n -> ~%Env.get \"Q125_A\")"]), S::Pin(&["k: g 1\ng: (n -> n)"])],
            &[S::Pin(&["g: (n -> ~%Env.get \"Q125_A\")"]), S::Pin(&["g: (n -> n)\nk: g 1"])],
            "_.k", "1", "held");
        bad
    }));
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        same_write(&mut bad, "r1 HEAD holds the IO name",
            "g: (n -> ~%Env.get \"Q125_A\")\nk: g 1",
            &[S::Pin(&["k: g 1\ng: (n -> n)"])], &[S::Pin(&["g: (n -> n)\nk: g 1"])],
            "_.k", "1", "landed");
        bad
    }));
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        same_write(&mut bad, "r1 two files in one pin",
            "g: (n -> ~%Env.get \"Q125_A\")\nk: g 1",
            &[S::Pin(&["k: g 1", "g: (n -> n)"])], &[S::Pin(&["g: (n -> n)", "k: g 1"])],
            "_.k", "1", "landed");
        bad
    }));
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        same_write(&mut bad, "r1 IO replaced by another IO",
            "g: (n -> ~%Env.get \"Q125_A\")\nk: g 1",
            &[S::Pin(&["k: g 1\ng: (n -> ~%Env.get \"Q125_B\")"])], &[S::Pin(&["g: (n -> ~%Env.get \"Q125_B\")\nk: g 1"])],
            "_.k", "\"beta\"", "landed");
        bad
    }));
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        same_write(&mut bad, "r1 through a record field",
            "h: { f: (n -> ~%Env.get \"Q125_A\") }\nk: h.f 1",
            &[S::Pin(&["k: h.f 1\nh.f: (n -> n)"])], &[S::Pin(&["h.f: (n -> n)\nk: h.f 1"])],
            "_.k", "1", "landed");
        bad
    }));
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        same_write(&mut bad, "r1 through _.",
            "g: (n -> ~%Env.get \"Q125_A\")\nk: g 1",
            &[S::Pin(&["k: _.g 1\ng: (n -> n)"])], &[S::Pin(&["g: (n -> n)\nk: _.g 1"])],
            "_.k", "1", "landed");
        bad
    }));
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        // Two observing fields: k reads q, which this write rewrites, and HEAD
        // holds an older q; k observes after q.
        same_write(&mut bad, "r1 a chain over the IO q HEAD holds",
            "q: ~%Env.get \"Q125_B\"",
            &[S::Pin(&["k: [q, 1]\nq: ~%Env.get p\np: \"Q125_A\""])], &[S::Pin(&["p: \"Q125_A\"\nq: ~%Env.get p\nk: [q, 1]"])],
            "_.k.0", "\"alpha\"", "landed");
        bad
    }));
    let n = cells(&mut bad, run);
    verdict(bad, n);
}

// ── r2 ── a plain write narrows or brings the name its observing field reads ──

#[test]
fn r2_a_plain_write_observes_after_the_names_it_writes() {
    let mut bad = Vec::new();
    let mut run: Vec<Box<dyn Fn() -> Vec<String> + Send + Sync>> = Vec::new();
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        same_write(&mut bad, "r2 a union narrowed",
            "p: \"Q125_A\" | \"Q125_B\"",
            &[S::Plain(&["k: ~%Env.get p\np: \"Q125_A\""])], &[S::Plain(&["p: \"Q125_A\"\nk: ~%Env.get p"])],
            "_.k", "\"alpha\"", "landed");
        bad
    }));
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        same_write(&mut bad, "r2 a type narrowed",
            "p: @string",
            &[S::Plain(&["k: ~%Env.get p\np: \"Q125_A\""])], &[S::Plain(&["p: \"Q125_A\"\nk: ~%Env.get p"])],
            "_.k", "\"alpha\"", "landed");
        bad
    }));
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        same_write(&mut bad, "r2 the name is new",
            "",
            &[S::Plain(&["k: ~%Env.get p\np: \"Q125_A\""])], &[S::Plain(&["p: \"Q125_A\"\nk: ~%Env.get p"])],
            "_.k", "\"alpha\"", "landed");
        bad
    }));
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        same_write(&mut bad, "r2 two files",
            "p: \"Q125_A\" | \"Q125_B\"",
            &[S::Plain(&["k: ~%Env.get p", "p: \"Q125_A\""])], &[S::Plain(&["p: \"Q125_A\"", "k: ~%Env.get p"])],
            "_.k", "\"alpha\"", "landed");
        bad
    }));
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        // An observing field that reads another one, which reads a name.
        same_write(&mut bad, "r2 a chain k, q, p",
            "",
            &[S::Plain(&["k: [q, 1]\nq: ~%Env.get p\np: \"Q125_A\""])], &[S::Plain(&["p: \"Q125_A\"\nq: ~%Env.get p\nk: [q, 1]"])],
            "_.k.0", "\"alpha\"", "landed");
        same_write(&mut bad, "r2 a chain q, k, p",
            "",
            &[S::Plain(&["q: ~%Env.get p\nk: [q, 1]\np: \"Q125_A\""])], &[S::Plain(&["p: \"Q125_A\"\nq: ~%Env.get p\nk: [q, 1]"])],
            "_.k.0", "\"alpha\"", "landed");
        bad
    }));
    let n = cells(&mut bad, run);
    verdict(bad, n);
}

// ── r3 ── `_.p` at evolve reads what `p` reads ──

#[test]
fn r3_root_spelling_reads_the_working_set_at_evolve() {
    let mut bad = Vec::new();
    let mut run: Vec<Box<dyn Fn() -> Vec<String> + Send + Sync>> = Vec::new();
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        same_write(&mut bad, "r3 p in an earlier evolve: _.p and p",
            "",
            &[S::Plain(&["p: \"Q125_A\""]), S::Plain(&["k: ~%Env.get _.p"])],
            &[S::Plain(&["p: \"Q125_A\""]), S::Plain(&["k: ~%Env.get p"])],
            "_.k", "\"alpha\"", "landed");
        bad
    }));
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        same_write(&mut bad, "r3 the same write, both orders",
            "",
            &[S::Plain(&["k: ~%Env.get _.p\np: \"Q125_A\""])], &[S::Plain(&["p: \"Q125_A\"\nk: ~%Env.get _.p"])],
            "_.k", "\"alpha\"", "landed");
        bad
    }));
    let n = cells(&mut bad, run);
    verdict(bad, n);
}

// ── r4 ── the effect runs once, in the write, at the place the write names ──

/// `k: ~%Io./append_file (p, "x")` with `p` naming a file in this
/// workspace's scratch: the bytes that reach each file after evolve,
/// status, eval and commit.
fn appended(tag: &str, base: &str, pin: bool, src: &dyn Fn(&str, &str) -> String) -> Vec<String> {
    let w = Ws::new(tag);
    let a = w.root.join("a.log").display().to_string();
    let b = w.root.join("b.log").display().to_string();
    let n = |p: &str| fs::read(p).map(|v| v.len()).unwrap_or(0);
    if !base.is_empty() {
        let (_, rc) = w.step(false, &[&base.replace("@A", &a)]);
        let c = w.commit();
        if rc != 0 || c != "landed" {
            return vec![format!("SETUP {c}")];
        }
    }
    let mut out = Vec::new();
    let (o, rc) = w.step(pin, &[&src(&a, &b)]);
    out.push(if rc == 0 { "e0".to_string() } else { format!("e{rc} {}", o.chars().take(60).collect::<String>()) });
    out.push(format!("evolve a={} b={}", n(&a), n(&b)));
    let _ = w.oo(&["status"]);
    let _ = w.oo(&["eval", "_.k"]);
    out.push(format!("status+eval a={} b={}", n(&a), n(&b)));
    let c = w.commit();
    out.push(format!("{c} a={} b={}", n(&a), n(&b)));
    out.push(format!("_.k {}", w.eval("_.k")));
    out
}

#[test]
fn r4_the_effect_runs_once_where_the_write_says() {
    let mut bad = Vec::new();
    let mut run: Vec<Box<dyn Fn() -> Vec<String> + Send + Sync>> = Vec::new();
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        let first = appended("r4-kp", "", false, &|a, _| format!("k: ~%Io./append_file (p, \"x\")\np: \"{a}\""));
        let second = appended("r4-pk", "", false, &|a, _| format!("p: \"{a}\"\nk: ~%Io./append_file (p, \"x\")"));
        let want = ["e0", "evolve a=1 b=0", "status+eval a=1 b=0", "landed a=1 b=0", "_.k #true"];
        check(&mut bad, "r4 plain, k before p", first.join(" | "), &want.join(" | "));
        check(&mut bad, "r4 plain, p before k", second.join(" | "), &want.join(" | "));
        bad
    }));
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        // HEAD names file a; one pin write names file b and appends to p.
        let first = appended("r4-pin-kp", "p: \"@A\"", true, &|_, b| format!("k: ~%Io./append_file (p, \"x\")\np: \"{b}\""));
        let second = appended("r4-pin-pk", "p: \"@A\"", true, &|_, b| format!("p: \"{b}\"\nk: ~%Io./append_file (p, \"x\")"));
        let want = ["e0", "evolve a=0 b=1", "status+eval a=0 b=1", "landed a=0 b=1", "_.k #true"];
        check(&mut bad, "r4 pin, k before p", first.join(" | "), &want.join(" | "));
        check(&mut bad, "r4 pin, p before k", second.join(" | "), &want.join(" | "));
        bad
    }));
    let n = cells(&mut bad, run);
    verdict(bad, n);
}

// ── r5 ── a one-shot run is one snapshot too (SPEC_03 simultaneity) ──

/// `oo run <files> --observe k` in an empty workspace: what it prints.
fn ran(tag: &str, srcs: &[&str]) -> String {
    let w = Ws::new(tag);
    let names: Vec<String> = srcs.iter().map(|s| w.file(s)).collect();
    let mut args = vec!["run"];
    args.extend(names.iter().map(|s| s.as_str()));
    args.extend(["--observe", "k"]);
    let (o, rc) = w.oo(&args);
    let o = match o.find(";;") {
        Some(i) => o[..i].trim().to_string(),
        None => o,
    };
    format!("rc={rc} {}", squash(&o))
}

/// `oo test <file>`: its PASS / FAIL lines.
fn tested(tag: &str, src: &str) -> String {
    let w = Ws::new(tag);
    let f = w.file(src);
    let (o, rc) = w.oo(&["test", &f]);
    let lines: Vec<&str> = o.lines().filter(|l| l.starts_with("PASS") || l.starts_with("FAIL")).collect();
    format!("rc={rc} {}", lines.iter().map(|l| l.split(" - ").last().unwrap_or("")).collect::<Vec<_>>().join(" "))
        + if lines.iter().all(|l| l.starts_with("PASS")) { " pass" } else { " fail" }
}

/// One `oo repl` line, then `z: k`: what that second line prints.
fn repl_read(tag: &str, line: &str) -> String {
    use std::io::Write;
    let w = Ws::new(tag);
    let mut child = Command::new("sh")
        .arg("-c")
        .arg("ulimit -v 3000000; exec timeout -s KILL 90 \"$0\" \"$@\"")
        .arg(env!("CARGO_BIN_EXE_oo"))
        .arg("repl")
        .current_dir(&w.ws)
        .env("OO_IDENTITY", w.root.join("identity"))
        .env("OO_NODE_HOME", w.root.join("node-home"))
        .env("Q125_A", "alpha")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("oo repl runs");
    child.stdin.take().unwrap().write_all(format!("{line}\nz: k\nexit\n").as_bytes()).unwrap();
    let o = child.wait_with_output().unwrap();
    let text = String::from_utf8_lossy(&o.stdout).to_string();
    // The last answer before `exit` is what `z: k` prints; a parse error
    // or a refusal prints no `=>`, so check the count too.
    let answers: Vec<&str> = text.split("=> ").skip(1).collect();
    if answers.len() < 2 + line.matches(',').count() {
        return format!("{} answers: {}", answers.len(), squash(&text));
    }
    let last = answers.last().map(|a| a.split(";;").next().unwrap_or("").split("n>").next().unwrap_or("")).unwrap_or("");
    squash(last)
}

#[test]
fn r5_a_run_observes_after_the_names_it_defines() {
    let mut bad = Vec::new();
    let mut run: Vec<Box<dyn Fn() -> Vec<String> + Send + Sync>> = Vec::new();
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        check(&mut bad, "r5 run, k before p", ran("r5-kp", &["k: ~%Env.get p\np: \"Q125_A\""]), "rc=0 \"alpha\"");
        check(&mut bad, "r5 run, p before k", ran("r5-pk", &["p: \"Q125_A\"\nk: ~%Env.get p"]), "rc=0 \"alpha\"");
        check(&mut bad, "r5 run, two files k|p", ran("r5-2", &["k: ~%Env.get p", "p: \"Q125_A\""]), "rc=0 \"alpha\"");
        bad
    }));
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        let t = "\ntest_k: k & \"alpha\"";
        check(&mut bad, "r5 test, k before p", tested("r5-tkp", &format!("k: ~%Env.get p\np: \"Q125_A\"{t}")), "rc=0 test_k pass");
        check(&mut bad, "r5 test, p before k", tested("r5-tpk", &format!("p: \"Q125_A\"\nk: ~%Env.get p{t}")), "rc=0 test_k pass");
        bad
    }));
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        check(&mut bad, "r5 repl, one line k, p", repl_read("r5-rkp", "k: ~%Env.get p, p: \"Q125_A\""), "\"alpha\"");
        check(&mut bad, "r5 repl, one line p, k", repl_read("r5-rpk", "p: \"Q125_A\", k: ~%Env.get p"), "\"alpha\"");
        bad
    }));
    let n = cells(&mut bad, run);
    verdict(bad, n);
}

// ── g1 ── what does not observe, or does not read this write, is as before ──

#[test]
fn g1_the_rest_is_as_before() {
    let mut bad = Vec::new();
    let mut run: Vec<Box<dyn Fn() -> Vec<String> + Send + Sync>> = Vec::new();
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        // Pure fields are recipes: order-free already, and their root stays.
        let (_, c, root, v) = take("g1-pure-kp", "p: 1 | 2", &[S::Plain(&["k: p + 10\np: 1"])], "_.k");
        check(&mut bad, "g1 pure, k before p", format!("{c} {v} {root}"), &format!("landed 11 {PURE_ROOT}"));
        let (_, c, root, v) = take("g1-pure-pk", "p: 1 | 2", &[S::Plain(&["p: 1\nk: p + 10"])], "_.k");
        check(&mut bad, "g1 pure, p before k", format!("{c} {v} {root}"), &format!("landed 11 {PURE_ROOT}"));
        bad
    }));
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        // An observing field that reads nothing this write writes.
        let (_, c, root, v) = take("g1-alone", "", &[S::Plain(&["k: ~%Env.get \"Q125_A\"\np: 1"])], "_.k");
        check(&mut bad, "g1 observes a constant", format!("{c} {v} {root}"), &format!("landed \"alpha\" {ALONE_ROOT}"));
        bad
    }));
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        // A pin of what HEAD holds, read through a name this write does not write.
        let (r, c, _, v) = take("g1-held", &format!("{IO_A}\nk: g 1"), &[S::Pin(&["k: g 1"])], "_.k");
        check(&mut bad, "g1 pin k alone over the IO g", format!("{r} {c} {v}"), "e0 held \"alpha\"");
        bad
    }));
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        // A conflict in a write with an observing field is still named.
        let w = Ws::new("g1-conflict");
        let _ = w.step(false, &["x: 1"]);
        let _ = w.commit();
        let (o, rc) = w.step(false, &["k: ~%Env.get p\nx: 2\np: \"Q125_A\""]);
        check(&mut bad, "g1 conflict named", format!("rc={rc} {}", o.contains("#conflict at x")), "rc=1 true");
        check(&mut bad, "g1 conflict: x", w.eval("_.x"), "1");
        bad
    }));
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        // `_.~h` stays outside: an observing field does not reach a private
        // name through `_.` (SPEC on privates; as v0.124.0).
        let w = Ws::new("g1-private");
        let _ = w.step(false, &["~h: \"Q125_A\""]);
        let _ = w.commit();
        let (_, rc) = w.step(false, &["k: ~%Env.get _.~h"]);
        let (o, _) = w.oo(&["commit", "-m", "c"]);
        check(&mut bad, "g1 _.~h in an observing field: the commit reports the violation at k",
            format!("e{rc} {} {}", o.contains("#private_access_violation at k"), o.contains("Commit successful")), "e0 true true");
        bad
    }));
    let n = cells(&mut bad, run);
    verdict(bad, n);
}

/// `p: 1 | 2` committed, then `k: p + 10`, `p: 1` (v0.124.0).
const PURE_ROOT: &str = "1c3d594f6e7218d9c9dd9abfea8f70253bf5ad7302a4180a97ab468476793dc2";
/// `k: ~%Env.get "Q125_A"`, `p: 1` (v0.124.0).
const ALONE_ROOT: &str = "962d02a22a51eef1c274d7597416174a771ce1bd207b127026078ae47f9454b2";
