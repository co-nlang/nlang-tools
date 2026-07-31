// #pin — the privileged overwrite (2026-07-26, pre-committed by work order:
// docs/pin_handover.md). SPEC_08 §6.2 first operation body.
//
// #pin is the ONLY §6.2 operation that touches the LATTICE itself; the other
// three (#commit/#rollback/#squash) only move the history chain. Evolution is
// monotone: once a root coordinate is bound, an incompatible rebinding is
// refused at the evolve boundary (universe.rs G2-S check). #pin is the
// privileged, audited exception — discussion/021's "quarantined dose of
// n/^op": removing a constraint is an UP-arrow move, so it needs external
// work (the capability) exactly as reversing entropy needs work.
//
// SURFACE (measured): `oo run` is by construction a one-shot pure universe
// ("no local staged load", main.rs) — it never sees committed state, so a
// root conflict cannot even arise there. The persistent-universe command is
// `oo evolve` (it calls load_universe), which is also the right axis:
// SPEC_00 §1.2 puts universe change on the EVOLUTION axis, not observation.
// So #pin rides `oo evolve`.
//
// TWO-STEP, like runPure: `--grant pin` AUTHORIZES (P1 trusted channel, the
// capability slot declared inert in the selective-discharge arc), `--pin`
// REQUESTS. Capability alone must not silently make evolution non-monotone.
//
// AUDIT PLACEMENT is derived, not chosen. SPEC_08 §6.2 語義保證: "特權操作
// 改變的是「收斂過程」，而非「幾何指紋」" — the pinned node's %id is still
// computed from its final physical structure. Therefore the audit record
// CANNOT live inside the value (that would move its CAID). It must live on
// the COMMIT. Hence the paired assertions below: the marker is visible in
// `oo log`, and absent from the staged value.
//
// MEASURED (baseline, v0.2.39): `oo evolve` accepts neither `--pin` nor
// `--grant` (clap: "unexpected argument"), and overwriting a committed
// coordinate is `Evolution Conflict`. Cross-universe CAID comparison was
// measured and REJECTED as an observable: two fresh stores given identical
// input produce different root digests (per-store salt / genesis identity),
// so every invariant here is observed WITHIN one universe.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

fn fresh_dir() -> PathBuf {
    let mut d = std::env::temp_dir();
    d.push(format!(
        "nlang-pin-{}-{}",
        std::process::id(),
        DIR_SEQ.fetch_add(1, Ordering::SeqCst)
    ));
    fs::remove_dir_all(&d).ok();
    fs::create_dir_all(&d).unwrap();
    d
}

/// Runs `oo <args…>` in `dir`, returns trimmed stdout+stderr.
fn oo(dir: &Path, args: &[&str]) -> String {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_oo"));
    for a in args {
        cmd.arg(a);
    }
    let out = cmd.current_dir(dir).output().unwrap();
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout).trim(),
        String::from_utf8_lossy(&out.stderr).trim()
    )
}

fn write(dir: &Path, name: &str, src: &str) {
    fs::write(dir.join(name), src).unwrap();
}

/// Commits `x: 0`, leaving a bound ROOT coordinate that a later incompatible
/// write must collide with.
fn universe_with_committed_x(dir: &Path) {
    write(dir, "a.n", "x: 0\n");
    oo(dir, &["evolve", "a.n"]);
    oo(dir, &["commit", "-m", "base"]);
}

