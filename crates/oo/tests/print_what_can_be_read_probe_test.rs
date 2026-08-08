// Print what can be read back (2026-08-09, pre-committed by work order:
// docs/print_what_can_be_read_handover.md).
//
// ── What is on the floor ─────────────────────────────────────────────────
//
// Measured on v0.12.0 (`dev 6e8beee`). `Value::to_nlang` (value.rs:2388)
// matches seven of eleven variants and ends with
//
//     _ => format!("{:?}", self)
//
// Thunk, Code and Ref fall into it. So for a four-line source
//
//     app: { k1: 1
//            msg: "hi" }
//
// `oo status` — the most-used command there is — prints the operator a
// `RwLock { data: None, poisoned: false, .. }`, source byte offsets in a
// `Span { start: 13, end: 14 }`, and `legacy_fields: {}`, a field no code
// path in the tree ever writes.
//
// ── Why this is not a cosmetics arc ──────────────────────────────────────
//
// oodp.rs:442, on the signing path:
//
//     pub fn identify_caid(engine: &Ouroboros, val: &Value) -> Result<String, String> {
//         let src = val.to_nlang(0);
//         let id = eval_nlang_value(engine, &format!("~%Discovery./identify {src}"))?;
//
// It feeds the printer's output back to the engine *as n/ source*. "What is
// printed must parse back" is therefore already a property the protocol
// depends on — not an output convention. R3 is that property, made into a
// test.
//
// ── What these probes are not ────────────────────────────────────────────
//
// Not a claim that printing round-trips *semantically*. A thunk prints its
// expr, so `1 + 2` prints as `1 + 2`; re-reading that text evaluates it in
// whatever scope it lands in, which need not be the closure it came from.
// This arc buys "the text parses", not "the text re-evaluates to the same
// value". The second one needs the closure to travel and is out of scope.
//
// Not forcing. Forcing changes what is stored and moves every CAID (W8′-b).
// P4 is the mechanical guarantee that this arc did not do that.
//
// ── The control that matters ─────────────────────────────────────────────
//
// C1 arms the detector against a string that is genuinely Debug-formatted at
// test time. Without it, a delivery that renamed the internal structs would
// turn every red below green while changing nothing an operator sees.

use std::fs;
use std::path::Path;
use std::process::Command;

use nlang_interpreter::value::{ComboVal, EffectTag, Value};
use nlang_parser::ast::{AtomKind, Expr, ExprKind, Path as NPath, PathAnchor, Span};

// ── harness ─────────────────────────────────────────────────────────────

fn fresh_dir(tag: &str) -> nlang_interpreter::ScratchDir {
    nlang_interpreter::ScratchDir::new(&format!("printer-{tag}"))
}

fn oo_raw(dir: &Path, args: &[&str]) -> (String, bool) {
    let out = Command::new(env!("CARGO_BIN_EXE_oo"))
        .args(args)
        .current_dir(dir)
        .env("OO_IDENTITY", dir.join("identity-for-tests"))
        .env("OO_NODE_HOME", dir.join("node-home-for-tests"))
        .output()
        .unwrap();
    (
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
        out.status.success(),
    )
}

fn oo(dir: &Path, args: &[&str]) -> String {
    oo_raw(dir, args).0
}

fn write_src(dir: &Path, name: &str, src: &str) {
    fs::write(dir.join(name), src).unwrap();
}

/// A workspace with `src` staged (evolved, not committed).
fn repo_staged(tag: &str, src: &str) -> nlang_interpreter::ScratchDir {
    let d = fresh_dir(tag);
    write_src(&d, "u.n", src);
    let (out, ok) = oo_raw(&d, &["evolve", "u.n"]);
    assert!(ok, "LIVENESS: evolve failed: {out}");
    d
}

const NESTED: &str = "app: {\n  k1: 1\n  msg: \"hi\"\n}\n";

/// Rust's own representation, in the shapes it actually reaches an operator in.
///
/// `EffectTag(` and `Atom(` are tuple-struct Debug; the rest are struct Debug
/// or types that have no n/ spelling at all.
const DEBRIS: [&str; 10] = [
    "RwLock",
    "SystemTime",
    "tv_sec",
    "legacy_fields",
    "pending_spreads",
    "Span {",
    "EffectTag(",
    "Thunk {",
    "ComboVal {",
    "Ref(Path",
];

fn debris_in(msg: &str) -> Vec<&'static str> {
    DEBRIS.iter().copied().filter(|d| msg.contains(d)).collect()
}

