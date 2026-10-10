// A table you cannot read — R-1: a stored value that can be applied.
// Order: nlang-tools/docs/a_table_you_cannot_read_handover.md §9
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-077.  Rulings: meta/oo/STATUS.md D100.
//
// ── What R-1 is ──────────────────────────────────────────────────────────
//
// D100: `t.k` reads what the branch `k` stores. Measured on the delivery
// (dev 8142242): that holds when the stored value is data, and fails when
// the stored value can itself be applied — an arrow, a partial builtin, a
// table. Navigation then applies the stored value to the constraint:
//   * `{ @{ @int }: "n", f: (x -> x) }.f`          "f"   (v0.77: the arrow)
//   * `{ @{ @int }: "n", f: (x -> x + 1) }.f 4`   ⊥     (v0.77: 5)
//   * `{ @{ @int }: "n", t: { @{ @str }: "s", a: 1 } }.t`   "s"   (v0.77: the table)
//   * a root that is a table: `_.` and `(_.)` ⟹ ⊥ #no_matching_branch
//     (v0.78.0 and v0.77: the root); `f 1` on a root arrow ⟹ ⊥.
// The `&` assembly read these right on v0.78.0; the delivery made all
// assemblies agree, on the wrong answer.
//
// Application is not navigation: dispatch that selects a branch holding an
// arrow applies it to the input (`{ @{ @int }: (x -> x + 1) } 5` is 6).
// That stays.
//
// The delivery may NOT edit this file. `rustfmt` must not touch it.

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
        let scratch = nlang_interpreter::ScratchDir::new(&format!("table-cannot-read-r1-{tag}"));
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
}

fn is(o: &str, want: &str) -> bool {
    o == want
}

/// Run every `(expression, check, expectation)` and report all misses at once.
fn table(w: &Ws, cases: &[(String, fn(&str, &str) -> bool, &str)]) {
    let mut wrong = Vec::new();
    for (e, check, want) in cases {
        let o = w.eval(e);
        if !check(&o, want) {
            wrong.push(format!("`{e}` => {o}   (wanted {want})"));
        }
    }
    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
}

/// `{ @{ @int }: "n" }` and `{ <key>: <value> }`, assembled every way the
/// language has.
fn assemblies(kv: &str) -> [String; 5] {
    [
        format!(r#"{{ @{{ @int }}: "n", {kv} }}"#),
        format!(r#"({{ @{{ @int }}: "n" }} & {{ {kv} }})"#),
        format!(r#"({{ {kv} }} & {{ @{{ @int }}: "n" }})"#),
        format!(r#"({{ @{{ @int }}: "n" }} |> {{ {kv} }})"#),
        format!(r#"{{ ...{{ @{{ @int }}: "n" }}, {kv} }}"#),
    ]
}

// ── Red on the delivery (dev 8142242) ────────────────────────────────────

/// D100: a stored arrow is read, not applied to its key — on every assembly.
#[test]
fn ra1_a_stored_arrow_is_read_not_applied() {
    let w = Ws::new("ra1");
    let mut cases = Vec::new();
    for a in assemblies("f: (x -> x + 1)") {
        cases.push((format!("{a}.f 4"), is as fn(&str, &str) -> bool, "5"));
        cases.push((format!("({a}.f) 4"), is, "5"));
    }
    cases.push((r#"{ @{ @int }: "n", f: ~%Math./add 1 }.f 2"#.to_string(), is, "3"));
    cases.push((r#"{ @{ @int }: "n", f: (x -> 7) }.f 1"#.to_string(), is, "7"));
    table(&w, &cases);
}

/// D100: a stored table is read, not applied to its key — on every assembly.
#[test]
fn ra2_a_stored_table_is_read_not_applied() {
    let w = Ws::new("ra2");
    let mut cases = Vec::new();
    for a in assemblies(r#"t: { @{ @str }: "s", a: 1 }"#) {
        cases.push((format!("{a}.t.a"), is as fn(&str, &str) -> bool, "1"));
        cases.push((format!(r#"({a}.t) 4"#), is, "_|_  ;; %cause: #no_matching_branch"));
        cases.push((format!(r#"({a}.t) "x""#), is, "\"s\""));
    }
    table(&w, &cases);
}

/// D100 on a root that is a table: the whole root, a root arrow by name, and
/// a later evolve that uses it.
#[test]
fn ra3_a_root_that_is_a_table_is_read() {
    let w = Ws::new("ra3");
    w.commit_each(&["@{ @int }: \"n\"\nj: 5\nf: (x -> x + j)\nt: { @{ @str }: \"s\", a: 1 }"]);
    let mut wrong = Vec::new();
    for e in ["_.", "(_.)"] {
        let o = w.eval(e);
        if o.starts_with("_|_") || !o.contains("j: 5") || !o.contains("@{ @int }: \"n\"") {
            wrong.push(format!("`{e}` printed:\n{o}\n(wanted the root: `j: 5`, `@{{ @int }}: \"n\"`)"));
        }
    }
    let o = w.eval("(y -> y)");
    if o.starts_with("_|_") || o.starts_with('"') || !o.contains("%code") {
        wrong.push(format!("`(y -> y)` at this root printed:\n{o}\n(wanted the arrow)"));
    }
    for (e, want) in [("f 1", "6"), ("_.f 1", "6"), ("_.t.a", "1"), ("(_.) 3", "\"n\""), ("{ @{ @str }: \"s\" } \"x\"", "\"s\"")] {
        let o = w.eval(e);
        if o != want {
            wrong.push(format!("`{e}` => {o}   (wanted {want})"));
        }
    }
    w.commit_each(&["g: (x -> f x + j)"]);
    let o = w.eval("_.g 1");
    if o != "11" {
        wrong.push(format!("after evolving `g: (x -> f x + j)`: `_.g 1` => {o}   (wanted 11)"));
    }
    fs::write(w.ws.join("o.n"), "@{ @int }: \"n\"\nt: { @{ @str }: \"s\", a: 1 }\n").unwrap();
    let (o, _) = w.oo(&["run", "--observe", "t.a", "o.n"]);
    if o != "1" {
        wrong.push(format!("`run --observe t.a` => {o}   (wanted 1)"));
    }
    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
}

// ── Guard: green on the delivery, must stay green ────────────────────────

/// Application is not navigation: a selected branch that holds an arrow
/// applies it to the input. (Mutation: dispatch returns the stored arrow
/// unapplied ⟹ red.)
#[test]
fn ga1_application_still_applies_a_stored_arrow() {
    let w = Ws::new("ga1");
    table(&w, &[
        (r#"{ @{ @int }: (x -> x + 1) } 5"#.to_string(), is, "6"),
        (r#"{ @{ @int }: "n", f: (x -> 7) } "f""#.to_string(), is, "7"),
        (r#"({ @{ @int }: "n" } & { f: (x -> x) }) "f""#.to_string(), is, "\"f\""),
    ]);
}
