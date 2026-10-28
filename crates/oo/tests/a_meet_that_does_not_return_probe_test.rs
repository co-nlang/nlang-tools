// A meet that does not return.
// Order: nlang-tools/docs/a_meet_that_does_not_return_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-110.  SPEC_07 §5.3: two
// morphisms merged are one morphism that satisfies both rules; applied, it
// answers the meet of what each answers (`((x -> 1) & (x -> 2)) 0` is ⊥).
// D105: two morphisms meet as one table. A builtin is a morphism.
//
// ── What is wrong (v0.110.0) ────────────────────────────────────────────
//
//   `(~%Math./add (1)) & (~%Math./add (2))` and `(~%Math./add) &
//   (~%Math./sub)` never return: memory grows ~34 MB/s until the machine
//   runs out. `f: ~%Math./add (1)` then `f: ~%Math./add (2)` aborts evolve
//   and commit (stack overflow). The same shape with an arrow,
//   `(x -> x + 1) & (x -> x + 2)`, answers at once.
//
// Every `oo` here runs under a 1 GB address-space cap and a 10 s kill, so a
// meet that does not return is a red cell, not a dead machine.
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
        let scratch = nlang_interpreter::ScratchDir::new(&format!("meet-{tag}"));
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
    for cause in ["#conflict", "#missing_key", "#no_matching_branch"] {
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

/// r1 — Two morphisms met, at least one a builtin, return; applied, the
/// meet answers the meet of what each answers (SPEC_07 §5.3).
#[test]
fn r1_two_builtins_meet_and_apply_as_both() {
    let mut bad = Vec::new();
    let cells = [
        ("((~%Math./add (1)) & (~%Math./add (2))) 3", "_|_ #conflict"),
        ("((~%Math./add (1)) & (~%Math./add (2))) (0 - 1)", "_|_ #conflict"),
        ("((~%Math./add) & (~%Math./sub)) (1, 2)", "_|_ #conflict"),
        ("((~%Math./add) & (~%Math./mul)) (2, 2)", "4"),
        ("((~%Math./add) & (~%Math./mul)) (1, 2)", "_|_ #conflict"),
        ("((~%Math./mul (0)) & (~%Math./mul (5))) 0", "0"),
        ("((~%Math./add (1)) & (~%Math./add (2)) & (~%Math./add (3))) 0", "_|_ #conflict"),
        ("((~%Math./add) & (~%Math./mul) & (~%Math./pow)) (2, 2)", "4"),
        ("((~%Str./len) & (~%Math./abs)) 3", "_|_ #conflict"),
        ("((~%Str./concat (\"a\")) & (~%Str./concat (\"a\"))) \"b\"", "\"ab\""),
        ("((~%Str./concat (\"a\")) & (~%Str./concat (\"b\"))) \"c\"", "_|_ #conflict"),
        ("({ k: ~%Math./add } & { k: ~%Math./sub }).k (1, 2)", "_|_ #conflict"),
        ("({ k: ~%Math./add } & { k: ~%Math./mul }).k (2, 2)", "4"),
        ("({ @{ _ }: ~%Math./add (1) } & { @{ _ }: ~%Math./add (2) }) 3", "_|_ #conflict"),
        ("({ @{ _ }: ~%Math./add (2) } & { @{ _ }: ~%Math./mul (2) }) 2", "4"),
        ("((~%Math./add (1)) & (x -> x + 1)) 3", "4"),
        ("((~%Math./add (1)) & (x -> x + 2)) 3", "_|_ #conflict"),
        ("((~%Math./abs) & (x -> x)) 3", "3"),
        ("((~%Math./abs) & (x -> x)) (0 - 3)", "_|_ #conflict"),
    ];
    table(&mut bad, &cells);
    verdict(bad, cells.len());
}

/// r2 — `&` is a meet: every order and grouping of the same morphisms is
/// one value, byte for byte as printed.
#[test]
fn r2_any_order_and_grouping_is_one_value() {
    let mut bad = Vec::new();
    let mut n = 0;
    let pairs = [
        ["~%Math./add (1)", "~%Math./add (2)"],
        ["~%Math./add", "~%Math./sub"],
        ["~%Str./len", "~%Math./abs"],
        ["~%Math./add (1)", "(x -> x + 1)"],
        ["~%Math./add (1)", "{ @{ _ }: ~%Math./add (2) }"],
    ];
    let mut srcs = Vec::new();
    for [a, b] in pairs {
        srcs.push(format!("({a}) & ({b})"));
        srcs.push(format!("({b}) & ({a})"));
    }
    let triples = [
        ["~%Math./add (1)", "~%Math./add (2)", "~%Math./add (3)"],
        ["~%Math./add", "~%Math./sub", "(x -> x)"],
    ];
    for [a, b, c] in triples {
        for (x, y, z) in [(a, b, c), (a, c, b), (b, a, c), (b, c, a), (c, a, b), (c, b, a)] {
            srcs.push(format!("(({x}) & ({y})) & ({z})"));
            srcs.push(format!("({x}) & (({y}) & ({z}))"));
        }
    }
    let got = eval_all(&srcs);
    for (i, [a, b]) in pairs.iter().enumerate() {
        let (ab, ba) = (&got[2 * i], &got[2 * i + 1]);
        n += 1;
        if ab == "DID NOT RETURN" || ab != ba {
            bad.push(format!("  ({a}) & ({b})\n    a&b {}\n    b&a {}", ab.chars().take(120).collect::<String>(), ba.chars().take(120).collect::<String>()));
        }
    }
    let base = 2 * pairs.len();
    for (t, [a, b, c]) in triples.iter().enumerate() {
        let all = &got[base + 12 * t..base + 12 * (t + 1)];
        n += 1;
        if all[0] == "DID NOT RETURN" || all.iter().any(|g| g != &all[0]) {
            let distinct: std::collections::BTreeSet<String> = all.iter().map(|g| g.chars().take(100).collect()).collect();
            bad.push(format!("  {a} / {b} / {c}: {} values over 12 orders and groupings: {distinct:?}", distinct.len()));
        }
    }
    verdict(bad, n);
}

/// r3 — Written to one coordinate: one file, two evolves, two commits
/// return, read the same and commit one root.
#[test]
fn r3_three_ways_one_reading_one_root() {
    let mut bad = Vec::new();
    let cells = [
        ("f: ~%Math./add (1)", "f: ~%Math./add (2)", "_.f 3", "_|_ #conflict"),
        ("f: ~%Math./add", "f: ~%Math./mul", "_.f (2, 2)", "4"),
        ("f: ~%Math./add", "f: ~%Math./sub", "_.f (1, 2)", "_|_ #conflict"),
        ("h: { k: ~%Math./add }", "h: { k: ~%Math./sub }", "_.h.k (1, 2)", "_|_ #conflict"),
        ("f: ~%Math./add (1)", "f: (x -> x + 1)", "_.f 3", "4"),
        ("f: { @{ _ }: ~%Math./add (1) }", "f: { @{ _ }: ~%Math./mul (2) }", "_.f 1", "2"),
    ];
    for (w1, w2, q, want) in cells {
        let (got, roots) = three_ways(w1, w2, q);
        let want = [format!("e0 c0 | {want}"), format!("e0 e0 c0 | {want}"), format!("e0 c0 e0 c0 | {want}")];
        for (g, w) in got.iter().zip(want.iter()) {
            check(&mut bad, &format!("{w1} ¦ {w2} ¦ {q}"), g.clone(), w);
        }
        if roots[0].is_empty() || roots.iter().any(|r| r != &roots[0]) {
            bad.push(format!("  {w1} ¦ {w2}: roots {roots:?}"));
        }
    }
    verdict(bad, cells.len() * 4);
}

/// r4 — Every builtin in the registry: met with another builtin, and two
/// of its partial applications met, return.
#[test]
fn r4_every_builtin_meets_and_returns() {
    let mut bad = Vec::new();
    for id in nlang_interpreter::builtins::contract_ids() {
        if !SPELL.iter().any(|(i, _)| *i == id) {
            bad.push(format!("  {id}: in the registry, not spelled in SPELL"));
        }
    }
    let mut srcs = Vec::new();
    for (id, s) in SPELL {
        let other = if *s == "~%Math./add" { "~%Math./sub" } else { "~%Math./add" };
        srcs.push(format!("({s}) & ({other})"));
        // `process.exit (1)` ends the process: its partial is not met here.
        if *id != "process.exit" {
            srcs.push(format!("({s} (1)) & ({s} (2))"));
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

/// g1 — The other half. A morphism met with itself is itself; the value
/// bodies of one branch still meet at once (D117), and a builtin body beside
/// them answers with them; arrow bodies written in two writes commit one
/// root; a morphism met with an atom is ⊥; roots of single writes do not
/// move.
#[test]
fn g1_what_meets_today_meets_the_same() {
    let mut bad = Vec::new();
    let (abs, add1) = (eval_full("~%Math./abs"), eval_full("~%Math./add (1)"));
    let cells = [
        ("((~%Math./add (1)) & (~%Math./add (1))) 3", "4"),
        ("(~%Math./add (1)) & 5", "_|_ #conflict"),
        ("((x -> 1) & (x -> 2)) 0", "_|_ #conflict"),
        ("((x -> x + 1) & (x -> x + 1)) 1", "2"),
        ("({ @{ _ }: 5 } & { @{ _ }: 6 }) 0", "_|_ #conflict"),
        ("({ @{ _ }: 5 } & { @{ _ }: 5 }) 0", "5"),
        ("({ @{ _ }: (x -> x) } & { @{ _ }: (x -> x + 1) }) 3", "_|_ #conflict"),
        ("{ @{ 4.. }: ~%Math./add (1) } 5", "6"),
        ("({ @{ _ }: ~%Math./add (1) } & { @{ _ }: 4 }) 3", "4"),
        ("({ @{ _ }: ~%Math./add (1) } & { @{ _ }: 5 }) 3", "_|_ #conflict"),
        ("({ @{ _ }: ~%Math./add (2) } & { @{ _ }: 4 } & { @{ _ }: ~%Math./mul (2) }) 2", "4"),
        ("({ @{ 1.. }: ~%Math./add (1) } & { @{ _ }: ~%Math./sub (9) }) 0", "9"),
        ("(~%Math./abs) & (~%Math./abs)", abs.as_str()),
        ("(~%Math./add (1)) & (~%Math./add (1))", add1.as_str()),
    ];
    table(&mut bad, &cells);
    let roots = [
        ("f: ~%Math./add (1)", "22780220b61858498d96889cc3e5d2f18a07a99b935e84e8b69d50bb0d15bf73"),
        ("f: (x -> x + 1) & (x -> x + 2)", "dd896359b44a447341a89679e169ae34870d7f7f9e21796e382611e70292afb5"),
        ("t: { @{ _ }: (x -> x) } & { @{ _ }: (x -> x + 1) }", "a56a53125aa9aabb90d466d28f1058b7875e7a6a0140c5a0bbb507d69fe2c2f5"),
        ("t: { @{ _ }: 5 } & { @{ _ }: 6 }", "2f4c3fbf504a717d00bb1a3a31b9ba8a80d82ebb776db67ead0b569a7f037a0f"),
        ("f: (~%Math./add (1)) & (x -> x + 1)", "2b1eb0c6aec6ed627d5f4324db34ee95aaa333b7dc066cc99ac80d7d99349c54"),
        ("f: (~%Str./len) & (~%Str./len)", "28931d9d9302af33c9d85a62516564b35cf27f4958ff7675fc38fb455882132c"),
        ("t: { @{ 1.. }: ~%Math./add (1) } & { @{ _ }: (x -> x) }", "e33ca7887ada518890c0e743171ae9ac95786dc271c9e550888363767af75433"),
    ];
    for (src, root) in roots {
        check(&mut bad, src, root_of(src), root);
    }
    // Arrow bodies of one branch still meet at once: written in two writes
    // they commit one root, whichever way they are written.
    let (got, roots3) = three_ways("t: { @{ _ }: (x -> x) }", "t: { @{ _ }: (x -> x + 1) }", "_.t 3");
    for (g, w) in got.iter().zip(["e0 c0 | _|_ #conflict", "e0 e0 c0 | _|_ #conflict", "e0 c0 e0 c0 | _|_ #conflict"]) {
        check(&mut bad, "t: { @{ _ }: (x -> x) } ¦ t: { @{ _ }: (x -> x + 1) } ¦ _.t 3", g.clone(), w);
    }
    if roots3.iter().any(|r| r.is_empty() || r != &roots3[0]) {
        bad.push(format!("  t: {{ @{{ _ }}: (x -> x) }} ¦ t: {{ @{{ _ }}: (x -> x + 1) }}: roots {roots3:?}"));
    }
    verdict(bad, cells.len() + roots.len() + 4);
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
