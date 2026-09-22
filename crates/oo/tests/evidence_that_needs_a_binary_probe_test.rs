// Evidence that needs a binary is evidence with an expiry date.
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-053.  Ruling: D72 丙 (STATUS.md).
//
// ── What this arc is ─────────────────────────────────────────────────────
//
// `fixtures/pre_sentinel_repo` is the witness for `REAL_03` §6.8: a repo
// written before the sentinel names no standard root digest, because it
// embeds the whole standard root instead. It was built by the real
// `oo v0.20.0` binary, and v0.56.0 (the `Thunk` identity epoch) stopped
// being able to verify it — `#caid_mismatch`, requested `16ba5683…` vs
// recomputed `cef5e484…`.
//
// D72 甲 ruled that the loss is the ordinary cost of an epoch. D72 丙 asked
// what the artifact can still prove. The recon's answer changed the question:
//
//   * Its BYTES are immortal. They are checked in, and every §6.8 fact about
//     them is readable as data — no engine, no digest verification, no
//     encoder. That is what this file does, and no future epoch can touch it.
//
//   * What is NOT immortal is `/home/gali/nlang-baselines/v0.20.0-target`,
//     the binary that made it. Measured 2026-09-22: it still reproduces the
//     root object BYTE-FOR-BYTE (same 67,913 B, same digest `16ba5683…`,
//     sha256 of the file `95dd69bb…` on both sides; only the commit object
//     differs, by its timestamp). That directory is inside no git repository.
//     The fixture is the durable copy; the reproducer is not.
//
//   * The next thing to be eaten was not the artifact but ONE STRING
//     CONSTANT: r1 in `a_commit_that_closes_the_door_probe_test.rs` pinned
//     `cef5e484`, the digest THIS engine recomputes. That literal now lives
//     alone in `e5` below, so that when an epoch moves it, the failing test
//     name says what happened instead of looking like a regression.
//
// ── The invariant, not the mechanism ─────────────────────────────────────
//
// A normative clause's evidence must not depend on a binary being present.
// Where the claim is about what an older engine DID, the claim is a fact
// about bytes, and the test must read bytes. Where it is about what THIS
// engine does, the test may run this engine — but then it must not also
// pin what the engine computes, or it is pinning the present, not the past.
//
// Baseline measured 2026-09-22 on dev 09b3a91 / oo v0.56.0: 6 green, 0 red.
// All six are green by construction — they assert facts about checked-in
// bytes and one refusal message. Calibration (each assertion was made to
// fail on purpose before hand-off) is recorded in the Q-053 queue entry.
// If a pin here is wrong, say so in the report — do not edit it.

use serde_json::Value as Json;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const ROOT_DIGEST_HEX: &str = "16ba56831ac26b8c4a9412840bbfc6eed7271f51f284c485c2e9c88f04db00d4";
const COMMIT_DIGEST_HEX: &str = "72b66928ca1ae236d29a16568ea67ccacbc0a6e62fa7b23ce17a9ee30531939b";

/// Every `Value` variant, so an absence is asserted against a closed list
/// rather than against the ones this test happened to think of.
const VALUE_KINDS: &[&str] = &[
    "Top", "TopCaused", "Atom", "Combo", "Union", "Code", "Thunk", "Ref", "Bottom", "Blur", "Range",
];

/// Every `AtomKind` variant, same reason.
const ATOM_KINDS: &[&str] = &[
    "Int", "Float", "Complex", "Str", "MultilineStr", "Tag", "TagStart", "TagEnd", "Regex", "Top",
    "Bottom", "Unit", "PathLit", "Bytes", "Uri", "Time",
];

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("pre_sentinel_repo")
}

fn oo_dir() -> PathBuf {
    fixture().join("oo_dir")
}

fn object_path(digest_hex: &str) -> PathBuf {
    oo_dir()
        .join("objects")
        .join("sha256")
        .join(&digest_hex[..2])
        .join(&digest_hex[2..])
}

