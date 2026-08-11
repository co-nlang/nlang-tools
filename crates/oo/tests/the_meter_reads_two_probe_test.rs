// The meter reads two (2026-08-11, pre-committed by work order:
// docs/the_meter_reads_two_recon.md).
//
// ── What is on the floor ─────────────────────────────────────────────────
//
// SPEC_08 §107 defines `%fuel` as bounding "單次收斂觀測涉及的節點展開與
// 遞迴總量" — the total node expansion and recursion of one converging
// observation. Measured minimum viable fuel:
//
//     nest=2    2        nest=30    2        nest=200   2
//     nest=8    2        nest=120   2
//
// and across kinds of work, with every fixture's value verified so none of
// them is measuring a program that did nothing:
//
//     observe `a: 1`                            2
//     `1 + 1`                        (= 2)      2
//     `(x -> x + 1) 1`               (= 2)      2
//     `5 |> inc`                     (= 6)      2
//     eight nested applications      (= 9)      2
//     `5 |> inc |> ... |> inc` (x8)  (= 13)     2
//     lift `inc` over 20 elements    (= [2..21])2
//     `{x:1} & {y:2}`                           4
//
// Eight real applications cost what one atom costs. The meter reads two.
//
// ── Mechanism ────────────────────────────────────────────────────────────
//
// `force_recursive` — the function that performs the nesting walk — calls
// `check_resources(0)` at both of its sites (lib.rs:3236, 3261). It takes part
// in depth accounting (`ctx.depth += 1`) and in the timeout check, and charges
// no fuel at all.
//
// It is not the only one. The same function opens with
//
//     if c.pending_spreads.is_empty() && value_is_fully_solid_combo(c) {
//         return val;                          // no charge
//     }
//
// and `value_is_fully_solid_combo` is itself a recursive walk of the whole
// subtree — work that grows with the structure, done before any charge, and
// belonging to no row of the billing table. Fixing the two zeroes does not fix
// that one. This is why the arc pins a PROPERTY (more work must cost more)
// and not a list of call sites: a probe that named the call sites would go
// green while the next unbilled path stayed free.
//
// ── Why this is not a performance matter ─────────────────────────────────
//
// REAL_01 §9 is a [Core Requirement] and states its own purpose: "為了確保在
// 視界邊緣產生一致的 #blur CAID，引擎必須遵循 MBU 能階計費". The bill decides
// where the horizon falls; the horizon is part of the blur's CHS; O42 made the
// blur's identity content-addressed. So two engines that bill differently mint
// DIFFERENT CAIDs for the same program at the same declared horizon. A meter
// that does not turn is not only a missing safety bound — it is an interop
// hazard on the identity fixed two versions ago.
//
// ── Rulings this arc implements ──────────────────────────────────────────
//
// 1. Bill in SEMANTIC units (REAL_01 §9.1: subspace expansion, operator
//    application, merge, lifting), not per implementation visit. This engine's
//    nesting work is ~n^1.5..n^2 (a D1 artifact of frame cloning); billing
//    visits would make the bill an artifact of one engine's data structures,
//    which is the opposite of §9's stated purpose. A depth-n nest bills ~n.
// 2. The engine's schedule must BE the spec's schedule. Measured today they
//    differ: application 2 (table: 10), merge 4 (table: 5), lifting over 20
//    elements 2 (table: 5 + E_inner).
// 3. Completeness, in two halves: no billable operation may be charged zero
//    (an operation is billable when its cost is NOT bounded by the size of the
//    already-evaluated AST), and its falsifiable form — an observation that
//    performs strictly more billable operations must not cost strictly less.
//
// ── Headroom ─────────────────────────────────────────────────────────────
//
// Real conformance vectors consume 1–7 units against a default of 10,000;
// the deepest nesting anywhere in the corpus is 9 and the longest operator run
// is 7. An honest bill cannot plausibly break existing code — which is why C0
// below, not a smaller default, is what guards against overcharging.

use std::fs;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};

// ── harness ─────────────────────────────────────────────────────────────

fn fresh(tag: &str) -> nlang_interpreter::ScratchDir {
    let d = nlang_interpreter::ScratchDir::new(&format!("meter-{tag}"));
    let _ = Command::new(env!("CARGO_BIN_EXE_oo"))
        .arg("init")
        .current_dir(&*d)
        .env("OO_IDENTITY", d.join("identity-for-tests"))
        .env("OO_NODE_HOME", d.join("node-home-for-tests"))
        .output();
    d
}

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

