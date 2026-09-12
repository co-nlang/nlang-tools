// The address names the point.
// Recon:  nlang-tools/docs/an_address_that_never_promised_those_bytes_recon.md
// Order:  nlang-tools/docs/the_address_names_the_point_handover.md
// Ruling: D65, ruled 2026-09-12. Three parts: drop `message`, leave `%cause`
//         alone, print a bottom as a bare atom with its cause in the
//         annotation layer.
//
// -- What this arc is ----------------------------------------------------
//
// Four different universes commit to one root address:
//
//     bad: 1 & 2                -> cbb7ef81...  { bad: { ~%__nlang_bottom: #conflict message: "..1 vs 2" } .. }
//     bad: 1 & 3                -> cbb7ef81...  { bad: { ~%__nlang_bottom: #conflict message: "..1 vs 3" } .. }
//     bad: bad + 1              -> cbb7ef81...  { bad: { ~%__nlang_bottom: #divergent } .. }
//     bad: ~%Math./add (1,"x")  -> cbb7ef81...  { bad: { ~%__nlang_bottom: #conflict } .. }
//
// Copy one repo's root object into another repo at the same path and the
// store serves the other bytes at the same address, exit zero, with nothing
// to say about it. What was copied in is not a forgery: it is an object
// another universe produced legitimately.
//
// REAL_03 6.7 has two clauses on this, verbatim: the bytes at a CAID must be
// a function of the value alone, and an object must not carry data that does
// not participate in its own address. The named precedent for the second is
// the source span, and the span was removed in v0.13.0. This is the second
// instance of an adjudicated MUST NOT.
//
// -- Why the cause stays and the message goes -----------------------------
//
// They look alike and they are not. `%cause` has a formal observation
// channel (`.%cause`), and REAL_03 6.9's second clause permits exactly this:
// a field outside the address MAY be observable on the causal channel,
// because it does not change what the value is. The precedent there is
// TopCaused -- and the top of the lattice stores its cause the same way.
//
// `message` has no channel at all. Under SPEC_11 3.4 that makes it
// non-semantic, and 3.4 says dropping a non-semantic annotation on reparse
// "does not constitute information loss". It is also not one of 3.4's two
// admitted members (`;; %effect:` and `#ext:`), so it is a third member that
// was never registered. So: no channel, not admitted, yet persisted.
//
// -- Why the printed form moves ------------------------------------------
//
// `_|_ (%cause: #conflict)` fails to parse back at column 15, at the
// parenthesis, not at the trailing comment. The grammar defines a bottom
// only as a bare atom. The engine put the cause inside the value body, where
// canonical spelling is required, when `%effect` had already demonstrated the
// shape: a property of the value, a formal channel, and a `;;` suffix. So the
// cause moves to `;; %cause: <tag>` and the value body becomes bare.
//
// -- Removal has two forms ------------------------------------------------
//
// Every red here could be reached by deleting the cause outright. G2 and G5
// close that door: the channel must keep answering at BOTH ends of the
// lattice, and the top's stored cause must survive.
//
// -- Explicitly NOT probed ------------------------------------------------
//
//   * Whether a commit object's `message:` participates in the commit's own
//     address. Commit objects carry a timestamp, so the acceptor could not
//     isolate the message's contribution; the work order asks for the answer
//     rather than guessing at it. Nothing here asserts anything about commit
//     objects.
//   * `~%__nlang_top_cause`'s `members: []` field. Same question, same
//     reason, also a work-order question.
//   * Whether the cause is reachable from a program that reads a COMMITTED
//     universe. It is not, today, and that is Q-018, not this arc.
//
// -- Do not let rustfmt sweep this file ----------------------------------

#![cfg(unix)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

const OO: &str = env!("CARGO_BIN_EXE_oo");

struct Run {
    stdout: String,
    stderr: String,
    code: i32,
}

impl Run {
    fn both(&self) -> String {
        format!("{}{}", self.stdout, self.stderr)
    }
}

