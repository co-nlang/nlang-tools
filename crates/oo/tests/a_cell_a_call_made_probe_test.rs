// Q-136 probe: a cell a call made.
//
// D132 (SPEC_10 §2.3.1): the walk that looks for bottoms costs the cells
// written, however many ways each record has to the next, and a cell made
// by a call is a cell. v0.135.0 kept the first and broke the second, and
// paid for the first where calls are concerned:
//
// *   a value a call made was walked again under every cell that held it:
//     `id: (x -> [x])`, `k_i: id k_{i+1}` has N × depth cells, and each was
//     forced again (v0.134.0 N=250 evolves in 6.7 s, v0.135.0 in 18.8 s;
//     ending in `~%Env.get`, 63 s and more than 120);
// *   each cell forced worked its chain out from the start: the walk's
//     context has no memo (a fresh `EvalContext`), so Q-134's "a name is one
//     value in one observation" did not hold there;
// *   applying a function to a record forced every named field of the
//     argument, and a field `id n_k` applies again: in a ring with two ways
//     per record, the whole ring per cell (L=64 12 s, every cell ~90 ms);
// *   the walk passed over a thunk that carries its input — every field of
//     a record a call made that reads the argument — so a bottom there was
//     never found: a late one landed (`id: (x -> { v: x, z: x + 1 })`,
//     `k0: id q`, then `q: "s"`: v0.134.0 refuses at `k0.z`, v0.135.0 takes
//     it), and the commit did not name it (SPEC_10 §4.1.2, D56).
//
// r1 — the walk costs the cells written where calls make them: a chain of
//      calls (250 and 1000 links), a ring of 64 records with two ways
//      through a call, a chain of 100 calls making records; each evolves
//      within its bound (v0.134.0 and v0.135.0 do not).
// r2 — a late bottom in a record a call made is refused, at the coordinate
//      v0.134.0 gives: the field (`k0.z`), the field of a record holding the
//      call (`k0.r.z`), an element of a list a call made (`k0.0`); a late
//      argument (`a: mkz qq`, then `qq: { v: "s" }`).
// r3 — the commit names a bottom a call made, once, at its own cell: a field
//      of a record a call made (`k0.z k1.z`); not again in a copy a call
//      made of a record that holds one (`k3.z`, `n1.z`).
// g1 — what is stored and read does not change: roots and readings of
//      shapes with calls, as v0.135.0 (and the order of writes); an effect
//      in an inline record's field a function does not read is done once,
//      as in v0.135.0 (the walk does no effect, so not forcing an
//      argument's fields is the walk's alone).
//
// Not here: past the depth horizon each cell's value differs (Q-120: each
// cell has its own budget; Q-134: a value that met the horizon is not kept),
// so a chain longer than the horizon walks to it once per cell (the IO chain
// of 250 links evolves in about 60 s, v0.134.0 63 s; the walk off, 19 s).

use std::fs;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

// r2 / r3 / g1: what the reference (q136-ref) answers.
const WANT_LATE_FIELD: &str = "e0 e1 | Error: Evolution Conflict in \"f1.n\": #conflict at k0.z";
const WANT_LATE_HELD: &str = "e0 e1 | Error: Evolution Conflict in \"f1.n\": #conflict at k0.r.z";
const WANT_LATE_LIST: &str = "e0 e1 | Error: Evolution Conflict in \"f1.n\": #conflict at k0.0";
const WANT_LATE_ARG: &str = "e0 c0 e1 | Error: Evolution Conflict in \"f2.n\": #conflict at n0.a.y";
const WANT_LIST_MADE: &str = "c0 k0.z k1.z";
const WANT_LIST_CHAIN: &str = "c0 k3.z";
const WANT_LIST_RING: &str = "c0 n1.z";
const WANT_G1: &[(&str, &str)] = &[
    ("call chain 5", "e0 | c0 99150fd2a5c7970f | _.k0=[[[[[7]]]]] | _.k4=[7] | bytes 0"),
    ("records a call made", "e0 | c0 75489ced0f06cde8 | _.k0.v.w.v={ v: { v: 7 w: 7 } w: { v: 7 w: 7 } } | _.k3={ v: { v: 7 w: 7 } w: { v: 7 w: 7 } } | bytes 0"),
    ("call ring 3", "e0 | c0 bf5baba597cab9e2 | _.n0.c0.v=1 | _.n2.c1.c0.v=1 | bytes 0"),
    ("call ring 3 in halves", "e0 e0 | c0 bf5baba597cab9e2 | _.n0.c0.v=1 | _.n2.c1.c0.v=1 | bytes 0"),
    ("an argument's effect field", "e0 | c0 5ee553e400460fd4 | _.m=2 ;; %effect: #io | _.r.a=\"alpha\" ;; %effect: #io | bytes 0"),
    ("a bottom a call made, read", "e0 | c0 dff8e8f9cd3c2827 | _.k0.v=7 | _.k0.z=_|_ ;; %cause: #conflict ;; Incompatible types: 1 vs 2 | bytes 0"),
    ("an inline record's effect, unread", "e0 | c0 0ba665fb71778403 | _.m=1 | bytes 1"),
];

