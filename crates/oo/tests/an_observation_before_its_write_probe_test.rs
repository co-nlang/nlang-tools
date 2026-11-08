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

// ════════════════════════════════════════════════════════════════════════
// R-1 (2026-11-07) — a field that observes because of this write
// ════════════════════════════════════════════════════════════════════════
//
//   The delivery decides which fields wait from the universe before the
//   write. A field that observes only because of this write does not wait:
//   `g` is new here and calls an effect; `g` is pinned from pure to an
//   effect; `k` calls a pure helper that calls `g`; `k` reads `q`, which
//   calls `g`. Written before `g`, `k` is stored as a recipe: the effect
//   runs at commit and not at evolve; under a pin it runs at status and
//   twice more at commit; through a chain, a helper or a record leaf it
//   runs 2–7 times, and the two orders commit different roots. A pin in
//   two files runs it three times in either order: the ⊥ check between the
//   files reads HEAD's `k`, a recipe, once `g` is an effect. The same
//   invariant as M1 and M3: one write is a set; the effect runs once, at
//   evolve, whichever line or file the field is on.
//
//   `a.log`, relative to the workspace, counts the effect: the same text in
//   every workspace, so the two orders' roots compare.

/// `g` calls an effect: one byte to `a.log` per call.
const LOG_G: &str = "g: (s -> ~%Io./append_file (\"a.log\", s))";
/// The same, as the leaf `h.f`.
const LOG_HF: &str = "h.f: (s -> ~%Io./append_file (\"a.log\", s))";

/// Evolve (or pin) `srcs` in one command over `base`: the bytes `a.log`
/// gained after evolve, after status and eval, after commit; what `rd`
/// reads; and the committed root.
fn logged(tag: &str, base: &str, pin: bool, srcs: &[&str], rd: &str) -> (String, String) {
    let w = Ws::new(tag);
    let n = || fs::read(w.ws.join("a.log")).map(|v| v.len()).unwrap_or(0);
    if !base.is_empty() {
        let (_, rc) = w.step(false, &[base]);
        let c = w.commit();
        if rc != 0 || c != "landed" {
            return (format!("SETUP {c}"), String::new());
        }
    }
    let before = n();
    let mut out = Vec::new();
    let (o, rc) = w.step(pin, srcs);
    out.push(if rc == 0 { "e0".to_string() } else { format!("e{rc} {}", o.chars().take(60).collect::<String>()) });
    out.push(format!("evolve +{}", n() - before));
    let _ = w.oo(&["status"]);
    let _ = w.oo(&["eval", rd]);
    out.push(format!("status+eval +{}", n() - before));
    let c = w.commit();
    out.push(format!("{c} +{}", n() - before));
    out.push(format!("{rd} {}", w.eval(rd)));
    (out.join(" | "), w.root())
}

/// Both orders of one write: each reads `want`, and the two roots are one.
fn both_logged(bad: &mut Vec<String>, label: &str, base: &str, pin: bool, first: &[&str], second: &[&str], rd: &str, want: &str) {
    let tag = label.replace(' ', "-");
    let (a, ra) = logged(&format!("{tag}-1"), base, pin, first, rd);
    let (b, rb) = logged(&format!("{tag}-2"), base, pin, second, rd);
    check(bad, &format!("{label}, k first"), a, want);
    check(bad, &format!("{label}, k last"), b, want);
    check(bad, &format!("{label}: one root"), (ra == rb && !ra.is_empty()).to_string(), "true");
}

const ONCE: &str = "e0 | evolve +1 | status+eval +1 | landed +1 | _.k #true";

