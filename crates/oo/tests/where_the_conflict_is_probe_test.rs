// Where the conflict is (2026-08-08, pre-committed by work order:
// docs/where_the_conflict_is_handover.md).
//
// ── What is on the floor ─────────────────────────────────────────────────
//
// Measured on v0.11.1 (`dev f0ecb21`). A workspace holding
//
//     app: { db: { host: "h", port: 5432, opts: { tls: true, retries: 3 } } }
//
// evolved against a source that differs in `retries` alone:
//
//     Error: Evolution Conflict in "u.n": Conflict at
//       Path(Path { anchor: Bare, segments: ["app"], span: Span { start: 0, end: 3 } })
//
// The contradiction is at `app.db.opts.retries` — four levels down, one leaf
// out of six. The message says `["app"]`, wrapped in a Debug-printed internal
// struct, byte offsets and all.
//
// And the REPL says less:
//
//     Evolution Conflict: Conflict
//
// ── The engine already knows the answer ──────────────────────────────────
//
// `unify_combo` accumulates the path on the way out of the recursion. Calling
// the exact merge `Universe::evolve` performs — `engine.unify(Combo(staged),
// Combo(incoming))` — and reading the Bottom it returns:
//
//     deep     cause=Conflict path=Some("app.db.opts.retries")
//     shallow  cause=Conflict path=Some("x")
//
// Absolute, to the leaf, both cases. It is thrown away one line later:
//
//     universe.rs:396   Value::Bottom(d) => Err(d.cause),
//     universe.rs:495   Value::Bottom(d) => Err(d.cause),
//
// `d` is in hand. Only `.cause` is taken, and `BottomCause` is a payload-free
// enum. What the operator then sees is `f.key` — the field they were typing.
// Not a shorter coordinate: a different thing entirely.
//
// ── What these probes are not ────────────────────────────────────────────
//
// Not the canonical form. `Bottom::to_nlang` also drops the path, and that one
// is out of scope on purpose — it is the canonical text of a *value*, and fmt
// v2 has been frozen since v0.2.0. A diagnostic message and a canonical form
// are different things and this arc only touches the first.
//
// Not the commit boundary either (W3′-b, blocked on W12): merge-induced ⊥ is
// caught here, at evolve, and never reaches a commit.
//
// ── The control that matters ─────────────────────────────────────────────
//
// C2/P3 hold the other half of D36: a refinement that succeeds says *nothing*,
// and that silence is a guarantee ("there is no obligation"), not an omission.
// A delivery that makes the engine chattier to be "consistent" breaks the
// guarantee this arc exists to complete.

use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

// ── harness ─────────────────────────────────────────────────────────────

fn fresh_dir(tag: &str) -> nlang_interpreter::ScratchDir {
    nlang_interpreter::ScratchDir::new(&format!("conflict-{tag}"))
}

fn oo_raw(dir: &Path, args: &[&str]) -> (String, bool) {
    let out = Command::new(env!("CARGO_BIN_EXE_oo"))
        .args(args)
        .current_dir(dir)
        .env("OO_IDENTITY", dir.join("identity-for-tests"))
        .env("OO_NODE_HOME", dir.join("node-home-for-tests"))
        .output()
        .unwrap();
    (
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout).trim(),
            String::from_utf8_lossy(&out.stderr).trim()
        ),
        out.status.success(),
    )
}

fn oo(dir: &Path, args: &[&str]) -> String {
    oo_raw(dir, args).0
}

