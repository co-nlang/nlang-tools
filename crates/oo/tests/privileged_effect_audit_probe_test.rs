// A privilege that leaves no trace (2026-07-27, pre-committed by work order:
// docs/privileged_effect_audit_handover.md).
//
// ── The headline, measured on v0.2.46 ────────────────────────────────────
//
//   oo evolve s.n --grant effect_override:io     ← capability presented here
//   oo commit -m "no capability at this stage"   ← and never again
//
//   committed universe:  v: 1785130396317
//   commit object:       kind = Standard
//                        meta = {author, timestamp, message}
//
// `s.n` is `v: (~%Effect./runPure (~%Time.now _))`. A nondeterministic IO
// observation has been laundered into an ordinary integer, sitting in history
// with nothing to say it was ever privileged. Its CAID is byte-identical to
// the same integer typed by hand — measured, and CORRECT: content addressing
// means the value IS the value, which is exactly why SPEC_08 §6.2 puts the
// audit on the COMMIT and forbids it in the value.
//
// Control, same source, no grant at evolve:
//
//   committed universe:  v: _|_ (%cause: #privileged_required)
//
// So the gate is real and the difference between the two universes is exactly
// one discharge.
//
// ── This is a compliance gap, not a design question ──────────────────────
// SPEC_08 §6.2 already names the audit tag for `#effect_override`:
// **`#privileged_effect`**. It already says 「由特權操作產生的 Commit 必須在
// 元資訊中標註其干預性質」. And its 授權時點 clause (2026-07-26) already says
// 「若一個特權操作跨越多個階段(如「演化期標記、提交期套用」),則每一個實際
// 施加效果的階段都必須各自出示能力」.
//
// The discharge is applied at evolve and FIXED INTO HISTORY at commit. Commit
// is a stage that applies the effect, and it presents nothing. This is the
// v0.2.40 `#pin` finding one operation across, and `#pin`'s shipped design is
// the answer: intent recorded in the assertion layer, capability re-presented
// at the moment the effect enters the record.
//
// ── The surface the new marker would be built on is forgeable ────────────
// Measured on v0.2.46 — an ordinary commit, no capability of any kind:
//
//   $ oo commit -m "pin"
//   $ oo log
//   commit hash:sha256:v1:f67e68bb…
//       pin
//       Date: …
//
// which is what a genuine `#pin` commit with no message renders, to the byte.
// v0.2.41 ruled 「無法憑檢視查驗的審計面不成其為審計面」 and repaired the
// auto-generated squash MESSAGE; the marker FORMAT was left, and main.rs:369
// says so in a comment. Adding a third marker to this surface would be
// building on sand, so the format is in scope here.
//
// ── What is NOT reopened ─────────────────────────────────────────────────
// A rollback with no subsequent commit leaves no record. That is not a gap:
// §6.2 R1 says the record is written by the next commit because 「該時點即
// 分歧真正進入鏈之時」 — nothing entered the chain, so there is nothing to
// record. Deliberate and coherent. Measured, read, left alone.
//
// ── Anti-vacuity ─────────────────────────────────────────────────────────
// Every gate here first proves the discharge actually happened, because the
// discharged value is an ordinary integer and "no marker" is indistinguishable
// from "nothing to mark" unless the pair is measured together.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

/// Forces a real discharge: `~%Time.now` is IO, `runPure` manufactures the
/// section, and without the grant the same source yields ⊥.
const DISCHARGE_SRC: &str = "v: (~%Effect./runPure (~%Time.now _))\n";
const PLAIN_SRC: &str = "v: { hello: \"world\" }\n";

/// From universe_determinism — an ordinary value's address must not move
/// because an audit marker was added to commits.
const GOLDEN_VALUE_CAID: &str = "hash:sha256:v2:_:gICS1LCf09bLAQD//5HUsJ/T1ssBAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA:681781ef857ac859326d707bdfcd04fc939b78e7c9060dd674d9a8be536f2ae4";

// ── harness ─────────────────────────────────────────────────────────────

fn fresh_dir(tag: &str) -> nlang_interpreter::ScratchDir {
    nlang_interpreter::ScratchDir::new(&format!("pe-{tag}"))
}

struct Run {
    out: String,
    ok: bool,
}