/// Observe `src` at the default horizon (no `~%Config.fuel` line at all).
fn observe(dir: &Path, src: &str, path: &str) -> String {
    fs::write(dir.join("u.n"), src).unwrap();
    oo(dir, &["run", "u.n", "--observe", path])
}

/// Observe `src` with an explicit fuel budget.
fn observe_with_fuel(dir: &Path, fuel: u64, src: &str, path: &str) -> String {
    fs::write(dir.join("u.n"), format!("~%Config.fuel: {fuel}\n{src}")).unwrap();
    oo(dir, &["run", "u.n", "--observe", path])
}

fn exhausted(out: &str) -> bool {
    out.contains("fuel_exhausted")
}

/// The smallest budget at which the observation completes. Saturates at
/// `CEILING`; a saturated result is still usable for the strict inequalities
/// below, and every assertion says so where it matters.
const CEILING: u64 = 100_000;

fn min_fuel(dir: &Path, src: &str, path: &str) -> u64 {
    let (mut lo, mut hi) = (0u64, CEILING);
    while hi - lo > 1 {
        let mid = lo + (hi - lo) / 2;
        if exhausted(&observe_with_fuel(dir, mid, src, path)) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    hi
}

// ── fixtures (values verified during recon, quoted in each probe) ────────

fn nest(depth: usize) -> String {
    let mut s = String::from("1");
    for _ in 0..depth {
        s = format!("{{{{a: {s}}}}}");
    }
    format!("z: {s}\n")
}

/// `inc` applied `k` times through a pipe chain. `5` with k=8 is 13.
fn pipe_chain(k: usize) -> String {
    format!("inc: x -> x + 1\nr: 5{}\n", " |> inc".repeat(k))
}

/// `inc` nested `k` times as an application. k=8 is 9.
fn nested_apply(k: usize) -> String {
    let mut s = String::from("1");
    for _ in 0..k {
        s = format!("(x -> x + 1) ({s})");
    }
    format!("r: {s}\n")
}

/// `inc` lifted over an `n`-element list.
fn lift_over(n: usize) -> String {
    let items: Vec<String> = (1..=n).map(|i| i.to_string()).collect();
    format!("inc: x -> x + 1\nr: [{}] |> inc\n", items.join(", "))
}

/// `k` merges chained.
fn merge_chain(k: usize) -> String {
    let parts: Vec<String> = (0..=k).map(|i| format!("{{f{i}: {i}}}")).collect();
    format!("r: {}\n", parts.join(" & "))
}

// ════════════════════════════════════════════════════════════════════════
//  CONTROL — green before and after
// ════════════════════════════════════════════════════════════════════════

/// C0 — ordinary programs still evaluate, at the DEFAULT budget.
///
/// THE CONTROL THE WHOLE ARC HANGS ON. Every red below is of the form "this
/// costs more than that", and the cheapest way to make a meter turn is to
/// charge so much that ordinary work stops finishing. If this goes red, the
/// delivery bought its accounting with the language.
///
/// Values are the ones measured during recon, so a delivery cannot satisfy
/// this by returning a bottom or a blur.
#[test]
fn c0_ordinary_programs_still_evaluate_at_the_default_budget() {
    let d = fresh("c0");
    assert_eq!(observe(&d, "d: 1 + 1\n", "_.d").trim(), "2", "arithmetic");
    assert_eq!(
        observe(&d, &nested_apply(8), "_.r").trim(),
        "9",
        "eight nested applications"
    );
    assert_eq!(
        observe(&d, &pipe_chain(8), "_.r").trim(),
        "13",
        "eight-stage pipe chain"
    );

    let lifted: String = observe(&d, &lift_over(20), "_.r")
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    assert!(
        lifted.starts_with("[2,3,4") && lifted.ends_with("21]"),
        "lifting over 20 elements: {lifted}"
    );

    // Corpus maxima: deepest nesting anywhere is 9, longest operator run is 7.
    let deep = observe(&d, &nest(9), "_.z");
    assert!(deep.contains("{{"), "a 9-level nest (corpus max): {deep}");
    assert_eq!(
        observe(&d, "s: 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1\n", "_.s").trim(),
        "8",
        "a 7-operator chain (corpus max)"
    );
}

/// C1 — running out of fuel still looks the way it looked.
///
/// The arc changes WHEN the horizon is reached, and must not change WHAT
/// reaching it produces: `#blur` carrying `%cause: #fuel_exhausted`, per
/// ERROR_CODES §1.2 and SPEC_08 §3.2.2 clause 3 (the cause must name the
/// resource that actually ran out).
#[test]
fn c1_exhaustion_still_looks_like_exhaustion() {
    let d = fresh("c1");
    let out = observe_with_fuel(&d, 1, &nest(8), "_.z");
    assert!(
        out.contains("#blur"),
        "exhaustion stopped producing a blur:\n{out}"
    );
    assert!(
        out.contains("#fuel_exhausted"),
        "exhaustion stopped naming fuel as the cause:\n{out}"
    );
}

/// C2 — a fuel-side blur is still reproducible (O42's guarantee).
///
/// The blur's CAID commits to the horizon parameters. This arc moves what a
/// given budget buys, which is exactly the sort of change that could
/// reintroduce a non-deterministic identity. Same program, same budget, twice.
#[test]
fn c2_a_fuel_side_blur_is_still_reproducible() {
    let a = fresh("c2a");
    let b = fresh("c2b");
    let one = observe_with_fuel(&a, 1, &nest(8), "_.z");
    let two = observe_with_fuel(&b, 1, &nest(8), "_.z");
    assert!(one.contains("%caid"), "no CAID on the blur:\n{one}");
    assert_eq!(
        one.trim(),
        two.trim(),
        "the same program at the same horizon minted two different blurs"
    );
}

// ════════════════════════════════════════════════════════════════════════
//  PIN — green before, must stay green
// ════════════════════════════════════════════════════════════════════════

/// P1 — observing an atom stays cheap.
///
/// Pins the other end from C0: the fix must not be "multiply every price
/// until the numbers look serious". Twenty is generous against a measured 2
/// and against REAL_01 §9.1's smallest row (subspace expansion = 1).
#[test]
fn p1_observing_an_atom_stays_cheap() {
    let d = fresh("p1");
    let cost = min_fuel(&d, "a: 1\n", "_.a");
    assert!(
        cost <= 20,
        "observing a single atom now costs {cost} units of fuel"
    );
}

/// P2 — the whole corpus's worst shape still runs on a fraction of the
/// default budget.
///
/// Deepest nesting in the corpus is 9. Measured today it needs 2; after an
/// honest bill it should need on the order of tens. A tenth of the default is
/// a wide ceiling that still fails loudly if the bill becomes extravagant.
#[test]
fn p2_the_corpus_worst_shape_fits_well_inside_the_default() {
    let d = fresh("p2");
    let cost = min_fuel(&d, &nest(9), "_.z");
    assert!(
        cost <= 1000,
        "the deepest shape in the entire corpus now needs {cost} of the \
         default 10000"
    );
}

// ════════════════════════════════════════════════════════════════════════
//  RED — must fail now, must pass after (#[ignore] removed by the delivery)
// ════════════════════════════════════════════════════════════════════════

/// R1 (rulings 1 & 3) — depth is billed.
///
/// Baseline: `min_fuel` is 2 at depth 2 and 2 at depth 200. SPEC_08 §107 says
/// `%fuel` bounds recursion; today it is constant in it.
///
/// The assertion is an inequality, not a formula: ruling 1 bills semantic node
/// expansions (~n for a depth-n nest), not this engine's ~n^1.5..n^2 of
/// implementation visits, and a probe demanding a particular curve would be
/// demanding more than the ruling grants.
#[test]
fn r1_depth_is_billed() {
    let d = fresh("r1");
    let shallow = min_fuel(&d, &nest(2), "_.z");
    let deep = min_fuel(&d, &nest(200), "_.z");
    assert!(
        deep > shallow,
        "a 200-level nest costs {deep} and a 2-level nest costs {shallow} — \
         the meter is constant in depth"
    );
}

/// R2 (rulings 1 & 3) — width is billed.
///
/// Baseline: lifting `inc` over one element and over two hundred both cost 2.
/// The recon verified the 20-element case really lifts (`[2,3,…,21]`), so this
/// is not measuring a pipe that did nothing.
#[test]
fn r2_width_is_billed() {
    let d = fresh("r2");
    let narrow = min_fuel(&d, &lift_over(1), "_.r");
    let wide = min_fuel(&d, &lift_over(200), "_.r");
    assert!(
        wide > narrow,
        "lifting over 200 elements costs {wide} and over 1 costs {narrow} — \
         the meter is constant in width"
    );
}

/// R3 (ruling 3) — every application is billed, and billed uniformly.
///
/// Baseline: a pipe chain of 1, 2, 4 and 8 stages all cost 2, with the values
/// (6, 7, 9, 13) confirming every stage really ran.
///
/// Two assertions, and the second is the one that catches a partial fix: the
/// marginal cost of one more stage must be the SAME each time. An engine that
/// billed only the first stage, or only some of them, would satisfy strict
/// monotonicity and fail here.
#[test]
fn r3_every_application_is_billed_uniformly() {
    let d = fresh("r3");
    let c: Vec<u64> = [1usize, 2, 3, 4]
        .iter()
        .map(|&k| min_fuel(&d, &pipe_chain(k), "_.r"))
        .collect();
    assert!(
        c[3] > c[2] && c[2] > c[1] && c[1] > c[0],
        "pipe-chain cost is not strictly increasing in stages: {c:?}"
    );
    let marginals: Vec<u64> = c.windows(2).map(|w| w[1] - w[0]).collect();
    assert!(
        marginals.windows(2).all(|w| w[0] == w[1]),
        "the price of one more pipe stage is not uniform: {marginals:?} \
         (costs {c:?})"
    );
}

/// R4 (ruling 2) — an operator application costs what the table says.
///
/// REAL_01 §9.1 prices 算子應用 at 10 MBU. Measured today, the marginal cost
/// of one more application is 0.
///
/// WHY THIS ASSERTS `>= 10` AND NOT `== 10`. No fixture adds an application
/// and nothing else: another nested `(x -> x + 1) (…)` also introduces a
/// morphism value to evaluate, and a named-morphism form (`inc (inc 1)`) adds
/// a path reference, itself priced at 1. Exact equality is not reachable by
/// construction, so the probe pins the floor and the uniformity, and the exact
/// correspondence to §9.1 is carried by the delivery's written decomposition
/// and the spec closure. If the marginal cannot reach 10, REPORT IT — the
/// probe is the acceptor's and may not be adjusted to fit.
#[test]
fn r4_an_application_costs_what_the_table_says() {
    let d = fresh("r4");
    let c: Vec<u64> = [1usize, 2, 3, 4]
        .iter()
        .map(|&k| min_fuel(&d, &nested_apply(k), "_.r"))
        .collect();
    let marginals: Vec<u64> = c.windows(2).map(|w| w[1] - w[0]).collect();
    assert!(
        marginals.iter().all(|&m| m >= 10),
        "one more operator application costs {marginals:?}; REAL_01 §9.1 \
         prices it at 10 (costs {c:?})"
    );
    assert!(
        marginals.windows(2).all(|w| w[0] == w[1]),
        "the price of an application is not uniform: {marginals:?}"
    );
}

/// R5 (ruling 3, completeness) — every family of growing work grows the bill.
///
/// This is the sweep, and it is the probe that does not need to know which
/// function forgot to charge. Four independent ways of doing more work; each
/// must cost more. A fix that billed only the nesting walk would pass R1 and
/// fail here.
///
/// The control is inside the loop and runs FIRST: the small case must actually
/// complete at the default budget, so a family failing the inequality is
/// failing about cost and not because its program is broken.
#[test]
fn r5_every_growing_family_grows_the_bill() {
    let d = fresh("r5");
    let families: [(&str, String, String, &str); 4] = [
        ("nesting", nest(2), nest(120), "_.z"),
        ("lifting", lift_over(1), lift_over(120), "_.r"),
        ("pipe stages", pipe_chain(1), pipe_chain(120), "_.r"),
        ("merges", merge_chain(1), merge_chain(120), "_.r"),
    ];
    for (name, small, large, path) in families {
        let out = observe(&d, &small, path);
        assert!(
            !exhausted(&out) && !out.trim().is_empty(),
            "control: the small `{name}` case does not complete at the default \
             budget, so the comparison below would be meaningless:\n{out}"
        );
        let (a, b) = (min_fuel(&d, &small, path), min_fuel(&d, &large, path));
        assert!(
            b > a,
            "`{name}`: 120 units of work cost {b} and 1 costs {a} — this \
             family is not billed"
        );
    }
}

// ════════════════════════════════════════════════════════════════════════
//  RED — ride-alongs (pre-launch cleanup; no ruling needed)
// ════════════════════════════════════════════════════════════════════════

/// R6 — closing a pipe early does not panic the engine.
///
/// `oo … | head` is something anyone does on their first day. Measured today:
/// exit 101 with
///
///     thread 'oo-main' panicked at library/std/src/io/stdio.rs:
///     failed printing to stdout: Broken pipe (os error 32)
///
/// It needs an output larger than the 64 KiB pipe buffer to show up at all —
/// a 300-line file exits 0 and proves nothing, which is why the fixture here
/// is deliberately large.
#[test]
fn r6_closing_a_pipe_early_does_not_panic() {
    let d = fresh("r6");
    let fields: Vec<String> = (0..20_000)
        .map(|i| format!("field_with_a_longish_name_{i}: {i}"))
        .collect();
    fs::write(d.join("huge.n"), fields.join("\n")).unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_oo"))
        .args(["fmt", "huge.n"])
        .current_dir(&*d)
        .env("OO_IDENTITY", d.join("identity-for-tests"))
        .env("OO_NODE_HOME", d.join("node-home-for-tests"))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    // Read a little, then drop the reader — this is what `head` does.
    let mut out = child.stdout.take().unwrap();
    let mut buf = [0u8; 256];
    let _ = out.read(&mut buf);
    drop(out);

    let mut err = String::new();
    let _ = child.stderr.take().unwrap().read_to_string(&mut err);
    let status = child.wait().unwrap();

    assert!(
        !err.contains("panicked"),
        "the engine panicked when its reader went away:\n{err}"
    );
    assert_ne!(
        status.code(),
        Some(101),
        "the engine exited 101 (panic) on a closed pipe:\n{err}"
    );
}

/// R7 — draft scaffolding is not in the shipped source.
///
/// `mod advert_debug` is a draft test module that shipped, and three tests
/// carry `#[ignore]` labels claiming defects that no longer exist. Measured
/// 2026-08-11: both `path_test` ignores PASS when run, and
/// `lazy_stress_test`'s "Stack Overflow on deep thunks" is not an engine
/// defect at all — it is the one interpreter test without the 64 MiB thread
/// builder the rest of the suite uses, so it runs on cargo's 2 MiB default.
/// Through the real CLI the same 50-deep thunk chain answers 149, and 200
/// answers a clean `#max_depth_exceeded`.
///
/// A false "known defect" is worse than no note: it tells the next reader that
/// a fixed thing is broken and that a broken thing is understood.
///
/// THE CONTROL RUNS FIRST, because this probe is a source scan and a scan that
/// silently matches nothing would pass by finding no violations.
#[test]
fn r7_draft_scaffolding_is_not_in_the_shipped_source() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let mut files = Vec::new();
    fn walk(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                if p.file_name().is_some_and(|n| n == "target" || n == ".git") {
                    continue;
                }
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push(p);
            }
        }
    }
    walk(&root.join("crates"), &mut files);

    // Control: the scanner reaches real source. Without this, a walker that
    // silently found nothing would satisfy every assertion below.
    assert!(
        files.len() > 50,
        "control: the source scan found only {} .rs files — it is not \
         reaching the tree, so the checks below would be vacuous",
        files.len()
    );
    // This probe quotes both offending strings in its own prose, so it would
    // otherwise report itself. Excluding it is not a loophole: the file is the
    // acceptor's and its contents are fixed by the protocol.
    let this_file = Path::new(file!())
        .file_name()
        .map(|n| n.to_owned())
        .unwrap();
    let bodies: Vec<(std::path::PathBuf, String)> = files
        .into_iter()
        .filter(|p| p.file_name() != Some(this_file.as_os_str()))
        .filter_map(|p| fs::read_to_string(&p).ok().map(|s| (p, s)))
        .collect();
    assert!(
        bodies.iter().any(|(_, s)| s.contains("check_resources")),
        "control: the scan did not find `check_resources` anywhere, so it is \
         not reading the engine source"
    );

    let debug_mod: Vec<_> = bodies
        .iter()
        .filter(|(_, s)| s.contains("mod advert_debug"))
        .map(|(p, _)| p.display().to_string())
        .collect();
    assert!(
        debug_mod.is_empty(),
        "draft debug scaffolding shipped: {debug_mod:?}"
    );

    let stale: Vec<_> = bodies
        .iter()
        .filter(|(_, s)| {
            s.contains("#[ignore = \"Known Issue") || s.contains("#[ignore = \"Known Defect")
        })
        .map(|(p, _)| p.display().to_string())
        .collect();
    assert!(
        stale.is_empty(),
        "tests still claim defects that measurement says are fixed or \
         misattributed: {stale:?}"
    );
}
