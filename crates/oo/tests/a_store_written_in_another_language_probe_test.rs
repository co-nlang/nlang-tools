// The store speaks a language the project does not.
// Order: nlang-tools/docs/a_store_written_in_another_language_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-012.  Ruling: meta/oo/STATUS.md O31.
//
// ── What this arc is ─────────────────────────────────────────────────────
//
// `.oo/` holds n/ values encoded as `serde` JSON of the Rust types that
// happen to implement them. Measured on v0.35.0, a 15-byte program produces
// a 137,185-byte store in which:
//
//   - the root is `{"Combo":{…}}` / `{"Atom":[{"Int":[1,[1]]},0,null]}` —
//     the Rust enum's external tagging, with the effect as a bare `0` and the
//     span as a bare `null`;
//   - the standard root is `"standard-root:<hex>"`, 68,126 bytes of JSON
//     hex-encoded inside a JSON string: **exactly 2.00x its own content**;
//   - the commit object is not a `Value` at all — a Rust struct with a base64
//     `lattice_sketch` and the digest written as 32 decimal integers, so the
//     same store spells a digest two different ways;
//   - `.oo/staged` is a third shape again: a bare `ComboVal` with `Thunk`,
//     `span` and `closure` still in it.
//
// The n/ printed form of the same standard root is 21,722 bytes — 6.3x
// smaller — and it round-trips to within 28 diff hunks. What stops it being
// the durable form is that **the language is missing two literals**:
//
//   1. the `system` axis: the printer writes `~%Cond:` and the parser answers
//      `_|_ (%cause: #system_reserved)` — 25 modules, deliberately, because
//      `SYNTAX_05` §3 reserves `~%` to the engine (`L2-60`, `L2-61`);
//   2. the effect tag: a genuinely effectful value prints `;; %effect: #io`,
//      a **comment**. `%effect:` as a combo key is a real declaration —
//      `{ %effect: #pure, v: impure }` is `_|_ #effect_violation`, so
//      Q-034's guard reads it — but it is a **wrapper, not a tag**: it adds a
//      field, and every field that lives in a value moves the CAID.
//
// ── The ruling (O31, 2026-08-26, user) ───────────────────────────────────
//
//   1. Encoding form = a frame plus two literals that are legal **only inside
//      the frame**. Making `~%` legal everywhere would delete `L2-60`/`L2-61`
//      and void the ownership clause; a privileged decoder was rejected
//      because it turns O35's "reading is decoding, not evaluation" into
//      "reading has two decoders".
//   2. The frame lives **outside the value** — measured, every field that
//      lives in a value moves the CAID (`{ a: 1 }` `fd335de1…` versus
//      `{ %kind: #store_document, a: 1 }` `1882fd8d…`), and `%val` would
//      project the frame away entirely. Two homes, both outside, both
//      required: `.oo/objects.format` `encoding=5` is the store-level gate,
//      and a token at the head of each object file is the per-object
//      self-declaration. The second is required because O35 already ruled
//      that the wire is the store — an object travels away from its store and
//      must still say what it is when it arrives.
//   3. Scope includes the commit object and `.oo/staged`.
//   4. One arc: literals, encoding, the hex layer, migration, cross-version
//      and GC traversal all together.
//
// ── The identity red line ────────────────────────────────────────────────
//
// Encoding is orthogonal to identity — measured on v0.35.0 by rewriting the
// root object's bytes from 428 to 762 (same value, different bytes): the CAID
// did not move and the store kept serving. So this arc is NOT an epoch. Two
// addresses must be byte-identical before and after:
//
//     root          932a9f9dd62297a7cb3cb9c9fb56907a06a8c4d4e945cc3dfc4782a6987fb0cb
//     standard root 7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911
//
// G1/G2 are those. **G2 is also the only witness this file has for the tag
// literal**: the durable `%effect` is part of the durable form, so an encoder
// that dropped it would move the standard root's digest. There is NO probe
// here for the tag literal's spelling, and none for the frame token's
// spelling — both are deliberately unpinned (pin properties, not spellings).
//
// ── Probe integrity ──────────────────────────────────────────────────────
//
// Reds are `#[ignore]`d and MUST be red at the baseline for the reason stated
// in each. The delivery may remove `#[ignore]` and NOTHING else in this file.
// If a pin here is wrong, say so in the report — do not edit it.
//
// Baseline measured 2026-08-26 on dev 2496207 / oo v0.35.0: 6 green, 7 red.
// Every red was checked to fail at its own assertion, not at a setup step.
// One calibration note, because a red that stops early does not calibrate the
// asserts below it: R7 reaches and PASSES its two `assert_eq!`s today — an
// encoding=4 store opens and `migrate` runs, advancing `layout` only, which is
// exactly what O73 ④ scoped it to. Only its last line is red.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const ROOT_CAID: &str = "932a9f9dd62297a7cb3cb9c9fb56907a06a8c4d4e945cc3dfc4782a6987fb0cb";
const STANDARD_ROOT: &str = "7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911";

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("encoding4_repo")
}