impl Run {
    fn has(&self, s: &str) -> bool {
        self.out.contains(s)
    }
}

/// Every invocation pins `OO_IDENTITY` (v0.2.46) so the suite never touches
/// the developer's real `~/.oo/`.
fn oo(dir: &Path, args: &[&str]) -> Run {
    let out = Command::new(env!("CARGO_BIN_EXE_oo"))
        .args(args)
        .current_dir(dir)
        .env("OO_IDENTITY", dir.join("identity-for-tests"))
        .output()
        .unwrap();
    Run {
        out: format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout).trim(),
            String::from_utf8_lossy(&out.stderr).trim()
        ),
        ok: out.status.success(),
    }
}

fn write(dir: &Path, name: &str, src: &str) {
    fs::write(dir.join(name), src).unwrap();
}

fn head_commit(dir: &Path) -> String {
    let c = oo(dir, &["log"])
        .out
        .lines()
        .find_map(|l| l.strip_prefix("commit ").map(|s| s.trim().to_string()))
        .unwrap_or_default();
    assert!(c.starts_with("hash:sha256:"), "no HEAD commit in {dir:?}");
    c
}

fn object_json(dir: &Path, caid: &str) -> serde_json::Value {
    let d = caid.rsplit(':').next().unwrap();
    let p = dir
        .join(".oo")
        .join("objects")
        .join("sha256")
        .join(&d[..2])
        .join(&d[2..]);
    serde_json::from_slice(&fs::read(&p).unwrap_or_else(|e| panic!("{p:?}: {e}"))).unwrap()
}

/// The universe root's rendering, so a discharge can be shown to have landed
/// in history rather than merely to have been evaluated.
fn committed_v(dir: &Path) -> String {
    let commit = object_json(dir, &head_commit(dir));
    let dg = &commit["root"]["digest"];
    let hex = if let Some(s) = dg.as_str() {
        s.to_string()
    } else if let Some(a) = dg.as_array() {
        a.iter()
            .map(|b| format!("{:02x}", b.as_u64().expect("digest byte")))
            .collect::<String>()
    } else {
        panic!("commit root has no usable `digest`: {}", commit["root"]);
    };
    assert_eq!(hex.len(), 64, "root digest is not 64 hex: {hex:?}");
    let dump = oo(dir, &["inspect", &format!("hash:sha256:v1:{hex}")]).out;
    dump.lines()
        .find(|l| l.trim_start().starts_with("v:"))
        .unwrap_or_else(|| panic!("no `v` in the committed universe:\n{dump}"))
        .trim()
        .to_string()
}

/// A repository whose staged content was produced under a real discharge.
/// Panics unless the discharge is proven to have happened.
fn repo_with_discharge(tag: &str) -> nlang_interpreter::ScratchDir {
    let d = fresh_dir(tag);
    write(&d, "s.n", DISCHARGE_SRC);

    // ANTI-VACUITY: the same source with no grant must be refused, or the
    // whole file measures nothing.
    let refused = oo(&d, &["eval", "(~%Effect./runPure (~%Time.now _))"]);
    assert!(
        refused.has("privileged_required"),
        "harness: runPure is not gated at all, so nothing here is a discharge: {}",
        refused.out
    );

    let e = oo(&d, &["evolve", "s.n", "--grant", "effect_override:io"]);
    assert!(e.ok, "harness: privileged evolve failed: {}", e.out);
    d
}

