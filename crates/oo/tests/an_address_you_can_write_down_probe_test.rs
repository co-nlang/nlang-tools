// An address you can write down.
// Ruling: nlang-spec/meta/oo/STATUS.md O67 / O70; material in
//         nlang-spec/meta/oo/address_literal.md §3 §4.1-4.4 §7.
//
// ── What this arc is ─────────────────────────────────────────────────────
//
// Until now the only handle on a value was a name, and every name needs a
// binding site, and the one site that is always there is the root. This
// arc gives the language a second handle: `_{sha256:<64 hex>}.` is an
// ANCHOR in the same family as `_.` and `^.` -- `_.` is this universe's
// root, `_{addr}.` is that one's.
//
// Ruled and NOT open here:
//   O67   the delimiter is `_{…}`; `_"…"` lost because quotes name a pure
//         name and an address is not a name.
//   §4.1  digest only, but the algorithm stays: `sha256:<hex>`, no `hash:`
//         prefix, no version, no sketch, no masa. blake3 is an existing
//         peer in REAL_03 §30 and shares the 64-hex shape, so a bare
//         digest is ambiguous the day blake3 lands.
//   §4.2  a missing ADDRESS is a named refusal. Never a silent `_`.
//   §4.3  resolving is pure. Availability is not an effect.
//   §4.4  RHS only. A definition's left-hand side is this universe's
//         coordinate, not somebody else's.
//   O70   the effect you observe belongs to the value that came back, not
//         to the act of reaching it.
//
// ── Out of scope, do not touch ───────────────────────────────────────────
//
//   * `_: X` as a fallback slot (O69) -- separate arc, moves addresses.
//   * `~%Discovery./fetch`'s effect tag (O70 ⑤) -- separate arc, moves the
//     standard root digest and therefore every universe.
//   * remote resolution. This arc reads the local store only.
//   * prefetch / dependency scanning tools.
//
// ── Probe integrity ──────────────────────────────────────────────────────
//
// Reds are `#[ignore]`d and MUST be red at the baseline for the reason
// stated in each. The delivery may remove `#[ignore]` and NOTHING else in
// this file. Changing an input to dodge a retired property leaves a test
// whose name lies; if a pin here is wrong, say so in the report.
//
// Baseline measured 2026-08-22 on dev ea05acb / oo 0.28.0: 4 green, 6 red.
// R2/R4/R5 were re-calibrated the same day: the first drafts passed at the
// baseline -- R2 by sniffing a substring out of a parse-error message, R4 and
// R5 because nothing was reached, so nothing could raise an effect. Each now
// asserts the reach first. An assertion that only witnesses the absence of an
// error witnesses nothing.

use std::path::Path;
use std::process::Command;

const HEX: &str = "426d518693561d9b17814ea8e2818ea7a8a12b4c9e87befdf7f54be1cef5f92b";

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
    nlang_interpreter::ScratchDir::new(&format!("addrlit-{tag}"))
}

/// Build a universe holding `src`, commit it, and return (dir, address of
/// the committed root object as `sha256:<hex>`).
fn universe_with(tag: &str, src: &str) -> (nlang_interpreter::ScratchDir, String) {
    let d = scratch(tag);
    std::fs::write(d.join("a.n"), src).unwrap();
    oo(&d, &["evolve", "a.n"]);
    oo(&d, &["commit", "-m", "x"]);
    let mut found = None;
    for e in walk(&d.join(".oo").join("objects")) {
        let body = std::fs::read_to_string(&e).unwrap_or_default();
        if body.starts_with('"') && body.contains("standard-root:") {
            continue;
        }
        if body.contains("\"parent\"") {
            continue;
        }
        let file = e.file_name().unwrap().to_string_lossy().to_string();
        let dir = e.parent().unwrap().file_name().unwrap().to_string_lossy().to_string();
        found = Some(format!("sha256:{dir}{file}"));
    }
    let addr = found.expect("a user root object exists");
    (d, addr)
}

