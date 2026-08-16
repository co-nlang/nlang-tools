// A snapshot is not a reading of the ruler (2026-08-09, pre-committed by
// work order: docs/a_snapshot_is_not_a_reading_handover.md).
//
// ── What is on the floor ─────────────────────────────────────────────────
//
// REAL_03 §7.3 has specified the `#blur` CAID since before the engine had
// one. It says the hash input is the **CHS envelope**:
//
//     node_content + "#horizon:" + canonical_json([params])
//
// with five mandatory params (%fuel, %strategy, %max_branches,
// %max_unification_depth, %max_pattern_nodes) — six after O43 adds
// %max_lifting_depth — and one explicit prohibition: %timeout, because
// physical time is not deterministic. (O43: it is also the only horizon
// parameter that is not discrete.)
//
// `BlurDetail::blur_caid()` hashes: cause + fuel_remaining + strategy + salt.
//
//   * `node_content` (the partial) — absent.
//   * `%fuel` — the spec word is the BUDGET ("允許消耗的資源上限").
//     The engine hashes `fuel_remaining`, a reading of global progress.
//   * max_branches / max_unification_depth / max_pattern_nodes — absent.
//     A stored `#max_depth_exceeded` blur does not record which depth limit
//     produced it. Under O37 `~%Config` is not committed, so the blur is the
//     ONLY record of the conditions it was made under — and it does not
//     carry them.
//   * salt — see the split below.
//
// Six of six mandatory inputs wrong or missing, and a prohibited kind of
// input present.
//
// ── The split, measured ──────────────────────────────────────────────────
//
// The salt has one minting site (`storage.rs get_horizon_salt`,
// `sha256(SystemTime::now().as_nanos())`) and one call site
// (`universe.rs`, the evolve path). So:
//
//   fuel-side blurs   minted at observe → fixed sha256("default") salt
//                     `~%Config.fuel: 5 / p: <<_.>>` gives f41c3b06… on
//                     three fresh repos. REPRODUCIBLE TODAY.
//   unify-side blurs  minted at evolve → clock salt
//                     a `#max_depth_exceeded` blur gives a different CAID
//                     every single run. NOT REPRODUCIBLE.
//
// ⟹ Today, whether a blur has an identity at all depends on WHICH RESOURCE
// stopped it. C3 pins the reproducible half so the reds cannot be read as
// "blurs are hopeless"; R1/R2 are the other half.
//
// Committed, three fresh repos, one source, root CAID digest:
//
//     pure values      4c45e486…  4c45e486…  4c45e486…   same
//     ⊥ (1 / 0)        08bb39de…  08bb39de…  08bb39de…   same
//     one depth #blur  2f559b90…  563566a6…  8a069506…   THREE DIFFERENT
//
// And `fuel_remaining`-in-the-identity means an unrelated field rewrites a
// blur's identity from across the universe (R3), and two textually identical
// expressions are two different snapshots (R4). SPEC_01 §2.4.1 declared
// exactly this disease illegal for the DISPLAY form ("無關欄位隔空改寫拼法")
// and excluded %caid/salt from the display sort key for it — while keeping
// `剩餘燃料` as sort key item 5, which is the same reading. The law's own
// key violates the law's own goal.
//
// ── Why the arc cannot be split ──────────────────────────────────────────
//
// Taking `fuel_remaining` out without putting `node_content` in removes an
// ACCIDENTAL discriminator: two blurs with the same cause under the same
// config would then collide, and SPEC_08 §3.2.2 #6(a) would rule them #true.
// P1 is the pin that holds this. The reds and P1 must be satisfied together
// or the delivery has traded a wrong identity for a wrong equality.
//
// ── Reading a red here ───────────────────────────────────────────────────
//
// R3 and R4 use a DEPTH blur, so today they are red under BOTH defects (the
// clock salt and the reading). That is not sloppiness — both causes are
// inside this arc, and no fuel-side fixture can isolate the reading, because
// a fuel-exhausted blur always stops at fuel_remaining = 0 and so never
// exhibits it. Stated so nobody later "discovers" the entanglement and
// concludes a probe was miscalibrated.
//
// ── What these probes are not ────────────────────────────────────────────
//
// Not O45 (`%partial` is unobservable — identity and visibility are two
// things). Not open question 1 of meta/oo/observation_result.md (whether
// Blur should be an annotation rather than a Value variant) — that question
// gets its counterweight back only after this arc, since §5.2's argument for
// blur's independent type is that its CAID bakes in the stopping conditions,
// and today it bakes in readings instead.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

