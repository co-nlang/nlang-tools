// A ceiling that was not yours.
// Recon:   nlang-tools/docs/a_ceiling_that_was_not_yours_recon.md
// Order:   nlang-tools/docs/a_ceiling_that_was_not_yours_handover.md
// Rulings: D62 (three conditions, three names) and D63 (the exit code is
//          decided by the carrier). Both were ruled 2026-09-07.
//
// -- What this arc is ----------------------------------------------------
//
// `oo eval '1 + 2'` under a 300 MB address-space limit answers
//
//     _|_ (%cause: #stack_overflow)     exit 0
//
// and TAG_REGISTRY 2.7.3 tells the operator, verbatim, that this tag means
// "flatten the structure or change implementation -- turning a knob will
// not help". The input is `1 + 2`. Flattening it does nothing. The one
// thing that does help is turning a knob: the process resource limit.
//
// The engine has one error type for three unrelated conditions:
//
//   (1) the shape is deeper than the fence      -> #stack_overflow, true
//   (2) the host refused a thread (EAGAIN)      -> the operator must raise
//                                                  a limit
//   (3) the parser thread panicked (a bug)      -> the operator can do
//                                                  nothing but report it
//
// parser/src/lib.rs:1182-1184 maps (2) and (3) onto (1):
//
//     let parser = builder.spawn_scoped(s, f).map_err(|_| ParserNestingLimitExceeded)?;
//     parser.join().map_err(|_| ParserNestingLimitExceeded)
//
// `join()` returns Err if and only if the thread panicked, and
// `spawn_scoped` returns Err only when the host refuses the thread. The
// real depth limits are two gates OUTSIDE that thread (textual nesting 256
// pre-parse, AST height 4096 post-parse) and both answer correctly today.
// There is no legitimate `#stack_overflow` on either of those two arms.
//
// -- The carrier rule ----------------------------------------------------
//
// D63: TAG_REGISTRY 0.2 already splits carriers by "the form the consumer
// sees" -- the value itself, or a diagnostic and an exit code. So:
//
//     a value carrier (bottom / blur / Top / value)  -> exit 0
//     the boundary carrier                            -> exit non-zero
//
// A bottom IS a value. A computation that produced `_|_ (%cause: #conflict)`
// succeeded: it produced something addressable that enters the working set
// and the history. Exit 0 is right and stays right. Twelve cells were
// measured; nine already obey. The three that do not are exactly the fence
// family probed here.
//
// 2.7.4 classifies the parse stage as a BOUNDARY error. 0.2 says the
// boundary carrier "does not mint a node-level `_|_`" and that its form is
// "a diagnostic message and an exit code". main.rs:126 does exactly what
// 0.2 forbids and supplies neither of the two things it requires.
//
// -- Arming ---------------------------------------------------------------
//
// Every red here depends on reaching a resource band that varies by
// machine, binary size and allocator. So each one SEARCHES for its band
// and asserts it ARRIVED before asserting anything else; a round that
// never reached the band fails as a VOID READING and never passes. This
// is the third arc in a row where a probe could go green by never
// reaching the contested state.
//
// The helpers here keep stdout and stderr APART, unlike the helper in the
// neighbouring probe files. This arc is about what the engine tells the
// operator, and a diagnostic that reached stderr is not the same event as
// a `_|_` printed to stdout. (The queue's Inbox has a live row about a
// panic text blending into asserted output for exactly this reason.)
//
// NOT PROBED, stated so no one mistakes silence for coverage:
//   * Condition (3), the parser-thread panic. There is no portable way to
//     make the parser panic from outside the process, so nothing here
//     covers it. The work order asks for it two other ways instead: a unit
//     test inside `crates/parser` that calls the thread helper with a
//     panicking closure, and a verbatim hand-measured CLI record.
//   * The WIDTH of the failure band. It was roughly 150-650 MB on the
//     acceptor's machine. No probe may pin that number.
//   * SIGPIPE on an early-closed downstream pipe. Same family, different
//     card; D63 put it behind the arc-D `init` work and it is not this arc.