// ════════════════════════════════════════════════════════════════════════
//  CONTROL — green before and after
// ════════════════════════════════════════════════════════════════════════

/// C1 — the detector is armed.
///
/// Builds a value and Debug-formats it *here*, at test time, then asserts the
/// detector fires on it. If a delivery renamed `ComboVal` or dropped
/// `legacy_fields`, every red below would go green without an operator seeing
/// any difference. This is the probe that tells those two apart.
#[test]
fn c1_the_detector_is_armed() {
    let mut cv = ComboVal::default();
    cv.data.insert(
        "k".into(),
        Value::Atom(AtomKind::Int(1.into()), EffectTag::Pure, None),
    );
    let debug = format!("{:?}", Value::Combo(cv));
    let hits = debris_in(&debug);
    assert!(
        !hits.is_empty(),
        "the detector matched nothing in a genuinely Debug-formatted value — \
         it can no longer tell Rust's representation from n/: {debug}"
    );
}

/// C2 — the *same command as R1*, on input that does not produce thunks.
///
/// `oo status` over top-level atoms prints `a: 1` and nothing of Rust's, today.
/// So R1 failing is not "status is broken" and not "the detector is
/// trigger-happy" — it is the nesting that makes a thunk that makes the leak.
#[test]
fn c2_status_is_clean_when_there_is_no_thunk() {
    let d = repo_staged("c2", "a: 1\nb: \"two\"\nc: 3.5\n");
    let out = oo(&d, &["status"]);
    assert!(
        out.contains("a: 1") && out.contains("b: \"two\""),
        "LIVENESS: status printed no staged values, so finding no debris proves nothing: {out}"
    );
    assert!(
        debris_in(&out).is_empty(),
        "status leaked {:?} on thunk-free input: {out}",
        debris_in(&out)
    );

    let ev = oo(&d, &["eval", "[1, 2, 3]"]);
    assert!(
        ev.contains("[1, 2, 3]"),
        "LIVENESS: eval printed nothing recognisable: {ev}"
    );
    assert!(
        debris_in(&ev).is_empty(),
        "eval leaked {:?}",
        debris_in(&ev)
    );

    // And the same extraction R3 performs already round-trips *today*, on
    // thunk-free input. Without this, R3 could be red because the extraction
    // is wrong rather than because the printer is.
    write_src(&d, "printed.n", &staged_block_as_program(&out));
    let (fmt_out, ok) = oo_raw(&d, &["fmt", "printed.n"]);
    assert!(
        ok,
        "LIVENESS: R3's extraction does not round-trip even on clean input, \
         so R3 would not be measuring the printer: {fmt_out}"
    );
}

/// `oo status` prints the staged state as a braced block. A braced block is
/// **not** a valid n/ program — measured: `oo fmt` on `{ a: 1 }` says
/// "expected program" at 1:1. The program is the block's *contents*, so the
/// outer braces come off and one indent level with them.
fn staged_block_as_program(status_out: &str) -> String {
    let start = status_out
        .find('{')
        .expect("LIVENESS: no block in status output");
    let end = status_out.rfind('}').expect("LIVENESS: no block end");
    let inner = &status_out[start + 1..end];
    let body: String = inner
        .lines()
        .map(|l| l.strip_prefix("  ").unwrap_or(l))
        .collect::<Vec<_>>()
        .join("\n");
    format!("{}\n", body.trim_matches('\n'))
}

// ════════════════════════════════════════════════════════════════════════
//  RED — the defect, one claim each
// ════════════════════════════════════════════════════════════════════════

/// R1 — `oo status` shows the operator their staged values, not Rust's.
///
/// Asserts an absence *and* a presence in the same run: the sources `1` and
/// `"hi"` must be there. A delivery that made `status` print nothing would
/// satisfy the absence alone.
#[test]
#[ignore]
fn r1_status_shows_values_not_rust() {
    let d = repo_staged("r1", NESTED);
    let out = oo(&d, &["status"]);

    assert!(
        out.contains("k1: 1"),
        "status did not show the staged source `k1: 1`: {out}"
    );
    assert!(
        out.contains("msg: \"hi\""),
        "status did not show the staged source `msg: \"hi\"`: {out}"
    );
    assert!(
        debris_in(&out).is_empty(),
        "status leaked {:?}: {out}",
        debris_in(&out)
    );
}

