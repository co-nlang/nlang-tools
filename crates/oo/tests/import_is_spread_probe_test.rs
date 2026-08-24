// Import is spread, and at the top level the spread does not expand.
// Ruling: nlang-spec/meta/oo/STATUS.md O72 (① .. ⑦);
//         material in nlang-spec/meta/oo/d2_what_underscore_means_brief.md.
// Order:  nlang-tools/docs/import_is_spread_handover.md
//
// ── What this arc is ─────────────────────────────────────────────────────
//
// O72 ruled that importing is `...` -- "open the cocoon, then meet" -- and
// that no overlay, mount, or new lookup rule enters the value language.
// The canonical examples in SYNTAX_05 §3 and SPEC_09 §5.2 are TOP-LEVEL
// programs: an import line, then bare `/if` on the next line. Measured
// 2026-08-24 on oo 0.31.0, no spelling makes that run.
//
// The reason is small and it is not a missing lookup rule. Bare names
// already resolve against root coordinates:
//
//     /if: ~%Cond./if
//     result: /if (#true, (@any -> "yes"), (@any -> "no"))   ;; => "yes"
//
// So the only thing missing is the step that puts a module's members INTO
// the root. Two defects stand in the way, both pre-existing:
//
//   (a) `range` has no guard against `...`. `field_start` requires a colon
//       (n.pest: `field_start = _{ field_key ~ ":" }`), a bare spread field
//       has none, so `a: 1` followed by `...~%Cond` parses as `1 .. .~%Cond`.
//   (b) The spread branch lives only in Combo construction (eval.rs:1227).
//       `Universe::evolve` (universe.rs:318) writes the field under the
//       literal key `"..."` instead.
//
// ── Out of scope, do not touch ───────────────────────────────────────────
//
//   * `_:` -- O72 ⑥ left it meaning ONLY the default branch. That is spec
//     work for the acceptor, not an engine change.
//   * mount / overlay / any new name-resolution rule (O72 ④).
//   * standard root contents, ComboVal fields, any address.
//   * nested spread's behaviour. G1..G3 below are here to keep it fixed.
//
// ── Probe integrity ──────────────────────────────────────────────────────
//
// Reds are `#[ignore]`d and MUST be red at the baseline for the reason
// stated in each. The delivery may remove `#[ignore]` and NOTHING else in
// this file. If a pin here is wrong, say so in the report -- do not edit it.
//
// Every red asserts REACH before it asserts a value: an assertion that only
// witnesses the absence of an error witnesses nothing.
//
// Baseline measured 2026-08-24 on dev c38e1c6 / oo 0.31.0: 3 green, 4 red.

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
    nlang_interpreter::ScratchDir::new(&format!("importspread-{tag}"))
}

/// Evolve `src` as a top-level program and return what `oo status` prints.
fn staged_of(tag: &str, src: &str) -> (nlang_interpreter::ScratchDir, String) {
    let d = scratch(tag);
    std::fs::write(d.join("a.n"), src).unwrap();
    oo(&d, &["evolve", "a.n"]);
    let out = oo(&d, &["status"]);
    (d, out)
}

/// The single line `oo status` prints for coordinate `key`, trimmed.
/// `None` when the coordinate is absent.
fn field_line(status: &str, key: &str) -> Option<String> {
    let needle = format!("{key}: ");
    status
        .lines()
        .find(|l| l.trim_start().starts_with(&needle))
        .map(|l| l.trim().to_string())
}

// ─────────────────────────────────────────────────────────────────────────
// RED -- what the arc must make true.
// ─────────────────────────────────────────────────────────────────────────

/// The canonical example. SPEC_09 §5.2 opens with an import line and then
/// uses a bare `/if` -- this is that example, reduced to one branch.
///
/// Baseline: `result` stays the unevaluated expression, because the module
/// landed under a coordinate literally named `"..."` instead of being
/// flattened into the root.
#[test]
#[ignore = "red at baseline: top-level spread does not expand into the root"]
fn r1_a_top_level_import_makes_its_names_resolve() {
    let (_d, status) = staged_of(
        "r1",
        "...~%Cond\nresult: /if (#true, (@any -> \"yes\"), (@any -> \"no\"))\n",
    );

    let got = field_line(&status, "result").unwrap_or_else(|| {
        panic!("REACH: coordinate `result` must exist after evolve.\n{status}")
    });

    assert_eq!(
        got, "result: \"yes\"",
        "a top-level `...~%Cond` must put `/if` in the root, where bare names \
         already resolve (measured: `/if: ~%Cond./if` + `/if (…)` => \"yes\"). \
         got {got:?}\n{status}"
    );
}

