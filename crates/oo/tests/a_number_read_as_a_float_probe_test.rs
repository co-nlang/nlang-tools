// A number read as a float.
// Order: nlang-tools/docs/a_number_read_as_a_float_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-115.  Numbers compare by their
// value (SYNTAX_02 §1), at every entrance that compares two of them: `==`
// and `!=`, the magnitude predicates `/lt` `/lte` `/gt` `/gte`, the
// direction probe `<=>`, a range's bounds and what it holds, the lattice
// order between two numbers, a union keeping or absorbing a number,
// `/clamp`, `/min_by` and `/max_by`, and the order a union is shown in. An
// integer is never rounded to a float to be compared.
//
// ── What is wrong (v0.115.0) ────────────────────────────────────────────
//
//   With A = 2^53 + 1 and Z = 2^53 (one f64): `A == Z` is `#true`;
//   `~%Math./gt (A, Z)` is `#false`; `A <=> Z` is `#eq`; `A & (0..Z)` is
//   `A`; `(0..Z) & (A..2^64+1)`, an empty range, is `Z`; `A <= Z.0` is
//   `#true`; `A | Z.0` is `Z.0` — the union drops `A`, and `Z.0 | A` keeps
//   `A` instead (two roots for one value); `~%Math./clamp (0, Z, A)` is `A`;
//   `~%List./max_by` of `[Z, A]` is `Z`; `10^400 == 10^400 + 1` is `#true`.
//   Each converts the integer to f64 first.
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
        let scratch = nlang_interpreter::ScratchDir::new(&format!("as-float-{tag}"));
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

fn root_of(src: &str) -> String {
    let w = Ws::new("root");
    let got = w.writes(&[src]);
    if got != "e0 c0" {
        return format!("not committed: {got}");
    }
    w.root()
}

// Numbers the probe compares. `A` and `Z` are one f64 apart from nothing:
// both read as 2^53 when rounded. `G` and `H` are past 64 bits; `E0` and
// `E1` past f64 (both read as infinity).
const A: &str = "9007199254740993";
const Z: &str = "9007199254740992";
const ZF: &str = "9007199254740992.0";
const G: &str = "18446744073709551617";
const H: &str = "18446744073709551616";

fn e0() -> String {
    format!("1{}", "0".repeat(400))
}

fn e1() -> String {
    format!("1{}1", "0".repeat(399))
}

/// Fill `{A}` `{Z}` `{ZF}` `{G}` `{H}` `{E0}` `{E1}` into a cell.
fn n(s: &str) -> String {
    s.replace("{ZF}", ZF)
        .replace("{A}", A)
        .replace("{Z}", Z)
        .replace("{G}", G)
        .replace("{H}", H)
        .replace("{E0}", &e0())
        .replace("{E1}", &e1())
}

fn run_table(cells: &[(&str, &str)]) {
    let mut bad = Vec::new();
    let srcs: Vec<String> = cells.iter().map(|c| n(c.0)).collect();
    for ((e, want), got) in cells.iter().zip(eval_all(&srcs)) {
        check(&mut bad, e, short(&got), &n(want));
    }
    verdict(bad, cells.len());
}

