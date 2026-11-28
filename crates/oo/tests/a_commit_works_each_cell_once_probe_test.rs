// Q-138 probe: a commit works each cell once.
//
// D132 (SPEC_10 §2.3.1): what a judgement costs is the cells written. The
// commit's projection (`project_for_commit`, D46) decides, field by field,
// whether a cell is stored as its recipe or as its answer: a predicted-pure
// definition that does an effect when worked out keeps its answer (Q-126,
// D131). v0.137.0 asks that by working each such definition out to the
// bottom, and pays it twice over:
//
// *   a record reached again is worked out again: a ring where every record
//     has two ways to the next (`n_i: { v: i, w0: n_{i+1}, w1: n_{i+1} }`)
//     costs 2^L at commit — L=16 31 s, L=64 does not return — though the
//     evolve of the same ring takes 0.15 s, and though nothing in it does
//     an effect, so every answer worked out is thrown away;
// *   a coordinate is read once per reader: `id: (x -> { v: x })`,
//     `k_i: id k_{i+1}` works the rest of the chain out under every link —
//     N² calls — and each record a call made stays kept as one value
//     (Q-126), N³ in memory: N=100 aborts at 2.4 GB. The evolve of the same
//     chain reads each coordinate once (Q-134): 1.0 s, 100 MB.
//
// r1 — a ring with two ways in every record commits within its bound, its
//      ways read directly, through a call and through a list (v0.137.0 does
//      not return).
// r2 — a chain of records a call makes commits within its bound (v0.137.0
//      aborts).
// g1 — what is stored, read and done does not change, as v0.137.0: roots,
//      readings, an effect's bytes; where a ring holds an effect the
//      answers kept are the ones v0.137.0 keeps — a call reached from a
//      record that already met the node keeps its answer; and in a ring
//      committed half by half, the answer stored is the one worked out in
//      full, not the one asked with each node met once (they differ there).
//
// Not here: reading a committed ring back (`a: { e: x, x: a }`, then
// `oo eval _.a`) unrolls it to the depth horizon and aborts at 2.8 GB —
// printing, not the commit. A ring that holds an effect stores its answers
// unrolled, as v0.137.0 does: the value is not shared.

use std::fs;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

