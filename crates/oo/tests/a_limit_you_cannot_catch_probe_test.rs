// A limit you cannot catch (2026-08-11, pre-committed by work order:
// docs/a_limit_you_cannot_catch_recon.md §work-order).
//
// ── What is on the floor ─────────────────────────────────────────────────
//
// The parser has no ceiling of its own, and two different hostile inputs walk
// through the gap. Both reach the discovery node over the wire.
//
//   A — EXPONENTIAL BACKTRACKING. `n.pest` orders `tuple` (which needs a comma)
//       before `"(" ~ expr ~ ")"`, so every level of `(…)` grouping is parsed
//       twice: once as a failed tuple, once as a grouping. Cost is 2^depth.
//
//         (((…1…)))   depth 20 → 12.2s   depth 22 → 48.9s   depth 24 → >120s
//
//       A counterfactual one character away is linear: `(1,)` hits the tuple
//       arm directly and 1000 levels parse in 0.044s. `[…]`, `{…}`, `{{…}}`,
//       `<<…>>` have no competing arm and are all linear. `~%Config.timeout`
//       does NOT cover it (measured: 49s with timeout:1000ms set) — the only
//       time knob the operator has cannot reach the parse stage.
//
//   B — NO DEPTH CEILING. The parser recurses on the native stack (a 64 MiB
//       thread) with no depth guard anywhere. Deep nesting of ANY form
//       overflows it and the process ABORTS (exit 134):
//
//         `oo fmt` on {{a: … }}   debug: 131 aborts   release: 1336 aborts
//
//       No ⊥, no %cause, no line number. This violates ERROR_CODES §2.7.3's
//       first MUST — "the implementation MUST have a recursion ceiling of its
//       own, strictly below what it can survive" — which last week we applied
//       to the EVALUATOR (HARD_RECURSION_LIMIT) and never to the parser. The
//       evaluator side is healthy: with the depth knob raised, a 200-level
//       nest evaluates to a clean `⊥ #stack_overflow`. The parser has no such
//       floor.
//
// Both are remote. `oo node serve` reads one line with no byte cap and hands
// it to `parse_expr_only`; the serve loop is single-threaded and handles each
// connection inline. A 69-byte request
//
//     {%op: #advertise, %ad: ((((…22 levels…))))}
//
// blocked the node for >90s while a concurrent legitimate `find_node` client
// timed out at 40s (measured). In a debug build the deep request ABORTS the
// whole server. `oodp.rs`'s `MAX_AD_DEPTH = 8` cannot help: it walks the
// already-parsed AST, and the process is dead before the walk. The lock is on
// the inside of a door that has no lock of its own.
//
// ── Why not pest's `set_call_limit` for B ────────────────────────────────
//
// pest 2.7 exposes `set_call_limit`, but its own docs say the calls are "a
// running total" — it counts total rule invocations, never decrementing. It
// is the parser's FUEL, not its depth. A 130-level chain and a 200-field flat
// combo do a comparable number of total calls (~800) but have wildly different
// stack depth (130 vs 1). No single call limit tells the dangerous-deep input
// apart from the harmless-wide one. B needs a measure of DEPTH, which pest
// does not offer — hence a pre-parse depth scan (see the work order).
//
// A Rust stack overflow ABORTS (guard-page SIGSEGV → abort); `catch_unwind`
// does not catch it. So the ceiling cannot be enforced after the fact by
// running and recovering — it MUST be checked before the parser recurses.
// That is the whole reason this arc's fix is a gate before the parse, not a
// number bolted on after it. A limit you cannot catch is a limit you must
// check first.
//
// ── What this arc is, and is not ─────────────────────────────────────────
//
// IN:  A — remove the backtracking by left-factoring the grammar so `(…)` is
//          parsed once (grouping and tuple share the `( expr` prefix and
//          branch on the next token). Grouping becomes linear.
//      B — a pre-parse depth gate: reject input nested past a single
//          conservative constant (below the debug native cliff, ~10x the
//          deepest real file, which nests 9) as `⊥ #stack_overflow` — same
//          tag and same §2.7.3 semantics as the evaluator's ceiling, NOT
//          `#max_depth_exceeded` (policy) and NOT a `#blur`.
//      byte cap — the wire read is bounded; an oversized single line is
//          refused by size before it is buffered whole.
//
// OUT: replacing pest with a hand-written heap-recursive parser (the true
//      root-cure that would raise B's ceiling from ~130 to available memory)
//      — a project, not a bugfix, and named as deferred in the work order.
//      Also out: D2, the evaluator's missing fuel meter on the nesting path.
//
// The evaluator's ceiling is load-bearing SEMANTICS (the §2.7.3 incapacity
// boundary). The parser's is DEFENCE (a crash fence). Same tag, two identities.