/// `oo log` output for HEAD with the lines that vary by run removed, so what
/// remains is exactly the audit surface a reader would compare.
fn head_audit_lines(dir: &Path) -> Vec<String> {
    let out = oo(dir, &["log"]).out;
    let mut lines = Vec::new();
    for l in out.lines().skip(1) {
        if l.starts_with("commit ") {
            break;
        }
        let t = l.trim();
        if t.is_empty() || t.starts_with("Date:") {
            continue;
        }
        lines.push(t.to_string());
    }
    lines
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES
// ─────────────────────────────────────────────────────────────────────────

/// R1 — the stage that fixes the discharge into history must present the
/// capability (SPEC_08 §6.2 授權時點).
///
/// PAIRED: with the capability the same commit must succeed, so a pass cannot
/// come from an engine that simply refuses every commit.
#[test]
fn red_commit_of_discharged_content_requires_the_capability() {
    let d = repo_with_discharge("r1");

    let bare = oo(&d, &["commit", "-m", "no capability at this stage"]);
    assert!(
        !bare.ok,
        "a discharge entered history with no capability presented at that stage: {}",
        bare.out
    );
    assert!(
        bare.has("privileged_required"),
        "the refusal must name the missing capability: {}",
        bare.out
    );

    // PAIR: the same commit, authorised, must go through.
    let granted = oo(
        &d,
        &[
            "commit",
            "-m",
            "authorised",
            "--grant",
            "effect_override:io",
        ],
    );
    assert!(
        granted.has("Commit successful"),
        "an authorised commit of discharged content was refused: {}",
        granted.out
    );

    // And the discharge really did land — not a ⊥ that would make the gate
    // above pass for the wrong reason.
    let v = committed_v(&d);
    assert!(
        !v.contains("_|_"),
        "the committed value is bottom, so no discharge was ever fixed: {v}"
    );
}

/// R2 — the commit object records the intervention (`#privileged_effect`,
/// SPEC_08 §6.2 表 + 透明度).
///
/// PAIRED with an ordinary commit, which must NOT be marked: an engine that
/// marks everything has not made the distinction the audit surface exists for.
#[test]
fn red_privileged_commit_is_marked_and_ordinary_commits_are_not() {
    let d = repo_with_discharge("r2");
    let granted = oo(
        &d,
        &[
            "commit",
            "-m",
            "authorised",
            "--grant",
            "effect_override:io",
        ],
    );
    assert!(granted.has("Commit successful"), "harness: {}", granted.out);
    let privileged = object_json(&d, &head_commit(&d));
    assert!(
        privileged.to_string().contains("privileged_effect"),
        "a commit that fixed a discharge into history carries no marker: {privileged}"
    );

    // DISCRIMINATOR: an ordinary commit in the same repository.
    write(&d, "p.n", "p: 1\n");
    let e = oo(&d, &["evolve", "p.n"]);
    assert!(e.ok, "harness: ordinary evolve failed: {}", e.out);
    let ok = oo(&d, &["commit", "-m", "ordinary"]);
    assert!(ok.has("Commit successful"), "harness: {}", ok.out);
    let ordinary = object_json(&d, &head_commit(&d));
    assert!(
        !ordinary.to_string().contains("privileged_effect"),
        "an ordinary commit was marked privileged: {ordinary}"
    );
}

/// R3 — the marker must reach the ordinary reader.
///
/// A verdict that exists only inside a stored object is not an audit surface;
/// v0.2.44 spent an arc on exactly this (a verdict reaching the value is not
/// clause 4 being satisfied), and v0.2.41's `oo log` was another discarded one.
#[test]
fn red_oo_log_shows_the_privileged_effect_marker() {
    let d = repo_with_discharge("r3");
    let granted = oo(
        &d,
        &[
            "commit",
            "-m",
            "authorised",
            "--grant",
            "effect_override:io",
        ],
    );
    assert!(granted.has("Commit successful"), "harness: {}", granted.out);

    let lines = head_audit_lines(&d);
    assert!(
        !lines.is_empty(),
        "harness: `oo log` printed no body for HEAD"
    );
    assert!(
        lines.iter().any(|l| l.contains("privileged_effect")),
        "`oo log` does not show the marker; audit lines were {lines:?}"
    );
}

/// R4 — an audit marker must not be forgeable by a commit message.
///
/// Measured on v0.2.46: `oo commit -m "pin"` renders
///
///     commit hash:sha256:v1:f67e68bb…
///         pin
///
/// which is what a genuine `#pin` commit with no message renders, to the byte.
/// This gate is deliberately wording-agnostic — it reads whatever the engine
/// prints for a REAL pin, then writes exactly that text as a message and
/// requires the two to be distinguishable. Whichever side the delivery
/// changes (marker prefix, message quoting) is its choice.
#[test]
fn red_audit_markers_are_not_forgeable_by_a_commit_message() {
    // A genuine pin commit, with NO message, so its body is the marker alone.
    let d = fresh_dir("r4a");
    write(&d, "s.n", "a: 1\n");
    assert!(oo(&d, &["evolve", "s.n"]).ok, "harness: evolve");
    assert!(
        oo(&d, &["commit", "-m", "base"]).has("Commit successful"),
        "harness: base commit"
    );
    write(&d, "t.n", "a: 2\n");
    let p = oo(&d, &["evolve", "t.n", "--pin", "--grant", "pin"]);
    assert!(p.ok, "harness: pin evolve failed: {}", p.out);
    let c = oo(&d, &["commit", "--grant", "pin"]);
    assert!(c.has("Commit successful"), "harness: pin commit: {}", c.out);

    let real = head_audit_lines(&d);
    assert_eq!(
        real.len(),
        1,
        "harness: expected exactly the marker line for a message-less pin \
         commit, got {real:?}"
    );
    let marker = real[0].clone();

    // An ordinary commit whose MESSAGE is that exact text.
    let e = fresh_dir("r4b");
    write(&e, "s.n", "a: 1\n");
    assert!(oo(&e, &["evolve", "s.n"]).ok, "harness: evolve");
    assert!(
        oo(&e, &["commit", "-m", &marker]).has("Commit successful"),
        "harness: forging commit"
    );
    let forged = head_audit_lines(&e);

    assert_ne!(
        forged, real,
        "a commit message reproduced an engine audit marker exactly: {marker:?}"
    );

    // ACCEPTOR STRENGTHENING, from the adversarial pass. The first fix
    // prefixed the message — but only its FIRST line. Measured on that build,
    // with no capability of any kind:
    //
    //   oo commit -m $'x\n    privileged_effect\n    pin'
    //       message: x
    //       privileged_effect      ← byte-identical to a real marker
    //       pin                    ← byte-identical to a real marker
    //
    // A message is not one line, and the probe that only tried one line is why
    // this reached the adversarial pass instead of the red gate.
    let f = fresh_dir("r4c");
    write(&f, "s.n", "a: 1\n");
    assert!(oo(&f, &["evolve", "s.n"]).ok, "harness: evolve");
    let multi = format!("harmless\n{marker}\n    privileged_effect");
    assert!(
        oo(&f, &["commit", "-m", &multi]).has("Commit successful"),
        "harness: multi-line commit"
    );
    let lines = head_audit_lines(&f);
    assert!(
        !lines.iter().any(|l| l == &marker),
        "a multi-line commit message injected a raw audit marker: {lines:?}"
    );
    assert!(
        !lines.iter().any(|l| l == "privileged_effect"),
        "a multi-line commit message injected `privileged_effect`: {lines:?}"
    );
}

/// R5 — `--grant commit` is retired.
///
/// SPEC_08 §6.2 retired `#commit` on 2026-07-26 (「量測顯示該描述無對應閘位」).
/// The CLI spelling outlived it and gates nothing: `privilege.commit` has ZERO
/// consumer sites in the engine. A capability the interface advertises and
/// never checks is the same shape as the dead morphisms this project retires.
///
/// PAIRED: the surviving grants must keep working.
#[test]
fn red_grant_commit_spelling_is_retired() {
    let d = fresh_dir("r5");
    write(&d, "s.n", "a: 1\n");

    let dead = oo(&d, &["eval", "1", "--grant", "commit"]);
    assert!(
        !dead.ok,
        "`--grant commit` is still accepted after the concept was retired: {}",
        dead.out
    );

    // CONTROL: the live capabilities must not have been taken with it.
    for g in ["pin", "rollback", "squash", "effect_override"] {
        let live = oo(&d, &["eval", "1", "--grant", g]);
        assert!(live.ok, "control: `--grant {g}` broke: {}", live.out);
    }
}

// ─────────────────────────────────────────────────────────────────────────
// PINS — green at baseline, must stay green
// ─────────────────────────────────────────────────────────────────────────

/// P1 — an ordinary commit's meta keeps exactly its ordinary keys.
///
/// This is the invariant the whole arc rests on. `Commit::content_hash` hashes
/// `format!("{:?}", meta)`, and `CommitMeta` stays bit-stable across versions
/// only because its HAND-WRITTEN `Debug` omits `abandoned` when it is `None`
/// (v0.2.45). Any new marker field must follow that pattern or every existing
/// commit CAID moves.
#[test]
fn pin_ordinary_commit_meta_has_no_extra_keys() {
    let d = fresh_dir("p1");
    write(&d, "s.n", "a: 1\n");
    assert!(oo(&d, &["evolve", "s.n"]).ok, "harness: evolve");
    assert!(
        oo(&d, &["commit", "-m", "ordinary"]).has("Commit successful"),
        "harness: commit"
    );
    let meta = &object_json(&d, &head_commit(&d))["meta"];
    let mut keys: Vec<&str> = meta
        .as_object()
        .unwrap_or_else(|| panic!("meta is not an object: {meta}"))
        .keys()
        .map(|s| s.as_str())
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        vec!["author", "message", "timestamp"],
        "an ordinary commit's meta gained a key, which moves every commit CAID"
    );
}