/// Drive the REPL by feeding it lines on stdin.
fn oo_repl(dir: &Path, lines: &str) -> String {
    let mut child = Command::new(env!("CARGO_BIN_EXE_oo"))
        .arg("repl")
        .current_dir(dir)
        .env("OO_IDENTITY", dir.join("identity-for-tests"))
        .env("OO_NODE_HOME", dir.join("node-home-for-tests"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(lines.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

fn write(dir: &Path, src: &str) {
    fs::write(dir.join("u.n"), src).unwrap();
}

const DEEP_A: &str = "app: { db: { host: \"h\", port: 5432, opts: { tls: true, retries: 3 } } }\n";
const DEEP_B: &str = "app: { db: { host: \"h\", port: 5432, opts: { tls: true, retries: 9 } } }\n";
const DEEP_COORD: &str = "app.db.opts.retries";

/// A workspace with `DEEP_A` committed, ready to be evolved against `DEEP_B`.
fn repo_with_deep_value(tag: &str) -> nlang_interpreter::ScratchDir {
    let d = fresh_dir(tag);
    write(&d, DEEP_A);
    let out = oo(&d, &["evolve", "u.n"]);
    assert!(
        out.is_empty(),
        "LIVENESS: the first evolve was not clean: {out}"
    );
    let out = oo(&d, &["commit", "-m", "base"]);
    assert!(
        out.contains("hash:"),
        "LIVENESS: base did not commit: {out}"
    );
    d
}

/// Rust internals that must never reach an operator.
const DEBRIS: [&str; 5] = ["segments:", "Span", "start:", "Path(Path", "anchor:"];

fn debris_in(msg: &str) -> Vec<&'static str> {
    DEBRIS.iter().copied().filter(|d| msg.contains(d)).collect()
}

// ════════════════════════════════════════════════════════════════════════
//  CONTROL — green before and after
// ════════════════════════════════════════════════════════════════════════

/// C1 — a conflict is reported at all, and evolve fails.
///
/// If reporting were broken outright, every red below would "pass" by saying
/// nothing at all. This is what tells those two readings apart.
#[test]
fn c1_a_conflict_is_reported_and_evolve_fails() {
    let d = fresh_dir("c1");
    write(&d, "x: 1\n");
    oo(&d, &["evolve", "u.n"]);
    oo(&d, &["commit", "-m", "base"]);

    write(&d, "x: 2\n");
    let (out, ok) = oo_raw(&d, &["evolve", "u.n"]);
    assert!(!ok, "a contradicting evolve succeeded: {out}");
    assert!(
        out.contains("Evolution Conflict"),
        "a contradicting evolve said nothing recognisable: {out}"
    );
}

/// C2 — and a refinement that succeeds says nothing at all (D36).
///
/// The silence means "there is no obligation to handle", not "the engine
/// forgot to mention one". O32 rescanned this on 2026-08-08 and found it
/// empty; this control keeps it that way while the arc makes the *other*
/// branch talk.
#[test]
fn c2_a_successful_evolve_says_nothing() {
    let d = repo_with_deep_value("c2");
    write(
        &d,
        "app: { db: { host: \"h\", port: 5432, opts: { tls: true, retries: 3, extra: 1 } } }\n",
    );
    let (out, ok) = oo_raw(&d, &["evolve", "u.n"]);
    assert!(ok, "a genuine refinement failed: {out}");
    assert!(out.is_empty(), "a successful evolve spoke: {out:?}");
}

// ════════════════════════════════════════════════════════════════════════
//  RED — must fail on `dev f0ecb21`, for the reason each name states
// ════════════════════════════════════════════════════════════════════════

/// R1 — the message must name the leaf, not the field you were typing.
///
/// Measured: the engine has `Some("app.db.opts.retries")` in hand and prints
/// `["app"]`.
#[test]
fn r1_the_message_names_the_leaf() {
    let d = repo_with_deep_value("r1");
    write(&d, DEEP_B);
    let (out, ok) = oo_raw(&d, &["evolve", "u.n"]);
    assert!(!ok, "LIVENESS: this was supposed to conflict: {out}");
    assert!(
        out.contains(DEEP_COORD),
        "the conflict is at `{DEEP_COORD}` and the message does not say so:\n{out}"
    );
}

/// R2 — and it must say it in n/'s spelling, not Rust's.
///
/// `Span { start: 0, end: 3 }` is a byte offset into the source file. It is an
/// implementation detail of the parser and it is currently printed to the
/// operator's face.
#[test]
fn r2_no_rust_internals_reach_the_operator() {
    let d = repo_with_deep_value("r2");
    write(&d, DEEP_B);
    let (out, _) = oo_raw(&d, &["evolve", "u.n"]);
    let found = debris_in(&out);
    assert!(
        found.is_empty(),
        "internal representation leaked into the message: {found:?}\n{out}"
    );
}

/// R3 — the REPL is not a second-class citizen.
///
/// Measured today, verbatim and complete: `Evolution Conflict: Conflict`.
/// No coordinate, not even the field.
#[test]
fn r3_the_repl_names_the_coordinate_too() {
    let d = fresh_dir("r3");
    let out = oo_repl(&d, &format!("{DEEP_A}{DEEP_B}exit\n"));
    assert!(
        out.contains("Evolution Conflict"),
        "LIVENESS: the REPL did not reach a conflict at all:\n{out}"
    );
    assert!(
        out.contains(DEEP_COORD),
        "the REPL reported a conflict without saying where:\n{out}"
    );
}

/// R4 — the shallow case gets a clean coordinate as well.
///
/// Red today for the same debris as R2. Its lasting value is the second half:
/// `path` is `Some("x")` here, but the type admits `None` (a non-Combo
/// incoming on the universe.rs:396 path — reachable in principle, not
/// reproduced). A delivery that concatenates a missing path must not emit
/// `x.` or an empty coordinate.
#[test]
fn r4_a_shallow_coordinate_is_clean_and_well_formed() {
    let d = fresh_dir("r4");
    write(&d, "x: 1\n");
    oo(&d, &["evolve", "u.n"]);
    oo(&d, &["commit", "-m", "base"]);

    write(&d, "x: 2\n");
    let (out, _) = oo_raw(&d, &["evolve", "u.n"]);
    let found = debris_in(&out);
    assert!(
        found.is_empty(),
        "internal representation leaked on the shallow path: {found:?}\n{out}"
    );
    assert!(
        out.contains('x'),
        "the shallow coordinate was not named:\n{out}"
    );
    assert!(
        !out.contains(".."),
        "a malformed coordinate (double dot) was printed:\n{out}"
    );
    for line in out.lines() {
        assert!(
            !line.trim_end().ends_with('.'),
            "a coordinate ends in a dot — a missing path was concatenated:\n{out}"
        );
    }
}

// ════════════════════════════════════════════════════════════════════════
//  PIN — green today; the delivery must not break them
// ════════════════════════════════════════════════════════════════════════

/// P1 — refusal keeps its exit code.
///
/// This arc changes what evolve *says*, never what it *does* (ruling R5).
#[test]
fn p1_a_conflicting_evolve_still_exits_nonzero() {
    let d = repo_with_deep_value("p1");
    write(&d, DEEP_B);
    let (out, ok) = oo_raw(&d, &["evolve", "u.n"]);
    assert!(!ok, "a conflicting evolve exited zero: {out}");
}

/// P2 — and it still writes no `staged`.
///
/// The refusal is what keeps the ⊥ away from the commit boundary in the first
/// place. If a delivery made the message nicer by letting the merge through,
/// R1 would pass and this would fail.
#[test]
fn p2_a_conflicting_evolve_stages_nothing() {
    let d = repo_with_deep_value("p2");
    write(&d, DEEP_B);
    oo_raw(&d, &["evolve", "u.n"]);

    assert!(
        !d.join(".oo").join("staged").exists(),
        "a refused evolve left a staged file behind"
    );
    let (out, ok) = oo_raw(&d, &["commit", "-m", "should not exist"]);
    assert!(
        !ok,
        "there was something to commit after a refused evolve: {out}"
    );
    assert!(
        out.contains("Nothing to commit"),
        "a refused evolve left committable state: {out}"
    );
}

/// P3 — silence on success survives the arc.
///
/// Same assertion as C2, kept as a pin on purpose: the obvious way to make
/// the failure branch consistent is to make *both* branches talk, and that
/// would spend the guarantee (D36) this arc is completing.
#[test]
fn p3_success_stays_silent_after_the_arc() {
    let d = fresh_dir("p3");
    write(&d, "a: 1\n");
    let (out1, ok1) = oo_raw(&d, &["evolve", "u.n"]);
    assert!(ok1 && out1.is_empty(), "first evolve spoke: {out1:?}");

    write(&d, "a: 1\nb: 2\n");
    let (out2, ok2) = oo_raw(&d, &["evolve", "u.n"]);
    assert!(ok2, "second evolve failed: {out2}");
    assert!(out2.is_empty(), "a refinement spoke: {out2:?}");
}
