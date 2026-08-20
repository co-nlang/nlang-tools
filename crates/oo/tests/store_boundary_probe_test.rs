// The store trust boundary — `.oo/` is the engine's, not the language's
// (2026-07-26, pre-committed by work order: docs/store_boundary_handover.md).
//
// ── What was measured (baseline v0.2.41, all end-to-end, not read off code) ──
//
// A4  An unprivileged n/ program wrote `.oo/HEAD` and moved history:
//         $ oo rollback <c1>            → #privileged_required   (refused)
//         $ oo run a4.n -o w            → #true                  (succeeded)
//         $ oo log                      → tip is now c1
//         $ ls .oo/abandoned            → absent
//     The same effect as the gated operation, through the back door, leaving
//     LESS audit trace than the legitimate path leaves.
//
// A5  An unprivileged program wrote `.oo/abandoned`; the next legitimate
//     commit sealed a forged abandonment claim into its CAID — a permanent
//     lie in the audit face, about a commit that never existed.
//
// A2  An unprivileged program wrote `.oo/architects.json`, the trust root for
//     `verify_refine_authority`. Compounding irony: `engine.add_architect` is
//     itself DEAD (see A1), and `save_architects` is its only caller — so the
//     forgeable file is the ONLY live path into the refine trust root.
//
// A1  `~%Official./add_architect` is always `_|_ #conflict`. The builtin does
//     `oo.force(arg).to_string_plain()` on the whole argument; apply hands it
//     `{0: str}`, whose `to_string_plain()` is `"{...}"` (len 5 != 64). Same
//     apply-seam class as v0.2.39's `check_oml`. Fail-safe, but it means the
//     morphism has no body.
//
// ── The thesis ────────────────────────────────────────────────────────────
// SPEC_08 §6.2 gained a normative clause in the #pin arc (v0.2.40): authority
// must be presented, through a trusted channel, AT THE MOMENT the privileged
// effect is applied. That clause was written for #pin and never generalized.
// Every §6.2 capability gate lives on a CLI verb, and every one of them has a
// filesystem back door. A4 is the proof: the capability lattice built across
// v0.2.38/40/41 is decorative for as long as `~%Io./write_file` can reach the
// store.
//
// ── Rulings ───────────────────────────────────────────────────────────────
// R-A (user, 2026-07-26): SANDBOX model. The language layer cannot touch the
//     store. Honest scope, stated as such in the spec: this closes the
//     language-level attack; it does not pretend to defend against someone
//     with shell access (who can also replace the `oo` binary). REAL_01 §7.2's
//     out-of-store keys / HSM remain ledgered, not built.
//
// R-B (user, 2026-07-26): authorization layer this arc (A1/A2/A4/A5).
//     CAS read integrity (a tampered object is loaded unverified — CAID is the
//     identity of a value, but the store returns whatever bytes sit at the
//     path) is the NEXT arc; it carries its own ruling (digest only, or
//     lattice_sketch too — REAL_03 §9.2 leaves a back-compat door).
//
// R-C (acceptor, stated for veto): the rule is a RESERVED PATH COMPONENT, not
//     a string prefix and not a base_dir comparison. A path is refused iff,
//     after resolving `.`/`..`/symlinks, any component equals exactly `.oo`.
//     Rationale: one sentence to specify, no base_dir dependency, no TOCTOU,
//     covers other workspaces' stores for free, and component-exact matching
//     makes the `.oo_peer_a` prefix trap structurally impossible. Cost: a
//     user directory that happens to be named `.oo` is unreachable — same
//     deal `.git` gets, and REAL_01 §4 already reserves the name.
//
// R-D (acceptor, stated for veto): the boundary is UNCONDITIONAL. `--privileged`
//     does not unlock it. The capability lattice governs §6.2 lattice and
//     history operations; it does not govern the store's physical bytes. If
//     privilege unlocked raw store writes, a privileged program could forge
//     the very audit records §6.2 requires — the guarantee would be circular.
//
// R-E (acceptor, stated for veto): reads are blocked too, not just writes.
//     The ruling says "n/ cannot touch the store", and a uniform boundary is
//     what the spec can state in one line. A read carve-out is cheap to add
//     later and expensive to reason about now.
//
// A1 resolution: retire `~%Official./add_architect` from the language surface
//     rather than repair it. It is the front door to exactly the trust root
//     this arc is closing the back door on, it is unused across the whole tree
//     (scanned: only its own definition plus historical handover docs), and
//     REAL_01 §7.2's own answer is out-of-band provisioning from
//     `~/.oo/authorized_keys`. Repairing it would open a hole in the same
//     breath as closing one.
//
// ── Out of scope, ledgered, NOT to be fixed here ──────────────────────────
// * The identity is `Identity::new_random()` at every engine start (lib.rs
//   354/377). The local pubkey differs every process, so a signature made in
//   one run is unverifiable in the next: refine authority is self-authorizing
//   within a process and impossible across processes. Real, but it is the
//   REAL_01 §7 / REAL_02 identity-persistence build, not this arc.
// * Byte-reclamation GC. Note the store already accumulates: every `oo eval`
//   and `oo run -o` leaves objects behind (measured — a `_|_` from a failed
//   call was found sitting in `objects/`). GC must come AFTER this arc: a GC
//   that trusts `.oo/HEAD` can be told to delete live history.
//
// ── Refusal shape ─────────────────────────────────────────────────────────
// `_|_` with a new `BottomCause` tail variant → `#store_boundary`. NOT `#none`
// and NOT `#false`: v0.2.41's lesson is that an audit face you cannot tell
// apart from an ordinary outcome is not an audit face. A refused `exists`
// must be distinguishable from "the file is not there".
//
// ── Filesystem surface (enumerated, untruncated) ──────────────────────────
//   io.read_file  io.write_file  io.exists  io.append_file   (builtins/io.rs)
//   csv.read_csv                                            (builtins/csv.rs)
//   disc.connect — ObjectStore::init(<peer base_dir>)        (builtins/disc.rs)
// No process-spawning builtin exists (process.rs is exit/pid only), so this
// list is closed.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

