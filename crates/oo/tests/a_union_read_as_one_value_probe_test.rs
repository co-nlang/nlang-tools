// A union read as one value.
// Order: nlang-tools/docs/a_union_read_as_one_value_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-112.  A value context reads one
// value. A union read there distributes: each branch is read alone and the
// answers are joined, ⊥ branches pruned (SPEC_08 §3.2.2 #7, SPEC_07 §4
// bind additivity). The value contexts are SYNTAX_06 §4 #6's list: a
// builtin's operands and the elements it reads, a predicate's or key
// function's answer, the ternary condition, a range bound, interpolation
// and `~%Str./format`, `==`/`!=` (D123, 2026-10-29, the user, 甲:
// `(1 | 2) == 1` is `#false | #true`). A horizon read there is the answer
// (G3, SPEC_08 §3.2.2 #1). What a builtin hands back or applies stays
// whole; the `=` family compares as `=` does (SYNTAX_06 §4 #6, #11).
//
// ── What is wrong (v0.112.0) ────────────────────────────────────────────
//
//   The same reading answers differently by the way in: `(1 | 2) + 10` is
//   `11 | 12`, `~%Math./add (1 | 2, 10)` is ⊥; `~%List./sum ([1 | 2, 10])`
//   is 10 (the element dropped); `(#true | #false) ? 1 : 2` is `_`;
//   `` `v=${1 | 2}` `` is "v=(...|...)"; `(1 | 2) == 1` is `#false`;
//   `~%List./sort ([3, 1 | 9, 2])` is `[1 | 9, 2, 3]`; a predicate answering
//   `#true | #false` counts nothing. A horizon given to `~%Math./add` is ⊥
//   `#conflict`, to `~%List./sum` dropped, to interpolation a string.
//
// Every `oo` here runs under a 1 GB address-space cap and a 10 s kill.
//
// The delivery may NOT edit this file. `rustfmt` must not touch it.


#![allow(dead_code)]

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const KILL_AFTER: Duration = Duration::from_secs(10);

struct Ws {
    _scratch: nlang_interpreter::ScratchDir,
    root: PathBuf,
    ws: PathBuf,
}

impl Ws {
    fn new(tag: &str) -> Self {
        let scratch = nlang_interpreter::ScratchDir::new(&format!("union-{tag}"));
        let root = scratch.path().to_path_buf();
        let ws = root.join("ws");
        fs::create_dir_all(&ws).unwrap();
        Ws { _scratch: scratch, root, ws }
    }

