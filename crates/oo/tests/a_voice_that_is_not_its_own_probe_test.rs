// A voice that is not its own.
// Recon:   nlang-tools/docs/is_the_cli_a_promise_recon.md
// Order:   nlang-tools/docs/a_voice_that_is_not_its_own_handover.md
// Ruling:  D64 (Q-018 takes jia-prime), ruled 2026-09-12.
//
// -- What this arc is ----------------------------------------------------
//
// REAL_01 1.3 says the engine must not answer in the host's internal
// representation, and 1.3's new fifth clause says every command, flag and
// positional argument must carry one sentence saying what it does. 1.4 says
// one concept gets one spelling, one signature and one description.
//
// Today, measured on the v0.46.0 tag build:
//
//     oo eval '1 & 2'
//     _|_ (%cause: #conflict)  ;; Incompatible types:
//         Atom(Int(1), EffectTag(0), None) vs Atom(Int(2), EffectTag(0), None)
//
// `Atom(...)`, `EffectTag(...)` and a bare `None` are Rust's Debug. The same
// family appears four more times: four CLI entry points hand the operator a
// raw `io::Error` -- `Error: No such file or directory (os error 2)` -- with
// no context at all (run_evolve, run_one_shot, run_fmt, run_test, each a
// bare `fs::read_to_string(&file)?`).
//
// And 9 of 18 top-level commands, 18 of 28 flags and 4 of 9 positional
// arguments say nothing whatsoever about themselves. The capability-grant
// flag is described on four commands and blank on four others -- and those
// four were not added together: they came from three separate changes, so
// the same omission repeated itself three times with nothing to catch it.
//
// -- Fix the class, not the instance -------------------------------------
//
// R1 names one expression and R2 names four entry points, but the invariant
// is neither: the engine must not answer in the host's words. A patch that
// special-cases `1 & 2` passes R1 and is wrong. R3-R6 are written as whole-
// surface sweeps with a printed denominator precisely so that no instance
// can be fixed alone.
//
// -- Removal has two forms ------------------------------------------------
//
// Every red here could be turned green by DELETING something: drop the
// diagnostic instead of rewriting it (R1), swallow the io error and say
// nothing (R2), or delete the four descriptions that exist so that "all
// shared flags agree" holds vacuously (R6). G2, G3 and G4 exist to close
// those three doors. G4 in particular pins a FLOOR under how much the CLI
// already says.
//
// -- Explicitly NOT probed, so no one mistakes silence for coverage ------
//
//   * The canonical printed form of `_|_`. `_|_ (%cause: #conflict)` is not
//     legal n/ -- it fails to parse back at column 15, at `(%cause:`, not at
//     the `;;` comment -- but the grammar never defined that notation and
//     the spec uses it verbatim in dozens of places. That is O85, it needs a
//     ruling, and it is excluded from this arc's scope by name. R1 asks only
//     that the CONTENT stop being the host's; it does not ask that the FORM
//     become legal.
//   * `--format`'s arity. The flag exists and takes no value while REAL_01
//     wrote `--format json|n`. Spellings are Reference as of D64, so the
//     mismatch is not a defect of the engine. The `os error 2` it emits when
//     handed a value IS in scope, via R2's class.
//   * SIGPIPE. Same family, different card, ordered behind arc D item 2.
//
// -- Do not let rustfmt sweep this file ----------------------------------

#![cfg(unix)]

use std::collections::BTreeMap;
use std::path::Path;
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

/// Run `oo`. stdout and stderr are kept apart; the exit code is taken from
/// the process, never from the end of a pipeline.
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
    nlang_interpreter::ScratchDir::new(&format!("voice-{tag}"))
}

// ---------------------------------------------------------------------
// Reading the help surface.
//
// clap writes each entry as "<spec>  <description>", two spaces or more
// between them, and an entry with no description ends after the spec. A
// missing description is therefore an empty tail, which is exactly what
// these probes count.
// ---------------------------------------------------------------------

fn help(dir: &Path, path: &[&str]) -> String {
    let mut args: Vec<&str> = path.to_vec();
    args.push("--help");
    let r = oo(dir, &args);
    r.both()
}

/// Split "  -o, --observe <OBSERVE>  what it does" into spec and description.
/// An entry with no description yields an empty second half.
fn split_entry(line: &str) -> (String, String) {
    let t = line.trim_end();
    let body = t.trim_start();
    match body.find("  ") {
        Some(i) => (body[..i].trim_end().to_string(), body[i..].trim().to_string()),
        None => (body.to_string(), String::new()),
    }
}

/// A flag spec's trailing `...` marks a repeatable value, not a description.
fn clean_desc(d: &str) -> String {
    d.trim_start_matches("...").trim().to_string()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    None,
    Commands,
    Options,
    Arguments,
}

