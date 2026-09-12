// The last thing it says.
// Recon:  the 2026-09-12 inventory in nlang-spec/meta/WORK_QUEUE.md §3
// Order:  nlang-tools/docs/the_last_thing_it_says_handover.md
// Ruling: none needed. Every property here is already written down; this arc
//         is the third and last family of "the engine answers in the host's
//         words", after Q-042 closed the source-file half.
//
// -- What this arc is ----------------------------------------------------
//
// Three failures, all of the engine's OWN io, all already forbidden.
//
// 1. A reader that stops early. `oo status | head -1`:
//
//      thread 'oo-main' panicked at /rustc/…/library/std/src/io/stdio.rs:1165:9:
//      failed printing to stdout: Broken pipe (os error 32)      exit 101
//
//    That one line breaks REAL_01 1.3's first clause twice over -- a host
//    source location and a bare errno -- and the panic itself is the third
//    member of the family D63 named a month ago: the engine answering with
//    the host's panic a question it has something of its own to say about.
//    There are 101 print sites in main.rs alone, so this cannot be fixed
//    call-site by call-site; nothing anywhere touches SIGPIPE today.
//
// 2. The store's own reads. With `.oo/HEAD` unreadable, `oo log` and
//    `oo status` both hand over `Permission denied (os error 13)`. Q-042
//    collapsed the SOURCE-file reads behind one total mapping; the store's
//    reads live in another crate and were never behind it. Measured across
//    six injections, two leak.
//
// 3. And two of the four that do NOT leak say something false instead. With
//    `.oo/objects` unreadable, the answer is `CAID not found in local
//    store`. The object is there. REAL_03 6.6 forbids this in as many words:
//    an implementation must not collapse the three verification outcomes
//    (together with "absent") into one reply, because that makes an intact
//    library and a tampered one indistinguishable.
//
// -- Fix the class, not the instance -------------------------------------
//
// R1/R2 name two commands and R3/R4 name six injections, but none of those
// lists is the property. A patch that special-cases `status` and `log`
// passes R1 and is wrong: the invariant is that NO operator-facing output
// path answers a closed reader with a panic. Same for the store: the
// invariant is that the engine's own io failures are named by the engine.
//
// -- Removal has two forms ------------------------------------------------
//
// Every red here could be turned green by making the engine quieter: exit 0
// on a closed pipe and say nothing, swallow the store error, or answer every
// store failure with one word. G2, G4 and R4 close those: normal output must
// be unchanged, every injected failure must still exit non-zero, and two
// different failures must still get two different answers.
//
// -- Explicitly NOT probed ------------------------------------------------
//
//   * WHETHER a closed pipe should end in silence or in a named refusal.
//     That is the carrier question, it needs a ruling, and D63 ordered it
//     behind the arc-D `init` work. This arc only forbids the panic. A quiet
//     exit and a named refusal both pass everything here.
//   * Which OTHER commands can hit the pipe panic. `status` and `log` do it
//     reproducibly; `fmt`, `run`, `lint` and `eval` did not, and the acceptor
//     could not make them -- it is not output size (`fmt` wrote 71 KB through
//     a closed reader without panicking while `status` panicked on 143 B).
//     The work order asks the delivery to say what actually decides it.
//
// -- Do not let rustfmt sweep this file ----------------------------------

#![cfg(unix)]

use std::path::Path;
use std::process::{Command, Stdio};

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

/// Run `oo <args>` with a reader that takes one line and leaves. The exit
/// code is the ENGINE's, taken from its own process — never from the end of
/// the pipeline.
fn oo_into_early_reader(dir: &Path, args: &[&str]) -> Run {
    let mut child = Command::new(OO)
        .current_dir(dir)
        .env("OO_IDENTITY", dir.join("identity-for-tests"))
        .env("OO_NODE_HOME", dir.join("node-home-for-tests"))
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("oo spawns");
    // Read one line, then drop the pipe: the reader stopped listening.
    {
        use std::io::{BufRead, BufReader};
        let out = child.stdout.take().expect("stdout piped");
        let mut r = BufReader::new(out);
        let mut line = String::new();
        let _ = r.read_line(&mut line);
    }
    let o = child.wait_with_output().expect("oo exits");
    Run {
        stdout: String::new(),
        stderr: String::from_utf8_lossy(&o.stderr).to_string(),
        code: o.status.code().unwrap_or(-1),
    }
}

