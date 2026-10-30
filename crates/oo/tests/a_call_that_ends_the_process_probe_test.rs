// A call that ends the process.
// Order: nlang-tools/docs/a_call_that_ends_the_process_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-111.  Every call answers a value
// of the lattice or a named boundary error (TAG_REGISTRY §2.7.5, D62); a
// panic or an abort is neither. D122 (2026-10-29, the user, 甲): a builtin
// whose work a number sets bills that work to the fuel before doing it —
// 1 + 0.1 MBU per unit it will make (an element, a character, a digit), as
// a combo of that many fields is billed (SPEC_08 §3.1.1) — and too little
// fuel answers the horizon, `#fuel_exhausted`.
//
// ── What is wrong (v0.111.0) ────────────────────────────────────────────
//
//   Ten builtins end the process on some operand: `~%Math./clamp (3, 1, 2)`
//   panics (`min > max`); `~%Stat./percentile ([1, 2], 9223372036854775807)`
//   indexes past the list; `~%Time./add_days (0, 9223372036854775807)`
//   overflows chrono; `~%Str./repeat`, `pad_left`, `pad_right`,
//   `~%List./range`, `~%Stat./histogram` with a huge count abort on the
//   allocation; `~%Math./factorial`, `~%List./slice (2, 10^9, [1, 2])`
//   never finish. `~%Math./pow (2, 4294967296)` answers 1 (the exponent read
//   as 0). Written to a coordinate, evolve dies.
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
        let scratch = nlang_interpreter::ScratchDir::new(&format!("call-{tag}"));
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
        bad.push(format!("  {label}\n    want {want}\n    got  {}", got.chars().take(160).collect::<String>()));
    }
}

fn verdict(bad: Vec<String>, n: usize) {
    assert!(bad.is_empty(), "{} of {} wrong:\n{}", bad.len(), n, bad.join("\n"));
}

fn bottom(got: &str) -> String {
    for cause in ["#conflict", "#missing_key", "#no_matching_branch", "#numerical_error"] {
        if got.starts_with("_|_") && got.contains(cause) {
            return format!("_|_ {cause}");
        }
    }
    got.into()
}

/// `oo eval` from an empty directory (no universe), not cut short.
fn eval_full(e: &str) -> String {
    Ws::new("eval").eval(e)
}

/// The same, a ⊥ reduced to its cause.
fn eval_here(e: &str) -> String {
    bottom(&eval_full(e))
}