#![cfg(unix)]

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
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

/// Run `oo` with no resource limit.
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

/// Run `oo` under an address-space limit, in kilobytes. The limit applies
/// only to the child; the test harness is untouched.
fn oo_limited(dir: &Path, kb: u64, args: &[&str]) -> Run {
    let quoted: Vec<String> = args.iter().map(|a| format!("'{}'", a.replace('\'', "'\\''"))).collect();
    let script = format!("ulimit -v {kb}; exec '{OO}' {}", quoted.join(" "));
    let o = Command::new("bash")
        .current_dir(dir)
        .env("OO_IDENTITY", dir.join("identity-for-tests"))
        .env("OO_NODE_HOME", dir.join("node-home-for-tests"))
        .args(["-c", &script])
        .output()
        .expect("bash runs");
    Run {
        stdout: String::from_utf8_lossy(&o.stdout).to_string(),
        stderr: String::from_utf8_lossy(&o.stderr).to_string(),
        code: o.status.code().unwrap_or(-1),
    }
}

/// The highest address-space limit at which `oo eval '1 + 2'` stops
/// answering `3`. That is the top of the failure band -- the cell where
/// the parser thread cannot be created but the process is otherwise fine.
/// `None` means the band was never reached, which is a void reading.
fn find_denial_band(dir: &Path) -> Option<u64> {
    let control = oo(dir, &["eval", "1 + 2"]);
    if control.stdout.trim() != "3" {
        return None; // the unconstrained control does not work; nothing to compare against
    }
    for kb in [900_000u64, 700_000, 650_000, 600_000, 500_000, 400_000, 300_000, 200_000] {
        let r = oo_limited(dir, kb, &["eval", "1 + 2"]);
        if r.stdout.trim() != "3" {
            return Some(kb);
        }
    }
    None
}

fn scratch(tag: &str) -> nlang_interpreter::ScratchDir {
    nlang_interpreter::ScratchDir::new(&format!("ceiling-{tag}"))
}

fn write(d: &Path, name: &str, body: &str) {
    std::fs::write(d.join(name), body).expect("fixture written");
}

fn deep(n: usize) -> String {
    format!("{}1{}", "(".repeat(n), ")".repeat(n))
}

// ---------------------------------------------------------------------
// G1-G4: what must stay true. These are green at the baseline and must
// stay green: they are the red lines of this arc.
// ---------------------------------------------------------------------

/// G1 -- the known-answer question. If this fails nothing else here means
/// anything.
#[test]
fn g1_the_engine_still_answers_a_question_it_knows() {
    let d = scratch("g1");
    let r = oo(d.path(), &["eval", "1 + 2"]);
    assert_eq!(r.stdout.trim(), "3", "known-answer failed: {}", r.both());
    assert_eq!(r.code, 0, "known-answer exited {}", r.code);
}

/// G2 -- a bottom is a value, and a computation that produced one
/// succeeded. D63 leaves this half of the carrier rule alone; this arc
/// must not drag it along.
#[test]
fn g2_a_bottom_is_a_value_and_still_exits_zero() {
    let d = scratch("g2");
    let r = oo(d.path(), &["eval", "1 & 2"]);
    assert!(
        r.stdout.contains("_|_") && r.stdout.contains("#conflict"),
        "a value-carrier bottom stopped being reported as a value: {}",
        r.both()
    );
    assert_eq!(
        r.code, 0,
        "a computation that produced a bottom must exit 0 -- it produced \
         something: {}",
        r.both()
    );
}

/// G3 -- an ordinary parse error is already a boundary error done right:
/// a diagnostic, a non-zero exit, and no node-level bottom. It is the
/// shape the fence path has to reach.
#[test]
fn g3_an_ordinary_parse_error_is_already_a_boundary() {
    let d = scratch("g3");
    oo(d.path(), &["init"]);
    write(d.path(), "syn.n", "a: (((\n");
    let r = oo(d.path(), &["evolve", "syn.n"]);
    assert_ne!(r.code, 0, "an ordinary parse error exited 0: {}", r.both());
    assert!(
        !r.stdout.contains("_|_"),
        "an ordinary parse error minted a node-level bottom: {}",
        r.both()
    );
    assert!(
        !r.both().trim().is_empty(),
        "a boundary error produced no diagnostic at all: {}",
        r.both()
    );
}