// ── harness ─────────────────────────────────────────────────────────────

fn fresh(tag: &str) -> nlang_interpreter::ScratchDir {
    let d = nlang_interpreter::ScratchDir::new(&format!("snapreading-{tag}"));
    let _ = Command::new(env!("CARGO_BIN_EXE_oo"))
        .arg("init")
        .current_dir(&*d)
        .env("OO_IDENTITY", d.join("identity-for-tests"))
        .env("OO_NODE_HOME", d.join("node-home-for-tests"))
        .output();
    d
}

fn oo_out(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_oo"))
        .args(args)
        .current_dir(dir)
        .env("OO_IDENTITY", dir.join("identity-for-tests"))
        .env("OO_NODE_HOME", dir.join("node-home-for-tests"))
        .output()
        .unwrap()
}

fn oo(dir: &Path, args: &[&str]) -> String {
    let o = oo_out(dir, args);
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

fn hash_in(s: &str) -> Option<String> {
    let i = s.find("hash:sha256:v1:")?;
    let rest = &s[i + "hash:sha256:v1:".len()..];
    let hex: String = rest.chars().take_while(|c| c.is_ascii_hexdigit()).collect();
    if hex.len() == 64 {
        Some(hex)
    } else {
        None
    }
}

/// `%caid` of the blur bound to `cp` in `src`, observed on a fresh repo.
/// Panics with the raw output when no CAID came back, so a failure says what
/// actually happened instead of "None".
fn caid(tag: &str, src: &str) -> String {
    let d = fresh(tag);
    fs::write(d.join("u.n"), src).unwrap();
    let out = oo(&d, &["run", "--observe", "_.cp", "u.n"]);
    hash_in(&out).unwrap_or_else(|| panic!("{tag}: no %caid in output:\n{out}"))
}

/// Root CAID digest of HEAD after evolve+commit of `src` on a fresh repo.
fn committed_root(tag: &str, src: &str) -> String {
    let d = fresh(tag);
    fs::write(d.join("u.n"), src).unwrap();
    let _ = oo_out(&d, &["evolve", "u.n"]);
    let _ = oo_out(&d, &["commit", "-m", "t"]);
    let log = oo(&d, &["log"]);
    let commit = log
        .split_whitespace()
        .find(|t| t.starts_with("hash:sha256:v1:"))
        .unwrap_or_else(|| panic!("{tag}: no commit in log:\n{log}"))
        .to_string();
    let insp = oo(&d, &["inspect", &commit]);
    insp.lines()
        .find(|l| l.trim_start().starts_with("root:"))
        .and_then(|l| l.rsplit(':').next())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| panic!("{tag}: no root in inspect:\n{insp}"))
}

const KNOB: &str = "~%Config.max_unification_depth: 2\n";
const DEEP_A: &str = "{{a: {{b: {{c: {{d: 1}}}}}}}} & {{a: {{b: {{c: {{d: 1}}}}}}}}";
const DEEP_B: &str = "{{zz: {{yy: {{xx: {{ww: 9}}}}}}}} & {{zz: {{yy: {{xx: {{ww: 9}}}}}}}}";
/// A fuel-side blur: minted during observe, so it carries the fixed salt.
const FUEL_BLUR: &str = "~%Config.fuel: 5\np: <<_.>>\n";

// ════════════════════════════════════════════════════════════════════════
//  CONTROL — green before and after
// ════════════════════════════════════════════════════════════════════════

/// C0 — the harness can see determinism at all.
///
/// Every red below has the form "these digests must agree". If
/// `committed_root` returned a constant, or the repos were not independent,
/// the reds would pass for free. Three fresh repos, no blur, one digest.
#[test]
fn c0_a_universe_without_a_horizon_is_reproducible() {
    let src = "x: 1\ny: {{a: 2}}\n";
    let a = committed_root("c0a", src);
    let b = committed_root("c0b", src);
    let c = committed_root("c0c", src);
    assert_eq!(a, b, "two identical pure universes disagreed");
    assert_eq!(b, c, "three identical pure universes disagreed");
}