/// R2 — and so does `oo inspect` of a stored root value.
#[test]
#[ignore]
fn r2_inspect_shows_values_not_rust() {
    let d = repo_staged("r2", NESTED);
    let out = oo(&d, &["commit", "-m", "base"]);
    assert!(out.contains("hash:"), "LIVENESS: no commit: {out}");

    let log = oo(&d, &["log"]);
    let commit_caid = log
        .split_whitespace()
        .find(|t| t.starts_with("hash:sha256:v1:"))
        .expect("LIVENESS: no commit CAID in log")
        .to_string();
    let head = oo(&d, &["inspect", &commit_caid]);
    let root_caid = head
        .split_whitespace()
        .find(|t| t.starts_with("hash:sha256:v2:"))
        .expect("LIVENESS: no root CAID in inspect")
        .to_string();

    let out = oo(&d, &["inspect", &root_caid]);
    assert!(
        out.contains("k1: 1"),
        "inspect did not show `k1: 1`: {}",
        &out[..out.len().min(600)]
    );
    assert!(
        debris_in(&out).is_empty(),
        "inspect leaked {:?}",
        debris_in(&out)
    );
}

/// R3 — what the printer prints, the reader can read.
///
/// This is the property `identify_caid` (oodp.rs:442) already depends on:
/// it prints a value and hands the text back to the engine as source.
#[test]
#[ignore]
fn r3_printed_state_parses_back() {
    let d = repo_staged("r3", NESTED);
    let out = oo(&d, &["status"]);
    let block = staged_block_as_program(&out);
    assert!(
        block.contains("app:"),
        "LIVENESS: extracted block is not the staged state: {block:?}"
    );

    write_src(&d, "printed.n", &block);
    let (fmt_out, ok) = oo_raw(&d, &["fmt", "printed.n"]);
    assert!(
        ok,
        "the engine could not read back what it printed: {fmt_out}\n--- printed ---\n{block}"
    );
}

/// R4 — a structural reference prints as a structural reference.
///
/// `<<>>` is not decoration: `Value::Ref` is produced only by `<<path>>`
/// (eval.rs:1545). Printed as a bare `_.a`, reading it back evaluates the
/// path instead of holding it — a different value.
#[test]
#[ignore]
fn r4_structural_ref_prints_as_structural_ref() {
    let d = repo_staged("r4", "a: 1\nb: <<_.a>>\n");
    let out = oo(&d, &["status"]);

    assert!(
        out.contains("a: 1"),
        "LIVENESS: status did not show the sibling `a: 1`: {out}"
    );
    assert!(
        out.contains("<<_.a>>"),
        "a structural reference did not print as `<<_.a>>`: {out}"
    );
    assert!(
        debris_in(&out).is_empty(),
        "status leaked {:?}: {out}",
        debris_in(&out)
    );
}

/// R5 — `oo log` prints a date, not a Rust clock reading.
#[test]
#[ignore]
fn r5_log_prints_a_date() {
    let d = repo_staged("r5", "a: 1\n");
    let out = oo(&d, &["commit", "-m", "base"]);
    assert!(out.contains("hash:"), "LIVENESS: no commit: {out}");

    let log = oo(&d, &["log"]);
    let date_line = log
        .lines()
        .find(|l| l.contains("Date:"))
        .expect("LIVENESS: log printed no Date line")
        .to_string();

    assert!(
        !date_line.contains("SystemTime") && !date_line.contains("tv_sec"),
        "log printed Rust's clock representation: {date_line}"
    );
    let has_year = date_line
        .as_bytes()
        .windows(4)
        .any(|w| w == b"2025" || w == b"2026" || w == b"2027");
    assert!(
        has_year,
        "log's Date line carries no four-digit year: {date_line}"
    );
}

/// R6 — unit level: quoted code prints as code, not as `Code(...)`.
///
/// The work order states plainly that `Value::Code` could not be reached from
/// the CLI within budget (`^.rules./double.%code` → `⊥ #out_of_horizon`;
/// `~%Reflection./quote` does not exist). So this one is built directly, and
/// says so rather than pretending a CLI red exists.
#[test]
#[ignore]
fn r6_quoted_code_prints_as_code() {
    let body = Expr {
        kind: ExprKind::Atom(AtomKind::Int(2.into())),
        span: Span { start: 0, end: 1 },
    };
    let printed = Value::Code(Box::new(body)).to_nlang(0);
    assert!(
        !printed.contains("Code(") && !printed.contains("Expr {"),
        "quoted code printed Rust's representation: {printed}"
    );
    assert!(
        printed.contains('2'),
        "quoted code printed nothing of its body: {printed}"
    );
}

// ════════════════════════════════════════════════════════════════════════
//  PINS — green before and after; these say what must NOT move
// ════════════════════════════════════════════════════════════════════════

