// A table that runs every branch when looked at.
// Order: nlang-tools/docs/a_table_that_runs_every_branch_when_looked_at_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-087.  Ruling: STATUS D109 (a
// branch body that needs the input `$` or has an effect is code until its
// branch is chosen; a closed pure body may be evaluated early and is
// identified by its value; several bodies of one branch print one row each).
// Spec: SPEC_07 §1 ("a body is evaluated only when dispatch chooses its
// branch"; one normal form, D101), SYNTAX_12 §4 #5 (`$` in a branch value is
// the matched input), SYNTAX_02 §4 #12 (what is printed reads back).
//
// ── What is wrong (v0.88.0; v0.78.0 the same) ────────────────────────────
//
// Observing a table forces every branch body as a value:
//   * an unchosen branch's effect runs on `oo eval`, `=`, `run --observe`
//     and `oo commit` (applying the table does not run it);
//   * commit freezes the body: `k: { @{ @int }: $ * 10 }` then `_.k 5`
//     ⟹ ⊥ #no_context (50 in place); an effect body is stored as its result;
//     `$ + 1` and `$ * 2` commit to one root;
//   * `{ @{ @int }: $ + 1 } = { @{ @int }: $ * 2 }` ⟹ #true;
//   * a `$` body prints as a forced ⊥; several bodies print joined by ` & `;
//   * `({ @{ @int }: 1 } & { @{ @int }: 2 }) = { @{ @int }: 1, @{ @int }: 2 }`
//     ⟹ #false (D101).
//
// The invariants: looking at a table runs no branch (r1); commit keeps what
// it was given (r2); `=` does not call two different functions equal, and
// the same table built two ways is equal (r3); a printed table reads back
// as itself (r4); several bodies print one row each (r5, Q2 of the ruling).
// Guard g1 holds Q1 of the ruling: a closed pure body is still a value, so
// those tables keep their roots and `1 + 1` still equals `2`.
//
// Not here: a field's effect running at commit (that is observation, CbO);
// `$` in a plain field (it means the outer evolution); the `#io` note a
// sibling's effect leaves on `_.x` (v0.88.0 the same).
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
        let scratch = nlang_interpreter::ScratchDir::new(&format!("branch-run-{tag}"));
        let root = scratch.path().to_path_buf();
        let ws = root.join("ws");
        fs::create_dir_all(&ws).unwrap();
        Ws { _scratch: scratch, root, ws }
    }

    /// Run `oo`, killing it after 60 s.
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
            if start.elapsed() > Duration::from_secs(60) {
                let _ = child.kill();
                let _ = child.wait();
                return (format!("TIMEOUT after 60 s: oo {}", args.join(" ")), -1);
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let o = child.wait_with_output().expect("output");
        (
            format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)).trim().to_string(),
            o.status.code().unwrap_or(-1),
        )
    }

    /// `oo eval` as printed, the trailing effect note dropped.
    fn eval_raw(&self, e: &str) -> String {
        let s = self.oo(&["eval", e]).0;
        match s.find(";; %effect") {
            Some(i) => s[..i].trim().to_string(),
            None => s,
        }
    }

    /// `oo eval`, whitespace collapsed.
    fn eval(&self, e: &str) -> String {
        squash(&self.eval_raw(e))
    }

    /// Evolve each source in turn, then commit. Every step must succeed: a
    /// reading taken after a failed commit is a reading of the previous
    /// universe.
    fn commit(&self, srcs: &[&str]) {
        for (i, src) in srcs.iter().enumerate() {
            let f = format!("f{i}.n");
            fs::write(self.ws.join(&f), src).unwrap();
            let (o, rc) = self.oo(&["evolve", &f]);
            assert_eq!(rc, 0, "evolve {src:?}: {o}");
        }
        let (o, rc) = self.oo(&["commit", "-m", "a"]);
        assert_eq!(rc, 0, "commit {srcs:?}: {o}");
    }

    /// The root digest of HEAD.
    fn root(&self) -> String {
        let head = fs::read_to_string(self.ws.join(".oo").join("HEAD")).unwrap_or_default();
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

/// Each `(expr, want)` evaluated in one fresh workspace with no universe.
fn answers(tag: &str, cases: &[(&str, &str)]) {
    let w = Ws::new(tag);
    let mut bad = Vec::new();
    for (e, want) in cases {
        let got = w.eval(e);
        if got != *want {
            bad.push(format!("  {e}\n    want {want}\n    got  {got}"));
        }
    }
    assert!(bad.is_empty(), "{} of {} wrong:\n{}", bad.len(), cases.len(), bad.join("\n"));
}

const WRITE: &str = "~%Io./write_file (\"q.txt\", \"x\")";

/// Run `f` and report whether it wrote `q.txt` (then remove it).
fn wrote(w: &Ws, f: impl FnOnce()) -> bool {
    let _ = fs::remove_file(w.ws.join("q.txt"));
    f();
    let did = w.ws.join("q.txt").exists();
    let _ = fs::remove_file(w.ws.join("q.txt"));
    did
}

// ── Red ──────────────────────────────────────────────────────────────────

/// SPEC_07 §1: a body is evaluated only when dispatch chooses its branch.
/// v0.88.0: eval, `=`, `run --observe` and commit write the file.
#[test]
fn r1_looking_at_a_table_runs_no_branch() {
    let w = Ws::new("r1");
    let t = format!("{{ @{{ @int }}: 1, _: {WRITE} }}");
    let src = format!("k: {t}\n");
    fs::write(w.ws.join("a.n"), &src).unwrap();
    let mut bad = Vec::new();
    for (name, run) in [
        ("eval", vec!["eval".to_string(), t.clone()]),
        ("=", vec!["eval".to_string(), format!("{t} = {{ @{{ @int }}: 1, _: 0 }}")]),
        ("run --observe", vec!["run".to_string(), "--observe".to_string(), "k".to_string(), "a.n".to_string()]),
        ("evolve", vec!["evolve".to_string(), "a.n".to_string()]),
        ("status", vec!["status".to_string()]),
        ("commit", vec!["commit".to_string(), "-m".to_string(), "a".to_string()]),
        ("_.k 5", vec!["eval".to_string(), "_.k 5".to_string()]),
    ] {
        let args: Vec<&str> = run.iter().map(|s| s.as_str()).collect();
        if wrote(&w, || { let _ = w.oo(&args); }) {
            bad.push(name);
        }
    }
    // Control, right on v0.88.0: applying the table in place to an input
    // that chooses the default branch runs it.
    let chose = wrote(&w, || { let _ = w.oo(&["eval", &format!("({t}) \"s\"")]); });
    assert!(bad.is_empty() && chose, "ran a branch nobody chose: {bad:?}; chosen branch ran: {chose}");
}

/// Commit keeps a deferred body as code. v0.88.0: `_.m 5` ⟹ ⊥ #no_context,
/// the effect body is stored as `#true`, and the two functions share a root.
#[test]
fn r2_commit_keeps_what_it_was_given() {
    let w = Ws::new("r2");
    w.commit(&[&format!("m: {{ @{{ @int }}: $ * 10 }}\nk: {{ @{{ @int }}: 1, _: {WRITE} }}\ns: {{ @{{ @str }}: 1, _: $ + 1 }}\n")]);
    let mut bad = Vec::new();
    for (e, want) in [("_.m 5", "50"), ("_.s 5", "6"), ("_.s \"a\"", "1")] {
        let got = w.eval(e);
        if got != want {
            bad.push(format!("{e}: want {want} got {got}"));
        }
    }
    if !wrote(&w, || { let _ = w.eval("_.k \"s\""); }) {
        bad.push("`_.k \"s\"` after commit did not write: the body was frozen".to_string());
    }
    let mut roots = Vec::new();
    for (i, body) in ["$ + 1", "$ * 2"].iter().enumerate() {
        let v = Ws::new(&format!("r2r{i}"));
        v.commit(&[&format!("k: {{ @{{ @int }}: {body} }}\n")]);
        roots.push(v.root());
    }
    if roots[0] == roots[1] {
        bad.push(format!("`$ + 1` and `$ * 2` share root {}", &roots[0][..12]));
    }
    assert!(bad.is_empty(), "{bad:?}");
}

/// `=` on tables: two different functions are not equal; the same table
/// built two ways is (D101). v0.88.0: the first is #true, the third #false.
#[test]
fn r3_equal_tables_answer_alike() {
    answers("r3", &[
        ("{ @{ @int }: $ + 1 } = { @{ @int }: $ * 2 }", "#false"),
        ("({ @{ @int }: $ + 1 } & { @{ @int }: $ * 2 }) = { @{ @int }: $ + 1, @{ @int }: $ * 2 }", "#true"),
        ("({ @{ @int }: 1 } & { @{ @int }: 2 }) = { @{ @int }: 1, @{ @int }: 2 }", "#true"),
        ("{ ...{ @{ @int }: $ + 1 }, _: $ } = { @{ @int }: $ + 1, _: $ }", "#true"),
        // Controls, right on v0.88.0.
        ("{ @{ @int }: $ + 1 } = { @{ @int }: $ + 1 }", "#true"),
        ("({ @{ @int }: $ + 1 }) 5", "6"),
        ("({ @{ @int }: $ + 1 } & { @{ @int }: $ * 2 }) 5", "_|_ ;; %cause: #conflict ;; Incompatible types: 10 vs 6"),
    ]);
}

/// SYNTAX_02 §4 #12: a printed table reads back as itself.
#[test]
fn r4_a_printed_table_reads_back() {
    let w = Ws::new("r4");
    let tables = [
        "{ @{ @int }: $ + 1, @{ @str }: \"s\" }",
        "{ @{ @int }: $ + 1 } & { @{ @int }: $ * 2 }",
        "{ @{ 4 }: \"f\", @{ 4 }: \"g\" }",
        "{ @{ @int }: 1 } & { @{ @int }: 1 | 2 }",
        "{ @{ 4 }: 1 + 1, _: $ }",
        "{ @{ @int }: { b: $ } }",
        "{ k: 3, t: { @{ @int }: $ + k } }",
        "{ ...{ @{ @int }: $ + 1 }, _: $ }",
        "{ t: { @{ @int }: $ * 10 } }",
        // Control, right on v0.88.0.
        "{ @{ @int }: 1, _: 0 }",
    ];
    let mut bad = Vec::new();
    for t in tables {
        let printed = w.eval_raw(t);
        let back = w.eval(&format!("({t}) = ({printed})"));
        if back != "#true" {
            bad.push(format!("  {t}\n    printed {}\n    read back = {back}", squash(&printed)));
        }
    }
    let printed = w.eval("{ @{ @int }: $ + 1 }");
    if printed != "{ @{ @int }: $ + 1 }" {
        bad.push(format!("  a `$` body prints as {printed}"));
    }
    assert!(bad.is_empty(), "{} wrong:\n{}", bad.len(), bad.join("\n"));
}

/// D109 Q2: several bodies of one branch print one row each, the key
/// repeated. Which body comes first is not fixed (spelling is not
/// normative, D64). v0.88.0: joined by ` & `.
#[test]
fn r5_several_bodies_print_one_row_each() {
    let w = Ws::new("r5");
    let mut bad = Vec::new();
    for (t, rows) in [
        // AMENDED 2026-10-23 for Q-101 (D117 Q2): value bodies of one branch
        // meet into one body; a set (one row each) is of deferred bodies.
        ("{ @{ 4 }: $ + 1, @{ 4 }: $ + 2 }", vec!["@{ 4 }: $ + 1", "@{ 4 }: $ + 2"]),
        ("{ @{ @int }: $ + 1, @{ @int }: $ * 2, @{ @str }: \"s\" }", vec!["@{ @int }: $ + 1", "@{ @int }: $ * 2", "@{ @str }: \"s\""]),
    ] {
        let printed = w.eval_raw(t);
        let lines: Vec<String> = printed.lines().map(|l| l.trim().to_string()).collect();
        let missing: Vec<&&str> = rows.iter().filter(|r| !lines.iter().any(|l| l == **r)).collect();
        if !missing.is_empty() || printed.contains(" & ") {
            bad.push(format!("  {t}\n    printed {}\n    missing rows {missing:?}", squash(&printed)));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

// ── Guard (green on v0.88.0) ─────────────────────────────────────────────

/// D109 Q1: a closed pure body is a value. `1 + 1` equals `2`, prints as
/// `2`, and tables made only of such bodies keep their roots.
#[test]
fn g1_a_closed_pure_body_is_a_value() {
    answers("g1", &[
        ("{ @{ 4 }: 1 + 1 } = { @{ 4 }: 2 }", "#true"),
        ("{ @{ @int }: 1 + 1, @{ @str }: 0 }", "{ @{ @int }: 2 @{ @str }: 0 }"),
    ]);
    for (src, root) in [
        ("k: { @{ 4 }: \"f\", _: \"z\" }\n", "43480b6b8fce1ec0c971f2d6d956b33f2b971371364d65006903ffd67ef41fe9"),
        ("k: { t: { @{ @int }: 1, _: 0 } }\n", "275bc55ddc3801d21b306de8cb06f1d9a8d455aea7bed823ab889975620d559f"),
    ] {
        let w = Ws::new("g1c");
        w.commit(&[src]);
        assert_eq!(w.root(), root, "{src}");
    }
}

// ── Acceptance (acceptor, 2026-10-14): R-1 ───────────────────────────────

/// I1 holds when the effect is not predicted. A body that calls an arrow
/// with an effect — a sibling by name, or one written in place — is
/// predicted pure, so delivery 92ce296 still forces it on observation and
/// commit. The ruling's text said "predicted" (the acceptor's words); the
/// invariant is that a branch nobody chose does not run.
/// v0.88.0 red, reference q087-ref 9e26e11 red, delivery 92ce296 red.
#[test]
fn r6_an_effect_nobody_predicted_does_not_run_either() {
    let w = Ws::new("r6");
    let sib = format!("{{ f: (x -> {WRITE}), t: {{ @{{ @int }}: 1, _: f 1 }} }}.t");
    let inl = format!("{{ t: {{ @{{ @int }}: 1, _: (x -> {WRITE}) 0 }} }}.t");
    let mut bad = Vec::new();
    for (name, e) in [
        ("eval, sibling arrow", sib.clone()),
        ("eval, arrow in place", inl.clone()),
        ("=, sibling arrow", format!("({sib}) = {{ @{{ @int }}: 1, _: 0 }}")),
    ] {
        if wrote(&w, || { let _ = w.oo(&["eval", &e]); }) {
            bad.push(name.to_string());
        }
    }
    fs::write(w.ws.join("b.n"), format!("f: (x -> {WRITE})\nk: {{ @{{ @int }}: 1, _: f 1 }}\n")).unwrap();
    let (o, rc) = w.oo(&["evolve", "b.n"]);
    assert_eq!(rc, 0, "evolve: {o}");
    if wrote(&w, || { let (o, rc) = w.oo(&["commit", "-m", "b"]); assert_eq!(rc, 0, "commit: {o}"); }) {
        bad.push("commit".to_string());
    }
    // Controls: the branch, once chosen, runs — in place and after commit.
    let chose_in_place = wrote(&w, || { let _ = w.oo(&["eval", &format!("({sib}) \"s\"")]); });
    let chose_after = wrote(&w, || { let _ = w.eval("_.k \"s\""); });
    assert!(bad.is_empty() && chose_in_place && chose_after,
        "ran a branch nobody chose: {bad:?}; chosen in place ran: {chose_in_place}; chosen after commit ran: {chose_after}");
}
