// A table you cannot read.
// Order: nlang-tools/docs/a_table_you_cannot_read_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-077.  Rulings: meta/oo/STATUS.md D100–D102.
//
// ── What this arc is ─────────────────────────────────────────────────────
//
// D97 (v0.78.0) made every non-meta key of a table a constraint: the
// literal `{ @{ "k" }: 1, j: 5 }` keeps `j` as a branch of `%rules`, not as
// a field. Application was made right on every path. Measured on v0.78.0,
// four other faces were not:
//   * navigation: the literal's `.j`, `.1`, `.cfg.a`, `.k` read `_` (v0.77:
//     5 / "a" / 1 / 1); the same table built by `&`, `|>`, `...` or a root
//     still reads them;
//   * scope: a branch body cannot see its sibling keys —
//     `{ @{ @int }: n + 1, n: 5 } 3` is `_` (v0.77: 6);
//   * bytes: the literal and the `&` of the same keys commit to two roots;
//   * print: a literal table prints digest names where v0.77 printed `j: 5`.
//
// D100: `t.k` on a table reads the branch whose constraint the key spells
//       (D97's key map); `$` there is that constraint; no branch ⟹ `_`
//       (never the `_:` default — `_:` means dispatch only, SPEC_07 §1.1.1);
//       several bodies of that constraint meet.
// D101: one normal form — literal, `&`, `|>`, `...` and the root's evolves
//       give the same bytes; the bodies of one branch are a set.
// D102: a table prints its branches by their constraints (atom constraints
//       as keys, other patterns as `@{ … }:`); digest names only on disk.
//
// The delivery may NOT edit this file. `rustfmt` must not touch it.
//
// Baseline measured 2026-10-10 on oo v0.78.0: see the order.

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};

struct Ws {
    _scratch: nlang_interpreter::ScratchDir,
    root: PathBuf,
    ws: PathBuf,
}

impl Ws {
    fn new(tag: &str) -> Self {
        let scratch = nlang_interpreter::ScratchDir::new(&format!("table-cannot-read-{tag}"));
        let root = scratch.path().to_path_buf();
        let ws = root.join("ws");
        fs::create_dir_all(&ws).unwrap();
        Ws { _scratch: scratch, root, ws }
    }

    fn oo(&self, args: &[&str]) -> (String, i32) {
        let o = Command::new(env!("CARGO_BIN_EXE_oo"))
            .args(args)
            .current_dir(&self.ws)
            .env("OO_IDENTITY", self.root.join("identity"))
            .env("OO_NODE_HOME", self.root.join("node-home"))
            .stdin(Stdio::null())
            .output()
            .expect("oo runs");
        (
            format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)).trim().to_string(),
            o.status.code().unwrap_or(-1),
        )
    }

    fn eval(&self, e: &str) -> String {
        self.oo(&["eval", e]).0
    }

    /// Evolve each source in turn (committing after each) in this universe.
    fn commit_each(&self, sources: &[&str]) {
        for (i, src) in sources.iter().enumerate() {
            let f = format!("s{i}.n");
            fs::write(self.ws.join(&f), format!("{src}\n")).unwrap();
            let (o, rc) = self.oo(&["evolve", &f]);
            assert_eq!(rc, 0, "setup evolve of `{src}`: {o}");
            let (o, rc) = self.oo(&["commit", "-m", &f]);
            assert_eq!(rc, 0, "setup commit of `{src}`: {o}");
        }
    }

    /// The root digest the latest commit (HEAD) records.
    fn head_root(&self) -> String {
        let head = fs::read_to_string(self.ws.join(".oo").join("HEAD")).unwrap_or_default();
        let h = head.trim().rsplit(':').next().unwrap_or("").to_string();
        assert_eq!(h.len(), 64, "VOID READING: HEAD is `{}`", head.trim());
        let p = self.ws.join(".oo").join("objects").join("sha256").join(&h[..2]).join(&h[2..]);
        let t = fs::read_to_string(&p).unwrap_or_default();
        assert!(t.starts_with("#nlang/store commit"), "VOID READING: HEAD object {} is not a commit", p.display());
        let i = t.find("digest: \"").expect("commit names its root") + 9;
        t[i..i + 64].to_string()
    }
}