/// Many `oo eval`s, six at a time.
fn eval_all(srcs: &[String]) -> Vec<String> {
    let mut out = vec![String::new(); srcs.len()];
    let per = srcs.len().div_ceil(6).max(1);
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
    let srcs: Vec<String> = cells.iter().map(|(e, _)| e.to_string()).collect();
    for ((e, want), got) in cells.iter().zip(eval_all(&srcs)) {
        check(bad, e, bottom(&got), want);
    }
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


const BLUR_FUEL: &str = "#blur #fuel_exhausted";

/// A `#blur` reduced to its cause, as `bottom` reduces a ⊥.
fn horizon(got: &str) -> String {
    if got.starts_with("#blur") && got.contains("#fuel_exhausted") {
        return BLUR_FUEL.into();
    }
    bottom(got)
}

fn table_h(bad: &mut Vec<String>, cells: &[(&str, &str)]) {
    let srcs: Vec<String> = cells.iter().map(|(e, _)| e.to_string()).collect();
    for ((e, want), got) in cells.iter().zip(eval_all(&srcs)) {
        check(bad, e, horizon(&got), want);
    }
}

/// r1 — An operand outside a builtin's domain answers a value: an empty
/// interval holds none (⊥ `#conflict`, as `(3..1) & 2`); past the calendar
/// is overflow (⊥ `#numerical_error`); a percentile past either end is that
/// end; a slice past the list is the list.
#[test]
fn r1_outside_the_domain_is_an_answer() {
    let mut bad = Vec::new();
    let cells = [
        ("~%Math./clamp (3, 1, 2)", "_|_ #conflict"),
        ("~%Math./clamp (0, -2.5, 1)", "_|_ #conflict"),
        ("~%Math./clamp (2.5, 1.5, 0)", "_|_ #conflict"),
        ("~%Math./clamp (1, -1, 9223372036854775807)", "_|_ #conflict"),
        ("~%Time./add_days (0, 9223372036854775807)", "_|_ #numerical_error"),
        ("~%Time./add_days (0, 0 - 9223372036854775807)", "_|_ #numerical_error"),
        ("~%Time./add_hours (0, 9223372036854775807)", "_|_ #numerical_error"),
        ("~%Time./add_days (0, 1000000000)", "_|_ #numerical_error"),
        ("~%Stat./percentile ([1, 2], 9223372036854775807)", "2"),
        ("~%Stat./percentile ([1, 2], 1000000)", "2"),
        ("~%Stat./percentile ([1, 2], 0 - 1000000)", "1"),
        ("~%List./slice (2, 1000000000, [1, 2])", "[]"),
        ("~%List./slice (0, 1000000000000, [1, 2, 3])", "[1, 2, 3]"),
        ("~%List./slice (0 - 1000000000000, 2, [1, 2, 3])", "[1, 2]"),
        // R-1: a list whose own index key is huge (found by the delivery).
        ("~%List./slice (0, 100000001, { %kind: #list, 0: 1, 100000000: 2 })", "[1, 2]"),
    ];
    table_h(&mut bad, &cells);
    verdict(bad, cells.len());
}

/// r2 — Work a number sets is billed first (D122): too big for the fuel is
/// the horizon, before anything is built; within the fuel, the answer.
#[test]
fn r2_work_a_number_sets_is_billed_first() {
    let mut bad = Vec::new();
    let cells = [
        ("~%Str./repeat (9223372036854775807, \"ab\")", BLUR_FUEL),
        ("~%Str./repeat (1000000000, \"ab\")", BLUR_FUEL),
        ("~%Str./pad_left (9223372036854775807, \"ab\")", BLUR_FUEL),
        ("~%Str./pad_right (1000000000, \"ab\")", BLUR_FUEL),
        ("~%List./range (0, 9223372036854775807)", BLUR_FUEL),
        ("~%List./range (0 - 1, 1000000)", BLUR_FUEL),
        ("~%List./range (0, 200000)", BLUR_FUEL),
        ("~%Stat./histogram ([1, 2], 9223372036854775807)", BLUR_FUEL),
        ("~%Stat./histogram ([1, 2], 1000000)", BLUR_FUEL),
        ("~%Math./factorial (9223372036854775807)", BLUR_FUEL),
        ("~%Math./factorial (1000000)", BLUR_FUEL),
        ("~%Math./pow (2, 1000000)", BLUR_FUEL),
        ("~%Math./pow (3, 9223372036854775807)", BLUR_FUEL),
        ("~%Math./choose (1000000000000000000, 500000000000000000)", BLUR_FUEL),
        ("~%Str./repeat (100000000000000000000, \"ab\")", BLUR_FUEL),
        ("~%Math./pow (1000000, 100000000000000000000)", BLUR_FUEL),
        ("~%List./len (~%List./range (0, 20000))", "20000"),
        ("~%Str./len (~%Str./repeat (20000, \"a\"))", "20000"),
        ("~%Str./len (~%Str./pad_left (20000, \"a\"))", "20000"),
        ("~%List./len (~%Stat./histogram ([1, 2], 2000))", "2000"),
        ("~%Str./len (~%Str./from_int (~%Math./factorial (1000)))", "2568"),
        ("~%Str./len (~%Str./from_int (~%Math./pow (2, 10000)))", "3011"),
        // R-1: C(n, k) is billed its own digits, not those of n^k.
        ("~%Str./len (~%Str./from_int (~%Math./choose (100000, 50000)))", "30101"),
        ("~%Str./len (~%Str./from_int (~%Math./choose (100000, 25000)))", "24420"),
    ];
    table_h(&mut bad, &cells);
    // R-1: numbers past the f64 range. Too big for the fuel is the horizon;
    // within it, the answer (a logarithm that overflows is not a free bill).
    let big = format!("1{}", "0".repeat(400));
    let e300 = format!("1{}", "0".repeat(300));
    let far: Vec<(String, &str)> = vec![
        (format!("~%Math./factorial ({big})"), BLUR_FUEL),
        (format!("~%Math./pow (2, {big})"), BLUR_FUEL),
        (format!("~%Math./choose ({big}, 1{})", "0".repeat(200)), BLUR_FUEL),
        (format!("~%Math./choose ({e300}, 100000)"), BLUR_FUEL),
        (format!("~%Str./len (~%Str./from_int (~%Math./pow ({big}, 2)))"), "801"),
        (format!("~%Str./len (~%Str./from_int (~%Math./choose ({big}, 3)))"), "1200"),
    ];
    let cells_far: Vec<(&str, &str)> = far.iter().map(|(e, w)| (e.as_str(), *w)).collect();
    table_h(&mut bad, &cells_far);
    verdict(bad, cells.len() + cells_far.len());
}

/// r3 — A number too big for the machine word is still that number: the
/// exponent is not read as 0, a count is not read as 0 or 1.
#[test]
fn r3_a_big_number_is_not_read_as_a_small_one() {
    let mut bad = Vec::new();
    let cells = [
        ("~%Math./pow (2, 4294967296)", BLUR_FUEL),
        ("~%Math./pow (0, 4294967296)", "0"),
        ("~%Math./pow (1, 9223372036854775807)", "1"),
        ("~%Math./pow (0 - 1, 4294967297)", "-1"),
        ("~%Math./pow (0 - 1, 4294967296)", "1"),
        ("~%Str./repeat (9223372036854775807, \"\")", "\"\""),
        ("~%Stat./histogram (\"ab\", 9223372036854775807)", "_|_ #conflict"),
    ];
    table_h(&mut bad, &cells);
    verdict(bad, cells.len());
}

/// r4 — Written to a coordinate: one file, two evolves, two commits answer,
/// read the same and commit one root.
#[test]
fn r4_three_ways_one_reading_one_root() {
    let mut bad = Vec::new();
    let cells = [
        ("n: 9223372036854775807", "r: ~%Str./repeat (n, \"ab\")", "_.r", BLUR_FUEL),
        ("n: 3", "r: ~%Math./clamp (n, 1, 2)", "_.r", "_|_ #conflict"),
        ("n: 9223372036854775807", "r: ~%Time./add_days (0, n)", "_.r", "_|_ #numerical_error"),
        ("n: 3", "r: ~%Str./repeat (n, \"ab\")", "_.r", "\"ababab\""),
    ];
    for (w1, w2, q, want) in cells {
        let (got, roots) = three_ways(w1, w2, q);
        let want = [format!("e0 c0 | {want}"), format!("e0 e0 c0 | {want}"), format!("e0 c0 e0 c0 | {want}")];
        for (g, w) in got.iter().zip(want.iter()) {
            let g = match g.split_once(" | ") {
                Some((rc, v)) => format!("{rc} | {}", horizon(v)),
                None => g.clone(),
            };
            check(&mut bad, &format!("{w1} ¦ {w2} ¦ {q}"), g, w);
        }
        if roots[0].is_empty() || roots.iter().any(|r| r != &roots[0]) {
            bad.push(format!("  {w1} ¦ {w2}: roots {roots:?}"));
        }
    }
    verdict(bad, cells.len() * 4);
}

/// Builtins that act on the host or its clock: not swept (r5 would write
/// files, end the process or sleep), each for the reason given.
const NOT_SWEPT: &[(&str, &str)] = &[
    ("io.", "writes and reads the host's files"),
    ("process.", "ends or spawns processes"),
    ("env.", "reads the host's environment"),
    ("discovery.", "opens the network"),
    ("engine.", "reads and writes the store"),
    ("config.", "writes the universe's config"),
    ("effect.", "runs an effect"),
    ("path.", "reads the host's file system"),
    ("time.sleep", "sleeps"),
    ("math.random", "answers a new number each time"),
];

/// Ten operands: the small ones a builtin reads, the huge ones that made
/// one end the process, and those not yet a value. The exhaustive sweep
/// (19 operands, every pair, 44,053 calls) is `acc/q111/crash_full.py`.
const OPS: &[&str] = &[
    "_", "0", "0 - 1", "2.5", "\"ab\"", "[1, 2]", "{ a: 1 }", "(x -> x)", "9223372036854775807",
    "1000000000",
];

/// r5 — Every builtin in the registry, with every operand pair (and a
/// sample of triples) drawn from ten operands, ints up to 2^63 among them:
/// no call panics, aborts or fails to return.
#[test]
fn r5_no_builtin_call_ends_the_process() {
    let mut bad = Vec::new();
    for id in nlang_interpreter::builtins::contract_ids() {
        if !SPELL.iter().any(|(i, _)| *i == id) {
            bad.push(format!("  {id}: in the registry, not spelled in SPELL"));
        }
    }
    let mut srcs = Vec::new();
    let mut seed: u64 = 7;
    for (id, s) in SPELL {
        if NOT_SWEPT.iter().any(|(p, _)| id.starts_with(p)) {
            continue;
        }
        let k = nlang_interpreter::builtins::contract_for(id).map(|c| c.required.len()).unwrap_or(1).max(1);
        let mut tuples: Vec<Vec<&str>> = Vec::new();
        if k <= 2 {
            let mut t: Vec<Vec<&str>> = vec![vec![]];
            for _ in 0..k {
                t = t.into_iter().flat_map(|p| OPS.iter().map(move |o| { let mut q = p.clone(); q.push(*o); q })).collect();
            }
            tuples = t;
        } else {
            for o in OPS {
                tuples.push(vec![*o; k]);
            }
            for _ in 0..40 {
                tuples.push((0..k).map(|_| { seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); OPS[(seed >> 33) as usize % OPS.len()] }).collect());
            }
        }
        for t in tuples {
            srcs.push(format!("{s} ({})", t.join(", ")));
        }
    }
    let got = eval_all(&srcs);
    for (e, g) in srcs.iter().zip(&got) {
        if g == "DID NOT RETURN" || g == "CRASHED" || g == "PANIC" {
            bad.push(format!("  {e}: {g}"));
        }
    }
    verdict(bad, srcs.len());
}