/// Run `oo`. stdout and stderr stay apart; the exit code comes from the
/// process, never from the end of a pipeline.
fn oo(dir: &Path, args: &[&str]) -> Run {
    let o = Command::new(OO)
        .current_dir(dir)
        .env("OO_IDENTITY", dir.join("identity-for-tests"))
        .env("OO_NODE_HOME", dir.join("node-home-for-tests"))
        .args(args)
        .output()
        .expect("oo runs");
    Run {
        stdout: String::from_utf8_lossy(&o.stdout).to_string(),
        stderr: String::from_utf8_lossy(&o.stderr).to_string(),
        code: o.status.code().unwrap_or(-1),
    }
}

fn scratch(tag: &str) -> nlang_interpreter::ScratchDir {
    nlang_interpreter::ScratchDir::new(&format!("point-{tag}"))
}

/// Evolve one source and commit it. Returns the directory.
fn universe(d: &Path, source: &str) -> bool {
    std::fs::write(d.join("main.n"), format!("{source}\n")).expect("fixture written");
    let e = oo(d, &["evolve", "main.n"]);
    let c = oo(d, &["commit", "-m", "m"]);
    e.code == 0 && c.code == 0
}

/// Every object file in a workspace's store, keyed by its address (the path
/// under `objects/` with the directory split removed).
fn objects(d: &Path) -> BTreeMap<String, (PathBuf, String)> {
    let mut out = BTreeMap::new();
    let root = d.join(".oo/objects");
    let mut stack = vec![root];
    while let Some(p) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&p) else { continue };
        for e in rd.flatten() {
            let path = e.path();
            if path.is_dir() {
                stack.push(path);
            } else if let Ok(body) = std::fs::read_to_string(&path) {
                let rel = path.strip_prefix(d.join(".oo/objects")).unwrap_or(&path);
                let addr: String = rel.components().map(|c| c.as_os_str().to_string_lossy().to_string()).collect();
                out.insert(addr, (path, body));
            }
        }
    }
    out
}

/// The stored root object of a workspace: the one framed `#nlang/store` that
/// names a standard root. `None` means the measurement did not reach it.
fn root_object(d: &Path) -> Option<(String, PathBuf, String)> {
    objects(d).into_iter().find_map(|(addr, (p, body))| {
        let mut lines = body.lines();
        (lines.next()? == "#nlang/store" && body.contains("__nlang_system_digest"))
            .then(|| (addr, p, body))
    })
}

/// The value body of a printed value: everything before the annotation layer.
fn value_body(printed: &str) -> String {
    match printed.find(";;") {
        Some(i) => printed[..i].trim().to_string(),
        None => printed.trim().to_string(),
    }
}

/// Feed a printed value back through the formatter as the RHS of a field.
/// `true` means it parsed.
fn reads_back(d: &Path, printed: &str) -> Run {
    std::fs::write(d.join("back.n"), format!("v: {}\n", value_body(printed))).expect("fixture written");
    oo(d, &["fmt", "back.n"])
}

/// Blank out the fields a formal channel answers for, so that what remains is
/// exactly what the address is supposed to determine. `%cause` at either end
/// of the lattice is such a field (REAL_03 6.9's second clause, precedent
/// TopCaused); everything else is not.
fn blank_channelled(body: &str) -> String {
    let mut out = String::with_capacity(body.len());
    let mut rest = body;
    for marker in ["~%__nlang_bottom: #", "cause: #"] {
        loop {
            let Some(i) = rest.find(marker) else { break };
            let after = i + marker.len();
            let end = rest[after..]
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .map(|k| after + k)
                .unwrap_or(rest.len());
            out.push_str(&rest[..after]);
            out.push_str("<CHANNELLED>");
            rest = &rest[end..];
        }
        out.push_str(rest);
        rest = "";
        // Second pass runs over what the first pass produced.
        let carried = std::mem::take(&mut out);
        rest = Box::leak(carried.into_boxed_str());
    }
    rest.to_string()
}