#[test]
fn r6_a_plain_write_that_makes_its_field_observe() {
    let mut bad = Vec::new();
    let mut run: Vec<Box<dyn Fn() -> Vec<String> + Send + Sync>> = Vec::new();
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        both_logged(&mut bad, "r6 g new", "", false,
            &[&format!("k: g \"x\"\n{LOG_G}")], &[&format!("{LOG_G}\nk: g \"x\"")], "_.k", ONCE);
        bad
    }));
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        both_logged(&mut bad, "r6 g new, read as _.g", "", false,
            &[&format!("k: _.g \"x\"\n{LOG_G}")], &[&format!("{LOG_G}\nk: _.g \"x\"")], "_.k", ONCE);
        bad
    }));
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        both_logged(&mut bad, "r6 g new, two files", "", false,
            &["k: g \"x\"", LOG_G], &[LOG_G, "k: g \"x\""], "_.k", ONCE);
        bad
    }));
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        both_logged(&mut bad, "r6 through a pure helper f", "", false,
            &[&format!("k: f \"x\"\nf: (s -> g s)\n{LOG_G}")], &[&format!("{LOG_G}\nf: (s -> g s)\nk: f \"x\"")], "_.k", ONCE);
        bad
    }));
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        both_logged(&mut bad, "r6 through q, which calls g", "", false,
            &[&format!("k: [q]\nq: g \"x\"\n{LOG_G}")], &[&format!("{LOG_G}\nq: g \"x\"\nk: [q]")], "_.k.0",
            "e0 | evolve +1 | status+eval +1 | landed +1 | _.k.0 #true");
        bad
    }));
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        // `q` names `g` or holds it: a call of `q` is a call of `g`.
        both_logged(&mut bad, "r6 through q: g", "", false,
            &[&format!("k: q \"x\"\nq: g\n{LOG_G}")], &[&format!("{LOG_G}\nq: g\nk: q \"x\"")], "_.k", ONCE);
        both_logged(&mut bad, "r6 through q: [g]", "", false,
            &[&format!("k: q.0 \"x\"\nq: [g]\n{LOG_G}")], &[&format!("{LOG_G}\nq: [g]\nk: q.0 \"x\"")], "_.k", ONCE);
        bad
    }));
    let n = cells(&mut bad, run);
    verdict(bad, n);
}

#[test]
fn r7_a_pin_that_makes_its_field_observe() {
    let mut bad = Vec::new();
    let mut run: Vec<Box<dyn Fn() -> Vec<String> + Send + Sync>> = Vec::new();
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        both_logged(&mut bad, "r7 g pure to effect, HEAD holds k", "g: (s -> s)\nk: g \"x\"", true,
            &[&format!("k: g \"x\"\n{LOG_G}")], &[&format!("{LOG_G}\nk: g \"x\"")], "_.k", ONCE);
        bad
    }));
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        both_logged(&mut bad, "r7 g pure to effect, k new", "g: (s -> s)", true,
            &[&format!("k: g \"x\"\n{LOG_G}")], &[&format!("{LOG_G}\nk: g \"x\"")], "_.k", ONCE);
        bad
    }));
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        both_logged(&mut bad, "r7 two files, HEAD holds k", "g: (s -> s)\nk: g \"x\"", true,
            &["k: g \"x\"", LOG_G], &[LOG_G, "k: g \"x\""], "_.k", ONCE);
        bad
    }));
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        both_logged(&mut bad, "r7 the record leaf h.f", "h: { f: (s -> s) }\nk: h.f \"x\"", true,
            &[&format!("k: h.f \"x\"\n{LOG_HF}")], &[&format!("{LOG_HF}\nk: h.f \"x\"")], "_.k", ONCE);
        bad
    }));
    let n = cells(&mut bad, run);
    verdict(bad, n);
}

/// One `oo repl` line, then `exit`: the line's answers (a function as `fn`,
/// the effect note dropped), sorted, and the bytes `a.log` gained.
fn repl_line(tag: &str, line: &str) -> String {
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
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("oo repl runs");
    child.stdin.take().unwrap().write_all(format!("{line}\nexit\n").as_bytes()).unwrap();
    let o = child.wait_with_output().unwrap();
    let text = String::from_utf8_lossy(&o.stdout).to_string();
    let mut answers: Vec<String> = text
        .split("=> ")
        .skip(1)
        .map(|a| {
            let a = squash(a.split("n>").next().unwrap_or(""));
            if a.starts_with("{{") { "fn".to_string() } else { a.split(";;").next().unwrap_or("").trim().to_string() }
        })
        .collect();
    answers.sort();
    let n = fs::read(w.ws.join("a.log")).map(|v| v.len()).unwrap_or(0);
    format!("{} +{n}", answers.join(" "))
}

#[test]
fn r8_a_repl_line_that_makes_its_field_observe() {
    let mut bad = Vec::new();
    let mut run: Vec<Box<dyn Fn() -> Vec<String> + Send + Sync>> = Vec::new();
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        check(&mut bad, "r8 repl, k before g", repl_line("r8-kg", &format!("k: g \"x\", {LOG_G}")), "#true fn +1");
        check(&mut bad, "r8 repl, g before k", repl_line("r8-gk", &format!("{LOG_G}, k: g \"x\"")), "#true fn +1");
        bad
    }));
    let n = cells(&mut bad, run);
    verdict(bad, n);
}