fn walk(p: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(p) {
        for e in rd.flatten() {
            let path = e.path();
            if path.is_dir() {
                out.extend(walk(&path));
            } else {
                out.push(path);
            }
        }
    }
    out
}

// ─────────────────────────────────────────────────────────────────────────
// GREEN -- controls. These must stay green through the whole arc.
// ─────────────────────────────────────────────────────────────────────────

/// C1  The existing anchor family is untouched. `_.` is this universe's
/// root and `^.` is one parent up; the new branch must be an addition to
/// `anchored_path`, not a rewrite of it.
#[test]
fn c1_the_existing_anchors_still_answer() {
    let d = scratch("c1");
    let out = oo(&d, &["eval", "_."]);
    assert!(
        out.contains('{'),
        "`_.` must still be this universe's root, got: {out}"
    );
}

/// C2  A digest with no algorithm is not an address. §4.1 ruled the
/// algorithm must stay because blake3 is already a peer in REAL_03 §30 and
/// its default output is the same 64 hex characters.
///
/// NOTE FOR THE DELIVERY: this is green at the baseline for an unrelated
/// reason -- `{426d…}` is not a legal combo today either. It is pinned
/// because it must be green for the RIGHT reason afterwards: rejected as
/// an address missing its algorithm, not as a malformed combo.
#[test]
fn c2_a_bare_digest_is_not_an_address() {
    let d = scratch("c2");
    let out = oo(&d, &["eval", &format!("_{{{HEX}}}.")]);
    assert!(
        out.contains("Error") || out.contains("rror"),
        "a digest without its algorithm must not parse as an address, got: {out}"
    );
}

/// C3  `_{…}` is compound-atomic: with a space it is not an anchor. It
/// falls back to the pre-existing reading -- an unknown operator applied
/// to a combo, which is `_` by the three-way rule. Silent, and that is
/// accepted (O67): `_"…"` was equally silent, so this never separated the
/// candidates, and the answer `_` is honest -- the language cannot tell
/// that a legitimate combo was meant as an address.
#[test]
fn c3_the_spaced_form_is_not_an_anchor() {
    let d = scratch("c3");
    let out = oo(&d, &["eval", &format!("_ {{sha256:{HEX}}}.x")]);
    assert!(
        out.trim_end().ends_with('_') || out.trim() == "_",
        "`_ {{…}}` must stay the old juxtaposition and answer `_`, got: {out}"
    );
}

