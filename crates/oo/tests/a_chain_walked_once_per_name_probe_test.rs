// Q-134 probe: a chain where each name is walked once.
//
// In one observation a written name is one value (SPEC_10 §2.3.1, Q-126 2):
// a record a name holds, or a call made, is one record. An evolve runs the
// write in stages (the scratch write, observing the write, the late bottoms)
// and in each stage every field forced the names it read again: a chain of N
// readers cost N walks of the chain per stage, and through a call that builds
// a list, a record or a tuple each walk copied the argument at every level.
// (v0.133.0: `k_i: id k_{i+1}`, `id: (x -> [x])`, N=100 evolves in 30 s;
// with the last link an effect, N=250 does not return in 300 s.)
//
// r1 — a chain through a call or a pipe evolves within 20 s at N=110.
// r2 — an argument met with a pattern costs its size: a deep argument handed
//      to a function many times evaluates within 15 s.
// g1 — what is stored and read does not change: twenty-one shapes (cycles,
//      effects on a cycle, the depth horizon in both orders, a meet on a held
//      coordinate in another evolve and in one write, a spread, a pin, a name
//      landing after its reader, readers out of order, a name written twice
//      after its reader, a lazy field, a conflict with HEAD) keep the root, the readings and the
//      effect count v0.133.0 gives.

use std::fs;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const E: &str = "~%Io./append_file (\"a.log\", \"x\")";

struct Ws {
    _scratch: nlang_interpreter::ScratchDir,
    root: std::path::PathBuf,
    ws: std::path::PathBuf,
}

impl Ws {
    fn new(tag: &str) -> Self {
        let scratch = nlang_interpreter::ScratchDir::new(&format!("walked-once-{tag}"));
        let root = scratch.path().to_path_buf();
        let ws = root.join("ws");
        fs::create_dir_all(&ws).unwrap();
        Ws { _scratch: scratch, root, ws }
    }