// ── g2 ── the other half of R-1 ──

#[test]
fn g2_what_does_not_call_the_effect_is_as_before() {
    let mut bad = Vec::new();
    let mut run: Vec<Box<dyn Fn() -> Vec<String> + Send + Sync>> = Vec::new();
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        // `k` holds `g` and does not call it: no effect until a reader calls it.
        both_logged(&mut bad, "g2 k holds g", "", false,
            &[&format!("k: [g]\n{LOG_G}")], &[&format!("{LOG_G}\nk: [g]")], "_.k.0 \"y\"",
            "e0 | evolve +0 | status+eval +0 | landed +0 | _.k.0 \"y\" #true");
        bad
    }));
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        // A conflict beside a field that waits is named where it is.
        for (i, src) in [format!("k: g \"x\"\nx: 2\n{LOG_G}"), format!("x: 2\n{LOG_G}\nk: g \"x\"")].iter().enumerate() {
            let w = Ws::new(&format!("g2-conflict-{i}"));
            let _ = w.step(false, &["x: 1"]);
            let _ = w.commit();
            let (o, rc) = w.step(false, &[src]);
            let n = fs::read(w.ws.join("a.log")).map(|v| v.len()).unwrap_or(0);
            check(&mut bad, &format!("g2 conflict named, order {i}"), format!("rc={rc} {} +{n}", o.contains("#conflict at x")), "rc=1 true +0");
            check(&mut bad, &format!("g2 conflict, order {i}: x"), w.eval("_.x"), "1");
        }
        bad
    }));
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        // A field that waits and makes a definition it does not write read ⊥
        // is refused at evolve, as Q-122 refuses any (one file or two).
        for (i, srcs) in [vec![format!("{LOG_G}\nk: g \"x\"")], vec![LOG_G.to_string(), "k: g \"x\"".to_string()]].iter().enumerate() {
            let w = Ws::new(&format!("g2-late-{i}"));
            let _ = w.step(false, &["g: (s -> s)\nk: g 1\nm: k + 1"]);
            let _ = w.commit();
            let srcs: Vec<&str> = srcs.iter().map(|s| s.as_str()).collect();
            let (o, rc) = w.step(true, &srcs);
            check(&mut bad, &format!("g2 late bottom at m refused, {} file(s)", srcs.len()), format!("rc={rc} {}", o.contains("#conflict at m")), "rc=1 true");
            check(&mut bad, &format!("g2 late bottom, {} file(s): m", srcs.len()), w.eval("_.m"), "2");
        }
        bad
    }));
    let n = cells(&mut bad, run);
    verdict(bad, n);
}

// ════════════════════════════════════════════════════════════════════════
// R-2 (2026-11-07) — a helper the universe already holds
// ════════════════════════════════════════════════════════════════════════
//
//   R-1 found the names that carry an effect among the write's own
//   definitions. A helper already in HEAD — `f: (s -> g s)`, or the recipe
//   `q: g` — is read through no more: the write makes `g` an effect, `k:
//   f "x"` reads only `f`, which this write does not write, and so does not
//   wait. Written before `g`: the effect runs 2–7 times and the two orders
//   commit two roots. The other way: HEAD's `g` observes, the pin makes it
//   pure, and `k: f "x"` written first stores HEAD's observation (`"alpha"`)
//   where the other order stores `"x"`. R1-M1: whatever a field reads
//   through — a helper is a helper wherever it is defined.