/// The silent one. With any ordinary value on the line above, the spread is
/// swallowed as a range continuation and the PREVIOUS coordinate is the one
/// that changes -- no error, no bottom.
///
/// Baseline: `a: 1..#_["~%Cond"]`.
#[test]
#[ignore = "red at baseline: `...` is eaten by `..` when a value precedes it"]
fn r2_a_spread_does_not_corrupt_the_line_above_it() {
    let (_d, status) = staged_of("r2", "a: 1\n...~%Cond\n");

    let got = field_line(&status, "a")
        .unwrap_or_else(|| panic!("REACH: coordinate `a` must exist.\n{status}"));

    assert_eq!(
        got, "a: 1",
        "`a` was assigned 1 and nothing in the program reassigns it; a spread \
         on the next line must not reach back into its value. got {got:?}\n{status}"
    );
}

/// The same defect where the swallow collapses instead of corrupting: a real
/// range above a spread consumes it as the step and goes to bottom.
///
/// Baseline: `a: _|_ (%cause: #conflict)`.
#[test]
#[ignore = "red at baseline: `1..5` + spread collapses to bottom"]
fn r3_a_spread_below_a_range_does_not_collapse_it() {
    let (_d, status) = staged_of("r3", "a: 1..5\n...~%Cond\n");

    let got = field_line(&status, "a")
        .unwrap_or_else(|| panic!("REACH: coordinate `a` must exist.\n{status}"));

    assert!(
        !got.contains("_|_") && !got.contains("#conflict"),
        "a spread on the following line must not turn a well-formed range into \
         bottom. got {got:?}\n{status}"
    );
}

/// `...` is an operator, never a name. If a coordinate spelled `"..."`
/// survives into staged state, the spread was stored rather than performed.
#[test]
#[ignore = "red at baseline: the spread is stored under a coordinate named \"...\""]
fn r4_no_coordinate_is_ever_named_dot_dot_dot() {
    let (_d, status) = staged_of("r4", "...~%Cond\n");

    assert!(
        status.contains("/if"),
        "REACH: the module's members must be somewhere in staged state; if \
         they are absent this test proves nothing.\n{status}"
    );
    assert!(
        !status.contains("\"...\""),
        "`...` is an operator, not a name -- no coordinate may be spelled \
         \"...\".\n{status}"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// GREEN -- what the arc must NOT break. These are the fence.
// ─────────────────────────────────────────────────────────────────────────

/// Nested spread already does the right thing: it opens the cocoon and
/// meets. Whatever makes the top level work must leave this untouched.
#[test]
fn g1_nested_spread_still_opens_and_meets() {
    let (_d, status) = staged_of("g1", "box: { ...~%Cond, v: 1 }\n");

    assert!(
        status.contains("/if") && status.contains("/cond") && status.contains("/match"),
        "nested spread must still flatten ~%Cond's three morphisms into `box`\n{status}"
    );
    let v = field_line(&status, "v")
        .unwrap_or_else(|| panic!("REACH: sibling `v` must survive the spread.\n{status}"));
    assert_eq!(v, "v: 1", "a sibling of a spread keeps its own value; got {v:?}");
}

/// L2-39 in one line: spread is lattice merge, so a colliding key is the
/// meet of both -- bottom when they disagree. Not last-wins.
///
/// Asserted on `q.a` itself rather than on a `%cause` read: the cause prints
/// as a multi-line cocoon, and a single-line match on it would be witnessing
/// the printer, not the merge.
#[test]
fn g2_a_colliding_spread_key_is_still_bottom() {
    let (_d, status) = staged_of("g2", "q: { a: 1, ...{ a: 2 } }\n");

    let got = field_line(&status, "a").unwrap_or_else(|| {
        panic!("REACH: `q.a` must exist -- if the spread vanished this test proves nothing.\n{status}")
    });
    assert!(
        got.contains("_|_") && got.contains("#conflict"),
        "1 and 2 disagree, so the meet is bottom #conflict -- spread must not \
         become last-wins overwrite (this is conformance L2-39). got {got:?}\n{status}"
    );
}

/// The range operator itself. Whatever guard keeps `..` off `...` must not
/// cost us ranges.
#[test]
fn g3_an_ordinary_range_is_unaffected() {
    let (_d, status) = staged_of("g3", "a: 1..5\nb: 2\n");

    let a = field_line(&status, "a")
        .unwrap_or_else(|| panic!("REACH: coordinate `a` must exist.\n{status}"));
    assert!(
        a.contains("..") && !a.contains("_|_"),
        "a range with no spread anywhere near it must still be a range; got {a:?}\n{status}"
    );
    let b = field_line(&status, "b").expect("REACH: `b` must exist");
    assert_eq!(b, "b: 2", "the line after a range is untouched; got {b:?}");
}
