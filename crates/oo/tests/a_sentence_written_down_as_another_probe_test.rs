// A sentence written down as another.
// Order: nlang-tools/docs/a_sentence_written_down_as_another_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-108.  `REAL_01` §1.3 (iii): what is
// printed MUST read back — an unobserved value prints its source, and the
// source read back is the same sentence. The store keeps a recipe as its
// printed text (`store_codec`) and parses it back, so a misprinted recipe is
// committed as another sentence (the address is that sentence's node table,
// Q-052); the working set, `fmt` and `status` print the same way.
//
// ── What is wrong (v0.108.0) ────────────────────────────────────────────
//
//   `g: (y -> { k: y })`, `r: g (@int)` is printed `r: g @int` — a type
//   annotation; committed, `_.r` is the arrow itself, and `r: g @int` has
//   the same root. `g ([1])` prints `g [1]` (a subscript), `g (<<n>>)`
//   prints `g <<n>>` (does not parse: the commit stores `_`), `(!n) @ T`
//   prints `!n @ T` (`!(n @ T)`), `n @ (!n)`, `c ? (1..n) : 2`,
//   `c ? (g n) : 2` and `{ ...(!n) }` print what does not parse.
//   (R-1, the acceptance side) `g (@{ ...x })` prints `g @{ ...x }`: after
//   `@`, `{ ...x }` is a record — a type annotation again.
//
// The delivery may NOT edit this file. `rustfmt` must not touch it.


#![allow(dead_code)]

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use nlang_parser::parse_expr_only;

struct Ws {
    _scratch: nlang_interpreter::ScratchDir,
    root: PathBuf,
    ws: PathBuf,
}

impl Ws {
    fn new(tag: &str) -> Self {
        let scratch = nlang_interpreter::ScratchDir::new(&format!("sentence-{tag}"));
        let root = scratch.path().to_path_buf();
        let ws = root.join("ws");
        fs::create_dir_all(&ws).unwrap();
        Ws { _scratch: scratch, root, ws }
    }

    /// Run `oo`, killing it after 120 s: (output, rc).
    fn oo(&self, args: &[&str]) -> (String, i32) {
        let mut child = Command::new(env!("CARGO_BIN_EXE_oo"))
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
            if start.elapsed() > Duration::from_secs(120) {
                let _ = child.kill();
                let _ = child.wait();
                return (format!("TIMEOUT after 120 s: oo {}", args.join(" ")), -1);
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let o = child.wait_with_output().expect("output");
        (
            format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)).trim().to_string(),
            o.status.code().unwrap_or(-1),
        )
    }

    /// Evolve `src` from a new file: (rc, output).
    fn evolve(&self, src: &str) -> (i32, String) {
        let n = fs::read_dir(&self.ws).map(|d| d.count()).unwrap_or(0);
        let f = format!("f{n}.n");
        fs::write(self.ws.join(&f), src).unwrap();
        let (o, rc) = self.oo(&["evolve", &f]);
        (rc, squash(&o))
    }

    /// Evolve each source as its own file, then commit: the evolve exit codes
    /// and the commit's ("e0 e1 c0").
    fn writes(&self, srcs: &[&str]) -> String {
        let mut out = Vec::new();
        for s in srcs {
            out.push(format!("e{}", self.evolve(s).0));
        }
        let (_, rc) = self.oo(&["commit", "-m", "a"]);
        out.push(format!("c{rc}"));
        out.join(" ")
    }

    /// `oo eval`, the trailing effect note dropped, whitespace collapsed.
    fn eval(&self, e: &str) -> String {
        let s = self.oo(&["eval", e]).0;
        let s = match s.find(";; %effect") {
            Some(i) => s[..i].trim().to_string(),
            None => s,
        };
        squash(&s).chars().take(120).collect()
    }

    /// `oo fmt` of `src` written to a file.
    fn fmt(&self, src: &str) -> String {
        fs::write(self.ws.join("fmt.n"), src).unwrap();
        self.oo(&["fmt", "fmt.n"]).0
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
        bad.push(format!("  {label}\n    want {want}\n    got  {got}"));
    }
}

fn verdict(bad: Vec<String>, n: usize) {
    assert!(bad.is_empty(), "{} of {} wrong:\n{}", bad.len(), n, bad.join("\n"));
}