/// G4 -- RED LINE. `#stack_overflow` genuinely has a second carrier: at
/// EVALUATION time it is a real bottom sitting in the working set, and
/// that one is correct exactly as it is. This arc must not touch it.
#[test]
fn g4_the_evaluation_stage_stack_overflow_stays_a_value() {
    let d = scratch("g4");
    oo(d.path(), &["init"]);
    let chain = std::iter::repeat("1").take(1000).collect::<Vec<_>>().join(" + ");
    write(
        d.path(),
        "u.n",
        &format!(
            "~%Config.max_unification_depth: 100000\n~%Config.strategy: #strict\nbig: {chain}\n"
        ),
    );
    let e = oo(d.path(), &["evolve", "u.n"]);
    assert!(
        !e.both().contains("panicked"),
        "LIVENESS: the evaluator crashed, so there is no report to inspect: {}",
        e.both()
    );
    let s = oo(d.path(), &["status"]);
    // ARMING. Every assertion below is about the shape of a report that
    // must exist. If the evaluator's ceiling never fired, they are all
    // vacuous.
    assert!(
        s.stdout.contains("#stack_overflow"),
        "VOID READING: the evaluator's own ceiling was never reached, so \
         nothing below is being tested: {}",
        s.both()
    );
    assert!(
        s.stdout.contains("_|_"),
        "the evaluation-stage ceiling stopped being a value carrier -- it \
         is a bottom in the working set and must stay one: {}",
        s.both()
    );
    assert_eq!(
        s.code, 0,
        "reading a working set that contains a bottom is not a failure: {}",
        s.both()
    );
}

// ---------------------------------------------------------------------
// R1-R6: red at the baseline.
// ---------------------------------------------------------------------

/// R1 -- the parse-stage fence is a BOUNDARY error (TAG_REGISTRY 2.7.4),
/// and 0.2 says a boundary carrier does not mint a node-level bottom and
/// shows itself as a diagnostic plus an exit code. Today it does the
/// forbidden thing and supplies neither required thing.
#[test]
fn r1_the_parse_stage_fence_is_a_boundary_not_a_value() {
    let d = scratch("r1");
    let control = oo(d.path(), &["eval", "1 + 2"]);
    assert_eq!(
        control.stdout.trim(),
        "3",
        "VOID READING: the control did not evaluate, so a refusal below \
         proves nothing: {}",
        control.both()
    );
    let r = oo(d.path(), &["eval", &deep(400)]);
    // ARMING: the fence must actually have fired. A build that simply
    // evaluated the deep input would satisfy every absence below.
    assert_ne!(
        r.stdout.trim(),
        "1",
        "VOID READING: the deep input evaluated normally, so the fence \
         never fired: {}",
        r.both()
    );
    assert!(
        !r.stdout.contains("_|_"),
        "the parse stage minted a node-level bottom, which TAG_REGISTRY \
         0.2 forbids for the boundary carrier -- nothing entered any \
         universe: {}",
        r.both()
    );
    assert_ne!(
        r.code, 0,
        "a boundary error exited 0; 0.2 says its form is a diagnostic and \
         an exit code, and this is the only parse-stage failure at this \
         entry point that exits 0: {}",
        r.both()
    );
}

