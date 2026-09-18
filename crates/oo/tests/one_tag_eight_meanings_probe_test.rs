// One tag, eight meanings.
// Ruling: nlang-spec/spec/zh_TW/REAL_03_CAID_Protocol.md §6.7, clause 2
//         (values must not share an address), landed in v0.52.0 by D69.
// Recon:  nlang-tools/docs/one_tag_eight_meanings_recon.md
// Order:  nlang-tools/docs/one_tag_eight_meanings_handover.md
//
// -- What this arc is ----------------------------------------------------
//
// `serialize_atom` has eight arms that push TAG_ATOM (0x10) and then write
// a bare string: Str/MultilineStr (the content), Code (a Rust `{:?}`),
// PathLit, Unit (the literal text "()"), Regex, Uri, Time, and Bytes (hex).
// Nothing distinguishes them afterwards, so values whose text coincides
// land on one address while the language calls them different.
//
//     "ab"  r"ab"  p"ab"  u"ab"  t"ab"     all 2f5992b8...
//     "()"  ()                             both 5f6b36a7...
//     "6162"  b"ab"                        both 4414b39b...
//
// This is the third family under D69, after the hash-time trim and the
// integer overflow fallback that v0.52.0 repaired. It violates the clause
// that same version added.
//
// -- Why the fix and the table are one job -------------------------------
//
// REAL_03 §6.2's value-tag table cannot honestly say "0x10 = Atom" while
// 0x10 means eight things. Giving each kind its own tag byte is both the
// repair and the rows the table was missing. That is why this is not a
// documentation arc.
//
// NOT PROBED, stated so no one mistakes silence for coverage:
//   * `serialize_union` pushes no tag and no length. The acceptor has read
//     that in the source and has NOT measured a collision from it. An
//     earlier reading that claimed one was withdrawn: the entry used to
//     measure it (`~%Discovery./identify_and_store`) DISTRIBUTES over a
//     union and returns a union of CAIDs, and the acceptor's helper took
//     only the first. It is a must-answer on the order, not an assertion
//     here, and it must not be measured through a distributing entry.
//   * `Code`'s identity is `format!("{:?}", expr)`, a Rust-specific
//     format a second implementation cannot reproduce. Same family, its
//     own Inbox row, and reachability from source was not measured.
//   * `Thunk` (0x17) and `Blur` (0xFD) reachability from surface syntax.
//     `Range` (0x18) is NOT reachable: `1...3` is a parse error.
//   * Floats. `0x13` is lossy by specification and that is O88, not this.

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
    nlang_interpreter::ScratchDir::new(&format!("onetag-{tag}"))
}

/// The CAID `~%Discovery./identify_and_store` mints for `expr`.
///
/// Rejects a union outright. The acceptor lost three readings to this:
/// the entry distributes, so a union answers with a union of CAIDs and a
/// naive parse silently reports the first member's address as the whole
/// value's. Nothing in this file may pass a union through here.
fn stored(dir: &Path, expr: &str) -> String {
    std::fs::write(
        dir.join("s.n"),
        format!("id: ~%Discovery./identify_and_store {expr}\n"),
    )
    .expect("write s.n");
    let (out, _) = run(dir, &["run", "s.n", "--observe", "id"]);
    let trimmed = out.trim();
    assert!(
        !trimmed.contains('|'),
        "REACH: {expr} minted a union of CAIDs ({trimmed}); this helper \
         reports one address and would report the first member's"
    );
    let caid = trimmed
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
        "REACH: stored() got {caid:?} for {expr}"
    );
    caid
}

// ---------------------------------------------------------------------
// G1. The control. Two plain strings already get two addresses. If this
// goes red the fixture is broken and nothing else here means anything.
// ---------------------------------------------------------------------
#[test]
fn g1_two_strings_already_have_two_addresses() {
    let s = scratch("g1");
    let d = s.path();
    assert_ne!(
        stored(d, "\"x\""),
        stored(d, "\"y\""),
        "CONTROL: \"x\" and \"y\" must differ"
    );
}