fn bottom(got: &str) -> String {
    for cause in ["#conflict", "#missing_key", "#no_matching_branch"] {
        if got.starts_with("_|_") && got.contains(cause) {
            return format!("_|_ {cause}");
        }
    }
    got.into()
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

/// The `r:` line `oo fmt` prints for `src`.
fn fmt_line(src: &str) -> String {
    let w = Ws::new("fmt");
    let out = w.fmt(src);
    out.lines().find(|l| l.starts_with("r:")).unwrap_or("").to_string()
}

/// The readings of `q` after `w1` then `w2` written as one file, as two
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
        got.push(format!("{} | {}", rc.join(" "), bottom(&w.eval(q))));
        roots.push(w.root());
    }
    (got, roots)
}

/// Sentences the store keeps as text (each reads a name), and what each reads.
const STORED: [(&str, &str); 11] = [
    ("g: (y -> { k: y })\nr: g (@int)", "{ k: {{ %kind: #type %name: \"int\" }} }"),
    ("g: (y -> y)\nr: g ([1])", "[1]"),
    ("g: (y -> y)\nn: 3\nr: g (<<n>>)", "3"),
    ("g: (y -> y)\nr: g ([1]).0", "1"),
    ("g: (y -> y)\nr: g (@int @ @int)", "{{ %kind: #type %name: \"int\" }}"),
    ("n: #true\nr: (!n) @ #false", "#false"),
    ("n: #true\nr: n @ (!n)", "_|_ #conflict"),
    ("n: 3\nr: n == 3 ? (1..n) : 2", "1..3"),
    ("g: (y -> y)\nn: 3\nr: n == 3 ? (g n) : 2", "3"),
    ("n: { a: 1 }\nr: { ...(!n) }", "{}"),
    ("g: (y -> y)\nh: { a: 1 }\nr: g (@{ ...h })", "{ a: 1 }"),
];

// ── r1: every expression the parser produces prints back to itself ─────

const ATOMS: &[&str] = &[
    "1", "-1", "\"s\"", "#a", "_", "x", "x.y", "_.x", "@int", "@{ 4.. }", "[1]", "[x, 2]",
    "{ a: 1 }", "(1, 2)", "<<x>>", "$", "1..5", "..5", "!x", "~%Math", "(y -> y)", "x @int",
    "x[0]", "/f", "@{ ...x }",
];
const OPS: &[&str] = &[
    " ", " + ", " - ", " * ", " / ", " & ", " | ", " \\ ", " == ", " < ", " <= ", " = ", " <=> ",
    " |> ", " -> ", " @ ",
];

/// Parse, print, parse again: the same tree, and printing it again prints
/// the same text. `None` when `src` is not n/ to begin with.
fn reads_back(src: &str) -> Option<Result<(), String>> {
    let mut a = parse_expr_only(src).ok()?;
    a.strip_spans();
    let p = a.to_nlang(0);
    Some(match parse_expr_only(&p) {
        Err(e) => Err(format!("{src}  printed {p:?}  does not parse: {}", e.to_string().lines().next().unwrap_or(""))),
        Ok(mut b) => {
            b.strip_spans();
            if a != b {
                Err(format!("{src}  printed {p:?}  reads back as {}", b.shape()))
            } else if b.to_nlang(0) != p {
                Err(format!("{src}  printed {p:?}  prints again as {:?}", b.to_nlang(0)))
            } else {
                Ok(())
            }
        }
    })
}

/// r1 — Every expression built from these pieces — two operands under every
/// binary operator and juxtaposition, in nine surroundings — prints as text
/// that parses back to the same tree.
#[test]
fn r1_every_expression_prints_back_to_itself() {
    let mut bad = Vec::new();
    let mut n = 0;
    for a in ATOMS {
        for b in ATOMS {
            for op in OPS {
                let e = format!("({a}){op}({b})");
                for src in [
                    e.clone(),
                    format!("f ({e})"),
                    format!("!({e})"),
                    format!("({e}).k"),
                    format!("({e}) ? 1 : 2"),
                    format!("c ? ({e}) : 2"),
                    format!("{{ k: {e} }}"),
                    format!("{{ ...({e}) }}"),
                    format!("({e}) -> 1"),
                ] {
                    if let Some(r) = reads_back(&src) {
                        n += 1;
                        if let Err(m) = r {
                            bad.push(m);
                        }
                    }
                }
            }
        }
    }
    assert!(n > 30_000, "only {n} sources parsed: the sweep is not reaching its target");
    let shown: Vec<_> = bad.iter().take(25).cloned().collect();
    assert!(bad.is_empty(), "{} of {} do not read back; first {}:\n  {}", bad.len(), n, shown.len(), shown.join("\n  "));
}