/// g1 — The other half. Calls within the fuel answer as they did; what was
/// already an answer at the edge stays (`~%Stat./percentile ([1, 2], 101)`
/// is 2); roots of single writes do not move.
#[test]
fn g1_what_answered_answers_the_same() {
    let mut bad = Vec::new();
    let cells = [
        ("~%Math./clamp (1, 3, 5)", "3"),
        ("~%Math./clamp (1, 3, 2)", "2"),
        ("~%Math./clamp (1.5, 1.5, 0)", "1.5"),
        ("~%Time./add_days (0, 1)", "86400000"),
        ("~%Time./add_hours (0, 0 - 1)", "-3600000"),
        ("~%Stat./percentile ([1, 2], 101)", "2"),
        ("~%Stat./percentile ([1, 2], 0 - 5)", "1"),
        ("~%Stat./percentile ([1, 2, 3, 4], 30)", "1.9"),
        ("~%Stat./histogram ([1, 2, 3], 2)", "[[1], [2, 3]]"),
        ("~%List./slice (1, 3, [1, 2, 3, 4])", "[2, 3]"),
        ("~%List./range (2, 5)", "[2, 3, 4]"),
        ("~%List./range (5, 2)", "[]"),
        ("~%Str./repeat (3, \"ab\")", "\"ababab\""),
        ("~%Str./repeat (0 - 1, \"ab\")", "\"\""),
        ("~%Str./len (~%Str./pad_left (5, \"ab\"))", "5"),
        ("~%Str./ends_with (\"ab\", ~%Str./pad_left (5, \"ab\"))", "#true"),
        ("~%Str./pad_right (1, \"ab\")", "\"ab\""),
        ("~%Math./pow (2, 100)", "1267650600228229401496703205376"),
        ("~%Math./pow (2, 0 - 1)", "_|_ #conflict"),
        ("~%Math./factorial (20)", "2432902008176640000"),
        ("~%Math./factorial (0 - 1)", "_|_ #conflict"),
        ("~%Math./choose (10, 3)", "120"),
        ("(3 .. 1) & 2", "_|_ #conflict"),
    ];
    table_h(&mut bad, &cells);
    let roots = [
        ("r: ~%List./range (0, 5)", "da3121211313c07a3d880bd144b85e18c9b6d419a122ae7500a8357f60725b91"),
        ("r: ~%Str./repeat (3, \"ab\")", "3ae5c3fd119898bc4b4b4832310387d2cc6f8a3332585f25049da3729695f156"),
        ("r: ~%Math./clamp (1, 3, 5)", "af54e9d17643ac667071bb048984da18f61fb23eb5ab5aa3c579ab9238e6d6d4"),
        ("r: ~%Math./pow (2, 100)", "ed17526b2822a517403987b0528e0403ccc557f4950a646b27e0b86609769330"),
        ("r: ~%Math./factorial (20)", "51ce4814af2f157bb521f6ea3b46542148366f30387c43270b9e8d7901c7591e"),
        ("r: ~%Math./choose (10, 3)", "873b93d90e1f1ee29dd83b718585cf266cae7084c4d3528a4325a22ac08f8549"),
        ("r: ~%Stat./histogram ([1, 2, 3], 2)", "e75a6cdd760aa536b6de317d85c1468d0795084cf155128e2286721b57360f2b"),
        ("r: ~%Stat./percentile ([1, 2, 3, 4], 30)", "e3f8d2bd5e5ae2b4413a4ca9be7c86d6d3944b2f54732ac70c44e70744a14fdc"),
        ("r: ~%Time./add_days (0, 1)", "95a39966e4df44cbe9de060c8cbb8bc820f3b4cbc77554b6e004345acb4e9a81"),
        ("r: ~%List./slice (1, 3, [1, 2, 3, 4])", "8ec20cdd768020ac9244370cb0902851fa25fff4d51d76f34c381d3a436e7ece"),
        ("r: ~%Str./pad_left (5, \"ab\")", "84880f2eb650c151ee5a48a55e12e03b7751003ee0bea7431cb642bf3e0e8f0d"),
        ("r: ~%Str./pad_right (5, \"ab\")", "99683fafd4fa32d1358b62741407ed0d5f3bf2c90567393fa42cf4552c98ddde"),
    ];
    for (src, root) in roots {
        check(&mut bad, src, root_of(src), root);
    }
    verdict(bad, cells.len() + roots.len());
}