// ---------------------------------------------------------------------
// G2. RED LINE. `=` is not what this arc touches. The language already
// tells these apart; only the addresses are wrong.
// ---------------------------------------------------------------------
#[test]
fn g2_equality_is_untouched() {
    let s = scratch("g2");
    let d = s.path();
    for (expr, want) in [
        ("(\"()\" = ())", "#false"),
        ("(r\"ab\" = \"ab\")", "#false"),
        ("(p\"ab\" = \"ab\")", "#false"),
        ("(u\"ab\" = \"ab\")", "#false"),
        ("(t\"ab\" = \"ab\")", "#false"),
        ("(b\"ab\" = \"6162\")", "#false"),
        ("(\"ab\" = \"ab\")", "#true"),
        ("(() = ())", "#true"),
    ] {
        let (out, _) = run(d, &["eval", expr]);
        assert!(out.contains(want), "{expr} must stay {want}: {out}");
    }
}

// ---------------------------------------------------------------------
// G3. RED LINE. The standard root does not move.
// ---------------------------------------------------------------------
#[test]
fn g3_the_standard_root_does_not_move() {
    let s = scratch("g3");
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
}

// ---------------------------------------------------------------------
// G4. RED LINE, and the one that bounds the epoch. Giving the overloaded
// kinds their own tags must move THEM and nothing else. These four are
// the neighbours most likely to be caught by a careless edit to
// `serialize_atom`, and their addresses are the v0.52.0 values.
// ---------------------------------------------------------------------
#[test]
fn g4_the_values_this_arc_does_not_touch_keep_their_addresses() {
    let s = scratch("g4");
    let d = s.path();
    for (expr, want) in [
        ("\"a\"", "cdf2d551ff168267"),
        ("0", "08da7c45cb204377"),
        ("1.5", "cb3bcb5020bd1356"),
        ("#true", "1bc573f2ac349356"),
    ] {
        let got = stored(d, expr);
        assert!(
            got.contains(want),
            "{expr} must keep its v0.52.0 address {want}, got {got}"
        );
    }
}

// ---------------------------------------------------------------------
// R1. A unit is not the two-character string that spells it.
// ---------------------------------------------------------------------
#[test]
fn r1_a_unit_is_not_the_string_that_spells_it() {
    let s = scratch("r1");
    let d = s.path();
    // Control: the string does differ from a different string, so a red
    // below is about the unit and not about strings being broken.
    assert_ne!(
        stored(d, "\"()\""),
        stored(d, "\"(){}\""),
        "CONTROL: two different strings must differ"
    );
    assert_ne!(
        stored(d, "\"()\""),
        stored(d, "()"),
        "the unit value and the string \"()\" share one address, because \
         `serialize_atom` writes the unit as the text `()` under the same \
         tag a string uses"
    );
}

// ---------------------------------------------------------------------
// R2. Five spellings, five values, five addresses. Today: one address.
// ---------------------------------------------------------------------
#[test]
fn r2_five_kinds_of_value_are_not_one_string() {
    let s = scratch("r2");
    let d = s.path();
    let plain = stored(d, "\"ab\"");
    for lit in ["r\"ab\"", "p\"ab\"", "u\"ab\"", "t\"ab\""] {
        assert_ne!(
            stored(d, lit),
            plain,
            "{lit} and the string \"ab\" share one address while `=` calls \
             them different"
        );
    }
    // And they are not each other either -- a repair that gave all four
    // one new tag would satisfy the loop above and still be wrong.
    let all = [
        stored(d, "r\"ab\""),
        stored(d, "p\"ab\""),
        stored(d, "u\"ab\""),
        stored(d, "t\"ab\""),
    ];
    for i in 0..all.len() {
        for j in (i + 1)..all.len() {
            assert_ne!(
                all[i], all[j],
                "two of the four prefixed literals share one address"
            );
        }
    }
}

// ---------------------------------------------------------------------
// R3. Bytes are not the text of their own hex.
// ---------------------------------------------------------------------
#[test]
fn r3_bytes_are_not_their_hex_text() {
    let s = scratch("r3");
    let d = s.path();
    assert_ne!(
        stored(d, "b\"ab\""),
        stored(d, "\"6162\""),
        "a byte string and the string spelling its hex share one address"
    );
}