/// P2 — an ordinary value's address does not move. The marker belongs on the
/// commit and must never reach a value (SPEC_08 §6.2 幾何指紋).
#[test]
fn pin_ordinary_value_caids_do_not_move() {
    let d = fresh_dir("p2");
    write(
        &d,
        "i.n",
        "id: ~%Discovery./identify_and_store { hello: \"world\" }\n",
    );
    let got = oo(&d, &["run", "i.n", "--observe", "id"])
        .out
        .trim()
        .trim_start_matches('"')
        .split('"')
        .next()
        .unwrap()
        .to_string();
    assert_eq!(got, GOLDEN_VALUE_CAID, "an ordinary value's address moved");
}

/// P3 — `runPure` stays gated. If this ever goes green-by-removal the whole
/// arc is measuring an ungated operation.
#[test]
fn pin_runpure_is_still_refused_without_a_grant() {
    let d = fresh_dir("p3");
    let refused = oo(&d, &["eval", "(~%Effect./runPure (~%Time.now _))"]);
    assert!(
        refused.has("privileged_required"),
        "runPure is no longer gated: {}",
        refused.out
    );
    let allowed = oo(
        &d,
        &[
            "eval",
            "(~%Effect./runPure (~%Time.now _))",
            "--grant",
            "effect_override:io",
        ],
    );
    assert!(
        !allowed.has("privileged_required"),
        "control: the grant no longer discharges: {}",
        allowed.out
    );
}