fn scratch(tag: &str) -> nlang_interpreter::ScratchDir {
    nlang_interpreter::ScratchDir::new(&format!("another-language-{tag}"))
}

fn oo(dir: &Path, args: &[&str]) -> String {
    let mut c = Command::new(env!("CARGO_BIN_EXE_oo"));
    c.current_dir(dir)
        .env("OO_IDENTITY", dir.join("identity-for-tests"))
        .env("OO_NODE_HOME", dir.join("node-home-for-tests"));
    let o = c.args(args).output().expect("oo runs");
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("mkdir");
    for e in fs::read_dir(from).expect("readdir") {
        let e = e.expect("entry");
        let dst = to.join(e.file_name());
        if e.file_type().expect("file type").is_dir() {
            copy_tree(&e.path(), &dst);
        } else {
            fs::copy(e.path(), &dst).expect("copy");
        }
    }
}

/// A fresh store built by THIS engine from the canonical 15-byte program.
fn fresh_store(tag: &str) -> nlang_interpreter::ScratchDir {
    let d = scratch(tag);
    fs::write(d.path().join("main.n"), "app: { k1: 1 }\n").expect("write");
    let out = oo(d.path(), &["evolve", "main.n"]);
    assert!(
        d.path().join(".oo").is_dir(),
        "REACH: evolve must create the store; got {out:?}"
    );
    let out = oo(d.path(), &["commit", "-m", "t"]);
    assert!(
        out.contains("Commit successful"),
        "REACH: commit must succeed, or nothing below is measuring a store; got {out:?}"
    );
    d
}

/// Lay the checked-in `encoding=4` repo down in a scratch directory, as `.oo`.
fn lay_out_encoding4(tag: &str) -> nlang_interpreter::ScratchDir {
    let d = scratch(tag);
    let f = fixture();
    copy_tree(&f.join("oo_dir"), &d.path().join(".oo"));
    fs::copy(f.join("main.n"), d.path().join("main.n")).expect("main.n");
    d
}

fn object_path(dir: &Path, digest: &str) -> PathBuf {
    dir.join(".oo/objects/sha256")
        .join(&digest[..2])
        .join(&digest[2..])
}

fn object_bytes(dir: &Path, digest: &str) -> Vec<u8> {
    fs::read(object_path(dir, digest)).unwrap_or_else(|e| panic!("object {digest} must exist: {e}"))
}

fn head_digest(dir: &Path) -> String {
    fs::read_to_string(dir.join(".oo/HEAD"))
        .expect("HEAD")
        .trim()
        .rsplit(':')
        .next()
        .expect("digest")
        .to_string()
}

/// Every CAS object in the store, as (digest, bytes).
fn all_objects(dir: &Path) -> Vec<(String, Vec<u8>)> {
    fn walk(p: &Path, out: &mut Vec<PathBuf>) {
        let Ok(rd) = fs::read_dir(p) else { return };
        for e in rd.flatten() {
            if e.path().is_dir() {
                walk(&e.path(), out);
            } else {
                out.push(e.path());
            }
        }
    }
    let mut files = Vec::new();
    walk(&dir.join(".oo/objects"), &mut files);
    files
        .into_iter()
        .map(|f| {
            let name = f.file_name().unwrap().to_string_lossy().to_string();
            let parent = f
                .parent()
                .unwrap()
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string();
            (format!("{parent}{name}"), fs::read(&f).expect("read"))
        })
        .collect()
}

/// The `serde` tags that give away "this is the Rust enum, not an n/ value".
/// Named individually so a report can say which one survived.
const RUST_SERDE_TAGS: &[&str] = &[
    "{\"Combo\"",
    "\"masa_ref\"",
    "{\"Atom\"",
    "\"lattice_sketch\"",
    "\"digest\":[",
    "{\"Thunk\"",
];

