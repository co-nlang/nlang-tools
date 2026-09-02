// An overwrite that was a coin flip.
// Ruling: nlang-spec/meta/oo/STATUS.md D58 (drawing: commit.md 1.7.11)
// Order:  nlang-tools/docs/an_overwrite_that_was_a_coin_flip_handover.md
//
// -- What this arc is ----------------------------------------------------
//
// Two commands, one writer, no concurrency at all:
//
//     oo evolve --pin --grant pin  (x: 2)
//     oo evolve --pin --grant pin  (x: 3)
//     oo status  ->  x: 2
//
// The later privileged overwrite is discarded. Measured 2026-09-02 on the
// v0.42.0 tag binary: over 20 trials the later pin won 12 times and the
// earlier one won 8, and 20/20 agreed with "the injection whose random
// 32-hex filename sorts last wins".
//
// -- Why it happens ------------------------------------------------------
//
// D48 made the working set a SET of immutable injections, folded with
// meet. Meet is associative, commutative and idempotent, so folding a set
// is well defined -- order cannot matter. `pin` is not meet: it is
// replace, which is neither. Folding a set with replace is not defined at
// all, and the engine does it anyway (universe.rs load_staged, under
// `pin_pending`), so the missing order is supplied by `paths()` sorting
// 16 bytes of OS entropy.
//
// The engine says so itself, in a comment at that very line: "Q-016 owns
// the order of concurrent pins". It is not only concurrent. It is two
// sequential commands.
//
// -- The ruling (D58, candidate B) ---------------------------------------
//
// A pin absorbs the existing injections AT THAT COORDINATE when it is
// evolved. The non-commutative step goes back to a place that already has
// an order -- an evolve is a command, and it happens after whatever is
// already on disk. The set stays a set and the fold stays meet.
//
// Two independent witnesses predate today's measurement:
//   * commit.md 1.1.6 (3) already refused candidate A in 2026-08-04.
//     LWW-Register really can implement pin's effect as a pure CRDT; the
//     price is trading "who has the right" for "who was later", and n/
//     declines to pay it. Ordering the injections is that road.
//   * A lock (candidate C) does nothing here. The defect is sequential.
//
// Today is worse than LWW, not better: both operands hold the pin
// capability, so "who has the right" cannot separate them; the set has no
// order, so "who was later" is unavailable. Both adjudicators come up
// empty and the winner is decided by entropy.
//
// -- Why the probes rename injection files -------------------------------
//
// The natural defect is a coin flip, and a red that is only red four
// times in five is not a nail. So R1-R3 rename the two injections to
// force each of the two possible sort orders. That does not construct an
// impossible state -- any pair of random ids is possible, and these are
// the two orders the engine already produces at random. It makes a
// 50% red into a 100% red.
//
// NOT PROBED, stated so no one mistakes silence for coverage:
//   * Two CONCURRENT pins on the SAME coordinate falling back to D49 /
//     SPEC_10 2.2.2 (both kept, bottom reported at that coordinate,
//     non-zero exit). That is a completion condition of D58 (iv). It is a
//     race, so a probe of it is statistical, and section 4 of the order
//     asks for the measurement in writing instead.
//   * R4 below IS concurrent and therefore statistical, not a nail. It
//     runs the pair eight times and requires all eight to hold. Said out
//     loud because the standing rule is that a probe which is red four
//     times in five is not a pin.

use std::path::Path;
use std::process::Command;

fn cmd(dir: &Path) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_oo"));
    c.current_dir(dir)
        .env("OO_IDENTITY", dir.join("identity-for-tests"))
        .env("OO_NODE_HOME", dir.join("node-home-for-tests"));
    c
}