/// r2 — A sentence the store keeps as text reads back as that sentence: the
/// committed reading is `oo eval`'s, and committing `oo fmt`'s text gives
/// the same root.
#[test]
fn r2_a_stored_sentence_reads_back_as_itself() {
    let mut bad = Vec::new();
    let mut n = 0;
    for (src, want) in STORED {
        let w = Ws::new("r2");
        let lit = src.replace('\n', ", ");
        check(&mut bad, &format!("{src}: eval of the literal"), bottom(&w.eval(&format!("{{ {lit} }}.r"))), want);
        check(&mut bad, &format!("{src}: commit"), w.writes(&[src]), "e0 c0");
        check(&mut bad, &format!("{src}: committed _.r"), bottom(&w.eval("_.r")), want);
        let printed = w.fmt(src);
        let f = Ws::new("r2fmt");
        check(&mut bad, &format!("{src}: commit oo fmt's text"), f.writes(&[&printed]), "e0 c0");
        check(&mut bad, &format!("{src}: oo fmt's text, same root"), f.root(), &w.root());
        n += 5;
    }
    verdict(bad, n);
}

/// r3 — Two sentences that read differently are two addresses.
#[test]
fn r3_two_sentences_two_addresses() {
    let mut bad = Vec::new();
    let pairs = [
        ("g: (y -> { k: y })\nr: g (@int)", "g: (y -> { k: y })\nr: g @int"),
        ("g: [7, 8]\nr: (y -> g) ([1])", "g: [7, 8]\nr: (y -> g) [1]"),
        ("n: #true\nr: (!n) @ #true", "n: #true\nr: !(n @ #true)"),
    ];
    for (a, b) in pairs {
        let (ra, rb) = (root_of(a), root_of(b));
        check(&mut bad, &format!("{a}  vs  {b}: both committed"), (ra.len() == 64 && rb.len() == 64).to_string(), "true");
        check(&mut bad, &format!("{a}  vs  {b}: two roots"), (ra != rb).to_string(), "true");
    }
    verdict(bad, pairs.len() * 2);
}

