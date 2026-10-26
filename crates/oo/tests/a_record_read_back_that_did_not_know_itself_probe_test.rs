// A record read back that did not know itself.
// Order: nlang-tools/docs/a_record_read_back_that_did_not_know_itself_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-105.  D118: a name resolves in
// the node it is written in, as that node is after meets, wherever it is
// forced. A later commit to a coordinate is a meet on that node; one file,
// two evolves and two commits are one program (Q-103).
//
// ── What is wrong (v0.105.0) ────────────────────────────────────────────
//
// A record read back from the store or the working set does not know it is
// the node its fields were written in, so a later meet does not reach them:
//   `h: { k: @int, r: k }`, then `h: { k: 5 }`  ⟹ `_.h.r` is `@int`
//   (one file ⟹ `5`; two evolves and two commits share another root)
//   committed `h: { k: @int, r: k }`: `(_.h & { k: 5 }).r` ⟹ `@int`
//   committed `r: { z: y }`:          `(_.r & { y: 6 }).z` ⟹ `_`
//
// The delivery may NOT edit this file. `rustfmt` must not touch it.


#![allow(dead_code)]

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

struct Ws {
    _scratch: nlang_interpreter::ScratchDir,
    root: PathBuf,
    ws: PathBuf,
}

impl Ws {
    fn new(tag: &str) -> Self {
        let scratch = nlang_interpreter::ScratchDir::new(&format!("read-back-{tag}"));
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

    /// Copy `other`'s working-set members in: two writes that did not see
    /// each other, as two concurrent evolves leave them.
    fn take_members_of(&self, other: &Ws) {
        let from = other.ws.join(".oo").join("injections");
        let to = self.ws.join(".oo").join("injections");
        for e in fs::read_dir(&from).unwrap() {
            let p = e.unwrap().path();
            fs::copy(&p, to.join(p.file_name().unwrap())).unwrap();
        }
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

/// `oo eval` from an empty directory (no universe), a ⊥ reduced to its cause.
fn eval_here(e: &str) -> String {
    let w = Ws::new("eval");
    bottom(&w.eval(e))
}

fn bottom(got: &str) -> String {
    for cause in ["#conflict", "#no_matching_branch"] {
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

fn table(bad: &mut Vec<String>, cells: &[(&str, &str)]) {
    for (e, want) in cells {
        check(bad, e, eval_here(e), want);
    }
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

/// r1 — A later write to a record is a meet on its node: its fields read
/// the met node, written as one file, as two evolves, as two commits — one
/// reading, one root. (The root of a meet in place moves from v0.105.0: a
/// met node's view no longer holds the frames it stands for — r4.)
#[test]
fn r1_a_later_write_meets_the_node_read_back() {
    let mut bad = Vec::new();
    let cases = [
        ("h: { k: @int, r: k }", "h: { k: 5 }", "_.h.r", "5", "2fc429bb463cdf2b1f922860c854f01f97f070d4783f16944093b80c4c0d0468"),
        ("h: { k: @int, r: k }", "h.k: 5", "_.h.r", "5", "2fc429bb463cdf2b1f922860c854f01f97f070d4783f16944093b80c4c0d0468"),
        ("h: { k: @int, r: [k] }", "h: { k: 5 }", "_.h.r", "[5]", "44536ebc5d892c3c22056e6ba8aa8692b3d2e40f5afe1bb44c4f6fecb0ea72d7"),
        ("h: { k: @int, t: { u: k } }", "h: { k: 5 }", "_.h.t.u", "5", "3d9dc42715633d9674d45308558f3b40955fdb1c0ecf3d9fc77f5cbfcfbc700c"),
        ("h: { k: @int, s: { r: ^.k } }", "h: { k: 5 }", "_.h.s.r", "5", "d5c0d912a537d186729ac0725a6900179f65321120f02f1711aeea613ac0bae3"),
        ("h: { ~k: @int, r: k }", "h: { ~k: 5 }", "_.h.r", "5", "c1a2e99ca7e88df0f88ad53b1d64608fd6a0b938e873a117c563accee6fa4142"),
        ("h: { k: @int, r: k | \"s\" }", "h: { k: 5 }", "_.h.r", "5 | \"s\"", "8e5dcd2e421e1bd551d6110f9d8f6157e1244ff9ccfe5e0872a44a6ac1c1d7ca"),
        ("h: { k: @int, r: k, z: r }", "h: { k: 5 }", "_.h.z", "5", "7149c2c1ea151981460a9caa2d720f70cdb29d15de04e313502e11d82f6ab880"),
        ("h: { a: { k: @int }, r: a.k }", "h: { a: { k: 5 } }", "_.h.r", "5", "2afbf965acbef2ff970efec9934646cd58bb701a3117527cdb4d3cf814df8d28"),
    ];
    let mut n = 0;
    for (w1, w2, q, want, root) in cases {
        let (got, roots) = three_ways(w1, w2, q);
        for (way, g) in ["one file", "two evolves", "two commits"].iter().zip(got) {
            let rc = if *way == "two evolves" { "e0 e0 c0" } else if *way == "one file" { "e0 c0" } else { "e0 c0 e0 c0" };
            check(&mut bad, &format!("{w1} / {w2}, {way}: {q}"), g, &format!("{rc} | {want}"));
            n += 1;
        }
        for (way, r) in ["one file", "two evolves", "two commits"].iter().zip(roots) {
            check(&mut bad, &format!("{w1} / {w2}, {way}: root"), r, root);
            n += 1;
        }
    }
    verdict(bad, n);
}

/// r2 — A committed record met in `oo eval` reads the met node: a name its
/// own literal has, one it did not have, a nested record, a reference.
#[test]
fn r2_a_committed_record_met_in_eval_reads_the_met_node() {
    let mut bad = Vec::new();
    let cases = [
        ("h: { k: @int, r: k }", "(_.h & { k: 5 }).r", "5"),
        ("h: { k: @int, r: k }", "(_.h & { k: 5 }) & { k: 5 }", "{ k: 5 r: 5 }"),
        ("h: { k: @int, t: { u: k } }", "(_.h & { k: 5 }).t.u", "5"),
        ("r: { z: y }", "(_.r & { y: 6 }).z", "6"),
        ("h: { r: <<k>> }", "(_.h & { k: 7 }).r", "7"),
        ("h: { k: @int, s: { r: ^.k } }", "(_.h & { k: 5 }).s.r", "5"),
    ];
    for (src, q, want) in cases {
        let w = Ws::new("r2");
        check(&mut bad, &format!("{src}: commit"), w.writes(&[src]), "e0 c0");
        check(&mut bad, &format!("{src}: {q}"), bottom(&w.eval(q)), want);
    }
    verdict(bad, cases.len() * 2);
}

/// r3 — Three writes to one record, grouped five ways: every grouping reads
/// the met node, and the same grouping written again gives the same root
/// (whatever order the working set's members sort in).
#[test]
fn r3_three_writes_every_grouping_reads_the_met_node() {
    let mut bad = Vec::new();
    let (a, b, c) = ("h: { k: @int, r: k }", "h: { j: 1 }", "h: { k: 5 }");
    let groupings: Vec<(&str, Vec<Vec<String>>)> = vec![
        ("one file", vec![vec![format!("{a}\n{b}\n{c}")]]),
        ("three evolves", vec![vec![a.into(), b.into(), c.into()]]),
        ("three commits", vec![vec![a.into()], vec![b.into()], vec![c.into()]]),
        ("one, then two", vec![vec![a.into()], vec![format!("{b}\n{c}")]]),
        ("two, then one", vec![vec![format!("{a}\n{b}")], vec![c.into()]]),
    ];
    let mut n = 0;
    for (label, commits) in &groupings {
        let mut roots = Vec::new();
        for _ in 0..4 {
            let w = Ws::new("r3");
            for commit in commits {
                let srcs: Vec<&str> = commit.iter().map(|s| s.as_str()).collect();
                w.writes(&srcs);
            }
            if roots.is_empty() {
                check(&mut bad, &format!("{label}: _.h.r"), w.eval("_.h.r"), "5");
                check(&mut bad, &format!("{label}: _.h"), w.eval("_.h"), "{ j: 1 k: 5 r: 5 }");
                n += 2;
            }
            roots.push(w.root());
        }
        roots.sort();
        roots.dedup();
        check(&mut bad, &format!("{label}: one root over four runs"), roots.len().to_string(), "1");
        n += 1;
    }
    verdict(bad, n);
}

/// r4 — Scale: a record of 400 fields reading its own name, met by a later
/// write, as one file and as two commits, reads the met node in time
/// (v0.105.0: one file > 300 s and 15 GB; two commits 3 s, reading `@int`).
#[test]
fn r4_a_large_record_met_reads_the_met_node_in_time() {
    let mut bad = Vec::new();
    let big = format!("h: {{ k: @int, {} }}", (0..400).map(|i| format!("f{i}: k")).collect::<Vec<_>>().join(", "));
    for (label, ways) in [
        ("one file", vec![vec![format!("{big}\nh: {{ k: 5 }}")]]),
        ("two commits", vec![vec![big.clone()], vec!["h: { k: 5 }".to_string()]]),
    ] {
        let w = Ws::new("r4");
        let start = Instant::now();
        let mut rc = Vec::new();
        for commit in &ways {
            let srcs: Vec<&str> = commit.iter().map(|s| s.as_str()).collect();
            rc.push(w.writes(&srcs));
        }
        let took = start.elapsed();
        check(&mut bad, &format!("{label}: writes"), rc.join(" "), if ways.len() == 1 { "e0 c0" } else { "e0 c0 e0 c0" });
        check(&mut bad, &format!("{label}: _.h.f399"), w.eval("_.h.f399"), "5");
        check(&mut bad, &format!("{label}: within 60 s"), (took < Duration::from_secs(60)).to_string(), "true");
    }
    verdict(bad, 6);
}

/// r5 — (R-1) A record read back that holds a meet waiting for the node (a
/// field written on both sides of one literal's meet, or twice in it): a
/// later write meets it there — one reading, one root, three ways.
#[test]
fn r5_a_meet_left_for_the_node_read_back() {
    let mut bad = Vec::new();
    let cases = [
        ("h: { k: @int, r: k } & { r: @int }", "h: { k: 5 }", "_.h.r", "5", "a218d7ccd07c2b72485b025d7fe4804ba2ae2f497243b50190859f8a5934a112"),
        ("h: { k: @int, r: k } & { r: @int }", "h.k: 5", "_.h.r", "5", "a218d7ccd07c2b72485b025d7fe4804ba2ae2f497243b50190859f8a5934a112"),
        ("h: { k: @int, r: k, r: @int }", "h: { k: 5 }", "_.h.r", "5", "22c9eea3230cdcb23c99885864e0679cbf0e86b7baaa0aa4dce28d230f3c1796"),
        ("h: { k: @int, r: [k] } & { r: [@int] }", "h: { k: 5 }", "_.h.r", "[5]", "e2cebad690e753f40fc791f4afbd2dd906c5e32cc29985bb948e256afc8b43b1"),
        ("h: { k: @int, r: k } & { k: @int, r: @int }", "h: { k: 5 }", "_.h.r", "5", "0a34b1f2b05c7c380bf097b93039d8cd6425f957314f54a505ef444a160bebda"),
        ("h: { k: @int, s: { r: k } & { r: @int } }", "h: { k: 5 }", "_.h.s.r", "5", "39793070c6811e2ec786573078b7d7347ff500202247cf6d4b8df4d396e6e0d6"),
    ];
    let mut n = 0;
    for (w1, w2, q, want, root) in cases {
        let (got, roots) = three_ways(w1, w2, q);
        for (way, g) in ["one file", "two evolves", "two commits"].iter().zip(got) {
            let rc = if *way == "two evolves" { "e0 e0 c0" } else if *way == "one file" { "e0 c0" } else { "e0 c0 e0 c0" };
            check(&mut bad, &format!("{w1} / {w2}, {way}: {q}"), g, &format!("{rc} | {want}"));
            n += 1;
        }
        for (way, r) in ["one file", "two evolves", "two commits"].iter().zip(roots) {
            check(&mut bad, &format!("{w1} / {w2}, {way}: root"), r, root);
            n += 1;
        }
    }
    let w = Ws::new("r5-eval");
    check(&mut bad, "committed, then met in eval: commit", w.writes(&["h: { k: @int, r: k } & { r: @int }"]), "e0 c0");
    check(&mut bad, "committed, then met in eval: (_.h & { k: 5 }).r", w.eval("(_.h & { k: 5 }).r"), "5");
    verdict(bad, n + 2);
}

/// r6 — (R-1) Scale: 50 fields each written on both sides of one literal's
/// meet, met by a later write, as one file and as two commits, reads the met
/// node in time (v0.105.0: one file 91 s and 14.8 GB; two commits 4.9 s,
/// reading `@int`; the first delivery: two commits 51 s and 14.8 GB).
#[test]
fn r6_a_large_record_with_meets_left_reads_the_met_node_in_time() {
    let mut bad = Vec::new();
    let left = (0..50).map(|i| format!("f{i}: k")).collect::<Vec<_>>().join(", ");
    let right = (0..50).map(|i| format!("f{i}: @int")).collect::<Vec<_>>().join(", ");
    let big = format!("h: {{ k: @int, {left} }} & {{ {right} }}");
    for (label, ways) in [
        ("one file", vec![vec![format!("{big}\nh: {{ k: 5 }}")]]),
        ("two commits", vec![vec![big.clone()], vec!["h: { k: 5 }".to_string()]]),
    ] {
        let w = Ws::new("r6");
        let start = Instant::now();
        let mut rc = Vec::new();
        for commit in &ways {
            let srcs: Vec<&str> = commit.iter().map(|s| s.as_str()).collect();
            rc.push(w.writes(&srcs));
        }
        let took = start.elapsed();
        check(&mut bad, &format!("{label}: writes"), rc.join(" "), if ways.len() == 1 { "e0 c0" } else { "e0 c0 e0 c0" });
        check(&mut bad, &format!("{label}: _.h.f49"), w.eval("_.h.f49"), "5");
        check(&mut bad, &format!("{label}: within 60 s"), (took < Duration::from_secs(60)).to_string(), "true");
    }
    verdict(bad, 6);
}

/// r7 — (R-2) A later write that also writes a field the node has, read by
/// the node's names, meets it in the met node — in the universe's own meet
/// too (one file, two evolves), not in the frames before the meet: a
/// collision is refused at evolve every way (as on the root), a conflict a
/// meet makes reads `_|_` every way, and every way reads the met node.
#[test]
fn r7_a_later_write_to_a_field_meets_in_the_met_node() {
    let mut bad = Vec::new();
    let mut n = 0;
    for (w1, w2, q, want) in [
        ("h: { k: @int, r: k }", "h: { k: 5, r: 6 }", "_.h.r", ["e1 c1 | _", "e0 e1 c0 | {{ %kind: #type %name: \"int\" }}", "e0 c0 e1 c1 | {{ %kind: #type %name: \"int\" }}"]),
        ("h: { k: @int, r: k } & { r: @int }", "h: { k: 5, r: 6 }", "_.h.r", ["e1 c1 | _", "e0 e1 c0 | {{ %kind: #type %name: \"int\" }}", "e0 c0 e1 c1 | {{ %kind: #type %name: \"int\" }}"]),
        ("k: @int\nr: k", "k: 5\nr: 6", "_.r", ["e1 c1 | _", "e0 e1 c0 | {{ %kind: #type %name: \"int\" }}", "e0 c0 e1 c1 | {{ %kind: #type %name: \"int\" }}"]),
    ] {
        let (got, _) = three_ways(w1, w2, q);
        for ((way, g), w) in ["one file", "two evolves", "two commits"].iter().zip(got).zip(want) {
            check(&mut bad, &format!("{w1} / {w2}, {way}: {q}"), g, w);
            n += 1;
        }
    }
    for (w1, w2, q, want, root) in [
        ("h: { k: @int, r: k }\nh: { r: 6 }", "h: { k: 5 }", "_.h.r", "_|_ #conflict", "61b0ca48ad9bf0a397d86660a89884597bd132caa5fec56756311a3326829ed5"),
        ("h: { k: @int, r: k }", "h: { r: 6 }", "_.h.r", "6", "42b704016598e2393bef30aa0beaaaabe8e2de9d6bfe8fcbad4dccfb07abcdea"),
        ("h: { k: @int, s: { j: @int, r: k + 0, q: j } }", "h: { k: 5, s: { j: 6 } }", "[_.h.s.r, _.h.s.q]", "[5, 6]", "25f13cb104d9a99e8bdab6a95deca3d900c05988f551869975df7ed48bc7060d"),
        // R-3: one meet `k & 1..`, not `(k & @int) & 1..` (r8).
        ("h: { k: @int, r: k } & { r: @int }\nh: { r: 1.. }", "h: { k: 5 }", "_.h.r", "5", "e2f0ce7e4a539510575048cab53ee08aef732468270351813c1758bc5ec571fc"),
        ("r: { p: @int, s: p }", "r: { p: 5, s: @int }", "_.r", "{ p: 5 s: 5 }", "b1796986895c0655587e95d21688d56b4cd1f7f762d4d8037dc2270a8c1a1d5a"),
    ] {
        let (got, roots) = three_ways(w1, w2, q);
        for (way, g) in ["one file", "two evolves", "two commits"].iter().zip(got) {
            let rc = if *way == "two evolves" { "e0 e0 c0" } else if *way == "one file" { "e0 c0" } else { "e0 c0 e0 c0" };
            check(&mut bad, &format!("{w1} / {w2}, {way}: {q}"), g, &format!("{rc} | {want}"));
            n += 1;
        }
        for (way, r) in ["one file", "two evolves", "two commits"].iter().zip(roots) {
            check(&mut bad, &format!("{w1} / {w2}, {way}: root"), r, root);
            n += 1;
        }
    }
    verdict(bad, n);
}

/// r8 — A meet left for the node is one value, however it is spelled and
/// however the writes are grouped (R-3): a literal spread into a record and
/// written alike is not the record (its names stay its own); a field that
/// reads the node through `<<k>>` collides at evolve every way, as `k`
/// does; three writes — the middle one not reading a name — commit one root
/// in every grouping, not one per grouping (R-2 left `(k & @int) & 5` in one
/// grouping and `k & 5` in another, and a view of the met node holding a
/// lazy `1` in one and the built `1` in another).
#[test]
fn r8_a_meet_left_for_the_node_is_one_value_every_way() {
    let mut bad = Vec::new();
    let mut n = 0;
    let int = "{{ %kind: #type %name: \"int\" }}";
    for (w1, w2, q, want) in [
        ("h: { ...{ k: @int, r: k } }", "h: { k: 5, r: 6 }", "_.h.r", ["e0 c0 | 6".to_string(), "e0 e0 c0 | 6".into(), "e0 c0 e0 c0 | 6".into()]),
        ("h: { ...{ k: @int, r: k } }", "h: { r: 6 }", "_.h.r", ["e0 c0 | 6".to_string(), "e0 e0 c0 | 6".into(), "e0 c0 e0 c0 | 6".into()]),
        ("h: { k: @int, r: <<k>> }", "h: { k: 5, r: 6 }", "_.h.r", ["e1 c1 | _".to_string(), format!("e0 e1 c0 | {int}"), format!("e0 c0 e1 c1 | {int}")]),
    ] {
        let (got, roots) = three_ways(w1, w2, q);
        for ((way, g), w) in ["one file", "two evolves", "two commits"].iter().zip(got).zip(want.iter()) {
            check(&mut bad, &format!("{w1} / {w2}, {way}: {q}"), g, w);
            n += 1;
        }
        let first = if want[0].starts_with("e1") { 1 } else { 0 };
        for (way, r) in ["one file", "two evolves", "two commits"].iter().zip(roots.iter()).skip(first + 1) {
            check(&mut bad, &format!("{w1} / {w2}, {way}: root as {}", ["one file", "two evolves"][first]), r.clone(), &roots[first]);
            n += 1;
        }
    }
    for (a, b, c) in [
        ("h: { k: @int, r: k }", "h: { j: 1 }", "h: { k: 5 }"),
        ("h: { k: @int, r: k }", "h: { r: @int, j: 1 }", "h: { k: 5, r: 5 }"),
        ("h: { k: @int, r: k } & { r: @int }", "h: { j: 1 }", "h: { k: 5 }"),
    ] {
        let groupings: Vec<(&str, Vec<Vec<String>>)> = vec![
            ("one file", vec![vec![format!("{a}\n{b}\n{c}")]]),
            ("three evolves", vec![vec![a.into(), b.into(), c.into()]]),
            ("three commits", vec![vec![a.into()], vec![b.into()], vec![c.into()]]),
            ("one, then two", vec![vec![a.into()], vec![format!("{b}\n{c}")]]),
            ("two, then one", vec![vec![format!("{a}\n{b}")], vec![c.into()]]),
        ];
        let mut roots = Vec::new();
        for (label, commits) in &groupings {
            let w = Ws::new("r8");
            for commit in commits {
                let srcs: Vec<&str> = commit.iter().map(|s| s.as_str()).collect();
                w.writes(&srcs);
            }
            check(&mut bad, &format!("{a} / {b} / {c}, {label}: _.h"), w.eval("_.h"), "{ j: 1 k: 5 r: 5 }");
            n += 1;
            roots.push((label, w.root()));
        }
        for (label, r) in &roots[1..] {
            check(&mut bad, &format!("{a} / {b} / {c}, {label}: root as one file"), r.clone(), &roots[0].1);
            n += 1;
        }
    }
    verdict(bad, n);
}

/// g1 — What a later write does not reach reads as before (green today): a
/// write that does not touch the name, a nested literal with its own name,
/// a spread source's names (Q-078 g2), the root, a name the record did not
/// have; and what is out of scope (§6) reads as today: a first-layer arrow
/// captured as values, a bottom the first commit wrote.
#[test]
fn g1_what_a_later_write_does_not_reach_reads_as_before() {
    let mut bad = Vec::new();
    let mut n = 0;
    for (writes, q, want) in [
        (vec!["h: { k: @int, r: k }", "h: { j: 1 }"], "_.h.r", "{{ %kind: #type %name: \"int\" }}"),
        (vec!["h: { k: @int, s: { k: 7, r: k } }", "h: { k: 5 }"], "_.h.s.r", "7"),
        (vec!["h: { k: 2, ...{ k: 1, q: k } }"], "_.h.q", "1"),
        (vec!["h: { k: 2, ...{ k: 1, q: k } }"], "(_.h & { j: 0 }).q", "1"),
        (vec!["k: @int\nr: k", "k: 5"], "_.r", "5"),
        (vec!["h: { r: k }", "h: { k: 5 }"], "_.h.r", "5"),
        (vec!["h: { s: { k: @int, r: k } }", "h: { s: { k: 5 } }"], "_.h.s.r", "5"),
        (vec!["h: { k: @int, r: <<k>> }", "h: { k: 5 }"], "_.h.r", "5"),
        (vec!["h: { k: @int, f: (x -> k) }", "h: { k: 5 }"], "_.h.f 0", "{{ %kind: #type %name: \"int\" }}"),
        (vec!["h: { k: @int, r: k + 1 }", "h: { k: 5 }"], "_.h.r", "_|_ #conflict"),
        (vec!["n: @int\nr: n + 1", "n: 5"], "_.r", "_|_ #conflict"),
    ] {
        let w = Ws::new("g1");
        for s in &writes {
            w.writes(&[s]);
        }
        check(&mut bad, &format!("{writes:?}: {q}"), bottom(&w.eval(q)), want);
        n += 1;
    }
    verdict(bad, n);
}

/// g2 — Roots of a single write do not move (v0.105.0).
#[test]
fn g2_roots_of_a_single_write_do_not_move() {
    let mut bad = Vec::new();
    let cases = [
        ("x: 0", "b74d775816f6b5be6cc152577b7556f6c8fdcc63433b0e31068cc2484a94ae0f"),
        ("h: { k: @int, r: k }", "9247b8a31920ff61cd5ae14c35c682bdf0d372d28c68e53d5036a6b80de3043e"),
        ("h: { k: 5, r: k }", "9c089093d7b40ed4d49f159b90814536abaf32aaa97a7211c331e099968e702b"),
        ("h: { k: @int, t: { u: k } }", "36cf200b438e0c649edd80d5e796d39a5c0904d39eeb3de78dd75e14ecc552c1"),
        ("h: { k: @int, s: { r: ^.k } }", "a9ed1582f63b53eb6562c2447044a95838923556efe57f9cccecd8ba1fc82465"),
        ("h: { ~k: @int, r: k }", "fcf9368961f1e22ab586d25e5fd1e3478cdc7f18d70501e6b0f1e1de18989116"),
        ("h: { k: 2, ...{ k: 1, q: k } }", "2e6e0abd971a14bee5cf17cabea04bff2925c6d6240044ad7fabe7735183b585"),
        ("r: { z: y }", "5012d76dd9312a53bdfd6f1ee604f0d47276fe650ba6bfcaa4e401f5d576eb64"),
    ];
    for (src, root) in cases {
        check(&mut bad, src, root_of(src), root);
    }
    verdict(bad, cases.len());
}