fn oo(dir: &Path, args: &[&str]) -> String {
    let o = cmd(dir).args(args).output().expect("oo runs");
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

fn oo_ok(dir: &Path, args: &[&str]) -> bool {
    cmd(dir)
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .expect("oo runs")
        .success()
}

fn scratch(tag: &str) -> nlang_interpreter::ScratchDir {
    nlang_interpreter::ScratchDir::new(&format!("coinflip-{tag}"))
}

fn write(d: &Path, name: &str, body: &str) {
    std::fs::write(d.join(name), body).expect("write source");
}

/// Injection filenames, sorted the way the engine sorts them. A repo
/// that has never had a successful evolve has no such directory, and
/// that is zero injections, not a broken probe.
fn injections(d: &Path) -> Vec<String> {
    let dir = d.join(".oo").join("injections");
    let Ok(rd) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out: Vec<String> = rd
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .filter(|n| !n.starts_with('.'))
        .collect();
    out.sort();
    out
}

/// Rename the two injections so `first` sorts first (`adverse == false`)
/// or last (`adverse == true`). Only the filename moves; the bytes do not.
fn force_order(d: &Path, first: &str, second: &str, adverse: bool) {
    let dir = d.join(".oo").join("injections");
    let low = "0".repeat(32);
    let high = "f".repeat(32);
    let (a, b) = if adverse {
        (&high, &low)
    } else {
        (&low, &high)
    };
    // Two hops so a rename never lands on a name still in use.
    std::fs::rename(dir.join(first), dir.join("tmp-first")).expect("stash first");
    std::fs::rename(dir.join(second), dir.join(b.as_str())).expect("place second");
    std::fs::rename(dir.join("tmp-first"), dir.join(a.as_str())).expect("place first");
}

/// A repo with `base` committed, then two pins. Returns (first, second)
/// injection filenames in the order they were minted.
fn two_pins(d: &Path, base: &str, first_src: &str, second_src: &str) -> (String, String) {
    write(d, "base.n", base);
    assert!(oo_ok(d, &["evolve", "base.n"]), "base evolve");
    assert!(oo_ok(d, &["commit", "-m", "base"]), "base commit");

    write(d, "p1.n", first_src);
    assert!(
        oo_ok(d, &["evolve", "--pin", "--grant", "pin", "p1.n"]),
        "first pin"
    );
    let after_first = injections(d);
    assert_eq!(after_first.len(), 1, "one injection after the first pin");
    let first = after_first[0].clone();

    write(d, "p2.n", second_src);
    assert!(
        oo_ok(d, &["evolve", "--pin", "--grant", "pin", "p2.n"]),
        "second pin"
    );
    let after_second = injections(d);
    assert_eq!(after_second.len(), 2, "two injections after the second pin");
    let second = after_second
        .into_iter()
        .find(|n| *n != first)
        .expect("the second injection");
    (first, second)
}

fn staged_block(out: &str) -> String {
    // Everything from the staged header to the entropy line; enough to
    // compare two readings without pinning the surrounding chrome.
    let start = out.find("Staged changes:").unwrap_or(0);
    let rest = &out[start..];
    let end = rest.find("Total Logical Entropy").unwrap_or(rest.len());
    rest[..end].trim().to_string()
}

// ---------------------------------------------------------------------
// R1. The invariant D58 names: for any coordinate, the value of the
// working set must not depend on the order the injections are walked in.
// Nothing about pin, nothing about which value should win -- only that
// the two orders agree. Today they disagree, deterministically.
// ---------------------------------------------------------------------
#[test]
fn r1_traversal_order_must_not_decide_the_answer() {
    let s = scratch("r1");
    let d = s.path();
    let (first, second) = two_pins(d, "x: 1\n", "x: 2\n", "x: 3\n");

    force_order(d, &first, &second, false);
    let natural = staged_block(&oo(d, &["status"]));
    // The names are now fixed, so read them back rather than reusing the
    // originals.
    let names = injections(d);
    let low = names[0].clone();
    let high = names[1].clone();
    force_order(d, &low, &high, true);
    let adverse = staged_block(&oo(d, &["status"]));

    assert_eq!(
        natural, adverse,
        "the working set changed when the injections were renamed. \
         Two immutable injections, same bytes, different filenames, and \
         the answer moved:\n--- one order ---\n{natural}\n--- the other \
         ---\n{adverse}"
    );
}

// ---------------------------------------------------------------------
// R2. The operator's second pin is the one they meant. Forced into the
// adverse order so the coin flip cannot make this accidentally green.
// ---------------------------------------------------------------------
#[test]
fn r2_the_later_pin_wins() {
    let s = scratch("r2");
    let d = s.path();
    let (first, second) = two_pins(d, "x: 1\n", "x: 2\n", "x: 3\n");
    force_order(d, &first, &second, true);

    let out = oo(d, &["status"]);
    assert!(
        out.contains("x: 3"),
        "the second pin is what the operator last said; status said:\n{out}"
    );
}

// ---------------------------------------------------------------------
// R3. Absorption is by coordinate, not by file. One injection can carry
// several coordinates; a pin that swallows the whole earlier FILE takes
// `y` down with it. `x: 3` is the red today (adverse order gives `x: 2`);
// `y: 7` is the guard against fixing this the file-shaped way.
// ---------------------------------------------------------------------
#[test]
fn r3_absorption_is_by_coordinate_not_by_file() {
    let s = scratch("r3");
    let d = s.path();
    let (first, second) = two_pins(d, "x: 1\ny: 1\n", "x: 2\ny: 7\n", "x: 3\n");
    force_order(d, &first, &second, true);

    let out = oo(d, &["status"]);
    assert!(
        out.contains("x: 3"),
        "the later pin owns the coordinate it names; status said:\n{out}"
    );
    assert!(
        out.contains("y: 7"),
        "the earlier pin's OTHER coordinate was never overwritten and must \
         survive -- absorbing the whole file takes it down; status said:\n{out}"
    );
}

// ---------------------------------------------------------------------
// R4. STATISTICAL, not a nail (eight rounds; see the header).
//
// The privileged intent lives in `.oo/pin_pending`, a single shared cell
// rewritten whole -- exactly the shape D48 removed for the working set.
// Two concurrent pins on DIFFERENT coordinates leave two injections and
// one coordinate in the cell, so the other silently loses replace
// semantics and the commit refuses. Measured 2026-09-02: 5/5 rounds gave
// a one-coordinate cell and `Error: Commit failed`, rc 1, with no
// coordinate and no cause.
//
// The invariant: the intent must travel with the value it modifies, in
// the same immutable object. This probe does not say where that is.
// ---------------------------------------------------------------------
#[test]
fn r4_privileged_intent_must_not_live_in_a_shared_cell() {
    for round in 0..8 {
        let s = scratch(&format!("r4-{round}"));
        let d = s.path();
        write(d, "base.n", "x: 1\ny: 1\n");
        assert!(oo_ok(d, &["evolve", "base.n"]), "base evolve");
        assert!(oo_ok(d, &["commit", "-m", "base"]), "base commit");
        write(d, "px.n", "x: 9\n");
        write(d, "py.n", "y: 9\n");

        let a = cmd(d)
            .args(["evolve", "--pin", "--grant", "pin", "px.n"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("spawn x");
        let b = cmd(d)
            .args(["evolve", "--pin", "--grant", "pin", "py.n"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("spawn y");
        for mut p in [a, b] {
            p.wait().expect("pin exits");
        }

        let n = injections(d).len();
        assert_eq!(n, 2, "round {round}: both pins minted an injection");

        let out = oo(d, &["commit", "--grant", "pin", "-m", "both"]);
        let ok = oo_ok(d, &["log"]);
        assert!(ok, "round {round}: log readable");
        assert!(
            !out.contains("Commit failed"),
            "round {round}: a pin lost its privilege because the shared \
             cell was overwritten by the other one; commit said:\n{out}"
        );
        let root = oo(d, &["status"]);
        assert!(
            root.contains("Universe is static"),
            "round {round}: the commit consumed the working set; \
             status said:\n{root}"
        );
    }
}

// ---------------------------------------------------------------------
// G1. Identity is a red line. This arc changes how injections combine,
// not what anything is.
// ---------------------------------------------------------------------
#[test]
fn g1_identity_is_a_red_line() {
    let s = scratch("g1");
    let d = s.path();
    write(d, "a.n", "x: 0\n");
    assert!(oo_ok(d, &["evolve", "a.n"]));
    assert!(oo_ok(d, &["commit", "-m", "identity"]));

    let objects = walkdir(&d.join(".oo").join("objects"));
    assert_eq!(objects, 3, "a solid `x: 0` universe is three objects");

    let out = oo(d, &["status"]);
    assert!(
        out.contains("7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911"),
        "the standard root digest must not move; status said:\n{out}"
    );
}

// ---------------------------------------------------------------------
// G2. The ordinary path is untouched. Two conflicting unprivileged
// evolves are still refused AT THE EVOLVE BOUNDARY, naming the
// coordinate, and neither writes an injection. This is the control group
// that proves the order sensitivity belongs to pin: without pin, two
// conflicting injections never coexist in the first place.
// ---------------------------------------------------------------------
#[test]
fn g2_the_ordinary_path_still_refuses_at_the_evolve_boundary() {
    let s = scratch("g2");
    let d = s.path();
    write(d, "base.n", "x: 1\n");
    assert!(oo_ok(d, &["evolve", "base.n"]));
    assert!(oo_ok(d, &["commit", "-m", "base"]));

    write(d, "b.n", "x: 2\n");
    write(d, "c.n", "x: 3\n");
    let b = oo(d, &["evolve", "b.n"]);
    assert!(
        b.contains("#conflict at x"),
        "the evolve boundary names the coordinate; it said:\n{b}"
    );
    assert!(!oo_ok(d, &["evolve", "c.n"]), "the second is refused too");
    assert_eq!(
        injections(d).len(),
        0,
        "a refused evolve writes no injection"
    );
}

// ---------------------------------------------------------------------
// G3. A single pin still overwrites the root. The point of the operation
// survives the change to how injections combine.
// ---------------------------------------------------------------------
#[test]
fn g3_a_single_pin_still_overwrites() {
    let s = scratch("g3");
    let d = s.path();
    write(d, "base.n", "x: 1\n");
    assert!(oo_ok(d, &["evolve", "base.n"]));
    assert!(oo_ok(d, &["commit", "-m", "base"]));

    write(d, "p.n", "x: 2\n");
    assert!(oo_ok(d, &["evolve", "--pin", "--grant", "pin", "p.n"]));
    assert!(
        oo_ok(d, &["commit", "--grant", "pin", "-m", "pinned"]),
        "a pinned commit lands"
    );
    let log = oo(d, &["log"]);
    assert!(
        log.contains("pin"),
        "the commit is marked as a pin; log said:\n{log}"
    );
}

/// Every regular file under `dir`, recursively.
fn walkdir(dir: &Path) -> usize {
    let mut n = 0;
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                n += walkdir(&p);
            } else {
                n += 1;
            }
        }
    }
    n
}