fn scratch(tag: &str) -> nlang_interpreter::ScratchDir {
    nlang_interpreter::ScratchDir::new(&format!("lastword-{tag}"))
}

/// A committed workspace with enough fields that its readers print more than
/// one line.
fn committed(d: &Path) -> bool {
    let body: String = (1..=200).map(|i| format!("field_number_{i}: {i}\n")).collect();
    std::fs::write(d.join("main.n"), body).expect("fixture written");
    oo(d, &["evolve", "main.n"]).code == 0 && oo(d, &["commit", "-m", "m"]).code == 0
}

fn chmod(p: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(p, std::fs::Permissions::from_mode(mode));
}

/// Tokens only the host would put in front of an operator.
fn host_words(text: &str) -> Vec<&'static str> {
    let mut found = Vec::new();
    for t in ["os error", "panicked at", "/rustc/", "library/std", "RUST_BACKTRACE"] {
        if text.contains(t) {
            found.push(t);
        }
    }
    found
}

/// The six ways this arc breaks the engine's own store. Each returns
/// (label, the command to run, a setup closure, a teardown closure).
fn store_injections() -> Vec<(&'static str, Vec<&'static str>, fn(&Path), fn(&Path))> {
    vec![
        ("HEAD unreadable / log", vec!["log"], |d: &Path| chmod(&d.join(".oo/HEAD"), 0o000), |d: &Path| chmod(&d.join(".oo/HEAD"), 0o644)),
        ("HEAD unreadable / status", vec!["status"], |d: &Path| chmod(&d.join(".oo/HEAD"), 0o000), |d: &Path| chmod(&d.join(".oo/HEAD"), 0o644)),
        ("objects unreadable / log", vec!["log"], |d: &Path| chmod(&d.join(".oo/objects"), 0o000), |d: &Path| chmod(&d.join(".oo/objects"), 0o755)),
        ("objects unreadable / commit", vec!["commit", "-m", "z"], |d: &Path| chmod(&d.join(".oo/objects"), 0o000), |d: &Path| chmod(&d.join(".oo/objects"), 0o755)),
        ("format unreadable / status", vec!["status"], |d: &Path| chmod(&d.join(".oo/format"), 0o000), |d: &Path| chmod(&d.join(".oo/format"), 0o644)),
        ("the root object removed / log", vec!["log"], |d: &Path| {
            if let Some(f) = stored_root(d) {
                let _ = std::fs::remove_file(f);
            }
        }, |_: &Path| {}),
    ]
}

/// The workspace's own root object — the framed one that names a standard
/// root. Picking "whichever file sorts first" is not the same thing: which
/// object that is depends on the fixture, and a reader that never needed it
/// would go on succeeding.
fn stored_root(d: &Path) -> Option<std::path::PathBuf> {
    walk_objects(d).into_iter().find(|p| {
        std::fs::read_to_string(p)
            .map(|b| b.starts_with("#nlang/store") && b.contains("__nlang_system_digest"))
            .unwrap_or(false)
    })
}

