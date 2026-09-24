// A commit that is a value.
// Order: nlang-tools/docs/a_commit_that_is_a_value_handover.md
// Recon: nlang-tools/docs/a_commit_address_quote_recon.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-057.  Ruling: meta/oo/STATUS.md D74.
//
// ── What this arc is ─────────────────────────────────────────────────────
//
// A commit's address was computed by hand-picking fields:
//
//     [parent.digest] ‖ root.digest ‖ kind ‖ source.digest* ‖ target.digest*
//         ‖ LEB128(len) ‖ format!("{:?}", meta)
//
// and three kinds of bytes that are written to disk, read back and shown to
// the operator were left outside it (recon F1–F3, all measured):
//
//   F1  `refine.authority_status`: rewrite "unverified" as "verified" and
//       `oo log` prints `refine authority: verified` with rc=0.
//   F2  source and target digests are concatenated with no counts: move one
//       CAID across the boundary and the address does not change.
//   F3  the root CAID's version/masa/sketch inside the commit: `inspect`
//       prints a root address the commit never committed to.
//   F4  (the original question) `{:?}` escapes strings by the Rust standard
//       library's Unicode tables, so the address depends on the toolchain.
//
// D74 (user, "甲"): a commit IS a value. Its address is the address of the n/
// value it is written as on disk, through the same table as every other value.
// Existing commits are never rewritten (REAL_02 §5.1.1); they keep verifying
// under the legacy algorithm. Fields the legacy address does not cover are
// shown as not covered — a legacy refine authority is never "verified", it is
// "unattested" (D74 ②).
//
// ── The fixture ──────────────────────────────────────────────────────────
//
// `fixtures/layout5_repo/` was built by the real `oo v0.58.0` binary: one
// ordinary commit and one refine commit (genesis exemption, "unverified"),
// addressed the legacy way. See its README.
//
// ── Probe integrity ──────────────────────────────────────────────────────
//
// `r1` defines "the address of its value" as the store's own value decoder
// applied to the commit's on-disk body (AMENDED at R-1 acceptance; it used to
// be `~%Discovery./identify`, which evaluates -- see `value_address_of_body`).
// `c1` shows that measurement reaches a commit body. Every
// tamper is checked to have changed the bytes it meant to change before the
// red assertion; otherwise `VOID READING`.
//
// The delivery may NOT edit this file. If a pin is wrong, say so in the
// report. `rustfmt` must not touch it.
//
// Baseline measured 2026-09-24 on dev fe7f578 / oo v0.58.0: see the order.
// r7 added at acceptance (repair round R-1): VOID on v0.58.0 (no v2 HEAD),
// red on the delivery 44cbe32; see the order §9.
// AMENDED at R-1 acceptance: the r1 oracle is the store value decoder, not
// `identify`; g5 added (green on v0.58.0, red on 8b9dda1); see §11.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

struct Ws {
    _scratch: nlang_interpreter::ScratchDir,
    root: PathBuf,
    ws: PathBuf,
}

impl Ws {
    fn new(tag: &str) -> Self {
        let scratch = nlang_interpreter::ScratchDir::new(&format!("commit-value-{tag}"));
        let root = scratch.path().to_path_buf();
        let ws = root.join("ws");
        fs::create_dir_all(&ws).unwrap();
        Ws { _scratch: scratch, root, ws }
    }

    /// A workspace holding a copy of the legacy fixture.
    fn legacy(tag: &str) -> Self {
        let w = Ws::new(tag);
        let f = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/layout5_repo");
        copy_tree(&f.join("oo_dir"), &w.ws.join(".oo"));
        fs::copy(f.join("main.n"), w.ws.join("main.n")).unwrap();
        w
    }