/// C1 — ⊥ is reproducible too, so R1 is about the horizon and not about
/// "anything unusual makes a universe non-deterministic".
#[test]
fn c1_a_bottom_bearing_universe_is_reproducible() {
    let src = "w: 1 / 0\n";
    assert_eq!(
        committed_root("c1a", src),
        committed_root("c1b", src),
        "⊥ made a universe non-reproducible"
    );
}

/// C2 — a blur is still minted, and `%cause` still answers exactly one tag.
///
/// Every red below would be satisfied by an engine that stopped producing
/// blurs, or collapsed them to ⊥. This forbids that reading. The single-tag
/// assertion also holds O46 to its ruling: the horizon RECORDS become a set,
/// the OBSERVED cause stays one tag, projected by REAL_04 §4 priority.
#[test]
fn c2_the_horizon_still_produces_a_blur() {
    let d = fresh("c2");
    fs::write(d.join("u.n"), format!("{KNOB}p: {DEEP_A}\nc: p.%cause\n")).unwrap();
    let v = oo(&d, &["run", "--observe", "_.p", "u.n"]);
    assert!(v.contains("#blur"), "no blur was produced:\n{v}");
    let c = oo(&d, &["run", "--observe", "_.c", "u.n"]);
    assert!(
        c.contains("#max_depth_exceeded"),
        "%cause did not answer the horizon tag:\n{c}"
    );
    assert!(
        !c.contains('|'),
        "%cause answered more than one tag; REAL_04 §4 projects to a single \
         primary cause:\n{c}"
    );
}

/// C3 — the half that already works, and must keep working.
///
/// A FUEL-side blur is minted during observe and carries the fixed salt, so
/// its identity is already reproducible across processes (f41c3b06… on three
/// fresh repos at v0.16.0). This control does three jobs:
///   * it proves the harness can observe blur reproducibility, so R2 is not
///     passing or failing on harness noise;
///   * it forbids reading R1/R2 as "blur identity is hopeless";
///   * it is the regression guard for the half of the engine this arc must
///     not disturb.
/// The VALUE will change (the hash inputs change); the REPRODUCIBILITY must
/// not, which is why this asserts agreement and not a digest.
#[test]
fn c3_a_fuel_side_blur_is_already_reproducible() {
    let src = format!("{FUEL_BLUR}cp: p.%caid\n");
    let a = caid("c3a", &src);
    let b = caid("c3b", &src);
    let c = caid("c3c", &src);
    assert_eq!(a, b, "a fuel-side blur lost its identity between processes");
    assert_eq!(b, c, "a fuel-side blur lost its identity between processes");
}

// ════════════════════════════════════════════════════════════════════════
//  PIN — green now, and must stay green
// ════════════════════════════════════════════════════════════════════════

/// P1 — two blurs whose contents differ must not share an identity.
///
/// THIS IS THE PIN THAT FORCES `node_content` INTO THE CHS. It is green today
/// by accident: the two differ in `fuel_remaining`, and that reading is in the
/// hash. Remove the reading (R3/R4) without adding §7.3's first envelope term
/// and these two collide — after which SPEC_08 §3.2.2 #6(a) rules two
/// different observations equal. No digest is pinned because today's values
/// are clock-salted and do not reproduce; the RELATION is what is pinned.
#[test]
fn p1_two_different_snapshots_do_not_share_an_identity() {
    let a = caid("p1a", &format!("{KNOB}p: {DEEP_A}\ncp: p.%caid\n"));
    let b = caid("p1b", &format!("{KNOB}p: {DEEP_B}\ncp: p.%caid\n"));
    assert_ne!(
        a, b,
        "two blurs over different content collided — the reading was removed \
         without adding §7.3's node_content term"
    );
}

