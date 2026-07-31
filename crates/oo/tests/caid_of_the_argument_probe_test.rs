// The CAID of x is the CAID of x (2026-07-29, pre-committed by work order:
// docs/caid_of_the_argument_handover.md).
//
// `~%Discovery./identify x` returns the CAID of the argument pack
// `apply_morphism` builds, not of `x`. SPEC_13 §6.1 says it returns the node's
// intrinsic CAID; REAL_02 §4.2 says an advertisement signature commits to
// `CAID(body)`. Neither holds.
//
// ── Why the marker, and not a smaller fix ────────────────────────────────
//
// Measured: `~%Math./add 1 2` and `~%Math./add (1, 2)` both give 3, and
// `identify (1,2)` equals `identify {{0:1, 1:2}}`. The convention flattens
// "applied to a pair" and "applied to two things" into one shape, so no
// builtin can recover its argument by looking. Unwrapping slot 0
// unconditionally would fix twelve shapes and break the two that are right
// today.
//
// `%arg` already exists — `is_arg_pack` reads it and nothing sets it. Setting
// it in the wrapping branch alone makes the two cases distinguishable with no
// residue, which is why R2 and R3 are here: they are the shapes a careless
// unwrap would break.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

fn fresh_dir(tag: &str) -> PathBuf {
    let mut d = std::env::temp_dir();
    d.push(format!(
        "nlang-caidarg-{}-{}-{}",
        tag,
        std::process::id(),
        DIR_SEQ.fetch_add(1, Ordering::SeqCst)
    ));
    fs::remove_dir_all(&d).ok();
    fs::create_dir_all(&d).unwrap();
    d
}

fn oo_cmd(dir: &Path) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_oo"));
    c.current_dir(dir)
        .env("OO_IDENTITY", dir.join("identity-for-tests"))
        .env("OO_NODE_HOME", dir.join("node-home-for-tests"));
    c
}

fn oo(dir: &Path, args: &[&str]) -> String {
    let out = oo_cmd(dir).args(args).output().unwrap();
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout).trim(),
        String::from_utf8_lossy(&out.stderr).trim()
    )
}

fn write(dir: &Path, name: &str, src: &str) {
    fs::write(dir.join(name), src).unwrap();
}

fn init(dir: &Path) {
    oo(dir, &["run", "--help"]);
    write(dir, "seed.n", "seed: { ok: #true }\n");
    oo(dir, &["run", "seed.n"]);
}

fn first_string(out: &str) -> String {
    out.split('"')
        .nth(1)
        .unwrap_or_else(|| panic!("no quoted string in {out:?}"))
        .to_string()
}

/// What `~%Discovery./identify` answers.
fn identify(dir: &Path, expr: &str) -> String {
    let out = oo(dir, &["eval", &format!("~%Discovery./identify {expr}")]);
    let c = first_string(&out);
    assert!(c.starts_with("hash:sha256:"), "identify({expr}) gave {c:?}");
    c
}

/// The address the content-addressed store actually uses for the same value.
///
/// Deliberately a *different* road to the answer: `identify_and_store` writes
/// the object and returns the address it was written under, so this is the
/// store's own opinion and not another call to the function under test.
fn store_address(dir: &Path, expr: &str) -> String {
    write(
        dir,
        "st.n",
        &format!("v: ~%Discovery./identify_and_store {expr}\n"),
    );
    let out = oo(dir, &["run", "st.n", "--observe", "v"]);
    let c = first_string(&out);
    assert!(
        c.starts_with("hash:sha256:"),
        "store_address({expr}) gave {c:?}"
    );
    c
}

/// Shapes that are **not** argument-pack shaped. These are the ones apply
/// wraps, and the ones `identify` gets wrong today.
const SHAPES: &[(&str, &str)] = &[
    ("int", "42"),
    ("float", "1.5"),
    ("string", "\"hello\""),
    ("bool", "#true"),
    ("tag", "#ok"),
    ("list", "[1, 2, 3]"),
    ("nested list", "[[1], [2, [3]]]"),
    ("combo", "{{ a: 1, b: 2 }}"),
    ("combo nested", "{{ a: {{ b: {{ c: 3 }} }} }}"),
    ("range", "1..10"),
    ("empty combo", "{{}}"),
    ("empty list", "[]"),
    (
        "advert body",
        "{{ node_id: \"n1\", public_key: \"aa\", services: [], listen_port: 8080, capacity: 10, ts: 1, ttl: 15 }}",
    ),
];