/// r1 — Every entrance that compares two numbers answers by their value
/// (SYNTAX_02 §1): integers f64 cannot tell apart, integers past 64 bits
/// and past f64, an integer beside a float of the next value.
#[test]
fn r1_every_entrance_compares_by_value() {
    run_table(&[
        // `==` / `!=`
        ("{A} == {Z}", "#false"),
        ("{A} != {Z}", "#true"),
        ("{G} == {H}", "#false"),
        ("{E0} == {E1}", "#false"),
        ("{A} == {ZF}", "#false"),
        ("{ZF} != {A}", "#true"),
        ("{Z} == {ZF}", "#true"),
        ("3 == 3.5", "#false"),
        // the magnitude predicates (SPEC_09: the home of size)
        ("~%Math./lt ({Z}, {A})", "#true"),
        ("~%Math./lte ({A}, {Z})", "#false"),
        ("~%Math./gt ({A}, {Z})", "#true"),
        ("~%Math./gte ({Z}, {A})", "#false"),
        ("~%Math./gt ({A}, {ZF})", "#true"),
        ("~%Math./lt ({ZF}, {A})", "#true"),
        ("~%Math./gte ({Z}, {ZF})", "#true"),
        ("~%Math./lt ({E0}, {E1})", "#true"),
        ("~%Math./lt ({E0}, 1.0e309)", "#true"),
        ("~%Math./lt (3, 3.5)", "#true"),
        ("~%Math./gt (-3, -3.5)", "#true"),
        // the direction probe
        ("{A} <=> {Z}", "#gt"),
        ("{G} <=> {H}", "#gt"),
        ("{A} <=> {ZF}", "#gt"),
        ("{ZF} <=> {A}", "#lt"),
        ("{E0} <=> {E1}", "#lt"),
        ("3 <=> 3.5", "#lt"),
        // a range holds what lies inside it
        ("{A} & (0..{Z})", "_|_ #conflict"),
        ("{A} & (0.0..{ZF})", "_|_ #conflict"),
        ("@nat & {G} & (0..{H})", "_|_ #conflict"),
        ("{A} & ({A}..{G})", "{A}"),
        ("(0..{Z}) & ({A}..{G})", "_|_ #conflict"),
        ("(0..{G}) & (5..{H})", "5..{H}"),
        ("(0..{A}) & (0..{G})", "0..{A}"),
        ("({A}..{G}) & ({A}..{H})", "{A}..{H}"),
        // the lattice order: one number is under another only if equal
        ("{A} <= (0..{Z})", "#false"),
        ("{A} <= {ZF}", "#false"),
        ("{ZF} <= {A}", "#false"),
        ("{A} >= {ZF}", "#false"),
        ("{Z} <= {ZF}", "#true"),
        // a union keeps two numbers that differ, in ascending order
        ("{A} | {ZF}", "{Z} | {A}"),
        ("{ZF} | {A}", "{Z} | {A}"),
        ("{A} | {Z}", "{Z} | {A}"),
        ("{A} | {Z} | 3", "3 | {Z} | {A}"),
        // clamp: inside its bounds
        ("~%Math./clamp (0, {Z}, {A})", "{Z}"),
        ("~%Math./clamp ({A}, {G}, {Z})", "{A}"),
        ("~%Math./clamp (0, {ZF}, {A})", "{Z}"),
        // the key that is most or least
        ("~%List./max_by ((v -> v), [{Z}, {A}])", "{A}"),
        ("~%List./min_by ((v -> v), [{A}, {Z}])", "{Z}"),
        ("~%List./max_by ((v -> v.k), [{ k: {Z} }, { k: {A} }])", "{ k: {A} }"),
        ("~%List./min_by ((v -> v), [{E1}, {E0}])", "{E0}"),
    ]);
}

/// r2 — What is stored is the value: two numbers that differ are both kept
/// whichever is written first (one root); clamp answers the bound it
/// crossed, as written; a range keeps its bounds as written.
#[test]
fn r2_what_is_stored_is_the_value() {
    let mut bad = Vec::new();
    let mut k = 0;
    let same = [
        ("x: {A} | {ZF}", "x: {ZF} | {A}"),
        ("x: {A} | {Z}", "x: {Z} | {A}"),
        ("x: ~%Math./clamp (0, 10, 12)", "x: 10"),
        ("x: ~%Math./clamp (0, 10, -3)", "x: 0"),
        ("x: ~%Math./clamp (0, 10.0, 12)", "x: 10.0"),
        ("x: ~%Math./clamp (0.0, 10, -3)", "x: 0.0"),
        ("x: ~%Math./clamp ({A}, {G}, {Z})", "x: {A}"),
        ("x: (0..{G}) & (5..{H})", "x: 5..{H}"),
        ("x: (0..{A}) & (0..{G})", "x: 0..{A}"),
    ];
    let differ = [("x: {A} | {ZF}", "x: {A}"), ("x: {A} | {ZF}", "x: {ZF}"), ("x: {A} | {ZF}", "x: {A} | {Z}")];
    let all: Vec<(String, String, bool)> = same
        .iter()
        .map(|(a, b)| (n(a), n(b), true))
        .chain(differ.iter().map(|(a, b)| (n(a), n(b), false)))
        .collect();
    let mut roots = vec![(String::new(), String::new()); all.len()];
    std::thread::scope(|s| {
        for (r, (a, b, _)) in roots.iter_mut().zip(&all) {
            s.spawn(move || *r = (root_of(a), root_of(b)));
        }
    });
    for ((a, b, eq), (ra, rb)) in all.iter().zip(roots) {
        let ok = ra.len() == 64 && rb.len() == 64 && (ra == rb) == *eq;
        let label = format!("{} / {}", &a[..a.len().min(50)], &b[..b.len().min(50)]);
        check(&mut bad, &label, if ok { "ok".into() } else { format!("{ra} vs {rb}") }, "ok");
        k += 1;
    }
    verdict(bad, k);
}