fn fresh_dir() -> nlang_interpreter::ScratchDir {
    nlang_interpreter::ScratchDir::new("store")
}

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
    if let Some(p) = dir.join(name).parent() {
        fs::create_dir_all(p).ok();
    }
    fs::write(dir.join(name), src).unwrap();
}

/// A universe with `n` commits (`f1: 1` … `fn: n`). Returns CAIDs oldest-first.
fn history(dir: &Path, n: usize) -> Vec<String> {
    for i in 1..=n {
        write(dir, "s.n", &format!("f{i}: {i}\n"));
        oo(dir, &["evolve", "s.n"]);
        oo(dir, &["commit", "-m", &format!("c{i}")]);
    }
    let mut caids = log_caids(dir);
    caids.reverse();
    caids
}

fn log_caids(dir: &Path) -> Vec<String> {
    oo(dir, &["log"])
        .lines()
        .filter_map(|l| l.strip_prefix("commit ").map(|s| s.trim().to_string()))
        .collect()
}

fn head(dir: &Path) -> String {
    fs::read_to_string(dir.join(".oo").join("HEAD")).unwrap_or_default()
}

/// Runs one n/ program that performs TWO operations: a CONTROL against an
/// ordinary path, which must succeed, and the TARGET operation under test.
///
/// The control is what makes every refusal gate below a real discriminator
/// rather than a vacuous one: it proves, inside the very same run, that the
/// builtin is live, the harness works, and the program actually executed. A
/// gate that only asserted "the target was refused" would also pass if the
/// program never ran at all — that is exactly the failure this suite's three
/// predecessors were miscalibrated by.
fn probe(dir: &Path, control: &str, target: &str, extra_args: &[&str]) -> String {
    write(
        dir,
        "probe.n",
        &format!("r: {{\n  ctl: {control}\n  tgt: {target}\n}}\n"),
    );
    let mut args: Vec<&str> = vec!["run", "probe.n", "-o", "r"];
    args.extend_from_slice(extra_args);
    oo(dir, &args)
}