/// R2 -- D62. The host refusing a thread is not a shape that is too deep.
/// The operator's remedy differs: raise a process limit, which is exactly
/// the action 2.7.3 tells them will not help.
#[test]
fn r2_a_host_that_refused_a_thread_is_not_a_shape_too_deep() {
    let d = scratch("r2");
    let kb = match find_denial_band(d.path()) {
        Some(kb) => kb,
        None => panic!(
            "VOID READING: no address-space limit in the searched range \
             changed the answer to `1 + 2`, so the denial band was never \
             reached and nothing below was tested"
        ),
    };
    let r = oo_limited(d.path(), kb, &["eval", "1 + 2"]);
    assert!(
        !r.both().contains("#stack_overflow"),
        "at ulimit -v {kb} the engine answered `1 + 2` with \
         #stack_overflow -- a tag whose registered remedy is `flatten the \
         structure, turning a knob will not help`, for an input of depth \
         one whose only remedy IS a knob: {}",
        r.both()
    );
    assert_ne!(
        r.code, 0,
        "at ulimit -v {kb} the engine could not parse at all and exited 0: {}",
        r.both()
    );
}

/// R3 -- the same denial reaches every parsing entry point, and `evolve`
/// is the expensive one: it reports success while nothing was evolved.
#[test]
fn r3_a_denied_evolve_must_not_report_success() {
    let d = scratch("r3");
    oo(d.path(), &["init"]);
    write(d.path(), "k.n", "a: 1 + 2\n");
    let kb = match find_denial_band(d.path()) {
        Some(kb) => kb,
        None => panic!("VOID READING: the denial band was never reached"),
    };
    let e = oo_limited(d.path(), kb, &["evolve", "k.n"]);
    // ARMING: the evolve must actually have been denied. If it succeeded,
    // the assertion below is about a different world.
    let s = oo(d.path(), &["status"]);
    assert!(
        !s.stdout.contains("a: 1 + 2"),
        "VOID READING: the constrained evolve went through, so there is no \
         denied evolve to inspect: {}",
        s.both()
    );
    assert_ne!(
        e.code, 0,
        "the evolve was denied and nothing entered the working set, and it \
         exited 0; an operator script reads that as success: {}",
        e.both()
    );
}

/// R4 -- a linter that could not read a file must not summarise it as
/// clean. Same family as REAL_02 5.1.1 "unreadable must not mean absent".
#[test]
fn r4_a_linter_that_could_not_read_must_not_report_clean() {
    let d = scratch("r4");
    write(d.path(), "warn.n", "secret_cat: #cat\n~%Repl./commit { message: \"m\" }\n");
    // ARMING, part one: the fixture must actually produce a diagnostic
    // unconstrained, or "0 diagnostics" under pressure proves nothing.
    let control = oo(d.path(), &["lint", "warn.n"]);
    assert!(
        control.stdout.contains("diagnostics: 1")
            || control.stdout.contains("diagnostics: 2")
            || control.stdout.contains("WARN")
            || control.stdout.contains("ERROR"),
        "VOID READING: the fixture lints clean even unconstrained, so a \
         clean report under pressure would prove nothing: {}",
        control.both()
    );
    let kb = match find_denial_band(d.path()) {
        Some(kb) => kb,
        None => panic!("VOID READING: the denial band was never reached"),
    };
    let r = oo_limited(d.path(), kb, &["lint", "warn.n"]);
    // ARMING, part two: the file must actually have been skipped.
    assert!(
        !r.stdout.contains("WARN") && !r.stdout.contains("ERROR"),
        "VOID READING: the constrained lint read the file after all: {}",
        r.both()
    );
    assert!(
        !(r.code == 0 && r.stdout.contains("diagnostics: 0")),
        "the linter could not read the file and still closed with \
         `diagnostics: 0` and exit 0 -- a CI gate goes green on a file it \
         never read: {}",
        r.both()
    );
}