// ---------------------------------------------------------------------
// R4. The harm, written as the invariant rather than as the mechanism.
//
// The address collision is not what an operator sees. What they see is a
// meet that takes two operands the language calls different and returns
// one of them, silently, because equal digests let unify exit early.
// ---------------------------------------------------------------------
#[test]
fn r4_a_meet_does_not_absorb_a_value_it_can_tell_apart() {
    let s = scratch("r4");
    let d = s.path();

    // Control: a meet of one value with itself survives, so a red below is
    // about distinguishability and not about meet being broken.
    let (same, _) = run(d, &["eval", "(\"ab\" & \"ab\")"]);
    assert!(
        same.contains("\"ab\""),
        "CONTROL: a value met with itself must survive: {same}"
    );

    for expr in [
        "(\"()\" & ())",
        "(r\"ab\" & \"ab\")",
        "(p\"ab\" & \"ab\")",
        "(u\"ab\" & \"ab\")",
        "(t\"ab\" & \"ab\")",
        "(b\"ab\" & \"6162\")",
    ] {
        let (out, _) = run(d, &["eval", expr]);
        assert!(
            out.contains("_|_") && out.contains("#conflict"),
            "`=` calls these two different and the meet swallowed one of \
             them without saying so: {expr} -> {out}"
        );
    }
}

const QUOTE3: &str = "\"\"\"";


// ---------------------------------------------------------------------
// R5. REPAIR ROUND 1 (D70, ruled 2026-09-18).
//
// A triple-quoted block is a way of WRITING a string, not a kind of
// value. The same text written both ways is one value.
//
// This slot exists because R1-R4 above, by making the address agree with
// `=`, cast an answer nobody had ruled. The arc did not create the
// question -- the collision had been hiding it -- but it was about to be
// cut into an address, so it was ruled first.
// ---------------------------------------------------------------------
#[test]
fn r5_one_text_written_two_ways_is_one_value() {
    let s = scratch("r5");
    let d = s.path();

    // Control: two different texts are still two values, so a red below is
    // about spelling and not about strings collapsing wholesale.
    assert_ne!(
        stored(d, "\"hello\""),
        stored(d, "\"world\""),
        "CONTROL: two different texts must stay two values"
    );
    let (ctl, _) = run(d, &["eval", "(\"hello\" = \"world\")"]);
    assert!(ctl.contains("#false"), "CONTROL: {ctl}");

    let single = stored(d, "\"hello\"");
    let triple = stored(d, &format!("{q}hello{q}", q = QUOTE3));
    assert_eq!(
        single, triple,
        "the same text written single-line and triple-quoted got two \
         addresses; D70 says a triple-quoted block is a way of writing a \
         string, not a kind of value"
    );

    let expr = format!("(\"hello\" = {q}hello{q})", q = QUOTE3);
    let (out, _) = run(d, &["eval", &expr]);
    assert!(
        out.contains("#true"),
        "`=` must call the two spellings one value: {out}"
    );
}

// ---------------------------------------------------------------------
// R6. REPAIR ROUND 1. The harm, as the invariant rather than the
// mechanism.
//
// This is what made D70 forced rather than preferred. A value printed in
// its own canonical form and read back was a DIFFERENT value:
//
//     v: "a<newline>b"        -> 2024d616...   prints as a triple-quoted block
//     that text, re-read      -> ecb0980d...
//
// SYNTAX_02 116 says a single-line string cannot hold a quote character,
// and 129 says a multiline must print triple-quoted. So the printer is
// forbidden from telling the two apart -- which means identity must not
// either, or writing a value down stops being lossless.
//
// Note this probe does not hard-code either address. It asserts the
// round-trip, which is the property; the addresses are free to be
// whatever the repair makes them.
// ---------------------------------------------------------------------
#[test]
fn r6_a_value_survives_being_written_down_and_read_back() {
    let s = scratch("r6");
    let d = s.path();

    for text in ["a\nb", "hello"] {
        // Mint the value, then mint whatever its own printed form denotes.
        let src = format!("v: \"{text}\"\n");
        std::fs::write(d.join("v.n"), &src).expect("write v.n");
        let (printed, rc) = run(d, &["run", "v.n", "--observe", "v"]);
        assert_eq!(rc, 0, "REACH: {src:?} did not evaluate: {printed}");
        let printed = printed.trim();
        assert!(!printed.is_empty(), "REACH: nothing printed for {src:?}");

        let before = stored(d, &format!("\"{text}\""));
        let after = stored(d, printed);
        assert_eq!(
            before, after,
            "a value printed in its own canonical form and read back \
             became a different value: {src:?} printed as {printed:?}"
        );
    }
}