    /// (stdout ++ stderr, exit code) — the child's own exit code.
    fn oo(&self, args: &[&str]) -> (String, i32) {
        let o = Command::new(env!("CARGO_BIN_EXE_oo"))
            .args(args)
            .current_dir(&self.ws)
            .env("OO_IDENTITY", self.root.join("identity"))
            .env("OO_NODE_HOME", self.root.join("node-home"))
            .output()
            .expect("oo runs");
        (
            format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)),
            o.status.code().unwrap_or(-1),
        )
    }

    fn ok(&self, args: &[&str]) -> String {
        let (o, rc) = self.oo(args);
        assert_eq!(rc, 0, "setup `oo {}`: {o}", args.join(" "));
        o
    }

    fn head(&self) -> String {
        fs::read_to_string(self.ws.join(".oo/HEAD")).unwrap().trim().to_string()
    }

    fn layout(&self) -> String {
        fs::read_to_string(self.ws.join(".oo/format")).unwrap().trim().to_string()
    }

    /// The object file for a CAID, found by its 64-hex digest.
    fn object(&self, caid: &str) -> PathBuf {
        let d = digest_of(caid);
        self.ws.join(".oo/objects/sha256").join(&d[..2]).join(&d[2..])
    }

    /// Evolve and commit one program; returns the new HEAD.
    fn commit(&self, program: &str, msg: &str) -> String {
        fs::write(self.ws.join("p.n"), program).unwrap();
        self.ok(&["evolve", "p.n"]);
        self.ok(&["commit", "-m", msg]);
        self.head()
    }

    /// The root CAID a commit names, as `inspect` prints it.
    fn root_of(&self, commit: &str) -> String {
        let o = self.ok(&["inspect", commit]);
        o.lines()
            .find_map(|l| l.strip_prefix("root:"))
            .unwrap_or_else(|| panic!("no root line: {o}"))
            .trim()
            .to_string()
    }

    /// The value address of a commit's on-disk body.
    ///
    /// AMENDED AT R-1 ACCEPTANCE (2026-09-24). This helper used to ask
    /// `oo eval '~%Discovery./identify (<body>)'`. That was the acceptor's
    /// error: evaluating the body yields a combo of THUNKS (D46), which is not
    /// the value on disk, so `identify` of the text and the stored value have
    /// different addresses. The measurement then became the mechanism twice —
    /// the first delivery evaluated every commit to match it, and the R-1
    /// delivery changed `identify` itself for commit-shaped input to match it.
    ///
    /// The oracle is now the store's own VALUE decoder: the body framed as a
    /// value document, decoded by `store_codec::decode_value` (the function
    /// that reads every value object from disk), then `content_hash`. That is
    /// D74 read literally — "the address of the n/ value it is written as on
    /// disk, through the same table as every other value" — and it involves
    /// no commit-specific code and no evaluator.
    fn value_address_of_body(&self, caid: &str) -> String {
        let text = fs::read_to_string(self.object(caid)).unwrap();
        let body = text.splitn(2, '\n').nth(1).unwrap_or_else(|| panic!("no frame line: {text}"));
        let doc = format!("{}\n{}", nlang_interpreter::store_codec::FRAME, body.trim());
        let v = nlang_interpreter::store_codec::decode_value(&doc)
            .unwrap_or_else(|e| panic!("the body does not decode as a value: {e}: {body}"));
        v.content_hash().to_string()
    }

    /// Replace `from` with `to` in a commit object, exactly once.
    fn tamper(&self, caid: &str, from: &str, to: &str) {
        let p = self.object(caid);
        let s = fs::read_to_string(&p).unwrap();
        if s.matches(from).count() != 1 {
            panic!("VOID READING: expected exactly one `{from}` in the commit object: {s}");
        }
        fs::write(&p, s.replacen(from, to, 1)).unwrap();
    }
}

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let dst = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_tree(&e.path(), &dst);
        } else {
            fs::copy(e.path(), &dst).unwrap();
        }
    }
}

fn digest_of(caid: &str) -> String {
    let d = caid.rsplit(':').next().unwrap().to_string();
    assert!(d.len() == 64 && d.chars().all(|c| c.is_ascii_hexdigit()), "not a CAID: {caid}");
    d
}

const LEGACY_BASE: &str =
    "hash:sha256:v1:71385b0a3effe7900d941d154a9a05f96b95b49a1f3d50ba5f91098fe168e30b";