fn root_json() -> Json {
    let raw = fs::read_to_string(object_path(ROOT_DIGEST_HEX)).expect("root object is checked in");
    serde_json::from_str(&raw).expect("the root object is JSON")
}

/// Walk the externally-tagged value tree, counting `Value` and `AtomKind`
/// variants. A single-key object whose key is a known variant name is a
/// node; anything else is structure. `field_names` collects the keys of
/// every `data` map so the caller can prove no field name collides with a
/// variant name — without that control the counts could be inflated by a
/// field someone happened to call `Atom`.
struct Census {
    values: BTreeMap<String, usize>,
    atoms: BTreeMap<String, usize>,
    field_names: BTreeMap<String, usize>,
}

fn census(root: &Json) -> Census {
    let mut c = Census {
        values: BTreeMap::new(),
        atoms: BTreeMap::new(),
        field_names: BTreeMap::new(),
    };
    walk(root, false, &mut c);
    c
}

fn walk(node: &Json, in_atom: bool, c: &mut Census) {
    match node {
        Json::Object(map) => {
            if map.len() == 1 {
                let (key, payload) = map.iter().next().expect("len 1");
                if in_atom && ATOM_KINDS.contains(&key.as_str()) {
                    *c.atoms.entry(key.clone()).or_default() += 1;
                    walk(payload, false, c);
                    return;
                }
                if VALUE_KINDS.contains(&key.as_str()) {
                    *c.values.entry(key.clone()).or_default() += 1;
                    walk(payload, key == "Atom", c);
                    return;
                }
            }
            for (key, child) in map {
                if key == "data" {
                    if let Json::Object(fields) = child {
                        for name in fields.keys() {
                            *c.field_names.entry(name.clone()).or_default() += 1;
                        }
                    }
                }
                walk(child, in_atom, c);
            }
        }
        Json::Array(items) => {
            for item in items {
                walk(item, in_atom, c);
            }
        }
        _ => {}
    }
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

fn lay_out_legacy(tag: &str) -> nlang_interpreter::ScratchDir {
    let d = nlang_interpreter::ScratchDir::new(&format!("needs-a-binary-{tag}"));
    copy_tree(&oo_dir(), &d.path().join(".oo"));
    fs::copy(fixture().join("main.n"), d.path().join("main.n")).expect("main.n");
    d
}

// ─────────────────────────────────────────────────────────────────────────
// The §6.8 facts, as facts about bytes. No engine is involved in e1–e5,
// except e5 which asks this engine one question and pins the answer on
// purpose.
// ─────────────────────────────────────────────────────────────────────────

/// §6.8 exists because a pre-sentinel root embedded the whole standard root
/// instead of naming it. That is a claim about proportion, and the artifact
/// settles it without being opened: 67,913 of the 68,526 bytes of `oo_dir`
/// are the single root object.
#[test]
fn e1_the_artifact_is_almost_entirely_an_embedded_standard_root() {
    let root = fs::metadata(object_path(ROOT_DIGEST_HEX)).expect("root object").len();
    let mut total = 0u64;
    let mut stack = vec![oo_dir()];
    while let Some(dir) = stack.pop() {
        for e in fs::read_dir(&dir).expect("readdir") {
            let e = e.expect("entry");
            if e.file_type().expect("type").is_dir() {
                stack.push(e.path());
            } else {
                total += e.metadata().expect("metadata").len();
            }
        }
    }
    assert_eq!(root, 67_913, "the root object's size is part of the evidence");
    assert_eq!(total, 68_526, "the whole pre-sentinel store is part of the evidence");
    let share = root as f64 / total as f64;
    assert!(
        share > 0.99,
        "the point of §6.8 is that the embedded standard root dwarfs the program; \
         got {share:.4} ({root} of {total} B)"
    );
}

/// The other half of §6.8: a pre-sentinel root names NO standard root by
/// digest. `masa_ref` is the field that would carry one, and in this artifact
/// every single one of them is the bare string `"Top"`. This is why `status`
/// answers `self-contained (pre-sentinel)` — the answer is a property of the
/// bytes, not of the engine reading them.
#[test]
fn e2_the_artifact_names_no_standard_root_digest() {
    let raw = fs::read_to_string(object_path(ROOT_DIGEST_HEX)).expect("root object");
    assert!(
        !raw.contains("Digest"),
        "a pre-sentinel root must name no standard root digest; found `Digest` in the bytes"
    );
    let root = root_json();
    let mut refs = BTreeMap::new();
    let mut stack = vec![&root];
    while let Some(node) = stack.pop() {
        match node {
            Json::Object(map) => {
                for (key, child) in map {
                    if key == "masa_ref" {
                        *refs.entry(child.to_string()).or_insert(0usize) += 1;
                    }
                    stack.push(child);
                }
            }
            Json::Array(items) => stack.extend(items.iter()),
            _ => {}
        }
    }
    assert!(!refs.is_empty(), "the measurement must reach at least one `masa_ref`");
    assert_eq!(
        refs.keys().collect::<Vec<_>>(),
        vec!["\"Top\""],
        "every `masa_ref` in a pre-sentinel root must be `Top`; got {refs:?}"
    );
}

/// The blast radius of the NEXT identity epoch, written down as a test. The
/// artifact contains exactly three value kinds and exactly three atom kinds;
/// an epoch that touches none of them, and touches nothing global (tag
/// numbering, length encoding, field order), cannot move its recomputed
/// digest. Notably there is no `Float`, so O88 cannot reach it by that route.
#[test]
fn e3_the_value_kinds_in_the_artifact_are_a_closed_list() {
    let c = census(&root_json());

    // Control first: if any `data` field were named like a variant, every
    // count below would be suspect.
    let collisions: Vec<&String> = c
        .field_names
        .keys()
        .filter(|n| VALUE_KINDS.contains(&n.as_str()) || ATOM_KINDS.contains(&n.as_str()))
        .collect();
    assert!(
        collisions.is_empty(),
        "a field name that collides with a variant name would inflate the census; got {collisions:?}"
    );
    assert!(
        c.field_names.len() >= 12,
        "the census must have reached the `data` maps; saw {} field names",
        c.field_names.len()
    );

    assert_eq!(c.values.get("Combo").copied(), Some(291), "Combo count");
    assert_eq!(c.values.get("Atom").copied(), Some(562), "Atom count");
    assert_eq!(c.values.get("Thunk").copied(), Some(2), "Thunk count");
    for kind in VALUE_KINDS {
        if !["Combo", "Atom", "Thunk"].contains(kind) {
            assert_eq!(
                c.values.get(*kind).copied().unwrap_or(0),
                0,
                "`{kind}` must be absent from this artifact"
            );
        }
    }

    assert_eq!(c.atoms.get("Tag").copied(), Some(294), "Tag count");
    assert_eq!(c.atoms.get("Str").copied(), Some(259), "Str count");
    assert_eq!(c.atoms.get("Int").copied(), Some(8), "Int count");
    for kind in ATOM_KINDS {
        if !["Tag", "Str", "Int"].contains(kind) {
            assert_eq!(
                c.atoms.get(*kind).copied().unwrap_or(0),
                0,
                "`AtomKind::{kind}` must be absent from this artifact"
            );
        }
    }
}

/// The artifact is internally consistent without any engine: HEAD names the
/// commit object, the commit object names the root by digest, and the root
/// object is at that digest's path. A repo that has lost this is a broken
/// fixture rather than a superseded one, and the distinction must not depend
/// on a binary being available to tell them apart.
#[test]
fn e4_head_and_the_commit_and_the_root_agree_by_digest() {
    let head = fs::read_to_string(oo_dir().join("HEAD")).expect("HEAD");
    assert_eq!(
        head.trim(),
        format!("hash:sha256:v1:{COMMIT_DIGEST_HEX}"),
        "HEAD must name the commit object"
    );
    assert_eq!(
        fs::read_to_string(oo_dir().join("format")).expect("format").trim(),
        "2",
        "this artifact is a bare-number (pre-split-axis) store"
    );

    let commit: Json = serde_json::from_str(
        &fs::read_to_string(object_path(COMMIT_DIGEST_HEX)).expect("commit object"),
    )
    .expect("commit object is JSON");
    let digest = commit["root"]["digest"]
        .as_array()
        .expect("root digest is a byte array")
        .iter()
        .map(|b| format!("{:02x}", b.as_u64().expect("byte")))
        .collect::<String>();
    assert_eq!(digest, ROOT_DIGEST_HEX, "the commit must name the root by digest");
    assert!(object_path(&digest).exists(), "the named root object must be present");
}

/// ⚠ THE FUSE, ON PURPOSE AND ALONE.
///
/// `cef5e484…` is not a fact about the artifact. It is what THIS engine
/// recomputes for it, and it moved once already — at v0.56.0, when `Thunk`
/// identity changed. Any future epoch touching `Combo`, `Atom{Int,Str,Tag}`,
/// or anything global will move it again.
///
/// When this test goes red, nothing is broken. It means an identity epoch
/// reached this artifact, and the required action is: record the new digest
/// in the epoch's CHANGELOG entry, update the literal here, and check that
/// `e1`–`e4` are still green — because those are the evidence, and this is
/// only the engine's current opinion of it.
#[test]
fn e5_this_engine_still_recomputes_cef5e484_for_the_artifact() {
    let d = lay_out_legacy("e5");
    let out = oo(d.path(), &["log"]);
    assert!(
        out.contains("#caid_mismatch"),
        "the artifact must be refused by name, never silently mis-answered; got {out:?}"
    );
    assert!(
        out.contains(ROOT_DIGEST_HEX) || out.contains("16ba5683"),
        "the refusal must name what the artifact asked for; got {out:?}"
    );
    assert!(
        out.contains("cef5e484"),
        "if this is the only failure in this file, an identity epoch moved the \
         recomputed digest — see this test's doc comment, and do not edit e1–e4. \
         got {out:?}"
    );
}

/// The red line the recon found, stated as an invariant: this engine must not
/// stop being able to READ a store it once wrote. The artifact is encoding 2;
/// `status` still parses it (it answers the standard-root question) even
/// though it refuses to verify the universe. And the refusal for an encoding
/// this engine does not know must keep naming the supported range starting
/// at 1 — if the floor ever rises, this goes red and says so.
#[test]
fn e6_the_oldest_object_encoding_this_engine_wrote_still_opens() {
    let d = lay_out_legacy("e6-read");
    let st = oo(d.path(), &["status"]);
    assert!(
        st.contains("Standard root dependency: self-contained (pre-sentinel)"),
        "an encoding-2 store must still be PARSED, whatever the epoch says about \
         its digests; got {st:?}"
    );

    let f = nlang_interpreter::ScratchDir::new("needs-a-binary-e6-floor");
    fs::write(f.path().join("main.n"), "x: 0\n").expect("write");
    oo(f.path(), &["evolve", "main.n"]);
    oo(f.path(), &["commit", "-m", "seed"]);
    let enc = f.path().join(".oo").join("objects.format");
    assert!(enc.exists(), "a current store declares its object encoding");
    // `encoding=N`, not a bare number: a bare number is refused one step
    // earlier, by the declaration parser, and never reaches the range check
    // this test is about. (Found while calibrating: the first version of this
    // test wrote "99" and was measuring the wrong refusal.)
    fs::write(&enc, "encoding=99\n").expect("write encoding");
    let refusal = oo(f.path(), &["status"]);
    assert!(
        refusal.contains("understands encoding 1 through"),
        "the supported-encoding floor must stay at 1: an engine that drops a \
         legacy encoding silently retires every artifact written in it. got {refusal:?}"
    );
}