/// P2 — a horizon parameter is part of the snapshot's conditions.
///
/// Anti-degenerate: forbids satisfying the reds by hashing a constant, or by
/// dropping the parameters along with the reading.
///
/// WHY IT IS GREEN TODAY IS NOT THE REASON IT MUST STAY GREEN. Today the two
/// differ because changing a knob changes fuel consumption and the salt, both
/// of which are in the hash. After the delivery they must differ because
/// `%max_branches` is one of the six CHS parameters. The assertion is
/// unchanged; the mechanism under it is being replaced. Written down because
/// a pin that does not say why it moves gets quietly edited next time.
#[test]
fn p2_a_horizon_knob_belongs_to_the_snapshot() {
    let wide = caid(
        "p2a",
        &format!("{KNOB}~%Config.max_branches: 64\np: {DEEP_A}\ncp: p.%caid\n"),
    );
    let narrow = caid(
        "p2b",
        &format!("{KNOB}~%Config.max_branches: 7\np: {DEEP_A}\ncp: p.%caid\n"),
    );
    assert_ne!(
        wide, narrow,
        "a blur observed under different horizon parameters kept one identity"
    );
}

/// P3 — the breaking scope is bounded: a universe with no blur does not move.
///
/// This arc changes the identity encoding of `Value::Blur` (bn_serial 0xFD).
/// Q-032 later separated the standard root and intentionally moved every root;
/// the digest is the measured post-Q-032 value.
#[test]
fn p3_a_universe_without_a_blur_keeps_its_root() {
    assert_eq!(
        committed_root("p3", "x: 1\ny: {{a: 2}}\n"),
        "483a1b42236b90586131aae5d200ba3412026bab8521ab2cb7969a57ba5cb069",
        "a universe containing no blur changed its root CAID; this arc's \
         breaking scope is blur-bearing universes only"
    );
}

// ════════════════════════════════════════════════════════════════════════
//  RED — must fail now, must pass after
// ════════════════════════════════════════════════════════════════════════

/// R1 — the same source must commit to the same universe.
///
/// Baseline (v0.16.0): 2f559b90… / 563566a6… / 8a069506… — three fresh repos,
/// one source, three universes. Content addressing does not hold for any
/// universe containing a unify-side blur.
#[test]

fn r1_a_universe_with_a_horizon_is_reproducible() {
    let src = format!("{KNOB}z: {DEEP_A}\n");
    let a = committed_root("r1a", &src);
    let b = committed_root("r1b", &src);
    let c = committed_root("r1c", &src);
    assert_eq!(a, b, "same source, two repos, two universes");
    assert_eq!(b, c, "same source, three repos, three universes");
}

/// R2 — the same observation must have the same identity in every process.
///
/// The value-level twin of R1, and the sharpest statement of the defect: a
/// depth blur's `%caid` is different on every run of the same file. C3 is the
/// control that makes this readable — the fuel-side half already passes.
#[test]

fn r2_a_blur_has_the_same_identity_in_every_process() {
    let src = format!("{KNOB}p: {DEEP_A}\ncp: p.%caid\n");
    let a = caid("r2a", &src);
    let b = caid("r2b", &src);
    let c = caid("r2c", &src);
    assert_eq!(a, b, "same source, two processes, two identities");
    assert_eq!(b, c, "same source, three processes, three identities");
}

/// R3 — an unrelated field must not rewrite a blur's identity.
///
/// `p` is byte-identical in both programs; the second declares an unrelated
/// sum above it. Measured without the salt in play (fuel-side fixtures cannot
/// exhibit this — see the header), so today it is red under both defects.
/// SPEC_01 §2.4.1 already outlawed this for the display form.
#[test]

fn r3_an_unrelated_field_does_not_move_a_blur() {
    let alone = caid("r3a", &format!("{KNOB}p: {DEEP_A}\ncp: p.%caid\n"));
    let crowded = caid(
        "r3b",
        &format!("{KNOB}zzz: ~%List./range 1 300 |> ~%List./sum\np: {DEEP_A}\ncp: p.%caid\n"),
    );
    assert_eq!(
        alone, crowded,
        "declaring an unrelated field changed another value's identity"
    );
}

/// R4 — two identical expressions are the same snapshot.
///
/// `p` and `q` are the same text under the same config, in one program and
/// one process, and get different identities because the second is reached
/// with less fuel left.
#[test]

