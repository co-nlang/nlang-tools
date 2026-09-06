// An authority you could delete.
// Rulings: nlang-spec/meta/oo/STATUS.md D60 (price), D58 (shape), D57 (why
//          audit does not live in the workspace)
// Order:   nlang-tools/docs/an_authority_you_could_delete_handover.md
//
// -- What this arc is ----------------------------------------------------
//
// Discharging an effect under `--grant effect_override:<tag>` records what
// was actually discharged, and the commit refuses unless a capability
// covering it is re-presented. That check works:
//
//     oo evolve --grant effect_override:io  (runPure over a clock read)
//     oo commit                             -> rc 1, "discharged #io"
//
// The record lives in `.oo/effect_pending`, one shared cell rewritten
// whole. Delete it:
//
//     rm .oo/effect_pending
//     oo commit                             -> rc 0, Commit successful
//                                              and no privileged mark
//
// So privileged-discharged content enters history with no capability
// presented and no audit trace. The engine's own comment at the pin gate
// (main.rs) says that directory is one any n/ program can write through
// `~%Io./write_file`, and that trusting durable intent as authority once
// let an unprivileged program obtain pin semantics. That repair was made
// for pin on 2026-07-26. It was never made for effect.
//
// The same shared cell also loses one of two concurrent writers, which is
// how this was found: two evolves discharging different tags leave two
// injections and one tag, so a commit holding only the surviving grant
// lands content discharged under the other one. Ten trials, five landed.
//
// -- The fix (D58's shape, D60's price) ----------------------------------
//
// The discharge intent travels in the same immutable member as the value
// it describes, and the commit takes the union over members. That costs a
// container layout, because the injection decoder rejects any metadata
// line it does not know -- deliberately, since that strictness is what
// makes "the declaration is still true" enforceable.
//
// What this does NOT buy, stated so no one claims it: putting the intent
// in the member does not make it tamper-proof. Rewriting a member is
// still the assertion layer. What it buys is that the authority fact is
// no longer SEPARABLE -- removing it means rewriting the object the value
// itself came from, so any integrity check over that object covers it.
//
// NOT PROBED, stated so no one mistakes silence for coverage:
//   * `.oo/abandoned` (`append_abandoned_file`) is the third member of
//     this family and has the same load-modify-write shape. Its exposure
//     differs: rollback refuses on a dirty worktree, and two concurrent
//     rollbacks abandon the same digest and dedupe. The remaining window
//     is a rollback landing between a commit clearing injections and that
//     same commit clearing `abandoned`. NOT REPRODUCED, and an earlier
//     attempt at it was a void reading -- rollback exited 1 in all 20
//     rounds because the measurement had staged a change first. Out of
//     scope here; the order asks for it to be left alone.
//   * Whether an unprivileged n/ program can actually reach the file
//     through `~%Io./write_file` on today's build. The threat model is
//     REAL_01 7.3's and does not need that demonstrated, but nothing here
//     demonstrates it either.

use std::path::Path;
use std::process::Command;

const IO: &str = "p: (~%Effect./runPure (~%Time.now _))\n";
const NONDET: &str = "q: (~%Effect./runPure (~%Math./random _))\n";

fn cmd(dir: &Path) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_oo"));
    c.current_dir(dir)
        .env("OO_IDENTITY", dir.join("identity-for-tests"))
        .env("OO_NODE_HOME", dir.join("node-home-for-tests"));
    c
}

fn oo(dir: &Path, args: &[&str]) -> (String, i32) {
    let o = cmd(dir).args(args).output().expect("oo runs");
    (
        format!(
            "{}{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        ),
        o.status.code().unwrap_or(-1),
    )
}

fn scratch(tag: &str) -> nlang_interpreter::ScratchDir {
    nlang_interpreter::ScratchDir::new(&format!("authority-{tag}"))
}

fn write(d: &Path, name: &str, body: &str) {
    std::fs::write(d.join(name), body).expect("write source");
}

/// Every regular file under `.oo/injections`.
fn injections(d: &Path) -> usize {
    match std::fs::read_dir(d.join(".oo").join("injections")) {
        Ok(rd) => rd
            .flatten()
            .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
            .count(),
        Err(_) => 0,
    }
}

/// A repo with one discharged evolve staged.
fn discharged(d: &Path, src: &str, grant: &str) {
    write(d, "a.n", src);
    let (out, rc) = oo(d, &["evolve", "--grant", grant, "a.n"]);
    assert_eq!(rc, 0, "REACH: the discharged evolve must land: {out}");
    assert_eq!(injections(d), 1, "REACH: it minted one member");
}

// ---------------------------------------------------------------------
// R1. The control: with nothing tampered, the gate works. If this ever
// goes red the rest of the file is measuring the wrong thing.
// ---------------------------------------------------------------------
#[test]
fn r1_the_gate_refuses_without_the_capability() {
    let s = scratch("r1");
    let d = s.path();
    discharged(d, IO, "effect_override:io");

    let (out, rc) = oo(d, &["commit", "-m", "x"]);
    assert_eq!(rc, 1, "a commit with no grant must be refused: {out}");
    assert!(
        out.contains("discharged"),
        "and it must say what was discharged: {out}"
    );
}

// ---------------------------------------------------------------------
// R2. The nail. Removing a workspace file must not remove the authority
// requirement, because the workspace is the assertion layer and offers
// nothing against an adversary who can write it (REAL_01 7.3, the same
// reasoning D57 used to put the consent mark on the commit).
//
// This does not name a file. It removes every plain file directly under
// `.oo/` that is not one of the durable structures, which is exactly the
// class "a cell beside the values". If the intent lives with the value,
// there is nothing here to remove and the assertion holds trivially.
// ---------------------------------------------------------------------
#[test]
fn r2_deleting_a_sidecar_must_not_delete_the_authority_requirement() {
    let s = scratch("r2");
    let d = s.path();
    discharged(d, IO, "effect_override:io");

    let oo_dir = d.join(".oo");
    let keep = ["format", "objects.format", "HEAD"];
    let mut removed = Vec::new();
    for e in std::fs::read_dir(&oo_dir).expect("read .oo").flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        if p.is_file() && !keep.contains(&name.as_str()) {
            std::fs::remove_file(&p).expect("remove sidecar");
            removed.push(name);
        }
    }

    let (out, rc) = oo(d, &["commit", "-m", "y"]);
    assert_eq!(
        rc, 1,
        "removing {removed:?} let a discharged commit land with no \
         capability presented. The workspace is the assertion layer; an \
         authority fact that a writer can delete is not one. Commit said:\n{out}"
    );
}