const LEGACY_REFINE: &str =
    "hash:sha256:v1:5e9e7d7b2af3261f4db4e41cede8d22a675d97c6a02f24d6de43e63b1d24fa75";

// ── Controls and guards (green at baseline, must stay green) ─────────────

/// The measurement `r1` relies on reaches a commit body: the store's value
/// decoder reads a commit object's body as a value and gives it an address,
/// and that address is NOT the legacy commit address (so `r1` was red on
/// v0.58.0 for a real reason).
#[test]
fn c1_a_commit_body_has_a_value_address() {
    let w = Ws::legacy("c1");
    let v = w.value_address_of_body(LEGACY_BASE);
    assert!(v.starts_with("hash:sha256:v2:"), "identify of a commit body: {v}");
    assert_ne!(digest_of(&v), digest_of(LEGACY_BASE), "the legacy address is not the value address");
}

/// Legacy commits still verify and still read: both commits are in `log`,
/// with their legacy addresses, and `inspect` reads each.
#[test]
fn g1_legacy_commits_still_verify() {
    let w = Ws::legacy("g1");
    let o = w.ok(&["log"]);
    assert!(o.contains(LEGACY_BASE) && o.contains(LEGACY_REFINE), "legacy history: {o}");
    w.ok(&["inspect", LEGACY_BASE]);
    w.ok(&["inspect", LEGACY_REFINE]);
}

/// The legacy algorithm still catches what it covers.
#[test]
fn g2_legacy_tamper_of_a_covered_field_is_still_caught() {
    let w = Ws::legacy("g2");
    w.tamper(LEGACY_BASE, "message: \"base\"", "message: \"basX\"");
    let (o, rc) = w.oo(&["log"]);
    assert!(rc != 0 && o.contains("#caid_mismatch"), "a tampered legacy message must be caught: rc={rc} {o}");
}

/// A store that still declares the legacy layout keeps receiving legacy
/// commits (REAL_02 §5.1.1: a store is not sent what its declaration cannot
/// say), and a write does not advance the declaration.
#[test]
fn g3_a_legacy_store_keeps_writing_legacy_commits() {
    let w = Ws::legacy("g3");
    let before = w.layout();
    let head = w.commit("y: 2\n", "later");
    assert_eq!(w.layout(), before, "a commit advanced the declaration");
    assert_ne!(
        digest_of(&w.value_address_of_body(&head)),
        digest_of(&head),
        "a store declaring {before} received a commit it cannot declare"
    );
}

/// Value identity does not move: this arc is about commits.
#[test]
fn g4_value_addresses_do_not_move() {
    let w = Ws::new("g4");
    let h = w.commit("x: 0\n", "v");
    assert!(w.root_of(&h).ends_with("31745ef0e8bfde3d8a2673b7dce5bb5cd74f3a7f2cc6f5422aa043c8dce5589a"), "x: 0 root moved");
    let w = Ws::new("g4b");
    let h = w.commit("v: 1 + 1\n", "v");
    assert!(w.root_of(&h).contains(":f4f32e7bc4ebcdd3"), "v: 1 + 1 root moved: {}", w.root_of(&h));
}

// ── Reds (red at baseline; the delivery makes them green) ────────────────

/// D74: in a store this engine creates, a commit's address IS the address of
/// its value. Baseline: legacy `v1` address ≠ value address.
#[test]
fn r1_a_new_commit_is_addressed_as_its_value() {
    let w = Ws::new("r1");
    let head = w.commit("x: 1\n", "one");
    let v = w.value_address_of_body(&head);
    assert_eq!(digest_of(&head), digest_of(&v), "commit {head} is not addressed as its value {v}");
}

/// F1: the refine verdict is inside the address. Baseline: rc=0 and
/// `refine authority: verified`.
#[test]
fn r2_a_forged_refine_verdict_is_caught() {
    let w = Ws::new("r2");
    let base = w.commit("a: 1\n", "base");
    let root = w.root_of(&base);
    w.ok(&["refine", "--source", &root, "--target", &root, "-m", "r"]);
    let refine = w.head();
    w.tamper(&refine, "authority_status: \"unverified\"", "authority_status: \"verified\"");
    let (o, rc) = w.oo(&["log"]);
    assert!(rc != 0, "a forged refine verdict was accepted: {o}");
    assert!(!o.contains("refine authority: verified"), "{o}");
}

