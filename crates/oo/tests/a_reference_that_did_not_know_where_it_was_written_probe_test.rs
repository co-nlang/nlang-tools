// A reference that did not know where it was written.
// Order: nlang-tools/docs/a_reference_that_did_not_know_where_it_was_written_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-104.  D118: a name resolves in
// the node it is written in, as that node is after meets, wherever it is
// forced. SPEC_04 §1.2: `<<path>>` differs from a name in that it does not
// collapse and sees later commits — not in where it binds.
//
// ── What is wrong (v0.104.0) ────────────────────────────────────────────
//
// A reference's names resolve where it is observed, falling back to the
// root — the dynamic scope D118 removed for every other name:
//   `{ k: 3, r: <<k>> }.r`  ⟹ `_`        `{ k: 3, s: { r: <<^.k>> } }.s.r`  ⟹ ⊥
//   `k: 1`, `h: { k: 5, r: <<k>> }` committed ⟹ `_.h.r` is `1`
// A reference in a record does not see a later commit to that record:
// `h: { k: @int, r: <<k>> }`, then `h: { k: 5 }` ⟹ `_.h.r` is `_`.
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
        let scratch = nlang_interpreter::ScratchDir::new(&format!("ref-written-{tag}"));
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

/// r1 — A meet's fields read the met node: one way round and the other, a
/// name only the other side has, nested, computed, through a closure, a
/// path, a reader that has the name too, three ways, a meet of meets, a

/// r1 — A reference's names resolve where it is written: a sibling, a
/// nested literal, `^`, a path, arithmetic through it, a closure, a caller
/// with the same name, a name holding it, an arrow's parameter.
#[test]
fn r1_a_reference_resolves_where_it_is_written() {
    let mut bad = Vec::new();
    let cells = [
        ("{ k: 3, r: <<k>> }.r", "3"),
        ("{ k: 3, r: <<k>> }.r + 1", "4"),
        ("{ k: 3, s: { r: <<k>> } }.s.r", "3"),
        ("{ k: 3, s: { r: <<^.k>> } }.s.r", "3"),
        ("{ s: { k: 3 }, r: <<s.k>> }.r", "3"),
        ("{ k: 3, f: (x -> <<k>>), r: f 0 }.r", "3"),
        ("{ k: 3, f: (x -> <<k>>), r: { k: 9, z: f 0 }.z }.r", "3"),
        ("{ k: 3, r: <<k>>, z: r }.z", "3"),
        ("(y -> <<y>>) { a: 7 }", "{ a: 7 }"),
    ];
    table(&mut bad, &cells);
    verdict(bad, cells.len());
}

/// r2 — A reference reads its node as it is after the meet (Q-103).
#[test]
fn r2_a_reference_reads_the_node_after_the_meet() {
    let mut bad = Vec::new();
    let cells = [
        ("({ r: <<k>> } & { k: 3 }).r", "3"),
        ("({ k: @int, r: <<k>> } & { k: 3 }).r", "3"),
        ("({ k: 3 } & { r: <<k>> + 1 }).r", "4"),
    ];
    table(&mut bad, &cells);
    verdict(bad, cells.len());
}

/// r3 — Committed, a reference reads the node it was written in: with the
/// same name on the root, nested, `^`, through an alias, printed whole.
#[test]
fn r3_a_committed_reference_reads_its_node() {
    let mut bad = Vec::new();
    let cases = [
        ("h: { k: 5, r: <<k>> }", "_.h.r", "5"),
        ("h: { k: 5, r: <<k>> }", "_.h", "{ k: 5 r: 5 }"),
        ("k: 1\nh: { k: 5, r: <<k>> }", "_.h.r", "5"),
        ("k: 1\nh: { k: 5, r: <<k>> }", "_.h.r + 1", "6"),
        ("h: { k: 5, s: { r: <<k>> } }", "_.h.s.r", "5"),
        ("h: { k: 5, s: { r: <<^.k>> } }", "_.h.s.r", "5"),
        ("h: { k: 5, r: <<k>> }\ng: h", "_.g.r", "5"),
        ("h: { r: <<k>> } & { k: 5 }", "_.h.r", "5"),
        ("k: 1\nh: { k: 5, f: (x -> <<k>>) }", "_.h.f 0", "5"),
    ];
    for (src, q, want) in cases {
        let w = Ws::new("r3");
        check(&mut bad, &format!("{src}: commit"), w.writes(&[src]), "e0 c0");
        check(&mut bad, &format!("{src}: {q}"), bottom(&w.eval(q)), want);
    }
    verdict(bad, cases.len() * 2);
}