/// R5 -- the wire. REAL_02 3.2.3 forbids silence at the entrance so that
/// the sender can tell "I was refused" from "that box is broken". A
/// receiver that names its own incapacity as a property of the request
/// defeats the reason that clause exists.
#[test]
fn r5_a_node_that_cannot_serve_must_not_blame_the_request() {
    let d = scratch("r5");
    oo(d.path(), &["init"]);
    let req = "{ %op: #fetch, %hash: \"hash:sha256:v1:0000000000000000000000000000000000000000000000000000000000000000\" }";

    fn ask(dir: &Path, kb: Option<u64>, req: &str) -> Option<String> {
        for port in 19_000u16..19_040 {
            let mut child = match kb {
                None => Command::new(OO)
                    .current_dir(dir)
                    .env("OO_IDENTITY", dir.join("identity-for-tests"))
                    .env("OO_NODE_HOME", dir.join("node-home-for-tests"))
                    .args(["node", "serve", "-p", &port.to_string()])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                    .expect("node spawns"),
                Some(kb) => Command::new("bash")
                    .current_dir(dir)
                    .env("OO_IDENTITY", dir.join("identity-for-tests"))
                    .env("OO_NODE_HOME", dir.join("node-home-for-tests"))
                    .args([
                        "-c",
                        &format!("ulimit -v {kb}; exec '{OO}' node serve -p {port}"),
                    ])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                    .expect("node spawns"),
            };
            let mut reply = None;
            for _ in 0..200 {
                if let Ok(mut s) = TcpStream::connect(("127.0.0.1", port)) {
                    if s.write_all(format!("{req}\n").as_bytes()).is_ok() {
                        let mut line = String::new();
                        let _ = BufReader::new(s.try_clone().unwrap()).read_line(&mut line);
                        if !line.trim().is_empty() {
                            reply = Some(line);
                        }
                    }
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
            let _ = child.kill();
            let _ = child.wait();
            if reply.is_some() {
                return reply;
            }
        }
        None
    }

    // ARMING: an unconstrained node must answer this request correctly,
    // or a wrong answer under pressure proves nothing about pressure.
    let good = match ask(d.path(), None, req) {
        Some(r) => r,
        None => panic!(
            "VOID READING: no unconstrained node answered on any probed \
             port, so the constrained comparison below is meaningless"
        ),
    };
    assert!(
        good.contains("#not_found") || good.contains("#not_held"),
        "VOID READING: the unconstrained node did not give the expected \
         answer, so it is not a control: {good}"
    );
    let kb = match find_denial_band(d.path()) {
        Some(kb) => kb,
        None => panic!("VOID READING: the denial band was never reached"),
    };
    let bad = match ask(d.path(), Some(kb), req) {
        Some(r) => r,
        None => panic!(
            "VOID READING: the constrained node never answered, so this is \
             a liveness result and not the misattribution being tested"
        ),
    };
    assert!(
        !bad.contains("stack_overflow"),
        "a node that cannot create a parser thread answered a one-line \
         flat request with `stack_overflow`, telling the peer its request \
         was nested too deeply; REAL_02 3.2.3 forbids silence precisely so \
         the sender can tell a refusal from a broken box, and this puts a \
         broken box in the refusal bucket: {bad}"
    );
}

/// R6 -- when the host refuses even the main thread, the operator gets a
/// Rust panic with a host source path and no n/ answer at all. Whatever
/// D62 names this condition, a panic is not one of the names.
#[test]
fn r6_the_entry_point_must_not_answer_with_a_host_panic() {
    let d = scratch("r6");
    let control = oo(d.path(), &["eval", "1 + 2"]);
    assert_eq!(
        control.stdout.trim(),
        "3",
        "VOID READING: the control did not evaluate: {}",
        control.both()
    );
    let mut arrived = false;
    let mut seen = String::new();
    for kb in [100_000u64, 80_000, 60_000, 40_000] {
        let r = oo_limited(d.path(), kb, &["eval", "1 + 2"]);
        if r.stdout.trim() != "3" {
            arrived = true;
            seen = format!("ulimit -v {kb}: {}", r.both());
            if r.both().contains("panicked at") {
                panic!(
                    "the operator got a host panic with a source path and no \
                     n/ answer at all: {seen}"
                );
            }
        }
    }
    assert!(
        arrived,
        "VOID READING: no limit in the searched range stopped the engine, \
         so the entry point was never put under pressure"
    );
    let _ = seen;
}
