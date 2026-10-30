// A field written as null.
// Order: nlang-tools/docs/a_field_written_as_null_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-113.  A serializer writes the
// value it is given. Two values that are `=` write the same text, however
// they were built: a record written out and the same record read back
// from JSON. Every field and element is read as a value (G6; SYNTAX_06
// §4 #6, the elements a builtin reads): a hybrid as its `%val`, a union
// branch by branch (Q-112), a horizon as the answer (G3), one not yet a
// value `_` (D119). D124 (2026-10-30, the user, 甲): `~%Csv./stringify` of
// records writes the first record's keys, in order, as the header line —
// the inverse of `~%Csv./parse_with_headers`; a key a record lacks is an
// empty cell, a key the header lacks is a conflict.
//
// ── What is wrong (v0.113.0) ────────────────────────────────────────────
//
//   `~%Json./stringify ({ a: 1 })` is `{"a":null}` — every field of a
//   record written out is null, while the same record from `~%Json./parse`
//   writes `{"a":1}`. `~%Toml./stringify ({ a: 1 })` is ⊥.
//   `~%Csv./stringify` prints Rust's own structure for any rows
//   (`Combo(ComboVal { data: … Thunk { expr: … span … } })`). A field that
//   is a horizon is written as null; a union field as null.
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
        let scratch = nlang_interpreter::ScratchDir::new(&format!("written-{tag}"));
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
    for cause in ["#conflict", "#missing_key", "#no_matching_branch", "#fuel_exhausted", "#max_depth_exceeded"] {
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

/// r1 — A serializer writes the value: a record written out writes its
/// fields as the same record read back writes them; JSON, TOML and CSV
/// read back what they wrote. CSV rows that are records write a header
/// (D124); a record with a key the header lacks, rows of two kinds, a
/// cell that is a structure are a conflict.
#[test]
fn r1_a_serializer_writes_the_value() {
    let mut bad = Vec::new();
    let cells = [
        ("~%Json./stringify ({ a: 1 })", "\"\"\"{\"a\":1}\"\"\""),
        ("~%Json./stringify ({ a: \"x\", b: [1, 2], c: #true, d: #none, e: 2.5 })", "\"\"\"{\"a\":\"x\",\"b\":[1,2],\"c\":true,\"d\":null,\"e\":2.5}\"\"\""),
        ("~%Json./stringify ({ a: { b: { c: 1 } } })", "\"\"\"{\"a\":{\"b\":{\"c\":1}}}\"\"\""),
        ("~%Json./stringify ([{ a: 1 }, { a: 2 }])", "\"\"\"[{\"a\":1},{\"a\":2}]\"\"\""),
        ("~%Json./stringify ({ \"k-1\": 1 })", "\"\"\"{\"k-1\":1}\"\"\""),
        ("~%Json./stringify ({ q: \"\"\"a\"b\"\"\" })", "\"\"\"{\"q\":\"a\\\"b\"}\"\"\""),
        ("~%Json./stringify ({ a: 5 & { n: 1 } })", "\"\"\"{\"a\":5}\"\"\""),
        ("~%Json./stringify ({ a: { %val: 5, n: 1 } })", "\"\"\"{\"a\":5}\"\"\""),
        ("{ x: { a: 1 }, r: ~%Json./stringify (x) }.r", "\"\"\"{\"a\":1}\"\"\""),
        ("(~%Json./parse (~%Json./stringify ({ a: \"x\", b: [1, { c: #false }] }))) = { a: \"x\", b: [1, { c: #false }] }", "#true"),
        ("~%Toml./stringify ({ a: 1 })", "\"\"\"a = 1 \"\"\""),
        ("~%Toml./stringify ({ a: \"x\", b: [1, 2] })", "\"\"\"a = \"x\" b = [ 1, 2, ] \"\"\""),
        ("~%Toml./stringify ({ t: { u: 1 } })", "\"\"\"[t] u = 1 \"\"\""),
        ("(~%Toml./parse (~%Toml./stringify ({ a: 1, t: { u: \"x\" } }))) = { a: 1, t: { u: \"x\" } }", "#true"),
        ("~%Csv./stringify ([[\"a\", \"b\"], [\"1\", \"2\"]])", "\"\"\"a,b 1,2\"\"\""),
        ("~%Csv./stringify ([[1, 2], [3, 4]])", "\"\"\"1,2 3,4\"\"\""),
        ("~%Csv./stringify ([[\"x,y\", \"\"\"q\"uote\"\"\"]])", "\"\"\"\"x,y\",\"q\"\"uote\"\"\"\""),
        ("(~%Csv./parse (~%Csv./stringify ([[\"a\", \"b\"], [\"1\", \"2\"]]))) = [[\"a\", \"b\"], [\"1\", \"2\"]]", "#true"),
        ("~%Csv./stringify ([{ a: \"1\", b: \"2\" }])", "\"\"\"a,b 1,2\"\"\""),
        ("~%Csv./stringify ([{ a: 1, b: \"x\" }, { a: 2, b: \"y\" }])", "\"\"\"a,b 1,x 2,y\"\"\""),
        ("~%Csv./stringify ([{ a: \"1\", b: \"2\" }, { a: \"3\" }])", "\"\"\"a,b 1,2 3,\"\"\""),
        ("(~%Csv./parse_with_headers (~%Csv./stringify ([{ a: \"1\", b: \"2\" }, { a: \"3\", b: \"4\" }]))) = [{ a: \"1\", b: \"2\" }, { a: \"3\", b: \"4\" }]", "#true"),
        ("~%Csv./stringify ([{ a: \"1\" }, { a: \"2\", z: \"3\" }])", "_|_ #conflict"),
        ("~%Csv./stringify ([[\"a\"], { a: \"1\" }])", "_|_ #conflict"),
        ("~%Csv./stringify ([[[1]]])", "_|_ #conflict"),
        ("~%Str./lines (~%Csv./stringify ([{ a: \"1\", b: \"2\" }, { a: \"3\", b: \"4\" }]))", "[\"a,b\", \"1,2\", \"3,4\"]"),
        ("~%Str./lines (~%Toml./stringify ({ a: 1, b: \"x\" }))", "[\"a = 1\", \"\"\"b = \"x\"\"\"\"]"),
        ("~%Str./len (~%Csv./stringify ([[\"a\", \"b\"], [\"1\", \"2\"]]))", "7"),
    ];
    table(&mut bad, &cells);
    verdict(bad, cells.len());
}

/// r2 — A field or element is read as a value: a union branch by branch,
/// a horizon is the answer, one not yet a value `_`.
#[test]
fn r2_a_field_is_read_as_a_value() {
    let mut bad = Vec::new();
    let cells = [
        ("~%Json./stringify ({ a: 1 | 2 })", "\"\"\"{\"a\":1}\"\"\" | \"\"\"{\"a\":2}\"\"\""),
        ("~%Json./stringify ([1 | 2])", "\"[1]\" | \"[2]\""),
        ("~%Toml./stringify ({ a: 1 | 2 })", "\"\"\"a = 1 \"\"\" | \"\"\"a = 2 \"\"\""),
        ("~%Csv./stringify ([[\"a\" | \"b\"]])", "\"a\" | \"b\""),
        ("~%Csv./stringify ([{ a: \"1\" | \"2\" }])", "\"\"\"a 1\"\"\" | \"\"\"a 2\"\"\""),
        ("~%Json./stringify ({ a: ~%List./sum (~%List./range (0, 1000000)) })", "#blur #fuel_exhausted"),
        ("~%Toml./stringify ({ a: ~%List./sum (~%List./range (0, 1000000)) })", "#blur #fuel_exhausted"),
        ("~%Csv./stringify ([[~%List./sum (~%List./range (0, 1000000))]])", "#blur #fuel_exhausted"),
        ("~%Json./stringify ({ a: _ })", "_"),
        ("~%Json./stringify ({ a: @int })", "_"),
        ("~%Csv./stringify ([[_]])", "_"),
        ("~%Toml./stringify ({ a: 1.. })", "_"),
    ];
    table(&mut bad, &cells);
    verdict(bad, cells.len());
}

/// r3 — Written and read back, every JSON-shaped value is itself, and the
/// text is the text the value read back writes: leaves (numbers, strings,
/// `#true`, `#false`, `#none`) under five shapes, through JSON; tables
/// through TOML.
#[test]
fn r3_written_and_read_back_is_itself() {
    let mut bad = Vec::new();
    let leaves = ["1", "-2", "2.5", "\"x\"", "\"\"\"a\"b\"\"\"", "#true", "#false", "#none"];
    let shapes = ["{ a: L }", "{ a: [L, L] }", "[{ a: L }, { b: L }]", "{ a: { b: L } }", "{ a: L, b: { c: [L] } }"];
    let mut srcs = Vec::new();
    for l in leaves {
        for s in shapes {
            let v = s.replace('L', l);
            srcs.push(format!("(~%Json./parse (~%Json./stringify ({v}))) = ({v})"));
            srcs.push(format!("~%Json./stringify ({v}) == ~%Json./stringify (~%Json./parse (~%Json./stringify ({v})))"));
            if l != "#none" && !s.starts_with('[') {
                srcs.push(format!("(~%Toml./parse (~%Toml./stringify ({v}))) = ({v})"));
            }
        }
    }
    for (e, got) in srcs.iter().zip(eval_all(&srcs)) {
        check(&mut bad, e, got, "#true");
    }
    verdict(bad, srcs.len());
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

/// r4 — Written as one file, as two evolves, as two commits: one reading,
/// one root.
#[test]
fn r4_three_ways_one_reading_one_root() {
    let mut bad = Vec::new();
    let mut n = 0;
    for (w1, w2, q, want) in [
        ("x: { a: 1, b: \"y\" }", "r: ~%Json./stringify (x)", "_.r", "\"\"\"{\"a\":1,\"b\":\"y\"}\"\"\""),
        ("r: ~%Json./stringify (x)", "x: { a: 1, b: \"y\" }", "_.r", "\"\"\"{\"a\":1,\"b\":\"y\"}\"\"\""),
        ("rows: [{ a: \"1\", b: \"2\" }]", "r: ~%Csv./stringify (rows)", "_.r", "\"\"\"a,b 1,2\"\"\""),
        ("t: { a: 1 }", "r: ~%Toml./stringify (t)", "_.r", "\"\"\"a = 1 \"\"\""),
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

/// r5 — Depth: a record nested two hundred deep is written whole; a record
/// that holds itself is a horizon, not a record of nulls.
#[test]
fn r5_deep_and_self_holding() {
    let mut bad = Vec::new();
    let deep = format!("~%Str./len (~%Json./stringify ({}1{}))", "{ a: ".repeat(200), " }".repeat(200));
    check(&mut bad, "200 deep", short(&eval_full(&deep)), "1201");
    let w = Ws::new("self");
    fs::write(w.ws.join("f.n"), "x: { a: 1, b: x }\nr: ~%Json./stringify (x)\n").unwrap();
    let got = w.oo(&["run", "f.n", "--observe", "r"]).0;
    check(&mut bad, "x: { a: 1, b: x }, stringify x", short(&squash(&got)), "#blur #max_depth_exceeded");
    verdict(bad, 2);
}

/// g1 — The other half, each as at v0.113.0: atoms, a list, an empty
/// record, a hybrid, a union and `_` at the top, a morphism, an integer
/// past 64 bits (written as a string), what `parse` reads, what `parse`
/// gives `stringify`, an empty CSV.
#[test]
fn g1_what_was_written_right_is_written_the_same() {
    let mut bad = Vec::new();
    let cells = [
        ("~%Json./stringify (1)", "\"1\""),
        ("~%Json./stringify (\"x\")", "\"\"\"\"x\"\"\"\""),
        ("~%Json./stringify (#true)", "\"true\""),
        ("~%Json./stringify (#none)", "\"null\""),
        ("~%Json./stringify (#foo)", "\"\"\"\"#foo\"\"\"\""),
        ("~%Json./stringify (())", "\"null\""),
        ("~%Json./stringify ([1, 2])", "\"[1,2]\""),
        ("~%Json./stringify ([])", "\"[]\""),
        ("~%Json./stringify ({})", "\"{}\""),
        ("~%Json./stringify (5 & { a: 1 })", "\"5\""),
        ("~%Json./stringify (1 | 2)", "\"1\" | \"2\""),
        ("~%Json./stringify (_)", "_"),
        ("~%Json./stringify ((x -> x))", "\"{}\""),
        ("~%Json./stringify (99999999999999999999)", "\"\"\"\"99999999999999999999\"\"\"\""),
        ("~%Json./stringify (~%Json./parse (\"\"\"{\"a\": {\"b\": 2}}\"\"\"))", "\"\"\"{\"a\":{\"b\":2}}\"\"\""),
        ("~%Json./parse (\"\"\"{\"a\": 1, \"b\": [true, null]}\"\"\")", "{ a: 1 b: [#true, #none] }"),
        ("~%Toml./parse (\"\"\"a = 1\"\"\")", "{ a: 1 }"),
        ("~%Toml./stringify (~%Toml./parse (\"\"\"a = 1\"\"\"))", "\"\"\"a = 1 \"\"\""),
        ("~%Csv./parse (\"\"\"a,b\n1,2\"\"\")", "[[\"a\", \"b\"], [\"1\", \"2\"]]"),
        ("~%Csv./parse_with_headers (\"\"\"a,b\n1,2\"\"\")", "[{ a: \"1\" b: \"2\" }]"),
        ("~%Csv./stringify ([])", "\"\""),
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

/// g2 — Roots of what read right at v0.113.0 do not move.
#[test]
fn g2_roots_that_read_right_do_not_move() {
    let mut bad = Vec::new();
    let cases = [
        ("r: ~%Json./stringify ([1, 2])", "30e187c00b38f0ec73ccad3d831b6ef8d698b3f1cf5263ad6a787395cda2092b"),
        ("r: ~%Json./stringify (5)", "ef4c0a4ed3892399bf161ea1ea414391a88d838525d31af708ebe6534ad9d9fc"),
        ("r: ~%Json./parse (\"\"\"{\"a\": 1}\"\"\")", "0123237164ecb12f7558d886a0d8baceb29777fe30d8e254d282fc217bda08a2"),
        ("r: ~%Json./stringify (~%Json./parse (\"\"\"{\"a\": 1}\"\"\"))", "682a3ae6aec39c76d2b1f6ee5e68ba1d285567a1f4326411861371aa955e8014"),
        ("r: ~%Csv./parse (\"\"\"a,b\n1,2\"\"\")", "88051519be9e8b5f11ee4f9987df3fd94e4fccd1701b4258a71c5f76239c9bdd"),
        ("r: ~%Toml./parse (\"\"\"a = 1\"\"\")", "0123237164ecb12f7558d886a0d8baceb29777fe30d8e254d282fc217bda08a2"),
        ("r: ~%Toml./stringify (~%Toml./parse (\"\"\"a = 1\"\"\"))", "75e6546a62369b5815ddc906e9c3a074309c363dd38cab086e99ca1377b5b0d9"),
        ("r: ~%Json./stringify (#none)", "56d26899def851f8dcc0107457f871eb90fb1ab7cbb5d8420b46c465b501b17e"),
    ];
    for (src, root) in cases {
        check(&mut bad, src, root_of(src), root);
    }
    verdict(bad, cases.len());
}