fn r4_the_same_expression_is_the_same_snapshot() {
    let d = fresh("r4");
    fs::write(
        d.join("u.n"),
        format!("{KNOB}p: {DEEP_A}\nq: {DEEP_A}\ncp: p.%caid\ncq: q.%caid\n"),
    )
    .unwrap();
    let p = hash_in(&oo(&d, &["run", "--observe", "_.cp", "u.n"])).expect("no p caid");
    let q = hash_in(&oo(&d, &["run", "--observe", "_.cq", "u.n"])).expect("no q caid");
    assert_eq!(p, q, "two identical expressions are not the same snapshot");
}

/// R5 — SPEC_08 §3.2.2 #6(a) must be reachable.
///
/// "兩側皆 #blur 且 CAID 相同 → #true". Baseline: `p == q` over two identical
/// blurs returns a `#blur` (via #6(b), because the CAIDs differ), never
/// `#true`. A clause no program can reach is not a clause.
#[test]

fn r5_identical_snapshots_compare_true() {
    let d = fresh("r5");
    fs::write(
        d.join("u.n"),
        format!("{KNOB}p: {DEEP_A}\nq: {DEEP_A}\ne: p == q\n"),
    )
    .unwrap();
    let out = oo(&d, &["run", "--observe", "_.e", "u.n"]);
    assert!(
        out.contains("#true"),
        "two identical snapshots did not compare #true; got:\n{out}"
    );
}

/// R6 — meet is commutative on blurs. (O46)
///
/// Today the merged blur inherits cause and horizon from whichever operand
/// was reached with less fuel left (`unify.rs`), so the operand order of a
/// commutative operator decides the identity.
///
/// The ruling: a `#blur` carries a canonically ordered SET of horizon records
/// — not a tuple (ordered, so this probe would stay red) and not a union
/// `a | b` (which says "one of", where the fact is "both horizons were hit").
/// REAL_03 §7.3's envelope is already `canonical_json([params])`.
#[test]
fn r6_merging_two_blurs_is_commutative() {
    // REWRITTEN 2026-08-10 AT FINAL ACCEPTANCE — the first version passed
    // without testing its own claim, and this probe is why the arc did not
    // close on the first try.
    //
    // It wrote the operands as COORDINATES (`x: A / y: B / p: x & y`). A
    // coordinate arrives at the meet as an unforced thunk, so the pair never
    // reached the `(Blur, Blur)` arm at all — it hit O47's absorption arm
    // `(Blur, _other) => blur`, which returns one operand untouched. Both
    // orders returned the same digest, the assertion passed, and nothing about
    // record union had been exercised.
    //
    // THE MISSING PIECE WAS A CONTROL. An equality assertion about an
    // operation needs a witness that the operation happened. With literals and
    // that control, the property fails:
    //
    //     A alone  d0967392…    B alone  e3d8e76a…
    //     A & B    7967db7f…    B & A    c64f900f…
    //
    // CAUSE — MY DIAGNOSIS HERE WAS WRONG, corrected after M4. I wrote that
    // the CHS fed the primary record separately from `co_horizons`, so which
    // blur was primary reached the hash. Measurement says otherwise: the
    // record set was already canonical, and the asymmetry came from SPANS.
    // `Value::Code` hashed `format!("{:?}", expr)` including source
    // positions, so the same partial written at a different byte offset had a
    // different digest — `A & B` and `B & A` place A and B at different
    // offsets. M4 hashes `expr.without_spans()` and the property holds.
    // Kept visible because the wrong diagnosis also reached ENGINE_SYNC, and
    // a diagnosis that was corrected is worth more than one that was quietly
    // replaced.
    let a_alone = caid("r6a", &format!("{KNOB}m: {DEEP_A}\ncp: m.%caid\n"));
    let b_alone = caid("r6b", &format!("{KNOB}m: {DEEP_B}\ncp: m.%caid\n"));
    let ab = caid(
        "r6ab",
        &format!("{KNOB}m: ({DEEP_A}) & ({DEEP_B})\ncp: m.%caid\n"),
    );
    let ba = caid(
        "r6ba",
        &format!("{KNOB}m: ({DEEP_B}) & ({DEEP_A})\ncp: m.%caid\n"),
    );
    assert_ne!(a_alone, b_alone, "control: the two operands must differ");
    assert_ne!(
        ab, a_alone,
        "control: the merge kept only the left operand — the (Blur, Blur) arm \
         was not reached, so the equality below would be vacuous"
    );
    assert_ne!(
        ab, b_alone,
        "control: the merge kept only the right operand — same vacuity"
    );
    assert_eq!(
        ab, ba,
        "`A & B` and `B & A` are different snapshots — the record set is not \
         canonically ordered before it reaches the CHS"
    );
}