fn is(o: &str, want: &str) -> bool {
    o == want
}

fn conflict(o: &str, _: &str) -> bool {
    o.starts_with("_|_") && o.contains("#conflict")
}

/// Run every `(expression, check, expectation)` and report all misses at once.
fn table(w: &Ws, cases: &[(&str, fn(&str, &str) -> bool, &str)]) {
    let mut wrong = Vec::new();
    for (e, check, want) in cases {
        let o = w.eval(e);
        if !check(&o, want) {
            wrong.push(format!("`{e}` => {o}   (wanted {want})"));
        }
    }
    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
}

const LIT: &str = r#"{ @{ "k" }: 1, j: 5 }"#;
/// The same keys, assembled every way the language has.
const ASSEMBLIES: [&str; 5] = [
    r#"{ @{ "k" }: 1, j: 5 }"#,
    r#"({ @{ "k" }: 1 } & { j: 5 })"#,
    r#"({ j: 5 } & { @{ "k" }: 1 })"#,
    r#"({ @{ "k" }: 1 } |> { j: 5 })"#,
    r#"{ ...{ @{ "k" }: 1 }, j: 5 }"#,
];

// ── Red on v0.78.0 ───────────────────────────────────────────────────────

/// D100: navigation reads the branch the key spells, on every assembly.
#[test]
fn r1_navigation_reads_the_branch_the_key_spells() {
    let w = Ws::new("r1");
    let mut wrong = Vec::new();
    for a in ASSEMBLIES {
        for (seg, want) in [("j", "5"), ("k", "1")] {
            let e = format!("{a}.{seg}");
            let o = w.eval(&e);
            if o != want {
                wrong.push(format!("`{e}` => {o}   (wanted {want})"));
            }
        }
    }
    for (e, want) in [
        (r#"{ 1: "a", @{ @int }: "n" }.1"#, "\"a\""),
        (r#"{ @{ @int }: "n", cfg: { a: 1 } }.cfg.a"#, "1"),
        (r#"{ @{ @int }: "n", _: 0 }._"#, "0"),
    ] {
        let o = w.eval(e);
        if o != want {
            wrong.push(format!("`{e}` => {o}   (wanted {want})"));
        }
    }
    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
}

/// D100: `$` in a navigated body is the constraint; several bodies of one
/// constraint meet.
#[test]
fn r2_a_navigated_body_sees_its_constraint() {
    let w = Ws::new("r2");
    table(&w, &[
        (r#"{ @{ @int }: "n", j: $ }.j"#, is, "\"j\""),
        (r#"{ @{ "j" }: 1, j: 5 }.j"#, conflict, "#conflict"),
        (r#"{ @{ "j" }: { x: 1 }, j: { y: 2 } }.j.y"#, is, "2"),
    ]);
}

/// D100 (scope face): a branch body sees its sibling keys by name.
#[test]
fn r3_a_branch_sees_its_siblings() {
    let w = Ws::new("r3");
    table(&w, &[
        (r#"{ @{ @int }: n + 1, n: 5 } 3"#, is, "6"),
        (r#"{ 1: "a", @{ @int }: n, n: "x" } 2"#, is, "\"x\""),
        (r#"{ @{ @int }: m, n: 5, m: n * 2 } 3"#, is, "10"),
        (r#"{ 0: zero, zero: "none", @{ @int }: "some" } 0"#, is, "\"none\""),
    ]);
}

/// D100 on a committed field: navigation and `run --observe` read the branch.
#[test]
fn r4_a_committed_table_is_readable() {
    let w = Ws::new("r4");
    w.commit_each(&[r#"t: { @{ "k" }: 1, j: 5 }"#]);
    table(&w, &[(r#"_.t.j"#, is, "5"), (r#"(_.t) "j""#, is, "5")]);
    fs::write(w.ws.join("o.n"), format!("t: {LIT}\n")).unwrap();
    let (o, _) = w.oo(&["run", "--observe", "t.j", "o.n"]);
    assert_eq!(o, "5", "`run --observe t.j` on `t: {LIT}`");
}

/// D101: every assembly is the same value.
#[test]
fn r5_every_assembly_is_the_same_value() {
    let w = Ws::new("r5");
    let mut wrong = Vec::new();
    for a in &ASSEMBLIES[1..] {
        let e = format!("{a} = {LIT}");
        let o = w.eval(&e);
        if o != "#true" {
            wrong.push(format!("`{e}` => {o}   (wanted #true)"));
        }
    }
    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
}

/// D101: every assembly commits to the same root (and the root is readable,
/// so equal roots of a collapsed table read nothing).
#[test]
fn r6_every_assembly_commits_to_one_root() {
    let groups: [&[&str]; 2] = [
        &ASSEMBLIES,
        &[
            r#"{ @{ "k" }: 1, @{ "k" }: 2, _: 0 }"#,
            r#"({ @{ "k" }: 1, _: 0 } & { @{ "k" }: 2 })"#,
            r#"{ @{ "k" }: 1, k: 2, _: 0 }"#,
            r#"(({ @{ "k" }: 1, _: 0 } & { @{ "k" }: 2 }) & { @{ "k" }: 2 })"#,
        ],
    ];
    let mut wrong = Vec::new();
    for (g, exprs) in groups.iter().enumerate() {
        let mut roots = Vec::new();
        for (i, e) in exprs.iter().enumerate() {
            let w = Ws::new(&format!("r6-{g}-{i}"));
            w.commit_each(&[&format!("t: {e}")]);
            let (probe, want) = if g == 0 { (r#"(_.t) "j""#, "5") } else { (r#"(_.t) "z""#, "0") };
            let o = w.eval(probe);
            if o != want {
                wrong.push(format!("`t: {e}` committed; `{probe}` => {o} (wanted {want})"));
            }
            roots.push((e, w.head_root()));
        }
        for (e, r) in &roots[1..] {
            if *r != roots[0].1 {
                wrong.push(format!("`{}` root {} ≠ `{}` root {}", roots[0].0, &roots[0].1[..12], e, &r[..12]));
            }
        }
    }
    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
}

/// D102: a table prints by its constraints, not by digest names.
#[test]
fn r7_a_table_prints_by_its_constraints() {
    let w = Ws::new("r7");
    let hex64 = |s: &str| {
        s.as_bytes().windows(64).any(|win| win.iter().all(|b| b.is_ascii_hexdigit()))
    };
    let mut wrong = Vec::new();
    for (e, must) in [
        (LIT, vec!["j: 5", "k: 1"]),
        (r#"{ @{ @int }: "n", _: 0 }"#, vec!["@{ @int }: \"n\"", "_: 0"]),
        (r#"{ a: 1, b: { @{ @str }: "s" } }"#, vec!["@{ @str }: \"s\""]),
    ] {
        let o = w.eval(e);
        if hex64(&o) || must.iter().any(|m| !o.contains(m)) {
            wrong.push(format!("`{e}` printed:\n{o}\n(wanted {must:?}, no digest names)"));
        }
    }
    fs::write(w.ws.join("p.n"), format!("t: {LIT}\n")).unwrap();
    let (o, _) = w.oo(&["run", "--observe", "t", "p.n"]);
    if hex64(&o) || !o.contains("j: 5") {
        wrong.push(format!("`run --observe t` printed:\n{o}"));
    }
    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
}

// ── Guards: green on v0.78.0, must stay green ────────────────────────────

/// Application answers the same on every assembly (and numeric keys are
/// integers). (Mutation: data keys become string constraints ⟹ red.)
#[test]
fn g1_application_is_unchanged() {
    let w = Ws::new("g1");
    let mut wrong = Vec::new();
    for a in ASSEMBLIES {
        for (arg, want) in [(r#""j""#, "5"), (r#""k""#, "1")] {
            let e = format!("{a} {arg}");
            let o = w.eval(&e);
            if o != want {
                wrong.push(format!("`{e}` => {o}   (wanted {want})"));
            }
        }
    }
    let o = w.eval(r#"{ 1: "a", @{ @int }: "n" } 1"#);
    if o != "\"a\"" {
        wrong.push(format!("numeric key => {o}"));
    }
    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
}

/// Navigation never falls to the `_:` default (SPEC_07 §1.1.1: dispatch
/// only). (Mutation: navigation dispatches ⟹ red.)
#[test]
fn g2_navigation_does_not_fall_to_the_default() {
    let w = Ws::new("g2");
    table(&w, &[
        (r#"{ @{ @int }: "n", _: 0 }.zzz"#, is, "_"),
        (r#"{ @{ @str }: "s", _: 0 }.zzz"#, is, "_"),
        (r#"{ a: 1 }.zzz"#, is, "_"),
    ]);
}

/// A Combo with no pattern key is not a table: its bytes and fields stay.
/// (Mutation: every Combo is normalized ⟹ red.)
#[test]
fn g3_a_plain_combo_is_not_a_table() {
    let w = Ws::new("g3");
    w.commit_each(&["k: { a: 1, _: 9 }"]);
    assert_eq!(
        w.head_root(),
        "24fe01b6001567e4b03edc5e0715cb4cf53eadea4945994d7a963b495382662a",
        "`k: {{ a: 1, _: 9 }}` root moved"
    );
    table(&w, &[(r#"_.k.a"#, is, "1"), (r#"{ j: 5 }.j"#, is, "5"), (r#"{ j: 5 }"#, is, "{\n  j: 5\n}")]);
}

/// Navigation reads only the branch it names: no other body runs. (The `&`
/// assembly, which v0.78.0 can already read.)
/// (Mutation: navigation forces the whole table first ⟹ red.)
#[test]
fn g4_navigation_runs_only_the_named_branch() {
    let w = Ws::new("g4");
    fs::write(
        w.ws.join("q.n"),
        "q: ({ @{ \"k\" }: ~%Io./write_file (\"k.txt\", \"x\") } & { j: 5 }).j\n",
    )
    .unwrap();
    let (o, _) = w.oo(&["run", "--observe", "q", "q.n"]);
    let wrote = w.ws.join("k.txt").exists();
    assert!(o == "5" && !wrote, "`.j` => {o}; k.txt written: {wrote} (wanted 5, nothing written)");
}

/// A root that is a table: one file, two evolves, either order — one root,
/// readable by name and by application.
#[test]
fn g5_a_root_that_is_a_table() {
    let a = Ws::new("g5-a");
    a.commit_each(&["@{ \"k\" }: 1\nj: 5\ny: j * 2"]);
    let b = Ws::new("g5-b");
    b.commit_each(&["@{ \"k\" }: 1", "j: 5\ny: j * 2"]);
    let mut wrong = Vec::new();
    for (w, label) in [(&a, "one file"), (&b, "two evolves")] {
        for (e, want) in [("_.j", "5"), ("_.y", "10"), (r#"(_.) "k""#, "1"), (r#"(_.) "j""#, "5")] {
            let o = w.eval(e);
            if o != want {
                wrong.push(format!("{label}: `{e}` => {o} (wanted {want})"));
            }
        }
    }
    if a.head_root() != b.head_root() {
        wrong.push(format!("roots differ: {} vs {}", &a.head_root()[..12], &b.head_root()[..12]));
    }
    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
}
