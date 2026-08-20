// A name the printer could not write.
// Recon: docs/a_name_the_printer_could_not_write_recon.md
// Rulings: nlang-spec/meta/oo/quoted_names.md §4 (Q1, Q3; Q2 separate)
//          + recon §4 = B, §6 canonical form, §6.1 escapes, §6.2 in scope.
//
// ── The one property ─────────────────────────────────────────────────────
//
// Printing a value must yield source that reads back as THE SAME VALUE.
// Today it can yield source that reads back as a DIFFERENT value, without
// an error: { "a.b": 1 } is one coordinate (SYNTAX_03 §106 #8) and prints
// as `a.b: 1`, which reads back as two.
//
// Every red below is that one property at a different input. They are not
// four bugs; they are one rule with four symptoms -- keys the printer
// leaves bare, and values it prints as a Rust debug form.
//
// ── What these assertions pin ────────────────────────────────────────────
//
// Identity, via %id -- never the wording of the printed form. A delivery
// is free to choose any output spelling that reads back to the same
// address. The canonical form ruled in the recon (quote anything that is
// not a plain identifier) is a spec clause, not a probe assertion.
//
// ── Probe integrity ──────────────────────────────────────────────────────
//
// Reds are `#[ignore]`d and MUST be red at the baseline for the reason
// stated in each. The delivery may remove `#[ignore]` and NOTHING else in
// this file.

use std::path::Path;
use std::process::Command;