fn sections(text: &str) -> Vec<(Section, String)> {
    let mut out = Vec::new();
    let mut sec = Section::None;
    for line in text.lines() {
        if !line.starts_with(char::is_whitespace) && line.trim_end().ends_with(':') {
            sec = match line.trim_end() {
                "Commands:" => Section::Commands,
                "Options:" => Section::Options,
                "Arguments:" => Section::Arguments,
                _ => Section::None,
            };
            continue;
        }
        if line.trim().is_empty() || sec == Section::None {
            continue;
        }
        out.push((sec, line.to_string()));
    }
    out
}

/// Subcommand names of `oo <path>`, excluding clap's own `help`.
fn subcommands(dir: &Path, path: &[&str]) -> Vec<(String, String)> {
    sections(&help(dir, path))
        .into_iter()
        .filter(|(s, _)| *s == Section::Commands)
        .map(|(_, l)| split_entry(&l))
        .filter(|(name, _)| name != "help" && !name.starts_with('-'))
        .collect()
}

/// Every command in the tree, as its argument path, depth-first.
fn all_commands(dir: &Path) -> Vec<Vec<String>> {
    fn walk(dir: &Path, path: Vec<String>, out: &mut Vec<Vec<String>>) {
        let borrowed: Vec<&str> = path.iter().map(|s| s.as_str()).collect();
        for (name, _) in subcommands(dir, &borrowed) {
            let mut child = path.clone();
            child.push(name);
            out.push(child.clone());
            walk(dir, child, out);
        }
    }
    let mut out = Vec::new();
    walk(dir, Vec::new(), &mut out);
    out
}

fn as_args(path: &[String]) -> Vec<&str> {
    path.iter().map(|s| s.as_str()).collect()
}

/// Long flags of one command: (flag, takes a value, description).
fn options(dir: &Path, path: &[&str]) -> Vec<(String, bool, String)> {
    sections(&help(dir, path))
        .into_iter()
        .filter(|(s, _)| *s == Section::Options)
        .map(|(_, l)| split_entry(&l))
        .filter(|(spec, _)| spec.starts_with('-'))
        .filter_map(|(spec, desc)| {
            let flag = spec.split_whitespace().find(|t| t.starts_with("--"))?;
            let flag = flag.trim_end_matches(',').to_string();
            if flag == "--help" || flag == "--version" {
                return None;
            }
            Some((flag, spec.contains('<'), clean_desc(&desc)))
        })
        .collect()
}

/// Positional arguments of one command: (spec, description).
fn positionals(dir: &Path, path: &[&str]) -> Vec<(String, String)> {
    sections(&help(dir, path))
        .into_iter()
        .filter(|(s, _)| *s == Section::Arguments)
        .map(|(_, l)| split_entry(&l))
        .filter(|(spec, _)| spec.starts_with('<') || spec.starts_with('['))
        .map(|(spec, desc)| (spec, clean_desc(&desc)))
        .collect()
}

/// Tokens that only Rust's Debug would put in front of an operator.
const HOST_DEBUG: &[&str] = &["Atom(", "EffectTag(", "Expr {", "Span {", "RwLock", "Thunk {"];

// =====================================================================
// G1-G5: green at the baseline, and they are this arc's red lines.
// =====================================================================

/// G1 -- the known-answer question. If this fails, nothing else here means
/// anything.
#[test]
fn g1_the_engine_can_still_answer_a_question_whose_answer_is_known() {
    let d = scratch("g1");
    let r = oo(d.path(), &["eval", "1 + 1"]);
    assert_eq!(r.code, 0, "control: `oo eval '1 + 1'` did not succeed:\n{}", r.both());
    assert_eq!(r.stdout.trim(), "2", "control: wrong answer:\n{}", r.both());
}

/// G2 -- R1 must not be satisfied by deleting the diagnostic. A conflict
/// still has to be NAMED.
#[test]
fn g2_a_conflict_is_still_named_a_conflict() {
    let d = scratch("g2");
    let r = oo(d.path(), &["eval", "1 & 2"]);
    assert_eq!(r.code, 0, "a bottom is a value; the carrier rule (D63) puts it at rc=0:\n{}", r.both());
    assert!(
        r.both().contains("#conflict"),
        "the cause disappeared -- R1 asks for different words, not for silence:\n{}",
        r.both()
    );
}

/// G3 -- R2 must not be satisfied by swallowing the failure. A missing file
/// is still a boundary carrier, so it still exits non-zero (D63).
#[test]
fn g3_a_missing_file_still_fails() {
    let d = scratch("g3");
    let mut checked = 0;
    for cmd in [["evolve", "nosuch.n"], ["run", "nosuch.n"], ["fmt", "nosuch.n"], ["test", "nosuch.n"]] {
        let r = oo(d.path(), &cmd);
        assert_ne!(
            r.code, 0,
            "`oo {}` reported success for a file that is not there:\n{}",
            cmd.join(" "),
            r.both()
        );
        checked += 1;
    }
    assert_eq!(checked, 4, "denominator: expected to ask 4 entry points, asked {checked}");
}