/// R7 — absorbing a value into a blur does not rewrite the blur. (O47)
///
/// SPEC_03 §90: spreading a blur makes the target "該 #blur 原樣（cause／
/// CAID／視界參數保全）", and derives itself from the merge law:
/// `{b: 1, ...big} ≡ {b: 1} & unbox(big)`. The engine instead computes
/// `partial = unify(existing_partial, other)` — merging a value into a
/// snapshot taken behind a horizon whose field set §90 says is unknowable.
///
/// TWO-STAGE PROBE — read this before believing a green. Today it is red
/// because the salt and `fuel_remaining` moved. Removing those turns it
/// green. Then putting `node_content` into the CHS turns it red AGAIN unless
/// the merge stops rewriting `partial`. It passes only if the reading, the
/// salt, the node_content term, and O47 all land together.
#[test]

fn r7_absorption_does_not_rewrite_the_snapshot() {
    let bare = caid("r7a", &format!("{KNOB}x: {DEEP_A}\np: x\ncp: p.%caid\n"));
    let absorbed = caid(
        "r7b",
        &format!("{KNOB}x: {DEEP_A}\np: {{{{b: 1}}}} & x\ncp: p.%caid\n"),
    );
    assert_eq!(
        bare, absorbed,
        "meeting a value with a blur produced a different blur; \
         SPEC_03 §90 requires the blur 原樣"
    );
}

// ════════════════════════════════════════════════════════════════════════
//  PIN — added at final acceptance (2026-08-10, acceptor)
// ════════════════════════════════════════════════════════════════════════

/// P4 — a unify-side blur has a literal address, and this is what it is.
///
/// This pin could not exist before this arc. A `#max_depth_exceeded` blur was
/// minted under a clock salt, so its CAID was a different 64 hex digits on
/// every run and there was nothing to write down. Pinning the literal is the
/// most compact possible statement of what O42 delivered.
///
/// Measured three times on the accepted tree, byte-identical. If this moves,
/// the CHS inputs moved — which is a breaking change (entry #11 was this arc)
/// and must be a ruling, not a side effect.
#[test]
fn p4_a_unify_side_blur_has_a_literal_address() {
    const KNOWN: &str = "d0967392f92cc2e77b156ae18dc98d8d1b3d31ba5ec570901ca5beb19e2561d3";
    let got = caid("p4", &format!("{KNOB}p: {DEEP_A}\ncp: p.%caid\n"));
    assert_eq!(got, KNOWN, "the CHS inputs of a depth-exhausted #blur moved");
}

/// P5 — the breaking scope, stated correctly after M4 widened it.
///
/// P3 pins that a value-only universe does not move, and claims in its comment
/// that "this arc's breaking scope is blur-bearing universes only". M4
/// FALSIFIED that claim and P3 could not see it, because P3's fixture holds no
/// `Value::Code`.
///
/// M4 made `Value::Code` identity span-free (`expr.without_spans()`), which is
/// the same principle as the rest of the arc — where a thing sits in a file is
/// a circumstance, not content — but it reaches EVERY universe holding a
/// morphism, blur or no blur. Measured across the M4 boundary:
///
///     value-only     4c45e486…  →  4c45e486…   unmoved
///     morphism       ba89a8a9…  →  5529bc46…   MOVED
///     blur (combo)   d21875a8…  →  d21875a8…   unmoved by M4 itself
///
/// So the honest boundary is not "blur / not blur". It is "does the universe
/// contain a value whose identity encoding this arc touched" — `#blur` and
/// `Value::Code`.
#[test]
fn p5_a_morphism_bearing_universe_has_its_new_root() {
    assert_eq!(
        committed_root("p5", "inc: (x) -> x + 1\nv: 5\n"),
        "76ae74ddfbd21a5eb244f42cc10bd74d2baf1b020d7c5ead640df095a617006e",
        "a universe holding a morphism moved again — M4's span-free Code \
         identity is the last change that was allowed to move it"
    );
}
