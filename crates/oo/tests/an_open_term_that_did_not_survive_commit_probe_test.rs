// An open term that did not survive commit.
// Order: nlang-tools/docs/an_open_term_that_did_not_survive_commit_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-088.  No ruling needed: SPEC_07
// §4.2 P3 (an expression with a free `$` is an open term: it may be defined
// and stored, and does not collapse unless observed; observed with no
// enclosing evolution it is ⊥ #no_context; its identity is its syntax), P1
// (`|>` is where `$` binds), GUIDE_03 §6.1 ("eager evaluation collapses the
// open term during evolution — what is stored is _|_, not the open term"),
// §11.2 (a thunk on the right of a pipe carries that pipe's binding).
//
// ── What is wrong (v0.89.0; v0.78.0 the same) ────────────────────────────
//
// A template stored in a universe is frozen at commit:
//   `t: { a: $.b + 1 }`, committed, then `{ b: 1 } |> _.t`
//     ⟹ `{ a: _|_ #no_context, b: 1 }`   (in place: `{ a: 2, b: 1 }`)
// so two different templates evolve to equal values. Two causes: commit
// observes the open term with no enclosing evolution and stores the ⊥;
// and a stored template reaching the right of `|>` carries no binding, so
// its open terms are observed after the pipe, again with none.
//
// The invariants: a stored template evolves as the written one does (r1,
// r2); two different templates evolve to different values (r3); a table's
// bodies and an observation with no evolution keep their own `$` (r4).
// Guard: values with no open term keep their roots (g1).
//
// Not here: `<<_.>>` read through `oo eval` resolves to the live root,
// which holds the eval's own field (SPEC_07 §4.2.2's `v.w.x.a` after
// commit; another Inbox row). `=` on two open terms observed with no
// evolution (both ⊥, #true) is P3's observation — the Inbox row on `=`
// and Thunk identity.
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
        let scratch = nlang_interpreter::ScratchDir::new(&format!("open-term-{tag}"));
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

/// Commit `src`, then evaluate each `(expr, want)`.
fn after_commit(tag: &str, src: &str, cases: &[(&str, &str)]) -> Vec<String> {
    let w = Ws::new(tag);
    w.commit(&[src]);
    let mut bad = Vec::new();
    for (e, want) in cases {
        let got = w.eval(e);
        if got != *want {
            bad.push(format!("  [{src}] {e}\n    want {want}\n    got  {got}"));
        }
    }
    bad
}

// ── Red ──────────────────────────────────────────────────────────────────

/// A stored template whose open term was frozen at commit. v0.89.0:
/// `{ a: _|_ … b: 1 }`.
#[test]
fn r1_a_stored_template_evolves_as_written() {
    let mut bad = after_commit("r1", "t: { a: $.b + 1 }\n", &[
        ("{ b: 1 } |> _.t", "{ a: 2 b: 1 }"),
        ("{ b: 5 } |> _.t", "{ a: 6 b: 5 }"),
    ]);
    // Control, right on v0.89.0: the same template written in place.
    bad.extend(after_commit("r1c", "x: 0\n", &[("{ b: 1 } |> { a: $.b + 1 }", "{ a: 2 b: 1 }")]));
    assert!(bad.is_empty(), "\n{}", bad.join("\n"));
}

/// A stored template whose open terms sit in a list and a nested record:
/// stored intact, but carrying no binding to the right of `|>`. v0.89.0:
/// the list element is ⊥ after the pipe.
#[test]
fn r2_a_stored_template_binds_at_the_pipe() {
    let bad = after_commit("r2", "t: { l: [$.b, 2], n: { c: $.b * 10 } }\n", &[
        ("{ b: 1 } |> _.t", "{ b: 1 l: [1, 2] n: { c: 10 } }"),
    ]);
    assert!(bad.is_empty(), "\n{}", bad.join("\n"));
}

/// Two different templates evolve to different values. v0.89.0: both
/// freeze to ⊥ and compare equal.
#[test]
fn r3_two_templates_are_two() {
    let bad = after_commit("r3", "t: { a: $.b + 1 }\nu: { a: $.b * 2 }\n", &[
        ("({ b: 3 } |> _.t) = ({ b: 3 } |> _.u)", "#false"),
        ("({ b: 3 } |> _.t).a", "4"),
        ("({ b: 3 } |> _.u).a", "6"),
    ]);
    assert!(bad.is_empty(), "\n{}", bad.join("\n"));
}