/// G4 -- a FLOOR under how much the CLI already says. R3/R4/R6 must not be
/// reached by deleting the descriptions that exist.
#[test]
fn g4_the_descriptions_that_exist_today_do_not_disappear() {
    let d = scratch("g4");
    let described = subcommands(d.path(), &[]).into_iter().filter(|(_, desc)| !desc.is_empty()).count();
    let total = subcommands(d.path(), &[]).len();
    assert!(total >= 10, "VOID READING: only found {total} top-level commands; the help was not parsed");
    assert!(
        described >= 9,
        "the number of described top-level commands fell to {described} of {total}; \
         it was 9 of 18 at the baseline and must never go down"
    );

    let mut paths = vec![Vec::new()];
    paths.extend(all_commands(d.path()));
    let mut flags_total = 0;
    let mut flags_described = 0;
    let mut grant_seen = false;
    for path in &paths {
        for (flag, _, desc) in options(d.path(), &as_args(path)) {
            flags_total += 1;
            if !desc.is_empty() {
                flags_described += 1;
            }
            if path.is_empty() || path == &["run".to_string()] {
                if flag == "--grant" {
                    grant_seen = true;
                    assert!(
                        !desc.is_empty(),
                        "the capability-grant flag on `oo run` lost its description -- \
                         agreement reached by deletion is not agreement"
                    );
                }
            }
        }
    }
    assert!(flags_total >= 20, "VOID READING: found only {flags_total} flags across the whole surface");
    assert!(grant_seen, "VOID READING: the capability-grant flag was not found on `oo run`");
    assert!(
        flags_described >= 18,
        "the number of described flags fell to {flags_described} of {flags_total}; \
         it was 18 at the baseline and must never go down"
    );
}

/// G5 -- the round-trip instrument itself works. Without this, any claim
/// about what parses back is a void reading.
#[test]
fn g5_the_round_trip_instrument_reaches_its_target() {
    let d = scratch("g5");
    std::fs::write(d.path().join("good.n"), "v: 1\n").expect("fixture written");
    let good = oo(d.path(), &["fmt", "good.n"]);
    assert_eq!(good.code, 0, "a form that must parse did not:\n{}", good.both());

    std::fs::write(d.path().join("bad.n"), "v: %%bogus((\n").expect("fixture written");
    let bad = oo(d.path(), &["fmt", "bad.n"]);
    assert_ne!(bad.code, 0, "a form that must NOT parse was accepted:\n{}", bad.both());
}

// =====================================================================
// R1-R6: red at the baseline. Each fails on its own assertion, never on
// a reachability check -- a probe that cannot reach its target fails as a
// VOID READING and never passes.
// =====================================================================

/// R1 -- S1. The engine must not hand the operator Rust's Debug.
#[test]
fn r1_a_conflict_is_not_explained_in_the_host_s_words() {
    let d = scratch("r1");
    let r = oo(d.path(), &["eval", "1 & 2"]);
    assert!(
        r.both().contains("#conflict"),
        "VOID READING: the expression did not produce a conflict, so nothing was measured:\n{}",
        r.both()
    );
    let found: Vec<&str> = HOST_DEBUG.iter().copied().filter(|t| r.both().contains(t)).collect();
    assert!(
        found.is_empty(),
        "the operator was answered in the host's internal representation {found:?}:\n{}",
        r.both()
    );
}

/// R2 -- S1 as a class. Every CLI entry point that opens a file must name
/// its own failure rather than forwarding the host's errno.
#[test]
fn r2_no_entry_point_forwards_a_bare_errno() {
    let d = scratch("r2");
    let entries: [&[&str]; 4] = [&["evolve", "nosuch.n"], &["run", "nosuch.n"], &["fmt", "nosuch.n"], &["test", "nosuch.n"]];
    let mut reached = 0;
    let mut leaking: Vec<String> = Vec::new();
    for args in entries {
        let r = oo(d.path(), args);
        if r.code == 0 {
            continue; // did not fail: this cell measured nothing
        }
        reached += 1;
        if r.both().contains("os error") {
            leaking.push(format!("oo {} -> {}", args.join(" "), r.both().trim().to_string()));
        }
    }
    assert_eq!(
        reached,
        entries.len(),
        "VOID READING: only {reached} of {} entry points actually failed",
        entries.len()
    );
    assert!(
        leaking.is_empty(),
        "{} of {} entry points handed the operator a host errno:\n{}",
        leaking.len(),
        entries.len(),
        leaking.join("\n")
    );
}

