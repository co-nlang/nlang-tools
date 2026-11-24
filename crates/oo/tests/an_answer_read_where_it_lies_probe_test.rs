// Q-137 probe: an answer read where it lies.
//
// D132 (SPEC_10 §2.3.1): the walk that looks for bottoms costs the cells
// written. A call whose argument does an effect keeps its answer in the
// store (D131, Q-133): `id: (x -> [x])`, `k_i: id k_{i+1}`, `k_N` holding
// `~%Env.get`, stores `k0` as `[[…[k_N's record]…]]`. That answer is N
// deep, the next is N − 1 deep, and so on: N²/2 records, every one of them
// stored. v0.136.0 walked each of them at a cost of its whole subtree:
//
// *   each record the walk passed was copied whole (`ComboVal`'s clone is
//     deep, and drops its cached digest) to be forced, and each field was
//     copied whole to be rebound — though forcing a record already worked
//     out gives it back, and rebinding touches only a thunk;
// *   to learn that a record is already worked out,
//     `solid_combo_expansion_cost` pushed a copy of every value under it.
//
// N³ in all. The commit pays it (N=100: 16 s, v0.134.0 12 s), and after the
// answer is stored so does every evolve and commit, though it writes
// nothing near the chain (`x: 1`: evolve 14 s, commit 9 s; v0.134.0 2 s and
// 3 s). Evaluation pays the second one too, wherever it forces a value:
// the first evolve of the same file takes 9 s.
//
// r1 — the walk reads a stored answer where it lies: the commit that stores
//      it, then an evolve and a commit of `x: 1`, each within its bound
//      (v0.136.0 does none).
// r2 — evaluation asks once whether a record is worked out: the first
//      evolve of the same file within its bound (v0.136.0 and v0.134.0 do
//      not).
// g1 — what is stored, read, done and named does not change, as v0.136.0:
//      roots, readings, an effect's bytes (once, through a stored answer
//      and after it), the bottoms a commit names (a stored answer's
//      bottoms are its own: it holds them, it does not read them).
// Q-136's probe (its r1 bounds) is a red line beside this one.
//
// Not here: past the depth horizon a chain is not stored as answers and
// each cell walks to the horizon (Q-120, Q-134): the same file with 250
// links evolves in about 60 s (v0.134.0 67 s). A record a call makes in
// every link, and a ring with two ways through a call, still abort at
// commit (the value is not shared).

use std::fs;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

