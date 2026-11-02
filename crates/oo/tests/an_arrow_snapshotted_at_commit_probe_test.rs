// An arrow snapshotted at commit.
// Order: nlang-tools/docs/an_arrow_snapshotted_at_commit_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-107.  D120: a first-layer arrow
// whose body reads its own node's names is stored as the node's other such
// fields are — its syntax in the node's frame — not forced into a morphism
// whose `%closure` keeps a snapshot. A later write is a meet on that node
// (Q-105); one file, two evolves and two commits are one program (Q-103).
//
// ── What is wrong (v0.107.0) ────────────────────────────────────────────
//
//   `h: { k: @int, f: (x -> k) }`, then commit `h: { k: 5 }` ⟹ `_.h.f 0`
//   is `@int` (one file, two evolves ⟹ `5`; three roots)
//   committed: `(_.h & { k: 5 }).f 0` ⟹ `@int`
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
        let scratch = nlang_interpreter::ScratchDir::new(&format!("snapshot-{tag}"));
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

/// The reading of `q` and the root after `w1`, `w2` three ways (Q-105).
fn ways(w1: &str, w2: &str, q: &str) -> (Vec<String>, Vec<String>) {
    three_ways(w1, w2, q)
}

/// r1 — An arrow reading its node's names reads the met node: one file, two
/// evolves, two commits — one reading, one root.
#[test]
// AMENDED 2026-11-02 for Q-118 (D126): a closure frame enters identity by its digest; roots that hold one moved.
fn r1_an_arrow_reads_the_met_node_every_way() {
    let mut bad = Vec::new();
    let cases = [
        ("h: { k: @int, f: (x -> k) }", "h: { k: 5 }", "_.h.f 0", "5", "a136524dba4f41c3d05d055ba3d125c892754a373ba1e5392091795c0666a22e"),
        ("h: { k: 1.., f: (x -> k) }", "h: { k: 5 }", "_.h.f 0", "5", "a136524dba4f41c3d05d055ba3d125c892754a373ba1e5392091795c0666a22e"),
        ("h: { k: @int, f: (x -> x + k) }", "h: { k: 5 }", "_.h.f 1", "6", "33f63ffb8d671a04122ae2c2fb7e8c926b2cca353cd1b956d652624290467480"),
        ("h: { k: @int, f: (x -> k), g: f }", "h: { k: 5 }", "_.h.g 0", "5", "c5a6d6be60ad572ecac2addb2d972157bdcf3a7c0708a73fe3df217d71af9cb9"),
        ("h: { k: @int, f: (x -> { v: k }) }", "h: { k: 5 }", "_.h.f 0", "{ v: 5 }", "bd8fda7665e61a8367bb7df32fb6bfe0798fb5716c36cf70f8283b0e9e968ad1"),
        ("h: { k: @int, f: (x -> ^.k) }", "h: { k: 5 }", "_.h.f 0", "5", "f206ad66bcbddcf58f4b5638806671ef9b0542dcd87be3854353b69cbbfe8b94"),
        ("h: { a: { k: @int }, f: (x -> a.k) }", "h: { a: { k: 5 } }", "_.h.f 0", "5", "9f6bcfd7a3a7dbba498ea68a85a01a6d940338fab71cc25fb323fc078cd7b520"),
        ("h: { ~k: @int, f: (x -> k) }", "h: { ~k: 5 }", "_.h.f 0", "5", "843497e6860b5546e5bda1e88d74cce869a4dfdb79a15eac8cb3d229bf560c80"),
        ("h: { k: @int, /f: (x -> k) }", "h: { k: 5 }", "_.h./f 0", "5", "9c5053f135cf79f0c6e73d5aecf6fb13cd6ef3a15269fc3d53afa0d7fe770b81"),
        ("h: { k: @int, ~f: (x -> k), r: f 0 }", "h: { k: 5 }", "_.h.r", "5", "271009ad5799f234a8dfb97bc5421281f3b9d81d096080495c2092fc565e99b6"),
        ("h: { k: @int, f: (x -> k), r: f 0 }", "h: { k: 5 }", "_.h.r", "5", "3d5e0e401c8ad7e001c49e08f57ceae7d16ad34213a39aef7e06ff19bf9a3ea9"),
        ("h: { k: @int, f: (x -> (y -> k)) }", "h: { k: 5 }", "(_.h.f 0) 0", "5", "df6b7102cc0f4311099082b8707f4b813a307d4faca06ab3fe4e4316835cefad"),
        ("h: { k: @int, f: (x -> k) }", "h.k: 5", "_.h.f 0", "5", "a136524dba4f41c3d05d055ba3d125c892754a373ba1e5392091795c0666a22e"),
        ("h: { k: @int, f: (x -> k) }", "h: { k: 5, f: (x -> 5) }", "_.h.f 0", "5", "584f3c4cef777204a8f1775cc83c7e69d3e255764eb44fcd25a15b3a94cad091"),
    ];
    let mut n = 0;
    for (w1, w2, q, want, root) in cases {
        let (got, roots) = ways(w1, w2, q);
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

/// r2 — A committed record met in `oo eval`: its arrows read the met node.
#[test]
fn r2_a_committed_arrow_met_in_eval_reads_the_met_node() {
    let mut bad = Vec::new();
    let src = "h: { k: @int, f: (x -> k), g: f, r: f 0 }";
    let w = Ws::new("r2");
    check(&mut bad, "commit", w.writes(&[src]), "e0 c0");
    for (q, want) in [("(_.h & { k: 5 }).f 0", "5"), ("(_.h & { k: 5 }).g 0", "5"), ("(_.h & { k: 5 }).r", "5"), ("_.h.f 0", "{{ %kind: #type %name: \"int\" }}")] {
        check(&mut bad, q, bottom(&w.eval(q)), want);
    }
    verdict(bad, 5);
}

/// r3 — Three writes grouped five ways: one reading, one root.
#[test]
fn r3_three_writes_every_grouping_one_root() {
    let mut bad = Vec::new();
    let mut n = 0;
    for (a, b, c, q, want) in [
        ("h: { k: @int, f: (x -> k) }", "h: { j: 1 }", "h: { k: 5 }", "_.h.f 0", "5"),
        ("h: { k: @int, f: (x -> k + j) }", "h: { j: 1 }", "h: { k: 5 }", "_.h.f 0", "6"),
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
            let w = Ws::new("r3");
            for commit in commits {
                let srcs: Vec<&str> = commit.iter().map(|s| s.as_str()).collect();
                w.writes(&srcs);
            }
            check(&mut bad, &format!("{a} / {b} / {c}, {label}: {q}"), bottom(&w.eval(q)), want);
            n += 1;
            roots.push(w.root());
        }
        roots.sort();
        roots.dedup();
        check(&mut bad, &format!("{a} / {b} / {c}: one root over five groupings"), roots.len().to_string(), "1");
        n += 1;
    }
    verdict(bad, n);
}

/// r4 — Scale: 400 arrows reading their node's name, met by a later write,
/// as one file and as two commits, read the met node in time.
#[test]
fn r4_many_arrows_read_the_met_node_in_time() {
    let mut bad = Vec::new();
    let big = format!("h: {{ k: @int, {} }}", (0..400).map(|i| format!("f{i}: (x -> x + k + {i})")).collect::<Vec<_>>().join(", "));
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
        check(&mut bad, &format!("{label}: _.h.f399 1"), w.eval("_.h.f399 1"), "405");
        check(&mut bad, &format!("{label}: within 60 s"), (took < Duration::from_secs(60)).to_string(), "true");
    }
    verdict(bad, 6);
}