struct Ws {
    _scratch: nlang_interpreter::ScratchDir,
    root: std::path::PathBuf,
    ws: std::path::PathBuf,
}

impl Ws {
    fn new(tag: &str) -> Self {
        let scratch = nlang_interpreter::ScratchDir::new(&format!("call-made-{tag}"));
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
            .env("Q136_A", "alpha")
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

/// `id: (x -> <body>)`, `k_i: id k_{i+1}` for i < n, `k_n: <last>`.
fn call_chain(n: usize, body: &str, last: &str) -> String {
    let mut s = format!("id: (x -> {body})\n");
    for i in 0..n {
        s.push_str(&format!("k{i}: id k{}\n", i + 1));
    }
    s.push_str(&format!("k{n}: {last}"));
    s
}

/// A ring of `l` records, each reaching the next twice through `id`.
fn call_ring(l: usize) -> String {
    let mut s = String::from("id: (x -> x)\n");
    for i in 0..l {
        let j = (i + 1) % l;
        s.push_str(&format!("n{i}: {{ v: {i}, c0: id n{j}, c1: id n{j} }}\n"));
    }
    s
}

/// Steps: ("e", src) evolves a file, ("c", "") commits. Each step's letter
/// and rc, then the last step's output.
fn steps(w: &Ws, st: &[(&str, &str)]) -> String {
    let mut codes = Vec::new();
    let mut last = String::new();
    for (k, (op, src)) in st.iter().enumerate() {
        let (o, rc, _) = if *op == "e" {
            let name = format!("f{k}.n");
            w.file(&name, src);
            w.oo(&["evolve", &name], Duration::from_secs(60))
        } else {
            w.oo(&["commit", "-m", "a"], Duration::from_secs(60))
        };
        codes.push(if o == "DID NOT RETURN" || o == "CRASHED" || o == "PANIC" { o.clone() } else { format!("{op}{rc}") });
        last = squash(&o);
        if rc != 0 {
            break;
        }
    }
    format!("{} | {}", codes.join(" "), if codes.last().is_some_and(|c| c.ends_with('0')) { "" } else { last.as_str() })
}

/// The coordinates a commit names, sorted.
fn listed(w: &Ws, src: &str) -> String {
    w.file("f.n", src);
    let (e, erc, _) = w.oo(&["evolve", "f.n"], Duration::from_secs(60));
    if erc != 0 {
        return format!("evolve rc={erc} {}", squash(&e));
    }
    let (c, crc, _) = w.oo(&["commit", "-m", "a"], Duration::from_secs(60));
    let mut at: Vec<&str> = c.split("#conflict at ").skip(1).filter_map(|t| t.split_whitespace().next()).collect();
    at.sort();
    format!("c{crc} {}", at.join(" "))
}

#[test]
fn r1_the_walk_costs_the_cells_a_call_made() {
    // (label, source, bound in seconds)
    let shapes: Vec<(String, String, f64)> = vec![
        ("call chain 250".into(), call_chain(250, "[x]", "7"), 5.0),
        ("call chain 1000".into(), call_chain(1000, "[x]", "7"), 6.0),
        ("call ring 64".into(), call_ring(64), 5.0),
        ("records a call made 100".into(), call_chain(100, "{ v: x }", "7"), 4.0),
    ];
    let mut bad = Vec::new();
    for (label, src, bound) in &shapes {
        let w = Ws::new("r1");
        w.file("f.n", src);
        let (o, rc, secs) = w.oo(&["evolve", "f.n"], Duration::from_secs_f64(bound + 5.0));
        let got = if rc != 0 || o == "DID NOT RETURN" {
            format!("{} rc={rc}", squash(&o).chars().take(80).collect::<String>())
        } else if secs > *bound {
            format!("e0 in {secs:.1} s")
        } else {
            "e0 within bound".into()
        };
        check(&mut bad, label, got, "e0 within bound");
    }
    verdict(bad, shapes.len());
}

#[test]
fn r2_a_late_bottom_in_a_record_a_call_made_is_refused() {
    let cases: Vec<(&str, Vec<(&str, &str)>, &str)> = vec![
        ("the field", vec![("e", "id: (x -> { v: x, z: x + 1 })\nk0: id q"), ("e", "q: \"s\"")], WANT_LATE_FIELD),
        ("a record holding the call", vec![("e", "id: (x -> { v: x, z: x + 1 })\nk0: { r: id q }"), ("e", "q: \"s\"")], WANT_LATE_HELD),
        ("an element of a list", vec![("e", "id: (x -> [x + 1])\nk0: id q"), ("e", "q: \"s\"")], WANT_LATE_LIST),
        (
            "a late argument",
            vec![("e", "mkz: (x -> { p: x, y: x.v + 1 })\nn0: { v: 0, a: mkz qq }"), ("c", ""), ("e", "qq: { v: \"s\" }")],
            WANT_LATE_ARG,
        ),
    ];
    let mut bad = Vec::new();
    for (label, st, want) in &cases {
        let w = Ws::new("r2");
        check(&mut bad, label, steps(&w, st), want);
    }
    verdict(bad, cases.len());
}

#[test]
fn r3_the_commit_names_a_bottom_a_call_made_once() {
    let cases: Vec<(&str, String, &str)> = vec![
        ("a field of a record a call made", "id: (x -> { v: x, z: 1 & 2 })\nk0: id k1\nk1: id k2\nk2: 7".into(), WANT_LIST_MADE),
        ("a chain of copies", call_chain(3, "[x]", "{ z: 1 & 2 }"), WANT_LIST_CHAIN),
        (
            "a ring of lists",
            "idl: (x -> [x])\nn0: { v: 0, c0: idl n1, c1: idl n1 }\nn1: { v: 1, c0: idl n0, z: 1 & 2 }".into(),
            WANT_LIST_RING,
        ),
    ];
    let mut bad = Vec::new();
    for (label, src, want) in &cases {
        let w = Ws::new("r3");
        check(&mut bad, label, listed(&w, src), want);
    }
    verdict(bad, cases.len());
}

#[test]
fn g1_what_is_stored_and_read_does_not_change() {
    let shapes: Vec<(&str, Vec<(&str, String)>, Vec<&str>)> = vec![
        ("call chain 5", vec![("e", call_chain(5, "[x]", "7"))], vec!["_.k0", "_.k4"]),
        ("records a call made", vec![("e", call_chain(5, "{ v: x, w: x }", "7"))], vec!["_.k0.v.w.v", "_.k3"]),
        ("call ring 3", vec![("e", call_ring(3))], vec!["_.n0.c0.v", "_.n2.c1.c0.v"]),
        (
            "call ring 3 in halves",
            vec![("e", "id: (x -> x)\nn0: { v: 0, c0: id n1, c1: id n1 }".into()), ("e", "n1: { v: 1, c0: id n2, c1: id n2 }\nn2: { v: 2, c0: id n0, c1: id n0 }".into())],
            vec!["_.n0.c0.v", "_.n2.c1.c0.v"],
        ),
        (
            "an argument's effect field",
            vec![("e", "f: (x -> x.b)\nr: { a: ~%Env.get \"Q136_A\", b: 2 }\nm: f r".into())],
            vec!["_.m", "_.r.a"],
        ),
        ("a bottom a call made, read", vec![("e", "id: (x -> { v: x, z: 1 & 2 })\nk0: id 7".into())], vec!["_.k0.v", "_.k0.z"]),
        (
            "an inline record's effect, unread",
            vec![("e", "f: (x -> 1)\nm: f { a: ~%Io./append_file (\"a.log\", \"x\") }".into())],
            vec!["_.m"],
        ),
    ];
    let mut bad = Vec::new();
    for (k, (label, st, reads)) in shapes.iter().enumerate() {
        let w = Ws::new("g1");
        let st: Vec<(&str, &str)> = st.iter().map(|(a, b)| (*a, b.as_str())).collect();
        let mut got = steps(&w, &st);
        if got.ends_with("| ") {
            let (c, crc, _) = w.oo(&["commit", "-m", "a"], Duration::from_secs(60));
            got = format!("{got}c{crc} {}", if crc == 0 { w.root_tail() } else { squash(&c) });
            for q in reads {
                let (o, _, _) = w.oo(&["eval", "--", q], Duration::from_secs(60));
                got.push_str(&format!(" | {q}={}", squash(&o)));
            }
            got.push_str(&format!(" | bytes {}", w.bytes()));
        }
        let want = WANT_G1.get(k).map(|(l, v)| if l == label { *v } else { "label mismatch" }).unwrap_or("missing");
        check(&mut bad, label, got, want);
    }
    verdict(bad, shapes.len());
}