// g1: what v0.136.0 answers (measured on its tag build).
const WANT_G1: &[(&str, &str)] = &[
    ("a stored chain, then x: 1", r#"e0 | c0 [k0.0.0.0.z k1.0.0.z k2.0.z k3.z] 1985a60fdbbfc370 | e0 c0 e11e4ace42d2b7f2 | _.k0=[[[{ %effect: #io e: "alpha" ;; %effect: #io z: _|_ ;; %cause: #conflict } ;; %effect: #io]]] | _.k3.e="alpha" ;; %effect: #io | _.x=1 ;; %effect: #io | bytes 0"#),
    ("a stored chain with a bottom, named", "e0 | c0 [k0.0.0.0.z k1.0.0.z k2.0.z k3.z] 1985a60fdbbfc370 | bytes 0"),
    ("a chain of recipes with a bottom, named", "e0 | c0 [k3.z] f5ea73bbbf06c572 | bytes 0"),
    ("an effect at the end of a stored chain", "e0 | c0 [] 7be9c74164a397c1 | e0 c0 f29f3fedce36c41f | _.k0=[[[{ %effect: #io e: #true ;; %effect: #io } ;; %effect: #io]]] | _.k3.e=#true ;; %effect: #io | bytes 1"),
    ("an inline record's effect, unread", "e0 | c0 [] 0ba665fb71778403 | _.m=1 | bytes 1"),
];

struct Ws {
    _scratch: nlang_interpreter::ScratchDir,
    root: std::path::PathBuf,
    ws: std::path::PathBuf,
}

impl Ws {
    fn new(tag: &str) -> Self {
        let scratch = nlang_interpreter::ScratchDir::new(&format!("answer-lies-{tag}"));
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
            .env("Q137_A", "alpha")
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

/// A chain whose last record does an effect, so each call keeps its answer.
fn stored_chain(n: usize) -> String {
    call_chain(n, "[x]", "{ z: 1 & 2, e: ~%Env.get \"Q137_A\" }")
}

/// One step within `bound` seconds: "ok", or what it did. Killed at 60 s, so
/// that a slow step still lands and the next one reads what it stored.
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
fn r1_the_walk_reads_a_stored_answer_where_it_lies() {
    let w = Ws::new("r1");
    w.file("f.n", &stored_chain(100));
    w.file("g.n", "x: 1");
    let (o, rc, _) = w.oo(&["evolve", "f.n"], Duration::from_secs(60));
    assert_eq!(rc, 0, "the chain evolves: {}", squash(&o));
    let mut bad = Vec::new();
    // (label, step, bound in seconds)
    let st: [(&str, &[&str], f64); 3] = [
        // v0.134.0: 11.9 s, 2.1 s, 3.1 s. The evolve's bound sits between the
        // reference (1.7 s) and a walk that still copies each record it
        // passes, or each field (3.5-4.3 s): v0.134.0's own second.
        ("the commit that stores the answers", &["commit", "-m", "a"], 7.0),
        ("an evolve of x: 1 after", &["evolve", "g.n"], 2.5),
        ("a commit of x: 1 after", &["commit", "-m", "b"], 5.0),
    ];
    for (label, args, bound) in st {
        check(&mut bad, label, timed(&w, args, bound), "ok");
    }
    verdict(bad, 3);
}

#[test]
fn r2_evaluation_asks_once_whether_a_record_is_worked_out() {
    let w = Ws::new("r2");
    w.file("f.n", &stored_chain(100));
    let mut bad = Vec::new();
    check(&mut bad, "the first evolve of a stored chain", timed(&w, &["evolve", "f.n"], 4.0), "ok");
    verdict(bad, 1);
}

#[test]
fn g1_what_is_stored_read_done_and_named_does_not_change() {
    let log = "~%Io./append_file (\"a.log\", \"x\")";
    // (label, source, then `x: 1`, readings)
    let shapes: Vec<(&str, String, bool, Vec<&str>)> = vec![
        ("a stored chain, then x: 1", stored_chain(3), true, vec!["_.k0", "_.k3.e", "_.x"]),
        ("a stored chain with a bottom, named", stored_chain(3), false, vec![]),
        ("a chain of recipes with a bottom, named", call_chain(3, "[x]", "{ z: 1 & 2 }"), false, vec![]),
        ("an effect at the end of a stored chain", call_chain(3, "[x]", &format!("{{ e: {log} }}")), true, vec!["_.k0", "_.k3.e"]),
        ("an inline record's effect, unread", format!("f: (x -> 1)\nm: f {{ a: {log} }}"), false, vec!["_.m"]),
    ];
    let mut bad = Vec::new();
    for (k, (label, src, then, reads)) in shapes.iter().enumerate() {
        let w = Ws::new("g1");
        w.file("f.n", src);
        w.file("g.n", "x: 1");
        let mut got = Vec::new();
        let (e, erc, _) = w.oo(&["evolve", "f.n"], Duration::from_secs(60));
        got.push(format!("e{erc}{}", if erc == 0 { String::new() } else { format!(" {}", squash(&e)) }));
        let (c, crc, _) = w.oo(&["commit", "-m", "a"], Duration::from_secs(60));
        let mut at: Vec<&str> = c.split("#conflict at ").skip(1).filter_map(|t| t.split_whitespace().next()).collect();
        at.sort();
        got.push(format!("c{crc} [{}] {}", at.join(" "), w.root_tail()));
        if *then {
            let (_, erc, _) = w.oo(&["evolve", "g.n"], Duration::from_secs(60));
            let (_, crc, _) = w.oo(&["commit", "-m", "b"], Duration::from_secs(60));
            got.push(format!("e{erc} c{crc} {}", w.root_tail()));
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