/// g1 — The other half: what does not read the node reads as before, three
/// ways — an arrow over its argument only, over the universe, one taken from
/// another literal, one a call made, a write that does not touch the name.
#[test]
fn g1_what_does_not_read_the_node_reads_as_before() {
    let mut bad = Vec::new();
    let mut n = 0;
    for (w1, w2, q, want) in [
        ("h: { f: (x -> x + 1) }", "h: { k: 5 }", "_.h.f 1", "2"),
        ("h: { k: @int, f: (x -> x + 1) }", "h: { k: 5 }", "_.h.f 1", "2"),
        ("n: @int\nh: { f: (x -> n) }", "n: 8", "_.h.f 0", "8"),
        ("h: { j: @int, f: { j: @int, m: (x -> j) }.m }", "h: { j: 5 }", "_.h.f 0", "{{ %kind: #type %name: \"int\" }}"),
        ("h: { k: @int, f: (k -> (x -> k)) 7 }", "h: { k: 5 }", "_.h.f 0", "7"),
        ("h: { k: @int, f: (x -> k) }", "h: { j: 1 }", "_.h.f 0", "{{ %kind: #type %name: \"int\" }}"),
        ("h: { s: { k: @int, f: (x -> k) } }", "h: { s: { k: 5 } }", "_.h.s.f 0", "5"),
        ("k: @int\nf: (x -> k)", "k: 5", "_.f 0", "5"),
    ] {
        let (got, roots) = ways(w1, w2, q);
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

/// g2 — Roots that never held an arrow reading its node do not move
/// (v0.107.0).
#[test]
// AMENDED 2026-11-02 for Q-118 (D126): a closure frame enters identity by its digest; roots that hold one moved.
fn g2_roots_without_this_form_do_not_move() {
    let mut bad = Vec::new();
    let cases = [
        ("x: 0", "b74d775816f6b5be6cc152577b7556f6c8fdcc63433b0e31068cc2484a94ae0f"),
        ("f: (x -> x + 1)", "258a1a08adfd0340cdbe103e4f4851bea107a67e298e72d6679badc0595bfd1f"),
        ("h: { f: (x -> x + 1) }", "587570c12af93617cc54d00105f31ed7191d0ea70b8058dfdf61746a158617d2"),
        ("h: { k: @int, f: (x -> x + 1) }", "43904019ffc243d837b95996d89a36f20e1d09b6c3f03e1f9a784087fe6bf862"),
        ("n: @int\nh: { f: (x -> n) }", "7c2a0fa36d329813b7b9816e271baf942ead95955a575373002a5819748df4c5"),
        ("h: { k: @int, r: k }", "609a2afbc64d274b0ca840576660f7b0afff568443cbed4ffb30e78419073c04"),
        ("h: { s: { k: @int, f: (x -> k) } }", "e8fa2a8ac203515190e3e881a06c2ecde204465c7bf9bd590f472f8cd666de55"),
        ("k: @int\nf: (x -> k)", "b4f912b83349d7beb7cab5225f49057640b04d613592ef8982bdd32965952716"),
    ];
    for (src, root) in cases {
        check(&mut bad, src, root_of(src), root);
    }
    verdict(bad, cases.len());
}

/// g3 — An operator read back is a value whose `%closure` captures only its
/// body's free variables (`SPEC_05` §3.3), whether it was stored as syntax
/// in its node or as a value.
#[test]
fn g3_an_operator_read_back_captures_only_its_free_variables() {
    let mut bad = Vec::new();
    let w = Ws::new("g3");
    check(&mut bad, "commit", w.writes(&["h: { k: 5, j: 6, z: 7, f: (x -> x + k), g: (x -> x + 1) }"]), "e0 c0");
    let f = w.eval("_.h.f");
    check(&mut bad, "_.h.f captures k", f.contains("k: 5").to_string(), "true");
    check(&mut bad, "_.h.f does not capture j, z", (!f.contains("j: 6") && !f.contains("z: 7")).to_string(), "true");
    let g = w.eval("_.h.g");
    check(&mut bad, "_.h.g captures nothing", (g.contains("%closure") && !g.contains("k: 5") && !g.contains("j: 6")).to_string(), "true");
    check(&mut bad, "_.h.f 1", w.eval("_.h.f 1"), "6");
    verdict(bad, 5);
}
