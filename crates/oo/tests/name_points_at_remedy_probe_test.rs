// The name points at the remedy (2026-08-09, pre-committed by work order:
// docs/the_name_points_at_the_remedy_handover.md).
//
// ── What is on the floor ─────────────────────────────────────────────────
//
// Measured on v0.13.0 (`dev 72c5fa8`). Two tags name something other than
// what happened, and in both cases the remedy the operator is handed does
// not apply:
//
//   1. disc.rs:472 — the ONLY mint site of `#semantic_eclipse` — fires when
//      `disc_routing_hops >= MAX_ROUTING_HOPS` (16). Its own message reads
//      "Routing budget exceeded after 16 hops". A peer that is merely far
//      away is reported as a suspected attack. Meanwhile the registry's
//      real detection tag, `#semantic_isolation`, has zero occurrences in
//      the engine.
//
//   2. lib.rs:193 — the depth gate returns `ResourceExhausted::FuelExhausted`.
//      Measured end to end with `~%Config.max_unification_depth: 2`:
//
//        out: { a: #blur { %cause: #fuel_exhausted,
//                          %caid: "hash:sha256:v1:6ebb46d7…" } }
//
//      Not one unit of fuel was spent. The same source with depth 64
//      converges completely. The operator is told to add fuel; the knob
//      that would help is a different knob.
//
// ── This class was already recognised once in this repo ──────────────────
//
// From the acceptor comment on `BottomCause::PeerTimeout` in value.rs:
//
//   ERROR_CODES gives `#timeout` the remedy 「請優化性能、減少嵌套…」, which
//   is not merely unhelpful for a silent peer, it points the reader at
//   their own code.
//
// Same argument, one arc earlier, one instance. These are two more.
//
// ── What these probes are not ────────────────────────────────────────────
//
// Not an implementation of `#semantic_isolation`. Real eclipse detection
// needs the meet of two trust paths (APP_05 §7.3) and does not exist.
//
// Not a claim that `#semantic_eclipse` disappears from the tree. ERROR_CODES
// §2.7.1 lets an engine keep *reading* an abolished tag for stored
// universes — exactly as `#invalid_path` is kept. R4 therefore asserts that
// nothing *mints* it, not that the name is gone.
//
// ── The pin that draws the line ──────────────────────────────────────────
//
// P2. `BlurCause` bytes enter the blur CAID (bn_serial), `BottomCause` does
// not (`Value::Bottom(_) => 0xFE`). So R2 is the one assertion in this file
// that moves a CAID, and P2 says how far it may move: fuel-exhausted blurs
// keep theirs to the byte.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use indexmap::IndexMap;
use nlang_interpreter::value::{BottomCause, ComboVal, EffectTag, Value};
use nlang_interpreter::{EvalContext, Ouroboros};
use nlang_parser::ast::AtomKind;

// ── harness ─────────────────────────────────────────────────────────────

fn oo_engine() -> Ouroboros {
    Ouroboros::new_in_memory()
}

fn int_val(n: i64) -> Value {
    Value::Atom(AtomKind::Int(n.into()), EffectTag::Pure, None)
}

fn combo(fields: &[(&str, i64)]) -> Value {
    let mut m = IndexMap::new();
    for (k, v) in fields {
        m.insert(k.to_string(), int_val(*v));
    }
    Value::Combo(ComboVal::new(
        m,
        false,
        IndexMap::new(),
        EffectTag::Pure,
        vec![],
    ))
}

fn call_find(oo: &Ouroboros, ctx: &mut EvalContext, arg: Value) -> Value {
    oo.builtin_registry.get("disc.find").unwrap().clone()(arg, oo, ctx)
}

fn call_advertise(oo: &Ouroboros, ctx: &mut EvalContext, arg: Value) -> Value {
    oo.builtin_registry.get("disc.advertise").unwrap().clone()(arg, oo, ctx)
}

fn cause_of(v: &Value) -> Option<BottomCause> {
    match v {
        Value::Bottom(bd) => Some(bd.cause),
        _ => None,
    }
}

