// A complement only booleans had.
// Order: nlang-tools/docs/a_complement_only_booleans_had_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-100.  D116: `!` is taken in the
// universe — `!C` is everything not in `C` (SPEC_01 §2.5, axioms 1–5).
// `#true` has no default type: `!#true` is everything that is not `#true`,
// and boolean not is `@bool & !x`. A complement is a value; a store holding
// one declares encoding 10, and a store declaring less refuses it by name.
//
// ── What is wrong (v0.100.0 and before) ─────────────────────────────────
//
// `orthocomplement` answers only for `_`, `_|_`, `#true`/`#false`. Any other
// atom, a range, a union of atoms is ⊥ ("Orthocomplement not defined"); a
// type is a closed combo whose fields cannot be complemented, so `!@int` is
// `{{ }}` and `5 & !@int` is `5`; an open combo becomes the union of its
// fields' complements (`{ b: 1 } & !{ a: #true }` is `#false`). Only the
// literal `x & !(range)` was rewritten to a membership test. A table key
// `@{ !(..0) }` is ⊥ when the table is built and its branch never matches.
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
        let scratch = nlang_interpreter::ScratchDir::new(&format!("complement-{tag}"));
        let root = scratch.path().to_path_buf();
        let ws = root.join("ws");
        fs::create_dir_all(&ws).unwrap();
        Ws { _scratch: scratch, root, ws }
    }

    /// A store that declares encoding 9 (what v0.100.0 writes).
    fn at_encoding_9(tag: &str) -> Ws {
        let w = Ws::new(tag);
        let oo = w.ws.join(".oo");
        fs::create_dir_all(&oo).unwrap();
        fs::write(oo.join("objects.format"), "encoding=9\n").unwrap();
        fs::write(oo.join("format"), "layout=9\n").unwrap();
        w
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

    /// Evolve `src`, then commit: "" when both succeed.
    fn commit(&self, src: &str) -> String {
        let (rc, o) = self.evolve(src);
        if rc != 0 {
            return format!("evolve rc={rc}: {}", o.chars().take(200).collect::<String>());
        }
        let (o, rc) = self.oo(&["commit", "-m", "a"]);
        if rc != 0 {
            return format!("commit rc={rc}: {}", squash(&o).chars().take(200).collect::<String>());
        }
        String::new()
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

    fn head(&self) -> String {
        fs::read_to_string(self.ws.join(".oo").join("HEAD")).unwrap_or_default().trim().to_string()
    }

    fn root(&self) -> String {
        let head = self.head();
        if head.is_empty() {
            return String::new();
        }
        let (o, _) = self.oo(&["inspect", &head]);
        o.lines()
            .find(|l| l.contains("root:"))
            .and_then(|l| l.rsplit(':').next())
            .unwrap_or("")
            .trim()
            .to_string()
    }

    fn decl(&self) -> String {
        fs::read_to_string(self.ws.join(".oo").join("objects.format")).unwrap_or_default().trim().to_string()
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

/// `oo eval` from an empty directory (no universe).
fn eval_here(e: &str) -> String {
    let w = Ws::new("eval");
    conflict(&w.eval(e))
}

fn conflict(got: &str) -> String {
    if got.starts_with("_|_") && got.contains("#conflict") { "_|_ #conflict".into() } else { got.into() }
}

fn run(cells: &[(&str, &str, &str)]) {
    let mut bad = Vec::new();
    for (label, e, want) in cells {
        check(&mut bad, &format!("{label}: {e}"), eval_here(e), want);
    }
    verdict(bad, cells.len());
}

/// r1 — `x & !C` is `x` outside `C` and ⊥ inside it, for every kind of `C`.
#[test]
fn r1_a_complement_is_decided_by_membership() {
    run(&[
        ("range, outside", "5 & !(..0)", "5"),
        ("range, inside", "(0 - 3) & !(..0)", "_|_ #conflict"),
        ("range, a string", "\"s\" & !(..0)", "\"s\""),
        ("int, outside", "2 & !0", "2"),
        ("int, inside", "0 & !0", "_|_ #conflict"),
        ("string", "\"b\" & !\"a\"", "\"b\""),
        ("tag", "#b & !#a", "#b"),
        ("type, inside", "5 & !@int", "_|_ #conflict"),
        ("type, inside", "\"s\" & !@str", "_|_ #conflict"),
        ("type, outside", "5 & !@str", "5"),
        ("combo, outside", "{ a: #false } & !{ a: #true }", "{ a: #false }"),
        ("combo, inside", "{ a: #true, b: 1 } & !{ a: #true }", "_|_ #conflict"),
        ("union of atoms", "2 & !(1 | 3)", "2"),
        ("a union meets member by member", "(1 | 2 | 3) & !2", "1 | 3"),
        ("a union meets member by member", "(1 | 2) & !(..1)", "2"),
        ("two complements", "5 & !(..0) & !(10..)", "5"),
        ("two complements, inside the second", "12 & !(..0) & !(10..)", "_|_ #conflict"),
        ("difference", "3 & (@int \\ @{0})", "3"),
        ("difference, inside", "0 & (@int \\ @{0})", "_|_ #conflict"),
        ("a part stays", "{ @NonZero: @int & !0, k: 5 & @NonZero }.k", "5"),
        ("a part stays, inside", "{ @NonZero: @int & !0, k: 0 & @NonZero }.k", "_|_ #conflict"),
        ("a part stays, outside the type", "{ @NonZero: @int & !0, k: \"s\" & @NonZero }.k", "_|_ #conflict"),
        ("a part of a range stays", "{ p: (-5..5) & !(..0), k: 3 & p }.k", "3"),
        ("a part of a range stays, inside", "{ p: (-5..5) & !(..0), k: -3 & p }.k", "_|_ #conflict"),
    ]);
}

/// r2 — `#true` has no default type. `!#true` is everything that is not
/// `#true`; boolean not is `@bool & !x`. SPEC_01 §2.5 examples hold.
#[test]
fn r2_true_is_a_point_not_a_type() {
    run(&[
        ("not a boolean", "5 & !#true", "5"),
        ("not a boolean", "#maybe & !#true", "#maybe"),
        ("observed", "!#true", "!#true"),
        ("boolean not", "@bool & !#true", "#false"),
        ("boolean not", "@bool & !#false", "#true"),
        ("SPEC_01 §2.5", "!#true & #false", "#false"),
        ("SPEC_01 §2.5", "!#true & (#true | #false)", "#false"),
        ("SPEC_01 §2.5", "(0 - 2) & !(@int & 1..)", "-2"),
        ("SPEC_01 §2.5, not an integer", "\"s\" & !(@int & 1..)", "\"s\""),
        ("a condition", "(!#true) ? 1 : 2", "2"),
        ("a condition", "(!#false) ? 1 : 2", "1"),
    ]);
}

/// r3 — SPEC_01 §2.5 axioms: involution, `A & !A = ⊥`, `A | !A = _`,
/// antitone, De Morgan. And the meet of complements does not depend on order.
#[test]
fn r3_the_axioms_hold() {
    run(&[
        ("involution", "!!(..0) = (..0)", "#true"),
        ("involution", "!!0 = 0", "#true"),
        ("involution", "!!#true = #true", "#true"),
        ("A & !A", "#a & !#a", "_|_ #conflict"),
        ("A & !A", "(..0) & !(..0)", "_|_ #conflict"),
        ("A | !A", "#a | !#a", "_"),
        ("A | !A", "(..0) | !(..0)", "_"),
        ("A | !A", "@int | !@int", "_"),
        ("antitone", "!(..10) <= !(..0)", "#true"),
        ("antitone", "!(..0) <= !(..10)", "#false"),
        ("antitone", "!@int <= !1", "#true"),
        ("De Morgan", "!(1 | 2) = (!1 & !2)", "#true"),
        ("De Morgan", "!(1 & @int) = (!1 | !@int)", "#true"),
        ("order", "(!0 & !1) = (!1 & !0)", "#true"),
    ]);
}

/// r4 — A complement in a table key, a type, and a condition.
#[test]
fn r4_tables_and_types_use_it() {
    let pos = "{ @{ !(..0) }: \"pos\", _: \"no\" }";
    let nz = "{ @{ @int & !0 }: \"nz\", _: \"z\" }";
    let ns = "{ @{ !@str }: \"y\", _: \"n\" }";
    run(&[
        ("outside", &format!("{pos} 12"), "\"pos\""),
        ("inside", &format!("{pos} (0 - 3)"), "\"no\""),
        ("a string is not in ..0", &format!("{pos} \"s\""), "\"pos\""),
        ("no other branch", "{ @{ !(..0) }: \"pos\" } 12", "\"pos\""),
        ("part of a type", &format!("{nz} 3"), "\"nz\""),
        ("part of a type, inside", &format!("{nz} 0"), "\"z\""),
        ("part of a type, outside the type", &format!("{nz} \"s\""), "\"z\""),
        ("a type", &format!("{ns} 2"), "\"y\""),
        ("a type, inside", &format!("{ns} \"s\""), "\"n\""),
        ("a named part of a type", "{ @NonZero: @int & !0, t: { @{ @NonZero }: \"nz\", _: \"z\" }, r: t 0 }.r", "\"z\""),
        ("a named part of a type", "{ @NonZero: @int & !0, t: { @{ @NonZero }: \"nz\", _: \"z\" }, r: t 3 }.r", "\"nz\""),
    ]);
    let got = eval_here("{ @{ !(..0) }: \"pos\" } (0 - 3)");
    let mut bad = Vec::new();
    check(&mut bad, "inside, no other branch", got.contains("#no_matching_branch").to_string(), "true");
    verdict(bad, 1);
}

const DURABLE: &str = "n: !(..0)\nz: @int & !0\nb: !#true\nt: { @{ !(..0) }: \"pos\", _: \"no\" }\nk: { v: !0 }\n";

/// r5 — Complements are committed and read back. New stores declare
/// encoding 10; `n: !0` has one root.
#[test]
fn r5_complements_commit_and_read_back() {
    let mut bad = Vec::new();
    let w = Ws::new("r5");
    check(&mut bad, "commit", w.commit(DURABLE), "");
    check(&mut bad, "declares", w.decl(), "encoding=10");
    for (e, want) in [
        ("5 & _.n", "5"),
        ("(0 - 1) & _.n", "_|_ #conflict"),
        ("5 & _.z", "5"),
        ("0 & _.z", "_|_ #conflict"),
        ("_.b", "!#true"),
        ("@bool & _.b", "#false"),
        ("_.t 12", "\"pos\""),
        ("_.t (0 - 3)", "\"no\""),
        ("_.k", "{ v: !0 }"),
        ("_.k.v = !0", "#true"),
    ] {
        check(&mut bad, e, conflict(&w.eval(e)), want);
    }
    let w = Ws::new("r5-root");
    check(&mut bad, "n: !0 commit", w.commit("n: !0\n"), "");
    check(&mut bad, "n: !0 root", w.root(), "24fb6a7619d84f718af10ba9ff93f2939cf9d56796b4e0ab72ccc0d929604f42");
    verdict(bad, 2 + 10 + 2);
}

/// r6 — A store that declares encoding 9 refuses a complement by name and
/// writes nothing; `oo migrate` names v0.100.0 and the write lands after it.
#[test]
fn r6_a_store_below_10_refuses_by_name() {
    let mut bad = Vec::new();
    let w = Ws::at_encoding_9("r6");
    check(&mut bad, "before: commit", w.commit("a: 1\n"), "");
    let head = w.head();
    let (rc, o) = w.evolve("n: !0\n");
    check(&mut bad, "evolve rc", rc.to_string(), "1");
    check(&mut bad, "names encoding 10", o.contains("encoding=10").to_string(), "true");
    check(&mut bad, "names migrate", o.contains("oo migrate").to_string(), "true");
    let (o, _) = w.oo(&["status"]);
    check(&mut bad, "nothing staged", o.contains("no staged changes").to_string(), "true");
    check(&mut bad, "head unchanged", (w.head() == head).to_string(), "true");
    check(&mut bad, "still declares", w.decl(), "encoding=9");
    let (o, rc) = w.oo(&["migrate", "--grant", "migrate"]);
    check(&mut bad, "migrate rc", rc.to_string(), "0");
    check(&mut bad, "migrate names the lockout", o.contains("unopenable by oo v0.100.0").to_string(), "true");
    check(&mut bad, "migrated declares", w.decl(), "encoding=10");
    check(&mut bad, "after migrate: _.a", w.eval("_.a"), "1");
    check(&mut bad, "after migrate: commit", w.commit("n: !0\n"), "");
    check(&mut bad, "after migrate: 5 & _.n", w.eval("5 & _.n"), "5");
    verdict(bad, 13);
}

/// g1 — Roots that hold no complement do not move (v0.100.0), whether the
/// store declares 10 or 9.
const UNTOUCHED: [(&str, &str); 4] = [
    ("x: 0\n", "b74d775816f6b5be6cc152577b7556f6c8fdcc63433b0e31068cc2484a94ae0f"),
    ("t: { a: 1, b: [1, 2] }\n", "51a571b4dc23426ee0687213bfb9b4f3d899bcee3731d716d80541775d3c9274"),
    ("b: #true\n", "b5f945e297b460505da459a5d015ba82ff45b5356b0db0363aab56892fdda6fc"),
    ("r: 1..3\n", "06254b6688a8cb61d2baa10793b1d2ae56815c87812b535fd71ae30f78acbc6d"),
];

#[test]
fn g1_roots_without_complements_do_not_move() {
    let mut bad = Vec::new();
    for (src, root) in UNTOUCHED {
        for at9 in [false, true] {
            let w = if at9 { Ws::at_encoding_9("g1-9") } else { Ws::new("g1") };
            let label = format!("{}{}", src.trim(), if at9 { " (encoding 9)" } else { "" });
            check(&mut bad, &format!("{label}: commit"), w.commit(src), "");
            check(&mut bad, &format!("{label}: root"), w.root(), root);
        }
    }
    verdict(bad, UNTOUCHED.len() * 4);
}

/// g2 — What already had an answer keeps it (green today).
#[test]
fn g2_what_had_an_answer_keeps_it() {
    run(&[
        ("top", "!_", "_|_ #conflict"),
        ("bottom", "!_|_", "_"),
        ("a condition", "#true ? 1 : 2", "1"),
        ("a condition", "#false ? 1 : 2", "2"),
        ("L1-16", "5 & !(..0)", "5"),
        ("L1-17", "!#true & #false", "#false"),
        ("a range", "5 & (..0)", "_|_ #conflict"),
        ("a type", "5 & @int", "5"),
    ]);
}