#[test]
fn r9_a_helper_the_universe_holds() {
    let mut bad = Vec::new();
    let mut run: Vec<Box<dyn Fn() -> Vec<String> + Send + Sync>> = Vec::new();
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        both_logged(&mut bad, "r9 HEAD f calls g, g new", "f: (s -> g s)", false,
            &[&format!("k: f \"x\"\n{LOG_G}")], &[&format!("{LOG_G}\nk: f \"x\"")], "_.k", ONCE);
        bad
    }));
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        both_logged(&mut bad, "r9 HEAD f calls g, g new, two files", "f: (s -> g s)", false,
            &["k: f \"x\"", LOG_G], &[LOG_G, "k: f \"x\""], "_.k", ONCE);
        bad
    }));
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        both_logged(&mut bad, "r9 HEAD f calls g, pin g pure to effect", "g: (s -> s)\nf: (s -> g s)", true,
            &[&format!("k: f \"x\"\n{LOG_G}")], &[&format!("{LOG_G}\nk: f \"x\"")], "_.k", ONCE);
        bad
    }));
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        both_logged(&mut bad, "r9 HEAD recipe q: g, pin g pure to effect", "g: (s -> s)\nq: g", true,
            &[&format!("k: q \"x\"\n{LOG_G}")], &[&format!("{LOG_G}\nk: q \"x\"")], "_.k", ONCE);
        bad
    }));
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        // HEAD's g observes; the pin makes it pure: `k` reads the pure one.
        both_logged(&mut bad, "r9 HEAD f calls g, pin g effect to pure",
            "g: (s -> ~%Env.get \"Q125_A\")\nf: (s -> g s)\nk: f \"x\"", true,
            &["k: f \"x\"\ng: (s -> s)"], &["g: (s -> s)\nk: f \"x\""], "_.k",
            "e0 | evolve +0 | status+eval +0 | landed +0 | _.k \"x\"");
        bad
    }));
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        // HEAD's f calls q; this write aliases q to the new g: `k` waits
        // after `q`, which waits too.
        both_logged(&mut bad, "r9 HEAD f calls q, q: g here", "f: (s -> q s)", false,
            &[&format!("k: f \"x\"\nq: g\n{LOG_G}")], &[&format!("{LOG_G}\nq: g\nk: f \"x\"")], "_.k", ONCE);
        bad
    }));
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        // The helper is in the working set, evolved before and not committed.
        let tag = "r9-ws";
        let (a, ra) = logged_after(&format!("{tag}-1"), "f: (s -> g s)", &[&format!("k: f \"x\"\n{LOG_G}")]);
        let (b, rb) = logged_after(&format!("{tag}-2"), "f: (s -> g s)", &[&format!("{LOG_G}\nk: f \"x\"")]);
        check(&mut bad, "r9 working-set f calls g, k first", a, ONCE);
        check(&mut bad, "r9 working-set f calls g, k last", b, ONCE);
        check(&mut bad, "r9 working-set f: one root", (ra == rb && !ra.is_empty()).to_string(), "true");
        bad
    }));
    let n = cells(&mut bad, run);
    verdict(bad, n);
}

/// As `logged`, with `pre` evolved first and left in the working set.
fn logged_after(tag: &str, pre: &str, srcs: &[&str]) -> (String, String) {
    let w = Ws::new(tag);
    let n = || fs::read(w.ws.join("a.log")).map(|v| v.len()).unwrap_or(0);
    let (_, rc) = w.step(false, &[pre]);
    if rc != 0 {
        return ("SETUP".into(), String::new());
    }
    let mut out = Vec::new();
    let (o, rc) = w.step(false, srcs);
    out.push(if rc == 0 { "e0".to_string() } else { format!("e{rc} {}", o.chars().take(60).collect::<String>()) });
    out.push(format!("evolve +{}", n()));
    let _ = w.oo(&["status"]);
    let _ = w.oo(&["eval", "_.k"]);
    out.push(format!("status+eval +{}", n()));
    let c = w.commit();
    out.push(format!("{c} +{}", n()));
    out.push(format!("_.k {}", w.eval("_.k")));
    (out.join(" | "), w.root())
}

// ════════════════════════════════════════════════════════════════════════
// R-3 (2026-11-07) — a call that returns the effect
// ════════════════════════════════════════════════════════════════════════
//
//   R-1 and R-2 take a call for an answer: `q: g "x"` observes, waits,
//   and carries nothing on. But what is called may return a function that
//   calls the effect: `mk: (u -> g)`, `f: mk 1`. Then `f` is a carrier like
//   `g`. In the delivery `f` waits (it reads the carrier `mk`) and `k: f
//   "x"`, which reads only `f`, does not — evolved before `f`, `k` is a
//   recipe and the effect runs at commit in both orders (v0.124.0 ran it
//   at evolve when `g` came first). With `mk` and `f` in HEAD, `k` first
//   runs it 2–5 times and the two orders commit two roots.

