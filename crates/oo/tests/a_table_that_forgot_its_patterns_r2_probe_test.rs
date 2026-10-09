// A table that forgot its patterns — R-2: a branch nobody chose ran.
// Order: nlang-tools/docs/a_table_that_forgot_its_patterns_handover.md §11
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-076.  Rulings: meta/oo/STATUS.md D95, D97.
//
// ── What R-2 is ──────────────────────────────────────────────────────────
//
// R-1 made a key that arrives later meet its branch. Measured on the R-1
// delivery (dev b187d83): the meet happens for *every* colliding branch at
// every application — before dispatch chooses — so both bodies are forced
// whether or not the branch is chosen. The value is right; the work is not:
//   * `({ @{ "k" }: 1, _: 0 } & { k: ~%Io./write_file ("met.txt", "x") }) "z"`
//     answers 0 and writes met.txt. The same keys in one literal write nothing.
//   * the existing body is forced too; so is a branch that matches but is
//     not minimal (`_` beside `@{ @int }`).
// And when two tables (or two pattern arrows) with the same pattern meet by
// `&`, the two branches meet *in `unify`*, at once: both bodies run, and a
// conflict between them makes the whole table ⊥ —
//   * `({ @{ "k" }: 1, _: 0 } & { @{ "k" }: 2 }) "z"` is ⊥; the same three
//     keys in one literal answer 0 (only the "k" branch is ⊥);
//   * `((@int -> 1) & (@int -> 2)) "s"` is ⊥ #conflict, not a miss —
//     SYNTAX_11 §4 #6: a conflict collapses at application, not definition.
// (The first delivery's Q6 reported the first of these; the acceptor read
// it as the branch being ⊥ and missed that it was the table.)
// A branch body is evaluated only when dispatch chooses that branch — as in
// the literal (call-by-observation, SPEC_04 §6). The cause is the acceptor's:
// the R-1 order described its reference as "force both bodies and unify".
//
// The delivery may NOT edit this file. `rustfmt` must not touch it.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

struct Ws {
    _scratch: nlang_interpreter::ScratchDir,
    root: PathBuf,
    ws: PathBuf,
}

