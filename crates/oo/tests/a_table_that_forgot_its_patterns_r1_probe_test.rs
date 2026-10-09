// A table that forgot its patterns — R-1: a key that arrives later.
// Order: nlang-tools/docs/a_table_that_forgot_its_patterns_handover.md §9
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-076.  Rulings: meta/oo/STATUS.md D95, D97.
//
// ── What R-1 is ──────────────────────────────────────────────────────────
//
// I2 (D95): the same pattern written twice is one branch, and the two bodies
// meet. I3 (D97): every non-meta key of a table is a constraint, and a table
// that was committed — or a root that is itself a table — answers like the
// literal. Measured on the delivery (dev 6784632): both hold when the keys
// sit in one literal, and both fail when a key arrives *later* — by `&`, or
// by a second `evolve` onto a root that is already a table. If the arriving
// key's constraint already names a branch, the arriving value is dropped:
//   * `{ @{ "k" }: 1, k: 5 } "k"`            ⊥ #conflict   (literal: right)
//   * `({ @{ "k" }: 1 } & { k: 5 }) "k"`      1             (5 is gone)
//   * `({ @{ @int }: 1, _: 2 } & { _: 9 }) "s"`  2           (9 is gone)
//   * root `@{ "k" }: 1`, then `evolve` of `k: 5` is accepted, and
//     `(_.) "k"` is 1 while `_.k` is 5.
// v0.77.0 refused that `evolve` (#conflict at k) — for the wrong reason (the
// printed key collided), but with the right answer.
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
        let scratch = nlang_interpreter::ScratchDir::new(&format!("forgot-patterns-r1-{tag}"));
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
}

fn is(o: &str, want: &str) -> bool {
    o == want
}

fn conflict(o: &str, _: &str) -> bool {
    o.starts_with("_|_") && o.contains("#conflict")
}

fn both(o: &str, _: &str) -> bool {
    o.starts_with('{') && o.contains("x: 1") && o.contains("y: 2")
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

/// The literal is the instrument: both keys in one literal already meet.
/// If this is not ⊥, the R-1 probes below read nothing.
#[test]
fn ra0_the_literal_meets() {
    let w = Ws::new("ra0");
    table(&w, &[
        (r#"{ @{ "k" }: 1, k: 5 } "k""#, conflict, "#conflict"),
        (r#"{ @{ @int }: 1, _: 2, _: 9 } "s""#, conflict, "#conflict"),
    ]);
}

/// I2 + I3: a named, quoted or reversed key that arrives by `&` meets the
/// branch its constraint names — as in the literal.
#[test]
fn ra1_a_key_that_arrives_by_meet_meets_its_branch() {
    let w = Ws::new("ra1");
    table(&w, &[
        (r#"({ @{ "k" }: 1 } & { k: 5 }) "k""#, conflict, "#conflict"),
        (r#"({ k: 5 } & { @{ "k" }: 1 }) "k""#, conflict, "#conflict"),
        (r#"({ @{ "k" }: 1 } & { "k": 5 }) "k""#, conflict, "#conflict"),
        (r#"({ @{ 1 }: 1 } & { 1: 5 }) 1"#, conflict, "#conflict"),
        (r#"({ @{ "k" }: { y: 2 } } & { k: { y: 3 } }) "k""#, conflict, "#conflict"),
        (r#"({ @{ "k" }: { y: 2 } } & { k: { x: 1 } }) "k""#, both, "x: 1 and y: 2"),
    ]);
}

/// I3: `_` that arrives by `&` meets the table's default.
#[test]
fn ra2_a_default_that_arrives_by_meet_meets_the_default() {
    let w = Ws::new("ra2");
    table(&w, &[
        (r#"({ @{ @int }: 1, _: 2 } & { _: 9 }) "s""#, conflict, "#conflict"),
    ]);
}

/// I3: a root that is a table, and gets a key by a second `evolve`, answers
/// like its literal. Refusing the second `evolve` is also an answer (the
/// root is then unchanged and answers 1). Accepting it and answering 1 is not.
#[test]
fn ra3_a_root_built_in_two_evolves_answers_like_its_literal() {
    let w = Ws::new("ra3");
    fs::write(w.ws.join("a.n"), "@{ \"k\" }: 1\n").unwrap();
    fs::write(w.ws.join("b.n"), "k: 5\n").unwrap();
    let (o, rc) = w.oo(&["evolve", "a.n"]);
    assert_eq!(rc, 0, "setup evolve: {o}");
    let (o, rc) = w.oo(&["commit", "-m", "a"]);
    assert_eq!(rc, 0, "setup commit: {o}");
    let (ev, erc) = w.oo(&["evolve", "b.n"]);
    if erc == 0 {
        let (o, rc) = w.oo(&["commit", "-m", "b"]);
        assert_eq!(rc, 0, "commit after an accepted evolve: {o}");
        let got = w.eval(r#"(_.) "k""#);
        assert!(conflict(&got, ""), "evolve of `k: 5` was accepted ({ev}); `(_.) \"k\"` => {got} (wanted ⊥ #conflict, as the literal)");
    } else {
        let got = w.eval(r#"(_.) "k""#);
        assert!(is(&got, "1"), "evolve of `k: 5` was refused ({ev}); `(_.) \"k\"` => {got} (wanted 1: the root is unchanged)");
    }
}

/// Guard: a key whose constraint names no branch still joins the table.
/// (Mutation: arriving keys are never added ⟹ red.)
#[test]
fn ga1_a_new_key_still_joins() {
    let w = Ws::new("ga1");
    table(&w, &[
        (r#"({ @{ "k" }: 1 } & { j: 5 }) "j""#, is, "5"),
        (r#"({ @{ "k" }: 1 } & { j: 5 }) "k""#, is, "1"),
        (r#"({ @{ @int }: 1 } & { _: 9 }) "s""#, is, "9"),
        (r#"({ @{ @int }: 1 } & { _: 9 }) 4"#, is, "1"),
    ]);
}

/// Guard: equal bodies meet to themselves.
/// (Mutation: an arriving key that names a branch is always ⊥ ⟹ red.)
#[test]
fn ga2_equal_bodies_still_meet_to_themselves() {
    let w = Ws::new("ga2");
    table(&w, &[
        (r#"({ @{ "k" }: 1 } & { k: 1 }) "k""#, is, "1"),
        (r#"({ @{ @int }: 1, _: 2 } & { _: 2 }) "s""#, is, "2"),
    ]);
}