/// r4 — The Inbox row: a record a call built, then a write to it — one file,
/// two evolves, two commits: one reading, one root.
#[test]
fn r4_a_call_built_record_then_a_write_every_way() {
    let mut bad = Vec::new();
    let mut n = 0;
    for (w1, w2, q, want) in [
        ("h: (y -> { k: y, f: (x -> k) }) (@int)", "h: { k: 5 }", "_.h.f 0", "5"),
        ("g: (y -> { k: y })\nh: g (@int)", "h: { k: 5 }", "_.h.k", "5"),
        ("g: (y -> y)\nh: g ([{ k: @int }])", "h: [{ k: 5 }]", "_.h.0.k", "5"),
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

/// r5 — `oo fmt` writes each of these as a sentence that reads back, and
/// printing that again changes nothing.
#[test]
fn r5_fmt_prints_a_sentence_that_reads_back() {
    let mut bad = Vec::new();
    let mut n = 0;
    for (src, _) in STORED {
        let w = Ws::new("r5");
        let once = w.fmt(src);
        let twice = w.fmt(&once);
        check(&mut bad, &format!("{src}: fmt of fmt"), squash(&twice), &squash(&once));
        // the program as a record: the source's tree and fmt's tree
        let (mut a, mut b) = (parse_expr_only(&format!("{{\n{src}\n}}")).unwrap(), parse_expr_only(&format!("{{\n{once}\n}}")).map_err(|e| e.to_string()));
        a.strip_spans();
        let same = match &mut b {
            Ok(b) => {
                b.strip_spans();
                (a == *b).to_string()
            }
            Err(e) => format!("does not parse: {}", e.lines().next().unwrap_or("")),
        };
        let line = squash(&once);
        check(&mut bad, &format!("{src}: fmt prints {line:?}, the same tree"), same, "true");
        n += 2;
    }
    verdict(bad, n);
}

/// g1 — The other half: a sentence that already read back prints exactly as
/// it did and keeps its root (v0.108.0).
#[test]
fn g1_what_already_read_back_prints_and_roots_as_before() {
    let mut bad = Vec::new();
    let cases = [
        ("g: (y -> y)\nr: g (-1)", "r: g (-1)", "2e5c1ed4f2059b2b4582c30af656873591db88afd21cae7b362f3adb986577ce"),
        ("g: (y -> y)\nr: g ([1, 2])", "r: g [1, 2]", "d59edbc300cefd3d560670daec1e455061c3f1e251f47ab6c44594f154ed00c7"),
        ("g: (y -> y)\nr: g (@{ 1.. })", "r: g @{ 1..#_ }", "a30d0b8e15476bd71c57a70bbdc3a65a353a582ab9b4ac7015e6556e2194c7a1"),
        ("n: 1\nr: n @ @int", "r: n @ @int", "79015bae7a0a5b30189e6c53db5cc3cba20d328786138dbad7f6847bae921992"),
        ("n: #true\nr: !(n @ #true)", "r: !(n @ #true)", "4334d51009098f035df247ff73c5dc6811d6e05c4ce4ec566ecfcdfd1cad7149"),
        ("n: 3\nr: n == 3 ? 2 : 1..n", "r: n == 3 ? 2 : 1..n", "70852e8113c7d691c15b08136acfa6b93fa59e7c0af9cd00bb967e6adc334e61"),
        ("n: { a: 1 }\nr: [...n]", "r: [...n]", "b79782d00f2d7b8d94bd9da0f69b54750fde6db8231a4d730cab2885ae5939d8"),
        ("g: (y -> y)\nr: g g 1", "r: g g 1", "670e619a97d3216ca52ca05086f2d990e52bd1fe15e62666f3dfa9e7ec004b11"),
        ("g: (y -> y)\nr: g (g 1)", "r: g (g 1)", "9ea8fde5504c465350296ad7bc5da9b004f7342d45e76c020b5b4823f260c11b"),
        ("g: (y -> y)\nr: (g [1, 2]).0", "r: (g [1, 2])[\"0\"]", "82b24d10304b8aa6e2d71618eb13223ef4367c83b7535006b4736d43c27aeb11"),
        ("g: [5]\nr: g[0]", "r: g[0]", "ac218bf7d5b5e621be7a2259d859de8f692bd8a08b6e2478b90c12bdb56870d3"),
        ("g: (y -> y)\nr: g []", "r: g []", "cff3cf71d63114729cb7a029e6f59de18bc70609ee144a68aac50bc178f9ddef"),
        ("g: (y -> y)\nr: g !#a", "r: g !#a", "fee24d74c28bb059a247fe60ed8ab361a2c239c290971a0f843c42f4dd9b2561"),
        ("g: (y -> y)\nn: 1\nr: n == 1 ? g ^.n : 2", "r: n == 1 ? g ^.n : 2", "c730acf8a1d17424dbcb1ed78c3c93f72ed08d0282b2facf40a5272b88336f49"),
        ("n: 1\nr: n == 1 ? n + 1 : 2", "r: n == 1 ? n + 1 : 2", "29c3c861a25ba56138f5c8e3e26beebd546fa83e612b3b23398ce03325305f23"),
        ("g: (y -> y)\nn: 1\nr: n == 1 ? g [n, 2] : 2", "r: n == 1 ? g [n, 2] : 2", "ee5b9d9f2ac5f3a3bc821d24517c1b5e4e4eee84c07b7ec283ef7fd99e03baae"),
    ];
    for (src, line, root) in cases {
        check(&mut bad, &format!("{src}: fmt"), fmt_line(src), line);
        check(&mut bad, &format!("{src}: root"), root_of(src), root);
    }
    verdict(bad, cases.len() * 2);
}

/// g2 — Identity red lines do not move.
#[test]
fn g2_identity_red_lines() {
    let mut bad = Vec::new();
    let cases = [
        ("x: 0", "b74d7758"),
        ("v: 1 + 1", "df91a8ad"),
        ("v: 1+1", "df91a8ad"),
        ("f: (x -> x + 1)", "258a1a08"),
        ("k: { a: 1, _: 9 }", "420a0c9f"),
        ("m: (_ -> 1)", "cf11d2ff"),
    ];
    for (src, root) in cases {
        let got = root_of(src);
        check(&mut bad, src, got[..8.min(got.len())].to_string(), root);
    }
    let w = Ws::new("g2");
    w.writes(&["x: 0"]);
    let head = fs::read_to_string(w.ws.join(".oo").join("HEAD")).unwrap_or_default();
    let (o, _) = w.oo(&["inspect", head.trim()]);
    let root = o.lines().find(|l| l.contains("root:")).and_then(|l| l.split_whitespace().last()).unwrap_or("").to_string();
    let (r, _) = w.oo(&["inspect", &root]);
    check(&mut bad, "standard root digest", r.contains("c96c29378c9395936ea4999db37e9b6a28291b57a47d4c016ca3cdf6833fed3b").to_string(), "true");
    verdict(bad, cases.len() + 1);
}