fn is_conflict(s: &str) -> bool {
    s.contains("Evolution Conflict")
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — the operation
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_pin_overwrites_a_committed_binding() {
    // THE operation. With capability and request, the incompatible rebinding
    // lands instead of collapsing. Baseline: Evolution Conflict.
    let d = fresh_dir();
    universe_with_committed_x(&d);
    write(&d, "b.n", "x: 42\n");
    let got = oo(&d, &["evolve", "--grant", "pin", "--pin", "b.n"]);
    assert!(
        !is_conflict(&got),
        "pin must overwrite a committed coordinate: {got:?}"
    );
    let status = oo(&d, &["status"]);
    assert!(
        status.contains("42"),
        "the pinned value must be staged: {status:?}"
    );
}

#[test]
fn red_pin_requires_the_capability() {
    // P1: the operation is requested, the capability authorizes. Requesting
    // without the grant must be refused — and refused LOUDLY, never silently
    // downgraded to an ordinary (conflicting) evolve.
    let d = fresh_dir();
    universe_with_committed_x(&d);
    write(&d, "b.n", "x: 42\n");
    let got = oo(&d, &["evolve", "--pin", "b.n"]);
    assert!(
        got.contains("privileged_required") || got.contains("privilege"),
        "--pin without the capability must be refused as unprivileged: {got:?}"
    );
    assert!(
        !oo(&d, &["status"]).contains("42"),
        "a refused pin must not have staged anything"
    );
}

#[test]
fn red_pin_capability_is_operation_specific() {
    // Axis-1 test, mirroring the selective-discharge arc: a DIFFERENT §6.2
    // capability must not authorize pinning. The refusal must be the PRIVILEGE
    // refusal — asserting merely "nothing was staged" would pass vacuously at
    // baseline, where the flag does not parse at all (the miscalibration this
    // exact check shipped with, caught in calibration).
    let d = fresh_dir();
    universe_with_committed_x(&d);
    write(&d, "b.n", "x: 42\n");
    let got = oo(
        &d,
        &["evolve", "--grant", "effect_override", "--pin", "b.n"],
    );
    assert!(
        got.contains("privileged_required") || got.contains("privilege"),
        "effect_override must not authorize #pin (expect a privilege refusal): {got:?}"
    );
    assert!(
        !oo(&d, &["status"]).contains("42"),
        "a refused pin must not have staged anything"
    );
}

#[test]
fn red_capability_alone_does_not_pin() {
    // The other half of two-step: holding the capability must NOT silently
    // make evolution non-monotone. Without --pin the conflict still stands.
    let d = fresh_dir();
    universe_with_committed_x(&d);
    write(&d, "b.n", "x: 42\n");
    let got = oo(&d, &["evolve", "--grant", "pin", "b.n"]);
    assert!(
        is_conflict(&got),
        "capability without request must stay monotone: {got:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — §6.2's two semantic guarantees (audit vs fingerprint)
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_pin_is_audited_in_the_commit() {
    // §6.1.3 / §6.2: a privileged intervention must be traceable downstream.
    // The commit carries it, so `oo log` shows it.
    let d = fresh_dir();
    universe_with_committed_x(&d);
    write(&d, "b.n", "x: 42\n");
    oo(&d, &["evolve", "--grant", "pin", "--pin", "b.n"]);
    // [PROBE AMENDMENT, acceptor, 2026-07-26] The commit now re-presents the
    // capability. That is the escalation repair, not a workflow nicety: the
    // commit is where the overwrite is APPLIED, and `.oo/pin_pending` records
    // intent, never authority (any n/ program can write that directory).
    oo(&d, &["commit", "--grant", "pin", "-m", "pinned"]);
    let log = oo(&d, &["log"]);
    assert!(
        log.to_lowercase().contains("pin"),
        "the pin commit must be marked in the history: {log:?}"
    );
}

#[test]
fn red_pin_does_not_mark_the_value() {
    // The OTHER half, and the load-bearing one: privilege changes the
    // convergence PROCESS, not the geometric fingerprint (§6.2). The pinned
    // value must carry no privilege residue — anything stored in the value
    // would move its CAID. Paired with the audit assertion above: marker in
    // the history, nothing in the value.
    let d = fresh_dir();
    universe_with_committed_x(&d);
    write(&d, "b.n", "x: 42\n");
    oo(&d, &["evolve", "--grant", "pin", "--pin", "b.n"]);
    let status = oo(&d, &["status"]);
    assert!(
        status.contains("42"),
        "precondition: the pin landed: {status:?}"
    );
    assert!(
        !status.contains("privileged") && !status.contains("%cause"),
        "the pinned value must carry no privilege residue: {status:?}"
    );
}

#[test]
fn red_pinned_value_equals_a_normally_written_one() {
    // Same guarantee from the other side, within ONE universe (cross-universe
    // CAID comparison is not available — fresh stores salt differently). Pin
    // x and ordinarily bind w to the same literal in the same evolve; the two
    // must render identically, i.e. the pin left no trace to distinguish them.
    let d = fresh_dir();
    universe_with_committed_x(&d);
    write(&d, "b.n", "x: 42\nw: 42\n");
    oo(&d, &["evolve", "--grant", "pin", "--pin", "b.n"]);
    let status = oo(&d, &["status"]);
    let x_line = status.lines().find(|l| l.trim_start().starts_with("x:"));
    let w_line = status.lines().find(|l| l.trim_start().starts_with("w:"));
    assert!(
        x_line.is_some() && w_line.is_some(),
        "both bindings must be staged: {status:?}"
    );
    assert_eq!(
        x_line.unwrap().trim().trim_start_matches("x:").trim(),
        w_line.unwrap().trim().trim_start_matches("w:").trim(),
        "a pinned binding must be indistinguishable from a normal one"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// ACTIVE PINS — monotonicity survives everywhere else
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn pin_ordinary_evolve_still_conflicts() {
    // The default universe stays monotone. This is the invariant #pin is an
    // exception to; if it ever goes green-by-default the arc has failed.
    let d = fresh_dir();
    universe_with_committed_x(&d);
    write(&d, "b.n", "x: 42\n");
    assert!(is_conflict(&oo(&d, &["evolve", "b.n"])));
}

#[test]
fn pin_fresh_coordinate_still_evolves() {
    // Monotone growth into an UNBOUND coordinate is not a conflict and must
    // stay unaffected.
    let d = fresh_dir();
    universe_with_committed_x(&d);
    write(&d, "b.n", "brand_new: 7\n");
    let got = oo(&d, &["evolve", "b.n"]);
    assert!(!is_conflict(&got), "fresh coordinate must evolve: {got:?}");
    assert!(oo(&d, &["status"]).contains("7"));
}

#[test]
fn pin_compatible_rebinding_still_evolves() {
    // Re-stating the SAME value is a lawful meet, not a conflict — must not
    // start requiring privilege.
    let d = fresh_dir();
    universe_with_committed_x(&d);
    write(&d, "b.n", "x: 0\n");
    assert!(!is_conflict(&oo(&d, &["evolve", "b.n"])));
}

#[test]
fn pin_ordinary_commit_is_unmarked() {
    // The audit marker must be specific to privileged commits: an ordinary
    // history must not acquire it.
    let d = fresh_dir();
    universe_with_committed_x(&d);
    let log = oo(&d, &["log"]);
    assert!(
        !log.to_lowercase().contains("pin"),
        "an ordinary commit must not be marked privileged: {log:?}"
    );
}

#[test]
fn pin_does_not_leak_replace_semantics_to_ordinary_fields() {
    // ACCEPTANCE REPAIR REGRESSION (2026-07-26). The delivery replace-merged
    // the WHOLE staged combo at commit whenever a pin was pending, so a pin on
    // one coordinate silently gave overwrite semantics to every ordinary write
    // sharing the commit — a privileged operation changing the meaning of
    // unprivileged ones.
    //
    // Setup: root has y: 5. An ordinary `y: @int` is a lawful WIDENING write
    // (G2-S allows it: meet(5, @int) = 5 ≠ ⊥) whose committed result must stay
    // 5 — the meet keeps the narrower value. Under the leak it became @int.
    //
    // Discriminator: afterwards write `y: 7`. If y is still 5 the meet is ⊥ and
    // evolution conflicts (correct). If y had been widened to @int, meet(@int,
    // 7) = 7 and the write is accepted — the leak.
    let d = fresh_dir();
    write(&d, "a.n", "x: 0\ny: 5\n");
    oo(&d, &["evolve", "a.n"]);
    oo(&d, &["commit", "-m", "base"]);

    write(&d, "b.n", "x: 42\n");
    oo(&d, &["evolve", "--grant", "pin", "--pin", "b.n"]);
    write(&d, "c.n", "y: @int\n");
    oo(&d, &["evolve", "c.n"]);
    oo(&d, &["commit", "--grant", "pin", "-m", "mixed"]);

    write(&d, "d.n", "y: 7\n");
    assert!(
        is_conflict(&oo(&d, &["evolve", "d.n"])),
        "an ordinary write sharing a pin's commit must still take the lattice \
         meet — privilege covers only the pinned coordinates"
    );
}

#[test]
fn pin_still_overwrites_its_own_coordinate_alongside_ordinary_ones() {
    // The other side of the repair: narrowing replace to the pinned coordinate
    // set must not stop the pin itself from landing when the same commit also
    // carries ordinary writes.
    let d = fresh_dir();
    write(&d, "a.n", "x: 0\ny: 5\n");
    oo(&d, &["evolve", "a.n"]);
    oo(&d, &["commit", "-m", "base"]);

    write(&d, "b.n", "x: 42\n");
    oo(&d, &["evolve", "--grant", "pin", "--pin", "b.n"]);
    write(&d, "c.n", "y: @int\n");
    oo(&d, &["evolve", "c.n"]);
    oo(&d, &["commit", "--grant", "pin", "-m", "mixed"]);

    // x is 42 now: an incompatible 99 must conflict, a compatible 42 must not.
    write(&d, "e.n", "x: 99\n");
    assert!(is_conflict(&oo(&d, &["evolve", "e.n"])));
    write(&d, "f.n", "x: 42\n");
    assert!(
        !is_conflict(&oo(&d, &["evolve", "f.n"])),
        "the pinned coordinate must have been overwritten to 42"
    );
}

#[test]
fn pin_intent_file_is_not_authority() {
    // ACCEPTANCE REPAIR REGRESSION — the sharpest finding of this arc.
    //
    // The delivery persisted the pin decision as `.oo/pin_pending` and let the
    // COMMIT act on it without re-checking the capability. But `.oo/` is
    // writable by any n/ program (`~%Io./write_file`), so a wholly
    // unprivileged program could forge the file and obtain #pin overwrite
    // semantics — plus a commit falsely marked as privileged. That is exactly
    // the implicit, tokenless backdoor SPEC_08 §6.1.2 forbids: the program
    // self-authorizes. Demonstrated end to end before the repair.
    //
    // The file records INTENT across two CLI processes; authority must be
    // re-presented through the trusted channel at the moment the privileged
    // effect is applied.
    //
    // UPDATED by the store-boundary arc (2026-07-26). The language-level
    // route this probe originally used is now refused outright, so the old
    // precondition ("an unprivileged program CAN write the intent file") is
    // false — which is the point. Both layers stay under test, because they
    // are independent and cover different attackers:
    //   layer 1 (this arc)   — n/ cannot reach the file at all;
    //   layer 2 (v0.2.40)    — a file that exists ANYWAY is not authority.
    // Layer 2 is what survives out-of-band tampering, which R-A explicitly
    // places outside the sandbox's scope, so it is planted here from the
    // harness rather than from n/.
    let d = fresh_dir();
    write(&d, "a.n", "x: 0\ny: 5\n");
    oo(&d, &["evolve", "a.n"]);
    oo(&d, &["commit", "-m", "base"]);

    // Layer 1: the original exploit, verbatim, now refused.
    write(
        &d,
        "exploit.n",
        "lst: [\"y\"]\nout: ~%Io./write_file \".oo/pin_pending\" (~%Json./stringify lst)\n",
    );
    let blocked = oo(&d, &["run", "exploit.n", "--observe", "out"]);
    assert!(
        blocked.contains("store_boundary"),
        "the language-level route must be refused at the boundary: {blocked:?}"
    );
    assert!(
        !d.join(".oo").join("pin_pending").exists(),
        "a refused write must not create the intent file"
    );

    // Layer 2: plant it out of band, where the boundary does not reach.
    fs::write(d.join(".oo").join("pin_pending"), "[\"y\"]").unwrap();
    assert!(
        d.join(".oo").join("pin_pending").exists(),
        "precondition: the intent file is present by some other route"
    );

    write(&d, "c.n", "y: @int\n");
    oo(&d, &["evolve", "c.n"]);
    let committed = oo(&d, &["commit", "-m", "innocent"]);
    assert!(
        committed.contains("privileged_required"),
        "a pin-pending commit without the capability must be refused: {committed:?}"
    );

    // And the forged intent must not have moved the lattice.
    write(&d, "d.n", "y: 7\n");
    assert!(
        is_conflict(&oo(&d, &["evolve", "d.n"])),
        "a forged intent file must not grant overwrite semantics"
    );
}

#[test]
fn pin_grant_still_refuses_effect_discharge() {
    // Carried from the selective-discharge arc: `pin` and `effect_override`
    // remain distinct capabilities. Activating the pin slot must not turn it
    // into a general privilege.
    let d = fresh_dir();
    write(&d, "c.n", "out: ~%Effect./runPure 42\n");
    let got = oo(&d, &["run", "c.n", "--grant", "pin", "--observe", "out"]);
    assert!(
        got.contains("privileged_required"),
        "granting pin must not authorize discharge: {got:?}"
    );
}