fn oo(dir: &Path, args: &[&str]) -> String {
    let mut c = Command::new(env!("CARGO_BIN_EXE_oo"));
    c.current_dir(dir)
        .env("OO_IDENTITY", dir.join("identity-for-tests"))
        .env("OO_NODE_HOME", dir.join("node-home-for-tests"));
    let o = c.args(args).output().expect("oo runs");
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

fn scratch(tag: &str) -> nlang_interpreter::ScratchDir {
    nlang_interpreter::ScratchDir::new(&format!("printerquote-{tag}"))
}

/// Observe `app` for one source and return what was printed.
fn printed(tag: &str, src: &str) -> String {
    let d = scratch(tag);
    std::fs::write(d.join("a.n"), src).unwrap();
    oo(&d, &["run", "a.n", "--observe", "app"]).trim().to_string()
}

/// The address of `app` for one source, or None if it did not observe.
fn id_of(tag: &str, src: &str) -> Option<String> {
    let body = format!("{src}\nprobe_out: {{ i: app.%id }}\n");
    let d = scratch(tag);
    std::fs::write(d.join("a.n"), body).unwrap();
    let out = oo(&d, &["run", "a.n", "--observe", "probe_out"]);
    out.split('"')
        .find(|t| t.starts_with("hash:sha256:"))
        .map(|s| s.to_string())
}

/// Print `app`, feed the printed form back as `app`, and return the two
/// addresses. This is the whole property, in one helper.
fn round_trip(tag: &str, src: &str) -> (Option<String>, Option<String>) {
    let before = id_of(tag, src);
    let back = printed(tag, src);
    let after = id_of(&format!("{tag}-back"), &format!("app: {back}\n"));
    (before, after)
}

fn assert_round_trips(tag: &str, src: &str, why: &str) {
    let (before, after) = round_trip(tag, src);
    let b = before.expect("harness: the source must observe at the baseline");
    assert_eq!(
        Some(&b),
        after.as_ref(),
        "{why}\n  source:  {src}\n  printed: {}\n  the printed form must read back as the same value",
        printed(tag, src)
    );
}

// ── C1..C4 ── controls: green at baseline, MUST stay green ───────────────

#[test]
fn c1_control_a_plain_key_round_trips() {
    assert_round_trips("c1", "app: { a: 1 }", "a plain identifier key");
}

#[test]
fn c2_control_a_single_line_string_round_trips() {
    assert_round_trips("c2", "app: { s: \"hello\" }", "a single-line string");
}

/// Nesting must SURVIVE as nesting. Guards the opposite error: a delivery
/// that quotes too eagerly could turn `a: { b: 1 }` into a key `"a.b"`.
#[test]
fn c3_control_nesting_stays_nested() {
    assert_round_trips("c3", "app: { a: { b: 1 } }", "a nested pair");
    let flat = id_of("c3-flat", "app: { \"a.b\": 1 }");
    let nested = id_of("c3-nest", "app: { a: { b: 1 } }");
    assert!(
        flat.is_some() && nested.is_some() && flat != nested,
        "one coordinate named a.b and a nested pair must stay different values"
    );
}

/// An UNQUOTED prefix still selects an axis. The arc changes what quotes
/// mean, never what a bare prefix means.
#[test]
fn c4_control_a_bare_prefix_still_selects_its_axis() {
    let d = scratch("c4");
    std::fs::write(
        d.join("a.n"),
        "x: { @t: 1, d: 2 }\napp: { k: ~%Reflection./keys(x) }\n",
    )
    .unwrap();
    let out = oo(&d, &["run", "a.n", "--observe", "app"]);
    assert!(
        out.contains("\"d\"") && !out.contains("\"t\""),
        "bare `@t` must land on the type axis, not the data axis, got: {out}"
    );
}

// ── R1..R6 ── reds ───────────────────────────────────────────────────────

/// RED, and the severe one: today this prints bare and reads back as TWO
/// coordinates with no error.
#[test]
#[ignore = "RED: a quoted dotted key prints bare and reads back as nesting"]
fn r1_a_dotted_key_round_trips() {
    assert_round_trips("r1", "app: { \"a.b\": 1 }", "one coordinate named a.b");
}

/// RED: prints bare, reads back as a parse error. Loud, but the same rule.
#[test]
#[ignore = "RED: a quoted key with a space prints bare and will not reparse"]
fn r2_a_spaced_key_round_trips() {
    assert_round_trips("r2", "app: { \"a b\": 1 }", "a key containing a space");
}

/// RED: quotes hold the whole name, prefixes included (Q1). Today the
/// quotes are a complete no-op and these two share an address.
#[test]
#[ignore = "RED: quotes are a no-op for prefixes -- both are the type axis"]
fn r3_a_quoted_prefix_is_part_of_the_name() {
    let quoted = id_of("r3-q", "app: { \"@t\": 1 }");
    let bare = id_of("r3-b", "app: { @t: 1 }");
    assert!(quoted.is_some() && bare.is_some(), "harness: both must observe");
    assert_ne!(
        quoted, bare,
        "a data key literally named @t must not be the same value as a type-axis key t"
    );
    assert_round_trips("r3-rt", "app: { \"@t\": 1 }", "a key literally named @t");
}

/// RED: the same rule on the value side -- a multiline string prints as a
/// Rust debug form carrying escapes this language does not have.
#[test]
#[ignore = "RED: multiline strings print as MultilineStr(..) and cannot reparse"]
fn r4_a_multiline_string_value_round_trips() {
    assert_round_trips(
        "r4",
        "app: { s: \"\"\"a\"b\"\"\" }",
        "a multiline string holding a quote character",
    );
}

/// RED: SYNTAX_02 §106 #11 names `quoted_key` and sends it to the
/// multiline form for content needing a quote -- but `field_key` has no
/// multiline alternative, so the route the spec prescribes does not exist.
#[test]
#[ignore = "RED: field_key has no multiline form, so this key cannot be written"]
fn r5_a_name_holding_a_quote_can_be_written() {
    let id = id_of("r5", "app: { \"\"\"a\"b\"\"\": 1 }");
    assert!(
        id.is_some(),
        "a key whose name contains a quote must be writable via the multiline form"
    );
}

/// RED: recon §4 ruling B -- rename the three overload keys so that no key
/// in the standard root needs quoting at all. Pins the PROPERTY, not the
/// new names: a delivery may choose any spelling that is a plain name.
#[test]
#[ignore = "RED: /%differential.1/.2/.3 are not plain names"]
fn r6_no_standard_root_key_needs_quoting() {
    let d = scratch("r6");
    let engine = nlang_interpreter::Ouroboros::init(&d).unwrap();
    let root = engine.root_with_system();

    fn plain(name: &str) -> bool {
        let bare = name
            .strip_prefix("~%")
            .or_else(|| name.strip_prefix('~'))
            .or_else(|| name.strip_prefix('@'))
            .or_else(|| name.strip_prefix('/'))
            .or_else(|| name.strip_prefix('%'))
            .unwrap_or(name);
        let mut cs = bare.chars();
        matches!(cs.next(), Some(c) if c.is_alphabetic() || c == '_')
            && cs.all(|c| c.is_alphanumeric() || c == '_' || c == '-')
    }

    fn walk(v: &nlang_interpreter::Value, path: &str, bad: &mut Vec<String>) {
        if let nlang_interpreter::Value::Combo(c) = v {
            for (k, child) in c.fields() {
                if !plain(&k) {
                    bad.push(format!("{path}/{k}"));
                }
                walk(&child, &format!("{path}/{k}"), bad);
            }
        }
    }

    let mut bad = Vec::new();
    walk(&nlang_interpreter::Value::Combo(root), "", &mut bad);
    assert!(
        bad.is_empty(),
        "every standard-root coordinate must be writable without quotes; these are not: {bad:#?}"
    );
}