/// r3 — Through a coordinate the same: one file, two evolves, two commits;
/// a number read from a coordinate, from a hybrid's `%val`, from a union's
/// branch compares by value; one root.
#[test]
fn r3_three_ways_one_answer_one_root() {
    let mut bad = Vec::new();
    let mut k = 0;
    let cells = [
        ("a: {A}", "r: a == {Z}", "#false"),
        ("r: a == {Z}", "a: {A}", "#false"),
        ("a: {A}", "r: ~%Math./gt (a, {Z})", "#true"),
        ("a: {A}", "r: a <=> {ZF}", "#gt"),
        ("a: {A}", "r: a | {ZF}", "{Z} | {A}"),
        ("r: a | {ZF}", "a: {A}", "{Z} | {A}"),
        ("a: { %val: {A}, note: \"n\" }", "r: a == {Z}", "#false"),
        ("a: {A} | 3", "r: a == {Z}", "#false"),
        ("a: {A}", "r: ~%Math./clamp (0, {Z}, a)", "{Z}"),
    ];
    for (w1, w2, want) in cells {
        let (w1, w2, want) = (n(w1), n(w2), n(want));
        let (got, roots) = three_ways(&w1, &w2, "_.r");
        let label = format!("{} / {}", &w1[..w1.len().min(40)], &w2[..w2.len().min(40)]);
        for (way, g) in ["one file", "two evolves", "two commits"].iter().zip(got) {
            let rc = if *way == "two evolves" { "e0 e0 c0" } else if *way == "one file" { "e0 c0" } else { "e0 c0 e0 c0" };
            check(&mut bad, &format!("{label}, {way}"), g, &format!("{rc} | {want}"));
            k += 1;
        }
        let mut r = roots.clone();
        r.sort();
        r.dedup();
        check(&mut bad, &format!("{label}: one root"), r.len().to_string(), "1");
        k += 1;
    }
    verdict(bad, k);
}

/// g1 — What compares right today compares the same: small numbers, an
/// integer beside the float of its own value, `<` and `>` as the lattice
/// order (not size), a mixed `/min` and `/max` answering a float, clamp
/// inside its bounds and with float bounds, a range of floats, ties in
/// `/max_by`, `=` telling `1` from `1.0`.
#[test]
fn g1_what_compared_right_compares_the_same() {
    run_table(&[
        ("3 == 3.0", "#true"),
        ("2 != 3", "#true"),
        ("{A} == {A}", "#true"),
        ("{E0} == 0", "#false"),
        ("1 = 1.0", "#false"),
        ("{A} = {A}", "#true"),
        ("~%Math./lt (1, 2)", "#true"),
        ("~%Math./gte (2.5, 2)", "#true"),
        ("~%Math./lte (2, 2.0)", "#true"),
        ("~%Math./gt (2.0, 2)", "#false"),
        ("~%Math./gt ({E0}, 1)", "#true"),
        ("~%Math./lt (1.0e308, {E0})", "#true"),
        ("~%Math./lt (\"a\", 1)", "_|_ #conflict"),
        ("3 <=> 2", "#gt"),
        ("2.5 <=> 3", "#lt"),
        ("3 <=> 3.0", "#eq"),
        ("\"a\" <=> \"b\"", "#lt"),
        ("3 > 2", "#false"),
        ("2 >= 2", "#true"),
        ("3 <= 3.0", "#true"),
        ("3 <= @int", "#true"),
        ("5 & (0..9)", "5"),
        ("10 & (0..9)", "_|_ #conflict"),
        ("(0..5) & (3..9)", "3..5"),
        ("(0.0..5.0) & (1.0..9.0)", "1..5"),
        ("(0.5..5) & (1..9.5)", "1..5"),
        ("3 | 3.0", "3"),
        ("3.0 | 3", "3"),
        ("2.5 | 2 | 3", "2 | 2.5 | 3"),
        ("1.5 | 1", "1 | 1.5"),
        ("~%Math./max (2, 2.5)", "2.5"),
        ("~%Math./min (2, 2.5)", "2"),
        ("~%Math./max (3, 3.0)", "3"),
        ("~%Math./max ({A}, {ZF})", "{Z}"),
        ("~%Math./clamp (0, 10, 5)", "5"),
        ("~%Math./clamp (0, 10, 5.0)", "5"),
        ("~%Math./clamp (0.0, 10.0, 15.0)", "10"),
        ("~%Math./clamp (3, 1, 2)", "_|_ #conflict"),
        ("~%List./max_by ((v -> v), [1, 3, 2])", "3"),
        ("~%List./max_by ((v -> v), [3, 3.0])", "3"),
        ("~%List./min_by ((v -> v.k), [{ k: 2 }, { k: 1.5 }])", "{ k: 1.5 }"),
        ("~%List./sort ([{A}, {Z}])", "[{Z}, {A}]"),
        ("~%List./unique ([{Z}, {A}])", "[{Z}, {A}]"),
        ("{A} - {Z}", "1"),
    ]);
}

