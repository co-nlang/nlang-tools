// A store you did not write (Q-011, pre-committed by work order:
// docs/a_store_you_did_not_write_handover.md).
//
// ── The claim ────────────────────────────────────────────────────────────
//
// A marker may only declare the thing it actually measures, and an engine
// must never write down a declaration it has not verified.
//
// `.oo/format` was introduced (2026-07-28, local_gc) as a LAYOUT marker:
// which files exist under `.oo/` and what they mean. It was then turned twice
// for something else — Q-010a (spans out of CAS objects) and Q-010b (the root
// names the standard root by digest). Measured: the layout of a format-2 and
// a format-3 store is file-for-file identical. The layout is still 1.
//
// ── What that cost, measured end to end ──────────────────────────────────
//
// Six steps, every component behaving as designed, no error at any step:
//
//   1. a format-3 store is missing `.oo/format`
//   2. a READ-ONLY `oo log` stamps `1` on it                    (false)
//   3. the format gate is defeated: v0.20.0 opens it
//   4. v0.20.0 calls the healthy root corrupt                   (false)
//   5. v0.20.0 WRITES: commit lands, HEAD moves, label becomes 2
//   6. a legitimate `oo gc --grant gc` collects the original root and commit
//
// Step 6 is privileged and never automatic (local_gc arc), and is NOT in
// scope. The engine did not delete history; it built a state in which a
// LEGITIMATE collection deletes history. The fix is steps 1–5.
//
// ── Why a CAS object cannot declare its own encoding ─────────────────────
//
// Measured: the address is not the hash of the file (path `b7741081…` vs the
// file's own sha256 `350c42c7…`). `content_hash()` goes through `bn_serial`;
// the JSON on disk is a container. Encoding and identity are orthogonal. So a
// self-describing version field would either enter the hash (putting
// something identity-irrelevant into identity) or not (violating REAL_03 §6.7
// outright, whose precedent is `span`). The container declares it instead —
// as `objects/sha256/` already declares the hash algorithm.
//
// The line, and C depends on it: an object MAY name what it depends on (that
// is content — the root's standard-root digest is a function of the value),
// and MAY NOT declare how it is encoded (that is the container's metadata).
//
// ── Probe integrity ──────────────────────────────────────────────────────
//
// Reds are `#[ignore]`d and MUST be red at the baseline for the reason stated
// in each. The delivery may remove `#[ignore]` and nothing else in this file.
// C0 runs first: an assertion about what a store does NOT contain is vacuous
// if the store was never built.

use std::path::Path;
use std::process::Command;