/// r4 — A later commit is a meet on the node: a reference in it sees the
/// later write, in one file, over two evolves and over two commits, with
/// one root (as v0.104.0 had, reading `_`).
#[test]
fn r4_a_reference_sees_a_later_commit_to_its_node() {
    let mut bad = Vec::new();
    let mut roots = Vec::new();
    for (label, writes) in [
        ("one file", vec!["h: { k: @int, r: <<k>> }\nh: { k: 5 }"]),
        ("two evolves", vec!["h: { k: @int, r: <<k>> }", "h: { k: 5 }"]),
    ] {
        let w = Ws::new("r4");
        let want = format!("{} c0", vec!["e0"; writes.len()].join(" "));
        check(&mut bad, &format!("{label}: commit"), w.writes(&writes), &want);
        check(&mut bad, &format!("{label}: _.h.r"), w.eval("_.h.r"), "5");
        roots.push(w.root());
    }
    let w = Ws::new("r4-two");
    check(&mut bad, "two commits: first", w.writes(&["h: { k: @int, r: <<k>> }"]), "e0 c0");
    check(&mut bad, "two commits: second", w.writes(&["h: { k: 5 }"]), "e0 c0");
    check(&mut bad, "two commits: _.h.r", w.eval("_.h.r"), "5");
    check(&mut bad, "two commits: _.h", w.eval("_.h"), "{ k: 5 r: 5 }");
    roots.push(w.root());
    for r in &roots {
        check(&mut bad, "root", r.clone(), "006795a829d98b4c4d5fcfd0cf7a66b2f8c823289276d62652248049c68ba09a");
    }
    let w = Ws::new("r4-open");
    check(&mut bad, "open: first", w.writes(&["h: { r: <<k>> }"]), "e0 c0");
    check(&mut bad, "open: second", w.writes(&["h: { k: 5 }"]), "e0 c0");
    check(&mut bad, "open: _.h.r", w.eval("_.h.r"), "5");
    verdict(bad, 4 + 4 + 3 + 3);
}

/// g1 — What reads the root reads as before (green today): a top-level
/// reference, a root path, a reference through a pipe (L2-126/130/131),
/// a live reference on the root; a reference that left its node — returned
/// by an arrow into another coordinate, or naming a parameter — is stored
/// where it lands and read there, as today (out of scope, §6).
#[test]
fn g1_what_reads_the_root_reads_as_before() {
    let mut bad = Vec::new();
    let cells = [
        ("{ k: 3, r: <<_.k>> }.r", "_"),
        ("{ r: <<k>> }.r", "_"),
    ];
    table(&mut bad, &cells);
    let mut n = cells.len();
    for (writes, q, want) in [
        (vec!["t: { a: 1 }\nc: { r: <<t>> }"], "c.r.a", "1"),
        (vec!["t: { a: $.b + 1 }\nc: { r: <<t>> }"], "{ b: 1 } |> { z: c.r.a }", "{ b: 1 z: 2 }"),
        (vec!["t: { a: $.b + 1 }"], "({ b: 1 } |> { inner: <<t>> }).inner.a", "_|_ ;; %cause: #no_context ;; free `$` observed without an enclosing evolution (P3)"),
        (vec!["k: @int\nr: <<k>>", "k: 2"], "_.r", "2"),
        (vec!["k: @int\nh: { r: <<k>> }", "k: 2"], "_.h.r", "2"),
        (vec!["k: 1\nh: { k: 5, f: (x -> <<k>>) }\nz: h.f 0"], "_.z", "1"),
        (vec!["f: (y -> <<y>>)\ng: f { a: 7 }"], "_.g", "_"),
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

/// g2 — Roots of references do not move (v0.104.0).
#[test]
fn g2_roots_of_references_do_not_move() {
    let mut bad = Vec::new();
    let cases = [
        ("x: 0", "b74d775816f6b5be6cc152577b7556f6c8fdcc63433b0e31068cc2484a94ae0f"),
        ("t: { a: 1 }\nc: { r: <<t>> }", "756830fc5e46327bd60b57e540d449e9c522ab76046718c9e85c9ae795f42817"),
        ("h: { k: 5, r: <<k>> }", "d7e8706b765f14a72b0df0884eb7f2e31fc35be7b40b3bef6490cd02342e006e"),
        ("k: 1\nh: { k: 5, r: <<k>> }", "f5b524cfda7816dd57d27f54fb24992a5f474ebe959b51b324b5964c3103b462"),
        ("h: { k: 5, s: { r: <<^.k>> } }", "8e1e8c2b6e3213f800775c180e9cc4181b8ed2eb08d9eca74c4f32746c34e5f2"),
        ("h: { k: 5, r: <<k>> }\ng: h", "f8fce2800e115b54279c7a288674cd2d3e3602b099a203b2a4299d3922490990"),
        ("v: <<_.>> |> <<_.>>", "5ce55aa56a6260f0a38232802ce05cd798e7ec8b250c873b7d15c2b22022d168"),
    ];
    for (src, root) in cases {
        check(&mut bad, src, root_of(src), root);
    }
    verdict(bad, cases.len());
}