// =====================================================================
// G1-G6: green at the baseline, and they are this arc's red lines.
// =====================================================================

/// G1 -- the known-answer question.
#[test]
fn g1_the_engine_can_still_answer_a_question_whose_answer_is_known() {
    let d = scratch("g1");
    let r = oo(d.path(), &["eval", "1 + 1"]);
    assert_eq!(r.code, 0, "control failed:\n{}", r.both());
    assert_eq!(r.stdout.trim(), "2", "control gave the wrong answer:\n{}", r.both());
}

/// G2 -- the causal channel must keep answering, at BOTH ends of the
/// lattice. Dropping the cause is not a way to satisfy this arc.
#[test]
fn g2_the_causal_channel_answers_at_both_ends_of_the_lattice() {
    let d = scratch("g2");
    let conflict = oo(d.path(), &["eval", "(1 & 2).%cause"]);
    assert_eq!(conflict.stdout.trim(), "#conflict", "the conflict cause is gone:\n{}", conflict.both());

    let top = oo(d.path(), &["eval", "(_).%cause"]);
    assert_eq!(top.stdout.trim(), "_", "an uncaused top should read `_`:\n{}", top.both());

    // A channel that answers the same thing for everything is not a channel.
    let unrelated = oo(d.path(), &["eval", "(1 & 2).%nosuchmeta"]);
    assert_ne!(
        unrelated.stdout.trim(),
        "#conflict",
        "VOID READING: `.%cause` may be a catch-all rather than a channel:\n{}",
        unrelated.both()
    );
}

/// G3 -- a bottom is a value, so the carrier rule (D63) keeps it at rc=0.
#[test]
fn g3_a_bottom_is_still_a_value() {
    let d = scratch("g3");
    let r = oo(d.path(), &["eval", "1 & 2"]);
    assert_eq!(r.code, 0, "a bottom is a value; D63 puts it at rc=0:\n{}", r.both());
    assert!(r.stdout.contains("_|_"), "no bottom was produced:\n{}", r.both());
}

/// G4 -- identity. None of this arc may move an address.
#[test]
fn g4_identity_is_a_red_line() {
    let d = scratch("g4");
    assert!(universe(d.path(), "x: 0"), "control: `x: 0` did not commit");
    let (addr, _, _) = root_object(d.path()).expect("VOID READING: no root object was found");
    assert!(
        addr.contains("31745ef0e8bfde3d8a2673b7dce5bb5cd74f3a7f2cc6f5422aa043c8dce5589a"),
        "the root of `x: 0` moved: {addr}"
    );
    assert_eq!(objects(d.path()).len(), 3, "the object count changed: {:?}", objects(d.path()).keys());

    let b = scratch("g4b");
    assert!(universe(b.path(), "bad: 1 & 2"), "control: the bottom universe did not commit");
    let (baddr, _, _) = root_object(b.path()).expect("VOID READING: no root object was found");
    assert!(
        baddr.contains("cbb7ef81861ad908234741642a1fa33071c183d391cb157dbe2b24ae90677a1c"),
        "the root of a committed bottom moved: {baddr}"
    );
}

/// G5 -- the top of the lattice stores its cause, and must keep doing so.
/// `%cause` has a channel; S1 removes fields that have none.
#[test]
fn g5_the_top_of_the_lattice_keeps_its_cause() {
    let d = scratch("g5");
    assert!(universe(d.path(), "a: b\nb: a"), "control: the cycle universe did not commit");
    let (_, _, body) = root_object(d.path()).expect("VOID READING: no root object was found");
    assert!(
        body.contains("__nlang_top_cause"),
        "the top's stored cause disappeared -- `%cause` is not what this arc removes:\n{body}"
    );
}