    /// Run `oo` capped at 2 GB, killed after `kill`: (output, rc). The output
    /// goes to a file, not a pipe (a printed value can pass 64 KB).
    fn oo(&self, args: &[&str], kill: Duration) -> (String, i32) {
        let out_path = self.root.join("out.txt");
        let out = fs::File::create(&out_path).expect("output file");
        let err = out.try_clone().expect("output file");
        let mut cmd = Command::new("sh");
        cmd.arg("-c")
            .arg("ulimit -v 2000000; exec \"$0\" \"$@\"")
            .arg(env!("CARGO_BIN_EXE_oo"))
            .args(args)
            .current_dir(&self.ws)
            .env("OO_IDENTITY", self.root.join("identity"))
            .env("OO_NODE_HOME", self.root.join("node-home"))
            .env("Q134_A", "alpha")
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
            std::thread::sleep(Duration::from_millis(20));
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

    fn code(o: &str, rc: i32, letter: &str) -> String {
        if o == "DID NOT RETURN" || o == "CRASHED" || o == "PANIC" { o.to_string() } else { format!("{letter}{rc}") }
    }

    fn file(&self, name: &str, src: &str) {
        fs::write(self.ws.join(name), format!("{src}\n")).unwrap();
    }

    fn bytes(&self) -> usize {
        fs::read(self.ws.join("a.log")).map(|b| b.len()).unwrap_or(0)
    }

    /// The committed root's address, last 16 hex digits.
    fn root_tail(&self) -> String {
        let head = fs::read_to_string(self.ws.join(".oo").join("HEAD")).unwrap_or_default();
        let (o, _) = self.oo(&["inspect", head.trim()], Duration::from_secs(60));
        o.lines()
            .find(|l| l.contains("root:"))
            .and_then(|l| l.split_whitespace().last())
            .map(|r| r[r.len().saturating_sub(16)..].to_string())
            .unwrap_or_else(|| "-".into())
    }
}

fn squash(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn sha16(s: &str) -> String {
    hex::encode(ring::digest::digest(&ring::digest::SHA256, s.as_bytes()).as_ref())[..16].to_string()
}

fn check(bad: &mut Vec<String>, label: &str, got: String, want: &str) {
    if got != want {
        bad.push(format!("  {label}\n    want {want}\n    got  {}", got.chars().take(200).collect::<String>()));
    }
}

fn verdict(bad: Vec<String>, n: usize) {
    assert!(bad.is_empty(), "{} of {} wrong:\n{}", bad.len(), n, bad.join("\n"));
}

fn chain(n: usize, link: &str, last: &str, pre: &str) -> String {
    let mut s = pre.to_string();
    for i in 0..n {
        s.push_str(&link.replace("{i}", &i.to_string()).replace("{j}", &(i + 1).to_string()));
        s.push('\n');
    }
    s.push_str(&last.replace("{n}", &n.to_string()));
    s
}

// ── r1 ── a chain through a call evolves once per name ─────────────────

/// r1 — Chains of 110 readers, each reading the next through a call or a
/// pipe (a list, a record, a tuple; the `_.` spelling; the last link an
/// effect), evolve within 20 s; then commit, and `k100` reads ten levels of
/// what its link builds. (v0.133.0: each field walked the chain again in each
/// stage of the evolve; 22–40 s. The record shape is only evolved: its commit
/// runs out of the 2 GB cap on v0.133.0 too — the Inbox row on records a
/// call makes.)
#[test]
fn r1_a_chain_through_a_call_evolves_once_per_name() {
    let mut bad = Vec::new();
    let n = 110;
    let io = format!("k{{n}}: {E}");
    let list = format!("{}7{}", "[".repeat(10), "]".repeat(10));
    let tuple = format!("{}7{}", "{{ 0: ".repeat(10), " 1: 1 }}".repeat(10));
    let effect = format!("{}#true ;; %effect: #io{}", "[".repeat(10), "]".repeat(10));
    let shapes: [(&str, String, Option<String>); 6] = [
        ("a list", chain(n, "k{i}: id k{j}", "k{n}: 7", "id: (x -> [x])\n"), Some(list.clone())),
        ("a record", chain(n, "k{i}: id k{j}", "k{n}: 7", "id: (x -> { v: x })\n"), None),
        ("a tuple", chain(n, "k{i}: id k{j}", "k{n}: 7", "id: (x -> (x, 1))\n"), Some(tuple)),
        ("a pipe", chain(n, "k{i}: k{j} |> (x -> [x])", "k{n}: 7", ""), Some(list.clone())),
        ("the _. spelling", chain(n, "k{i}: id _.k{j}", "k{n}: 7", "id: (x -> [x])\n"), Some(list)),
        ("the last link an effect", chain(n, "k{i}: id k{j}", &io, "id: (x -> [x])\n"), Some(effect)),
    ];
    let mut cells = 0;
    for (shape, src, k100) in &shapes {
        let w = Ws::new("r1");
        w.file("f.n", src);
        let (o, rc) = w.oo(&["evolve", "f.n"], Duration::from_secs(20));
        check(&mut bad, &format!("{shape}, N={n}: evolve within 20 s"), Ws::code(&o, rc, "e"), "e0");
        cells += 1;
        if let (0, Some(want)) = (rc, k100) {
            let (o, rc) = w.oo(&["commit", "-m", "a"], Duration::from_secs(60));
            check(&mut bad, &format!("{shape}, N={n}: commit"), Ws::code(&o, rc, "c"), "c0");
            let (o, _) = w.oo(&["eval", "--", "_.k100"], Duration::from_secs(60));
            check(&mut bad, &format!("{shape}, N={n}: _.k100"), squash(&o), want);
            cells += 2;
        }
    }
    verdict(bad, cells);
}

// ── r2 ── an argument met with a pattern costs its size ────────────────

/// r2 — A list nested 120 deep, handed to a function 600 times, evaluates
/// within 15 s. (v0.133.0: each meet of the argument with the parameter
/// copied the record at every level of it, its depth squared per call.)
#[test]
fn r2_an_argument_met_with_a_pattern_costs_its_size() {
    let mut bad = Vec::new();
    let d = 120;
    let m = 600;
    let deep = format!("{}1{}", "[".repeat(d), "]".repeat(d));
    let calls = vec!["f v"; m].join(", ");
    let e = format!("{{ v: {deep}, f: (x -> 1), r: [{calls}] }}.r");
    let w = Ws::new("r2");
    let (o, rc) = w.oo(&["eval", "--", &e], Duration::from_secs(15));
    check(&mut bad, &format!("depth {d}, {m} calls: within 15 s"), Ws::code(&o, rc, "rc"), "rc0");
    let ones = vec!["1"; m].join(", ");
    check(&mut bad, &format!("depth {d}, {m} calls: the answer"), squash(&o), &format!("[{ones}]"));
    verdict(bad, 2);
}

// ── g1 ── what is stored and read does not change ──────────────────────

/// One shape: its steps — ("e", src) evolve, ("p", src) pin, ("c", "") commit.
type Steps = &'static [(&'static str, &'static str)];

/// g1 — Each shape keeps the root, the readings of its written names and the
/// effect count v0.133.0 gives: rcs, the root's last 16 hex digits, the
/// first 16 of a digest of the readings, the bytes the effect appended.
#[test]
fn g1_what_is_stored_and_read_does_not_change() {
    let c30 = chain(30, "k{i}: id k{j}", "k{n}: 7", "id: (x -> [x])\n");
    let c30io = chain(30, "k{i}: id k{j}", &format!("k{{n}}: {E}"), "id: (x -> [x])\n");
    let hz = chain(300, "k{i}: k{j} + 1", "k{n}: 5", "");
    let hzr = hz.lines().rev().collect::<Vec<_>>().join("\n");
    let split = |s: &str, at: usize| {
        let ls: Vec<&str> = s.lines().collect();
        (ls[..at].join("\n"), ls[at..].join("\n"))
    };
    let (c30a, c30b) = split(&c30, 16);
    let (c30ioa, c30iob) = split(&c30io, 31);
    let cyc_e = format!("a: [{E}, b]\nb: [a]");
    let cyc_e_a = format!("a: [{E}, b]");
    let local = format!("~a: [p]\nm: [~a]\n@T: [p]\nn: [@T]\np: {E}");
    let lazy = format!("q: {{ a: (x -> {E}) 1 }}\nk: [q.a]");
    let pinned = format!("a: [p]\nm: [a]\np: {E}");
    let shapes: Vec<(&str, Vec<(&str, String)>, &str)> = vec![
        ("a chain through a call, one write", vec![("e", c30.clone())], "e0 c0 877ff571f4658fbf 23d3a392d8c60a45 0"),
        ("a chain through a call, readers first", vec![("e", c30a), ("e", c30b)], "e0 e0 c0 877ff571f4658fbf 23d3a392d8c60a45 0"),
        ("a chain ending in an effect, one write", vec![("e", c30io.clone())], "e0 c0 f3f6f422b5f72f34 65a8ead625a51906 1"),
        ("a chain ending in an effect, the effect last", vec![("e", c30ioa), ("e", c30iob)], "e0 e0 c0 f3f6f422b5f72f34 65a8ead625a51906 1"),
        ("a chain through a call making records", vec![("e", chain(20, "k{i}: id k{j}", "k{n}: 7", "id: (x -> { v: x })\n"))], "e0 c0 b66df7737aebc428 78b3a173075757a0 0"),
        ("a chain through a pipe and _.", vec![("e", chain(20, "k{i}: _.k{j} |> (x -> [x])", "k{n}: 7", ""))], "e0 c0 c9aab4d035321066 975b950172455018 0"),
        ("a meet on a coordinate the working set holds", vec![("e", "h: { k: 1 }\nr: [h.k, h.j]".into()), ("e", "h: { j: 2 }".into())], "e0 e0 c0 06b9b1278b153ecd a2cafcd8f53e29e2 0"),
        ("a name written after its reader, in another evolve", vec![("e", "a: [b, c]\nm: [a]".into()), ("e", "b: 5".into()), ("e", "c: [b]".into())], "e0 e0 e0 c0 60a1288ed9f51dce eaf1757aa0b1ce79 0"),
        ("a cycle", vec![("e", "a: b\nb: [a]\nout: a".into())], "e0 c0 065c7880a9a0e213 8ad2cd411eb3dfb5 0"),
        ("a cycle through an effect", vec![("e", cyc_e)], "e0 c0 467105d210ff3784 c17cdedf1d786643 1"),
        ("a cycle through an effect, reader first", vec![("e", "b: [a]".into()), ("e", cyc_e_a)], "e0 e0 c0 467105d210ff3784 373a4bdf148a127c 1"),
        ("the horizon, names first", vec![("e", hz)], "e0 c0 ffaac0c92aef280c 4f5ac1acc3d9bd82 0"),
        ("the horizon, readers first", vec![("e", hzr)], "e0 c0 ffaac0c92aef280c 292437349900c822 0"),
        ("a spread at the root", vec![("e", "c: { b: 2 }".into()), ("e", "...c\nd: [b]".into())], "e0 e0 c0 f0176bfc76ed2a45 9cd551a2246c2a3e 0"),
        ("a local and a type reader", vec![("e", local)], "e0 c0 a16d470e01460384 6617d0b8881711e1 1"),
        ("a lazy field doing an effect (D129)", vec![("e", lazy)], "e0 c0 806e06d008ec103d fcd1454048c07dd2 1"),
        ("a pin under readers", vec![("e", pinned), ("p", "p: 7".into())], "e0 p0 c0 76d05dd2a8a42177 036f023efdabf019 1"),
        ("a meet in one write", vec![("e", "h: { k: 1 }\nr: [h.k, h.j]\nh: { j: 2 }".into())], "e0 c0 06b9b1278b153ecd a2cafcd8f53e29e2 0"),
        ("readers out of order in one write", vec![("e", "m: [a]\na: [b, c]\nc: [b]\nb: 5".into())], "e0 c0 60a1288ed9f51dce dc12864ca416bce0 0"),
        ("a name read, then written twice in one write", vec![("e", "a: [b]\nb: { x: 1 }\nb: { y: 2 }\nm: [a, b.y]".into())], "e0 c0 283914a4ba69c30e 29d10ee777077025 0"),
        ("a conflict with HEAD, readers first", vec![("e", "h: { k: 0, r: 6 }".into()), ("c", String::new()), ("e", "q: [h.r]\nh: { r: 7 }".into())], "e0 c0 e1 c1 d2f99f44bc32404c f22761c8ce743566 0"),
    ];
    let mut bad = Vec::new();
    let t = Duration::from_secs(60);
    for (label, steps, want) in &shapes {
        let w = Ws::new("g1");
        let mut rcs = Vec::new();
        let mut names: Vec<String> = Vec::new();
        for (k, (kind, src)) in steps.iter().enumerate() {
            if *kind == "c" {
                let (o, rc) = w.oo(&["commit", "-m", "a"], t);
                rcs.push(Ws::code(&o, rc, "c"));
                continue;
            }
            let f = format!("f{k}.n");
            w.file(&f, src);
            let (o, rc) = if *kind == "e" {
                w.oo(&["evolve", &f], t)
            } else {
                w.oo(&["evolve", "--pin", "--grant", "pin", &f], t)
            };
            rcs.push(Ws::code(&o, rc, kind));
            for l in src.lines() {
                let l = l.trim();
                if let Some((n, _)) = l.split_once(':') {
                    let ok = n.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '@')
                        && n.chars().skip(1).all(|c| c.is_ascii_alphanumeric() || c == '_');
                    if ok && !names.iter().any(|x| x == n) {
                        names.push(n.to_string());
                    }
                }
            }
        }
        let (o, rc) = w.oo(&["commit", "-m", "b", "--grant", "pin"], t);
        rcs.push(Ws::code(&o, rc, "c"));
        if names.len() > 6 {
            let tail: Vec<String> = names[names.len() - 2..].to_vec();
            names.truncate(4);
            names.extend(tail);
        }
        let readings = names
            .iter()
            .map(|n| format!("{n}={}", squash(&w.oo(&["eval", "--", &format!("_.{n}")], t).0)))
            .collect::<Vec<_>>()
            .join(" | ");
        let got = format!("{} {} {} {}", rcs.join(" "), w.root_tail(), sha16(&readings), w.bytes());
        check(&mut bad, label, got, want);
    }
    verdict(bad, shapes.len());
}