fn walk_objects(d: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![d.join(".oo/objects")];
    while let Some(p) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&p) else { continue };
        for e in rd.flatten() {
            let path = e.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

// =====================================================================
// G1-G5: green at the baseline, and they are this arc's red lines.
// =====================================================================

/// G1 -- the known-answer question.
#[test]
fn g1_the_engine_can_still_answer_a_question_whose_answer_is_known() {
    let d = scratch("g1");
    let r = oo(d.path(), &["eval", "1 + 1"]);
    assert_eq!(r.code, 0, "control failed:\n{}", r.both());
    assert_eq!(r.stdout.trim(), "2", "control gave the wrong answer:\n{}", r.both());
}

/// G2 -- the ordinary path must not change. Nothing here may be reached by
/// making the engine quieter when nobody closed anything.
#[test]
fn g2_an_uninterrupted_reader_still_gets_everything() {
    let d = scratch("g2");
    assert!(committed(d.path()), "control: the fixture did not commit");
    for cmd in [["status"], ["log"]] {
        let r = oo(d.path(), &cmd);
        assert_eq!(r.code, 0, "`oo {}` failed with nobody interrupting:\n{}", cmd[0], r.both());
        assert!(r.stderr.is_empty(), "`oo {}` wrote to stderr unprompted:\n{}", cmd[0], r.stderr);
        assert!(!r.stdout.trim().is_empty(), "`oo {}` printed nothing:\n{}", cmd[0], r.both());
    }
}

/// G3 -- identity.
#[test]
fn g3_identity_is_a_red_line() {
    let d = scratch("g3");
    std::fs::write(d.path().join("main.n"), "x: 0\n").expect("fixture written");
    assert_eq!(oo(d.path(), &["evolve", "main.n"]).code, 0, "control: evolve failed");
    assert_eq!(oo(d.path(), &["commit", "-m", "m"]).code, 0, "control: commit failed");
    let objs = walk_objects(d.path());
    assert_eq!(objs.len(), 3, "the object count changed: {objs:?}");
    // The object directory splits a digest after two characters, so join the
    // path components back up before looking for it.
    let names: Vec<String> =
        objs.iter().map(|p| p.to_string_lossy().replace('/', "")).collect();
    assert!(
        names.iter().any(|n| n.contains("31745ef0e8bfde3d8a2673b7dce5bb5cd74f3a7f2cc6f5422aa043c8dce5589a")),
        "the root of `x: 0` moved: {objs:?}"
    );
}

/// G4 -- a broken store must still fail. None of this may be reached by
/// swallowing the failure.
#[test]
fn g4_a_broken_store_still_fails() {
    let mut checked = 0;
    for (label, args, setup, teardown) in store_injections() {
        let d = scratch(&format!("g4-{}", checked));
        assert!(committed(d.path()), "control: the fixture did not commit for {label}");
        setup(d.path());
        let r = oo(d.path(), &args.iter().copied().collect::<Vec<_>>());
        teardown(d.path());
        assert_ne!(r.code, 0, "{label}: a broken store reported success:\n{}", r.both());
        checked += 1;
    }
    assert_eq!(checked, 6, "denominator: expected 6 injections, ran {checked}");
}

/// G5 -- Q-042's result must not regress: source-file reads stay named.
#[test]
fn g5_the_source_file_reads_stay_named() {
    let d = scratch("g5");
    for cmd in [["run", "nosuch.n"], ["evolve", "nosuch.n"], ["fmt", "nosuch.n"]] {
        let r = oo(d.path(), &cmd);
        assert_ne!(r.code, 0, "`oo {}` should have failed:\n{}", cmd.join(" "), r.both());
        let found = host_words(&r.both());
        assert!(found.is_empty(), "Q-042 regressed on `oo {}`: {found:?}\n{}", cmd.join(" "), r.both());
    }
}

// =====================================================================
// R1-R4: red at the baseline, each failing on its own assertion.
// =====================================================================

/// R1 -- a reader that stops early must not make the engine panic.
#[test]
fn r1_a_reader_who_stops_early_does_not_crash_the_engine() {
    let mut reached = 0;
    let mut panicking: Vec<String> = Vec::new();
    for (i, cmd) in [["status"], ["log"]].iter().enumerate() {
        let d = scratch(&format!("r1-{i}"));
        assert!(committed(d.path()), "control: the fixture did not commit");
        // The command must print more than one line, or closing after one
        // line interrupts nothing and this cell measured nothing.
        let full = oo(d.path(), cmd);
        if full.code != 0 || full.stdout.lines().count() < 2 {
            continue;
        }
        reached += 1;
        let r = oo_into_early_reader(d.path(), cmd);
        if r.stderr.contains("panicked") {
            panicking.push(format!("oo {} -> exit {} :: {}", cmd[0], r.code, r.stderr.trim()));
        }
    }
    assert_eq!(reached, 2, "VOID READING: only {reached} of 2 commands printed more than one line");
    assert!(
        panicking.is_empty(),
        "{} of 2 commands answered a closed reader with a panic:\n{}",
        panicking.len(),
        panicking.join("\n")
    );
}

/// R2 -- and whatever it does say must be its own words.
#[test]
fn r2_a_closed_reader_is_not_answered_in_the_host_s_words() {
    let mut reached = 0;
    let mut leaking: Vec<String> = Vec::new();
    for (i, cmd) in [["status"], ["log"]].iter().enumerate() {
        let d = scratch(&format!("r2-{i}"));
        assert!(committed(d.path()), "control: the fixture did not commit");
        let full = oo(d.path(), cmd);
        if full.code != 0 || full.stdout.lines().count() < 2 {
            continue;
        }
        reached += 1;
        let r = oo_into_early_reader(d.path(), cmd);
        let found = host_words(&r.stderr);
        if !found.is_empty() {
            leaking.push(format!("oo {} -> {found:?} :: {}", cmd[0], r.stderr.trim()));
        }
    }
    assert_eq!(reached, 2, "VOID READING: only {reached} of 2 commands printed more than one line");
    assert!(
        leaking.is_empty(),
        "{} of 2 commands answered a closed reader in the host's words:\n{}",
        leaking.len(),
        leaking.join("\n")
    );
}

/// R3 -- the store's own io failures are named by the engine.
#[test]
fn r3_the_store_names_its_own_failures() {
    let mut reached = 0;
    let mut leaking: Vec<String> = Vec::new();
    let injections = store_injections();
    let total = injections.len();
    for (i, (label, args, setup, teardown)) in injections.into_iter().enumerate() {
        let d = scratch(&format!("r3-{i}"));
        assert!(committed(d.path()), "control: the fixture did not commit for {label}");
        setup(d.path());
        let r = oo(d.path(), &args.iter().copied().collect::<Vec<_>>());
        teardown(d.path());
        if r.code == 0 {
            continue; // it did not fail: this cell measured nothing
        }
        reached += 1;
        let found = host_words(&r.both());
        if !found.is_empty() {
            leaking.push(format!("  {label} -> {found:?} :: {}", r.both().trim()));
        }
    }
    assert_eq!(reached, total, "VOID READING: only {reached} of {total} injections actually failed");
    assert!(
        leaking.is_empty(),
        "{} of {total} store failures were reported in the host's words:\n{}",
        leaking.len(),
        leaking.join("\n")
    );
}

/// R4 -- REAL_03 6.6. An object that is present but unreadable must not be
/// reported the same way as an object that is not there. This asserts only
/// that the two answers DIFFER, so it prescribes no vocabulary.
#[test]
fn r4_unreadable_is_not_reported_as_absent() {
    let absent = scratch("r4-absent");
    assert!(committed(absent.path()), "control: the fixture did not commit");
    let victim = stored_root(absent.path()).expect("VOID READING: no stored root object");
    std::fs::remove_file(&victim).expect("object removed");
    let a = oo(absent.path(), &["log"]);

    let opaque = scratch("r4-opaque");
    assert!(committed(opaque.path()), "control: the fixture did not commit");
    chmod(&opaque.path().join(".oo/objects"), 0o000);
    let b = oo(opaque.path(), &["log"]);
    chmod(&opaque.path().join(".oo/objects"), 0o755);

    assert_ne!(a.code, 0, "VOID READING: removing an object did not fail:\n{}", a.both());
    assert_ne!(b.code, 0, "VOID READING: an unreadable store did not fail:\n{}", b.both());

    // Compare the shape of the sentence, not the CAID inside it.
    let strip = |s: &str| -> String {
        s.split_whitespace()
            .filter(|w| !w.starts_with("hash:"))
            .collect::<Vec<_>>()
            .join(" ")
    };
    assert_ne!(
        strip(&a.both()),
        strip(&b.both()),
        "an absent object and an unreadable one get the same answer, so an intact \
         store and a tampered one are indistinguishable (REAL_03 §6.6):\n  absent:   {}\n  opaque:   {}",
        a.both().trim(),
        b.both().trim()
    );
}
