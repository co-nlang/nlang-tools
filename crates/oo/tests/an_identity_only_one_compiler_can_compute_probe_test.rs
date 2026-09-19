// An identity only one compiler can compute.
// Ruling: nlang-spec/meta/oo/STATUS.md D71 (a Code's identity is intensional:
//         a syntax tree taken as a value has syntax for its content).
// Recon:  nlang-tools/docs/an_identity_only_one_compiler_can_compute_recon.md
// Order:  nlang-tools/docs/an_identity_only_one_compiler_can_compute_handover.md
//
// -- READ THIS FIRST: this baseline is ALL GREEN, deliberately --------------
//
// `Value::Code`'s identity is `format!("{:?}", expr.without_spans())`, and
// `Expr` is `#[derive(Debug, ...)]`. Rust does not promise that a derived
// Debug rendering is stable across compiler versions, so the address of any
// stored morphism body is reproducible only by this toolchain -- let alone
// by a second implementation reading the stored bytes, which hold canonical
// n/ source (`~%__nlang_expr: …`) rather than the Debug string that was
// hashed. Same value, two renderings.
//
// That defect CANNOT be exhibited by one binary. Measured: `Expr` has two
// fields, `kind` and `span`, and `without_spans()` strips the span, so the
// Debug form is purely structural -- there is no hidden field that the
// stored source omits, and therefore no store round-trip that moves an
// address. Every check that could go red here is already green.
//
// So the four tests below are RED LINES, not a baseline to flip. They say
// what a repair must not break. A delivery that satisfies all four has NOT
// thereby been shown to fix anything: the fix is a NORMATIVE ENCODING, and
// its probe is written in the repair round, once there is a specified table
// to take a golden digest from. Nothing can serve as a golden value today.
//
// The order says this too, in §4, and says why it is not laziness.

use std::path::Path;
use std::process::Command;

fn cmd(dir: &Path) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_oo"));
    c.current_dir(dir)
        .env("OO_IDENTITY", dir.join("identity-for-tests"))
        .env("OO_NODE_HOME", dir.join("node-home-for-tests"));
    c
}

fn run(dir: &Path, args: &[&str]) -> (String, i32) {
    let o = cmd(dir).args(args).output().expect("oo runs");
    (
        format!(
            "{}{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        ),
        o.status.code().unwrap_or(-1),
    )
}

fn scratch(tag: &str) -> nlang_interpreter::ScratchDir {
    nlang_interpreter::ScratchDir::new(&format!("codeid-{tag}"))
}

/// The CAID `~%Discovery./identify_and_store` mints for `src`'s `f`.
/// Rejects a union: that entry distributes, and reading the first member as
/// if it were the whole answer has already cost this project three void
/// readings (Q-049 §5).
fn stored_f(dir: &Path, src: &str) -> String {
    std::fs::write(
        dir.join("s.n"),
        format!("{src}\nid: ~%Discovery./identify_and_store f\n"),
    )
    .expect("write s.n");
    let (out, _) = run(dir, &["run", "s.n", "--observe", "id"]);
    let t = out.trim();
    assert!(
        !t.contains('|'),
        "REACH: minted a union of CAIDs ({t}); this helper reports one"
    );
    let caid = t
        .trim_start_matches('"')
        .split('"')
        .next()
        .unwrap_or("")
        .split(";;")
        .next()
        .unwrap_or("")
        .trim()
        .to_string();
    assert!(
        caid.starts_with("hash:sha256:"),
        "REACH: stored_f got {caid:?} for {src:?}"
    );
    caid
}

// ---------------------------------------------------------------------
// G1. The control. Two morphisms with different bodies get different
// addresses. If this goes red the fixture is broken.
// ---------------------------------------------------------------------
#[test]
fn g1_two_different_bodies_have_two_addresses() {
    let s = scratch("g1");
    let d = s.path();
    assert_ne!(
        stored_f(d, "f: x -> ~%Math./add (x, x)"),
        stored_f(d, "f: x -> ~%Math./add (x, 1)"),
        "CONTROL: two different bodies must differ"
    );
}

// ---------------------------------------------------------------------
// G2. RED LINE, D71. A Code's content is syntax, so the spelling of a
// string inside a body is part of it. A repair that normalises the tree
// before hashing would make these two equal and would be wrong.
// ---------------------------------------------------------------------
#[test]
fn g2_the_spelling_inside_a_body_is_part_of_the_code() {
    let s = scratch("g2");
    let d = s.path();
    let single = stored_f(d, "f: x -> \"a\nb\"");
    let triple = stored_f(d, "f: x -> \"\"\"a\nb\"\"\"");
    assert_ne!(
        single, triple,
        "D71: a Code is a syntax tree taken as a value, so the two \
         spellings of one text are two Codes"
    );
    let (eq, _) = run(
        d,
        &["eval", "((x -> \"a\nb\") = (x -> \"\"\"a\nb\"\"\"))"],
    );
    assert!(
        eq.contains("#false"),
        "D71: `=` must keep the two spellings apart inside a body: {eq}"
    );
}

// ---------------------------------------------------------------------
// G3. RED LINE, L2-65. The engine comment says `to_nlang` recurses and
// overflows the 64 MiB stack at ~10^3 levels, which is why Debug was
// chosen. Conformance L2-65 is a 3999-term left-associated chain and its
// `%caid` is computed today. A repair that simply switches to `to_nlang`
// dies on a vector already in the corpus.
// ---------------------------------------------------------------------
#[test]
fn g3_a_four_thousand_deep_chain_still_gets_an_address() {
    let s = scratch("g3");
    let d = s.path();
    let mut src = String::from("big: 1");
    for _ in 0..3999 {
        src.push_str(" + 1");
    }
    src.push_str("\nout: (big & { b: 1 }).%caid == big.%caid\n");
    std::fs::write(d.join("deep.n"), &src).expect("write deep.n");
    let (out, rc) = run(d, &["run", "deep.n", "--observe", "out"]);
    assert_eq!(
        rc, 0,
        "a 3999-term chain must still get an address (this is conformance \
         L2-65's shape): {out}"
    );
    assert!(
        out.contains("#true"),
        "the deep chain's %caid must hold verbatim: {out}"
    );
}

// ---------------------------------------------------------------------
// G4. RED LINE. The standard root does not move, and it contains no Code
// at all -- which is the measurement that withdrew the acceptor's claim
// that Code blocked publishing the standard root's CAID.
// ---------------------------------------------------------------------
#[test]
fn g4_the_standard_root_does_not_move_and_holds_no_code() {
    let s = scratch("g4");
    let d = s.path();
    std::fs::write(d.join("a.n"), "x: 0\n").expect("write a.n");
    let (out, rc) = run(d, &["evolve", "a.n"]);
    assert_eq!(rc, 0, "REACH: evolve: {out}");
    let (out, rc) = run(d, &["commit", "-m", "r"]);
    assert_eq!(rc, 0, "REACH: commit: {out}");
    let (status, _) = run(d, &["status"]);
    assert!(
        status.contains("7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911"),
        "the standard root moved: {status}"
    );

    let root = d
        .join(".oo/objects/sha256/70")
        .join("38e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911");
    let bytes = std::fs::read(&root).expect("REACH: the standard root object is on disk");
    let text = String::from_utf8_lossy(&bytes);
    for marker in ["__nlang_code", "__nlang_expr", "ExprKind"] {
        assert!(
            !text.contains(marker),
            "the standard root now holds a Code ({marker}); the epoch scope \
             and the §6.8.2 argument both change if so"
        );
    }
}
