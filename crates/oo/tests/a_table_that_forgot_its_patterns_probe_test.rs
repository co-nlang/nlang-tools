// A table that forgot its patterns.
// Order: nlang-tools/docs/a_table_that_forgot_its_patterns_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-076.  Rulings: meta/oo/STATUS.md D95–D99.
//
// ── What this arc is ─────────────────────────────────────────────────────
//
// SPEC_07 §1.1, step 1: the input is merged with every key of the morphism
// — "Key (input constraint)". Measured on v0.77.0 (and the same, cell for
// cell, back to v0.27.0): the engine drops the constraint while evaluating
// and keeps something else in its place.
//   * an `@{expr}:` key becomes the *printed string* of the pattern, and
//     dispatch guesses a pattern back from the string; whatever it cannot
//     guess becomes Top. `{ @{ @int }: "A" } "s"` is "A"; `@{ @int }` and
//     `@{ @str }` are both "{...}", so a two-type table is ⊥ on everything;
//   * the left side of `->` keeps only a binder name: `(@int -> 7) "s"` is
//     7, `(x @int -> x) 4` is `_`;
//   * applying a Combo looks the key up by the argument's printed string:
//     `{ "{...}": "caught" } { x: 1 }` is "caught".
// Also: `_:` in a table fails ("Rule has no %code"), `$` in a branch value
// is `#no_context`, a miss is `#conflict` (the registered `#no_matching_branch`
// is never issued), and an undocumented `it` key/binder outranks `_`.
//
// D95: a pattern is a value kept with its branch; its printed form is never
//      read back as it. Bare-binder morphisms (`x -> …`) keep their bytes.
// D96: a bare `@T:` key is always the type facet; constraints are `@{ @T }:`.
// D97: a Combo with a pattern key is a table; its other keys are constraints
//      (number, tag, string; `_` is the default). Without a pattern key,
//      application is lookup — and only an atom has a key.
// D98: `it` (key and binder) and the implicit `0` binder are retired.
// D99: a table an older engine wrote (printed pattern names, no `%rules`)
//      answers ⊥ `#pattern_not_kept` — not a guess.
//
// ── Probe integrity ──────────────────────────────────────────────────────
//
// `@str` is the string type (`@string` is not a known type and merges with
// anything — a separate defect). `@int & 4..` evaluates to `4..` in this
// engine (a separate defect), so no probe relies on that pair being two
// patterns. The delivery may NOT edit this file. `rustfmt` must not touch it.
//
// Baseline measured 2026-10-09 on dev 17c328a / oo v0.77.0: see the order.

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
        let scratch = nlang_interpreter::ScratchDir::new(&format!("forgot-patterns-{tag}"));
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

    fn committed(tag: &str, text: &str) -> Self {
        let w = Ws::new(tag);
        fs::write(w.ws.join("a.n"), text).unwrap();
        let (o, rc) = w.oo(&["evolve", "a.n"]);
        assert_eq!(rc, 0, "setup evolve: {o}");
        let (o, rc) = w.oo(&["commit", "-m", "a"]);
        assert_eq!(rc, 0, "setup commit: {o}");
        w
    }

    /// The root address the (single) commit records.
    fn root_digest(&self) -> String {
        let mut files = Vec::new();
        walk(&self.ws.join(".oo").join("objects"), &mut files);
        let mut found = Vec::new();
        for p in files {
            let t = fs::read_to_string(&p).unwrap_or_default();
            if t.starts_with("#nlang/store commit") {
                let i = t.find("digest: \"").expect("commit names its root") + 9;
                found.push(t[i..i + 64].to_string());
            }
        }
        assert_eq!(found.len(), 1, "VOID READING: expected one commit, found {}", found.len());
        found.remove(0)
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

fn is(o: &str, want: &str) -> bool {
    o == want
}

fn no_match(o: &str) -> bool {
    o.contains("#no_matching_branch")
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

fn nm(o: &str, _: &str) -> bool {
    no_match(o)
}

// ── Red ──────────────────────────────────────────────────────────────────

/// r1 — two type branches are two branches.
#[test]
fn r1_two_type_branches_are_two_branches() {
    let w = Ws::new("r1");
    table(&w, &[
        (r#"{ @{ @int }: "A", @{ @str }: "B" } 1"#, is, r#""A""#),
        (r#"{ @{ @int }: "A", @{ @str }: "B" } "s""#, is, r#""B""#),
        (r#"{ @{ @int }: "A", @{ @str }: "B" } 2.5"#, nm, "#no_matching_branch"),
    ]);
}

/// r2 — a pattern the engine cannot print faithfully still constrains:
/// no input outside it is answered.
#[test]
fn r2_no_pattern_matches_everything() {
    let w = Ws::new("r2");
    table(&w, &[
        (r#"{ @{ @int }: "A" } "s""#, nm, "#no_matching_branch"),
        (r#"{ @{ "s" }: 1 } 5"#, nm, "#no_matching_branch"),
        (r#"{ @{ 1 | 2 }: "U", @{ 1 }: "O" } 5"#, nm, "#no_matching_branch"),
        (r#"{ @{ x -> x }: "M" } 5"#, nm, "#no_matching_branch"),
        (r#"{ @{ 4.. }: "A" } 1"#, nm, "#no_matching_branch"),
    ]);
}

/// r3 — patterns that print alike are still different patterns.
#[test]
fn r3_patterns_that_print_alike_stay_apart() {
    let w = Ws::new("r3");
    table(&w, &[
        (r#"{ @{ 1 | 2 }: "U", @{ 1 }: "O" } 1"#, is, r#""O""#),
        (r#"{ @{ 1 | 2 }: "U", @{ 1 }: "O" } 2"#, is, r#""U""#),
        (r#"{ @{ "s" }: 1, @{ "t" }: 2 } "t""#, is, "2"),
        (r#"{ @{ "4..6" }: "S", @{ 4..6 }: "R" } 5"#, is, r#""R""#),
        (r#"{ @{ "4..6" }: "S", @{ 4..6 }: "R" } "4..6""#, is, r#""S""#),
    ]);
}

/// r4 — the left side of `->` is a pattern, and `x @T` keeps both halves.
#[test]
fn r4_the_left_of_an_arrow_is_a_pattern() {
    let w = Ws::new("r4");
    table(&w, &[
        (r#"(@int -> 7) "s""#, nm, "#no_matching_branch"),
        ("(@int -> 7) 4", is, "7"),
        ("(x @int -> x + 1) 4", is, "5"),
        (r#"(x @int -> x) "s""#, nm, "#no_matching_branch"),
        ("(4.. -> \"big\") 1", nm, "#no_matching_branch"),
        ("(4.. -> \"big\") 9", is, r#""big""#),
        ("(#a -> 1) #b", nm, "#no_matching_branch"),
        ("(#a -> 1) #a", is, "1"),
    ]);
}

/// r5 — a value remembers its pattern: different patterns, different values.
#[test]
fn r5_different_patterns_are_different_values() {
    let w = Ws::new("r5");
    table(&w, &[
        ("(x @int -> 1) = (x @str -> 1)", is, "#false"),
        ("(@int -> 1) = (int -> 1)", is, "#false"),
        (r#"{ @{ @int }: "A" } = { @{ @str }: "A" }"#, is, "#false"),
    ]);
}

/// r6 — the table's own keys: `_` is the default branch (SPEC_07 §1.1.1),
/// `$` in a branch is the matched input (SYNTAX_12 §2 #5), number keys are
/// constraints (the spec's `/fib`, written with D96's spelling).
#[test]
fn r6_default_dollar_and_fib() {
    let w = Ws::new("r6");
    table(&w, &[
        (r#"{ @{ 4.. }: "A", _: "other" } "s""#, is, r#""other""#),
        (r#"{ @{ 4.. }: "A", _: "other" } 5"#, is, r#""A""#),
        ("{ @{ 4.. }: $ + 1 } 5", is, "6"),
    ]);
    fs::write(
        w.ws.join("fib.n"),
        "/fib: {\n    0: 0\n    1: 1\n    @{ @int }: (/fib ($ - 1)) + (/fib ($ - 2))\n}\nq0: /fib 0\nq1: /fib 1\nq5: /fib 5\n",
    )
    .unwrap();
    let mut wrong = Vec::new();
    for (k, want) in [("q0", "0"), ("q1", "1"), ("q5", "5")] {
        let (o, _) = w.oo(&["run", "--observe", k, "fib.n"]);
        if o != want {
            wrong.push(format!("/fib {k}: {o} (wanted {want})"));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// r7 — `it` is retired (D98): not a key, not a binder.
#[test]
fn r7_it_is_retired() {
    let w = Ws::new("r7");
    table(&w, &[
        (r#"{ it: "d" } 42"#, nm, "#no_matching_branch"),
        (r#"{ a: 1, it: 88, _: 99 } "zzz""#, is, "99"),
    ]);
    let o = w.eval("(x -> it) 5");
    assert!(o != "5", "`it` still names the argument: {o}");
}

/// r8 — a lookup never matches a key against an argument's printed form,
/// and a miss is `#no_matching_branch`.
#[test]
fn r8_lookup_is_not_by_printed_form() {
    let w = Ws::new("r8");
    table(&w, &[
        (r#"{ "{...}": "caught" } { x: 1 }"#, nm, "#no_matching_branch"),
        ("{ a: 1 } 7", nm, "#no_matching_branch"),
    ]);
}

/// r9 — a committed table, and a committed root that is a table, dispatch
/// the same as the literal.
#[test]
fn r9_committed_tables_dispatch() {
    let w = Ws::committed("r9", "t: { @{ @int }: \"A\", @{ @str }: \"B\" }\n");
    table(&w, &[
        ("_.t 1", is, r#""A""#),
        (r#"_.t "s""#, is, r#""B""#),
    ]);
    let w = Ws::committed("r9-root", "@{1}: 42\nk: 5\n");
    table(&w, &[
        ("(_.) 1", is, "42"),
        (r#"(_.) "k""#, is, "5"),
        ("(_.) 2", nm, "#no_matching_branch"),
    ]);
}

/// r10 — a table in the old shape (printed pattern names, no `%rules`) is
/// not guessed back (D99), whether written now or read from a store.
#[test]
fn r10_an_old_table_is_not_guessed() {
    let old = r#"{ %morphism: #true, "4..#_": {{ %val: "A" }} }"#;
    let w = Ws::new("r10");
    let o = w.eval(&format!("{old} 5"));
    assert!(o.contains("#pattern_not_kept"), "literal old-shape table: {o}");
    let w = Ws::committed("r10-c", &format!("t: {old}\n"));
    let o = w.eval("_.t 5");
    assert!(o.contains("#pattern_not_kept"), "committed old-shape table: {o}");
}

// ── Green ────────────────────────────────────────────────────────────────

/// g1 — range tables (SPEC_07 §1.1 scenarios B and C) and an explicit
/// `%morphism` beside pattern keys keep working.
#[test]
fn g1_range_tables_still_dispatch() {
    let w = Ws::new("g1");
    table(&w, &[
        (r#"{ @{ @int & 4.. }: "A", @{ @int & ..6 }: "B", @{ @int & 4..6 }: "C" } 5"#, is, r#""C""#),
        (r#"{ @{ @int & 4.. }: "A", @{ @int & ..6 }: "B" } 9"#, is, r#""A""#),
        (r#"{ %morphism: #true, @{1..3}: "low", @{4..}: "high" } 5"#, is, r#""high""#),
    ]);
    let o = w.eval(r#"{ @{ @int & 4.. }: "A", @{ @int & ..6 }: "B" } 5"#);
    assert!(o.contains(r#""A""#) && o.contains(r#""B""#) && o.contains('|'), "scenario B: {o}");
}

/// g2 — lookup on a Combo without pattern keys.
#[test]
fn g2_lookup_still_works() {
    let w = Ws::new("g2");
    table(&w, &[
        (r#"{ a: 1, b: 2, _: 99 } "zzz""#, is, "99"),
        (r#"{ a: 1 } "a""#, is, "1"),
        ("{ #on: 1, #off: 0 } #on", is, "1"),
        (r#"{ 0: "z", 1: "o" } 1"#, is, r#""o""#),
        ("{ @{ #a }: 1, @{ #b }: 2 } #b", is, "2"),
    ]);
}

/// g3 — bare-binder morphisms behave as before.
#[test]
fn g3_bare_binders_behave_as_before() {
    let w = Ws::new("g3");
    table(&w, &[
        ("(x -> x + 1) 4", is, "5"),
        ("(x y -> x + y) 2 3", is, "5"),
        ("((a, b) -> a + b) (1, 2)", is, "3"),
        (r#"(_ -> 1) "q""#, is, "1"),
        ("{ @{ 4.. }: (x -> x + 1) } 5", is, "6"),
    ]);
}

/// g4 — bare-binder morphisms keep their bytes (D95): these roots are the
/// v0.77.0 roots.
#[test]
fn g4_bare_binders_keep_their_bytes() {
    let mut wrong = Vec::new();
    for (i, (text, want)) in [
        ("f: (x -> x + 1)\n", "039d07351a998261d3150af05a84bd0fcf0ad4133b24d644d35ab1e59517d2b4"),
        ("g: (x y -> x + y)\nh: ((a, b) -> a + b)\n", "e7793d964930ee884019432971e5082f10f1c5576c8abc89b4ab2f48dc013a27"),
        ("k: { a: 1, _: 9 }\n", "24fe01b6001567e4b03edc5e0715cb4cf53eadea4945994d7a963b495382662a"),
        ("m: (_ -> 1)\n", "c23f8a56608b9aa32b3fa089aa5d54ec4b0385b990d0cc2fe695970db114fb85"),
    ]
    .iter()
    .enumerate()
    {
        let w = Ws::committed(&format!("g4-{i}"), text);
        let got = w.root_digest();
        if got != *want {
            wrong.push(format!("{text:?}: {got}"));
        }
    }
    assert!(wrong.is_empty(), "roots moved:\n{}", wrong.join("\n"));
}

/// g5 — the order a table is written in does not reach its bytes.
#[test]
fn g5_order_does_not_reach_the_bytes() {
    let a = Ws::committed("g5-a", "t: { @{ @int }: \"A\", @{ @str }: \"B\", @{ 4.. }: \"C\" }\n");
    let b = Ws::committed("g5-b", "t: { @{ 4.. }: \"C\", @{ @str }: \"B\", @{ @int }: \"A\" }\n");
    assert_eq!(a.root_digest(), b.root_digest());
}

/// g6 — `@any -> …` (conformance L2/105's shape) still answers.
#[test]
fn g6_any_arrows_still_answer() {
    let w = Ws::new("g6");
    fs::write(w.ws.join("c.n"), "...~%Cond\nout: /if (#true, (@any -> \"yes\"), (@any -> \"no\"))\n").unwrap();
    let (o, _) = w.oo(&["run", "--observe", "out", "c.n"]);
    assert_eq!(o, r#""yes""#);
}

/// g7 — the same pattern twice is one branch whose bodies meet (SPEC_03
/// §3.1): equal bodies agree, different bodies conflict.
#[test]
fn g7_the_same_pattern_twice_meets() {
    let w = Ws::new("g7");
    table(&w, &[
        (r#"{ @{ 4.. }: "A", @{ 4.. }: "A" } 5"#, is, r#""A""#),
    ]);
    let o = w.eval(r#"{ @{ 4.. }: "A", @{ 4.. }: "B" } 5"#);
    assert!(o.contains("#conflict"), "two bodies for one pattern: {o}");
}