fn serde_tags_in(bytes: &[u8]) -> Vec<&'static str> {
    let s = String::from_utf8_lossy(bytes);
    RUST_SERDE_TAGS
        .iter()
        .copied()
        .filter(|t| s.contains(t))
        .collect()
}

/// The longest run of hex characters in the bytes. A payload that has been
/// hex-encoded shows up as a run in the thousands; a digest is 64.
fn longest_hex_run(bytes: &[u8]) -> usize {
    let mut best = 0usize;
    let mut cur = 0usize;
    for b in bytes {
        if b.is_ascii_hexdigit() {
            cur += 1;
            best = best.max(cur);
        } else {
            cur = 0;
        }
    }
    best
}

// ─────────────────────────────────────────────────────────────────────────
// RED — what the arc must make true.
// ─────────────────────────────────────────────────────────────────────────

/// The headline. The root object on disk is the Rust enum's serde form.
#[test]
fn r1_the_root_object_is_not_a_rust_enum_dump() {
    let d = fresh_store("r1");
    let bytes = object_bytes(d.path(), ROOT_CAID);
    let found = serde_tags_in(&bytes);
    assert!(
        found.is_empty(),
        "the root object still carries the Rust type's serde tags {found:?}. \
         The durable form is supposed to be n/, not the shape of whichever \
         Rust enum happens to implement it. First 200 bytes: {:?}",
        String::from_utf8_lossy(&bytes[..bytes.len().min(200)])
    );
}

/// The standard root is JSON hex-encoded inside a JSON string: 136,268 bytes
/// carrying 68,126 bytes of content, exactly 2.00x. Half of that file is the
/// hex, and the hex has nothing to do with which language the store speaks.
#[test]
fn r2_the_standard_root_is_not_a_hex_blob() {
    let d = fresh_store("r2");
    let bytes = object_bytes(d.path(), STANDARD_ROOT);
    let run = longest_hex_run(&bytes);
    assert!(
        run < 1000,
        "the standard root object still contains a {run}-character hex run, \
         i.e. its payload is hex-encoded. A digest is 64 characters; anything \
         in the thousands is a hex-encoded body. Object is {} bytes.",
        bytes.len()
    );
}

/// The commit object is not a `Value`, so it never went through the value
/// encoder at all — and it spells a digest as 32 decimal integers while the
/// root object next to it spells the same kind of thing as 64 hex characters.
/// O31 ② put it in scope.
#[test]
fn r3_the_commit_object_speaks_the_same_language_as_the_root() {
    let d = fresh_store("r3");
    let digest = head_digest(d.path());
    let bytes = object_bytes(d.path(), &digest);
    let found = serde_tags_in(&bytes);
    assert!(
        found.is_empty(),
        "the commit object still carries {found:?}. O31 ② put the commit \
         object in scope precisely so the store stops holding two encodings \
         and two spellings of a digest. First 200 bytes: {:?}",
        String::from_utf8_lossy(&bytes[..bytes.len().min(200)])
    );
}

/// `.oo/staged` is a third shape again — a bare `ComboVal` with `Thunk`,
/// `span` and `closure` still in it. It is durable and it has no address, so
/// O51 keeps the Thunk; O31 ② still puts it in scope, which means the
/// encoding form has to be able to express an unforced value.
#[test]
fn r4_staged_is_in_the_same_encoding() {
    let d = scratch("r4");
    fs::write(d.path().join("main.n"), "app: { k1: 1 }\n").expect("write");
    let out = oo(d.path(), &["evolve", "main.n"]);
    let ps = nlang_interpreter::injections::paths(d.path()).expect("list injections");
    let staged = ps.first().cloned();
    assert!(
        staged.as_ref().is_some_and(|p| p.is_file()),
        "REACH: evolve without commit must leave an injection; got {out:?}"
    );
    let bytes = fs::read(staged.as_ref().unwrap()).expect("read injection");
    let found = serde_tags_in(&bytes);
    assert!(
        found.is_empty(),
        "the working-set injection still carries {found:?}. It keeps its Thunk by O51 — \
         that is not what this pins. What this pins is that the unforced form \
         is written in the same language as everything else. First 200 bytes: {:?}",
        String::from_utf8_lossy(&bytes[..bytes.len().min(200)])
    );
}