/// G6 -- S3. A store written by an older engine carries `message` on its
/// bottoms, and must stay readable.
#[test]
fn g6_a_store_that_still_carries_the_old_field_is_readable() {
    let d = scratch("g6");
    assert!(universe(d.path(), "bad: bad + 1"), "control: the divergent universe did not commit");
    let (addr, path, body) = root_object(d.path()).expect("VOID READING: no root object was found");

    let before = oo(d.path(), &["status"]);
    assert_eq!(before.code, 0, "control: `status` failed before the edit:\n{}", before.both());

    let legacy = body.replace("~%__nlang_bottom: #divergent", "~%__nlang_bottom: #divergent message: \"from an older engine\"");
    assert_ne!(legacy, body, "VOID READING: the legacy field was never injected");
    std::fs::write(&path, &legacy).expect("legacy object written");

    let after = oo(d.path(), &["status"]);
    assert_eq!(after.code, 0, "a store carrying the old field stopped being readable:\n{}", after.both());
    let caid = format!("hash:sha256:v1:{}", addr.trim_start_matches("sha256"));
    let inspect = oo(d.path(), &["inspect", &caid]);
    assert_eq!(inspect.code, 0, "`inspect` refused a store carrying the old field:\n{}", inspect.both());
}

// =====================================================================
// R1-R5: red at the baseline, each failing on its own assertion.
// =====================================================================

/// R1 -- S1 as a class. A stored object may carry a field only if the field
/// enters the address or is readable on a formal channel. `message` is
/// neither.
#[test]
fn r1_no_stored_object_carries_a_field_with_no_channel() {
    let sources = ["bad: 1 & 2", "bad: 1 & 3", "bad: bad + 1", "bad: ~%Math./add (1, \"x\")"];
    let mut reached = 0;
    let mut carrying: Vec<String> = Vec::new();
    for (i, src) in sources.iter().enumerate() {
        let d = scratch(&format!("r1-{i}"));
        if !universe(d.path(), src) {
            continue;
        }
        let Some((addr, _, body)) = root_object(d.path()) else { continue };
        if !body.contains("__nlang_bottom") {
            continue; // this cell did not store a bottom: it measured nothing
        }
        reached += 1;
        if body.contains("message:") {
            carrying.push(format!("{src} -> {addr}"));
        }
    }
    assert_eq!(
        reached,
        sources.len(),
        "VOID READING: only {reached} of {} universes stored a bottom",
        sources.len()
    );
    assert!(
        carrying.is_empty(),
        "{} of {} stored bottoms carry a field with no formal channel:\n{}",
        carrying.len(),
        sources.len(),
        carrying.join("\n")
    );
}

/// R2 -- S1's other half. REAL_03 6.7's first clause says the bytes at an
/// address are a function of the value alone, and 6.9's second clause carves
/// out one exception: a field outside the address MAY differ if it is
/// readable on a formal channel. D65 puts `%cause` inside that exception on
/// purpose, at both ends of the lattice.
///
/// So the property is not "one address, one byte string" -- that reading
/// contradicts the ruling this arc implements, and the acceptor wrote it
/// that way by mistake (see the handover's acceptance round). It is:
///
///     two objects at one address may differ ONLY where a channel answers.
///
/// The comparison therefore blanks the channelled fields and requires the
/// remainder to be byte-identical. At the baseline the remainder still
/// differs, by a field with no channel at all.
#[test]
fn r2_addresses_differ_only_where_a_channel_answers() {
    let sources = ["bad: 1 & 2", "bad: 1 & 3", "bad: bad + 1", "bad: ~%Math./add (1, \"x\")"];
    let mut by_address: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    for (i, src) in sources.iter().enumerate() {
        let d = scratch(&format!("r2-{i}"));
        if !universe(d.path(), src) {
            continue;
        }
        if let Some((addr, _, body)) = root_object(d.path()) {
            by_address.entry(addr).or_default().push((src.to_string(), body));
        }
    }
    let shared: Vec<_> = by_address.iter().filter(|(_, v)| v.len() > 1).collect();
    assert!(
        !shared.is_empty(),
        "VOID READING: no two of {} universes shared a root address, so nothing was compared",
        sources.len()
    );

    let mut split: Vec<String> = Vec::new();
    for (addr, uses) in &shared {
        let blanked: Vec<(String, String)> =
            uses.iter().map(|(src, body)| (src.clone(), blank_channelled(body))).collect();
        // Reachability: blanking must actually have removed something,
        // otherwise this probe is comparing raw bytes under another name.
        assert!(
            blanked.iter().zip(uses.iter()).any(|((_, b), (_, raw))| b != raw),
            "VOID READING: no channelled field was recognised in {addr}"
        );
        let first = &blanked[0].1;
        let differing: Vec<&str> =
            blanked.iter().filter(|(_, b)| b != first).map(|(s, _)| s.as_str()).collect();
        if !differing.is_empty() {
            split.push(format!(
                "  {addr}\n    is shared by {} universes and {} of them differ OUTSIDE any channel: {differing:?}",
                uses.len(),
                differing.len()
            ));
        }
    }
    assert!(
        split.is_empty(),
        "{} of {} shared addresses differ where no channel answers:\n{}",
        split.len(),
        shared.len(),
        split.join("\n")
    );
}