/// Run `oo run FILE -o out` in a scratch repo whose source is `src`.
fn oo_run(tag: &str, src: &str) -> String {
    let d = nlang_interpreter::ScratchDir::new(&format!("remedy-{tag}"));
    fs::write(d.join("u.n"), src).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_oo"))
        .args(["run", "u.n", "-o", "out"])
        .current_dir(&*d)
        .env("OO_IDENTITY", d.join("identity-for-tests"))
        .env("OO_NODE_HOME", d.join("node-home-for-tests"))
        .output()
        .unwrap();
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

/// Merge two four-level combos under a given unification-depth budget.
const DEEP_MERGE: &str = "out: { a: { b: { c: { e: 1 } } } } & { a: { b: { c: { e: 1 } } } }\n";

fn deep_merge_src(depth: u64, strategy: Option<&str>) -> String {
    let mut s = format!("~%Config.max_unification_depth: {depth}\n");
    if let Some(st) = strategy {
        s.push_str(&format!("~%Config.strategy: {st}\n"));
    }
    s.push_str(DEEP_MERGE);
    s
}

fn interpreter_src_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("interpreter")
        .join("src")
}

/// Every `.rs` under a directory, concatenated, comment lines dropped.
fn code_under(dir: &Path) -> String {
    let mut out = String::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(p) = stack.pop() {
        for e in fs::read_dir(&p).unwrap() {
            let e = e.unwrap();
            let path = e.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().map(|x| x == "rs").unwrap_or(false) {
                let t = fs::read_to_string(&path).unwrap();
                for l in t.lines() {
                    if !l.trim_start().starts_with("//") {
                        out.push_str(l);
                        out.push('\n');
                    }
                }
            }
        }
    }
    out
}

// ════════════════════════════════════════════════════════════════════════
//  CONTROL — green before and after
// ════════════════════════════════════════════════════════════════════════

/// C1 — real fuel exhaustion still says `#fuel_exhausted`.
///
/// This arc renames what depth exhaustion reports. If a delivery renamed the
/// *fuel* path too, every red below would still go green while a second,
/// larger lie took the first one's place.
#[test]
fn c1_fuel_exhaustion_still_says_fuel() {
    let out = oo_run("c1", "~%Config.fuel: 5\nv: <<_.>>\nout: v.%cause\n");
    assert!(
        out.contains("#fuel_exhausted"),
        "genuine fuel exhaustion no longer reports #fuel_exhausted: {out}"
    );
}

/// C2 — the same source converges when the depth budget is adequate.
///
/// Without this, R1/R2 could be red because the fixture is broken rather
/// than because the tag is wrong.
#[test]
fn c2_adequate_depth_converges() {
    let out = oo_run("c2", &deep_merge_src(64, None));
    assert!(
        out.contains("e: 1"),
        "LIVENESS: the deep merge does not converge even at depth 64: {out}"
    );
    assert!(
        !out.contains("#blur") && !out.contains("_|_"),
        "LIVENESS: depth 64 should not hit any horizon: {out}"
    );
}

// ════════════════════════════════════════════════════════════════════════
//  RED — one claim each
// ════════════════════════════════════════════════════════════════════════

/// R1 — depth exhaustion under strict says `#max_depth_exceeded`.
#[test]
#[ignore]
fn r1_strict_depth_exhaustion_names_depth() {
    let out = oo_run("r1", &deep_merge_src(2, Some("#strict")));
    assert!(
        !out.contains("#fuel_exhausted"),
        "depth exhaustion reported as fuel exhaustion — the remedy it hands \
         the operator (add fuel) is the wrong knob: {out}"
    );
    assert!(
        out.contains("#max_depth_exceeded"),
        "depth exhaustion did not report #max_depth_exceeded: {out}"
    );
}

/// R2 — and under blur. **This is the one assertion here that moves a CAID.**
#[test]
#[ignore]
fn r2_blur_depth_exhaustion_names_depth() {
    let out = oo_run("r2", &deep_merge_src(2, Some("#blur")));
    assert!(
        out.contains("#blur"),
        "LIVENESS: the blur strategy did not produce a #blur: {out}"
    );
    assert!(
        !out.contains("#fuel_exhausted"),
        "blur from depth exhaustion is caused #fuel_exhausted: {out}"
    );
    assert!(
        out.contains("#max_depth_exceeded"),
        "blur from depth exhaustion did not report #max_depth_exceeded: {out}"
    );
}