#[test]
fn r10_a_call_that_returns_the_effect() {
    let mut bad = Vec::new();
    let mut run: Vec<Box<dyn Fn() -> Vec<String> + Send + Sync>> = Vec::new();
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        both_logged(&mut bad, "r10 mk and f: mk 1 here", "", false,
            &[&format!("k: f \"x\"\nf: mk 1\nmk: (u -> g)\n{LOG_G}")], &[&format!("{LOG_G}\nmk: (u -> g)\nf: mk 1\nk: f \"x\"")], "_.k", ONCE);
        bad
    }));
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        both_logged(&mut bad, "r10 HEAD mk, f: mk 1 here", "mk: (u -> g)", false,
            &[&format!("k: f \"x\"\nf: mk 1\n{LOG_G}")], &[&format!("{LOG_G}\nf: mk 1\nk: f \"x\"")], "_.k", ONCE);
        bad
    }));
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        both_logged(&mut bad, "r10 HEAD mk and f: mk 1", "mk: (u -> g)\nf: mk 1", false,
            &[&format!("k: f \"x\"\n{LOG_G}")], &[&format!("{LOG_G}\nk: f \"x\"")], "_.k", ONCE);
        bad
    }));
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        // What returns a carrier is passed on: a maker that calls the maker,
        // and a name for the maker.
        both_logged(&mut bad, "r10 mk2: (v -> mk v)", "", false,
            &[&format!("k: f \"x\"\nf: mk2 1\nmk2: (v -> mk v)\nmk: (u -> g)\n{LOG_G}")],
            &[&format!("{LOG_G}\nmk: (u -> g)\nmk2: (v -> mk v)\nf: mk2 1\nk: f \"x\"")], "_.k", ONCE);
        both_logged(&mut bad, "r10 mk2: mk", "", false,
            &[&format!("k: f \"x\"\nf: mk2 1\nmk2: mk\nmk: (u -> g)\n{LOG_G}")],
            &[&format!("{LOG_G}\nmk: (u -> g)\nmk2: mk\nf: mk2 1\nk: f \"x\"")], "_.k", ONCE);
        bad
    }));
    let n = cells(&mut bad, run);
    verdict(bad, n);
}

/// Q-125 R-4: the function a maker returns is built in its body — the effect
/// is two or more calls away (`mk: (u -> (s -> g s))`), and `f: mk 1` carries
/// it as `mk: (u -> g)` does. Inline, three deep, held in a record, and with
/// the maker and `f` in HEAD; and a call through functions that call each
/// other.
#[test]
fn r11_an_effect_two_calls_away() {
    let mut bad = Vec::new();
    let mut run: Vec<Box<dyn Fn() -> Vec<String> + Send + Sync>> = Vec::new();
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        both_logged(&mut bad, "r11 mk: (u -> (s -> g s)) here", "", false,
            &[&format!("k: f \"x\"\nf: mk 1\nmk: (u -> (s -> g s))\n{LOG_G}")],
            &[&format!("{LOG_G}\nmk: (u -> (s -> g s))\nf: mk 1\nk: f \"x\"")], "_.k", ONCE);
        bad
    }));
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        both_logged(&mut bad, "r11 HEAD mk: (u -> (s -> g s)) and f: mk 1", "mk: (u -> (s -> g s))\nf: mk 1", false,
            &[&format!("k: f \"x\"\n{LOG_G}")], &[&format!("{LOG_G}\nk: f \"x\"")], "_.k", ONCE);
        bad
    }));
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        both_logged(&mut bad, "r11 mk3 three deep, f: mk3 1 2", "", false,
            &[&format!("k: f \"x\"\nf: mk3 1 2\nmk3: (a -> (b -> (c -> g c)))\n{LOG_G}")],
            &[&format!("{LOG_G}\nmk3: (a -> (b -> (c -> g c)))\nf: mk3 1 2\nk: f \"x\"")], "_.k", ONCE);
        bad
    }));
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        both_logged(&mut bad, "r11 inline effect two deep", "", false,
            &["k: f \"x\"\nf: mk 1\nmk: (u -> (s -> ~%Io./append_file (\"a.log\", s)))"],
            &["mk: (u -> (s -> ~%Io./append_file (\"a.log\", s)))\nf: mk 1\nk: f \"x\""], "_.k", ONCE);
        bad
    }));
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        both_logged(&mut bad, "r11 held in a record", "", false,
            &[&format!("k: f.h \"x\"\nf: mk 1\nmk: (u -> {{ h: (s -> g s) }})\n{LOG_G}")],
            &[&format!("{LOG_G}\nmk: (u -> {{ h: (s -> g s) }})\nf: mk 1\nk: f.h \"x\"")], "_.k", ONCE);
        bad
    }));
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        // Functions that call each other go before the field that calls
        // through them; `a` before `b` in both orders.
        both_logged(&mut bad, "r11 a call through mutual recursion", "", false,
            &[&format!("k: (f \"x\").e \"y\"\nf: b 1\na: (u -> {{ e: g, n: b }})\nb: (u -> a)\n{LOG_G}")],
            &[&format!("{LOG_G}\na: (u -> {{ e: g, n: b }})\nb: (u -> a)\nf: b 1\nk: (f \"x\").e \"y\"")], "_.k", ONCE);
        bad
    }));
    let n = cells(&mut bad, run);
    verdict(bad, n);
}