/// R3 -- S2. The value body of a bottom is legal n/.
#[test]
fn r3_a_bottom_reads_back() {
    let d = scratch("r3");
    // The instrument first: a form that must parse, and one that must not.
    std::fs::write(d.path().join("ok.n"), "v: 1\n").expect("fixture written");
    assert_eq!(oo(d.path(), &["fmt", "ok.n"]).code, 0, "VOID READING: the formatter rejects `v: 1`");
    std::fs::write(d.path().join("no.n"), "v: %%bogus((\n").expect("fixture written");
    assert_ne!(oo(d.path(), &["fmt", "no.n"]).code, 0, "VOID READING: the formatter accepts nonsense");

    let printed = oo(d.path(), &["eval", "1 & 2"]);
    assert!(printed.stdout.contains("_|_"), "VOID READING: no bottom was printed:\n{}", printed.both());
    let back = reads_back(d.path(), &printed.stdout);
    assert_eq!(
        back.code,
        0,
        "the value body {:?} is not legal n/:\n{}",
        value_body(&printed.stdout),
        back.both()
    );
}

/// R4 -- S2, and the reason it matters: an illegal bottom form makes every
/// record containing one unreadable too.
#[test]
fn r4_a_record_holding_a_bottom_reads_back() {
    let d = scratch("r4");
    let printed = oo(d.path(), &["eval", "{ a: 1, b: 1 & 2 }"]);
    assert!(
        printed.stdout.contains("_|_"),
        "VOID READING: the record did not contain a bottom:\n{}",
        printed.both()
    );
    std::fs::write(d.path().join("back.n"), format!("v: {}\n", printed.stdout.trim())).expect("fixture written");
    let back = oo(d.path(), &["fmt", "back.n"]);
    assert_eq!(back.code, 0, "a record holding a bottom does not read back:\n{}", back.both());
}

/// R5 -- S2's placement. The cause belongs in the annotation layer, spelled
/// the way `%effect` already is, and the value body must be bare.
#[test]
fn r5_the_cause_rides_the_annotation_layer() {
    let d = scratch("r5");
    let printed = oo(d.path(), &["eval", "1 & 2"]);
    let out = printed.stdout.trim().to_string();
    assert!(out.contains("_|_"), "VOID READING: no bottom was printed:\n{}", printed.both());
    assert!(
        out.contains(";; %cause:"),
        "the cause is not on the annotation layer (`%effect` is the spelling to match):\n{out}"
    );
    assert_eq!(
        value_body(&out),
        "_|_",
        "the value body still carries more than the bare atom:\n{out}"
    );
}