// ════════════════════════════════════════════════════════════════════════
//  CONTROL
// ════════════════════════════════════════════════════════════════════════

/// C0 — the two roads to an address give *some* answer, and distinct values
/// get distinct addresses.
///
/// Leads the file. Every red below compares two functions; if either were
/// degenerate — returning a constant, or failing and being papered over — the
/// comparisons would be meaningless.
#[test]
fn c0_both_roads_answer_and_discriminate() {
    let dir = fresh_dir("c0");
    init(&dir);

    let mut ids = Vec::new();
    let mut addrs = Vec::new();
    for (name, src) in SHAPES {
        let i = identify(&dir, src);
        let a = store_address(&dir, src);
        assert!(i.len() > 40, "{name}: identify degenerate");
        assert!(a.len() > 40, "{name}: store degenerate");
        ids.push((name, i));
        addrs.push((name, a));
    }
    for list in [&ids, &addrs] {
        for i in 0..list.len() {
            for j in (i + 1)..list.len() {
                assert_ne!(
                    list[i].1, list[j].1,
                    "{} and {} share an address — the axis is degenerate",
                    list[i].0, list[j].0
                );
            }
        }
    }
}

// ════════════════════════════════════════════════════════════════════════
//  REDS
// ════════════════════════════════════════════════════════════════════════

/// R1 — `./identify v` is `v`'s address. Scanned, untruncated.
#[test]
fn r1_identify_returns_the_caid_of_the_value() {
    let dir = fresh_dir("r1");
    init(&dir);
    let mut wrong = Vec::new();
    for (name, src) in SHAPES {
        let i = identify(&dir, src);
        let a = store_address(&dir, src);
        if i != a {
            wrong.push(format!("{name}\n      identify: {i}\n      store   : {a}"));
        }
    }
    assert!(
        wrong.is_empty(),
        "`~%Discovery./identify` disagreed with the address the store uses, \
         for {} of {} shapes. SPEC_13 §6.1 says it returns the value's \
         intrinsic CAID:\n{}",
        wrong.len(),
        SHAPES.len(),
        wrong.join("\n  ")
    );
}

/// R2 — a tuple is still its own value.
///
/// This is the shape an unconditional unwrap would break: apply does not wrap
/// a tuple, so slot 0 holds the tuple's *first element*, not the tuple.
#[test]
fn r2_a_tuple_is_still_its_own_value() {
    let dir = fresh_dir("r2");
    init(&dir);
    let i = identify(&dir, "(1, 2)");
    let a = store_address(&dir, "(1, 2)");
    assert_eq!(i, a, "the tuple's own CAID");
    let one = store_address(&dir, "1");
    assert_ne!(
        i, one,
        "identify unwrapped a tuple down to its first element"
    );
}

/// R3 — a combo that happens to have slot 0 is still itself.
#[test]
fn r3_a_combo_with_slot_zero_is_still_itself() {
    let dir = fresh_dir("r3");
    init(&dir);
    let i = identify(&dir, "{{ 0: 5 }}");
    let a = store_address(&dir, "{{ 0: 5 }}");
    assert_eq!(
        i, a,
        "a combo shaped like an argument pack is still a value"
    );
    let five = store_address(&dir, "5");
    assert_ne!(i, five, "identify unwrapped a value that was never wrapped");
}

/// R4 — the LADD key and the CAS address are the same address.
#[test]
fn r4_the_ladd_key_is_the_cas_address() {
    let dir = fresh_dir("r4");
    init(&dir);
    let addr = store_address(&dir, "{ treasure: \"r4\" }");
    write(
        &dir,
        "l.n",
        "ad:  ~%Discovery./advertise { treasure: \"r4\" }\nfound: ~%Discovery./find \"REPLACE\"\n"
            .replace("REPLACE", &addr)
            .as_str(),
    );
    let out = oo(&dir, &["run", "l.n", "--observe", "found", "--privileged"]);
    assert!(
        !out.contains("_|_"),
        "advertised a value and then could not find it by the address the \
         store uses for it — the discovery address space and the CAS address \
         space do not meet: {out}"
    );
}