fn fresh(tag: &str) -> nlang_interpreter::ScratchDir {
    let d = nlang_interpreter::ScratchDir::new(&format!("notyours-{tag}"));
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

fn committed(tag: &str, src: &str) -> nlang_interpreter::ScratchDir {
    let d = fresh(tag);
    std::fs::write(d.join("u.n"), src).unwrap();
    oo(&d, &["evolve", "u.n"]);
    oo(&d, &["commit", "-m", "probe"]);
    d
}

const SRC: &str = "app: { k1: 1 + 2, v: 10 }\n";

fn layout_file(d: &Path) -> std::path::PathBuf {
    d.join(".oo").join("format")
}
fn encoding_file(d: &Path) -> std::path::PathBuf {
    d.join(".oo").join("objects.format")
}
fn read(p: &std::path::PathBuf) -> Option<String> {
    std::fs::read_to_string(p).ok().map(|s| s.trim().to_string())
}

/// Every file under `.oo/`, relative, with CAS object paths collapsed.
/// Never truncated.
fn oo_files(dir: &Path) -> Vec<String> {
    fn walk(p: &Path, base: &Path, out: &mut Vec<String>) {
        let Ok(rd) = std::fs::read_dir(p) else { return };
        for e in rd.flatten() {
            let path = e.path();
            if path.is_dir() {
                walk(&path, base, out);
            } else if let Ok(rel) = path.strip_prefix(base) {
                out.push(rel.to_string_lossy().to_string());
            }
        }
    }
    let mut out = Vec::new();
    walk(&dir.join(".oo"), dir, &mut out);
    let re_collapse = |s: &String| -> String {
        if s.starts_with(".oo/objects/sha256/") {
            ".oo/objects/sha256/<CAS>".to_string()
        } else {
            s.clone()
        }
    };
    let mut v: Vec<String> = out.iter().map(re_collapse).collect();
    v.sort();
    v.dedup();
    v
}

// ── C0 ── the shelf is not empty ─────────────────────────────────────────
// FIRST ON PURPOSE. R2/R3 assert that something is ABSENT or REFUSED; all of
// them pass vacuously against a store that was never built.
#[test]
fn c0_the_store_is_really_built() {
    let d = committed("c0", SRC);
    let files = oo_files(&d);
    assert!(
        files.iter().any(|f| f.starts_with(".oo/objects/sha256/")),
        "no CAS objects at all — every assertion below would be vacuous: {files:#?}"
    );
    assert!(
        files.iter().any(|f| f == ".oo/HEAD"),
        "no HEAD — this is not a committed store: {files:#?}"
    );
}

// ── C1 ── an ordinary store still works end to end ───────────────────────
#[test]
fn c1_a_normal_store_opens_and_reads() {
    let d = committed("c1", SRC);
    let out = oo(&d, &["log"]);
    assert!(
        out.contains("commit hash:") && !out.to_lowercase().contains("error"),
        "a store this engine just wrote does not open: {out}"
    );
}

// ── C2 ── the honest refusals that already work must keep working ────────
// These are the cells of the M3 matrix that are already right. If a delivery
// tightening the absent case also breaks these, it has traded one silence for
// another.
#[test]
fn c2_unknown_declarations_are_still_refused_by_name() {
    for bad in ["0", "999", "abc", ""] {
        let d = committed(&format!("c2-{}", bad.len()), SRC);
        std::fs::write(layout_file(&d), format!("{bad}\n")).unwrap();
        let out = oo(&d, &["log"]);
        assert!(
            out.to_lowercase().contains("refus") || out.to_lowercase().contains("not supported"),
            "declaration {bad:?} was not refused: {out}"
        );
    }
}

// ── C3 ── gc is untouched by this arc ────────────────────────────────────
// Step 6 of the damage chain is a legitimate collection and stays legitimate.
#[test]
fn c3_gc_on_a_healthy_store_still_says_nothing_is_wrong() {
    let d = committed("c3", SRC);
    let out = oo(&d, &["gc", "--grant", "gc"]);
    assert!(
        !out.contains("integrity") && !out.to_lowercase().contains("refus"),
        "gc accused a healthy store — REAL_03 §6.6, and not this arc's doing: {out}"
    );
}

// ── P1 ── the layout really has not changed, and this arc may move it once
// The whole ruling rests on this: formats 2 and 3 have the same file set. This
// pin records the CURRENT set so that the delivery's one intended addition
// (`.oo/objects.format`) is a visible, deliberate act rather than drift.
#[test]
fn p1_the_layout_is_a_short_and_known_list() {
    let d = committed("p1", SRC);
    let files = oo_files(&d);
    // ACCEPTOR (Q-011, 2026-08-14): `.oo/objects.format` added. This pin fired
    // exactly as its own comment predicted, and the work order still failed to
    // list it among the scheduled changes — the same omission as the three
    // other `.oo/` file-set assertions in the tree, which §7.6 waved at with
    // "if any" instead of grepping. A pin written to fire on schedule is still
    // a scheduled change and belongs in the work order.
    let expected = vec![
        ".oo/HEAD".to_string(),
        ".oo/format".to_string(),          // layout axis
        ".oo/objects.format".to_string(),  // object-encoding axis (O23)
        ".oo/objects/sha256/<CAS>".to_string(),
    ];
    let extra: Vec<&String> = files.iter().filter(|f| !expected.contains(f)).collect();
    assert!(
        extra.is_empty(),
        "the `.oo/` layout gained a file nobody declared. Adding a file here \
         IS a layout change and must be named in a work order: {extra:#?}"
    );
}

// ── P2 ── the address is not the hash of the file ────────────────────────
// The load-bearing measurement behind O23. If this ever becomes false, a CAS
// object COULD carry its own encoding tag, and the whole ruling is rederivable
// the other way.
#[test]
fn p2_the_address_is_not_the_hash_of_the_file() {
    let d = committed("p2", SRC);
    let mut checked = 0;
    for f in oo_files(&d) {
        if !f.starts_with(".oo/objects/sha256/") {
            continue;
        }
        // Re-walk for the real paths (oo_files collapses them).
        let sha = d.join(".oo").join("objects").join("sha256");
        for a in std::fs::read_dir(&sha).unwrap().flatten() {
            if !a.path().is_dir() {
                continue;
            }
            let pre = a.file_name().to_string_lossy().to_string();
            for b in std::fs::read_dir(a.path()).unwrap().flatten() {
                let addr = format!("{pre}{}", b.file_name().to_string_lossy());
                let bytes = std::fs::read(b.path()).unwrap();
                let file_hash = hex::encode(
                    ring::digest::digest(&ring::digest::SHA256, &bytes).as_ref(),
                );
                assert_ne!(
                    addr, file_hash,
                    "the address IS the hash of the file. Encoding would then \
                     be part of identity, and O23's reasoning has to be redone"
                );
                checked += 1;
            }
        }
        break;
    }
    assert!(checked >= 2, "checked only {checked} objects — see C0");
}

// ── R1 ── a read-only command must not write a declaration ───────────────
// Step 2 of the damage chain. Includes the existence half: the command has to
// SUCCEED at nothing (a refusal), not fail to run at all.
#[test]
fn r1_a_read_only_command_writes_nothing() {
    let d = committed("r1", SRC);
    let before = read(&layout_file(&d)).expect("a fresh store must declare something");
    std::fs::remove_file(layout_file(&d)).unwrap();

    let out = oo(&d, &["log"]);

    assert!(
        read(&layout_file(&d)).is_none(),
        "a READ-ONLY command wrote `{}` into a store it did not write. The \
         store was {before} before the file was removed — the engine guessed, \
         and it guessed a value it had not verified (O53)",
        read(&layout_file(&d)).unwrap_or_default()
    );
    // Existence half: it must refuse, and say so. A silent success would mean
    // the engine simply ignored the missing declaration.
    assert!(
        out.to_lowercase().contains("refus") || out.to_lowercase().contains("cannot"),
        "no declaration is not the same as a declaration of 1 — the engine \
         must say it cannot tell what this store is. Got: {out}"
    );
}

// ── R2 ── the two axes are declared separately ───────────────────────────
// Includes the existence half: the layout file must still be there and still
// say something, or "the encoding is not in it" would be true of a deleted
// file.
#[test]
fn r2_layout_and_encoding_are_two_declarations() {
    let d = committed("r2", SRC);

    let layout = read(&layout_file(&d)).expect("the layout declaration must exist");
    assert!(
        layout.contains("layout"),
        "`.oo/format` still holds a bare number ({layout:?}). It must declare \
         what it measures — a bare number is exactly what let one counter be \
         turned twice for something it was not measuring"
    );

    let encoding = read(&encoding_file(&d)).unwrap_or_else(|| {
        panic!(
            "`.oo/objects.format` does not exist. The object encoding needs a \
             declaration of its own, and it cannot live inside a CAS object \
             (see P2) nor inside `.oo/objects/` (five suites read every file \
             there as an object)"
        )
    });
    assert!(
        encoding.contains("encoding"),
        "the encoding declaration does not say what it measures: {encoding:?}"
    );
}

// ── R3 ── a legacy store is read by its old rule, explicitly ─────────────
//
// CALIBRATION NOTE (2026-08-14). The first draft wrote a bare `3` and checked
// that it still opened — and it was GREEN at the baseline, because a bare `3`
// IS the current scheme. It asserted nothing: it would have passed whether or
// not a legacy rule existed. A red that cannot be red is not a gate.
//
// The property has two halves and both are needed. A fresh store must write
// the NEW self-describing form (otherwise "bare number" names nothing and
// there is no legacy to speak of), AND a bare number must still open and must
// not be rewritten by the reading. The second half is the existence half: it
// stops the whole thing being "satisfied" by refusing every legacy store.
#[test]
fn r3_a_bare_number_is_read_as_the_old_conflated_counter() {
    // Half one: the new form must be distinguishable from the old.
    let fresh_store = committed("r3-fresh", SRC);
    let fresh_decl = read(&layout_file(&fresh_store)).expect("a fresh store declares something");
    assert!(
        fresh_decl.parse::<u32>().is_err(),
        "a fresh store still writes the bare number {fresh_decl:?}. Then a bare          number cannot mean \"written by the old scheme\", and the legacy rule          has nothing to key on"
    );

    // Half two: enter the legacy route before any value is written. Relabelling
    // encoding-4 bytes after the fact would not construct an old store; it
    // would construct a lying container and a CAID mismatch. The non-CAS
    // anchor makes `init` treat this as an existing declared container, then
    // the ordinary CLI writes its first commit through encoding 3.
    let d = fresh("r3-legacy");
    std::fs::create_dir_all(d.join(".oo").join("objects")).unwrap();
    std::fs::write(layout_file(&d), "3\n").unwrap();
    let _ = std::fs::remove_file(encoding_file(&d));
    std::fs::write(d.join(".oo").join("objects").join(".legacy-fixture-anchor"), b"")
        .unwrap();
    std::fs::write(d.join("u.n"), SRC).unwrap();
    oo(&d, &["evolve", "u.n"]);
    let commit = oo(&d, &["commit", "-m", "legacy probe"]);
    assert!(
        commit.contains("Commit successful"),
        "the legacy write path could not create its fixture: {commit}"
    );

    // O73 (Q-038): `commit` must not rewrite a layout it found. Splitting
    // the conflated counter into `layout=N` + `objects.format` is `oo migrate`,
    // never a side effect of writing a root. Encoding 3 is retained as the
    // bare number the store already declared.
    assert_eq!(
        read(&layout_file(&d)).as_deref(),
        Some("3"),
        "commit rewrote a pre-split layout declaration"
    );
    assert_eq!(
        read(&encoding_file(&d)).as_deref(),
        None,
        "commit added objects.format to a store that did not declare an encoding axis"
    );
    let declaration_before_read = read(&layout_file(&d));
    let out = oo(&d, &["log"]);
    assert!(
        out.contains("commit hash:") && !out.to_lowercase().contains("error"),
        "a store written by v0.21.0 (bare `3`) no longer opens. Reading legacy \
stores is not optional — every store in existence today is one: {out}"
    );
    assert_eq!(
        read(&layout_file(&d)),
        declaration_before_read,
        "reading a legacy store rewrote its declaration. Reading is not \
migrating (O53)"
    );
}

// ── R4 ── DELIBERATELY ABSENT ────────────────────────────────────────────
//
// O54 (hydrate only roots that name a standard root) has no black-box probe
// in this tree, and saying so is better than shipping a gate that cannot fail
// for the right reason.
//
// The defect is invisible while the engine's standard root equals the one the
// store was written against — which is always true inside one build. Telling
// "hydrated" from "left alone" needs two engines whose standard roots DIFFER,
// and a test cannot build a second standard root.
//
// Measured with three real binaries (v0.20.0, v0.21.0, and v0.21.0 plus one
// extra standard-root entry):
//
//   format-2 store, read by v0.20.0+one   → opens             (old path is safe)
//   format-3 store, read by v0.21.0+one   → refused BY NAME   (correct, §6.8)
//   format-2 store, read by v0.21.0+one   → "#caid_mismatch:
//                                            object ... is corrupt"  ← the bug
//   format-2 store, read by v0.21.0       → opens             (control)
//
// So O54 is verified at ACCEPTANCE, by rebuilding that third binary, not by a
// probe here. The work order §7 carries it as a required acceptance step.
// (Baselines live in /home/gali/nlang-baselines — never /tmp, which a crash
// wipes.)