/// P4 — the other audit markers survive the R4 format change.
#[test]
fn pin_pin_marker_still_present_on_a_pin_commit() {
    let d = fresh_dir("p4");
    write(&d, "s.n", "a: 1\n");
    assert!(oo(&d, &["evolve", "s.n"]).ok, "harness: evolve");
    assert!(
        oo(&d, &["commit", "-m", "base"]).has("Commit successful"),
        "harness: base"
    );
    write(&d, "t.n", "a: 2\n");
    assert!(
        oo(&d, &["evolve", "t.n", "--pin", "--grant", "pin"]).ok,
        "harness: pin evolve"
    );
    assert!(
        oo(&d, &["commit", "-m", "pinned", "--grant", "pin"]).has("Commit successful"),
        "harness: pin commit"
    );
    let lines = head_audit_lines(&d);
    assert!(
        lines.iter().any(|l| l.contains("pin")),
        "the pin marker vanished from `oo log`: {lines:?}"
    );
}

/// P5 — an unprivileged repository still commits normally. The new gate must
/// fire on discharged content and on nothing else.
#[test]
fn pin_ordinary_work_needs_no_capability() {
    let d = fresh_dir("p5");
    write(&d, "s.n", PLAIN_SRC);
    assert!(oo(&d, &["evolve", "s.n"]).ok, "harness: evolve");
    let c = oo(&d, &["commit", "-m", "ordinary"]);
    assert!(
        c.has("Commit successful"),
        "an ordinary commit now demands a capability: {}",
        c.out
    );
}

/// P7 — the marker must reflect the DISCHARGE, not the grant.
///
/// `oo commit --grant effect_override:io` over content that never discharged
/// anything must not be marked. Marking on the presence of a flag rather than
/// on the fact would be a lying audit surface pointing the other way, and this
/// project has retired one of those per arc since v0.2.41.
///
/// Trivially green at baseline (nothing is ever marked); it is exactly what an
/// implementation keyed on `privilege.effect_override.is_some()` would break.
#[test]
fn pin_a_grant_without_a_discharge_marks_nothing() {
    let d = fresh_dir("p7");
    write(&d, "s.n", PLAIN_SRC);
    assert!(
        oo(&d, &["evolve", "s.n", "--grant", "effect_override:io"]).ok,
        "harness: evolve"
    );
    let c = oo(
        &d,
        &[
            "commit",
            "-m",
            "granted but pure",
            "--grant",
            "effect_override:io",
        ],
    );
    assert!(c.has("Commit successful"), "harness: commit: {}", c.out);

    let commit = object_json(&d, &head_commit(&d));
    assert!(
        !commit.to_string().contains("privileged_effect"),
        "a commit that discharged nothing was marked privileged: {commit}"
    );
}