/// Every builtin the standard root spells: each `~%Ns` and `@option` /
/// `@result` printed at v0.110.0, one spelling per id.
const SPELL: &[(&str, &str)] = &[
    ("math.abs", "~%Math./abs"),
    ("math.add", "~%Math./add"),
    ("math.atan2", "~%Math./atan2"),
    ("math.bitAnd", "~%Math./bitAnd"),
    ("math.bitNot", "~%Math./bitNot"),
    ("math.bitOr", "~%Math./bitOr"),
    ("math.bitXor", "~%Math./bitXor"),
    ("math.bits", "~%Math./bits"),
    ("math.ceil", "~%Math./ceil"),
    ("math.choose", "~%Math./choose"),
    ("math.clamp", "~%Math./clamp"),
    ("math.cos", "~%Math./cos"),
    ("math.cosh", "~%Math./cosh"),
    ("math.div", "~%Math./div"),
    ("math.eml", "~%Math./eml"),
    ("math.exp", "~%Math./exp"),
    ("math.factorial", "~%Math./factorial"),
    ("math.floor", "~%Math./floor"),
    ("math.fract", "~%Math./fract"),
    ("math.gcd", "~%Math./gcd"),
    ("math.gt", "~%Math./gt"),
    ("math.gte", "~%Math./gte"),
    ("math.hypot", "~%Math./hypot"),
    ("math.is_prime", "~%Math./is_prime"),
    ("math.lcm", "~%Math./lcm"),
    ("math.ln", "~%Math./ln"),
    ("math.log10", "~%Math./log10"),
    ("math.log2", "~%Math./log2"),
    ("math.lt", "~%Math./lt"),
    ("math.lte", "~%Math./lte"),
    ("math.max", "~%Math./max"),
    ("math.min", "~%Math./min"),
    ("math.mul", "~%Math./mul"),
    ("math.pow", "~%Math./pow"),
    ("math.pow_mod", "~%Math./pow_mod"),
    ("math.random", "~%Math./random"),
    ("math.rem", "~%Math./rem"),
    ("math.round", "~%Math./round"),
    ("math.shl", "~%Math./shl"),
    ("math.shr", "~%Math./shr"),
    ("math.sign", "~%Math./sign"),
    ("math.sin", "~%Math./sin"),
    ("math.sinh", "~%Math./sinh"),
    ("math.sqrt", "~%Math./sqrt"),
    ("math.sub", "~%Math./sub"),
    ("math.tanh", "~%Math./tanh"),
    ("math.to_float", "~%Math./to_float"),
    ("math.trunc", "~%Math./trunc"),
    ("list.all", "~%List./all"),
    ("list.any", "~%List./any"),
    ("list.at", "~%List./at"),
    ("list.chunk", "~%List./chunk"),
    ("list.concat", "~%List./concat"),
    ("list.count", "~%List./count"),
    ("list.dedup", "~%List./dedup"),
    ("list.drop", "~%List./drop"),
    ("list.drop_while", "~%List./drop_while"),
    ("list.enumerate", "~%List./enumerate"),
    ("list.filter", "~%List./filter"),
    ("list.find", "~%List./find"),
    ("list.flat_map", "~%List./flat_map"),
    ("list.flatten", "~%List./flatten"),
    ("list.fold", "~%List./fold"),
    ("list.group_by", "~%List./group_by"),
    ("list.head", "~%List./head"),
    ("list.intersperse", "~%List./intersperse"),
    ("list.len", "~%List./len"),
    ("list.map", "~%List./map"),
    ("list.max_by", "~%List./max_by"),
    ("list.min_by", "~%List./min_by"),
    ("list.partition", "~%List./partition"),
    ("list.product", "~%List./product"),
    ("list.range", "~%List./range"),
    ("list.reduce", "~%List./reduce"),
    ("list.reverse", "~%List./reverse"),
    ("list.scan", "~%List./scan"),
    ("list.slice", "~%List./slice"),
    ("list.sort", "~%List./sort"),
    ("list.sort_by", "~%List./sort_by"),
    ("list.sum", "~%List./sum"),
    ("list.tail", "~%List./tail"),
    ("list.take", "~%List./take"),
    ("list.take_while", "~%List./take_while"),
    ("list.transpose", "~%List./transpose"),
    ("list.unique", "~%List./unique"),
    ("list.window", "~%List./window"),
    ("list.zip", "~%List./zip"),
    ("list.zip_with", "~%List./zip_with"),
    ("cond.cond", "~%Cond./cond"),
    ("cond.if", "~%Cond./if"),
    ("cond.match", "~%Cond./match"),
    ("disc.advertise", "~%Discovery./advertise"),
    ("disc.connect", "~%Discovery./connect"),
    ("disc.fetch", "~%Discovery./fetch"),
    ("disc.find", "~%Discovery./find"),
    ("disc.identify", "~%Discovery./identify"),
    ("engine.save", "~%Discovery./identify_and_store"),
    ("str.char_at", "~%Str./char_at"),
    ("str.chars", "~%Str./chars"),
    ("str.concat", "~%Str./concat"),
    ("str.contains", "~%Str./contains"),
    ("str.count", "~%Str./count"),
    ("str.decode_uri", "~%Str./decode_uri"),
    ("str.encode_uri", "~%Str./encode_uri"),
    ("str.ends_with", "~%Str./ends_with"),
    ("str.format", "~%Str./format"),
    ("str.from_int", "~%Str./from_int"),
    ("str.index_of", "~%Str./index_of"),
    ("str.is_empty", "~%Str./is_empty"),
    ("str.join", "~%Str./join"),
    ("str.len", "~%Str./len"),
    ("str.levenshtein", "~%Str./levenshtein"),
    ("str.lines", "~%Str./lines"),
    ("str.pad_left", "~%Str./pad_left"),
    ("str.pad_right", "~%Str./pad_right"),
    ("str.parse_float", "~%Str./parse_float"),
    ("str.parse_int", "~%Str./parse_int"),
    ("str.repeat", "~%Str./repeat"),
    ("str.replace", "~%Str./replace"),
    ("str.reverse", "~%Str./reverse"),
    ("str.slice", "~%Str./slice"),
    ("str.split", "~%Str./split"),
    ("str.starts_with", "~%Str./starts_with"),
    ("str.title_case", "~%Str./title_case"),
    ("str.to_lower", "~%Str./to_lower"),
    ("str.to_upper", "~%Str./to_upper"),
    ("str.trim", "~%Str./trim"),
    ("str.trim_end", "~%Str./trim_end"),
    ("str.trim_start", "~%Str./trim_start"),
    ("str.word_count", "~%Str./word_count"),
    ("complex.conj", "~%Complex./conj"),
    ("complex.imag", "~%Complex./imag"),
    ("complex.phase", "~%Complex./phase"),
    ("complex.real", "~%Complex./real"),
    ("refl.bottom_cause", "~%Reflection./bottom_cause"),
    ("refl.delete", "~%Reflection./delete"),
    ("refl.entries", "~%Reflection./entries"),
    ("refl.get", "~%Reflection./get"),
    ("refl.has", "~%Reflection./has"),
    ("refl.is_blur", "~%Reflection./is_blur"),
    ("refl.is_bottom", "~%Reflection./is_bottom"),
    ("refl.is_cocoon", "~%Reflection./is_cocoon"),
    ("refl.is_err", "~%Reflection./is_err"),
    ("refl.is_none", "~%Reflection./is_none"),
    ("refl.is_ok", "~%Reflection./is_ok"),
    ("refl.is_some", "~%Reflection./is_some"),
    ("refl.keys", "~%Reflection./keys"),
    ("refl.set", "~%Reflection./set"),
    ("refl.to_str", "~%Reflection./to_str"),
    ("refl.type_of", "~%Reflection./type_of"),
    ("refl.values", "~%Reflection./values"),
    ("time.add_days", "~%Time./add_days"),
    ("time.add_hours", "~%Time./add_hours"),
    ("time.add_ms", "~%Time./add_ms"),
    ("time.diff", "~%Time./diff"),
    ("time.format", "~%Time./format"),
    ("time.now", "~%Time./now"),
    ("time.parse", "~%Time./parse"),
    ("time.to_iso8601", "~%Time./to_iso8601"),
    ("time.weekday", "~%Time./weekday"),
    ("bytes.at", "~%Bytes./at"),
    ("bytes.base64_decode", "~%Bytes./base64_decode"),
    ("bytes.base64_encode", "~%Bytes./base64_encode"),
    ("bytes.concat", "~%Bytes./concat"),
    ("bytes.from_hex", "~%Bytes./from_hex"),
    ("bytes.from_str", "~%Bytes./from_str"),
    ("bytes.hmac_sha256", "~%Bytes./hmac_sha256"),
    ("bytes.len", "~%Bytes./len"),
    ("bytes.sha256", "~%Bytes./sha256"),
    ("bytes.slice", "~%Bytes./slice"),
    ("bytes.to_hex", "~%Bytes./to_hex"),
    ("bytes.to_str", "~%Bytes./to_str"),
    ("json.get", "~%Json./get"),
    ("json.keys", "~%Json./keys"),
    ("json.parse", "~%Json./parse"),
    ("json.stringify", "~%Json./stringify"),
    ("io.append_file", "~%Io./append_file"),
    ("io.exists", "~%Io./exists"),
    ("io.read_file", "~%Io./read_file"),
    ("io.write_file", "~%Io./write_file"),
    ("env.args", "~%Env./args"),
    ("env.cwd", "~%Env./cwd"),
    ("env.get", "~%Env./get"),
    ("process.exit", "~%Process./exit"),
    ("process.pid", "~%Process./pid"),
    ("path.basename", "~%Path./basename"),
    ("path.dirname", "~%Path./dirname"),
    ("path.extension", "~%Path./extension"),
    ("path.is_absolute", "~%Path./is_absolute"),
    ("path.join", "~%Path./join"),
    ("query.deep_merge", "~%Query./deep_merge"),
    ("query.pluck", "~%Query./pluck"),
    ("query.select", "~%Query./select"),
    ("query.where", "~%Query./where"),
    ("diff.diff", "~%Diff./diff"),
    ("diff.is_compatible", "~%Diff./is_compatible"),
    ("diff.patch", "~%Diff./patch"),
    ("set.contains", "~%Set./contains"),
    ("set.difference", "~%Set./difference"),
    ("set.from_list", "~%Set./from_list"),
    ("set.intersection", "~%Set./intersection"),
    ("set.is_disjoint", "~%Set./is_disjoint"),
    ("set.is_subset", "~%Set./is_subset"),
    ("set.is_superset", "~%Set./is_superset"),
    ("set.union", "~%Set./union"),
    ("stat.histogram", "~%Stat./histogram"),
    ("stat.mean", "~%Stat./mean"),
    ("stat.median", "~%Stat./median"),
    ("stat.percentile", "~%Stat./percentile"),
    ("stat.std_dev", "~%Stat./std_dev"),
    ("stat.variance", "~%Stat./variance"),
    ("csv.parse", "~%Csv./parse"),
    ("csv.parse_with_headers", "~%Csv./parse_with_headers"),
    ("csv.read_csv", "~%Csv./read_csv"),
    ("csv.stringify", "~%Csv./stringify"),
    ("url.decode", "~%Url./decode"),
    ("url.encode", "~%Url./encode"),
    ("url.join", "~%Url./join"),
    ("url.parse", "~%Url./parse"),
    ("url.query_params", "~%Url./query_params"),
    ("toml.parse", "~%Toml./parse"),
    ("toml.stringify", "~%Toml./stringify"),
    ("regex.find", "~%Regex./find"),
    ("regex.match", "~%Regex./match"),
    ("regex.replace", "~%Regex./replace"),
    ("regex.split", "~%Regex./split"),
    ("engine.check_oml", "~%Engine./check_oml"),
    ("engine.differential", "~%Engine./differential_1"),
    ("engine.equivalence_map", "~%Engine./equivalence_map"),
    ("engine.observe", "~%Engine./observe"),
    ("engine.project_down", "~%Engine./project_down"),
    ("engine.project_up", "~%Engine./project_up"),
    ("engine.resolve", "~%Engine./resolve"),
    ("engine.set_strategy", "~%Engine./set_strategy"),
    ("effect.run_pure", "~%Effect./runPure"),
    ("option.and_then", "@option./and_then"),
    ("option.expect", "@option./expect"),
    ("option.filter", "@option./filter"),
    ("option.flatten", "@option./flatten"),
    ("option.or", "@option./or"),
    ("option.unwrap_or", "@option./unwrap_or"),
    ("option.zip", "@option./zip"),
    ("result.and", "@result./and"),
    ("result.and_then", "@result./and_then"),
    ("result.expect", "@result./expect"),
    ("result.flatten", "@result./flatten"),
    ("result.or", "@result./or"),
    ("result.unwrap", "@result./unwrap"),
    ("option.map", "@option.%fmap"),
    ("result.map", "@result.%fmap"),
    ("result.map_err", "@result.%map_err"),
];