/// g2 — Roots of single commits that read right at v0.115.0 do not move.
#[test]
fn g2_roots_that_read_right_do_not_move() {
    let cells = [
        ("x: 3 | 2", "030917ae484aa0f4f535870bf603ef6a9b690367e02abbf4aabc83dae179c5c0"),
        ("x: 2.0 | 3", "d08749fcf274e73e2a8de3f440b6334da4f99800677058f61b1fca72cb24d264"),
        ("x: 2.5 | 2 | 3", "67e1551d3a8a43a4f71c66d298d963a56645f02f399375184e8170cf439bb1ea"),
        ("x: (0..5) & (3..9)", "58345d1794c4166ca0ef17bea07e8ea42a62af797cc407f85e2bb1626c087412"),
        ("x: (0.0..5.0) & (1.0..9.0)", "561f592c963178e986a7193474628e71a7520fe7aca81ebc3b1324497921a7a8"),
        ("x: 5 & (0..9)", "b74988809e561e6c9d3d8899ffc876c967d9a9a98a1bee0b6bc0ed02f0394a55"),
        ("x: ~%Math./clamp (0, 10, 5)", "b74988809e561e6c9d3d8899ffc876c967d9a9a98a1bee0b6bc0ed02f0394a55"),
        ("x: ~%Math./clamp (0.0, 10.0, 15.0)", "fcbcecb586078f30f0ad8adbafb534fa1a3ba864ced8724260ac8fead7099315"),
        ("x: ~%Math./clamp (0, 10, 5.0)", "0bb1e64338e5d893e8a7e9bf2fd4348f4ee55b2e770d6d6ada23a45d533b822a"),
        ("x: ~%Math./clamp (0.0, 10.0, 5)", "b74988809e561e6c9d3d8899ffc876c967d9a9a98a1bee0b6bc0ed02f0394a55"),
        ("x: ~%Math./min (2, 2.5)", "e6a51e23814c9e3283180c4387199b68043afc21f8b9ec57c99a520cee196d38"),
        ("x: ~%Math./max (3.0, 3)", "b1a381dc92ce3d48599c209c4aa2522c5bbe39c83918a773fff6ec5c62f8cfc3"),
        ("x: 3 == 3.0", "e673665a4cd37b8ccda664081ac15d2dd529ba710e0fceae2009c1fabc2b0e94"),
        ("x: ~%Math./lt (1, 2.5)", "8c4f8528eca33f3aca0d1e3f8ce1fde3140328fbe80989fccd25fb849a0b2d92"),
        ("x: 2 <=> 3", "024d4d1cf6d68cbace6c938cbe65ac429a1b70d31e4c4223d0537f62ddac55f3"),
        ("x: ~%List./max_by ((v -> v), [1, 3, 2])", "577305451101d1416bef88bd0be0f0023b173313e9628ae964221c5afb404089"),
        ("x: [9007199254740993, 9007199254740992]", "56842b294af2e5c000723ca2350cf64c01b07b2c7e44a993e9f8154bd61f4289"),
        ("x: 9007199254740993", "581787f6200807e0a25c4fb021cb2d12050f661356a4968ab9f381b8b2dffa19"),
    ];
    let mut bad = Vec::new();
    let mut roots = vec![String::new(); cells.len()];
    std::thread::scope(|s| {
        for (r, (src, _)) in roots.iter_mut().zip(&cells) {
            s.spawn(move || *r = root_of(src));
        }
    });
    for ((src, want), got) in cells.iter().zip(roots) {
        check(&mut bad, src, got, want);
    }
    verdict(bad, cells.len());
}
