// A verdict only a signature can carry.
// Order: nlang-tools/docs/a_verdict_only_a_signature_can_carry_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-058.  Ruling: meta/oo/STATUS.md D75.
//
// ── What this arc is ─────────────────────────────────────────────────────
//
// A refine commit carries a word, `authority_status: "verified"` or
// `"unverified"`, written by whoever wrote the commit. Since v0.59.0 (D74) the
// word is inside a new commit's address, so it cannot be ALTERED afterwards —
// but anyone who can write `.oo` can MINT a brand-new commit whose address is
// self-consistent and whose word says "verified". Hashing stops tampering, not
// fabrication. `oo log` prints the word as fact.
//
// D75 (user, "甲"): the only authority a reader presents is the stored
// signature, re-verified at read time against the payload the address
// already covers. A valid signature is shown as WHOSE it is; the stored word
// is not presented. Membership in the architect registry at the time of
// writing is a historical fact a reader cannot re-check, so it is not claimed.
// On a legacy commit (D74 ②), re-verifying a stored signature is a new fact the
// reader computes, not the unattested word, so it may be shown.
//
// ── Probe integrity ──────────────────────────────────────────────────────
//
// `mint` writes a self-consistent commit: the body is decoded by the store's
// own value decoder (`store_codec::decode_value`) and addressed by
// `content_hash`, the D74 address; `c2` shows a minted commit opens and reads.
// No red depends on a particular wording except `r2`/`r4`, which require the
// signer's public key to appear (that is what "whose it is" means).
//
// The delivery may NOT edit this file. If a pin is wrong, say so in the
// report. `rustfmt` must not touch it.
//
// Baseline measured 2026-09-25 on dev 0d50fef / oo v0.59.0: see the order.
// r5 added at acceptance (repair round R-1, D75 ②): green on v0.59.0, red on
// the delivery 1f52139; see the order §9.

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
        let scratch = nlang_interpreter::ScratchDir::new(&format!("sig-verdict-{tag}"));
        let root = scratch.path().to_path_buf();
        let ws = root.join("ws");
        fs::create_dir_all(&ws).unwrap();
        Ws { _scratch: scratch, root, ws }
    }

    fn legacy_signed(tag: &str) -> Self {
        let w = Ws::new(tag);
        let f = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/layout5_signed_repo");
        copy_tree(&f.join("oo_dir"), &w.ws.join(".oo"));
        w
    }

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

    fn object(&self, caid: &str) -> PathBuf {
        let d = caid.rsplit(':').next().unwrap();
        self.ws.join(".oo/objects/sha256").join(&d[..2]).join(&d[2..])
    }

    /// A committed `a: 1`, then a refine of its root onto itself. Returns the
    /// refine command's output; HEAD is the refine commit.
    fn refine(&self, sign: bool) -> String {
        fs::write(self.ws.join("p.n"), "a: 1\n").unwrap();
        self.ok(&["evolve", "p.n"]);
        self.ok(&["commit", "-m", "base"]);
        let ins = self.ok(&["inspect", &self.head()]);
        let root = ins.lines().find_map(|l| l.strip_prefix("root:")).unwrap().trim().to_string();
        let mut args = vec!["refine", "--source", &root, "--target", &root, "-m", "r"];
        if sign {
            args.push("--sign");
        }
        self.ok(&args)
    }

    /// The body of the commit HEAD points to.
    fn head_body(&self) -> String {
        let text = fs::read_to_string(self.object(&self.head())).unwrap();
        text.splitn(2, '\n').nth(1).unwrap().trim().to_string()
    }

    /// Write a self-consistent commit with `body` and point HEAD at it.
    fn mint(&self, body: &str) -> String {
        let doc = format!("{}\n{}", nlang_interpreter::store_codec::FRAME, body);
        let v = nlang_interpreter::store_codec::decode_value(&doc)
            .unwrap_or_else(|e| panic!("VOID READING: the minted body does not decode: {e}"));
        let caid = v.content_hash().to_string();
        let p = self.object(&caid);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, format!("{} commit\n{}", nlang_interpreter::store_codec::FRAME, body)).unwrap();
        fs::write(self.ws.join(".oo/HEAD"), format!("{caid}\n")).unwrap();
        caid
    }

    fn pubkey(&self) -> String {
        self.ok(&["identity"]).lines().next().unwrap().trim().to_string()
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

fn replace_once(s: &str, from: &str, to: &str) -> String {
    if s.matches(from).count() != 1 {
        panic!("VOID READING: expected exactly one `{from}` in: {s}");
    }
    s.replacen(from, to, 1)
}

fn authority_line(log: &str) -> String {
    log.lines().find(|l| l.trim_start().starts_with("refine authority")).unwrap_or("").trim().to_string()
}

const LEGACY_SIGNER: &str = "79b908e3eb5fc19f5f6645906765ae76e5c2963950c4753c8c9c5b05e6616de4";

// ── Controls and guards ──────────────────────────────────────────────────

/// A signed refine stores the operator's public key and a signature.
#[test]
fn c1_a_signed_refine_stores_its_signature() {
    let w = Ws::new("c1");
    w.refine(true);
    let body = w.head_body();
    let key = w.pubkey();
    assert!(body.contains(&format!("signer_pubkey_hex: \"{key}\"")), "no signer in the stored refine: {body}");
    assert!(body.contains("signature_hex: \""), "no signature in the stored refine: {body}");
}

/// The minting helper reaches the engine: a minted commit (a genuine body with
/// a different message) opens, and `log` shows the new message.
#[test]
fn c2_a_minted_commit_opens() {
    let w = Ws::new("c2");
    w.refine(false);
    let body = replace_once(&w.head_body(), "message: \"r\"", "message: \"minted\"");
    let caid = w.mint(&body);
    let o = w.ok(&["log"]);
    assert!(o.contains(&caid) && o.contains("message: minted"), "a minted commit must open: {o}");
}

/// The stored word on a legacy commit is still not fact (D74 ②).
#[test]
fn g1_a_legacy_verdict_word_is_still_not_fact() {
    let w = Ws::legacy_signed("g1");
    let head = w.head();
    let p = w.object(&head);
    let s = fs::read_to_string(&p).unwrap();
    fs::write(&p, replace_once(&s, "authority_status: \"unverified\"", "authority_status: \"verified\"")).unwrap();
    let (o, _) = w.oo(&["log"]);
    assert!(!o.contains("refine authority: verified"), "{o}");
}

/// A signer outside a non-empty architect registry is still refused at write.
#[test]
fn g2_an_unregistered_signer_is_still_refused() {
    let w = Ws::new("g2");
    fs::write(w.ws.join("p.n"), "a: 1\n").unwrap();
    w.ok(&["evolve", "p.n"]);
    w.ok(&["commit", "-m", "base"]);
    fs::write(w.ws.join(".oo/architects.json"), format!("[\"{}\"]", "0".repeat(64))).unwrap();
    let ins = w.ok(&["inspect", &w.head()]);
    let root = ins.lines().find_map(|l| l.strip_prefix("root:")).unwrap().trim().to_string();
    let (o, rc) = w.oo(&["refine", "--source", &root, "--target", &root, "-m", "r", "--sign"]);
    assert!(rc != 0 && o.contains("not in architect_registry"), "an unregistered signer must be refused: rc={rc} {o}");
}

// ── Reds ─────────────────────────────────────────────────────────────────

/// Fabrication: a minted, address-consistent refine whose stored word says
/// "verified" and which carries no signature. Baseline: `log` prints
/// `refine authority: verified`.
#[test]
fn r1_a_minted_verdict_without_a_signature_is_not_shown() {
    let w = Ws::new("r1");
    w.refine(false);
    let body = replace_once(&w.head_body(), "authority_status: \"unverified\"", "authority_status: \"verified\"");
    w.mint(&body);
    let (o, rc) = w.oo(&["log"]);
    assert_eq!(rc, 0, "VOID READING: the minted commit did not open: {o}");
    assert!(!o.contains("refine authority: verified"), "a verdict nobody signed was shown as fact: {o}");
}

/// A valid signature is shown as whose it is, in `log` and in `refine`'s own
/// output. Baseline: both say `unverified` and never name the signer.
#[test]
fn r2_a_valid_signature_is_shown_as_whose() {
    let w = Ws::new("r2");
    let out = w.refine(true);
    let key = w.pubkey();
    let short = &key[..16];
    assert!(out.contains(short), "`oo refine --sign` does not say who signed: {out}");
    let log = w.ok(&["log"]);
    assert!(authority_line(&log).contains(short), "`oo log` does not say who signed: {log}");
}

/// A broken signature is not shown the way a valid one is. The minted commit
/// is the genuine signed refine with one signature digit changed, re-addressed
/// so the address is consistent. Baseline: both authority lines read
/// `refine authority: unverified`, identically.
#[test]
fn r3_a_broken_signature_is_not_shown_as_valid() {
    let w = Ws::new("r3");
    w.refine(true);
    let genuine = authority_line(&w.ok(&["log"]));
    let body = w.head_body();
    let at = body.find("signature_hex: \"").unwrap_or_else(|| panic!("VOID READING: no signature: {body}")) + "signature_hex: \"".len();
    let c = &body[at..at + 1];
    let flipped = if c == "0" { "1" } else { "0" };
    let broken = format!("{}{}{}", &body[..at], flipped, &body[at + 1..]);
    w.mint(&broken);
    let (o, rc) = w.oo(&["log"]);
    assert_eq!(rc, 0, "VOID READING: the minted commit did not open: {o}");
    assert_ne!(authority_line(&o), genuine, "a broken signature is shown exactly like a valid one: {o}");
}

/// D75 on a legacy commit: its signature is re-verified at read time and
/// shown as whose it is — a fact the reader computes, not the unattested word.
/// Baseline: `refine authority: unattested`, no signer.
#[test]
fn r4_a_legacy_signature_is_reverified() {
    let w = Ws::legacy_signed("r4");
    let log = w.ok(&["log"]);
    assert!(authority_line(&log).contains(&LEGACY_SIGNER[..16]), "a legacy signature was not re-verified: {log}");
}

// ── Added at acceptance (2026-09-25), repair round R-1 (D75 ②) ───────────

/// SPEC_10 §2.5 "未驗證必須留痕" (MUST, 2026-07-27): a refine signed while the
/// whitelist is empty ("somebody signed") must be distinguishable by
/// inspection from one verified against a non-empty whitelist ("an
/// authorised key signed"). D75 as delivered prints only the signer for both.
/// D75 ② (user, "甲"): on a new-form commit the writer's recorded word is in
/// the address, so it is shown beside the re-verified signer, attributed to
/// the writer — not as fact, but distinguishable.
/// Baseline: green on v0.59.0 (the two lines differ); red on 1f52139.
#[test]
fn r5_signed_by_anyone_and_signed_by_an_architect_can_be_told_apart() {
    let exempt = Ws::new("r5a");
    exempt.refine(true);
    let a = authority_line(&exempt.ok(&["log"]));

    let listed = Ws::new("r5b");
    fs::write(listed.ws.join("p.n"), "a: 1\n").unwrap();
    listed.ok(&["evolve", "p.n"]);
    listed.ok(&["commit", "-m", "base"]);
    let key = listed.pubkey();
    fs::write(listed.ws.join(".oo/architects.json"), format!("[\"{key}\"]")).unwrap();
    let ins = listed.ok(&["inspect", &listed.head()]);
    let root = ins.lines().find_map(|l| l.strip_prefix("root:")).unwrap().trim().to_string();
    let out = listed.ok(&["refine", "--source", &root, "--target", &root, "-m", "r", "--sign"]);
    if out.contains("unverified") {
        panic!("VOID READING: the listed signer was not verified at write time: {out}");
    }
    let b = authority_line(&listed.ok(&["log"]));

    let strip = |l: &str, k: &str| l.replace(k, "<key>");
    let ka = exempt.pubkey();
    assert_ne!(
        strip(&a, &ka),
        strip(&b, &key),
        "`oo log` cannot tell 'somebody signed' from 'an architect signed': {a} / {b}"
    );
}