// g1: what v0.137.0 answers (measured on its tag build).
const WANT_G1: &[(&str, &str)] = &[
    ("a ring that reads an effect, two ways", r#"e0 | c0 630feb389e9345d2 | _.n0.w1.v=1 ;; %effect: #io | _.n0.w0.w1.g="alpha" ;; %effect: #io | bytes 0"#),
    ("a ring through a call that reads an effect", r#"e0 | c0 a961a971641a4ebb | _.n0.c1.v=1 ;; %effect: #io | _.n0.c0.c1.g="alpha" ;; %effect: #io | bytes 0"#),
    ("a ring through a list that reads an effect", r#"e0 | c0 db1456d7385ebb30 | _.n0.l1.0.v=1 ;; %effect: #io | _.n2.l0.0.g="alpha" ;; %effect: #io | bytes 0"#),
    ("a node met where its effect is on the path, then by a call", r#"e0 | c0 0b01d419d4161d61 | _.z.0.y.g="alpha" ;; %effect: #io | _.n0.x.y.g="alpha" ;; %effect: #io | bytes 0"#),
    ("an effect done once, read through a ring and a call", r#"e0 | c0 7e2ea419b3f81261 | _.z.0.w0.g=#true ;; %effect: #io | _.n1.w0.g=#true ;; %effect: #io | bytes 1"#),
    ("a pure ring, two ways", r#"e0 | c0 2b6db40b0b6bb876 | _.n0.w1.w0.v=2 | bytes 0"#),
    ("a chain of records a call makes", r#"e0 | c0 b66df7737aebc428 | _.k0.v.v.v={ v: { v: { v: { v: { v: { v: { v: { v: { v: { v: { v: { v: { v: { v: { v: { v: { v: 7 } } } } } } } } } } } } } } } } } | _.k18={ v: { v: 7 } } | bytes 0"#),
    ("a ring through a call that makes a record twice, committed half by half", r#"e0 | c0 4bf6045902ff147e | e0 | c0 b557e70a2f9f173d | _.n0.c0.p.v=1 ;; %effect: #io | _.n1.c0.p.c0.q.v=1 ;; %effect: #io | bytes 0"#),
    ("a ring that reads an effect, two ways, committed half by half", r#"e0 | c0 b9bce2584bfb91eb | e0 | c0 dbd2b07847f49fa4 | _.n0.w0.v=1 ;; %effect: #io | _.n4.w0.w0.v=1 ;; %effect: #io | bytes 0"#),
];

struct Ws {
    _scratch: nlang_interpreter::ScratchDir,
    root: std::path::PathBuf,
    ws: std::path::PathBuf,
}

impl Ws {
    fn new(tag: &str) -> Self {
        let scratch = nlang_interpreter::ScratchDir::new(&format!("each-cell-once-{tag}"));
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
            .env("Q138_A", "alpha")
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

    fn file(&self, name: &str, src: &str) {
        fs::write(self.ws.join(name), format!("{src}\n")).unwrap();
    }

    /// Bytes an effect appended to `a.log`.
    fn bytes(&self) -> usize {
        fs::read(self.ws.join("a.log")).map(|b| b.len()).unwrap_or(0)
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
}

fn squash(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn check(bad: &mut Vec<String>, label: &str, got: String, want: &str) {
    if got != want {
        bad.push(format!("{label}\n      got:  {got}\n      want: {want}"));
    }
}

fn verdict(bad: Vec<String>, n: usize) {
    assert!(bad.is_empty(), "{} of {n} wrong:\n  {}", bad.len(), bad.join("\n  "));
}

/// A ring of `l` records, each with `m` ways to the next: read directly
/// (`w_j: n_{i+1}`), through a call (`c_j: id n_{i+1}`) or through a list
/// (`l_j: [n_{i+1}]`); `extra` is written into every record.
fn ring(l: usize, m: usize, way: &str, extra: &str) -> String {
    let mut s = String::new();
    if way == "call" {
        s.push_str("id: (x -> x)\n");
    }
    for i in 0..l {
        let next = format!("n{}", (i + 1) % l);
        let mut f = vec![format!("v: {i}")];
        for j in 0..m {
            f.push(match way {
                "call" => format!("c{j}: id {next}"),
                "list" => format!("l{j}: [{next}]"),
                _ => format!("w{j}: {next}"),
            });
        }
        if !extra.is_empty() {
            f.push(extra.to_string());
        }
        s.push_str(&format!("n{i}: {{ {} }}\n", f.join(", ")));
    }
    s
}

/// `id: (x -> { v: x })`, `k_i: id k_{i+1}` for i < n, `k_n: 7`.
fn record_chain(n: usize) -> String {
    let mut s = "id: (x -> { v: x })\n".to_string();
    for i in 0..n {
        s.push_str(&format!("k{i}: id k{}\n", i + 1));
    }
    s.push_str(&format!("k{n}: 7"));
    s
}

/// One step within `bound` seconds: "ok", or what it did. Killed at 60 s.
fn timed(w: &Ws, args: &[&str], bound: f64) -> String {
    let (o, rc, secs) = w.oo(args, Duration::from_secs(60));
    if rc != 0 || o == "DID NOT RETURN" {
        format!("{} rc={rc}", squash(&o).chars().take(80).collect::<String>())
    } else if secs > bound {
        format!("{secs:.1} s")
    } else {
        "ok".into()
    }
}

#[test]
fn r1_a_ring_with_two_ways_commits_at_the_cost_of_its_cells() {
    let mut bad = Vec::new();
    // The reference: 0.36, 0.45, 0.50 s. v0.137.0: L=16 31 s, L=64 does not return.
    for way in ["plain", "call", "list"] {
        let w = Ws::new("r1");
        w.file("f.n", &ring(64, 2, way, ""));
        let (o, rc, _) = w.oo(&["evolve", "f.n"], Duration::from_secs(60));
        assert_eq!(rc, 0, "the ring evolves: {}", squash(&o));
        check(&mut bad, &format!("a ring of 64, two ways each, {way}"), timed(&w, &["commit", "-m", "a"], 3.0), "ok");
    }
    verdict(bad, 3);
}

#[test]
fn r2_a_chain_of_records_a_call_makes_commits() {
    let w = Ws::new("r2");
    w.file("f.n", &record_chain(100));
    let (o, rc, _) = w.oo(&["evolve", "f.n"], Duration::from_secs(60));
    assert_eq!(rc, 0, "the chain evolves: {}", squash(&o));
    let mut bad = Vec::new();
    // The reference: 1.4 s, 246 MB. v0.137.0: aborts at 2.4 GB in 10.7 s.
    check(&mut bad, "a chain of 100 records a call makes", timed(&w, &["commit", "-m", "a"], 5.0), "ok");
    verdict(bad, 1);
}

#[test]
fn g1_what_is_stored_read_and_done_does_not_change() {
    let env = "g: ~%Env.get \"Q138_A\"";
    let log = "~%Io./append_file (\"a.log\", \"x\")";
    let mk2 = "mk2: (x -> { p: x, q: x })";
    // (label, source, readings); a source in parts is evolved and committed part by part.
    let shapes: Vec<(&str, String, Vec<&str>)> = vec![
        ("a ring that reads an effect, two ways", ring(3, 2, "plain", env), vec!["_.n0.w1.v", "_.n0.w0.w1.g"]),
        ("a ring through a call that reads an effect", ring(3, 2, "call", env), vec!["_.n0.c1.v", "_.n0.c0.c1.g"]),
        ("a ring through a list that reads an effect", ring(3, 2, "list", env), vec!["_.n0.l1.0.v", "_.n2.l0.0.g"]),
        (
            "a node met where its effect is on the path, then by a call",
            format!("n0: {{ {env}, x: n1 }}\nn1: {{ y: n0 }}\nid: (x -> [x])\nz: id n1"),
            vec!["_.z.0.y.g", "_.n0.x.y.g"],
        ),
        (
            "an effect done once, read through a ring and a call",
            format!("n0: {{ g: {log}, w0: n1, w1: n1 }}\nn1: {{ w0: n0, w1: n0 }}\nid: (x -> [x])\nz: id n1"),
            vec!["_.z.0.w0.g", "_.n1.w0.g"],
        ),
        ("a pure ring, two ways", ring(8, 2, "plain", ""), vec!["_.n0.w1.w0.v"]),
        ("a chain of records a call makes", record_chain(20), vec!["_.k0.v.v.v", "_.k18"]),
        (
            "a ring through a call that makes a record twice, committed half by half",
            format!("{mk2}\nn0: {{ v: 0, c0: mk2 n1, {env} }}\n----\nn1: {{ v: 1, c0: mk2 n0, {env} }}"),
            vec!["_.n0.c0.p.v", "_.n1.c0.p.c0.q.v"],
        ),
        (
            "a ring that reads an effect, two ways, committed half by half",
            format!("{}\n----\n{}", ring(5, 2, "plain", env).lines().take(3).collect::<Vec<_>>().join("\n"), ring(5, 2, "plain", env).lines().skip(3).collect::<Vec<_>>().join("\n")),
            vec!["_.n0.w0.v", "_.n4.w0.w0.v"],
        ),
    ];
    let mut bad = Vec::new();
    for (k, (label, src, reads)) in shapes.iter().enumerate() {
        let w = Ws::new("g1");
        let mut got = Vec::new();
        for part in src.split("\n----\n") {
            w.file("f.n", part);
            let (e, erc, _) = w.oo(&["evolve", "f.n"], Duration::from_secs(60));
            got.push(format!("e{erc}{}", if erc == 0 { String::new() } else { format!(" {}", squash(&e)) }));
            let (c, crc, _) = w.oo(&["commit", "-m", "a"], Duration::from_secs(60));
            got.push(format!("c{crc}{} {}", if crc == 0 { String::new() } else { format!(" {}", squash(&c)) }, w.root_tail()));
        }
        for q in reads {
            let (o, _, _) = w.oo(&["eval", "--", q], Duration::from_secs(60));
            got.push(format!("{q}={}", squash(&o)));
        }
        got.push(format!("bytes {}", w.bytes()));
        let want = WANT_G1.get(k).map(|(l, v)| if l == label { *v } else { "label mismatch" }).unwrap_or("missing");
        check(&mut bad, label, got.join(" | "), want);
    }
    verdict(bad, shapes.len());
}