/// P8 — the capability re-presented at commit must COVER what was discharged.
///
/// ACCEPTOR REPAIR pin. Measured on the delivered build: a discharge of `io`
/// was authorised at commit by `--grant effect_override:nondet`, because the
/// gate asked `is_none()` — *a* capability, not *the* capability. SPEC_08
/// §6.1.4 axis 2 is `C ⊇ E`, and a capability that would not have authorised
/// the discharge cannot authorise fixing it into history either.
#[test]
fn pin_commit_capability_must_cover_the_discharged_tags() {
    let d = repo_with_discharge("p8");

    let wrong = oo(
        &d,
        &[
            "commit",
            "-m",
            "wrong tag",
            "--grant",
            "effect_override:nondet",
        ],
    );
    assert!(
        !wrong.ok,
        "a capability that does not cover the discharge authorised the commit: {}",
        wrong.out
    );
    assert!(
        wrong.has("privileged_required"),
        "the refusal must name the missing capability: {}",
        wrong.out
    );

    // PAIR: the covering capability works, so the gate is not simply stuck.
    let right = oo(
        &d,
        &["commit", "-m", "right tag", "--grant", "effect_override:io"],
    );
    assert!(
        right.has("Commit successful"),
        "the covering capability was refused: {}",
        right.out
    );
}

/// P9 — `runPure` over an already-pure value overrides nothing, so it is not
/// a privileged intervention and must not be recorded as one.
///
/// ACCEPTOR REPAIR pin. Measured on the delivered build, `v: (~%Effect./runPure
/// 42)` demanded a capability at commit and stamped `#privileged_effect` on the
/// result. `#effect_override` is 「強制將**含副作用**節點標記為 `#pure`」 —
/// with no effect there is nothing to force, and an audit line asserting an
/// intervention that never happened is the surface this project keeps retiring.
#[test]
fn pin_runpure_over_a_pure_value_is_not_an_intervention() {
    let d = fresh_dir("p9");
    write(&d, "s.n", "v: (~%Effect./runPure 42)\n");
    assert!(
        oo(&d, &["evolve", "s.n", "--grant", "effect_override:io"]).ok,
        "harness: evolve"
    );

    let c = oo(&d, &["commit", "-m", "nothing was overridden"]);
    assert!(
        c.has("Commit successful"),
        "a commit that overrode nothing demanded a capability: {}",
        c.out
    );
    let commit = object_json(&d, &head_commit(&d));
    assert!(
        !commit.to_string().contains("privileged_effect"),
        "a commit that overrode nothing was marked privileged: {commit}"
    );
}

/// P6 — the universe root stays independent of process and workspace
/// (v0.2.45 §4.1.2 #1, re-pinned because this arc touches the commit path).
#[test]
fn pin_root_caid_stays_deterministic() {
    let digests: std::collections::BTreeSet<String> = (0..3)
        .map(|i| {
            let d = fresh_dir(&format!("p6-{i}"));
            write(&d, "s.n", PLAIN_SRC);
            oo(&d, &["evolve", "s.n"]);
            assert!(
                oo(&d, &["commit", "-m", "x"]).has("Commit successful"),
                "harness: commit"
            );
            let commit = object_json(&d, &head_commit(&d));
            let dg = &commit["root"]["digest"];
            if let Some(s) = dg.as_str() {
                s.to_string()
            } else if let Some(a) = dg.as_array() {
                a.iter()
                    .map(|b| format!("{:02x}", b.as_u64().expect("digest byte")))
                    .collect::<String>()
            } else {
                panic!("no usable digest: {}", commit["root"]);
            }
        })
        .collect();
    assert_eq!(
        digests.len(),
        1,
        "the universe root became process-dependent again: {digests:#?}"
    );
}