impl Ws {
    fn new(tag: &str) -> Self {
        let scratch = nlang_interpreter::ScratchDir::new(&format!("forgot-patterns-r2-{tag}"));
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

    /// Observe `q` in a file whose only line is `q: <expr>`; return the
    /// answer and whether `mark` was written.
    fn observe(&self, expr: &str, mark: &str) -> (String, bool) {
        let _ = fs::remove_file(self.ws.join(mark));
        fs::write(self.ws.join("q.n"), format!("q: {expr}\n")).unwrap();
        let (o, _) = self.oo(&["run", "--observe", "q", "q.n"]);
        (o, self.ws.join(mark).exists())
    }
}

fn walk(p: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(d) = fs::read_dir(p) {
        for e in d.flatten() {
            let q = e.path();
            if q.is_dir() {
                walk(&q, out);
            } else {
                out.push(q);
            }
        }
    }
}

/// Commit `t: <expr>` in a fresh universe; return the root digest the commit
/// records and what `(_.t) "z"` answers there.
fn committed(tag: &str, expr: &str) -> (String, String) {
    let ws = Ws::new(tag);
    fs::write(ws.ws.join("a.n"), format!("t: {expr}\n")).unwrap();
    let (o, rc) = ws.oo(&["evolve", "a.n"]);
    assert_eq!(rc, 0, "setup evolve of `{expr}`: {o}");
    let (o, rc) = ws.oo(&["commit", "-m", "a"]);
    assert_eq!(rc, 0, "setup commit of `{expr}`: {o}");
    let mut files = Vec::new();
    walk(&ws.ws.join(".oo").join("objects"), &mut files);
    let mut found = Vec::new();
    for p in files {
        let t = fs::read_to_string(&p).unwrap_or_default();
        if t.starts_with("#nlang/store commit") {
            let i = t.find("digest: \"").expect("commit names its root") + 9;
            found.push(t[i..i + 64].to_string());
        }
    }
    assert_eq!(found.len(), 1, "VOID READING: expected one commit, found {}", found.len());
    let (z, _) = ws.oo(&["eval", r#"(_.t) "z""#]);
    (found.remove(0), z)
}

fn w(mark: &str) -> String {
    format!(r#"~%Io./write_file ("{mark}", "x")"#)
}

/// Guard — the instrument reads: a chosen branch's effect does run, both
/// in a literal and when the key arrived by `&`.
/// (Mutation: a chosen branch drops its arriving body ⟹ red.)
#[test]
fn gb1_a_chosen_branch_runs() {
    let ws = Ws::new("gb1");
    let mut wrong = Vec::new();
    for (expr, mark) in [
        (format!(r#"{{ @{{ "k" }}: {}, _: 0 }} "k""#, w("lit.txt")), "lit.txt"),
        (format!(r#"({{ @{{ "k" }}: 1, _: 0 }} & {{ k: {} }}) "k""#, w("met.txt")), "met.txt"),
    ] {
        let (o, wrote) = ws.observe(&expr, mark);
        if !wrote {
            wrong.push(format!("`{expr}` => {o}; {mark} was not written (the branch was chosen)"));
        }
    }
    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
}

/// Control — in one literal, a branch nobody chose does not run. If this is
/// red, the R-2 probes below read nothing.
#[test]
fn rb0_in_a_literal_an_unchosen_branch_does_not_run() {
    let ws = Ws::new("rb0");
    let mut wrong = Vec::new();
    for (expr, mark) in [
        (format!(r#"{{ @{{ "k" }}: 1, k: {}, _: 0 }} "z""#, w("a.txt")), "a.txt"),
        (format!(r#"{{ @{{ @int }}: 1, _: {} }} 4"#, w("b.txt")), "b.txt"),
    ] {
        let (o, wrote) = ws.observe(&expr, mark);
        if wrote {
            wrong.push(format!("`{expr}` => {o}; {mark} was written (no branch that writes was chosen)"));
        }
    }
    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
}

/// The arriving body of a branch the argument does not match is not run.
#[test]
fn rb1_an_arriving_body_of_an_unmatched_branch_does_not_run() {
    let ws = Ws::new("rb1");
    let expr = format!(r#"({{ @{{ "k" }}: 1, _: 0 }} & {{ k: {} }}) "z""#, w("met.txt"));
    let (o, wrote) = ws.observe(&expr, "met.txt");
    assert!(!wrote && o.starts_with('0'), "`{expr}` => {o}; written: {wrote} (wanted 0, nothing written)");
}

/// The existing body of a branch the argument does not match is not run
/// because a key arrived beside it.
#[test]
fn rb2_an_existing_body_of_an_unmatched_branch_does_not_run() {
    let ws = Ws::new("rb2");
    let expr = format!(r#"({{ @{{ "k" }}: {}, _: 0 }} & {{ k: 5 }}) "z""#, w("left.txt"));
    let (o, wrote) = ws.observe(&expr, "left.txt");
    assert!(!wrote && o.starts_with('0'), "`{expr}` => {o}; written: {wrote} (wanted 0, nothing written)");
}

/// A branch that matches but is not minimal is not run either.
#[test]
fn rb3_a_matching_but_not_minimal_branch_does_not_run() {
    let ws = Ws::new("rb3");
    let expr = format!(r#"({{ @{{ @int }}: 1, _: 2 }} & {{ _: {} }}) 4"#, w("dflt.txt"));
    let (o, wrote) = ws.observe(&expr, "dflt.txt");
    assert!(!wrote && o.starts_with('1'), "`{expr}` => {o}; written: {wrote} (wanted 1, nothing written)");
}

/// Two tables with the same pattern: a branch nobody chose does not run.
#[test]
fn rb4_tables_that_meet_do_not_run_an_unchosen_branch() {
    let ws = Ws::new("rb4");
    let mut wrong = Vec::new();
    for (expr, mark, want) in [
        (format!(r#"({{ @{{ "k" }}: {}, _: 0 }} & {{ @{{ "k" }}: 1 }}) "z""#, w("t.txt")), "t.txt", "0"),
        (format!(r#"((@str -> {}) & (@str -> 1)) 4"#, w("a.txt")), "a.txt", "_|_"),
    ] {
        let (o, wrote) = ws.observe(&expr, mark);
        if wrote || !o.starts_with(want) {
            wrong.push(format!("`{expr}` => {o}; written: {wrote} (wanted {want}, nothing written)"));
        }
    }
    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
}

/// Two tables with the same pattern: a conflict between the two bodies is
/// that branch's, not the table's — as in the literal.
#[test]
fn rb5_a_conflict_between_two_bodies_is_the_branchs() {
    let ws = Ws::new("rb5");
    let mut wrong = Vec::new();
    for (e, want) in [
        (r#"{ @{ "k" }: 1, _: 0, @{ "k" }: 2 } "z""#, "0"),
        (r#"({ @{ "k" }: 1, _: 0 } & { @{ "k" }: 2 }) "z""#, "0"),
        (r#"({ @{ "k" }: 1, _: 0 } & { @{ "k" }: 2 }) "k""#, "#conflict"),
        (r#"({ @{ 4.. }: "A", _: 0 } & { @{ 4.. }: "B" }) 1"#, "0"),
    ] {
        let (o, _) = ws.oo(&["eval", e]);
        let ok = if want.starts_with('#') { o.starts_with("_|_") && o.contains(want) } else { o == want };
        if !ok {
            wrong.push(format!("`{e}` => {o}   (wanted {want})"));
        }
    }
    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
}

/// Two pattern arrows with the same pattern conflict at application, per
/// input (SYNTAX_11 §4 #6; SPEC_07 §5.3 definition 3).
#[test]
fn rb6_arrows_with_one_pattern_conflict_at_application() {
    let ws = Ws::new("rb6");
    let mut wrong = Vec::new();
    for (e, want) in [
        (r#"((@int -> 1) & (@int -> 2)) "s""#, "#no_matching_branch"),
        (r#"((@int -> 1) & (@int -> 2)) 4"#, "#conflict"),
    ] {
        let (o, _) = ws.oo(&["eval", e]);
        if !(o.starts_with("_|_") && o.contains(want)) {
            wrong.push(format!("`{e}` => {o}   (wanted {want})"));
        }
    }
    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
}

/// Guard: equal bodies of one pattern still meet to themselves, and
/// different bodies still merge, when the tables or arrows meet by `&`.
/// (Mutation: a deferred meet always answers ⊥ ⟹ red.)
#[test]
fn gb2_bodies_of_one_pattern_still_meet() {
    let ws = Ws::new("gb2");
    let mut wrong = Vec::new();
    for (e, ok) in [
        (r#"({ @{ "k" }: 1, _: 0 } & { @{ "k" }: 1 }) "k""#, (|o: &str| o == "1") as fn(&str) -> bool),
        (r#"((@int -> 1) & (@int -> 1)) 4"#, |o: &str| o == "1"),
        (r#"({ @{ "k" }: { y: 2 } } & { @{ "k" }: { x: 1 } }) "k""#, |o: &str| o.starts_with('{') && o.contains("x: 1") && o.contains("y: 2")),
        (r#"({ @{ @int }: "A" } & { @{ @str }: "B" }) "s""#, |o: &str| o == "\"B\""),
    ] {
        let (o, _) = ws.oo(&["eval", e]);
        if !ok(&o) {
            wrong.push(format!("`{e}` => {o}"));
        }
    }
    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
}

/// The bodies of one branch are a set: how the tables met — which side
/// first, how grouped, the same one twice — does not reach the bytes. A
/// merge that depends on its order cannot converge (commit.md §1.1.6).
/// Each universe must also answer `(_.t) "z"` with 0: equal roots of a
/// table that collapsed to ⊥ read nothing.
#[test]
fn rb7_the_bodies_of_one_branch_are_a_set() {
    let groups: [&[&str]; 4] = [
        &[
            r#"{ @{ "k" }: 1, _: 0 } & { @{ "k" }: 2 }"#,
            r#"{ @{ "k" }: 2 } & { @{ "k" }: 1, _: 0 }"#,
            r#"({ @{ "k" }: 1, _: 0 } & { @{ "k" }: 2 }) & { @{ "k" }: 2 }"#,
        ],
        &[
            r#"({ @{ "k" }: 1, _: 0 } & { @{ "k" }: 2 }) & { @{ "k" }: 3 }"#,
            r#"{ @{ "k" }: 1, _: 0 } & ({ @{ "k" }: 2 } & { @{ "k" }: 3 })"#,
            r#"{ @{ "k" }: 3 } & ({ @{ "k" }: 1, _: 0 } & { @{ "k" }: 2 })"#,
        ],
        &[
            r#"{ @{ "k" }: 1, _: 0 } & { @{ "k" }: 1, _: 0 }"#,
            r#"{ @{ "k" }: 1, _: 0 }"#,
        ],
        &[
            r#"(@int -> 1) & (@int -> 2) & { @{ "z" }: 0 }"#,
            r#"(@int -> 2) & { @{ "z" }: 0 } & (@int -> 1)"#,
        ],
    ];
    let mut wrong = Vec::new();
    for (g, exprs) in groups.iter().enumerate() {
        let mut roots = Vec::new();
        for (i, e) in exprs.iter().enumerate() {
            let (root, z) = committed(&format!("rb7-{g}-{i}"), e);
            if z != "0" {
                wrong.push(format!("`t: {e}` committed; `(_.t) \"z\"` => {z} (wanted 0)"));
            }
            roots.push((e, root));
        }
        let first = &roots[0].1;
        for (e, r) in &roots[1..] {
            if r != first {
                wrong.push(format!("`{}` root {} ≠ `{}` root {}", roots[0].0, &first[..12], e, &r[..12]));
            }
        }
    }
    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
}