/// R3 -- S2. Every command says what it does.
#[test]
fn r3_every_command_says_what_it_does() {
    let d = scratch("r3");
    let paths = all_commands(d.path());
    assert!(paths.len() >= 20, "VOID READING: found only {} commands in the tree", paths.len());

    let mut silent: Vec<String> = Vec::new();
    for path in &paths {
        let parent: Vec<&str> = path[..path.len() - 1].iter().map(|s| s.as_str()).collect();
        let leaf = path.last().expect("a command has a name");
        if let Some((_, desc)) = subcommands(d.path(), &parent).into_iter().find(|(n, _)| n == leaf) {
            if desc.is_empty() {
                silent.push(format!("oo {}", path.join(" ")));
            }
        }
    }
    assert!(
        silent.is_empty(),
        "{} of {} commands say nothing about themselves:\n{}",
        silent.len(),
        paths.len(),
        silent.join("\n")
    );
}

/// R4 -- S2. Every flag says what it does.
#[test]
fn r4_every_flag_says_what_it_does() {
    let d = scratch("r4");
    let mut paths = vec![Vec::new()];
    paths.extend(all_commands(d.path()));

    let mut total = 0;
    let mut silent: Vec<String> = Vec::new();
    for path in &paths {
        for (flag, _, desc) in options(d.path(), &as_args(path)) {
            total += 1;
            if desc.is_empty() {
                silent.push(format!("oo {} {flag}", path.join(" ")));
            }
        }
    }
    assert!(total >= 20, "VOID READING: found only {total} flags across the whole surface");
    assert!(
        silent.is_empty(),
        "{silent_n} of {total} flags say nothing about themselves:\n{list}",
        silent_n = silent.len(),
        list = silent.join("\n")
    );
}

/// R5 -- S2. Every positional argument says what it is.
#[test]
fn r5_every_argument_says_what_it_is() {
    let d = scratch("r5");
    let mut paths = vec![Vec::new()];
    paths.extend(all_commands(d.path()));

    let mut total = 0;
    let mut silent: Vec<String> = Vec::new();
    for path in &paths {
        for (spec, desc) in positionals(d.path(), &as_args(path)) {
            total += 1;
            if desc.is_empty() {
                silent.push(format!("oo {} {spec}", path.join(" ")));
            }
        }
    }
    assert!(total >= 5, "VOID READING: found only {total} positional arguments across the whole surface");
    assert!(
        silent.is_empty(),
        "{silent_n} of {total} positional arguments say nothing about themselves:\n{list}",
        silent_n = silent.len(),
        list = silent.join("\n")
    );
}

/// R6 -- S3. A flag that appears on more than one command must take the same
/// kind of argument everywhere, and must not be explained in one place and
/// left silent in another.
///
/// This is deliberately WEAKER than REAL_01 1.4's clause, which says one
/// concept gets one description. Two occurrences of one spelling may be two
/// different concepts -- `--target` today is a refinement target, a service
/// CAID and a 160-bit node id -- and this probe cannot tell a homonym from
/// an inconsistency. So it asserts only the half that needs no such
/// judgement, and the work order asks for the other half in words.
#[test]
fn r6_a_flag_means_the_same_thing_wherever_it_appears() {
    let d = scratch("r6");
    let mut paths = vec![Vec::new()];
    paths.extend(all_commands(d.path()));

    let mut seen: BTreeMap<String, Vec<(String, bool, String)>> = BTreeMap::new();
    for path in &paths {
        for (flag, takes_value, desc) in options(d.path(), &as_args(path)) {
            seen.entry(flag).or_default().push((path.join(" "), takes_value, desc));
        }
    }
    let shared: Vec<_> = seen.iter().filter(|(_, uses)| uses.len() > 1).collect();
    assert!(
        shared.len() >= 2,
        "VOID READING: only {} flags appear on more than one command; nothing to compare",
        shared.len()
    );

    let mut disagreeing: Vec<String> = Vec::new();
    for (flag, uses) in &shared {
        let (_, first_value, _) = &uses[0];
        let described = uses.iter().filter(|(_, _, d)| !d.is_empty()).count();
        let desc_split = described > 0 && described < uses.len();
        let arity_split = uses.iter().any(|(_, v, _)| v != first_value);
        if desc_split || arity_split {
            let detail: Vec<String> = uses
                .iter()
                .map(|(cmd, v, dsc)| {
                    let where_ = if cmd.is_empty() { "oo".to_string() } else { format!("oo {cmd}") };
                    format!("    {where_}: takes_value={v} desc={:?}", dsc)
                })
                .collect();
            disagreeing.push(format!("  {flag} disagrees across {} commands:\n{}", uses.len(), detail.join("\n")));
        }
    }
    assert!(
        disagreeing.is_empty(),
        "{} of {} shared flags are documented or typed inconsistently:\n{}",
        disagreeing.len(),
        shared.len(),
        disagreeing.join("\n")
    );
}