/// R3 — a spent routing budget is a spent routing budget.
#[test]
#[ignore]
fn r3_hop_budget_is_not_an_attack() {
    let oo = oo_engine();
    let mut ctx = oo.eval_context();
    let node = combo(&[("x", 42)]);
    call_advertise(&oo, &mut ctx, node.clone());
    ctx.disc_routing_hops = 16;

    let result = call_find(&oo, &mut ctx, node);
    let cause = cause_of(&result)
        .unwrap_or_else(|| panic!("LIVENESS: exhausting the hop budget produced no ⊥: {result:?}"));
    assert_ne!(
        cause.as_tag(),
        "semantic_eclipse",
        "a spent routing budget is reported as a suspected attack"
    );
    assert_eq!(
        cause.as_tag(),
        "routing_budget_exceeded",
        "a spent routing budget did not report #routing_budget_exceeded"
    );
}

/// R4 — nothing mints `#semantic_eclipse` any more.
///
/// Not "the name is gone": ERROR_CODES §2.7.1 lets an engine keep *reading*
/// an abolished tag for stored universes, as `#invalid_path` already is.
/// So this scans for *construction* sites outside the enum's own file.
#[test]
#[ignore]
fn r4_nothing_mints_the_abolished_tag() {
    let src = interpreter_src_dir();
    let code = code_under(&src);

    // CONTROL: the scan is reading a tree that contains cause construction.
    assert!(
        code.contains("cause: BottomCause::"),
        "LIVENESS: the source scan found no cause construction at all — \
         it is not reading the engine"
    );

    let mint_sites: Vec<&str> = code
        .lines()
        .filter(|l| l.contains("cause: BottomCause::SemanticEclipse"))
        .collect();
    assert!(
        mint_sites.is_empty(),
        "#semantic_eclipse is still being minted at {} site(s): {:?}",
        mint_sites.len(),
        mint_sites
    );
}

// ════════════════════════════════════════════════════════════════════════
//  PINS — green before and after
// ════════════════════════════════════════════════════════════════════════

/// P1 — a ⊥'s CAID does not depend on its cause.
///
/// `bn_serial.rs:59` writes `0xFE` and drops the cause. That is what makes
/// R1/R3 non-breaking, and it is pinned rather than read.
#[test]
fn p1_bottom_caid_ignores_cause() {
    let a = Value::Bottom(Box::new(nlang_interpreter::value::BottomDetail {
        cause: BottomCause::Conflict,
        ..Default::default()
    }));
    let b = Value::Bottom(Box::new(nlang_interpreter::value::BottomDetail {
        cause: BottomCause::MissingKey,
        ..Default::default()
    }));
    assert_eq!(
        hex::encode(a.content_hash().digest),
        hex::encode(b.content_hash().digest),
        "⊥ CAID now depends on the cause — renaming a cause would be breaking"
    );
}

/// P2 — the blur CAID of *fuel* exhaustion does not move.
///
/// R2 moves the depth-exhaustion blur's CAID on purpose. This is the border:
/// the fuel side keeps its address to the byte.
#[test]
fn p2_fuel_blur_caid_holds() {
    const KNOWN: &str = "e4dc016e7ba3dd22f2e06175991407cbd1735d3b9c269e5852b5109e456a0f6a";
    let out = oo_run("p2", "~%Config.fuel: 5\nv: <<_.>>\nout: v.%caid\n");
    assert!(
        out.contains(KNOWN),
        "the CAID of a fuel-exhausted #blur moved — this arc must not touch \
         the fuel side.\n  expected …{KNOWN}\n  got: {out}"
    );
}

/// P3 — `#timeout` and `#peer_timeout` stay separable.
///
/// The same defect class was fixed once already, for that pair. This arc
/// must not roll it back on the way past.
#[test]
fn p3_local_and_peer_timeout_stay_separable() {
    // `as_tag()` yields the bare name; the `#` forms live in two *other*
    // mappings (`BottomDetail::as_cause_combo` and `oo/src/main.rs`). That
    // three-way split is an adjacent item, not this arc's — see §9.
    assert_eq!(BottomCause::Timeout.as_tag(), "timeout");
    assert_eq!(BottomCause::PeerTimeout.as_tag(), "peer_timeout");
    assert_ne!(
        BottomCause::Timeout.as_tag(),
        BottomCause::PeerTimeout.as_tag()
    );
}