/// R5 — the signature commits to `CAID(body)`, computed without `./identify`.
///
/// The probe derives the payload from the store's address for the body, which
/// is the road REAL_02 §4.2 describes. If the node accepts it, the protocol
/// means what the spec says.
#[test]
fn r5_the_signature_commits_to_the_body_caid() {
    let dir = fresh_dir("r5");
    init(&dir);
    let body = "{{ node_id: \"n1\", public_key: \"aa\", services: [], \
                 listen_port: 8080, capacity: 10, ts: 1, ttl: 15 }}";
    let via_identify = identify(&dir, body);
    let via_store = store_address(&dir, body);
    assert_eq!(
        via_identify, via_store,
        "the CAID an advertisement signature commits to is not the body's own \
         address, so a second implementation reading REAL_02 §4.2 literally \
         would compute a different payload and its signatures would not verify"
    );
}

/// R6 — storing a tuple stores the tuple.
///
/// `engine.save` already unwraps slot 0 unconditionally, which is right for a
/// wrapped argument and takes the first element of one that was never wrapped.
/// Measured on v0.2.55: `identify_and_store (1, 2)` stores `1` and returns
/// `1`'s address, and `oo inspect` on it prints `1`. **The store confirms it
/// saved something it did not save.**
#[test]
fn r6_storing_a_tuple_stores_the_tuple() {
    let dir = fresh_dir("r6");
    init(&dir);
    let tuple = store_address(&dir, "(1, 2)");
    let one = store_address(&dir, "1");
    assert_ne!(
        tuple, one,
        "storing the tuple (1, 2) stored the value 1 — silent data loss on \
         the write path, and the store returned an address confirming it"
    );
    let back = oo(&dir, &["inspect", &tuple]);
    assert!(
        back.contains('2'),
        "the object at the tuple's address does not contain its second \
         element: {back}"
    );
}

/// R7 — and the same for a combo that happens to have slot 0.
#[test]
fn r7_storing_a_slot_zero_combo_stores_the_combo() {
    let dir = fresh_dir("r7");
    init(&dir);
    let combo = store_address(&dir, "{{ 0: 9 }}");
    let nine = store_address(&dir, "9");
    assert_ne!(combo, nine, "storing {{{{ 0: 9 }}}} stored the value 9");
}

// ════════════════════════════════════════════════════════════════════════
//  PINS
// ════════════════════════════════════════════════════════════════════════

/// P1 — multi-argument application is untouched, including currying.
#[test]
fn p1_multi_argument_builtins_are_unchanged() {
    let dir = fresh_dir("p1");
    init(&dir);
    for (expr, want) in [("~%Math./add 1 2", "3"), ("~%Math./add (1, 2)", "3")] {
        let got = oo(&dir, &["eval", expr]);
        assert!(got.contains(want), "{expr} gave {got:?}, wanted {want}");
    }
}

/// P2 — the marker never reaches anything a program can observe.
#[test]
fn p2_the_marker_never_reaches_a_user_visible_value() {
    let dir = fresh_dir("p2");
    init(&dir);
    write(&dir, "m.n", "f: /id { 1: $ }\nout: f { a: 1, b: [2, 3] }\n");
    let got = oo(&dir, &["run", "m.n", "--observe", "out"]);
    assert!(
        !got.contains("%arg") && !got.contains("arg:"),
        "an internal calling-convention marker surfaced in an observation: {got}"
    );
}

/// P3 — a value's address in the store does not move.
#[test]
fn p3_store_round_trip_is_unchanged() {
    let dir = fresh_dir("p3");
    init(&dir);
    let a = store_address(&dir, "{ stable: \"p3\", n: [1, 2, 3] }");
    let b = store_address(&dir, "{ stable: \"p3\", n: [1, 2, 3] }");
    assert_eq!(a, b, "the same value got two addresses");
    let c = oo(&dir, &["inspect", &a]);
    assert!(
        c.contains("stable"),
        "the stored value did not read back: {c}"
    );
}

/// P4 — the whole-argument iterators over *operands* are unaffected.
#[test]
fn p4_diff_and_toml_do_not_see_the_marker() {
    let dir = fresh_dir("p4");
    init(&dir);
    let got = oo(&dir, &["eval", "~%Diff./compare ({ a: 1 }, { a: 2 })"]);
    assert!(
        !got.contains("%arg") && !got.contains("\"arg\""),
        "the marker leaked into a diff: {got}"
    );
}