/// P1 — surfaces that were already clean do not shift by a byte.
#[test]
fn p1_clean_surfaces_are_byte_identical() {
    let d = fresh_dir("p1");
    for (expr, expect) in [
        ("1+1", "2"),
        ("2 * 3", "6"),
        ("[1, 2, 3]", "[1, 2, 3]"),
        ("{ x: 1 }", "{\n  x: 1\n}"),
    ] {
        assert_eq!(
            oo(&d, &["eval", expr]).trim(),
            expect,
            "`oo eval {expr}` output moved"
        );
    }
    let d = repo_staged("p1s", "a: 1\nb: \"two\"\nc: 3.5\n");
    let out = oo(&d, &["status"]);
    assert!(
        out.contains("{\n  a: 1\n  b: \"two\"\n  c: 3.5\n}"),
        "status's rendering of thunk-free values moved: {out}"
    );
}

/// P2 — union display order does not move.
///
/// `value.rs:692`/`706` sort union branches by their canonical string. This
/// arc changes that string for unforced thunks, so the order is exactly the
/// thing at risk. Pinned, not assumed.
#[test]
fn p2_union_display_order_holds() {
    let d = fresh_dir("p2");
    let out = oo(&d, &["eval", "1 | 3 | 2"]);
    assert!(
        !out.trim().is_empty(),
        "LIVENESS: the union fixture printed nothing"
    );
    let first = out.clone();
    for _ in 0..3 {
        assert_eq!(
            oo(&d, &["eval", "1 | 3 | 2"]),
            first,
            "union display order is not stable across runs"
        );
    }
    assert!(
        debris_in(&first).is_empty(),
        "union display leaked {:?}: {first}",
        debris_in(&first)
    );
}

/// P3 — `oo fmt` is frozen (v2, since v0.2.0) and this arc must not touch it.
///
/// fmt goes through `Expr::to_nlang`, a different function from the one this
/// arc changes. The pin is what makes "different function" a fact rather than
/// a reading of the code.
///
/// The input is deliberately *un*formatted: `oo fmt` without `-w` prints to
/// stdout and leaves the file alone (measured), so a pin that compared the
/// file against already-canonical text would pass while measuring nothing.
#[test]
fn p3_fmt_output_is_byte_identical() {
    let d = fresh_dir("p3");
    write_src(&d, "f.n", "app:{k1:1,msg:\"hi\",n:[1,2,3]}\n");
    let (out, ok) = oo_raw(&d, &["fmt", "f.n"]);
    assert!(ok, "LIVENESS: fmt failed: {out}");
    assert_eq!(
        out, "app: {\n  k1: 1\n  msg: \"hi\"\n  n: [1, 2, 3]\n}\n\n",
        "fmt v2 output moved — this arc must not touch Expr::to_nlang"
    );
}

/// P4 — the root CAID does not move. This arc is not breaking.
///
/// `app: { k1: 1 }` has hashed to this exact digest on every engine from
/// v0.2.55 through v0.12.0 — measured, ten weeks, five builds. CAID goes
/// through `bn_serial`, which uses `Expr::to_nlang`, not the printer this arc
/// changes. If this pin moves, the delivery went into W8′-b.
#[test]
fn p4_root_caid_does_not_move() {
    const KNOWN: &str = "6e8eae8b3998fe947c719990c569494db9629f2c9e8246f4fe9207814aaf9aec";
    let d = repo_staged("p4", "app: {\n  k1: 1\n}\n");
    let out = oo(&d, &["commit", "-m", "base"]);
    assert!(out.contains("hash:"), "LIVENESS: no commit: {out}");

    let log = oo(&d, &["log"]);
    let commit_caid = log
        .split_whitespace()
        .find(|t| t.starts_with("hash:sha256:v1:"))
        .expect("LIVENESS: no commit CAID")
        .to_string();
    let head = oo(&d, &["inspect", &commit_caid]);
    let root = head
        .split_whitespace()
        .find(|t| t.starts_with("hash:sha256:v2:"))
        .expect("LIVENESS: no root CAID")
        .to_string();

    assert!(
        root.ends_with(KNOWN),
        "the root CAID of `app: {{ k1: 1 }}` moved.\n  expected …{KNOWN}\n  got      {root}"
    );
}

// Silence the unused-import warning when only some paths are exercised.
#[allow(dead_code)]
fn _path_spelling_exists() -> NPath {
    NPath {
        anchor: PathAnchor::Root,
        segments: vec!["a".into()],
        span: Span { start: 0, end: 1 },
    }
}