    /// Run `oo` capped at 1 GB, killed after 10 s: (output, rc). A run
    /// that is killed or dies without a word says so.
    fn oo(&self, args: &[&str]) -> (String, i32) {
        let mut child = Command::new("sh")
            .arg("-c")
            .arg("ulimit -v 1000000; exec \"$0\" \"$@\"")
            .arg(env!("CARGO_BIN_EXE_oo"))
            .args(args)
            .current_dir(&self.ws)
            .env("OO_IDENTITY", self.root.join("identity"))
            .env("OO_NODE_HOME", self.root.join("node-home"))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("oo runs");
        let start = Instant::now();
        loop {
            if child.try_wait().expect("wait").is_some() {
                break;
            }
            if start.elapsed() > KILL_AFTER {
                let _ = child.kill();
                let _ = child.wait();
                return ("DID NOT RETURN".into(), -1);
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let o = child.wait_with_output().expect("output");
        let text = format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
        let rc = o.status.code().unwrap_or(-1);
        if text.contains("panicked") {
            return ("PANIC".into(), rc);
        }
        if text.contains("stack overflow") || text.contains("memory allocation") || (rc != 0 && text.trim().is_empty()) {
            return ("CRASHED".into(), rc);
        }
        (text.trim().to_string(), rc)
    }

    /// Evolve `src` from a new file: the exit code, or the crash.
    fn evolve(&self, src: &str) -> String {
        let n = fs::read_dir(&self.ws).map(|d| d.count()).unwrap_or(0);
        let f = format!("f{n}.n");
        fs::write(self.ws.join(&f), src).unwrap();
        let (o, rc) = self.oo(&["evolve", &f]);
        if o == "DID NOT RETURN" || o == "CRASHED" || o == "PANIC" { o } else { format!("e{rc}") }
    }

    /// Evolve each source as its own file, then commit.
    fn writes(&self, srcs: &[&str]) -> String {
        let mut out: Vec<String> = srcs.iter().map(|s| self.evolve(s)).collect();
        let (o, rc) = self.oo(&["commit", "-m", "a"]);
        out.push(if o == "DID NOT RETURN" || o == "CRASHED" || o == "PANIC" { o } else { format!("c{rc}") });
        out.join(" ")
    }

    /// `oo eval`, the trailing effect note dropped, whitespace collapsed.
    fn eval(&self, e: &str) -> String {
        let s = self.oo(&["eval", e]).0;
        let s = match s.find(";; %effect") {
            Some(i) => s[..i].trim().to_string(),
            None => s,
        };
        squash(&s)
    }

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

/// A ⊥ or a horizon reduced to its cause.
fn short(got: &str) -> String {
    for cause in ["#conflict", "#missing_key", "#no_matching_branch", "#fuel_exhausted"] {
        if (got.starts_with("_|_") || got.starts_with("#blur")) && got.contains(cause) {
            return format!("{} {cause}", got.split_whitespace().next().unwrap_or(""));
        }
    }
    got.into()
}

/// `oo eval` from an empty directory, the effect note dropped, whitespace
/// collapsed, not cut short.
fn eval_full(e: &str) -> String {
    Ws::new("eval").eval(e)
}

fn eval_all(srcs: &[String]) -> Vec<String> {
    let mut out = vec![String::new(); srcs.len()];
    let per = srcs.len().div_ceil(8).max(1);
    std::thread::scope(|s| {
        for (o, i) in out.chunks_mut(per).zip(srcs.chunks(per)) {
            s.spawn(move || {
                for (o, e) in o.iter_mut().zip(i) {
                    *o = eval_full(e);
                }
            });
        }
    });
    out
}

fn table(bad: &mut Vec<String>, cells: &[(&str, &str)]) {
    let srcs: Vec<String> = cells.iter().map(|c| c.0.to_string()).collect();
    for ((e, want), got) in cells.iter().zip(eval_all(&srcs)) {
        check(bad, e, short(&got), want);
    }
}

/// Split at `sep` outside brackets and strings.
fn split_top(s: &str, sep: &str) -> Vec<String> {
    let (mut out, mut cur, mut depth, mut q, mut i) = (Vec::new(), String::new(), 0i32, false, 0);
    let b = s.as_bytes();
    while i < s.len() {
        let ch = b[i] as char;
        if q {
            cur.push(ch);
            q = !(ch == '"' && (i == 0 || b[i - 1] != b'\\'));
            i += 1;
            continue;
        }
        match ch {
            '"' => q = true,
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            _ => {}
        }
        if depth == 0 && s[i..].starts_with(sep) {
            out.push(cur.trim().to_string());
            cur.clear();
            i += sep.len();
            continue;
        }
        cur.push(ch);
        i += 1;
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}

/// A union's members in one order, each once.
fn members(s: &str) -> String {
    let mut m = split_top(s, " | ");
    m.sort();
    m.dedup();
    m.join(" | ")
}

const H: &str = "(~%List./sum (~%List./range (0, 1000000)))";

/// r1 — A union in a value context distributes: a builtin's operand and
/// the elements it reads, a predicate's and a key function's answer, the
/// ternary condition, a range bound, interpolation and `~%Str./format`,
/// `==`/`!=` (D123). Two names reading one union are read apart, as
/// `x + x` reads them. The answers are joined by the union's own rules.
#[test]
fn r1_a_union_in_a_value_context_distributes() {
    let mut bad = Vec::new();
    let cells = [
        ("(#true | #false) ? 1 : 2", "1 | 2"),
        ("{ c: #true | #false, r: c ? \"y\" : \"n\" }.r", "\"n\" | \"y\""),
        ("((#true | #false) ? 1 : 2) + 10", "11 | 12"),
        ("(!0) ? 1 : 2", "1 | 2"),
        ("{ c: !0, r: c ? \"y\" : \"n\" }.r", "\"n\" | \"y\""),
        ("{ a: 1 | 10, r: (a .. 20) & 3 }.r", "3"),
        ("{ a: 1 | 10, r: (a .. 20) & 15 }.r", "15"),
        ("{ b: 5 | 30, r: (0 .. b) & 20 }.r", "20"),
        ("`v=${1 | 2}`", "\"v=1\" | \"v=2\""),
        ("`${1 | 2}-${\"a\" | \"b\"}`", "\"1-a\" | \"1-b\" | \"2-a\" | \"2-b\""),
        ("`${(1 | 2) + 1}`", "\"2\" | \"3\""),
        ("~%Str./format (\"{}-{}\", [1 | 2, 3])", "\"1-3\" | \"2-3\""),
        ("~%Str./format (\"{}\", [#true | #false])", "\"#false\" | \"#true\""),
        ("(1 | 2) == 1", "#false | #true"),
        ("(1 | 2) != 1", "#false | #true"),
        ("(1 | 2) == 3", "#false"),
        ("(1 | 2) == (1 | 2)", "#false | #true"),
        ("(\"a\" | 1) == \"a\"", "#false | #true"),
        ("{ x: 1 | 2, r: x == x }.r", "#false | #true"),
        ("~%Math./add (1 | 2, 10)", "11 | 12"),
        ("~%Math./add (1 | 2, 3 | 4)", "4 | 5 | 6"),
        ("{ x: 1 | 2, r: ~%Math./add (x, x) }.r", "2 | 3 | 4"),
        ("{ x: 1 | 2, r: x + x }.r", "2 | 3 | 4"),
        ("~%Math./max (1 | 5, 3)", "3 | 5"),
        ("~%Math./gt (1 | 9, 5)", "#false | #true"),
        ("~%Str./concat (\"a\" | \"b\", \"c\")", "\"ac\" | \"bc\""),
        ("~%List./at (0 | 1, [5, 6])", "5 | 6"),
        ("~%Stat./mean (1 | 3, 5)", "3 | 4"),
        ("~%Stat./median (1 | 9, 5, 6)", "5 | 6"),
        ("~%Stat./variance (1 | 3, 5)", "1 | 4"),
        ("~%List./sum ([1 | 2, 10])", "11 | 12"),
        ("~%List./sum ([1 | 2, 10 | 20])", "11 | 12 | 21 | 22"),
        ("~%List./product ([1 | 2, 3])", "3 | 6"),
        ("~%Str./join (\",\", [\"a\" | \"b\", \"c\"])", "\"a,c\" | \"b,c\""),
        ("~%List./sort ([3, 1 | 9, 2])", "[1, 2, 3] | [2, 3, 9]"),
        ("~%List./sort ([2 | 1, 2 | 1])", "[1, 1] | [1, 2] | [2, 2]"),
        ("~%List./count ((x -> #true | #false), [1, 2])", "0 | 1 | 2"),
        ("~%List./partition ((x -> #true | #false), [1, 2])", "{ no: [1, 2] yes: [] } | { no: [1] yes: [2] } | { no: [2] yes: [1] } | { no: [] yes: [1, 2] }"),
        ("~%List./any ((x -> x), [#true | #false])", "#false | #true"),
        ("~%List./all ((x -> #true | #false), [1])", "#false | #true"),
        ("~%List./find ((x -> ~%Math./gt (x, 1 | 5)), [1, 3, 9])", "3 | 9"),
        ("~%List./max_by ((x -> (x | 0 - x)), [1, 2])", "1 | 2"),
        ("~%List./min_by ((x -> (x | 0 - x)), [1, 2])", "1 | 2"),
        ("~%Bytes./at (0 | 1, (~%Bytes./from_str (\"ab\")))", "97 | 98"),
        ("~%List./filter ((x -> ~%Math./gt (x, 1)), [1 | 3, 2])", "[1 | 3, 2] | [2]"),
        ("~%Math./shl (1 | 2, 1)", "_|_ ;; %cause: #unprovided_builtin"),
        ("~%Bytes./len (~%Bytes./from_str (\"ab\" | \"abc\"))", "2 | 3"),
    ];
    table(&mut bad, &cells);
    verdict(bad, cells.len());
}

/// r2 — A horizon in a value context is the answer (G3): not ⊥, not a
/// string, not a dropped element, not a partial, not `_`.
#[test]
fn r2_a_horizon_in_a_value_context_is_the_answer() {
    let mut bad = Vec::new();
    let cells = [
        ("(~%List./sum (~%List./range (0, 1000000))) ? 1 : 2", "#blur #fuel_exhausted"),
        ("{ c: ~%List./sum (~%List./range (0, 1000000)), r: c ? 1 : 2 }.r", "#blur #fuel_exhausted"),
        ("`v=${~%List./sum (~%List./range (0, 1000000))}`", "#blur #fuel_exhausted"),
        ("~%Str./format (\"{}\", [~%List./sum (~%List./range (0, 1000000))])", "#blur #fuel_exhausted"),
        ("~%Math./add (~%List./sum (~%List./range (0, 1000000)), 1)", "#blur #fuel_exhausted"),
        ("~%Math./max (3, ~%List./sum (~%List./range (0, 1000000)))", "#blur #fuel_exhausted"),
        ("~%Str./concat (\"a\", ~%Str./from_int (~%List./sum (~%List./range (0, 1000000))))", "#blur #fuel_exhausted"),
        ("~%List./sum ([1, ~%List./sum (~%List./range (0, 1000000))])", "#blur #fuel_exhausted"),
        ("~%Str./join (\",\", [\"a\", ~%Str./from_int (~%List./sum (~%List./range (0, 1000000)))])", "#blur #fuel_exhausted"),
        ("~%List./sort ([3, ~%List./sum (~%List./range (0, 1000000))])", "#blur #fuel_exhausted"),
        ("~%List./count ((x -> ~%List./sum (~%List./range (0, 1000000))), [1])", "#blur #fuel_exhausted"),
        ("~%List./filter ((x -> ~%List./sum (~%List./range (0, 1000000))), [1, 2])", "#blur #fuel_exhausted"),
        ("~%List./max_by ((x -> ~%List./sum (~%List./range (0, 1000000))), [1, 2])", "#blur #fuel_exhausted"),
        ("~%Stat./mean (1, ~%List./sum (~%List./range (0, 1000000)))", "#blur #fuel_exhausted"),
        ("(~%List./sum (~%List./range (0, 1000000))) == 1", "#blur #fuel_exhausted"),
        ("(~%List./sum (~%List./range (0, 1000000))) + 1", "#blur #fuel_exhausted"),
    ];
    table(&mut bad, &cells);
    verdict(bad, cells.len());
}

/// Each builtin's calls that answer a value with bare operands at v0.109.0
/// (`acc/q109/pick.json`), an atom operand or a list's first element put as
/// a union with a second value (`acc/q112/sweep.py`): (builtin, position,
/// the call, the union call, the call on the second value, the two values).
const CELLS: &[(&str, &str, &str, &str, &str, &str, &str)] = &[
    ("bytes.base64_decode", "arg0", "~%Bytes./base64_decode (\"ab\")", "~%Bytes./base64_decode (\"ab\" | \"zz\")", "~%Bytes./base64_decode (\"zz\")", "\"ab\"", "\"zz\""),
    ("bytes.from_hex", "arg0", "~%Bytes./from_hex (\"ab\")", "~%Bytes./from_hex (\"ab\" | \"zz\")", "~%Bytes./from_hex (\"zz\")", "\"ab\"", "\"zz\""),
    ("bytes.from_hex", "arg0", "~%Bytes./from_hex (\"a,b\")", "~%Bytes./from_hex (\"a,b\" | \"zz\")", "~%Bytes./from_hex (\"zz\")", "\"a,b\"", "\"zz\""),
    ("bytes.from_str", "arg0", "~%Bytes./from_str (\"ab\")", "~%Bytes./from_str (\"ab\" | \"zz\")", "~%Bytes./from_str (\"zz\")", "\"ab\"", "\"zz\""),
    ("bytes.from_str", "arg0", "~%Bytes./from_str (\"a,b\")", "~%Bytes./from_str (\"a,b\" | \"zz\")", "~%Bytes./from_str (\"zz\")", "\"a,b\"", "\"zz\""),
    ("bytes.from_str", "arg0", "~%Bytes./from_str (\"b\")", "~%Bytes./from_str (\"b\" | \"zz\")", "~%Bytes./from_str (\"zz\")", "\"b\"", "\"zz\""),
    ("bytes.slice", "arg0", "~%Bytes./slice (1, 1, (~%Bytes./from_str (\"ab\")))", "~%Bytes./slice (1 | 9, 1, (~%Bytes./from_str (\"ab\")))", "~%Bytes./slice (9, 1, (~%Bytes./from_str (\"ab\")))", "1", "9"),
    ("bytes.slice", "arg1", "~%Bytes./slice (1, 1, (~%Bytes./from_str (\"ab\")))", "~%Bytes./slice (1, 1 | 9, (~%Bytes./from_str (\"ab\")))", "~%Bytes./slice (1, 9, (~%Bytes./from_str (\"ab\")))", "1", "9"),
    ("bytes.slice", "arg0", "~%Bytes./slice (1, 2, (~%Bytes./from_str (\"ab\")))", "~%Bytes./slice (1 | 9, 2, (~%Bytes./from_str (\"ab\")))", "~%Bytes./slice (9, 2, (~%Bytes./from_str (\"ab\")))", "1", "9"),
    ("bytes.slice", "arg1", "~%Bytes./slice (1, 2, (~%Bytes./from_str (\"ab\")))", "~%Bytes./slice (1, 2 | 10, (~%Bytes./from_str (\"ab\")))", "~%Bytes./slice (1, 10, (~%Bytes./from_str (\"ab\")))", "2", "10"),
    ("complex.conj", "arg0", "~%Complex./conj (1.5)", "~%Complex./conj (1.5 | 9.5)", "~%Complex./conj (9.5)", "1.5", "9.5"),
    ("complex.conj", "arg0", "~%Complex./conj (1+2i)", "~%Complex./conj (1+2i | 9+9i)", "~%Complex./conj (9+9i)", "1+2i", "9+9i"),
    ("complex.imag", "arg0", "~%Complex./imag (1)", "~%Complex./imag (1 | 9)", "~%Complex./imag (9)", "1", "9"),
    ("complex.imag", "arg0", "~%Complex./imag (1+2i)", "~%Complex./imag (1+2i | 9+9i)", "~%Complex./imag (9+9i)", "1+2i", "9+9i"),
    ("complex.phase", "arg0", "~%Complex./phase (1+2i)", "~%Complex./phase (1+2i | 9+9i)", "~%Complex./phase (9+9i)", "1+2i", "9+9i"),
    ("complex.phase", "arg0", "~%Complex./phase (1)", "~%Complex./phase (1 | 9)", "~%Complex./phase (9)", "1", "9"),
    ("complex.real", "arg0", "~%Complex./real (1.5)", "~%Complex./real (1.5 | 9.5)", "~%Complex./real (9.5)", "1.5", "9.5"),
    ("complex.real", "arg0", "~%Complex./real (1)", "~%Complex./real (1 | 9)", "~%Complex./real (9)", "1", "9"),
    ("complex.real", "arg0", "~%Complex./real (2)", "~%Complex./real (2 | 10)", "~%Complex./real (10)", "2", "10"),
    ("csv.parse", "arg0", "~%Csv./parse (\"ab\")", "~%Csv./parse (\"ab\" | \"zz\")", "~%Csv./parse (\"zz\")", "\"ab\"", "\"zz\""),
    ("csv.parse", "arg0", "~%Csv./parse (\"a,b\")", "~%Csv./parse (\"a,b\" | \"zz\")", "~%Csv./parse (\"zz\")", "\"a,b\"", "\"zz\""),
    ("csv.parse", "arg0", "~%Csv./parse (\"b\")", "~%Csv./parse (\"b\" | \"zz\")", "~%Csv./parse (\"zz\")", "\"b\"", "\"zz\""),
    ("csv.parse_with_headers", "arg0", "~%Csv./parse_with_headers (\"ab\")", "~%Csv./parse_with_headers (\"ab\" | \"zz\")", "~%Csv./parse_with_headers (\"zz\")", "\"ab\"", "\"zz\""),
    ("json.get", "arg0", "~%Json./get (\"ab\", 1)", "~%Json./get (\"ab\" | \"zz\", 1)", "~%Json./get (\"zz\", 1)", "\"ab\"", "\"zz\""),
    ("json.get", "arg1", "~%Json./get (\"ab\", 1)", "~%Json./get (\"ab\", 1 | 9)", "~%Json./get (\"ab\", 9)", "1", "9"),
    ("json.keys", "el0", "~%Json./keys ([1, 2, 3])", "~%Json./keys ([1 | 9, 2, 3])", "~%Json./keys ([9, 2, 3])", "1", "9"),
    ("json.keys", "el0", "~%Json./keys ([\"a\", \"b\"])", "~%Json./keys ([\"a\" | \"zz\", \"b\"])", "~%Json./keys ([\"zz\", \"b\"])", "\"a\"", "\"zz\""),
    ("json.parse", "arg0", "~%Json./parse (\"ab\")", "~%Json./parse (\"ab\" | \"zz\")", "~%Json./parse (\"zz\")", "\"ab\"", "\"zz\""),
    ("json.stringify", "arg0", "~%Json./stringify (1)", "~%Json./stringify (1 | 9)", "~%Json./stringify (9)", "1", "9"),
    ("json.stringify", "arg0", "~%Json./stringify (2)", "~%Json./stringify (2 | 10)", "~%Json./stringify (10)", "2", "10"),
    ("json.stringify", "arg0", "~%Json./stringify (3)", "~%Json./stringify (3 | 11)", "~%Json./stringify (11)", "3", "11"),
    ("list.at", "el1", "~%List./at (1, [\"a\", \"b\"])", "~%List./at (1, [\"a\" | \"zz\", \"b\"])", "~%List./at (1, [\"zz\", \"b\"])", "\"a\"", "\"zz\""),
    ("list.at", "el1", "~%List./at (1, [1, 2, 3])", "~%List./at (1, [1 | 9, 2, 3])", "~%List./at (1, [9, 2, 3])", "1", "9"),
    ("list.at", "el1", "~%List./at (2, [1, 2, 3])", "~%List./at (2, [1 | 9, 2, 3])", "~%List./at (2, [9, 2, 3])", "1", "9"),
    ("list.chunk", "arg0", "~%List./chunk (1, [1, 2, 3])", "~%List./chunk (1 | 9, [1, 2, 3])", "~%List./chunk (9, [1, 2, 3])", "1", "9"),
    ("list.chunk", "el1", "~%List./chunk (1, [1, 2, 3])", "~%List./chunk (1, [1 | 9, 2, 3])", "~%List./chunk (1, [9, 2, 3])", "1", "9"),
    ("list.chunk", "arg0", "~%List./chunk (1, [\"a\", \"b\"])", "~%List./chunk (1 | 9, [\"a\", \"b\"])", "~%List./chunk (9, [\"a\", \"b\"])", "1", "9"),
    ("list.chunk", "el1", "~%List./chunk (1, [\"a\", \"b\"])", "~%List./chunk (1, [\"a\" | \"zz\", \"b\"])", "~%List./chunk (1, [\"zz\", \"b\"])", "\"a\"", "\"zz\""),
    ("list.chunk", "arg0", "~%List./chunk (2, [1, 2, 3])", "~%List./chunk (2 | 10, [1, 2, 3])", "~%List./chunk (10, [1, 2, 3])", "2", "10"),
    ("list.chunk", "el1", "~%List./chunk (2, [1, 2, 3])", "~%List./chunk (2, [1 | 9, 2, 3])", "~%List./chunk (2, [9, 2, 3])", "1", "9"),
    ("list.concat", "el0", "~%List./concat ([1, 2, 3], [1, 2, 3])", "~%List./concat ([1 | 9, 2, 3], [1, 2, 3])", "~%List./concat ([9, 2, 3], [1, 2, 3])", "1", "9"),
    ("list.concat", "el1", "~%List./concat ([1, 2, 3], [1, 2, 3])", "~%List./concat ([1, 2, 3], [1 | 9, 2, 3])", "~%List./concat ([1, 2, 3], [9, 2, 3])", "1", "9"),
    ("list.concat", "el0", "~%List./concat ([1, 2, 3], [\"a\", \"b\"])", "~%List./concat ([1 | 9, 2, 3], [\"a\", \"b\"])", "~%List./concat ([9, 2, 3], [\"a\", \"b\"])", "1", "9"),
    ("list.concat", "el1", "~%List./concat ([1, 2, 3], [\"a\", \"b\"])", "~%List./concat ([1, 2, 3], [\"a\" | \"zz\", \"b\"])", "~%List./concat ([1, 2, 3], [\"zz\", \"b\"])", "\"a\"", "\"zz\""),
    ("list.concat", "el0", "~%List./concat ([\"a\", \"b\"], [1, 2, 3])", "~%List./concat ([\"a\" | \"zz\", \"b\"], [1, 2, 3])", "~%List./concat ([\"zz\", \"b\"], [1, 2, 3])", "\"a\"", "\"zz\""),
    ("list.concat", "el1", "~%List./concat ([\"a\", \"b\"], [1, 2, 3])", "~%List./concat ([\"a\", \"b\"], [1 | 9, 2, 3])", "~%List./concat ([\"a\", \"b\"], [9, 2, 3])", "1", "9"),
    ("list.drop", "arg0", "~%List./drop (1, [1, 2, 3])", "~%List./drop (1 | 9, [1, 2, 3])", "~%List./drop (9, [1, 2, 3])", "1", "9"),
    ("list.drop", "el1", "~%List./drop (1, [1, 2, 3])", "~%List./drop (1, [1 | 9, 2, 3])", "~%List./drop (1, [9, 2, 3])", "1", "9"),
    ("list.drop", "arg0", "~%List./drop (1, [\"a\", \"b\"])", "~%List./drop (1 | 9, [\"a\", \"b\"])", "~%List./drop (9, [\"a\", \"b\"])", "1", "9"),
    ("list.drop", "el1", "~%List./drop (1, [\"a\", \"b\"])", "~%List./drop (1, [\"a\" | \"zz\", \"b\"])", "~%List./drop (1, [\"zz\", \"b\"])", "\"a\"", "\"zz\""),
    ("list.drop", "arg0", "~%List./drop (2, [1, 2, 3])", "~%List./drop (2 | 10, [1, 2, 3])", "~%List./drop (10, [1, 2, 3])", "2", "10"),
    ("list.drop", "el1", "~%List./drop (2, [1, 2, 3])", "~%List./drop (2, [1 | 9, 2, 3])", "~%List./drop (2, [9, 2, 3])", "1", "9"),
    ("list.enumerate", "el0", "~%List./enumerate ([1, 2, 3])", "~%List./enumerate ([1 | 9, 2, 3])", "~%List./enumerate ([9, 2, 3])", "1", "9"),
    ("list.enumerate", "arg0", "~%List./enumerate (1)", "~%List./enumerate (1 | 9)", "~%List./enumerate (9)", "1", "9"),
    ("list.flatten", "el0", "~%List./flatten ([1, 2, 3])", "~%List./flatten ([1 | 9, 2, 3])", "~%List./flatten ([9, 2, 3])", "1", "9"),
    ("list.flatten", "el0", "~%List./flatten ([\"a\", \"b\"])", "~%List./flatten ([\"a\" | \"zz\", \"b\"])", "~%List./flatten ([\"zz\", \"b\"])", "\"a\"", "\"zz\""),
    ("list.head", "el0", "~%List./head ([\"a\", \"b\"])", "~%List./head ([\"a\" | \"zz\", \"b\"])", "~%List./head ([\"zz\", \"b\"])", "\"a\"", "\"zz\""),
    ("list.head", "arg0", "~%List./head (1)", "~%List./head (1 | 9)", "~%List./head (9)", "1", "9"),
    ("list.head", "el0", "~%List./head ([1, 2, 3])", "~%List./head ([1 | 9, 2, 3])", "~%List./head ([9, 2, 3])", "1", "9"),
    ("list.intersperse", "arg0", "~%List./intersperse (1, [1, 2, 3])", "~%List./intersperse (1 | 9, [1, 2, 3])", "~%List./intersperse (9, [1, 2, 3])", "1", "9"),
    ("list.intersperse", "el1", "~%List./intersperse (1, [1, 2, 3])", "~%List./intersperse (1, [1 | 9, 2, 3])", "~%List./intersperse (1, [9, 2, 3])", "1", "9"),
    ("list.intersperse", "arg0", "~%List./intersperse (1, [\"a\", \"b\"])", "~%List./intersperse (1 | 9, [\"a\", \"b\"])", "~%List./intersperse (9, [\"a\", \"b\"])", "1", "9"),
    ("list.intersperse", "el1", "~%List./intersperse (1, [\"a\", \"b\"])", "~%List./intersperse (1, [\"a\" | \"zz\", \"b\"])", "~%List./intersperse (1, [\"zz\", \"b\"])", "\"a\"", "\"zz\""),
    ("list.intersperse", "arg0", "~%List./intersperse (2, [1, 2, 3])", "~%List./intersperse (2 | 10, [1, 2, 3])", "~%List./intersperse (10, [1, 2, 3])", "2", "10"),
    ("list.intersperse", "el1", "~%List./intersperse (2, [1, 2, 3])", "~%List./intersperse (2, [1 | 9, 2, 3])", "~%List./intersperse (2, [9, 2, 3])", "1", "9"),
    ("list.len", "el0", "~%List./len ([1, 2, 3])", "~%List./len ([1 | 9, 2, 3])", "~%List./len ([9, 2, 3])", "1", "9"),
    ("list.len", "el0", "~%List./len ([\"a\", \"b\"])", "~%List./len ([\"a\" | \"zz\", \"b\"])", "~%List./len ([\"zz\", \"b\"])", "\"a\"", "\"zz\""),
    ("list.product", "el0", "~%List./product ([1, 2, 3])", "~%List./product ([1 | 9, 2, 3])", "~%List./product ([9, 2, 3])", "1", "9"),
    ("list.product", "arg0", "~%List./product (1)", "~%List./product (1 | 9)", "~%List./product (9)", "1", "9"),
    ("list.range", "arg0", "~%List./range (1, 2)", "~%List./range (1 | 9, 2)", "~%List./range (9, 2)", "1", "9"),
    ("list.range", "arg1", "~%List./range (1, 2)", "~%List./range (1, 2 | 10)", "~%List./range (1, 10)", "2", "10"),
    ("list.range", "arg0", "~%List./range (1, 3)", "~%List./range (1 | 9, 3)", "~%List./range (9, 3)", "1", "9"),
    ("list.range", "arg1", "~%List./range (1, 3)", "~%List./range (1, 3 | 11)", "~%List./range (1, 11)", "3", "11"),
    ("list.range", "arg0", "~%List./range (2, 3)", "~%List./range (2 | 10, 3)", "~%List./range (10, 3)", "2", "10"),
    ("list.range", "arg1", "~%List./range (2, 3)", "~%List./range (2, 3 | 11)", "~%List./range (2, 11)", "3", "11"),
    ("list.reverse", "el0", "~%List./reverse ([1, 2, 3])", "~%List./reverse ([1 | 9, 2, 3])", "~%List./reverse ([9, 2, 3])", "1", "9"),
    ("list.reverse", "el0", "~%List./reverse ([\"a\", \"b\"])", "~%List./reverse ([\"a\" | \"zz\", \"b\"])", "~%List./reverse ([\"zz\", \"b\"])", "\"a\"", "\"zz\""),
    ("list.slice", "arg0", "~%List./slice (1, 2, [1, 2, 3])", "~%List./slice (1 | 9, 2, [1, 2, 3])", "~%List./slice (9, 2, [1, 2, 3])", "1", "9"),
    ("list.slice", "arg1", "~%List./slice (1, 2, [1, 2, 3])", "~%List./slice (1, 2 | 10, [1, 2, 3])", "~%List./slice (1, 10, [1, 2, 3])", "2", "10"),
    ("list.slice", "el2", "~%List./slice (1, 2, [1, 2, 3])", "~%List./slice (1, 2, [1 | 9, 2, 3])", "~%List./slice (1, 2, [9, 2, 3])", "1", "9"),
    ("list.slice", "arg0", "~%List./slice (1, 2, [\"a\", \"b\"])", "~%List./slice (1 | 9, 2, [\"a\", \"b\"])", "~%List./slice (9, 2, [\"a\", \"b\"])", "1", "9"),
    ("list.slice", "arg1", "~%List./slice (1, 2, [\"a\", \"b\"])", "~%List./slice (1, 2 | 10, [\"a\", \"b\"])", "~%List./slice (1, 10, [\"a\", \"b\"])", "2", "10"),
    ("list.slice", "el2", "~%List./slice (1, 2, [\"a\", \"b\"])", "~%List./slice (1, 2, [\"a\" | \"zz\", \"b\"])", "~%List./slice (1, 2, [\"zz\", \"b\"])", "\"a\"", "\"zz\""),
    ("list.slice", "arg0", "~%List./slice (1, 3, [1, 2, 3])", "~%List./slice (1 | 9, 3, [1, 2, 3])", "~%List./slice (9, 3, [1, 2, 3])", "1", "9"),
    ("list.slice", "arg1", "~%List./slice (1, 3, [1, 2, 3])", "~%List./slice (1, 3 | 11, [1, 2, 3])", "~%List./slice (1, 11, [1, 2, 3])", "3", "11"),
    ("list.slice", "el2", "~%List./slice (1, 3, [1, 2, 3])", "~%List./slice (1, 3, [1 | 9, 2, 3])", "~%List./slice (1, 3, [9, 2, 3])", "1", "9"),
    ("list.sort", "el0", "~%List./sort ([1, 2, 3])", "~%List./sort ([1 | 9, 2, 3])", "~%List./sort ([9, 2, 3])", "1", "9"),
    ("list.sort", "el0", "~%List./sort ([\"a\", \"b\"])", "~%List./sort ([\"a\" | \"zz\", \"b\"])", "~%List./sort ([\"zz\", \"b\"])", "\"a\"", "\"zz\""),
    ("list.sum", "el0", "~%List./sum ([1, 2, 3])", "~%List./sum ([1 | 9, 2, 3])", "~%List./sum ([9, 2, 3])", "1", "9"),
    ("list.sum", "arg0", "~%List./sum (1)", "~%List./sum (1 | 9)", "~%List./sum (9)", "1", "9"),
    ("list.tail", "el0", "~%List./tail ([1, 2, 3])", "~%List./tail ([1 | 9, 2, 3])", "~%List./tail ([9, 2, 3])", "1", "9"),
    ("list.tail", "el0", "~%List./tail ([\"a\", \"b\"])", "~%List./tail ([\"a\" | \"zz\", \"b\"])", "~%List./tail ([\"zz\", \"b\"])", "\"a\"", "\"zz\""),
    ("list.take", "arg0", "~%List./take (1, [1, 2, 3])", "~%List./take (1 | 9, [1, 2, 3])", "~%List./take (9, [1, 2, 3])", "1", "9"),
    ("list.take", "el1", "~%List./take (1, [1, 2, 3])", "~%List./take (1, [1 | 9, 2, 3])", "~%List./take (1, [9, 2, 3])", "1", "9"),
    ("list.take", "arg0", "~%List./take (1, [\"a\", \"b\"])", "~%List./take (1 | 9, [\"a\", \"b\"])", "~%List./take (9, [\"a\", \"b\"])", "1", "9"),
    ("list.take", "el1", "~%List./take (1, [\"a\", \"b\"])", "~%List./take (1, [\"a\" | \"zz\", \"b\"])", "~%List./take (1, [\"zz\", \"b\"])", "\"a\"", "\"zz\""),
    ("list.take", "arg0", "~%List./take (2, [1, 2, 3])", "~%List./take (2 | 10, [1, 2, 3])", "~%List./take (10, [1, 2, 3])", "2", "10"),
    ("list.take", "el1", "~%List./take (2, [1, 2, 3])", "~%List./take (2, [1 | 9, 2, 3])", "~%List./take (2, [9, 2, 3])", "1", "9"),
    ("list.transpose", "arg0", "~%List./transpose (1)", "~%List./transpose (1 | 9)", "~%List./transpose (9)", "1", "9"),
    ("list.window", "arg0", "~%List./window (1, [1, 2, 3])", "~%List./window (1 | 9, [1, 2, 3])", "~%List./window (9, [1, 2, 3])", "1", "9"),
    ("list.window", "el1", "~%List./window (1, [1, 2, 3])", "~%List./window (1, [1 | 9, 2, 3])", "~%List./window (1, [9, 2, 3])", "1", "9"),
    ("list.window", "arg0", "~%List./window (1, [\"a\", \"b\"])", "~%List./window (1 | 9, [\"a\", \"b\"])", "~%List./window (9, [\"a\", \"b\"])", "1", "9"),
    ("list.window", "el1", "~%List./window (1, [\"a\", \"b\"])", "~%List./window (1, [\"a\" | \"zz\", \"b\"])", "~%List./window (1, [\"zz\", \"b\"])", "\"a\"", "\"zz\""),
    ("list.window", "arg0", "~%List./window (2, [1, 2, 3])", "~%List./window (2 | 10, [1, 2, 3])", "~%List./window (10, [1, 2, 3])", "2", "10"),
    ("list.window", "el1", "~%List./window (2, [1, 2, 3])", "~%List./window (2, [1 | 9, 2, 3])", "~%List./window (2, [9, 2, 3])", "1", "9"),
    ("list.zip", "el0", "~%List./zip ([1, 2, 3], (x -> x + 1))", "~%List./zip ([1 | 9, 2, 3], (x -> x + 1))", "~%List./zip ([9, 2, 3], (x -> x + 1))", "1", "9"),
    ("math.abs", "arg0", "~%Math./abs (1.5)", "~%Math./abs (1.5 | 9.5)", "~%Math./abs (9.5)", "1.5", "9.5"),
    ("math.abs", "arg0", "~%Math./abs (1+2i)", "~%Math./abs (1+2i | 9+9i)", "~%Math./abs (9+9i)", "1+2i", "9+9i"),
    ("math.add", "arg0", "~%Math./add (1, 2)", "~%Math./add (1 | 9, 2)", "~%Math./add (9, 2)", "1", "9"),
    ("math.add", "arg1", "~%Math./add (1, 2)", "~%Math./add (1, 2 | 10)", "~%Math./add (1, 10)", "2", "10"),
    ("math.atan2", "arg0", "~%Math./atan2 (1, 1)", "~%Math./atan2 (1 | 9, 1)", "~%Math./atan2 (9, 1)", "1", "9"),
    ("math.atan2", "arg1", "~%Math./atan2 (1, 1)", "~%Math./atan2 (1, 1 | 9)", "~%Math./atan2 (1, 9)", "1", "9"),
    ("math.atan2", "arg0", "~%Math./atan2 (1, 2)", "~%Math./atan2 (1 | 9, 2)", "~%Math./atan2 (9, 2)", "1", "9"),
    ("math.atan2", "arg1", "~%Math./atan2 (1, 2)", "~%Math./atan2 (1, 2 | 10)", "~%Math./atan2 (1, 10)", "2", "10"),
    ("math.atan2", "arg0", "~%Math./atan2 (1, 3)", "~%Math./atan2 (1 | 9, 3)", "~%Math./atan2 (9, 3)", "1", "9"),
    ("math.atan2", "arg1", "~%Math./atan2 (1, 3)", "~%Math./atan2 (1, 3 | 11)", "~%Math./atan2 (1, 11)", "3", "11"),
    ("math.bits", "arg0", "~%Math./bits (\"ab\")", "~%Math./bits (\"ab\" | \"zz\")", "~%Math./bits (\"zz\")", "\"ab\"", "\"zz\""),
    ("math.bits", "arg0", "~%Math./bits (\"a,b\")", "~%Math./bits (\"a,b\" | \"zz\")", "~%Math./bits (\"zz\")", "\"a,b\"", "\"zz\""),
    ("math.bits", "arg0", "~%Math./bits (\"b\")", "~%Math./bits (\"b\" | \"zz\")", "~%Math./bits (\"zz\")", "\"b\"", "\"zz\""),
    ("math.ceil", "arg0", "~%Math./ceil (1)", "~%Math./ceil (1 | 9)", "~%Math./ceil (9)", "1", "9"),
    ("math.ceil", "arg0", "~%Math./ceil (2)", "~%Math./ceil (2 | 10)", "~%Math./ceil (10)", "2", "10"),
    ("math.ceil", "arg0", "~%Math./ceil (3)", "~%Math./ceil (3 | 11)", "~%Math./ceil (11)", "3", "11"),
    ("math.choose", "arg0", "~%Math./choose (1, 1)", "~%Math./choose (1 | 9, 1)", "~%Math./choose (9, 1)", "1", "9"),
    ("math.choose", "arg1", "~%Math./choose (1, 1)", "~%Math./choose (1, 1 | 9)", "~%Math./choose (1, 9)", "1", "9"),
    ("math.choose", "arg0", "~%Math./choose (1, 2)", "~%Math./choose (1 | 9, 2)", "~%Math./choose (9, 2)", "1", "9"),
    ("math.choose", "arg1", "~%Math./choose (1, 2)", "~%Math./choose (1, 2 | 10)", "~%Math./choose (1, 10)", "2", "10"),
    ("math.choose", "arg0", "~%Math./choose (2, 1)", "~%Math./choose (2 | 10, 1)", "~%Math./choose (10, 1)", "2", "10"),
    ("math.choose", "arg1", "~%Math./choose (2, 1)", "~%Math./choose (2, 1 | 9)", "~%Math./choose (2, 9)", "1", "9"),
    ("math.clamp", "arg1", "~%Math./clamp (1, 2, 1.5)", "~%Math./clamp (1, 2 | 10, 1.5)", "~%Math./clamp (1, 10, 1.5)", "2", "10"),
    ("math.clamp", "arg2", "~%Math./clamp (1, 2, 1.5)", "~%Math./clamp (1, 2, 1.5 | 9.5)", "~%Math./clamp (1, 2, 9.5)", "1.5", "9.5"),
    ("math.clamp", "arg1", "~%Math./clamp (1, 1, 1)", "~%Math./clamp (1, 1 | 9, 1)", "~%Math./clamp (1, 9, 1)", "1", "9"),
    ("math.clamp", "arg2", "~%Math./clamp (1, 1, 1)", "~%Math./clamp (1, 1, 1 | 9)", "~%Math./clamp (1, 1, 9)", "1", "9"),
    ("math.clamp", "arg1", "~%Math./clamp (1, 2, 2)", "~%Math./clamp (1, 2 | 10, 2)", "~%Math./clamp (1, 10, 2)", "2", "10"),
    ("math.clamp", "arg2", "~%Math./clamp (1, 2, 2)", "~%Math./clamp (1, 2, 2 | 10)", "~%Math./clamp (1, 2, 10)", "2", "10"),
    ("math.cos", "arg0", "~%Math./cos (1)", "~%Math./cos (1 | 9)", "~%Math./cos (9)", "1", "9"),
    ("math.cos", "arg0", "~%Math./cos (2)", "~%Math./cos (2 | 10)", "~%Math./cos (10)", "2", "10"),
    ("math.cos", "arg0", "~%Math./cos (3)", "~%Math./cos (3 | 11)", "~%Math./cos (11)", "3", "11"),
    ("math.cosh", "arg0", "~%Math./cosh (1)", "~%Math./cosh (1 | 9)", "~%Math./cosh (9)", "1", "9"),
    ("math.cosh", "arg0", "~%Math./cosh (2)", "~%Math./cosh (2 | 10)", "~%Math./cosh (10)", "2", "10"),
    ("math.cosh", "arg0", "~%Math./cosh (3)", "~%Math./cosh (3 | 11)", "~%Math./cosh (11)", "3", "11"),
    ("math.div", "arg0", "~%Math./div (1, 1.5)", "~%Math./div (1 | 9, 1.5)", "~%Math./div (9, 1.5)", "1", "9"),
    ("math.div", "arg1", "~%Math./div (1, 1.5)", "~%Math./div (1, 1.5 | 9.5)", "~%Math./div (1, 9.5)", "1.5", "9.5"),
    ("math.div", "arg0", "~%Math./div (1, 1+2i)", "~%Math./div (1 | 9, 1+2i)", "~%Math./div (9, 1+2i)", "1", "9"),
    ("math.div", "arg1", "~%Math./div (1, 1+2i)", "~%Math./div (1, 1+2i | 9+9i)", "~%Math./div (1, 9+9i)", "1+2i", "9+9i"),
    ("math.div", "arg0", "~%Math./div (2, 1.5)", "~%Math./div (2 | 10, 1.5)", "~%Math./div (10, 1.5)", "2", "10"),
    ("math.div", "arg1", "~%Math./div (2, 1.5)", "~%Math./div (2, 1.5 | 9.5)", "~%Math./div (2, 9.5)", "1.5", "9.5"),
    ("math.eml", "arg0", "~%Math./eml (1, 1)", "~%Math./eml (1 | 9, 1)", "~%Math./eml (9, 1)", "1", "9"),
    ("math.eml", "arg1", "~%Math./eml (1, 1)", "~%Math./eml (1, 1 | 9)", "~%Math./eml (1, 9)", "1", "9"),
    ("math.eml", "arg0", "~%Math./eml (1, 2)", "~%Math./eml (1 | 9, 2)", "~%Math./eml (9, 2)", "1", "9"),
    ("math.eml", "arg1", "~%Math./eml (1, 2)", "~%Math./eml (1, 2 | 10)", "~%Math./eml (1, 10)", "2", "10"),
    ("math.eml", "arg0", "~%Math./eml (1, 3)", "~%Math./eml (1 | 9, 3)", "~%Math./eml (9, 3)", "1", "9"),
    ("math.eml", "arg1", "~%Math./eml (1, 3)", "~%Math./eml (1, 3 | 11)", "~%Math./eml (1, 11)", "3", "11"),
    ("math.exp", "arg0", "~%Math./exp (1)", "~%Math./exp (1 | 9)", "~%Math./exp (9)", "1", "9"),
    ("math.exp", "arg0", "~%Math./exp (2)", "~%Math./exp (2 | 10)", "~%Math./exp (10)", "2", "10"),
    ("math.exp", "arg0", "~%Math./exp (3)", "~%Math./exp (3 | 11)", "~%Math./exp (11)", "3", "11"),
    ("math.factorial", "arg0", "~%Math./factorial (3)", "~%Math./factorial (3 | 11)", "~%Math./factorial (11)", "3", "11"),
    ("math.factorial", "arg0", "~%Math./factorial (1)", "~%Math./factorial (1 | 9)", "~%Math./factorial (9)", "1", "9"),
    ("math.factorial", "arg0", "~%Math./factorial (2)", "~%Math./factorial (2 | 10)", "~%Math./factorial (10)", "2", "10"),
    ("math.floor", "arg0", "~%Math./floor (1)", "~%Math./floor (1 | 9)", "~%Math./floor (9)", "1", "9"),
    ("math.floor", "arg0", "~%Math./floor (2)", "~%Math./floor (2 | 10)", "~%Math./floor (10)", "2", "10"),
    ("math.floor", "arg0", "~%Math./floor (3)", "~%Math./floor (3 | 11)", "~%Math./floor (11)", "3", "11"),
    ("math.fract", "arg0", "~%Math./fract (1.5)", "~%Math./fract (1.5 | 9.5)", "~%Math./fract (9.5)", "1.5", "9.5"),
    ("math.fract", "arg0", "~%Math./fract (1)", "~%Math./fract (1 | 9)", "~%Math./fract (9)", "1", "9"),
    ("math.gcd", "arg0", "~%Math./gcd (1, 1)", "~%Math./gcd (1 | 9, 1)", "~%Math./gcd (9, 1)", "1", "9"),
    ("math.gcd", "arg1", "~%Math./gcd (1, 1)", "~%Math./gcd (1, 1 | 9)", "~%Math./gcd (1, 9)", "1", "9"),
    ("math.gcd", "arg0", "~%Math./gcd (2, 2)", "~%Math./gcd (2 | 10, 2)", "~%Math./gcd (10, 2)", "2", "10"),
    ("math.gcd", "arg1", "~%Math./gcd (2, 2)", "~%Math./gcd (2, 2 | 10)", "~%Math./gcd (2, 10)", "2", "10"),
    ("math.gcd", "arg0", "~%Math./gcd (3, 3)", "~%Math./gcd (3 | 11, 3)", "~%Math./gcd (11, 3)", "3", "11"),
    ("math.gcd", "arg1", "~%Math./gcd (3, 3)", "~%Math./gcd (3, 3 | 11)", "~%Math./gcd (3, 11)", "3", "11"),
    ("math.gt", "arg0", "~%Math./gt (1, 1)", "~%Math./gt (1 | 9, 1)", "~%Math./gt (9, 1)", "1", "9"),
    ("math.gt", "arg1", "~%Math./gt (1, 1)", "~%Math./gt (1, 1 | 9)", "~%Math./gt (1, 9)", "1", "9"),
    ("math.gt", "arg0", "~%Math./gt (2, 1)", "~%Math./gt (2 | 10, 1)", "~%Math./gt (10, 1)", "2", "10"),
    ("math.gt", "arg1", "~%Math./gt (2, 1)", "~%Math./gt (2, 1 | 9)", "~%Math./gt (2, 9)", "1", "9"),
    ("math.gte", "arg0", "~%Math./gte (1, 1)", "~%Math./gte (1 | 9, 1)", "~%Math./gte (9, 1)", "1", "9"),
    ("math.gte", "arg1", "~%Math./gte (1, 1)", "~%Math./gte (1, 1 | 9)", "~%Math./gte (1, 9)", "1", "9"),
    ("math.gte", "arg0", "~%Math./gte (1, 2)", "~%Math./gte (1 | 9, 2)", "~%Math./gte (9, 2)", "1", "9"),
    ("math.gte", "arg1", "~%Math./gte (1, 2)", "~%Math./gte (1, 2 | 10)", "~%Math./gte (1, 10)", "2", "10"),
    ("math.hypot", "arg0", "~%Math./hypot (1, 1)", "~%Math./hypot (1 | 9, 1)", "~%Math./hypot (9, 1)", "1", "9"),
    ("math.hypot", "arg1", "~%Math./hypot (1, 1)", "~%Math./hypot (1, 1 | 9)", "~%Math./hypot (1, 9)", "1", "9"),
    ("math.hypot", "arg0", "~%Math./hypot (1, 2)", "~%Math./hypot (1 | 9, 2)", "~%Math./hypot (9, 2)", "1", "9"),
    ("math.hypot", "arg1", "~%Math./hypot (1, 2)", "~%Math./hypot (1, 2 | 10)", "~%Math./hypot (1, 10)", "2", "10"),
    ("math.hypot", "arg0", "~%Math./hypot (1, 3)", "~%Math./hypot (1 | 9, 3)", "~%Math./hypot (9, 3)", "1", "9"),
    ("math.hypot", "arg1", "~%Math./hypot (1, 3)", "~%Math./hypot (1, 3 | 11)", "~%Math./hypot (1, 11)", "3", "11"),
    ("math.is_prime", "arg0", "~%Math./is_prime (1)", "~%Math./is_prime (1 | 9)", "~%Math./is_prime (9)", "1", "9"),
    ("math.is_prime", "arg0", "~%Math./is_prime (2)", "~%Math./is_prime (2 | 10)", "~%Math./is_prime (10)", "2", "10"),
    ("math.lcm", "arg0", "~%Math./lcm (2, 3)", "~%Math./lcm (2 | 10, 3)", "~%Math./lcm (10, 3)", "2", "10"),
    ("math.lcm", "arg1", "~%Math./lcm (2, 3)", "~%Math./lcm (2, 3 | 11)", "~%Math./lcm (2, 11)", "3", "11"),
    ("math.lcm", "arg0", "~%Math./lcm (1, 1)", "~%Math./lcm (1 | 9, 1)", "~%Math./lcm (9, 1)", "1", "9"),
    ("math.lcm", "arg1", "~%Math./lcm (1, 1)", "~%Math./lcm (1, 1 | 9)", "~%Math./lcm (1, 9)", "1", "9"),
    ("math.lcm", "arg0", "~%Math./lcm (1, 2)", "~%Math./lcm (1 | 9, 2)", "~%Math./lcm (9, 2)", "1", "9"),
    ("math.lcm", "arg1", "~%Math./lcm (1, 2)", "~%Math./lcm (1, 2 | 10)", "~%Math./lcm (1, 10)", "2", "10"),
    ("math.ln", "arg0", "~%Math./ln (1)", "~%Math./ln (1 | 9)", "~%Math./ln (9)", "1", "9"),
    ("math.ln", "arg0", "~%Math./ln (2)", "~%Math./ln (2 | 10)", "~%Math./ln (10)", "2", "10"),
    ("math.ln", "arg0", "~%Math./ln (3)", "~%Math./ln (3 | 11)", "~%Math./ln (11)", "3", "11"),
    ("math.log10", "arg0", "~%Math./log10 (2)", "~%Math./log10 (2 | 10)", "~%Math./log10 (10)", "2", "10"),
    ("math.log10", "arg0", "~%Math./log10 (3)", "~%Math./log10 (3 | 11)", "~%Math./log10 (11)", "3", "11"),
    ("math.log10", "arg0", "~%Math./log10 (1.5)", "~%Math./log10 (1.5 | 9.5)", "~%Math./log10 (9.5)", "1.5", "9.5"),
    ("math.log2", "arg0", "~%Math./log2 (3)", "~%Math./log2 (3 | 11)", "~%Math./log2 (11)", "3", "11"),
    ("math.log2", "arg0", "~%Math./log2 (1.5)", "~%Math./log2 (1.5 | 9.5)", "~%Math./log2 (9.5)", "1.5", "9.5"),
    ("math.lt", "arg0", "~%Math./lt (1, 1)", "~%Math./lt (1 | 9, 1)", "~%Math./lt (9, 1)", "1", "9"),
    ("math.lt", "arg1", "~%Math./lt (1, 1)", "~%Math./lt (1, 1 | 9)", "~%Math./lt (1, 9)", "1", "9"),
    ("math.lt", "arg0", "~%Math./lt (1, 2)", "~%Math./lt (1 | 9, 2)", "~%Math./lt (9, 2)", "1", "9"),
    ("math.lt", "arg1", "~%Math./lt (1, 2)", "~%Math./lt (1, 2 | 10)", "~%Math./lt (1, 10)", "2", "10"),
    ("math.lte", "arg0", "~%Math./lte (1, 1)", "~%Math./lte (1 | 9, 1)", "~%Math./lte (9, 1)", "1", "9"),
    ("math.lte", "arg1", "~%Math./lte (1, 1)", "~%Math./lte (1, 1 | 9)", "~%Math./lte (1, 9)", "1", "9"),
    ("math.lte", "arg0", "~%Math./lte (2, 1)", "~%Math./lte (2 | 10, 1)", "~%Math./lte (10, 1)", "2", "10"),
    ("math.lte", "arg1", "~%Math./lte (2, 1)", "~%Math./lte (2, 1 | 9)", "~%Math./lte (2, 9)", "1", "9"),
    ("math.max", "arg0", "~%Math./max (1, 1.5)", "~%Math./max (1 | 9, 1.5)", "~%Math./max (9, 1.5)", "1", "9"),
    ("math.max", "arg1", "~%Math./max (1, 1.5)", "~%Math./max (1, 1.5 | 9.5)", "~%Math./max (1, 9.5)", "1.5", "9.5"),
    ("math.max", "arg0", "~%Math./max (1, 1)", "~%Math./max (1 | 9, 1)", "~%Math./max (9, 1)", "1", "9"),
    ("math.max", "arg1", "~%Math./max (1, 1)", "~%Math./max (1, 1 | 9)", "~%Math./max (1, 9)", "1", "9"),
    ("math.max", "arg0", "~%Math./max (1, 2)", "~%Math./max (1 | 9, 2)", "~%Math./max (9, 2)", "1", "9"),
    ("math.max", "arg1", "~%Math./max (1, 2)", "~%Math./max (1, 2 | 10)", "~%Math./max (1, 10)", "2", "10"),
    ("math.min", "arg0", "~%Math./min (2, 1.5)", "~%Math./min (2 | 10, 1.5)", "~%Math./min (10, 1.5)", "2", "10"),
    ("math.min", "arg1", "~%Math./min (2, 1.5)", "~%Math./min (2, 1.5 | 9.5)", "~%Math./min (2, 9.5)", "1.5", "9.5"),
    ("math.min", "arg0", "~%Math./min (1, 1)", "~%Math./min (1 | 9, 1)", "~%Math./min (9, 1)", "1", "9"),
    ("math.min", "arg1", "~%Math./min (1, 1)", "~%Math./min (1, 1 | 9)", "~%Math./min (1, 9)", "1", "9"),
    ("math.min", "arg0", "~%Math./min (2, 2)", "~%Math./min (2 | 10, 2)", "~%Math./min (10, 2)", "2", "10"),
    ("math.min", "arg1", "~%Math./min (2, 2)", "~%Math./min (2, 2 | 10)", "~%Math./min (2, 10)", "2", "10"),
    ("math.mul", "arg0", "~%Math./mul (1, 1.5)", "~%Math./mul (1 | 9, 1.5)", "~%Math./mul (9, 1.5)", "1", "9"),
    ("math.mul", "arg1", "~%Math./mul (1, 1.5)", "~%Math./mul (1, 1.5 | 9.5)", "~%Math./mul (1, 9.5)", "1.5", "9.5"),
    ("math.mul", "arg0", "~%Math./mul (1, 1+2i)", "~%Math./mul (1 | 9, 1+2i)", "~%Math./mul (9, 1+2i)", "1", "9"),
    ("math.mul", "arg1", "~%Math./mul (1, 1+2i)", "~%Math./mul (1, 1+2i | 9+9i)", "~%Math./mul (1, 9+9i)", "1+2i", "9+9i"),
    ("math.mul", "arg0", "~%Math./mul (2, 2)", "~%Math./mul (2 | 10, 2)", "~%Math./mul (10, 2)", "2", "10"),
    ("math.mul", "arg1", "~%Math./mul (2, 2)", "~%Math./mul (2, 2 | 10)", "~%Math./mul (2, 10)", "2", "10"),
    ("math.pow", "arg0", "~%Math./pow (2, 2)", "~%Math./pow (2 | 10, 2)", "~%Math./pow (10, 2)", "2", "10"),
    ("math.pow", "arg1", "~%Math./pow (2, 2)", "~%Math./pow (2, 2 | 10)", "~%Math./pow (2, 10)", "2", "10"),
    ("math.pow", "arg0", "~%Math./pow (2, 3)", "~%Math./pow (2 | 10, 3)", "~%Math./pow (10, 3)", "2", "10"),
    ("math.pow", "arg1", "~%Math./pow (2, 3)", "~%Math./pow (2, 3 | 11)", "~%Math./pow (2, 11)", "3", "11"),
    ("math.pow", "arg0", "~%Math./pow (2, 1.5)", "~%Math./pow (2 | 10, 1.5)", "~%Math./pow (10, 1.5)", "2", "10"),
    ("math.pow", "arg1", "~%Math./pow (2, 1.5)", "~%Math./pow (2, 1.5 | 9.5)", "~%Math./pow (2, 9.5)", "1.5", "9.5"),
    ("math.pow_mod", "arg0", "~%Math./pow_mod (1, 1, 1)", "~%Math./pow_mod (1 | 9, 1, 1)", "~%Math./pow_mod (9, 1, 1)", "1", "9"),
    ("math.pow_mod", "arg1", "~%Math./pow_mod (1, 1, 1)", "~%Math./pow_mod (1, 1 | 9, 1)", "~%Math./pow_mod (1, 9, 1)", "1", "9"),
    ("math.pow_mod", "arg2", "~%Math./pow_mod (1, 1, 1)", "~%Math./pow_mod (1, 1, 1 | 9)", "~%Math./pow_mod (1, 1, 9)", "1", "9"),
    ("math.pow_mod", "arg0", "~%Math./pow_mod (1, 1, 2)", "~%Math./pow_mod (1 | 9, 1, 2)", "~%Math./pow_mod (9, 1, 2)", "1", "9"),
    ("math.pow_mod", "arg1", "~%Math./pow_mod (1, 1, 2)", "~%Math./pow_mod (1, 1 | 9, 2)", "~%Math./pow_mod (1, 9, 2)", "1", "9"),
    ("math.pow_mod", "arg2", "~%Math./pow_mod (1, 1, 2)", "~%Math./pow_mod (1, 1, 2 | 10)", "~%Math./pow_mod (1, 1, 10)", "2", "10"),
    ("math.pow_mod", "arg0", "~%Math./pow_mod (2, 1, 3)", "~%Math./pow_mod (2 | 10, 1, 3)", "~%Math./pow_mod (10, 1, 3)", "2", "10"),
    ("math.pow_mod", "arg1", "~%Math./pow_mod (2, 1, 3)", "~%Math./pow_mod (2, 1 | 9, 3)", "~%Math./pow_mod (2, 9, 3)", "1", "9"),
    ("math.pow_mod", "arg2", "~%Math./pow_mod (2, 1, 3)", "~%Math./pow_mod (2, 1, 3 | 11)", "~%Math./pow_mod (2, 1, 11)", "3", "11"),
    ("math.rem", "arg0", "~%Math./rem (1, 1)", "~%Math./rem (1 | 9, 1)", "~%Math./rem (9, 1)", "1", "9"),
    ("math.rem", "arg1", "~%Math./rem (1, 1)", "~%Math./rem (1, 1 | 9)", "~%Math./rem (1, 9)", "1", "9"),
    ("math.rem", "arg0", "~%Math./rem (1, 2)", "~%Math./rem (1 | 9, 2)", "~%Math./rem (9, 2)", "1", "9"),
    ("math.rem", "arg1", "~%Math./rem (1, 2)", "~%Math./rem (1, 2 | 10)", "~%Math./rem (1, 10)", "2", "10"),
    ("math.rem", "arg0", "~%Math./rem (2, 3)", "~%Math./rem (2 | 10, 3)", "~%Math./rem (10, 3)", "2", "10"),
    ("math.rem", "arg1", "~%Math./rem (2, 3)", "~%Math./rem (2, 3 | 11)", "~%Math./rem (2, 11)", "3", "11"),
    ("math.round", "arg0", "~%Math./round (1)", "~%Math./round (1 | 9)", "~%Math./round (9)", "1", "9"),
    ("math.round", "arg0", "~%Math./round (2)", "~%Math./round (2 | 10)", "~%Math./round (10)", "2", "10"),
    ("math.round", "arg0", "~%Math./round (3)", "~%Math./round (3 | 11)", "~%Math./round (11)", "3", "11"),
    ("math.sign", "arg0", "~%Math./sign (1)", "~%Math./sign (1 | 9)", "~%Math./sign (9)", "1", "9"),
    ("math.sin", "arg0", "~%Math./sin (1)", "~%Math./sin (1 | 9)", "~%Math./sin (9)", "1", "9"),
    ("math.sin", "arg0", "~%Math./sin (2)", "~%Math./sin (2 | 10)", "~%Math./sin (10)", "2", "10"),
    ("math.sin", "arg0", "~%Math./sin (3)", "~%Math./sin (3 | 11)", "~%Math./sin (11)", "3", "11"),
    ("math.sinh", "arg0", "~%Math./sinh (1)", "~%Math./sinh (1 | 9)", "~%Math./sinh (9)", "1", "9"),
    ("math.sinh", "arg0", "~%Math./sinh (2)", "~%Math./sinh (2 | 10)", "~%Math./sinh (10)", "2", "10"),
    ("math.sinh", "arg0", "~%Math./sinh (3)", "~%Math./sinh (3 | 11)", "~%Math./sinh (11)", "3", "11"),
    ("math.sqrt", "arg0", "~%Math./sqrt (2)", "~%Math./sqrt (2 | 10)", "~%Math./sqrt (10)", "2", "10"),
    ("math.sqrt", "arg0", "~%Math./sqrt (3)", "~%Math./sqrt (3 | 11)", "~%Math./sqrt (11)", "3", "11"),
    ("math.sqrt", "arg0", "~%Math./sqrt (1.5)", "~%Math./sqrt (1.5 | 9.5)", "~%Math./sqrt (9.5)", "1.5", "9.5"),
    ("math.sub", "arg0", "~%Math./sub (1, 2)", "~%Math./sub (1 | 9, 2)", "~%Math./sub (9, 2)", "1", "9"),
    ("math.sub", "arg1", "~%Math./sub (1, 2)", "~%Math./sub (1, 2 | 10)", "~%Math./sub (1, 10)", "2", "10"),
    ("math.sub", "arg0", "~%Math./sub (1, 3)", "~%Math./sub (1 | 9, 3)", "~%Math./sub (9, 3)", "1", "9"),
    ("math.sub", "arg1", "~%Math./sub (1, 3)", "~%Math./sub (1, 3 | 11)", "~%Math./sub (1, 11)", "3", "11"),
    ("math.sub", "arg0", "~%Math./sub (1, 1.5)", "~%Math./sub (1 | 9, 1.5)", "~%Math./sub (9, 1.5)", "1", "9"),
    ("math.sub", "arg1", "~%Math./sub (1, 1.5)", "~%Math./sub (1, 1.5 | 9.5)", "~%Math./sub (1, 9.5)", "1.5", "9.5"),
    ("math.tanh", "arg0", "~%Math./tanh (1)", "~%Math./tanh (1 | 9)", "~%Math./tanh (9)", "1", "9"),
    ("math.tanh", "arg0", "~%Math./tanh (2)", "~%Math./tanh (2 | 10)", "~%Math./tanh (10)", "2", "10"),
    ("math.tanh", "arg0", "~%Math./tanh (3)", "~%Math./tanh (3 | 11)", "~%Math./tanh (11)", "3", "11"),
    ("math.to_float", "arg0", "~%Math./to_float (1.5)", "~%Math./to_float (1.5 | 9.5)", "~%Math./to_float (9.5)", "1.5", "9.5"),
    ("math.to_float", "arg0", "~%Math./to_float (1)", "~%Math./to_float (1 | 9)", "~%Math./to_float (9)", "1", "9"),
    ("math.to_float", "arg0", "~%Math./to_float (2)", "~%Math./to_float (2 | 10)", "~%Math./to_float (10)", "2", "10"),
    ("math.trunc", "arg0", "~%Math./trunc (1)", "~%Math./trunc (1 | 9)", "~%Math./trunc (9)", "1", "9"),
    ("math.trunc", "arg0", "~%Math./trunc (2)", "~%Math./trunc (2 | 10)", "~%Math./trunc (10)", "2", "10"),
    ("math.trunc", "arg0", "~%Math./trunc (3)", "~%Math./trunc (3 | 11)", "~%Math./trunc (11)", "3", "11"),
    ("path.basename", "arg0", "~%Path./basename (\"a,b\")", "~%Path./basename (\"a,b\" | \"zz\")", "~%Path./basename (\"zz\")", "\"a,b\"", "\"zz\""),
    ("path.basename", "arg0", "~%Path./basename (\"b\")", "~%Path./basename (\"b\" | \"zz\")", "~%Path./basename (\"zz\")", "\"b\"", "\"zz\""),
    ("path.dirname", "arg0", "~%Path./dirname (\"ab\")", "~%Path./dirname (\"ab\" | \"zz\")", "~%Path./dirname (\"zz\")", "\"ab\"", "\"zz\""),
    ("path.extension", "arg0", "~%Path./extension (\"ab\")", "~%Path./extension (\"ab\" | \"zz\")", "~%Path./extension (\"zz\")", "\"ab\"", "\"zz\""),
    ("path.is_absolute", "arg0", "~%Path./is_absolute (\"ab\")", "~%Path./is_absolute (\"ab\" | \"zz\")", "~%Path./is_absolute (\"zz\")", "\"ab\"", "\"zz\""),
    ("path.join", "arg0", "~%Path./join (\"ab\", \"ab\")", "~%Path./join (\"ab\" | \"zz\", \"ab\")", "~%Path./join (\"zz\", \"ab\")", "\"ab\"", "\"zz\""),
    ("path.join", "arg1", "~%Path./join (\"ab\", \"ab\")", "~%Path./join (\"ab\", \"ab\" | \"zz\")", "~%Path./join (\"ab\", \"zz\")", "\"ab\"", "\"zz\""),
    ("path.join", "arg0", "~%Path./join (\"ab\", \"a,b\")", "~%Path./join (\"ab\" | \"zz\", \"a,b\")", "~%Path./join (\"zz\", \"a,b\")", "\"ab\"", "\"zz\""),
    ("path.join", "arg1", "~%Path./join (\"ab\", \"a,b\")", "~%Path./join (\"ab\", \"a,b\" | \"zz\")", "~%Path./join (\"ab\", \"zz\")", "\"a,b\"", "\"zz\""),
    ("path.join", "arg0", "~%Path./join (\"ab\", \"b\")", "~%Path./join (\"ab\" | \"zz\", \"b\")", "~%Path./join (\"zz\", \"b\")", "\"ab\"", "\"zz\""),
    ("path.join", "arg1", "~%Path./join (\"ab\", \"b\")", "~%Path./join (\"ab\", \"b\" | \"zz\")", "~%Path./join (\"ab\", \"zz\")", "\"b\"", "\"zz\""),
    ("regex.find", "arg0", "~%Regex./find (\"ab\", \"ab\")", "~%Regex./find (\"ab\" | \"zz\", \"ab\")", "~%Regex./find (\"zz\", \"ab\")", "\"ab\"", "\"zz\""),
    ("regex.find", "arg1", "~%Regex./find (\"ab\", \"ab\")", "~%Regex./find (\"ab\", \"ab\" | \"zz\")", "~%Regex./find (\"ab\", \"zz\")", "\"ab\"", "\"zz\""),
    ("regex.find", "arg0", "~%Regex./find (\"ab\", \"a,b\")", "~%Regex./find (\"ab\" | \"zz\", \"a,b\")", "~%Regex./find (\"zz\", \"a,b\")", "\"ab\"", "\"zz\""),
    ("regex.find", "arg1", "~%Regex./find (\"ab\", \"a,b\")", "~%Regex./find (\"ab\", \"a,b\" | \"zz\")", "~%Regex./find (\"ab\", \"zz\")", "\"a,b\"", "\"zz\""),
    ("regex.match", "arg0", "~%Regex./match (\"ab\", \"ab\")", "~%Regex./match (\"ab\" | \"zz\", \"ab\")", "~%Regex./match (\"zz\", \"ab\")", "\"ab\"", "\"zz\""),
    ("regex.match", "arg1", "~%Regex./match (\"ab\", \"ab\")", "~%Regex./match (\"ab\", \"ab\" | \"zz\")", "~%Regex./match (\"ab\", \"zz\")", "\"ab\"", "\"zz\""),
    ("regex.match", "arg0", "~%Regex./match (\"ab\", \"a,b\")", "~%Regex./match (\"ab\" | \"zz\", \"a,b\")", "~%Regex./match (\"zz\", \"a,b\")", "\"ab\"", "\"zz\""),
    ("regex.match", "arg1", "~%Regex./match (\"ab\", \"a,b\")", "~%Regex./match (\"ab\", \"a,b\" | \"zz\")", "~%Regex./match (\"ab\", \"zz\")", "\"a,b\"", "\"zz\""),
    ("regex.replace", "arg0", "~%Regex./replace (\"ab\", \"ab\", \"a,b\")", "~%Regex./replace (\"ab\" | \"zz\", \"ab\", \"a,b\")", "~%Regex./replace (\"zz\", \"ab\", \"a,b\")", "\"ab\"", "\"zz\""),
    ("regex.replace", "arg1", "~%Regex./replace (\"ab\", \"ab\", \"a,b\")", "~%Regex./replace (\"ab\", \"ab\" | \"zz\", \"a,b\")", "~%Regex./replace (\"ab\", \"zz\", \"a,b\")", "\"ab\"", "\"zz\""),
    ("regex.replace", "arg2", "~%Regex./replace (\"ab\", \"ab\", \"a,b\")", "~%Regex./replace (\"ab\", \"ab\", \"a,b\" | \"zz\")", "~%Regex./replace (\"ab\", \"ab\", \"zz\")", "\"a,b\"", "\"zz\""),
    ("regex.replace", "arg0", "~%Regex./replace (\"ab\", \"ab\", \"b\")", "~%Regex./replace (\"ab\" | \"zz\", \"ab\", \"b\")", "~%Regex./replace (\"zz\", \"ab\", \"b\")", "\"ab\"", "\"zz\""),
    ("regex.replace", "arg1", "~%Regex./replace (\"ab\", \"ab\", \"b\")", "~%Regex./replace (\"ab\", \"ab\" | \"zz\", \"b\")", "~%Regex./replace (\"ab\", \"zz\", \"b\")", "\"ab\"", "\"zz\""),
    ("regex.replace", "arg2", "~%Regex./replace (\"ab\", \"ab\", \"b\")", "~%Regex./replace (\"ab\", \"ab\", \"b\" | \"zz\")", "~%Regex./replace (\"ab\", \"ab\", \"zz\")", "\"b\"", "\"zz\""),
    ("regex.replace", "arg0", "~%Regex./replace (\"b\", \"ab\", \"ab\")", "~%Regex./replace (\"b\" | \"zz\", \"ab\", \"ab\")", "~%Regex./replace (\"zz\", \"ab\", \"ab\")", "\"b\"", "\"zz\""),
    ("regex.replace", "arg1", "~%Regex./replace (\"b\", \"ab\", \"ab\")", "~%Regex./replace (\"b\", \"ab\" | \"zz\", \"ab\")", "~%Regex./replace (\"b\", \"zz\", \"ab\")", "\"ab\"", "\"zz\""),
    ("regex.replace", "arg2", "~%Regex./replace (\"b\", \"ab\", \"ab\")", "~%Regex./replace (\"b\", \"ab\", \"ab\" | \"zz\")", "~%Regex./replace (\"b\", \"ab\", \"zz\")", "\"ab\"", "\"zz\""),
    ("regex.split", "arg0", "~%Regex./split (\"ab\", \"ab\")", "~%Regex./split (\"ab\" | \"zz\", \"ab\")", "~%Regex./split (\"zz\", \"ab\")", "\"ab\"", "\"zz\""),
    ("regex.split", "arg1", "~%Regex./split (\"ab\", \"ab\")", "~%Regex./split (\"ab\", \"ab\" | \"zz\")", "~%Regex./split (\"ab\", \"zz\")", "\"ab\"", "\"zz\""),
    ("regex.split", "arg0", "~%Regex./split (\"ab\", \"a,b\")", "~%Regex./split (\"ab\" | \"zz\", \"a,b\")", "~%Regex./split (\"zz\", \"a,b\")", "\"ab\"", "\"zz\""),
    ("regex.split", "arg1", "~%Regex./split (\"ab\", \"a,b\")", "~%Regex./split (\"ab\", \"a,b\" | \"zz\")", "~%Regex./split (\"ab\", \"zz\")", "\"a,b\"", "\"zz\""),
    ("regex.split", "arg0", "~%Regex./split (\"ab\", \"b\")", "~%Regex./split (\"ab\" | \"zz\", \"b\")", "~%Regex./split (\"zz\", \"b\")", "\"ab\"", "\"zz\""),
    ("regex.split", "arg1", "~%Regex./split (\"ab\", \"b\")", "~%Regex./split (\"ab\", \"b\" | \"zz\")", "~%Regex./split (\"ab\", \"zz\")", "\"b\"", "\"zz\""),
    ("stat.histogram", "el0", "~%Stat./histogram ([1, 2, 3], 1)", "~%Stat./histogram ([1 | 9, 2, 3], 1)", "~%Stat./histogram ([9, 2, 3], 1)", "1", "9"),
    ("stat.histogram", "arg1", "~%Stat./histogram ([1, 2, 3], 1)", "~%Stat./histogram ([1, 2, 3], 1 | 9)", "~%Stat./histogram ([1, 2, 3], 9)", "1", "9"),
    ("stat.histogram", "el0", "~%Stat./histogram ([1, 2, 3], 2)", "~%Stat./histogram ([1 | 9, 2, 3], 2)", "~%Stat./histogram ([9, 2, 3], 2)", "1", "9"),
    ("stat.histogram", "arg1", "~%Stat./histogram ([1, 2, 3], 2)", "~%Stat./histogram ([1, 2, 3], 2 | 10)", "~%Stat./histogram ([1, 2, 3], 10)", "2", "10"),
    ("stat.histogram", "el0", "~%Stat./histogram ([1, 2, 3], 3)", "~%Stat./histogram ([1 | 9, 2, 3], 3)", "~%Stat./histogram ([9, 2, 3], 3)", "1", "9"),
    ("stat.histogram", "arg1", "~%Stat./histogram ([1, 2, 3], 3)", "~%Stat./histogram ([1, 2, 3], 3 | 11)", "~%Stat./histogram ([1, 2, 3], 11)", "3", "11"),
    ("stat.percentile", "el0", "~%Stat./percentile ([1, 2, 3], 1)", "~%Stat./percentile ([1 | 9, 2, 3], 1)", "~%Stat./percentile ([9, 2, 3], 1)", "1", "9"),
    ("stat.percentile", "arg1", "~%Stat./percentile ([1, 2, 3], 1)", "~%Stat./percentile ([1, 2, 3], 1 | 9)", "~%Stat./percentile ([1, 2, 3], 9)", "1", "9"),
    ("stat.percentile", "el0", "~%Stat./percentile ([1, 2, 3], 2)", "~%Stat./percentile ([1 | 9, 2, 3], 2)", "~%Stat./percentile ([9, 2, 3], 2)", "1", "9"),
    ("stat.percentile", "arg1", "~%Stat./percentile ([1, 2, 3], 2)", "~%Stat./percentile ([1, 2, 3], 2 | 10)", "~%Stat./percentile ([1, 2, 3], 10)", "2", "10"),
    ("stat.percentile", "el0", "~%Stat./percentile ([1, 2, 3], 3)", "~%Stat./percentile ([1 | 9, 2, 3], 3)", "~%Stat./percentile ([9, 2, 3], 3)", "1", "9"),
    ("stat.percentile", "arg1", "~%Stat./percentile ([1, 2, 3], 3)", "~%Stat./percentile ([1, 2, 3], 3 | 11)", "~%Stat./percentile ([1, 2, 3], 11)", "3", "11"),
    ("str.char_at", "arg1", "~%Str./char_at (1, \"ab\")", "~%Str./char_at (1, \"ab\" | \"zz\")", "~%Str./char_at (1, \"zz\")", "\"ab\"", "\"zz\""),
    ("str.char_at", "arg1", "~%Str./char_at (1, \"a,b\")", "~%Str./char_at (1, \"a,b\" | \"zz\")", "~%Str./char_at (1, \"zz\")", "\"a,b\"", "\"zz\""),
    ("str.chars", "arg0", "~%Str./chars (\"ab\")", "~%Str./chars (\"ab\" | \"zz\")", "~%Str./chars (\"zz\")", "\"ab\"", "\"zz\""),
    ("str.chars", "arg0", "~%Str./chars (\"a,b\")", "~%Str./chars (\"a,b\" | \"zz\")", "~%Str./chars (\"zz\")", "\"a,b\"", "\"zz\""),
    ("str.chars", "arg0", "~%Str./chars (\"b\")", "~%Str./chars (\"b\" | \"zz\")", "~%Str./chars (\"zz\")", "\"b\"", "\"zz\""),
    ("str.concat", "arg0", "~%Str./concat (1, 3)", "~%Str./concat (1 | 9, 3)", "~%Str./concat (9, 3)", "1", "9"),
    ("str.concat", "arg1", "~%Str./concat (1, 3)", "~%Str./concat (1, 3 | 11)", "~%Str./concat (1, 11)", "3", "11"),
    ("str.concat", "arg0", "~%Str./concat (1, 1.5)", "~%Str./concat (1 | 9, 1.5)", "~%Str./concat (9, 1.5)", "1", "9"),
    ("str.concat", "arg1", "~%Str./concat (1, 1.5)", "~%Str./concat (1, 1.5 | 9.5)", "~%Str./concat (1, 9.5)", "1.5", "9.5"),
    ("str.concat", "arg0", "~%Str./concat (1, 1+2i)", "~%Str./concat (1 | 9, 1+2i)", "~%Str./concat (9, 1+2i)", "1", "9"),
    ("str.concat", "arg1", "~%Str./concat (1, 1+2i)", "~%Str./concat (1, 1+2i | 9+9i)", "~%Str./concat (1, 9+9i)", "1+2i", "9+9i"),
    ("str.contains", "arg0", "~%Str./contains (\"ab\", \"ab\")", "~%Str./contains (\"ab\" | \"zz\", \"ab\")", "~%Str./contains (\"zz\", \"ab\")", "\"ab\"", "\"zz\""),
    ("str.contains", "arg1", "~%Str./contains (\"ab\", \"ab\")", "~%Str./contains (\"ab\", \"ab\" | \"zz\")", "~%Str./contains (\"ab\", \"zz\")", "\"ab\"", "\"zz\""),
    ("str.contains", "arg0", "~%Str./contains (\"ab\", \"a,b\")", "~%Str./contains (\"ab\" | \"zz\", \"a,b\")", "~%Str./contains (\"zz\", \"a,b\")", "\"ab\"", "\"zz\""),
    ("str.contains", "arg1", "~%Str./contains (\"ab\", \"a,b\")", "~%Str./contains (\"ab\", \"a,b\" | \"zz\")", "~%Str./contains (\"ab\", \"zz\")", "\"a,b\"", "\"zz\""),
    ("str.count", "arg0", "~%Str./count (\"ab\", \"ab\")", "~%Str./count (\"ab\" | \"zz\", \"ab\")", "~%Str./count (\"zz\", \"ab\")", "\"ab\"", "\"zz\""),
    ("str.count", "arg1", "~%Str./count (\"ab\", \"ab\")", "~%Str./count (\"ab\", \"ab\" | \"zz\")", "~%Str./count (\"ab\", \"zz\")", "\"ab\"", "\"zz\""),
    ("str.count", "arg0", "~%Str./count (\"ab\", \"a,b\")", "~%Str./count (\"ab\" | \"zz\", \"a,b\")", "~%Str./count (\"zz\", \"a,b\")", "\"ab\"", "\"zz\""),
    ("str.count", "arg1", "~%Str./count (\"ab\", \"a,b\")", "~%Str./count (\"ab\", \"a,b\" | \"zz\")", "~%Str./count (\"ab\", \"zz\")", "\"a,b\"", "\"zz\""),
    ("str.decode_uri", "arg0", "~%Str./decode_uri (\"a,b\")", "~%Str./decode_uri (\"a,b\" | \"zz\")", "~%Str./decode_uri (\"zz\")", "\"a,b\"", "\"zz\""),
    ("str.decode_uri", "arg0", "~%Str./decode_uri (\"b\")", "~%Str./decode_uri (\"b\" | \"zz\")", "~%Str./decode_uri (\"zz\")", "\"b\"", "\"zz\""),
    ("str.encode_uri", "arg0", "~%Str./encode_uri (\"a,b\")", "~%Str./encode_uri (\"a,b\" | \"zz\")", "~%Str./encode_uri (\"zz\")", "\"a,b\"", "\"zz\""),
    ("str.encode_uri", "arg0", "~%Str./encode_uri (\"b\")", "~%Str./encode_uri (\"b\" | \"zz\")", "~%Str./encode_uri (\"zz\")", "\"b\"", "\"zz\""),
    ("str.ends_with", "arg0", "~%Str./ends_with (\"ab\", \"ab\")", "~%Str./ends_with (\"ab\" | \"zz\", \"ab\")", "~%Str./ends_with (\"zz\", \"ab\")", "\"ab\"", "\"zz\""),
    ("str.ends_with", "arg1", "~%Str./ends_with (\"ab\", \"ab\")", "~%Str./ends_with (\"ab\", \"ab\" | \"zz\")", "~%Str./ends_with (\"ab\", \"zz\")", "\"ab\"", "\"zz\""),
    ("str.ends_with", "arg0", "~%Str./ends_with (\"ab\", \"a,b\")", "~%Str./ends_with (\"ab\" | \"zz\", \"a,b\")", "~%Str./ends_with (\"zz\", \"a,b\")", "\"ab\"", "\"zz\""),
    ("str.ends_with", "arg1", "~%Str./ends_with (\"ab\", \"a,b\")", "~%Str./ends_with (\"ab\", \"a,b\" | \"zz\")", "~%Str./ends_with (\"ab\", \"zz\")", "\"a,b\"", "\"zz\""),
    ("str.format", "arg0", "~%Str./format (\"a,b\", 1)", "~%Str./format (\"a,b\" | \"zz\", 1)", "~%Str./format (\"zz\", 1)", "\"a,b\"", "\"zz\""),
    ("str.format", "arg1", "~%Str./format (\"a,b\", 1)", "~%Str./format (\"a,b\", 1 | 9)", "~%Str./format (\"a,b\", 9)", "1", "9"),
    ("str.format", "arg0", "~%Str./format (\"b\", 1)", "~%Str./format (\"b\" | \"zz\", 1)", "~%Str./format (\"zz\", 1)", "\"b\"", "\"zz\""),
    ("str.format", "arg1", "~%Str./format (\"b\", 1)", "~%Str./format (\"b\", 1 | 9)", "~%Str./format (\"b\", 9)", "1", "9"),
    ("str.from_int", "arg0", "~%Str./from_int (1)", "~%Str./from_int (1 | 9)", "~%Str./from_int (9)", "1", "9"),
    ("str.from_int", "arg0", "~%Str./from_int (2)", "~%Str./from_int (2 | 10)", "~%Str./from_int (10)", "2", "10"),
    ("str.from_int", "arg0", "~%Str./from_int (3)", "~%Str./from_int (3 | 11)", "~%Str./from_int (11)", "3", "11"),
    ("str.index_of", "arg0", "~%Str./index_of (\"ab\", \"ab\")", "~%Str./index_of (\"ab\" | \"zz\", \"ab\")", "~%Str./index_of (\"zz\", \"ab\")", "\"ab\"", "\"zz\""),
    ("str.index_of", "arg1", "~%Str./index_of (\"ab\", \"ab\")", "~%Str./index_of (\"ab\", \"ab\" | \"zz\")", "~%Str./index_of (\"ab\", \"zz\")", "\"ab\"", "\"zz\""),
    ("str.index_of", "arg0", "~%Str./index_of (\"ab\", \"a,b\")", "~%Str./index_of (\"ab\" | \"zz\", \"a,b\")", "~%Str./index_of (\"zz\", \"a,b\")", "\"ab\"", "\"zz\""),
    ("str.index_of", "arg1", "~%Str./index_of (\"ab\", \"a,b\")", "~%Str./index_of (\"ab\", \"a,b\" | \"zz\")", "~%Str./index_of (\"ab\", \"zz\")", "\"a,b\"", "\"zz\""),
    ("str.index_of", "arg0", "~%Str./index_of (\"b\", \"ab\")", "~%Str./index_of (\"b\" | \"zz\", \"ab\")", "~%Str./index_of (\"zz\", \"ab\")", "\"b\"", "\"zz\""),
    ("str.index_of", "arg1", "~%Str./index_of (\"b\", \"ab\")", "~%Str./index_of (\"b\", \"ab\" | \"zz\")", "~%Str./index_of (\"b\", \"zz\")", "\"ab\"", "\"zz\""),
    ("str.is_empty", "arg0", "~%Str./is_empty (\"ab\")", "~%Str./is_empty (\"ab\" | \"zz\")", "~%Str./is_empty (\"zz\")", "\"ab\"", "\"zz\""),
    ("str.join", "arg0", "~%Str./join (\"ab\", [\"a\", \"b\"])", "~%Str./join (\"ab\" | \"zz\", [\"a\", \"b\"])", "~%Str./join (\"zz\", [\"a\", \"b\"])", "\"ab\"", "\"zz\""),
    ("str.join", "el1", "~%Str./join (\"ab\", [\"a\", \"b\"])", "~%Str./join (\"ab\", [\"a\" | \"zz\", \"b\"])", "~%Str./join (\"ab\", [\"zz\", \"b\"])", "\"a\"", "\"zz\""),
    ("str.join", "arg0", "~%Str./join (\"a,b\", [\"a\", \"b\"])", "~%Str./join (\"a,b\" | \"zz\", [\"a\", \"b\"])", "~%Str./join (\"zz\", [\"a\", \"b\"])", "\"a,b\"", "\"zz\""),
    ("str.join", "el1", "~%Str./join (\"a,b\", [\"a\", \"b\"])", "~%Str./join (\"a,b\", [\"a\" | \"zz\", \"b\"])", "~%Str./join (\"a,b\", [\"zz\", \"b\"])", "\"a\"", "\"zz\""),
    ("str.join", "arg0", "~%Str./join (\"b\", [\"a\", \"b\"])", "~%Str./join (\"b\" | \"zz\", [\"a\", \"b\"])", "~%Str./join (\"zz\", [\"a\", \"b\"])", "\"b\"", "\"zz\""),
    ("str.join", "el1", "~%Str./join (\"b\", [\"a\", \"b\"])", "~%Str./join (\"b\", [\"a\" | \"zz\", \"b\"])", "~%Str./join (\"b\", [\"zz\", \"b\"])", "\"a\"", "\"zz\""),
    ("str.len", "arg0", "~%Str./len (\"ab\")", "~%Str./len (\"ab\" | \"zz\")", "~%Str./len (\"zz\")", "\"ab\"", "\"zz\""),
    ("str.len", "arg0", "~%Str./len (\"a,b\")", "~%Str./len (\"a,b\" | \"zz\")", "~%Str./len (\"zz\")", "\"a,b\"", "\"zz\""),
    ("str.len", "arg0", "~%Str./len (\"b\")", "~%Str./len (\"b\" | \"zz\")", "~%Str./len (\"zz\")", "\"b\"", "\"zz\""),
    ("str.levenshtein", "arg0", "~%Str./levenshtein (\"ab\", \"ab\")", "~%Str./levenshtein (\"ab\" | \"zz\", \"ab\")", "~%Str./levenshtein (\"zz\", \"ab\")", "\"ab\"", "\"zz\""),
    ("str.levenshtein", "arg1", "~%Str./levenshtein (\"ab\", \"ab\")", "~%Str./levenshtein (\"ab\", \"ab\" | \"zz\")", "~%Str./levenshtein (\"ab\", \"zz\")", "\"ab\"", "\"zz\""),
    ("str.levenshtein", "arg0", "~%Str./levenshtein (\"ab\", \"a,b\")", "~%Str./levenshtein (\"ab\" | \"zz\", \"a,b\")", "~%Str./levenshtein (\"zz\", \"a,b\")", "\"ab\"", "\"zz\""),
    ("str.levenshtein", "arg1", "~%Str./levenshtein (\"ab\", \"a,b\")", "~%Str./levenshtein (\"ab\", \"a,b\" | \"zz\")", "~%Str./levenshtein (\"ab\", \"zz\")", "\"a,b\"", "\"zz\""),
    ("str.levenshtein", "arg0", "~%Str./levenshtein (\"a,b\", \"b\")", "~%Str./levenshtein (\"a,b\" | \"zz\", \"b\")", "~%Str./levenshtein (\"zz\", \"b\")", "\"a,b\"", "\"zz\""),
    ("str.levenshtein", "arg1", "~%Str./levenshtein (\"a,b\", \"b\")", "~%Str./levenshtein (\"a,b\", \"b\" | \"zz\")", "~%Str./levenshtein (\"a,b\", \"zz\")", "\"b\"", "\"zz\""),
    ("str.lines", "arg0", "~%Str./lines (\"ab\")", "~%Str./lines (\"ab\" | \"zz\")", "~%Str./lines (\"zz\")", "\"ab\"", "\"zz\""),
    ("str.lines", "arg0", "~%Str./lines (\"a,b\")", "~%Str./lines (\"a,b\" | \"zz\")", "~%Str./lines (\"zz\")", "\"a,b\"", "\"zz\""),
    ("str.lines", "arg0", "~%Str./lines (\"b\")", "~%Str./lines (\"b\" | \"zz\")", "~%Str./lines (\"zz\")", "\"b\"", "\"zz\""),
    ("str.pad_left", "arg0", "~%Str./pad_left (1, \"a,b\")", "~%Str./pad_left (1 | 9, \"a,b\")", "~%Str./pad_left (9, \"a,b\")", "1", "9"),
    ("str.pad_left", "arg1", "~%Str./pad_left (1, \"a,b\")", "~%Str./pad_left (1, \"a,b\" | \"zz\")", "~%Str./pad_left (1, \"zz\")", "\"a,b\"", "\"zz\""),
    ("str.pad_left", "arg0", "~%Str./pad_left (1, \"b\")", "~%Str./pad_left (1 | 9, \"b\")", "~%Str./pad_left (9, \"b\")", "1", "9"),
    ("str.pad_left", "arg1", "~%Str./pad_left (1, \"b\")", "~%Str./pad_left (1, \"b\" | \"zz\")", "~%Str./pad_left (1, \"zz\")", "\"b\"", "\"zz\""),
    ("str.pad_left", "arg0", "~%Str./pad_left (2, \"b\")", "~%Str./pad_left (2 | 10, \"b\")", "~%Str./pad_left (10, \"b\")", "2", "10"),
    ("str.pad_left", "arg1", "~%Str./pad_left (2, \"b\")", "~%Str./pad_left (2, \"b\" | \"zz\")", "~%Str./pad_left (2, \"zz\")", "\"b\"", "\"zz\""),
    ("str.pad_right", "arg0", "~%Str./pad_right (1, \"a,b\")", "~%Str./pad_right (1 | 9, \"a,b\")", "~%Str./pad_right (9, \"a,b\")", "1", "9"),
    ("str.pad_right", "arg1", "~%Str./pad_right (1, \"a,b\")", "~%Str./pad_right (1, \"a,b\" | \"zz\")", "~%Str./pad_right (1, \"zz\")", "\"a,b\"", "\"zz\""),
    ("str.pad_right", "arg0", "~%Str./pad_right (1, \"b\")", "~%Str./pad_right (1 | 9, \"b\")", "~%Str./pad_right (9, \"b\")", "1", "9"),
    ("str.pad_right", "arg1", "~%Str./pad_right (1, \"b\")", "~%Str./pad_right (1, \"b\" | \"zz\")", "~%Str./pad_right (1, \"zz\")", "\"b\"", "\"zz\""),
    ("str.pad_right", "arg0", "~%Str./pad_right (2, \"b\")", "~%Str./pad_right (2 | 10, \"b\")", "~%Str./pad_right (10, \"b\")", "2", "10"),
    ("str.pad_right", "arg1", "~%Str./pad_right (2, \"b\")", "~%Str./pad_right (2, \"b\" | \"zz\")", "~%Str./pad_right (2, \"zz\")", "\"b\"", "\"zz\""),
    ("str.repeat", "arg0", "~%Str./repeat (1, \"a,b\")", "~%Str./repeat (1 | 9, \"a,b\")", "~%Str./repeat (9, \"a,b\")", "1", "9"),
    ("str.repeat", "arg1", "~%Str./repeat (1, \"a,b\")", "~%Str./repeat (1, \"a,b\" | \"zz\")", "~%Str./repeat (1, \"zz\")", "\"a,b\"", "\"zz\""),
    ("str.repeat", "arg0", "~%Str./repeat (1, \"b\")", "~%Str./repeat (1 | 9, \"b\")", "~%Str./repeat (9, \"b\")", "1", "9"),
    ("str.repeat", "arg1", "~%Str./repeat (1, \"b\")", "~%Str./repeat (1, \"b\" | \"zz\")", "~%Str./repeat (1, \"zz\")", "\"b\"", "\"zz\""),
    ("str.repeat", "arg0", "~%Str./repeat (2, \"ab\")", "~%Str./repeat (2 | 10, \"ab\")", "~%Str./repeat (10, \"ab\")", "2", "10"),
    ("str.repeat", "arg1", "~%Str./repeat (2, \"ab\")", "~%Str./repeat (2, \"ab\" | \"zz\")", "~%Str./repeat (2, \"zz\")", "\"ab\"", "\"zz\""),
    ("str.replace", "arg0", "~%Str./replace (\"ab\", \"ab\", \"a,b\")", "~%Str./replace (\"ab\" | \"zz\", \"ab\", \"a,b\")", "~%Str./replace (\"zz\", \"ab\", \"a,b\")", "\"ab\"", "\"zz\""),
    ("str.replace", "arg1", "~%Str./replace (\"ab\", \"ab\", \"a,b\")", "~%Str./replace (\"ab\", \"ab\" | \"zz\", \"a,b\")", "~%Str./replace (\"ab\", \"zz\", \"a,b\")", "\"ab\"", "\"zz\""),
    ("str.replace", "arg2", "~%Str./replace (\"ab\", \"ab\", \"a,b\")", "~%Str./replace (\"ab\", \"ab\", \"a,b\" | \"zz\")", "~%Str./replace (\"ab\", \"ab\", \"zz\")", "\"a,b\"", "\"zz\""),
    ("str.replace", "arg0", "~%Str./replace (\"ab\", \"ab\", \"b\")", "~%Str./replace (\"ab\" | \"zz\", \"ab\", \"b\")", "~%Str./replace (\"zz\", \"ab\", \"b\")", "\"ab\"", "\"zz\""),
    ("str.replace", "arg1", "~%Str./replace (\"ab\", \"ab\", \"b\")", "~%Str./replace (\"ab\", \"ab\" | \"zz\", \"b\")", "~%Str./replace (\"ab\", \"zz\", \"b\")", "\"ab\"", "\"zz\""),
    ("str.replace", "arg2", "~%Str./replace (\"ab\", \"ab\", \"b\")", "~%Str./replace (\"ab\", \"ab\", \"b\" | \"zz\")", "~%Str./replace (\"ab\", \"ab\", \"zz\")", "\"b\"", "\"zz\""),
    ("str.replace", "arg0", "~%Str./replace (\"b\", \"ab\", \"ab\")", "~%Str./replace (\"b\" | \"zz\", \"ab\", \"ab\")", "~%Str./replace (\"zz\", \"ab\", \"ab\")", "\"b\"", "\"zz\""),
    ("str.replace", "arg1", "~%Str./replace (\"b\", \"ab\", \"ab\")", "~%Str./replace (\"b\", \"ab\" | \"zz\", \"ab\")", "~%Str./replace (\"b\", \"zz\", \"ab\")", "\"ab\"", "\"zz\""),
    ("str.replace", "arg2", "~%Str./replace (\"b\", \"ab\", \"ab\")", "~%Str./replace (\"b\", \"ab\", \"ab\" | \"zz\")", "~%Str./replace (\"b\", \"ab\", \"zz\")", "\"ab\"", "\"zz\""),
    ("str.reverse", "arg0", "~%Str./reverse (\"ab\")", "~%Str./reverse (\"ab\" | \"zz\")", "~%Str./reverse (\"zz\")", "\"ab\"", "\"zz\""),
    ("str.reverse", "arg0", "~%Str./reverse (\"a,b\")", "~%Str./reverse (\"a,b\" | \"zz\")", "~%Str./reverse (\"zz\")", "\"a,b\"", "\"zz\""),
    ("str.reverse", "arg0", "~%Str./reverse (\"b\")", "~%Str./reverse (\"b\" | \"zz\")", "~%Str./reverse (\"zz\")", "\"b\"", "\"zz\""),
    ("str.slice", "arg0", "~%Str./slice (1, 2, \"ab\")", "~%Str./slice (1 | 9, 2, \"ab\")", "~%Str./slice (9, 2, \"ab\")", "1", "9"),
    ("str.slice", "arg1", "~%Str./slice (1, 2, \"ab\")", "~%Str./slice (1, 2 | 10, \"ab\")", "~%Str./slice (1, 10, \"ab\")", "2", "10"),
    ("str.slice", "arg2", "~%Str./slice (1, 2, \"ab\")", "~%Str./slice (1, 2, \"ab\" | \"zz\")", "~%Str./slice (1, 2, \"zz\")", "\"ab\"", "\"zz\""),
    ("str.slice", "arg0", "~%Str./slice (1, 2, \"a,b\")", "~%Str./slice (1 | 9, 2, \"a,b\")", "~%Str./slice (9, 2, \"a,b\")", "1", "9"),
    ("str.slice", "arg1", "~%Str./slice (1, 2, \"a,b\")", "~%Str./slice (1, 2 | 10, \"a,b\")", "~%Str./slice (1, 10, \"a,b\")", "2", "10"),
    ("str.slice", "arg2", "~%Str./slice (1, 2, \"a,b\")", "~%Str./slice (1, 2, \"a,b\" | \"zz\")", "~%Str./slice (1, 2, \"zz\")", "\"a,b\"", "\"zz\""),
    ("str.slice", "arg0", "~%Str./slice (1, 3, \"a,b\")", "~%Str./slice (1 | 9, 3, \"a,b\")", "~%Str./slice (9, 3, \"a,b\")", "1", "9"),
    ("str.slice", "arg1", "~%Str./slice (1, 3, \"a,b\")", "~%Str./slice (1, 3 | 11, \"a,b\")", "~%Str./slice (1, 11, \"a,b\")", "3", "11"),
    ("str.slice", "arg2", "~%Str./slice (1, 3, \"a,b\")", "~%Str./slice (1, 3, \"a,b\" | \"zz\")", "~%Str./slice (1, 3, \"zz\")", "\"a,b\"", "\"zz\""),
    ("str.split", "arg0", "~%Str./split (\"ab\", \"ab\")", "~%Str./split (\"ab\" | \"zz\", \"ab\")", "~%Str./split (\"zz\", \"ab\")", "\"ab\"", "\"zz\""),
    ("str.split", "arg1", "~%Str./split (\"ab\", \"ab\")", "~%Str./split (\"ab\", \"ab\" | \"zz\")", "~%Str./split (\"ab\", \"zz\")", "\"ab\"", "\"zz\""),
    ("str.split", "arg0", "~%Str./split (\"ab\", \"a,b\")", "~%Str./split (\"ab\" | \"zz\", \"a,b\")", "~%Str./split (\"zz\", \"a,b\")", "\"ab\"", "\"zz\""),
    ("str.split", "arg1", "~%Str./split (\"ab\", \"a,b\")", "~%Str./split (\"ab\", \"a,b\" | \"zz\")", "~%Str./split (\"ab\", \"zz\")", "\"a,b\"", "\"zz\""),
    ("str.split", "arg0", "~%Str./split (\"ab\", \"b\")", "~%Str./split (\"ab\" | \"zz\", \"b\")", "~%Str./split (\"zz\", \"b\")", "\"ab\"", "\"zz\""),
    ("str.split", "arg1", "~%Str./split (\"ab\", \"b\")", "~%Str./split (\"ab\", \"b\" | \"zz\")", "~%Str./split (\"ab\", \"zz\")", "\"b\"", "\"zz\""),
    ("str.starts_with", "arg0", "~%Str./starts_with (\"ab\", \"ab\")", "~%Str./starts_with (\"ab\" | \"zz\", \"ab\")", "~%Str./starts_with (\"zz\", \"ab\")", "\"ab\"", "\"zz\""),
    ("str.starts_with", "arg1", "~%Str./starts_with (\"ab\", \"ab\")", "~%Str./starts_with (\"ab\", \"ab\" | \"zz\")", "~%Str./starts_with (\"ab\", \"zz\")", "\"ab\"", "\"zz\""),
    ("str.starts_with", "arg0", "~%Str./starts_with (\"ab\", \"a,b\")", "~%Str./starts_with (\"ab\" | \"zz\", \"a,b\")", "~%Str./starts_with (\"zz\", \"a,b\")", "\"ab\"", "\"zz\""),
    ("str.starts_with", "arg1", "~%Str./starts_with (\"ab\", \"a,b\")", "~%Str./starts_with (\"ab\", \"a,b\" | \"zz\")", "~%Str./starts_with (\"ab\", \"zz\")", "\"a,b\"", "\"zz\""),
    ("str.title_case", "arg0", "~%Str./title_case (\"ab\")", "~%Str./title_case (\"ab\" | \"zz\")", "~%Str./title_case (\"zz\")", "\"ab\"", "\"zz\""),
    ("str.title_case", "arg0", "~%Str./title_case (\"a,b\")", "~%Str./title_case (\"a,b\" | \"zz\")", "~%Str./title_case (\"zz\")", "\"a,b\"", "\"zz\""),
    ("str.title_case", "arg0", "~%Str./title_case (\"b\")", "~%Str./title_case (\"b\" | \"zz\")", "~%Str./title_case (\"zz\")", "\"b\"", "\"zz\""),
    ("str.to_lower", "arg0", "~%Str./to_lower (\"a,b\")", "~%Str./to_lower (\"a,b\" | \"zz\")", "~%Str./to_lower (\"zz\")", "\"a,b\"", "\"zz\""),
    ("str.to_lower", "arg0", "~%Str./to_lower (\"b\")", "~%Str./to_lower (\"b\" | \"zz\")", "~%Str./to_lower (\"zz\")", "\"b\"", "\"zz\""),
    ("str.to_upper", "arg0", "~%Str./to_upper (\"ab\")", "~%Str./to_upper (\"ab\" | \"zz\")", "~%Str./to_upper (\"zz\")", "\"ab\"", "\"zz\""),
    ("str.to_upper", "arg0", "~%Str./to_upper (\"a,b\")", "~%Str./to_upper (\"a,b\" | \"zz\")", "~%Str./to_upper (\"zz\")", "\"a,b\"", "\"zz\""),
    ("str.to_upper", "arg0", "~%Str./to_upper (\"b\")", "~%Str./to_upper (\"b\" | \"zz\")", "~%Str./to_upper (\"zz\")", "\"b\"", "\"zz\""),
    ("str.trim", "arg0", "~%Str./trim (\"a,b\")", "~%Str./trim (\"a,b\" | \"zz\")", "~%Str./trim (\"zz\")", "\"a,b\"", "\"zz\""),
    ("str.trim", "arg0", "~%Str./trim (\"b\")", "~%Str./trim (\"b\" | \"zz\")", "~%Str./trim (\"zz\")", "\"b\"", "\"zz\""),
    ("str.trim_end", "arg0", "~%Str./trim_end (\"a,b\")", "~%Str./trim_end (\"a,b\" | \"zz\")", "~%Str./trim_end (\"zz\")", "\"a,b\"", "\"zz\""),
    ("str.trim_end", "arg0", "~%Str./trim_end (\"b\")", "~%Str./trim_end (\"b\" | \"zz\")", "~%Str./trim_end (\"zz\")", "\"b\"", "\"zz\""),
    ("str.trim_start", "arg0", "~%Str./trim_start (\"a,b\")", "~%Str./trim_start (\"a,b\" | \"zz\")", "~%Str./trim_start (\"zz\")", "\"a,b\"", "\"zz\""),
    ("str.trim_start", "arg0", "~%Str./trim_start (\"b\")", "~%Str./trim_start (\"b\" | \"zz\")", "~%Str./trim_start (\"zz\")", "\"b\"", "\"zz\""),
    ("str.word_count", "arg0", "~%Str./word_count (\"ab\")", "~%Str./word_count (\"ab\" | \"zz\")", "~%Str./word_count (\"zz\")", "\"ab\"", "\"zz\""),
    ("time.add_days", "arg0", "~%Time./add_days (1, 1)", "~%Time./add_days (1 | 9, 1)", "~%Time./add_days (9, 1)", "1", "9"),
    ("time.add_days", "arg1", "~%Time./add_days (1, 1)", "~%Time./add_days (1, 1 | 9)", "~%Time./add_days (1, 9)", "1", "9"),
    ("time.add_days", "arg0", "~%Time./add_days (1, 2)", "~%Time./add_days (1 | 9, 2)", "~%Time./add_days (9, 2)", "1", "9"),
    ("time.add_days", "arg1", "~%Time./add_days (1, 2)", "~%Time./add_days (1, 2 | 10)", "~%Time./add_days (1, 10)", "2", "10"),
    ("time.add_days", "arg0", "~%Time./add_days (1, 3)", "~%Time./add_days (1 | 9, 3)", "~%Time./add_days (9, 3)", "1", "9"),
    ("time.add_days", "arg1", "~%Time./add_days (1, 3)", "~%Time./add_days (1, 3 | 11)", "~%Time./add_days (1, 11)", "3", "11"),
    ("time.add_hours", "arg0", "~%Time./add_hours (1, 1)", "~%Time./add_hours (1 | 9, 1)", "~%Time./add_hours (9, 1)", "1", "9"),
    ("time.add_hours", "arg1", "~%Time./add_hours (1, 1)", "~%Time./add_hours (1, 1 | 9)", "~%Time./add_hours (1, 9)", "1", "9"),
    ("time.add_hours", "arg0", "~%Time./add_hours (1, 2)", "~%Time./add_hours (1 | 9, 2)", "~%Time./add_hours (9, 2)", "1", "9"),
    ("time.add_hours", "arg1", "~%Time./add_hours (1, 2)", "~%Time./add_hours (1, 2 | 10)", "~%Time./add_hours (1, 10)", "2", "10"),
    ("time.add_hours", "arg0", "~%Time./add_hours (1, 3)", "~%Time./add_hours (1 | 9, 3)", "~%Time./add_hours (9, 3)", "1", "9"),
    ("time.add_hours", "arg1", "~%Time./add_hours (1, 3)", "~%Time./add_hours (1, 3 | 11)", "~%Time./add_hours (1, 11)", "3", "11"),
    ("time.add_ms", "arg0", "~%Time./add_ms (1, 3)", "~%Time./add_ms (1 | 9, 3)", "~%Time./add_ms (9, 3)", "1", "9"),
    ("time.add_ms", "arg1", "~%Time./add_ms (1, 3)", "~%Time./add_ms (1, 3 | 11)", "~%Time./add_ms (1, 11)", "3", "11"),
    ("time.add_ms", "arg0", "~%Time./add_ms (2, 3)", "~%Time./add_ms (2 | 10, 3)", "~%Time./add_ms (10, 3)", "2", "10"),
    ("time.add_ms", "arg1", "~%Time./add_ms (2, 3)", "~%Time./add_ms (2, 3 | 11)", "~%Time./add_ms (2, 11)", "3", "11"),
    ("time.add_ms", "arg0", "~%Time./add_ms (3, 3)", "~%Time./add_ms (3 | 11, 3)", "~%Time./add_ms (11, 3)", "3", "11"),
    ("time.add_ms", "arg1", "~%Time./add_ms (3, 3)", "~%Time./add_ms (3, 3 | 11)", "~%Time./add_ms (3, 11)", "3", "11"),
    ("time.diff", "arg0", "~%Time./diff (1, 2)", "~%Time./diff (1 | 9, 2)", "~%Time./diff (9, 2)", "1", "9"),
    ("time.diff", "arg1", "~%Time./diff (1, 2)", "~%Time./diff (1, 2 | 10)", "~%Time./diff (1, 10)", "2", "10"),
    ("time.diff", "arg0", "~%Time./diff (1, 3)", "~%Time./diff (1 | 9, 3)", "~%Time./diff (9, 3)", "1", "9"),
    ("time.diff", "arg1", "~%Time./diff (1, 3)", "~%Time./diff (1, 3 | 11)", "~%Time./diff (1, 11)", "3", "11"),
    ("time.format", "arg0", "~%Time./format (\"a,b\", 1)", "~%Time./format (\"a,b\" | \"zz\", 1)", "~%Time./format (\"zz\", 1)", "\"a,b\"", "\"zz\""),
    ("time.format", "arg1", "~%Time./format (\"a,b\", 1)", "~%Time./format (\"a,b\", 1 | 9)", "~%Time./format (\"a,b\", 9)", "1", "9"),
    ("time.format", "arg0", "~%Time./format (\"b\", 1)", "~%Time./format (\"b\" | \"zz\", 1)", "~%Time./format (\"zz\", 1)", "\"b\"", "\"zz\""),
    ("time.format", "arg1", "~%Time./format (\"b\", 1)", "~%Time./format (\"b\", 1 | 9)", "~%Time./format (\"b\", 9)", "1", "9"),
    ("time.to_iso8601", "arg0", "~%Time./to_iso8601 (1)", "~%Time./to_iso8601 (1 | 9)", "~%Time./to_iso8601 (9)", "1", "9"),
    ("time.to_iso8601", "arg0", "~%Time./to_iso8601 (2)", "~%Time./to_iso8601 (2 | 10)", "~%Time./to_iso8601 (10)", "2", "10"),
    ("time.to_iso8601", "arg0", "~%Time./to_iso8601 (3)", "~%Time./to_iso8601 (3 | 11)", "~%Time./to_iso8601 (11)", "3", "11"),
    ("time.weekday", "arg0", "~%Time./weekday (1)", "~%Time./weekday (1 | 9)", "~%Time./weekday (9)", "1", "9"),
    ("url.decode", "arg0", "~%Url./decode (\"a%20b\")", "~%Url./decode (\"a%20b\" | \"zz\")", "~%Url./decode (\"zz\")", "\"a%20b\"", "\"zz\""),
    ("url.encode", "arg0", "~%Url./encode (\"a b\")", "~%Url./encode (\"a b\" | \"zz\")", "~%Url./encode (\"zz\")", "\"a b\"", "\"zz\""),
    ("url.join", "arg1", "~%Url./join (\"http://a/\", \"b\")", "~%Url./join (\"http://a/\", \"b\" | \"zz\")", "~%Url./join (\"http://a/\", \"zz\")", "\"b\"", "\"zz\""),
    ("url.query_params", "arg0", "~%Url./query_params (\"http://a/b?x=1\")", "~%Url./query_params (\"http://a/b?x=1\" | \"zz\")", "~%Url./query_params (\"zz\")", "\"http://a/b?x=1\"", "\"zz\""),
];

/// Builtins r3 does not sweep, and why.
const NOT_SWEPT: &[(&str, &str)] = &[
    ("cond.", "carries its operands"),
    ("option.", "carries its operands"),
    ("result.", "carries its operands"),
    ("refl.", "reads structure"),
    ("query.", "reads structure"),
    ("diff.", "compares as `=` does (SYNTAX_06 §4 #6, #11) — g1"),
    ("set.", "compares as `=` does — g1"),
    ("list.dedup", "compares as `=` does — g1"),
    ("list.unique", "compares as `=` does"),
    ("engine.", "engine reflection"),
    ("disc.", "network"),
    ("effect.", "effects"),
    ("env.", "host"),
    ("io.", "host"),
    ("process.", "host"),
    ("csv.read_csv", "host"),
    ("math.random", "draws"),
    ("time.now", "reads the clock"),
    ("time.parse", "no bare call answers a value at v0.109.0"),
    ("csv.stringify", "no bare call answers a value at v0.109.0"),
    ("toml.stringify", "reads structure"),
    ("math.shl", "not provided by this engine"),
    ("math.shr", "not provided by this engine"),
    ("stat.mean", "a variadic tuple — r1, r2"),
    ("stat.median", "a variadic tuple — r1"),
    ("stat.variance", "a variadic tuple — r1"),
    ("stat.std_dev", "a variadic tuple, as stat.variance"),
    ("bytes.at", "the swept second index is past the bytes — r1"),
    ("bytes.base64_encode", "a bytes operand, from `~%Bytes./from_str`, which distributes on its own — g1"),
    ("bytes.concat", "a bytes operand, from `~%Bytes./from_str`, which distributes on its own — g1"),
    ("bytes.hmac_sha256", "a bytes operand, from `~%Bytes./from_str`, which distributes on its own — g1"),
    ("bytes.len", "a bytes operand, from `~%Bytes./from_str`, which distributes on its own — g1"),
    ("bytes.sha256", "a bytes operand, from `~%Bytes./from_str`, which distributes on its own — g1"),
    ("bytes.to_hex", "a bytes operand, from `~%Bytes./from_str`, which distributes on its own — g1"),
    ("bytes.to_str", "a bytes operand, from `~%Bytes./from_str`, which distributes on its own — g1"),
    ("str.parse_int", "one operand, distributed at v0.112.0 already; the swept second value does not parse — g1"),
    ("str.parse_float", "one operand, distributed at v0.112.0 already; the swept second value does not parse — g1"),
    ("url.parse", "one operand, distributed at v0.112.0 already; the swept second value does not parse — g1"),
    ("toml.parse", "one operand, distributed at v0.112.0 already; the swept second value does not parse — g1"),
    ("list.all", "operand 0 a morphism — r1"),
    ("list.any", "operand 0 a morphism — r1"),
    ("list.count", "operand 0 a morphism — r1, r2, r4"),
    ("list.drop_while", "operand 0 a morphism, as list.take_while"),
    ("list.filter", "operand 0 a morphism — r1, r2"),
    ("list.find", "operand 0 a morphism — r1"),
    ("list.flat_map", "operand 0 a morphism, its answers handed back"),
    ("list.fold", "operand 0 a morphism, its answers handed back"),
    ("list.group_by", "operand 0 a morphism, as list.max_by"),
    ("list.map", "operand 0 a morphism, its answers handed back — g1"),
    ("list.max_by", "operand 0 a morphism — r1, r2"),
    ("list.min_by", "operand 0 a morphism — r1"),
    ("list.partition", "operand 0 a morphism — r1"),
    ("list.reduce", "operand 0 a morphism, its answers handed back"),
    ("list.scan", "operand 0 a morphism, its answers handed back"),
    ("list.sort_by", "operand 0 a comparator; does not sort at v0.112.0 (Inbox)"),
    ("list.take_while", "operand 0 a morphism, as list.filter"),
    ("list.zip_with", "operand 0 a morphism, its answers handed back"),
];

/// r3 — Every builtin, each atom operand and each list's first element put
/// as a union `X | Y`: the call answers the join of the call on X and the
/// call on Y — `(call X) | (call Y)` as the engine joins it — or, where the
/// builtin hands the operand back, the union whole in its place, each
/// branch there reading as that branch's call. Where the position is read
/// (the two calls answer apart), a horizon put there is the answer, or is
/// carried in a structure. Every builtin is in the table or named in
/// NOT_SWEPT.
#[test]
fn r3_every_builtin_reads_a_union_branchwise() {
    let mut bad = Vec::new();
    let mut srcs: Vec<String> = Vec::new();
    for c in CELLS {
        srcs.push(c.2.to_string());
        srcs.push(c.3.to_string());
        srcs.push(c.4.to_string());
        srcs.push(format!("({}) | ({})", c.2, c.4));
        srcs.push(c.3.replacen(&format!("{} | {}", c.5, c.6), H, 1));
    }
    let got = eval_all(&srcs);
    let mut n = 0;
    for (c, g) in CELLS.iter().zip(got.chunks(5)) {
        let g: Vec<String> = g.iter().map(|x| short(x)).collect();
        let (plain, union, alt, joined, horizon) = (&g[0], &g[1], &g[2], &g[3], &g[4]);
        let u = format!("{} | {}", c.5, c.6);
        let carried = [u.clone(), format!("{} | {}", c.6, c.5)].iter().any(|w| {
            union.contains(w.as_str())
                && members(&union.replace(w.as_str(), c.5)) == members(plain)
                && members(&union.replace(w.as_str(), c.6)) == members(alt)
        });
        if members(union) != members(joined) && !carried {
            bad.push(format!("  {} [{}] {}\n    answers {}\n    want    {}", c.0, c.1, c.3, union.chars().take(160).collect::<String>(), joined.chars().take(160).collect::<String>()));
        }
        n += 1;
        if members(plain) != members(alt) {
            let ok = horizon == "#blur #fuel_exhausted"
                || (!horizon.starts_with('"') && !horizon.starts_with("{{") && !horizon.starts_with("_|_") && horizon.contains("#blur {"));
            if !ok {
                bad.push(format!("  {} [{}] the horizon there\n    answers {}", c.0, c.1, horizon.chars().take(160).collect::<String>()));
            }
            n += 1;
        }
    }
    for id in nlang_interpreter::builtins::contract_ids() {
        let swept = CELLS.iter().any(|c| c.0 == id);
        let named = NOT_SWEPT.iter().any(|(p, _)| id == *p || (p.ends_with('.') && id.starts_with(p)));
        if swept == named {
            bad.push(format!("  {id}: swept {swept}, named in NOT_SWEPT {named}"));
        }
    }
    verdict(bad, n);
}

/// r4 — Many branches: the call runs once per combination, and a count of
/// combinations past the fuel answers the horizon — the full answer or
/// `#fuel_exhausted`, within the kill, not a partial, not a dropped branch.
#[test]
fn r4_many_branches_answer_or_reach_the_horizon() {
    let mut bad = Vec::new();
    let bits = |n: usize| vec!["0 | 1"; n].join(", ");
    let upto = |n: usize| (0..=n).map(|i| i.to_string()).collect::<Vec<_>>().join(" | ");
    let u10 = (0..10).map(|i| i.to_string()).collect::<Vec<_>>().join(" | ");
    let exact = [
        (format!("~%List./sum ([{}])", bits(8)), upto(8)),
        (format!("~%List./count ((x -> #true | #false), [{}])", bits(6).replace("0 | 1", "1")), upto(6)),
        (format!("`{}`", "${0 | 1}".repeat(6)), {
            let mut v: Vec<String> = (0..64).map(|i| format!("\"{:06b}\"", i)).collect();
            v.sort();
            v.join(" | ")
        }),
        (format!("~%Math./add ({u10}, {u10})"), (0..19).map(|i| i.to_string()).collect::<Vec<_>>().join(" | ")),
    ];
    let srcs: Vec<String> = exact.iter().map(|c| c.0.clone()).collect();
    for ((e, want), got) in exact.iter().zip(eval_all(&srcs)) {
        check(&mut bad, e, members(&got), &members(want));
    }
    let far = [
        (format!("~%List./sum ([{}])", bits(20)), upto(20)),
        (format!("~%List./sum ([{}])", bits(40)), upto(40)),
        (format!("~%List./count ((x -> #true | #false), [{}])", bits(16).replace("0 | 1", "1")), upto(16)),
        (format!("`{}`", "${0 | 1}".repeat(30)), String::new()),
    ];
    let srcs: Vec<String> = far.iter().map(|c| c.0.clone()).collect();
    for ((e, want), got) in far.iter().zip(eval_all(&srcs)) {
        if short(&got) != "#blur #fuel_exhausted" && (want.is_empty() || members(&got) != members(want)) {
            bad.push(format!("  {}\n    answers {}", e.chars().take(80).collect::<String>(), got.chars().take(120).collect::<String>()));
        }
    }
    verdict(bad, exact.len() + far.len());
}

/// The reading of `q` after `w1` then `w2` written as one file, as two
/// evolves and one commit, and as two commits; and the three roots.
fn three_ways(w1: &str, w2: &str, q: &str) -> (Vec<String>, Vec<String>) {
    let mut got = Vec::new();
    let mut roots = Vec::new();
    for ways in [vec![vec![format!("{w1}\n{w2}")]], vec![vec![w1.to_string(), w2.to_string()]], vec![vec![w1.to_string()], vec![w2.to_string()]]] {
        let w = Ws::new("three");
        let mut rc = Vec::new();
        for commit in &ways {
            let srcs: Vec<&str> = commit.iter().map(|s| s.as_str()).collect();
            rc.push(w.writes(&srcs));
        }
        got.push(format!("{} | {}", rc.join(" "), short(&w.eval(q))));
        roots.push(w.root());
    }
    (got, roots)
}

/// r5 — Written as one file, as two evolves, as two commits: one reading,
/// one root.
#[test]
fn r5_three_ways_one_reading_one_root() {
    let mut bad = Vec::new();
    let mut n = 0;
    for (w1, w2, q, want) in [
        ("c: #true | #false", "r: c ? 1 : 2", "_.r", "1 | 2"),
        ("x: 1 | 2", "r: ~%List./sum ([x, 10])", "_.r", "11 | 12"),
        ("x: 1 | 2", "r: `v=${x}`", "_.r", "\"v=1\" | \"v=2\""),
        ("x: 1 | 2", "r: x == 1", "_.r", "#false | #true"),
        ("x: 1 | 2", "r: ~%Math./add (x, 10)", "_.r", "11 | 12"),
        ("x: 1 | 9", "r: ~%List./sort ([3, x])", "_.r", "[1, 3] | [3, 9]"),
        ("r: ~%Math./add (x, 10)", "x: 1 | 2", "_.r", "11 | 12"),
        ("x: 1 | 10", "r: (x .. 20) & 3", "_.r", "3"),
    ] {
        let (got, roots) = three_ways(w1, w2, q);
        for (way, g) in ["one file", "two evolves", "two commits"].iter().zip(got) {
            let rc = if *way == "two evolves" { "e0 e0 c0" } else if *way == "one file" { "e0 c0" } else { "e0 c0 e0 c0" };
            check(&mut bad, &format!("{w1} / {w2}, {way}: {q}"), g, &format!("{rc} | {want}"));
            n += 1;
        }
        let mut r = roots.clone();
        r.sort();
        r.dedup();
        check(&mut bad, &format!("{w1} / {w2}: one root"), r.len().to_string(), "1");
        n += 1;
    }
    verdict(bad, n);
}

/// g1 — The other half, each as at v0.112.0. What a builtin hands back or
/// applies stays whole (`reverse`, `head`, `at`'s list, `concat`, `map`'s
/// answers, `intersperse`'s separator); the `=` family compares as `=`
/// does; the set relations; a `_` branch absorbs (D119); all branches ⊥ is
/// ⊥; a lookup key, a list application, a one-operand builtin and a pipe
/// already distributed; single values; interpolation shows `_` as `_`.
#[test]
fn g1_what_is_carried_and_what_already_distributed() {
    let mut bad = Vec::new();
    let cells = [
        ("~%List./reverse ([1 | 2, 3])", "[3, 1 | 2]"),
        ("~%List./head ([1 | 2, 3])", "1 | 2"),
        ("~%List./at (0, [1 | 2, 3])", "1 | 2"),
        ("~%List./concat ([1 | 2], [3])", "[1 | 2, 3]"),
        ("~%List./len ([1 | 2, 3])", "2"),
        ("~%List./map ((x -> x + 1), [1 | 9, 2])", "[2 | 10, 3]"),
        ("~%List./intersperse (0 | 9, [1, 2])", "[1, 0 | 9, 2]"),
        ("~%Set./contains ([1, 2, 3], 1 | 9)", "#false"),
        ("~%List./dedup ([1 | 2, 1 | 2])", "[1 | 2]"),
        ("~%Set./union ([1 | 2], [3])", "[1 | 2, 3]"),
        ("(1 | 2) <= (1 | 2 | 3)", "#true"),
        ("(1 | 2) = (2 | 1)", "#true"),
        ("(1 | 2) < 1", "#false"),
        ("~%Math./add (_ | 1, 10)", "_"),
        ("(@int | 3) + 1", "_"),
        ("(@int | 3) == 3", "_"),
        ("(1 .. 5 | 3) == 3", "_"),
        ("~%Math./add (\"a\" | \"b\", 1)", "_|_ #conflict"),
        ("(#true | 5) ? 1 : 2", "_"),
        ("(\"a\" | \"b\") == 1", "#false"),
        ("~%Math./add (1, 10)", "11"),
        ("~%List./sum ([1, 10])", "11"),
        ("#true ? 1 : 2", "1"),
        ("(!#true) ? 1 : 2", "2"),
        ("`v=${_}`", "\"v=_\""),
        ("`v=${5}`", "\"v=5\""),
        ("1 == 1", "#true"),
        ("{ a: 1, b: 2 } (\"a\" | \"b\")", "1 | 2"),
        ("[10, 20] (0 | 1)", "10 | 20"),
        ("~%Str./len (\"ab\" | \"abc\")", "2 | 3"),
        ("~%Str./from_int (1 | 2)", "\"1\" | \"2\""),
        ("~%Bytes./len (~%Bytes./from_str (\"ab\" | \"abc\"))", "2 | 3"),
        ("(1 | 2) + 10", "11 | 12"),
        ("(1 | 2) |> ~%Str./from_int", "\"1\" | \"2\""),
        ("{ #true: 1, #false: 2 } (#true | #false)", "1 | 2"),
        ("~%List./sort ([3, 1, 2])", "[1, 2, 3]"),
        ("~%Str./format (\"{}\", [_])", "\"_\""),
        ("~%Str./parse_int (\"5\" | \"7\")", "5 | 7"),
        ("~%Str./parse_int (\"5\" | \"zz\")", "5"),
        ("~%Str./parse_float (\"1.5\" | \"2.5\")", "1.5 | 2.5"),
        ("~%Url./parse (\"http://a/b\" | \"http://c/d\")", "{ fragment: \"\" host: \"a\" path: \"/b\" query: \"\" scheme: \"http\" } | { fragment: \"\" host: \"c\" path: \"/d\" query: \"\" scheme: \"http\" }"),
        ("~%Toml./parse (\"a = 1\" | \"a = 2\")", "{ a: 1 } | { a: 2 }"),
    ];
    table(&mut bad, &cells);
    verdict(bad, cells.len());
}

/// The root a single evolve of `src` commits.
fn root_of(src: &str) -> String {
    let w = Ws::new("root");
    let got = w.writes(&[src]);
    if got != "e0 c0" {
        return format!("not committed: {got}");
    }
    w.root()
}

/// g2 — Roots of what read right at v0.112.0 do not move.
#[test]
fn g2_roots_that_read_right_do_not_move() {
    let mut bad = Vec::new();
    let cases = [
        ("r: ~%Math./add (1, 10)", "826f8a2a209e6682f1d54415d84034fe3a8304957d946b92ff7118a83ea6ee58"),
        ("r: #true ? 1 : 2", "04f389b7b5b4af1bca66bdf05e0ac406f47d3aff4a54b2b0a8c7884a0fc09c2c"),
        ("r: `v=${5}`", "cd63b4211b2ad56c587b162e2cbfd642e93b1ae73dd2f22ea6f4695bf9e361b5"),
        ("r: 1 == 1", "c46adf426ca9b4ff3b340ecc72d9fdfb71103913b3e7962090244e04c5b4f0b2"),
        ("r: ~%List./sum ([1, 10])", "826f8a2a209e6682f1d54415d84034fe3a8304957d946b92ff7118a83ea6ee58"),
        ("r: ~%List./sort ([3, 1, 2])", "42d52b1d8be2c4ee717a18effdfa0b14b046d0d2885794d9b4754b3d3e6aa924"),
        ("r: [3, 1 | 2]", "98cb52c063b5e339343688b6d0b193fb0a1ff7926bb0168981fc5d2a7a83e9a9"),
        ("r: ~%List./reverse ([1 | 2, 3])", "98cb52c063b5e339343688b6d0b193fb0a1ff7926bb0168981fc5d2a7a83e9a9"),
        ("r: (1 | 2) + 10", "ab1cdb01b4697d2e2b4fe4fdfd9f13556a2d84177622ef6d6a6d40eda9f62922"),
        ("r: ~%Str./len (\"ab\" | \"abc\")", "73c43dc4adfa5f0eff8e2a68c409b2bfdc9bff064e4cba53eb9fbf2a70dcd771"),
        ("r: { a: 1 | 2, b: 0 .. 5 }", "3ab8d0896b3079fa3e72f98c66ae32b64f01c8b6940eb49ec62dc1fbf80efc81"),
        ("r: (1 | 2) <= (1 | 2 | 3)", "a9bb46e377a2e5a76b0d53dee220c6e484af931ca22d2b5670025327d0be33c2"),
    ];
    for (src, root) in cases {
        check(&mut bad, src, root_of(src), root);
    }
    verdict(bad, cases.len());
}