/// The per-object self-declaration (O31 ② second home). Reading a CAS object
/// must not depend on knowing which store you took it out of, because O35
/// already ruled that the wire is the store: `peer fetch` is `get_value`, so
/// objects travel alone. The spelling of the token is NOT pinned — only that
/// an object no longer opens straight into a bare value.
#[test]
fn r5_every_object_declares_what_it_is() {
    let d = fresh_store("r5");
    let objects = all_objects(d.path());
    assert!(
        objects.len() >= 3,
        "REACH: a committed store must hold at least root, standard root and \
         commit; found {}",
        objects.len()
    );
    let bare: Vec<String> = objects
        .iter()
        .filter(|(_, b)| b.first().is_some_and(|c| *c == b'{' || *c == b'"'))
        .map(|(d, _)| d[..8].to_string())
        .collect();
    assert!(
        bare.is_empty(),
        "these objects open straight into a value with no declaration: {bare:?}. \
         An object that travels away from its store has to say what it is when \
         it arrives; `objects.format` stays behind."
    );
}

/// The store-level gate (O31 ② first home). A store this engine writes
/// declares the new encoding, so a pre-Q-012 engine refuses to open it out
/// loud rather than misreading it — `ensure_supported_encoding` already
/// refuses anything outside the range it understands.
#[test]
fn r6_a_new_store_declares_the_new_encoding() {
    let d = fresh_store("r6");
    let declared = fs::read_to_string(d.path().join(".oo/objects.format")).expect("format");
    assert!(
        declared.contains("encoding=5"),
        "a store written by this engine must declare the new encoding; got {declared:?}"
    );
}