/// `$` in a table body is the matched input (D109), not the pipe's; with no
/// evolution an open term observes as ⊥ #no_context (P3). v0.89.0: red only
/// on `.a` (the stored `a` froze); the table cell and the observations with
/// no evolution are controls, green on v0.89.0. The cause is compared, not
/// the wording of its message.
#[test]
fn r4_each_dollar_keeps_its_own_binding() {
    let mut bad = after_commit("r4", "t: { a: $.b + 1, k: { @{ @int }: $ + 100 } }\n", &[
        ("({ b: 1 } |> _.t).a", "2"),
        // Control, right on v0.89.0.
        ("(({ b: 1 } |> _.t).k) 5", "105"),
    ]);
    let w = Ws::new("r4o");
    w.commit(&["t: { a: $.b + 1 }\n"]);
    for e in ["_.t.a", "{ a: $ + 1 }.a"] {
        let got = w.eval(e);
        if !got.starts_with("_|_ ;; %cause: #no_context") {
            bad.push(format!("  {e} observed with no evolution: {got}"));
        }
    }
    assert!(bad.is_empty(), "\n{}", bad.join("\n"));
}

// ── Guard (green on v0.89.0) ─────────────────────────────────────────────

/// Values with no open term keep their roots, and a definition that is ⊥
/// for a reason of its own is still stored as that ⊥ (D33/D44: a divergent
/// definition must say why). The fix is about open terms, not about ⊥.
#[test]
fn g1_values_without_open_terms_keep_their_roots() {
    for (src, root) in [
        ("c: c + 1\n", "3f0a62e4ac00a7517e258bee6afbc348f33167cb1cd42e9bcefa419645c2da34"),
        ("k: { a: 1 / 0 }\n", "889b4174eb7a87bb8452197bc0a7247e6cd0d86d2b84a63e5ede00f87f1a15f2"),
        ("k: { a: 1 + 1 }\n", "9445101c4b15c42562daed770b5d153f554dee67af0fdb9c8be8325247d111b7"),
        ("k: { @{ @int }: $ + 1 }\n", "37026f7d8c3572710a84e93e3c4c81f367408b86986d8747a202bdbc3189ad2d"),
        ("u: { b: 1 } |> { a: $.b + 1 }\n", "d3de86f4352682c5a44e1081994cf0b36290a0c37825664c06c06f7904a8f4ed"),
    ] {
        let w = Ws::new("g1");
        w.commit(&[src]);
        let r = w.root();
        assert_eq!(r, root, "{src}");
    }
}

// ── Red, added at acceptance (R-1) ───────────────────────────────────────