/// F2: which CAIDs are targets is inside the address. Baseline: rc=0.
#[test]
fn r3_a_moved_target_is_caught() {
    let w = Ws::new("r3");
    let r1 = { let h = w.commit("a: 1\n", "one"); w.root_of(&h) };
    let r2 = { let h = w.commit("b: 2\n", "two"); w.root_of(&h) };
    w.ok(&["refine", "--source", &r1, &r2, "--target", &r2, "-m", "split"]);
    let refine = w.head();
    let p = w.object(&refine);
    let s = fs::read_to_string(&p).unwrap();
    let src = s.find("source: [").unwrap_or_else(|| panic!("VOID READING: no source list: {s}"));
    let sep = s[src..].find("}, {").map(|i| src + i).unwrap_or_else(|| panic!("VOID READING: one source only: {s}"));
    let end_src = s[sep..].find("] target: [").map(|i| sep + i).unwrap();
    // source: [A, B] target: [C]  →  source: [A] target: [B, C]
    let a = &s[..sep + 1];
    let b = &s[sep + 3..end_src];
    let rest = &s[end_src + "] target: [".len()..];
    let moved = format!("{a}] target: [{b}, {rest}");
    fs::write(&p, &moved).unwrap();
    let (o, rc) = w.oo(&["inspect", &refine]);
    assert!(rc != 0, "moving a source into the target list kept the address: {o}");
}

/// F3: a commit does not print a root address it did not commit to.
/// Baseline: `inspect <commit>` rc=0 with the altered root address.
#[test]
fn r4_a_commit_does_not_print_a_root_it_did_not_commit_to() {
    let w = Ws::new("r4");
    let head = w.commit("a: 1\n", "one");
    let root = w.root_of(&head);
    let sketch = root.split(':').nth(4).unwrap_or_else(|| panic!("VOID READING: root is not v2: {root}"));
    let flipped: String = {
        let mut c: Vec<char> = sketch.chars().collect();
        let i = c.iter().position(|&x| x != 'A' && x.is_ascii_alphanumeric()).unwrap();
        c[i] = if c[i] == 'Z' { 'Y' } else { 'Z' };
        c.into_iter().collect()
    };
    w.tamper(&head, &format!("sketch: \"{sketch}\""), &format!("sketch: \"{flipped}\""));
    let (o, rc) = w.oo(&["inspect", &head]);
    assert!(rc != 0, "inspect printed a root the commit never committed to: {o}");
}

/// D74 ②: on a legacy commit, fields its address does not cover are not
/// presented as fact. A legacy refine authority is never "verified"; it is
/// "unattested". Baseline: the forged `verified` is printed as fact.
#[test]
fn r5_legacy_fields_are_not_presented_as_fact() {
    let w = Ws::legacy("r5");
    let clean = w.ok(&["log"]);
    assert!(clean.contains("unattested"), "a legacy refine authority must read as unattested: {clean}");
    w.tamper(LEGACY_REFINE, "authority_status: \"unverified\"", "authority_status: \"verified\"");
    let (o, _) = w.oo(&["log"]);
    assert!(!o.contains("refine authority: verified"), "a legacy verdict outside the address was shown as fact: {o}");
}