// ---------------------------------------------------------------------
// R3. If it lands anyway, it must at least not land unmarked. Separate
// from R2 because they fail independently: an engine could refuse and
// still mark wrongly, or mark and still let it through.
// ---------------------------------------------------------------------
#[test]
fn r3_a_discharged_commit_is_never_unmarked() {
    let s = scratch("r3");
    let d = s.path();
    discharged(d, IO, "effect_override:io");

    let oo_dir = d.join(".oo");
    for e in std::fs::read_dir(&oo_dir).expect("read .oo").flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        if p.is_file() && !["format", "objects.format", "HEAD"].contains(&name.as_str()) {
            let _ = std::fs::remove_file(&p);
        }
    }
    let (_, rc) = oo(d, &["commit", "-m", "y"]);
    if rc != 0 {
        return; // R2 covers the refusal; nothing landed to inspect.
    }
    let (log, _) = oo(d, &["log"]);
    assert!(
        log.contains("privileged_effect"),
        "a commit that fixed privileged-discharged content into history \
         carries no mark of it; log said:\n{log}"
    );
}

// ---------------------------------------------------------------------
// R4. STATISTICAL, not a nail (ten rounds; this is a race).
//
// Two evolves discharging different tags. Whatever holds the intent must
// end up holding BOTH, so a commit presenting only one of them is
// refused. Measured 2026-09-03 on v0.43.0: the shared cell never took the
// union, and five of ten such commits landed.
// ---------------------------------------------------------------------
#[test]
fn r4_two_concurrent_discharges_both_survive() {
    for round in 0..10 {
        let s = scratch(&format!("r4-{round}"));
        let d = s.path();
        write(d, "a.n", IO);
        write(d, "b.n", NONDET);

        let a = cmd(d)
            .args(["evolve", "--grant", "effect_override:io", "a.n"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("spawn io");
        let b = cmd(d)
            .args(["evolve", "--grant", "effect_override:nondet", "b.n"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("spawn nondet");
        for mut p in [a, b] {
            p.wait().expect("evolve exits");
        }
        assert_eq!(injections(d), 2, "round {round}: REACH: both landed");

        let (out, rc) = oo(d, &["commit", "--grant", "effect_override:io", "-m", "c"]);
        assert_eq!(
            rc, 1,
            "round {round}: a capability covering only #io let content \
             discharged under #nondet into history -- the intent of one \
             writer was overwritten by the other. Commit said:\n{out}"
        );
    }
}

// ---------------------------------------------------------------------
// G1. Identity is a red line.
// ---------------------------------------------------------------------
#[test]
fn g1_identity_is_a_red_line() {
    let s = scratch("g1");
    let d = s.path();
    write(d, "a.n", "x: 0\n");
    assert_eq!(oo(d, &["evolve", "a.n"]).1, 0);
    assert_eq!(oo(d, &["commit", "-m", "identity"]).1, 0);

    let mut objects = 0;
    let mut stack = vec![d.join(".oo").join("objects")];
    while let Some(p) = stack.pop() {
        if let Ok(rd) = std::fs::read_dir(&p) {
            for e in rd.flatten() {
                let q = e.path();
                if q.is_dir() {
                    stack.push(q);
                } else {
                    objects += 1;
                }
            }
        }
    }
    assert_eq!(objects, 3, "a solid `x: 0` universe is three objects");

    let (out, _) = oo(d, &["status"]);
    assert!(
        out.contains("7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911"),
        "the standard root digest must not move; status said:\n{out}"
    );
}

// ---------------------------------------------------------------------
// G2. An ordinary commit is untouched: no discharge, no gate, no mark.
// ---------------------------------------------------------------------
#[test]
fn g2_an_ordinary_commit_needs_nothing() {
    let s = scratch("g2");
    let d = s.path();
    write(d, "a.n", "x: 1\n");
    assert_eq!(oo(d, &["evolve", "a.n"]).1, 0);
    let (out, rc) = oo(d, &["commit", "-m", "plain"]);
    assert_eq!(rc, 0, "an ordinary commit lands: {out}");
    let (log, _) = oo(d, &["log"]);
    assert!(
        !log.contains("privileged_effect"),
        "and carries no privileged mark; log said:\n{log}"
    );
}

// ---------------------------------------------------------------------
// G3. Presenting the capability still works, which is the point of the
// operation surviving the change.
// ---------------------------------------------------------------------
#[test]
fn g3_presenting_the_capability_still_lands() {
    let s = scratch("g3");
    let d = s.path();
    discharged(d, IO, "effect_override:io");
    let (out, rc) = oo(d, &["commit", "--grant", "effect_override:io", "-m", "ok"]);
    assert_eq!(rc, 0, "the capability was presented; commit said:\n{out}");
    let (log, _) = oo(d, &["log"]);
    assert!(
        log.contains("privileged_effect"),
        "and the commit is marked; log said:\n{log}"
    );
}