/// Cross-version, the half that must keep working. A store written by
/// v0.35.0 declares `encoding=4`; it must still open, and `oo migrate` must
/// advance it — without moving HEAD and without moving a single address.
/// Q-038/O73 built `migrate` and its rule: migration advances the declaration
/// only, and is something you ask for.
///
/// Measured at the baseline: the store DOES still open and `migrate` DOES run
/// — it advances `layout` and leaves `encoding=4` alone, which is what O73 ④
/// scoped it to. So the two `assert_eq!`s at the end are reached today and
/// pass today; what is red is only the encoding line.
#[test]
fn r7_an_encoding_4_store_opens_and_migrates_without_moving_anything() {
    let d = lay_out_encoding4("r7");
    let before_head = head_digest(d.path());
    let before: Vec<String> = {
        let mut v: Vec<String> = all_objects(d.path()).into_iter().map(|(k, _)| k).collect();
        v.sort();
        v
    };

    let status = oo(d.path(), &["status"]);
    assert!(
        !status.contains("not supported") && !status.contains("refusing"),
        "REACH: an encoding=4 store must still open after this arc; got {status:?}"
    );

    let out = oo(d.path(), &["migrate", "--grant", "migrate"]);

    assert_eq!(
        head_digest(d.path()),
        before_head,
        "migrate must not move HEAD (O73 ④); migrate said {out:?}"
    );
    let after: Vec<String> = {
        let mut v: Vec<String> = all_objects(d.path()).into_iter().map(|(k, _)| k).collect();
        v.sort();
        v
    };
    assert_eq!(
        after, before,
        "migrate must not move a single address (O73 ④)"
    );

    let after_decl = fs::read_to_string(d.path().join(".oo/objects.format")).expect("format");
    assert!(
        after_decl.contains("encoding=5"),
        "migrate must advance the encoding declaration too. Today it advances \
         `layout` only and leaves this line alone. got {after_decl:?} \
         (migrate said {out:?})"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// GREEN — fences. These pass today and must still pass afterwards.
// ─────────────────────────────────────────────────────────────────────────

/// Identity red line, half one. The canonical 15-byte program's root address.
#[test]
fn g1_the_root_address_does_not_move() {
    let d = fresh_store("g1");
    assert!(
        object_path(d.path(), ROOT_CAID).is_file(),
        "root {ROOT_CAID} must exist byte-for-byte where it always has. \
         Encoding is orthogonal to identity — measured on v0.35.0 by rewriting \
         this object's bytes from 428 to 762 with the CAID unchanged. If this \
         goes red the arc has become an epoch."
    );
}

/// Identity red line, half two — and the ONLY witness in this file that the
/// durable effect survived the re-encoding, since `%effect` is part of the
/// durable form and an encoder that dropped it would move this digest.
#[test]
fn g2_the_standard_root_digest_does_not_move() {
    let d = fresh_store("g2");
    assert!(
        object_path(d.path(), STANDARD_ROOT).is_file(),
        "standard root {STANDARD_ROOT} must not move. This also stands in for \
         the durable `%effect`: it is part of the durable form, so dropping it \
         while re-encoding would show up here."
    );
    let root = String::from_utf8_lossy(&object_bytes(d.path(), ROOT_CAID)).to_string();
    assert!(
        root.contains(STANDARD_ROOT),
        "REACH: the root must still name its standard root by that digest \
         (REAL_03 §6.8), or this test is not measuring the sentinel it thinks \
         it is. Root object: {root:?}"
    );
}

/// The frame must not leak. `~%` as a combo definition key stays illegal in
/// ordinary source — `SYNTAX_05` §3 ownership, conformance `L2-60`/`L2-61`.
/// This is the whole reason the literal is frame-only rather than global.
#[test]
fn g3_the_system_axis_is_still_reserved_in_ordinary_source() {
    let d = scratch("g3");
    let f = d.path().join("main.n");
    fs::write(&f, "out: { ~%Math: 9 }\n").expect("write");
    let got = oo(d.path(), &["run", f.to_str().unwrap(), "--observe", "out"]);
    assert!(
        got.contains("_|_") && got.contains("system_reserved"),
        "a `~%` definition key in a combo literal must still be `_|_` with \
         `#system_reserved`. If the frame's literal became legal everywhere, \
         L2-60 and L2-61 are gone and the ownership clause is void. got {got:?}"
    );
}

/// Q-034's declaration guard must survive the re-encoding. A false `#pure`
/// still collapses — the effect is not something the new encoder may quietly
/// normalise away.
#[test]
fn g4_a_false_pure_declaration_still_collapses() {
    let d = scratch("g4");
    let f = d.path().join("main.n");
    fs::write(&f, "out: { %effect: #pure, v: (~%Time.now _) }\n").expect("write");
    let got = oo(d.path(), &["run", f.to_str().unwrap(), "--observe", "out"]);
    assert!(
        got.contains("_|_") && got.contains("effect_violation"),
        "Q-034's guard must still read the value it was handed. got {got:?}"
    );
}

/// GC's traversal must find exactly the same objects. Today the traversal is
/// syntactic — `gc.rs:48/72/85` scans for any 64-character hex string, any
/// integer array that hexes to 64, and any `hash:sha256:` prefix. Changing the
/// encoding changes every one of those, so this is the regression gate W8'
/// pointed at: same store, same reachable count, nothing condemned.
#[test]
fn g5_gc_finds_every_object_and_condemns_none() {
    let d = fresh_store("g5");
    let n = all_objects(d.path()).len();
    let got = oo(d.path(), &["gc", "--grant", "gc"]);
    assert!(
        got.contains(&format!("{n} objects, {n} reachable, 0 collectable")),
        "gc must reach all {n} objects and collect none in a healthy store. \
         If the traversal stops finding a reference after the re-encoding, it \
         deletes a live object. got {got:?}"
    );
    assert_eq!(
        all_objects(d.path()).len(),
        n,
        "gc must not have removed anything"
    );
    let status = oo(d.path(), &["status"]);
    assert!(
        !status.contains("refusing") && !status.contains("unavailable"),
        "the store must still open after gc; got {status:?}"
    );
}

/// The checked-in `encoding=4` repo really is the old shape. Without this the
/// cross-version tests could be measuring a fixture that quietly got rebuilt.
#[test]
fn g6_the_fixture_really_is_the_old_encoding() {
    let d = lay_out_encoding4("g6");
    let decl = fs::read_to_string(d.path().join(".oo/objects.format")).expect("format");
    assert!(
        decl.contains("encoding=4"),
        "fixture must declare encoding=4; got {decl:?}"
    );
    let root = object_bytes(d.path(), ROOT_CAID);
    assert!(
        !serde_tags_in(&root).is_empty(),
        "fixture's root must still be the Rust serde form — that is what makes \
         it a cross-version fixture rather than a copy of today's output"
    );
    let std_root = object_bytes(d.path(), STANDARD_ROOT);
    assert!(
        longest_hex_run(&std_root) > 1000,
        "fixture's standard root must still be the hex blob"
    );
}
