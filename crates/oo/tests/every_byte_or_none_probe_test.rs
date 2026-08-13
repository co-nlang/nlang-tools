// Every byte or none (Q-010a, pre-committed by work order:
// docs/every_byte_or_none_handover.md).
//
// ── The claim ────────────────────────────────────────────────────────────
//
// O48 (ruled 2026-08-13): every byte in a CAS object either enters the hash,
// or has no business being on disk.
//
// Its testable form is a bijection. The engine already has one direction —
// bytes determine the address, and a valid semantic tamper is caught with
// `#caid_mismatch` (C1). The other direction is missing: the address does NOT
// determine the bytes.
//
// ── What is on the floor (measured 2026-08-13, engine v0.19.0) ───────────
//
//   A — NON-DETERMINISTIC KEY ORDER. Three independent runs of BYTE-IDENTICAL
//       source produce the same address and three different files:
//
//         run 1  addr ba56831a…  bytes 84db4cfb…   252435 B
//         run 2  addr ba56831a…  bytes b23f5929…   252435 B
//         run 3  addr ba56831a…  bytes b9200dc9…   252435 B
//
//       Same size, and the sorted line multisets are identical — it is pure
//       reordering. `system`'s top-level key order differs per process:
//       run 1 `[String, Effect, Config, …]`, run 2 `[Process, Csv, Cond, …]`.
//       The hash path is already deterministic (the address never moved); it
//       is the SERIALIZATION path that is not.
//
//   B — `span` IS STORED AND NOT HASHED. Two programs differing only in
//       whitespace store different spans under the SAME address:
//
//         `app: { k1: 1 }`    span {11,12}   addr 16ba5683…
//         `app: {   k1: 1 }`  span {13,14}   addr 16ba5683…   ← same address
//
//       Tampering with the span digits on disk is NOT detected: `oo inspect`
//       serves the object and returns `k1: 1` with no warning. Tampering with
//       the integer (valid encoding, `[1,[1]]` → `[1,[2]]`) IS detected. So
//       the check exists; its coverage is a proper subset of the bytes.
//
//   C — PRETTY-PRINTING. The object is written as pretty JSON: 252,435 B on
//       disk against 67,971 B compact. ~184 KB of indentation that no hash
//       covers.
//
//   Together A + B account for ALL of the divergence: after sorting away the
//   ordering noise, the only remaining difference between the two whitespace
//   variants is 8 lines, all of them `"start"`/`"end"`. Non-span differences
//   after sort: 0. That is why R2 and R3 are both satisfiable.
//
// ── Why this arc does not move any identity ──────────────────────────────
//
//   span   does NOT enter the hash — tampering with it goes undetected ⟹
//          removing it cannot move a CAID.  (P1 pins this.)
//   order  does NOT enter the hash — the address was stable across three runs
//          whose byte order differed ⟹ canonicalising it cannot move a CAID.
//   The closure DOES enter the hash (emptying it or adding a key both give
//          `#caid_mismatch`) ⟹ it is NOT in this arc. It rides with the
//          forcing change in Q-010b, because we do not get two epochs.
//
// ── Scope fence ──────────────────────────────────────────────────────────
//
// O35/O48/O49 govern CAS OBJECTS. `.oo/staged` is not one: it is durable
// today but has no address, and per O51 it KEEPS its Thunks. Forcing happens
// at commit, not at evolve. P2 exists so that a delivery reaching for the
// shared serializer cannot quietly take the laziness out of the working set.
//
// ── Probe integrity ──────────────────────────────────────────────────────
//
// Reds are `#[ignore]`d and MUST be red at the baseline for the reason stated
// in each. The delivery may remove `#[ignore]` and nothing else in this file.
// C0 runs first on purpose: every "X is absent" assertion below is vacuous if
// the scan finds no objects at all.

use std::path::Path;
use std::process::Command;