/// I1 at any depth: a stored open term reached inside an evolution takes
/// that evolution's `$` wherever it is reached — not only as the whole
/// right side of `|>`. Each cell is paired with the same template defined
/// in place and reached by name, which is right on v0.89.0 and on the
/// first delivery; the stored one must give the same answer. P3: "observed,
/// it is bound by the nearest enclosing evolution"; SPEC_07 §4.2.2's own
/// example binds an open term nested in the right side (`v.w.x.a`).
/// v0.89.0 and `4b0c765`: every stored cell ⊥ #no_context.
#[test]
fn r5_a_stored_open_term_binds_wherever_it_is_reached() {
    let mut bad = Vec::new();
    // A bare open term, not a record: it collapsed at evolve, before commit.
    let bare = [("{ b: 1 } |> _.t", "{ b: 1 } |> t", "2")];
    // A record template, reached in a field, as an arrow's result, and as a
    // table body. Not here: `<<…>>` on the right of `|>` stays a live
    // reference whose `$` is bound when it is dereferenced (Stage 3, F1) —
    // the Inbox row on `<<_.>>`.
    let rec = [
        ("{ b: 1 } |> { r: _.t }", "{ b: 1 } |> { r: t }", "{ b: 1 r: { a: 2 } }"),
        ("{ b: 1 } |> (x -> _.t)", "{ b: 1 } |> (x -> t)", "{ a: 2 }"),
    ];
    for (def, cells) in [("$.b + 1", &bare[..]), ("{ a: $.b + 1 }", &rec[..])] {
        let src = format!("t: {def}\n");
        for (stored, in_place, want) in cells {
            // Control, right on v0.89.0.
            let e = format!("{{ t: {def}, r: {in_place} }}.r");
            bad.extend(after_commit("r5c", "x: 0\n", &[(e.as_str(), *want)]));
            bad.extend(after_commit("r5", &src, &[(*stored, *want)]));
        }
    }
    // Reached by a path of more than one segment, in an arrow's body.
    bad.extend(after_commit("r5c", "x: 0\n", &[("{ n: { t: { a: $.b + 1 } }, q: { b: 1 } |> (x -> n.t) }.q", "{ a: 2 }")]));
    bad.extend(after_commit("r5", "n: { t: { a: $.b + 1 } }\n", &[("{ b: 1 } |> (x -> n.t)", "{ a: 2 }")]));
    bad.extend(after_commit("r5c", "x: 0\n", &[("{ t: { a: $ + 1 }, k: { @{ @int }: t } }.k 5", "{ a: 6 }")]));
    bad.extend(after_commit("r5", "t: { a: $ + 1 }\nk: { @{ @int }: t }\n", &[("_.k 5", "{ a: 6 }")]));
    assert!(bad.is_empty(), "\n{}", bad.join("\n"));
}

// ── Red, added at R-1 acceptance (R-2) ───────────────────────────────────

/// What an evolution is given passes through it unchanged: a template that
/// is the input `$` or an arrow's argument is data there, not reached there,
/// and is not bound to itself. `(x -> x) v` is `v`. Each cell is paired with
/// the same template defined in place and reached by name (right on
/// v0.89.0). v0.89.0: the stored cells ⊥ #no_context; `4b0c765`: right;
/// `118b149`: `{ a: _ b: 1 }` — the template was bound to itself on the way
/// through. With no evolution around it, given data still observes ⊥ — an
/// arrow's argument is not reached by the table the arrow returns.
#[test]
fn r6_a_template_passed_through_is_not_bound_to_itself() {
    let mut bad = Vec::new();
    let def = "{ a: $.b + 1 }";
    for (stored, in_place) in [
        ("{ b: 1 } |> (_.t |> (x -> x))", "{ b: 1 } |> (t |> (x -> x))"),
        ("{ b: 1 } |> (_.t |> { r: $ }).r", "{ b: 1 } |> (t |> { r: $ }).r"),
        ("{ b: 1 } |> ((x -> x) _.t)", "{ b: 1 } |> ((x -> x) t)"),
    ] {
        // Control, right on v0.89.0.
        let e = format!("{{ t: {def}, r: {in_place} }}.r");
        bad.extend(after_commit("r6c", "x: 0\n", &[(e.as_str(), "{ a: 2 b: 1 }")]));
        bad.extend(after_commit("r6", "t: { a: $.b + 1 }\n", &[(stored, "{ a: 2 b: 1 }")]));
    }
    // With no evolution around it, given data observes ⊥ — stored, and
    // written in place (code that never touched the store; right on v0.89.0
    // and `4b0c765`, `{ a: _ }` / `_` on `118b149`).
    let w = Ws::new("r6o");
    w.commit(&["t: { a: $.b + 1 }\n"]);
    for (e, want) in [
        ("(_.t |> (x -> x)).a", "_|_ ;; %cause: #no_context"),
        ("((x -> { @{ @int }: x }) _.t) 5", "{ a: _|_ ;; %cause: #no_context"),
        ("{ t: { a: $.b + 1 }, r: (t |> (x -> x)).a }.r", "_|_ ;; %cause: #no_context"),
        ("{ t: { a: $.b + 1 }, r: ((x -> { @{ @int }: x }) t) 5 }.r", "{ a: _|_ ;; %cause: #no_context"),
    ] {
        let got = w.eval(e);
        if !got.starts_with(want) {
            bad.push(format!("  {e} observed with no evolution: {got}"));
        }
    }
    assert!(bad.is_empty(), "\n{}", bad.join("\n"));
}