use std::fs::{self, File};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

// ── harness ─────────────────────────────────────────────────────────────

fn fresh(tag: &str) -> nlang_interpreter::ScratchDir {
    let d = nlang_interpreter::ScratchDir::new(&format!("cannotcatch-{tag}"));
    let _ = Command::new(env!("CARGO_BIN_EXE_oo"))
        .arg("init")
        .current_dir(&*d)
        .env("OO_IDENTITY", d.join("identity-for-tests"))
        .env("OO_NODE_HOME", d.join("node-home-for-tests"))
        .output();
    d
}

/// Combined stdout+stderr of an `oo` invocation (for value/parse assertions).
fn oo(dir: &Path, args: &[&str]) -> String {
    let o = Command::new(env!("CARGO_BIN_EXE_oo"))
        .args(args)
        .current_dir(dir)
        .env("OO_IDENTITY", dir.join("identity-for-tests"))
        .env("OO_NODE_HOME", dir.join("node-home-for-tests"))
        .output()
        .unwrap();
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

enum Ran {
    Done { out: String, code: Option<i32> },
    OverBudget,
}

/// Run `oo` with a wall-clock deadline, killing it on overrun. stdout+stderr
/// go to files so a process we are about to kill cannot deadlock on a full
/// pipe. `code` is None if the process was terminated by a signal (e.g. the
/// SIGABRT of a native stack overflow) rather than exiting.
fn run_within(dir: &Path, args: &[&str], budget: Duration) -> Ran {
    let out_sink: PathBuf = dir.join("probe-stdout");
    let err_sink: PathBuf = dir.join("probe-stderr");
    let fo = File::create(&out_sink).unwrap();
    let fe = File::create(&err_sink).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_oo"))
        .args(args)
        .current_dir(dir)
        .env("OO_IDENTITY", dir.join("identity-for-tests"))
        .env("OO_NODE_HOME", dir.join("node-home-for-tests"))
        .stdout(Stdio::from(fo))
        .stderr(Stdio::from(fe))
        .spawn()
        .unwrap();
    let started = Instant::now();
    loop {
        match child.try_wait().unwrap() {
            Some(status) => {
                let mut out = fs::read_to_string(&out_sink).unwrap_or_default();
                out.push_str(&fs::read_to_string(&err_sink).unwrap_or_default());
                return Ran::Done {
                    out,
                    code: status.code(),
                };
            }
            None => {
                if started.elapsed() > budget {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Ran::OverBudget;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
}

/// Generous by design. The reds are separated from the baseline by orders of
/// magnitude (a timeout of hours vs a fix of milliseconds; an abort vs a clean
/// value), so a wide budget removes flakiness without blurring the boundary.
const BUDGET: Duration = Duration::from_secs(12);

fn grouping(depth: usize, leaf: &str) -> String {
    format!("{}{leaf}{}", "(".repeat(depth), ")".repeat(depth))
}

fn cocoon_chain(depth: usize, leaf: &str) -> String {
    let mut s = leaf.to_string();
    for _ in 0..depth {
        s = format!("{{{{a: {s}}}}}");
    }
    s
}

// ── wire harness ─────────────────────────────────────────────────────────

struct Server {
    child: Child,
    port: u16,
    _dir: nlang_interpreter::ScratchDir,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Start `oo node serve` on a free port and wait until it announces itself.
fn spawn_server(tag: &str) -> Server {
    let d = fresh(tag);
    let port = {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    let log: PathBuf = d.join("serve.log");
    let f = File::create(&log).unwrap();
    let child = Command::new(env!("CARGO_BIN_EXE_oo"))
        .args(["node", "serve", "--port", &port.to_string()])
        .current_dir(&*d)
        .env("OO_IDENTITY", d.join("identity-for-tests"))
        .env("OO_NODE_HOME", d.join("node-home-for-tests"))
        .stdout(Stdio::from(f))
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    // Wait for the "serving at port" banner (or give up).
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(10) {
        if fs::read_to_string(&log)
            .map(|s| s.contains("serving at port"))
            .unwrap_or(false)
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    Server {
        child,
        port,
        _dir: d,
    }
}

/// Send one line, read the whole reply under a deadline. Returns None on
/// timeout or connection failure (a felled node).
fn wire_send(port: u16, payload: &str, budget: Duration) -> Option<String> {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    let mut stream = TcpStream::connect(("127.0.0.1", port)).ok()?;
    stream.set_read_timeout(Some(budget)).ok()?;
    stream.set_write_timeout(Some(budget)).ok()?;
    stream.write_all(payload.as_bytes()).ok()?;
    stream.write_all(b"\n").ok()?;
    stream.flush().ok()?;
    let mut buf = Vec::new();
    // Read to EOF or deadline; the server closes after one reply.
    let mut chunk = [0u8; 4096];
    let started = Instant::now();
    loop {
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
            Err(_) => break,
        }
        if started.elapsed() > budget {
            break;
        }
    }
    if buf.is_empty() {
        None
    } else {
        Some(String::from_utf8_lossy(&buf).into_owned())
    }
}

// ════════════════════════════════════════════════════════════════════════
//  CONTROL — green before and after
// ════════════════════════════════════════════════════════════════════════

/// C0 — the parser still parses. THE CONTROL THE WHOLE ARC HANGS ON.
///
/// Every red below is of the form "hostile input is handled." The cheapest way
/// to handle hostile input is to stop parsing correctly. If this goes red, the
/// delivery bought its safety by breaking the language.
#[test]
fn c0_a_shallow_program_still_parses_and_runs() {
    let d = fresh("c0");
    fs::write(d.join("u.n"), "a: 1\nb: {{ c: 2, d: [3, 4] }}\ne: b.c + a\n").unwrap();
    let out = oo(&d, &["run", "u.n", "-o", "e"]);
    assert_eq!(out.trim(), "3", "a shallow program stopped evaluating:\n{out}");
}

/// C1 — a legitimately deep nest still has its value.
///
/// Forty levels of cocoon — 4x the deepest file in the entire corpus (which
/// nests 9), and comfortably under any ceiling this arc may set. Forbids
/// "fixing" B by setting the gate absurdly low: a real program that nests must
/// still parse and evaluate.
#[test]
fn c1_a_legitimately_deep_nest_still_has_its_value() {
    let d = fresh("c1");
    let leaf_path = format!("z{}", ".a".repeat(40));
    fs::write(
        d.join("u.n"),
        format!("z: {}\nleaf: {leaf_path}\n", cocoon_chain(40, "7")),
    )
    .unwrap();
    match run_within(&d, &["run", "u.n", "-o", "leaf"], BUDGET) {
        Ran::Done { out, .. } => assert_eq!(out.trim(), "7", "40-level nest value: {out}"),
        Ran::OverBudget => panic!("a 40-level nest stopped finishing"),
    }
}

/// C2 — grouping and tuple keep their three distinct values.
///
/// The load-bearing control for the left-factoring in ruling A. Reordering the
/// grammar so `(…)` is parsed once MUST preserve: `(x)` is grouping (identity),
/// `(x,)` is a 1-tuple, `(x, y)` is a 2-tuple. The counterfactual that a
/// projection would collapse is exactly these three staying apart.
#[test]
fn c2_grouping_and_tuple_keep_their_distinct_values() {
    let d = fresh("c2");
    let grp = oo(&d, &["eval", "(7)"]);
    assert_eq!(grp.trim(), "7", "grouping (7) is no longer the identity:\n{grp}");

    let one = oo(&d, &["eval", "(7,)"]);
    let one_flat: String = one.chars().filter(|c| !c.is_whitespace()).collect();
    assert!(
        one_flat.contains("0:7") && one_flat != "7",
        "(7,) is not a 1-tuple distinct from 7:\n{one}"
    );

    let two = oo(&d, &["eval", "(7, 9)"]);
    let two_flat: String = two.chars().filter(|c| !c.is_whitespace()).collect();
    assert!(
        two_flat.contains("0:7") && two_flat.contains("1:9"),
        "(7, 9) is not a 2-tuple:\n{two}"
    );
}

/// C3 — a legit wire request is served, fast, with no attacker present.
///
/// Attributes R4: if this is green and R4 red, the difference is the attack,
/// not a broken server or harness.
#[test]
fn c3_a_legit_wire_request_is_served() {
    let s = spawn_server("c3");
    let reply = wire_send(s.port, "{%op: #find_node}", Duration::from_secs(8));
    let reply = reply.expect("a legit find_node got no reply from an idle node");
    assert!(
        reply.contains("%source"),
        "a legit find_node did not get a well-formed reply:\n{reply}"
    );
}

// ════════════════════════════════════════════════════════════════════════
//  PIN — green before, must stay green
// ════════════════════════════════════════════════════════════════════════

/// P1 — grouping is the identity. `(1)` == `1`. Pins ruling A's semantics at
/// the smallest scale, where the exponential cannot interfere.
#[test]
fn p1_grouping_is_identity() {
    let d = fresh("p1");
    assert_eq!(oo(&d, &["eval", "(1)"]).trim(), "1");
    assert_eq!(oo(&d, &["eval", "((1))"]).trim(), "1");
}

/// P2 — a 1-tuple is not its element. `(1,)` != `1`. Pins that left-factoring
/// does not swallow the tuple arm into grouping.
#[test]
fn p2_a_one_tuple_is_not_its_element() {
    let d = fresh("p2");
    let out = oo(&d, &["eval", "(1,)"]);
    let flat: String = out.chars().filter(|c| !c.is_whitespace()).collect();
    assert!(flat.contains("0:1"), "(1,) lost its tuple field:\n{out}");
    assert_ne!(flat, "1", "(1,) collapsed to its element:\n{out}");
}

// ════════════════════════════════════════════════════════════════════════
//  RED — must fail now, must pass after (#[ignore] removed by the delivery)
// ════════════════════════════════════════════════════════════════════════

/// R1 (ruling A) — deep grouping finishes, with the right value.
///
/// Baseline: 24 levels of `(…)` do not finish (measured >120s; 2^depth). After
/// left-factoring: linear, milliseconds. The value assertion (== 7) rides
/// along so mere completion cannot satisfy it, AND so a delivery that "fixes"
/// this by rejecting deep parens (a low depth gate) FAILS — 24 is far below
/// any ceiling this arc sets, so it must genuinely parse to 7.
#[test]
#[ignore = "red until the grammar is left-factored (ruling A)"]
fn r1_deep_grouping_finishes() {
    let d = fresh("r1");
    let src = grouping(24, "7");
    match run_within(&d, &["eval", &src], BUDGET) {
        Ran::OverBudget => panic!("24 levels of grouping did not finish within {BUDGET:?}"),
        Ran::Done { out, code } => {
            assert_eq!(code, Some(0), "deep grouping did not exit cleanly:\n{out}");
            assert_eq!(out.trim(), "7", "deep grouping finished with a wrong value:\n{out}");
        }
    }
}

/// R2 (ruling B) — a nest past the native cliff is a clean ⊥, not a crash.
///
/// 2000 levels overflow the parser stack in BOTH profiles (debug 131, release
/// 1336 — measured), so the fixture is robustly red regardless of how the test
/// is built. Baseline: exit 134, "has overflowed its stack", no value. After
/// the pre-parse depth gate: exit 0 and `⊥ #stack_overflow`.
///
/// The tag is asserted precisely: §2.7.3 forbids reporting an incapacity as
/// `#max_depth_exceeded` (a policy the operator could "raise") and forbids
/// minting a `#blur` (which claims an addressable snapshot an aborted parse has
/// none of). Same ruling as the evaluator's ceiling, now on the parser.
#[test]
#[ignore = "red until the parser has a pre-parse depth ceiling (ruling B)"]
fn r2_a_deep_nest_is_a_clean_bottom_not_a_crash() {
    let d = fresh("r2");
    fs::write(d.join("u.n"), format!("z: {}\n", cocoon_chain(2000, "7"))).unwrap();
    match run_within(&d, &["fmt", "u.n"], BUDGET) {
        Ran::OverBudget => panic!("a 2000-level nest neither aborted nor finished — unexpected"),
        Ran::Done { out, code } => {
            assert_eq!(
                code,
                Some(0),
                "the parser aborted on deep input (native stack overflow) \
                 instead of reporting a bottom:\n{out}"
            );
            assert!(
                out.contains("#stack_overflow"),
                "deep input did not report #stack_overflow:\n{out}"
            );
            assert!(
                !out.contains("#max_depth_exceeded"),
                "an incapacity was reported under the operator's policy name:\n{out}"
            );
            assert!(
                !out.contains("#blur"),
                "an aborted parse minted an addressable snapshot:\n{out}"
            );
        }
    }
}

/// R3 (byte cap) — an oversized single request is refused by size.
///
/// The wire read has no byte cap: `read_line` buffers a line of any length.
/// A request one byte over the cap must be refused as `#request_too_large`
/// (a new reason, ruled in the work order), not read whole and then processed.
/// Baseline (measured): the whole 128 KiB line is read and processed as an
/// ordinary find_node, answering `#missing_field` — the cap does not exist.
/// The control is the shape: a WELL-FORMED find_node under the cap is still
/// served (C3), so R3's refusal is by size, not by content.
#[test]
#[ignore = "red until the wire read is byte-capped (byte cap)"]
fn r3_an_oversized_request_is_refused_by_size() {
    let s = spawn_server("r3");
    // 128 KiB of a valid-looking prefix — over the 64 KiB cap, well under any
    // depth limit (it is flat).
    let big = format!("{{%op: #find_node, %pad: \"{}\"}}", "a".repeat(128 * 1024));
    let reply = wire_send(s.port, &big, Duration::from_secs(8))
        .expect("an oversized request got no reply at all");
    assert!(
        reply.contains("#request_too_large"),
        "an oversized request was not refused by size:\n{}",
        &reply[..reply.len().min(120)]
    );
}

/// R4 (the wire DoS, end to end) — a hostile request does not fell the node.
///
/// This pins the PROPERTY, not a number: WHILE a hostile deep request is in
/// flight, a legitimate client is still served. The hostile `%ad` hides 2000
/// levels of grouping inside a well-formed advertise envelope — a payload that
/// parses (a computing load), not a malformed shape. Baseline: the deep parse
/// aborts the single-threaded server (debug) or blocks it for >90s (release),
/// and the legit client below gets nothing. After the pre-parse gate: the
/// hostile request is refused before the parser recurses, the node stays up,
/// and the legit client is answered within the budget.
#[test]
#[ignore = "red until deep wire input is refused before the parser recurses (rulings A+B)"]
fn r4_a_hostile_request_does_not_fell_the_node() {
    let s = spawn_server("r4");
    let port = s.port;
    // Fire the attacker in the background; it may hang or be refused — we do
    // not wait on its verdict, only on whether the node survives it.
    let attacker = std::thread::spawn(move || {
        let hostile = format!("{{%op: #advertise, %ad: {}}}", grouping(2000, "1"));
        let _ = wire_send(port, &hostile, Duration::from_secs(20));
    });
    // Give the attacker a moment to land and start the node parsing.
    std::thread::sleep(Duration::from_millis(500));
    // A legitimate client must still be served promptly.
    let reply = wire_send(port, "{%op: #find_node}", Duration::from_secs(8));
    let _ = attacker.join();
    let reply = reply.expect("a legit client was starved (or the node was felled) by one request");
    assert!(
        reply.contains("%source"),
        "the node did not serve a legit client during a hostile request:\n{reply}"
    );
}