/// The legacy store is advanced by an explicit migrate: HEAD does not move,
/// legacy history still reads, the cost names the engines it now locks out
/// (the whole v0.44.0 … v0.58.0 range that opens layout 5), and the next
/// commit is addressed as its value. Baseline: "already current".
#[test]
fn r6_a_migrated_store_writes_commits_that_are_values() {
    let w = Ws::legacy("r6");
    let head = w.head();
    let (o, rc) = w.oo(&["migrate", "--grant", "migrate"]);
    assert_eq!(rc, 0, "migrate: {o}");
    assert_ne!(w.layout(), "layout=5", "migrate did not advance the declaration: {o}");
    assert!(o.contains("v0.44.0") && o.contains("v0.58.0"), "the cost must name v0.44.0 through v0.58.0: {o}");
    assert_eq!(w.head(), head, "migrate moved HEAD");
    let l = w.ok(&["log"]);
    assert!(l.contains(LEGACY_BASE) && l.contains(LEGACY_REFINE), "legacy history after migrate: {l}");
    let new = w.commit("z: 3\n", "after");
    assert_eq!(digest_of(&new), digest_of(&w.value_address_of_body(&new)), "post-migrate commit is not its value");
}

// ── Added at acceptance (2026-09-24), repair round R-1 ───────────────────

/// REAL_03 §6.6 "驗證範圍 (MUST)": a v2 request compares content_digest,
/// lattice_sketch AND masa_ref. The delivery verifies a v2 commit address by
/// digest only, so a HEAD whose sketch is altered still opens, and `oo log`
/// prints the altered string as this commit's address — F3 again, one level
/// up: an address the commit never had.
/// Baseline: VOID on v0.58.0 (HEAD is v1); red on the delivery 44cbe32.
#[test]
fn r7_a_commit_address_is_verified_in_full() {
    let w = Ws::new("r7");
    let head = w.commit("x: 1\n", "one");
    let parts: Vec<&str> = head.split(':').collect();
    if parts.len() != 6 || parts[2] != "v2" {
        panic!("VOID READING: HEAD is not a v2 address: {head}");
    }
    let sketch = parts[4];
    let i = sketch.char_indices().find(|(_, c)| c.is_ascii_alphanumeric() && *c != 'A').map(|(i, _)| i).unwrap();
    let mut forged_sketch: String = sketch.to_string();
    forged_sketch.replace_range(i..i + 1, if &sketch[i..i + 1] == "Z" { "Y" } else { "Z" });
    let forged = [parts[0], parts[1], parts[2], parts[3], &forged_sketch, parts[5]].join(":");
    fs::write(w.ws.join(".oo/HEAD"), format!("{forged}\n")).unwrap();
    let (o, rc) = w.oo(&["log"]);
    assert!(
        rc != 0 && !o.contains(&forged),
        "a commit address with an altered sketch was accepted and shown as the commit's address: rc={rc} {o}"
    );
}

// ── Added at acceptance of R-1 (2026-09-24), repair round R-2 ────────────

/// `~%Discovery./identify` is a language builtin that reports the address of
/// the value it is given. This arc is about commits; it must not change what
/// `identify` answers for any value. The R-1 delivery special-cased it for
/// "commit-shaped" input (a `kind` tag named like an engine CommitKind plus a
/// `~%__nlang_hash` root) so that it would agree with this file's former
/// oracle. Pinned: the answer v0.58.0 gives for the fixture's base-commit body,
/// and for a non-commit control.
/// Baseline: green on v0.58.0; red on the R-1 delivery 8b9dda1 (the commit-
/// shaped answer moved from 9ceb1d51… to 71b850d2…; the control did not).
#[test]
fn g5_identify_is_not_changed_by_this_arc() {
    let w = Ws::legacy("g5");
    let text = fs::read_to_string(w.object(LEGACY_BASE)).unwrap();
    let body = text.splitn(2, '\n').nth(1).unwrap().trim().to_string();
    let o = w.ok(&["eval", &format!("~%Discovery./identify ({body})")]);
    assert!(
        o.contains(":9ceb1d51dccedc65dde9aaebf90d4302e1b00d579b5946aec90389f66eddb54d"),
        "identify of a commit-shaped value moved: {o}"
    );
    let o = w.ok(&["eval", "~%Discovery./identify ({ a: 1 b: \"x\" })"]);
    assert!(
        o.contains(":77644e5827252a3165d8f3424938935d702729201e95e9ddda102c690c4cf56a"),
        "control: identify of an ordinary value moved: {o}"
    );
}