/// Every boundary gate asserts, in order: (1) the program ran and the builtin
/// is live — the control operation reports its ordinary success value; (2) the
/// target was refused with the boundary cause specifically, not some other
/// bottom and not a silent failure value.
///
/// CALIBRATION NOTE. Two of these gates were vacuous on the first pass: their
/// payload was spelled `"[\"bb\"]"`, which evaluates to `_|_ #conflict`, so
/// `write_file` was never applied and the gate went red without the operation
/// under test ever running. Build string payloads with `~%Json./stringify`.
/// The lesson generalizes: a red gate must be checked for WHY it is red, not
/// merely that it is.
fn assert_refused(out: &str, control_ok: &str) {
    assert!(
        !out.is_empty(),
        "the probe program must have run and produced output"
    );
    assert!(
        out.contains(control_ok),
        "CONTROL must succeed — otherwise this gate proves nothing about the \
         boundary, only that the program failed. expected {control_ok:?} in: {out:?}"
    );
    assert!(
        out.contains("store_boundary"),
        "TARGET must be refused with #store_boundary specifically: {out:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — the boundary itself
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_write_into_store_is_refused() {
    let d = fresh_dir();
    history(&d, 1);
    let before = head(&d);
    let out = probe(
        &d,
        r#"~%Io./write_file("ctl.txt", "live")"#,
        r#"~%Io./write_file(".oo/HEAD", "TAMPER")"#,
        &[],
    );
    assert_refused(&out, "ctl: #true");
    assert_eq!(head(&d), before, "a refused write must not reach the file");
}

#[test]
fn red_append_into_store_is_refused() {
    let d = fresh_dir();
    history(&d, 1);
    let out = probe(
        &d,
        r#"~%Io./append_file("ctl.txt", "live")"#,
        r#"~%Io./append_file(".oo/abandoned", "forged")"#,
        &[],
    );
    assert_refused(&out, "ctl: #true");
    assert!(
        !d.join(".oo").join("abandoned").exists(),
        "a refused append must not create the file"
    );
}

#[test]
fn red_read_from_store_is_refused() {
    let d = fresh_dir();
    history(&d, 1);
    let out = probe(
        &d,
        r#"~%Io./read_file("s.n")"#,
        r#"~%Io./read_file(".oo/HEAD")"#,
        &[],
    );
    assert_refused(&out, "f1: 1");
    assert!(
        !out.contains("hash:sha256"),
        "the store's contents must not come back through the refusal: {out:?}"
    );
}

#[test]
fn red_exists_on_store_is_refused_not_answered_false() {
    // Legibility (the v0.2.41 rule): a refusal that renders as `#false` is
    // indistinguishable from "the file is not there", so it is not an audit
    // face. The control pins the true `#false` answer in the same run.
    let d = fresh_dir();
    history(&d, 1);
    let out = probe(
        &d,
        r#"~%Io./exists("definitely-absent.txt")"#,
        r#"~%Io./exists(".oo/HEAD")"#,
        &[],
    );
    assert_refused(&out, "ctl: #false");
}

#[test]
fn red_dotdot_escape_into_store_is_refused() {
    // MEASURED at baseline: `sub/../.oo/HEAD` overwrote HEAD. The rule must
    // resolve the path before judging it, never match on the literal string.
    let d = fresh_dir();
    history(&d, 1);
    fs::create_dir_all(d.join("sub")).unwrap();
    let before = head(&d);
    let out = probe(
        &d,
        r#"~%Io./write_file("sub/../ctl.txt", "live")"#,
        r#"~%Io./write_file("sub/../.oo/HEAD", "TAMPER")"#,
        &[],
    );
    assert_refused(&out, "ctl: #true");
    assert_eq!(head(&d), before);
}

#[test]
fn red_absolute_path_into_store_is_refused() {
    let d = fresh_dir();
    history(&d, 1);
    let before = head(&d);
    let abs = d.join(".oo").join("HEAD");
    let abs_ctl = d.join("ctl.txt");
    let out = probe(
        &d,
        &format!(
            r#"~%Io./write_file("{}", "live")"#,
            abs_ctl.display().to_string().replace('\\', "/")
        ),
        &format!(
            r#"~%Io./write_file("{}", "TAMPER")"#,
            abs.display().to_string().replace('\\', "/")
        ),
        &[],
    );
    assert_refused(&out, "ctl: #true");
    assert_eq!(head(&d), before);
}

#[cfg(unix)]
#[test]
fn red_symlink_escape_into_store_is_refused() {
    // A symlink whose name is innocent and whose target is the store. The
    // boundary must resolve links, not trust the spelling it was handed.
    let d = fresh_dir();
    history(&d, 1);
    std::os::unix::fs::symlink(d.join(".oo"), d.join("innocent")).unwrap();
    let before = head(&d);
    let out = probe(
        &d,
        r#"~%Io./write_file("ctl.txt", "live")"#,
        r#"~%Io./write_file("innocent/HEAD", "TAMPER")"#,
        &[],
    );
    assert_refused(&out, "ctl: #true");
    assert_eq!(head(&d), before);
}

#[test]
fn red_privilege_does_not_unlock_the_boundary() {
    // R-D. The capability lattice governs §6.2 operations, not the store's
    // bytes. If `--privileged` opened this door, a privileged program could
    // forge the audit records §6.2 depends on and the guarantee would close
    // on itself.
    let d = fresh_dir();
    history(&d, 1);
    let before = head(&d);
    let out = probe(
        &d,
        r#"~%Io./write_file("ctl.txt", "live")"#,
        r#"~%Io./write_file(".oo/HEAD", "TAMPER")"#,
        &["--privileged"],
    );
    assert_refused(&out, "ctl: #true");
    assert_eq!(head(&d), before);

    // …and the same for the finest-grained grant that exists.
    let out2 = probe(
        &d,
        r#"~%Io./write_file("ctl.txt", "live")"#,
        r#"~%Io./write_file(".oo/HEAD", "TAMPER")"#,
        &["--grant", "rollback"],
    );
    assert_refused(&out2, "ctl: #true");
    assert_eq!(head(&d), before);
}

#[test]
fn red_csv_read_from_store_is_refused() {
    let d = fresh_dir();
    history(&d, 1);
    write(&d, "ok.csv", "a,b\n1,2\n");
    let out = probe(
        &d,
        r#"~%Csv./read_csv("ok.csv")"#,
        r#"~%Csv./read_csv(".oo/HEAD")"#,
        &[],
    );
    assert_refused(&out, "\"a\"");
}

#[test]
fn red_disc_connect_to_a_store_is_refused() {
    // `disc.connect` takes a peer BASE dir and calls `ObjectStore::init` on
    // it, which creates and then reads `<base>/.oo/objects`. Handing it a
    // path that already names a store is the same boundary crossing wearing
    // a federation hat.
    let d = fresh_dir();
    history(&d, 1);
    fs::create_dir_all(d.join("peer")).unwrap();
    let out = probe(
        &d,
        r#"~%Discovery./connect { 0: "PeerOK", 1: "peer" }"#,
        r#"~%Discovery./connect { 0: "PeerBad", 1: ".oo" }"#,
        &[],
    );
    assert!(!out.is_empty(), "the probe program must have run");
    assert!(
        out.contains("store_boundary"),
        "a peer path inside a store must be refused: {out:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — the escalations, end to end
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_a4_head_rewrite_no_longer_rolls_back_history() {
    // The whole arc in one test, as a three-way discriminator. Closing the
    // back door is only half the claim; the other half is that the front door
    // still opens exactly as far as it did before.
    let d = fresh_dir();
    let c = history(&d, 3);
    let tip = log_caids(&d)[0].clone();

    // (a) back door: an unprivileged program cannot move HEAD.
    let out = probe(
        &d,
        r#"~%Io./write_file("ctl.txt", "live")"#,
        &format!(r#"~%Io./write_file(".oo/HEAD", "{}")"#, c[0]),
        &[],
    );
    assert_refused(&out, "ctl: #true");
    assert_eq!(
        log_caids(&d)[0],
        tip,
        "history must not have moved through the back door"
    );

    // (b) front door, ungranted: still refused, for the stated reason.
    let refused = oo(&d, &["rollback", &c[0]]);
    assert!(
        refused.contains("privileged_required"),
        "the capability gate must still refuse: {refused:?}"
    );
    assert_eq!(log_caids(&d)[0], tip);

    // (c) front door, granted: still works. Without this the test would also
    // pass if the arc simply broke rollback.
    let ok = oo(&d, &["rollback", &c[0], "--grant", "rollback"]);
    assert!(
        !ok.contains("error"),
        "granted rollback must succeed: {ok:?}"
    );
    assert_eq!(
        log_caids(&d)[0],
        c[0],
        "granted rollback must still move HEAD"
    );
}

#[test]
fn red_a5_abandonment_record_cannot_be_forged() {
    // Paired against the genuine mechanism: the test must be able to SEE an
    // abandonment record when one is real, or "no record appeared" proves
    // nothing.
    let d = fresh_dir();
    let c = history(&d, 2);

    // Forgery attempt, then an ordinary commit that would consume it.
    let out = probe(
        &d,
        r#"~%Io./write_file("ctl.txt", "live")"#,
        &format!(r#"~%Io./write_file(".oo/abandoned", "{}")"#, c[0]),
        &[],
    );
    assert_refused(&out, "ctl: #true");
    write(&d, "n.n", "later: 9\n");
    oo(&d, &["evolve", "n.n"]);
    oo(&d, &["commit", "-m", "ordinary"]);
    assert!(
        !oo(&d, &["log"]).contains("abandoned"),
        "no commit may carry an abandonment record that no rollback created"
    );

    // Control: the genuine path still records one.
    oo(&d, &["rollback", &c[0], "--grant", "rollback"]);
    write(&d, "m.n", "resumed: 7\n");
    oo(&d, &["evolve", "m.n"]);
    oo(&d, &["commit", "-m", "after-real-rollback"]);
    assert!(
        oo(&d, &["log"]).contains("abandoned"),
        "a genuine rollback must still be recorded — otherwise the assertion \
         above is satisfied by a broken mechanism rather than by the boundary"
    );
}

#[test]
fn red_a2_architect_trust_root_is_unwritable() {
    let d = fresh_dir();
    history(&d, 1);
    let out = probe(
        &d,
        r#"~%Io./write_file("ctl.txt", "live")"#,
        r#"~%Io./write_file(".oo/architects.json", ~%Json./stringify(["bb"]))"#,
        &[],
    );
    assert_refused(&out, "ctl: #true");
    assert!(
        !d.join(".oo").join("architects.json").exists(),
        "the refine trust root must not be reachable from the language"
    );
}

#[test]
fn red_pin_pending_is_unwritable() {
    // v0.2.40 sealed this at `oo commit` by re-demanding the capability. The
    // file itself stayed writable, which left the intent record forgeable on
    // top of a legitimate pin. The boundary closes the file too.
    let d = fresh_dir();
    history(&d, 1);
    let out = probe(
        &d,
        r#"~%Io./write_file("ctl.txt", "live")"#,
        r#"~%Io./write_file(".oo/pin_pending", ~%Json./stringify(["f1"]))"#,
        &[],
    );
    assert_refused(&out, "ctl: #true");
    assert!(!d.join(".oo").join("pin_pending").exists());
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATE — A1, the dead morphism
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_add_architect_is_off_the_language_surface() {
    // Paired discriminator. At baseline this answers `#conflict` (the apply
    // seam), and a genuinely absent key answers `#missing_key` — so the two
    // are distinguishable and "retired" is a checkable claim rather than an
    // indistinguishable shade of bottom. `~%Official` itself is the control:
    // it proves the module is still mounted, so a pass cannot come from the
    // whole object having vanished.
    //
    // The control used to be `/sign_refine`. The identity_persistence arc
    // retires that morphism, so the ACCEPTOR moved this control to the module
    // rather than leave a false red for that delivery. `{{` and not "not
    // bottom": a module removed from the (open) system root evaluates to `_`.
    let d = fresh_dir();
    history(&d, 1);
    let sign = oo(&d, &["eval", "~%Official"]);
    assert!(
        sign.contains("missing_key"),
        "control: ~%Official must answer #missing_key, not a synthesised shell: {sign:?}"
    );
    let got = oo(&d, &["eval", r#"~%Official./add_architect("x")"#]);
    assert!(
        got.contains("missing_key"),
        "a retired morphism must be absent, not silently bottom: {got:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// PINS — green at baseline, must stay green
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn pin_ordinary_filesystem_access_still_works() {
    let d = fresh_dir();
    history(&d, 1);
    let out = probe(
        &d,
        r#"~%Io./write_file("a.txt", "one")"#,
        r#"~%Io./append_file("a.txt", "-two")"#,
        &[],
    );
    assert!(out.contains("ctl: #true"), "{out:?}");
    assert!(out.contains("tgt: #true"), "{out:?}");
    assert_eq!(fs::read_to_string(d.join("a.txt")).unwrap(), "one-two");

    let back = oo(&d, &["eval", r#"~%Io./read_file("a.txt")"#]);
    assert!(back.contains("one-two"), "{back:?}");
    let ex = oo(&d, &["eval", r#"~%Io./exists("a.txt")"#]);
    assert!(ex.contains("#true"), "{ex:?}");
}

#[test]
fn pin_dot_oo_is_matched_by_component_never_by_prefix() {
    // THE trap. `.oo_peer_a` is a real path in this tree
    // (tests/pending/federation_test.n), and `str::starts_with(".oo")`
    // swallows it. So do `.oomisc` and `foo.oo`. Component-exact matching is
    // the requirement, not an optimization.
    let d = fresh_dir();
    history(&d, 1);
    for name in [".oo_peer_a/f.txt", ".oomisc", "foo.oo", "sub/.ooo/f.txt"] {
        if let Some(p) = d.join(name).parent() {
            fs::create_dir_all(p).unwrap();
        }
        let out = probe(
            &d,
            r#"~%Io./write_file("ctl.txt", "live")"#,
            &format!(r#"~%Io./write_file("{name}", "ok")"#),
            &[],
        );
        assert!(
            out.contains("tgt: #true"),
            "{name} is not the store and must remain writable: {out:?}"
        );
        assert!(!out.contains("store_boundary"), "{name}: {out:?}");
    }
}

#[test]
fn pin_engine_writes_are_unaffected() {
    // The boundary is on the language surface. The engine reaches the store
    // through ObjectStore/Universe and must be untouched by it.
    let d = fresh_dir();
    let c = history(&d, 3);
    assert_eq!(c.len(), 3, "three commits must have been created");
    assert!(head(&d).starts_with("hash:sha256"), "HEAD must be written");
    assert!(oo(&d, &["status"]).contains("static"), "status must work");
}

#[test]
fn pin_gated_history_operations_still_work() {
    let d = fresh_dir();
    let c = history(&d, 3);
    let r = oo(&d, &["rollback", &c[0], "--grant", "rollback"]);
    assert!(!r.contains("error"), "rollback --grant must work: {r:?}");
    assert_eq!(log_caids(&d)[0], c[0]);

    let d2 = fresh_dir();
    let c2 = history(&d2, 3);
    let s = oo(&d2, &["squash", &c2[0], "--grant", "squash"]);
    assert!(!s.contains("error"), "squash --grant must work: {s:?}");
}

#[test]
fn pin_csv_read_of_an_ordinary_file_still_works() {
    let d = fresh_dir();
    history(&d, 1);
    write(&d, "t.csv", "a,b\n1,2\n");
    let got = oo(&d, &["eval", r#"~%Csv./read_csv("t.csv")"#]);
    assert!(got.contains('a') && got.contains('1'), "{got:?}");
}