fn fresh(tag: &str) -> nlang_interpreter::ScratchDir {
    let d = nlang_interpreter::ScratchDir::new(&format!("everybyte-{tag}"));
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

/// Evolve `src` and commit it in a fresh repo. Returns the repo.
fn committed(tag: &str, src: &str) -> nlang_interpreter::ScratchDir {
    let d = fresh(tag);
    std::fs::write(d.join("u.n"), src).unwrap();
    oo(&d, &["evolve", "u.n"]);
    oo(&d, &["commit", "-m", "probe"]);
    d
}

/// Every file under `.oo/objects`, as (path, bytes). Never truncated.
fn objects(dir: &Path) -> Vec<(std::path::PathBuf, Vec<u8>)> {
    fn walk(p: &Path, out: &mut Vec<(std::path::PathBuf, Vec<u8>)>) {
        let Ok(rd) = std::fs::read_dir(p) else { return };
        for e in rd.flatten() {
            let path = e.path();
            if path.is_dir() {
                walk(&path, out);
            } else if let Ok(b) = std::fs::read(&path) {
                out.push((path, b));
            }
        }
    }
    let mut out = Vec::new();
    walk(&dir.join(".oo").join("objects"), &mut out);
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// The largest object — the root combo, as opposed to the commit record.
fn root_object(dir: &Path) -> (std::path::PathBuf, Vec<u8>) {
    objects(dir)
        .into_iter()
        .max_by_key(|(_, b)| b.len())
        .expect("no objects at all — see C0")
}

/// `hash:sha256:v1:<dir><file>` reconstructed from the store path.
fn caid_of(path: &Path) -> String {
    let file = path.file_name().unwrap().to_string_lossy().to_string();
    let dir = path
        .parent()
        .unwrap()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();
    format!("hash:sha256:v1:{dir}{file}")
}

const PROGRAM: &str = "app: { k1: 1 }\n";
const PROGRAM_SPACED: &str = "app: {   k1: 1 }\n";

/// Byte offset of the first difference, or `None` if equal. Used instead of
/// `assert_eq!` on the raw buffers: these objects are ~250 KB and a failing
/// `assert_eq!` would print both copies.
fn first_difference(a: &[u8], b: &[u8]) -> Option<usize> {
    if a == b {
        return None;
    }
    Some(
        a.iter()
            .zip(b.iter())
            .position(|(x, y)| x != y)
            .unwrap_or(a.len().min(b.len())),
    )
}

/// 60 bytes of printable context around `off`.
fn excerpt(b: &[u8], off: usize) -> String {
    let lo = off.saturating_sub(30);
    let hi = (off + 30).min(b.len());
    String::from_utf8_lossy(&b[lo..hi]).replace('\n', "⏎")
}

// ── C0 ── the scan is not looking at an empty shelf ──────────────────────
// FIRST ON PURPOSE. R1/R4 assert that something is ABSENT from every object.
// If the walker silently fails, every one of them passes by finding nothing.
#[test]
fn c0_the_store_actually_has_objects() {
    let d = committed("c0", PROGRAM);
    let objs = objects(&d);
    assert!(
        objs.len() >= 2,
        "expected at least a root and a commit object, found {}",
        objs.len()
    );
    let (_, root) = root_object(&d);
    assert!(
        root.len() > 1000,
        "root object is {} bytes — too small to be the real root; the \
         absence assertions in R1/R4 would be vacuous",
        root.len()
    );
}

// ── C1 ── detection works, so R-side "not detected" means something ──────
// A payload that COMPUTES: `[1,[1]]` → `[1,[2]]` is a valid Int encoding
// (sign 1, digits [2]) and means 2. An invalid one (e.g. sign 2) would be
// stopped by serde as `#object_undecodable` and would prove only that
// deserialization is strict — the v0.2.50 rule, learned the hard way.
#[test]
fn c1_a_semantic_tamper_is_caught() {
    let d = committed("c1", PROGRAM);
    let (path, bytes) = root_object(&d);
    let text = String::from_utf8(bytes).unwrap();
    let tampered = text.replacen("\"Int\": [\n", "\"Int\": [\n", 1);
    let tampered = tamper_int_one_to_two(&tampered);
    assert_ne!(tampered, text, "the tamper did not apply — fixture drifted");
    std::fs::write(&path, &tampered).unwrap();

    let out = oo(&d, &["inspect", &caid_of(&path)]);
    assert!(
        out.contains("caid_mismatch"),
        "a semantic tamper went undetected — the integrity check itself is \
         broken, and every red below would be measuring nothing. Got: {out}"
    );
}

/// Replace the first `1` digit of an `Int` payload with `2`, keeping the
/// encoding valid. Works against both pretty and compact JSON.
fn tamper_int_one_to_two(s: &str) -> String {
    for pat in [
        "\"Int\": [\n          1,\n          [\n            1\n          ]",
        "\"Int\":[1,[1]]",
    ] {
        if let Some(i) = s.find(pat) {
            let mut out = s.to_string();
            let j = i + pat.rfind('1').unwrap();
            out.replace_range(j..j + 1, "2");
            return out;
        }
    }
    // Fall back: last `1` inside the first Int block.
    let i = s.find("\"Int\"").expect("no Int in the object — fixture drifted");
    let j = s[i..].find(']').unwrap() + i;
    let k = s[i..j].rfind('1').expect("no digit to flip") + i;
    let mut out = s.to_string();
    out.replace_range(k..k + 1, "2");
    out
}

// ── C2 ── the value still exists after all of this ───────────────────────
#[test]
fn c2_the_value_survives_a_round_trip() {
    let d = committed("c2", PROGRAM);
    let (path, _) = root_object(&d);
    let out = oo(&d, &["inspect", &caid_of(&path)]);
    assert!(
        out.contains("k1: 1"),
        "the committed value did not read back as `k1: 1`: {out}"
    );
}

// ── P1 ── this arc moves no identity ─────────────────────────────────────
// The whole reason Q-010a is separable from Q-010b. If this pin goes red,
// the delivery has touched the hashed projection and the arc has silently
// become epoch-level.
const ROOT_CAID: &str = "16ba56831ac26b8c4a9412840bbfc6eed7271f51f284c485c2e9c88f04db00d4";

#[test]
fn p1_the_root_caid_does_not_move() {
    let d = committed("p1", PROGRAM);
    let (path, _) = root_object(&d);
    assert_eq!(
        caid_of(&path),
        format!("hash:sha256:v1:{ROOT_CAID}"),
        "the root CAID for `{}` moved. Removing span and canonicalising key \
         order MUST NOT touch the hashed projection — if it did, this arc is \
         no longer non-breaking and cannot ship separately from Q-010b",
        PROGRAM.trim()
    );
}

// ── P2 ── the scope fence: staged is not a CAS object ────────────────────
// O51: staged keeps its Thunks; forcing happens at commit. A delivery that
// reaches for the shared serializer can take the laziness out of the working
// set without meaning to. It must not.
#[test]
fn p2_staged_still_keeps_its_thunks() {
    let d = fresh("p2");
    std::fs::write(d.join("u.n"), PROGRAM).unwrap();
    oo(&d, &["evolve", "u.n"]);

    let staged = std::fs::read_to_string(d.join(".oo").join("staged"))
        .expect("`.oo/staged` does not exist after evolve — the fixture for \
                 this pin is gone, not the property");
    assert!(
        staged.contains("Thunk"),
        "`.oo/staged` no longer holds a Thunk. O51 rules that the working set \
         stays lazy and that forcing happens at commit; O48 governs CAS \
         objects and `.oo/staged` is not one (it has no address). Content: {}",
        &staged[..staged.len().min(400)]
    );
}

// ── P3 ── no write path to the fields serde skips ────────────────────────
// M7: `legacy_fields`/`legacy_local` are `#[serde(skip)]` yet participate in
// the hash — anything written there produces an object that cannot be read
// back. Today nothing writes them. This arc touches the serde surface, so it
// is exactly when someone might.
//
// The existence half is not decoration: without it, a scan that matches
// nothing (renamed field, broken glob) passes while asserting nothing.
#[test]
fn p3_nothing_writes_the_skipped_fields() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let me = Path::new(file!()).file_name().unwrap();

    let mut read_sites = 0usize;
    let mut write_sites: Vec<String> = Vec::new();
    let mut scanned = 0usize;

    fn rs_files(p: &Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(rd) = std::fs::read_dir(p) else { return };
        for e in rd.flatten() {
            let path = e.path();
            if path.is_dir() {
                if path.file_name().is_some_and(|n| n == "target") {
                    continue;
                }
                rs_files(&path, out);
            } else if path.extension().is_some_and(|x| x == "rs") {
                out.push(path);
            }
        }
    }
    let mut files = Vec::new();
    rs_files(root, &mut files);

    for f in &files {
        if f.file_name() == Some(me) {
            continue; // a scan that reports itself reports its own prose
        }
        let Ok(src) = std::fs::read_to_string(f) else { continue };
        scanned += 1;
        for (n, line) in src.lines().enumerate() {
            for field in ["legacy_fields", "legacy_local"] {
                if !line.contains(field) {
                    continue;
                }
                read_sites += 1;
                let after = line.split(field).nth(1).unwrap_or("");
                let assigns = after.trim_start().starts_with('=')
                    && !after.trim_start().starts_with("==");
                if assigns {
                    write_sites.push(format!("{}:{}: {}", f.display(), n + 1, line.trim()));
                }
            }
        }
    }

    assert!(scanned > 50, "only {scanned} .rs files scanned — walker failed");
    assert!(
        read_sites > 0,
        "no mention of `legacy_fields`/`legacy_local` anywhere in {scanned} \
         files. Either they were renamed or the scan is broken; either way \
         the absence assertion below proves nothing"
    );
    assert!(
        write_sites.is_empty(),
        "something now writes a serde-skipped field that participates in the \
         hash (M7). Anything stored there becomes unreadable on the way back: \
         {write_sites:#?}"
    );
}

// ── C3 ── the refusal mechanism is live ──────────────────────────────────
// Added 2026-08-13 by the acceptor after model #4 correctly found that R1 and
// the original §6.5 could not both hold: `ast.rs` declares `pub span: Span` in
// four places with no serde attribute at all, so an object written without it
// is not readable by v0.19.0 — no compatibility shim can recover a field that
// is not there. The arc therefore bumps the store format, and this control
// exists so that R5 below is not measuring a gate that never fires.
#[test]
fn c3_an_unknown_store_format_is_refused_by_name() {
    let d = committed("c3", PROGRAM);
    std::fs::write(d.join(".oo").join("format"), "99\n").unwrap();

    let out = oo(&d, &["log"]);
    assert!(
        out.contains("99") && out.to_lowercase().contains("refus"),
        "an unknown store format was not refused with its version named. The \
         honest-refusal path is what makes a format bump better than letting \
         an old engine hit a serde error, and R5 depends on it. Got: {out}"
    );
}

// ── R1 ── no unhashed field on disk ──────────────────────────────────────
#[test]
#[ignore = "Q-010a: `span` is stored today; removing it is the delivery"]
fn r1_no_span_survives_into_a_cas_object() {
    let d = committed("r1", PROGRAM);
    let objs = objects(&d);
    assert!(!objs.is_empty(), "see C0");

    let mut with_span = Vec::new();
    let mut thunk_seen = false;
    for (p, b) in &objs {
        let s = String::from_utf8_lossy(b);
        if s.contains("\"span\"") {
            with_span.push(p.display().to_string());
        }
        if s.contains("Thunk") {
            thunk_seen = true;
        }
    }

    // The existence half. Q-010a does NOT force values, so a Thunk must still
    // be there afterwards — if it is not, the delivery has done Q-010b's job
    // and R1 would be passing for the wrong reason.
    assert!(
        thunk_seen,
        "no Thunk in any object. This arc does not force values (that is \
         Q-010b) — a store with no Thunks means the delivery overshot, and \
         `span` being gone would then prove nothing"
    );
    assert!(
        with_span.is_empty(),
        "`span` is on disk and is not covered by the hash — tampering with it \
         is not detected. Objects still carrying it: {with_span:#?}"
    );
}

// ── R2 ── the same value is the same file ────────────────────────────────
#[test]
#[ignore = "Q-010a: serialization key order is per-process today"]
fn r2_identical_source_gives_identical_bytes() {
    let a = committed("r2a", PROGRAM);
    let b = committed("r2b", PROGRAM);
    let c = committed("r2c", PROGRAM);

    let (pa, ba) = root_object(&a);
    let (pb, bb) = root_object(&b);
    let (pc, bc) = root_object(&c);

    assert_eq!(caid_of(&pa), caid_of(&pb), "addresses already disagree");
    assert_eq!(caid_of(&pa), caid_of(&pc), "addresses already disagree");
    assert_eq!(ba.len(), bb.len(), "sizes differ — not a pure reordering");

    assert!(
        ba == bb && ba == bc,
        "three runs of byte-identical source produced {} distinct files under \
         one address. The hash path is deterministic (the address never \
         moved); the serialization path is not — `system` iterates in \
         per-process order",
        [&ba, &bb, &bc]
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
    );
}

// ── R3 ── the same address is the same file ──────────────────────────────
#[test]
#[ignore = "Q-010a: spans differ under one address today"]
fn r3_whitespace_does_not_change_the_stored_bytes() {
    let a = committed("r3a", PROGRAM);
    let b = committed("r3b", PROGRAM_SPACED);

    let (pa, ba) = root_object(&a);
    let (pb, bb) = root_object(&b);

    // If the two do not even share an address, this probe is measuring the
    // wrong thing and must say so rather than fail as if it found the defect.
    assert_eq!(
        caid_of(&pa),
        caid_of(&pb),
        "the two programs no longer share a CAID; R3's premise is gone"
    );
    // NOT `assert_eq!(ba, bb)`: these are ~250 KB each and the failure would
    // print both, burying the finding under a megabyte of digits.
    if let Some(off) = first_difference(&ba, &bb) {
        panic!(
            "two programs differing only in whitespace store different bytes \
             under the SAME address — the address does not determine the \
             file.\n  first difference at byte {off} of {} / {}\n  a: …{}…\n  \
             b: …{}…",
            ba.len(),
            bb.len(),
            excerpt(&ba, off),
            excerpt(&bb, off)
        );
    }
}

// ── R4 ── no formatting bytes that no hash covers ────────────────────────
#[test]
#[ignore = "Q-010a: objects are written as pretty JSON today"]
fn r4_no_unhashed_formatting_on_disk() {
    let d = committed("r4", PROGRAM);
    let objs = objects(&d);
    assert!(!objs.is_empty(), "see C0");

    let mut offenders = Vec::new();
    for (p, b) in &objs {
        let newlines = b.iter().filter(|&&c| c == b'\n').count();
        if newlines > 1 {
            offenders.push(format!("{}: {newlines} newlines, {} bytes", p.display(), b.len()));
        }
    }
    assert!(
        offenders.is_empty(),
        "objects carry indentation no hash covers (measured at baseline: \
         252,435 B on disk against 67,971 B compact — ~184 KB of whitespace): \
         {offenders:#?}"
    );
}

// ── R5 ── the format break is declared, not stumbled into ────────────────
// Dropping `span` makes new objects unreadable by v0.19.0 — that is a fact
// about `ast.rs`, not a choice. The choice is whether an old engine meets it
// as an honest refusal (`.oo/format` says 2, "refusing to open") or as a serde
// error about a missing field it has never heard of. This arc takes the first.
//
// The IDENTITY is untouched — P1 still pins the same root CAID. Read
// compatibility and identity are two different axes, and the original work
// order conflated them.
#[test]
#[ignore = "Q-010a: the store format is still 1; the bump is the delivery"]
fn r5_the_store_format_says_it_changed() {
    let d = committed("r5", PROGRAM);
    let f = std::fs::read_to_string(d.join(".oo").join("format")).unwrap();
    assert_eq!(
        f.trim(),
        "2",
        "`.oo/format` still reads `{}`. New objects have no `span`, and \
         v0.19.0's `ast.rs` requires it — an engine that opens this store \
         without being told the format moved will fail on a missing field \
         instead of saying which version it cannot read",
        f.trim()
    );
}