/// C4  RHS only (§4.4). A definition's left-hand side is this universe's
/// coordinate; you may not define into somebody else's root.
#[test]
fn c4_an_address_is_not_a_definition_site() {
    let d = scratch("c4");
    std::fs::write(
        d.join("a.n"),
        format!("_{{sha256:{HEX}}}.k: 1\nout: 1\n"),
    )
    .unwrap();
    let out = oo(&d, &["run", "a.n", "--observe", "out"]);
    assert!(
        out.contains("Error") || out.contains("rror"),
        "an address literal must be rejected on the left of `:`, got: {out}"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// RED -- the arc. Each must be red at the baseline for the reason stated.
// ─────────────────────────────────────────────────────────────────────────

/// R1  The literal parses as an anchor and reaches the value at that
/// address in the local store.
///
/// Baseline: `_{sha256:…}.` is a parse error at the trailing `.`, and
/// `_{sha256:…}.x` parses as `_` applied to a combo and answers `_`.
#[test]
#[ignore]
fn r1_an_address_reaches_the_value_it_names() {
    let (d, addr) = universe_with("r1", "k: 7\n");
    let out = oo(&d, &["eval", &format!("_{{{addr}}}.k")]);
    assert!(
        out.contains('7'),
        "`_{{{addr}}}.k` must answer 7 from the local store, got: {out}"
    );
}

/// R2  The whole value, with no path segments. `_{addr}.` is an anchor,
/// so the trailing `.` is part of it and must parse.
#[test]
#[ignore]
fn r2_an_address_with_no_segments_is_the_whole_value() {
    let (d, addr) = universe_with("r2", "k: 7\n");
    let out = oo(&d, &["eval", &format!("_{{{addr}}}.")]);
    assert!(
        !out.contains("Error") && !out.contains("rror"),
        "`_{{{addr}}}.` must parse -- the trailing `.` belongs to the anchor, got: {out}"
    );
    assert!(
        out.contains("k: 7"),
        "`_{{{addr}}}.` must be the whole root, got: {out}"
    );
}

/// R3  A missing address is a NAMED refusal, never a silent `_` (§4.2).
/// The engine must be able to say which address it could not resolve.
#[test]
#[ignore]
fn r3_a_missing_address_is_named_not_silent() {
    let d = scratch("r3");
    let out = oo(&d, &["eval", &format!("_{{sha256:{HEX}}}.k")]);
    let quiet = out.trim() == "_" || out.trim().is_empty();
    assert!(!quiet, "an unresolvable address must not answer silently, got: {out}");
    assert!(
        out.contains(&HEX[..16]),
        "the refusal must name the address it could not resolve, got: {out}"
    );
}

/// R4  Resolving is pure (§4.3): reaching a pure value through an address
/// must not change the identity of the value that reached it. This is the
/// property `~%Discovery./fetch` fails today -- a purely local read taints
/// the enclosing combo with `#io`, and effects have entered the address
/// since v0.26.0, so importing changes the importer.
#[test]
#[ignore]
fn r4_reaching_a_pure_value_does_not_move_the_reacher() {
    let (d, addr) = universe_with("r4", "k: 7\n");
    // Guard: the reach must actually happen, or this proves nothing.
    let reached = oo(&d, &["eval", &format!("_{{{addr}}}.k")]);
    assert!(
        reached.contains('7'),
        "precondition: the address must resolve before purity means anything, got: {reached}"
    );
    // The property: same content, same address, whichever way it was reached.
    let via = oo(
        &d,
        &["eval", &format!("~%Discovery./identify ({{ v: _{{{addr}}}.k }})")],
    );
    let lit = oo(&d, &["eval", "~%Discovery./identify ({ v: 7 })"]);
    assert_eq!(
        via.trim(),
        lit.trim(),
        "reaching a pure value by address must not move the reacher"
    );
}

/// R5  But the effect of the VALUE still shows (O70 ④). Purity belongs to
/// the resolution, not to the thing resolved. This is the half the
/// acceptance side got wrong first time round: "the fetched value is
/// still pure" is false -- it is whatever it is.
#[test]
#[ignore]
fn r5_the_value_keeps_its_own_effect() {
    let (d, addr) = universe_with("r5", "f: ~%Math./random\n");
    let reached = oo(&d, &["eval", &format!("_{{{addr}}}.f")]);
    assert!(
        reached.contains("math.random"),
        "precondition: the address must resolve to the morphism, got: {reached}"
    );
    let out = oo(&d, &["eval", &format!("(_{{{addr}}}.f).%effect")]);
    assert!(
        out.contains("nondet"),
        "the value's own effect must survive being reached by address, got: {out}"
    );
}

/// R6  Printing must emit legal source (SYNTAX_02 §5 #12). A value or a
/// program holding an address literal must round-trip through `oo fmt`
/// as the same literal -- not as a combo with a `sha256` field.
#[test]
#[ignore]
fn r6_an_address_literal_prints_back_as_itself() {
    let d = scratch("r6");
    let src = format!("out: _{{sha256:{HEX}}}.k\n");
    std::fs::write(d.join("a.n"), &src).unwrap();
    let out = oo(&d, &["fmt", "a.n"]);
    assert!(
        out.contains(&format!("_{{sha256:{HEX}}}")),
        "fmt must print the address literal back as itself, got: {out}"
    );
}