/// Q-125 R-5: a record, a list and a table are functions of their keys —
/// `mk: { _: (s -> g s) }` returns the function on `mk 1`, as
/// `mk: (u -> (s -> g s))` does; the same in HEAD, a record holding `g`,
/// inside a maker, a list in HEAD; an open term `f: g $` that `|>` binds;
/// and a dotted key.
#[test]
fn r12_a_record_is_called_by_its_keys() {
    let mut bad = Vec::new();
    let mut run: Vec<Box<dyn Fn() -> Vec<String> + Send + Sync>> = Vec::new();
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        both_logged(&mut bad, "r12 mk: { _: (s -> g s) } here", "", false,
            &[&format!("k: f \"x\"\nf: mk 1\nmk: {{ _: (s -> g s) }}\n{LOG_G}")],
            &[&format!("{LOG_G}\nmk: {{ _: (s -> g s) }}\nf: mk 1\nk: f \"x\"")], "_.k", ONCE);
        bad
    }));
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        both_logged(&mut bad, "r12 HEAD mk: { _: (s -> g s) } and f: mk 1", "mk: { _: (s -> g s) }\nf: mk 1", false,
            &[&format!("k: f \"x\"\n{LOG_G}")], &[&format!("{LOG_G}\nk: f \"x\"")], "_.k", ONCE);
        bad
    }));
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        both_logged(&mut bad, "r12 mk: { _: g }", "", false,
            &[&format!("k: f \"x\"\nf: mk 1\nmk: {{ _: g }}\n{LOG_G}")],
            &[&format!("{LOG_G}\nmk: {{ _: g }}\nf: mk 1\nk: f \"x\"")], "_.k", ONCE);
        bad
    }));
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        both_logged(&mut bad, "r12 mk: (u -> { _: (s -> g s) }), f: mk 1 2", "", false,
            &[&format!("k: f \"x\"\nf: mk 1 2\nmk: (u -> {{ _: (s -> g s) }})\n{LOG_G}")],
            &[&format!("{LOG_G}\nmk: (u -> {{ _: (s -> g s) }})\nf: mk 1 2\nk: f \"x\"")], "_.k", ONCE);
        bad
    }));
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        both_logged(&mut bad, "r12 HEAD q: [(s -> g s)] and f: q 0", "q: [(s -> g s)]\nf: q 0", false,
            &[&format!("k: f \"x\"\n{LOG_G}")], &[&format!("{LOG_G}\nk: f \"x\"")], "_.k", ONCE);
        bad
    }));
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        // `|>` meets its input with the bound term: `#conflict` either way;
        // the effect is still done once, at evolve.
        both_logged(&mut bad, "r12 open term f: g $", "", false,
            &[&format!("k: \"x\" |> f\nf: g $\n{LOG_G}")], &[&format!("{LOG_G}\nf: g $\nk: \"x\" |> f")], "_.k",
            "e0 | evolve +1 | status+eval +1 | landed +1 | _.k _|_");
        bad
    }));
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        // `h.f: ...` defines `h` one key deeper: `q: h.f` is the function.
        both_logged(&mut bad, "r12 h.f: (s -> g s), q: h.f", "", false,
            &[&format!("k: q \"x\"\nq: h.f\nh.f: (s -> g s)\n{LOG_G}")],
            &[&format!("{LOG_G}\nh.f: (s -> g s)\nq: h.f\nk: q \"x\"")], "_.k", ONCE);
        bad
    }));
    let n = cells(&mut bad, run);
    verdict(bad, n);
}
