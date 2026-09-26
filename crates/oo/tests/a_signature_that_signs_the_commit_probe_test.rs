// A signature that signs the commit.
// Order: nlang-tools/docs/a_signature_that_signs_the_commit_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-059.  Ruling: meta/oo/STATUS.md D76.
//
// ── What this arc is ─────────────────────────────────────────────────────
//
// SPEC_10 §2.5 says a refine signature signs "the commit's CAID, computed with
// %authority excluded". Through v0.60.0 the engine signs only
// `refine:` + the sorted source and target CAIDs. Measured 2026-09-26 on the
// v0.60.0 tree: the same signature stays valid, and `oo log` names its signer,
// after (m1) the writer's word is flipped to "verified", (m2) the message is
// changed, (m3) it is transplanted into another repo's unsigned refine with
// the same source and target. m1 is the worst: D75 ② prints the signer's key
// beside a word the signer never signed.
//
// D76 (user, 2026-09-26):
//   甲   an old-form signature that re-verifies is shown for what it signs
//        (sources and targets only), distinguishable from one that signs the
//        commit;
//   ②甲 `authority.timestamp` is removed (it was never persisted; the signed
//        time is the commit's own meta.timestamp, inside the signed value);
//   ③甲 whole-commit signatures only in a `layout=7` store; a store declared
//        older keeps receiving the old form until explicitly migrated
//        (REAL_02 §5.1.1: a store must not hold what the engine that made it
//        cannot verify).
//
// ── The pinned signing target (this is the interop definition) ───────────
//
//   V       = the commit's value with `refine.authority` absent — exactly the
//             body the same commit would have had unsigned;
//   payload = "refine-commit:v1:" ++ CAID(V)      (CAID as its string form)
//   sig     = Ed25519(operator key, payload)
//
// Same shape as REAL_02 §4.2.1 (`oodp-advert:v1:` ++ CAID(body without
// `signature`)). The old form, kept only for reading, is
//   "refine:" ++ sorted source CAIDs joined "|" ++ ":" ++ sorted targets joined "|".
//
// ── Probe integrity ──────────────────────────────────────────────────────
//
// CAID(V) is computed by the store's own value decoder
// (`store_codec::decode_value`) and `content_hash` — the D74 address, the same
// oracle Q-057 and Q-058 use. Signatures are checked with `ring` directly,
// never through the engine's `authority` module. c1/c2 calibrate both
// oracles at both poles without the engine. No red depends on wording: lines
// are compared after the key and the writer's word are normalised away.
//
// `fixtures/layout6_signed_repo` was built by the real v0.60.0 binary (see its
// README); `fixtures/layout5_signed_repo` by the real v0.58.0.
//
// The delivery may NOT edit this file. If a pin is wrong, say so in the
// report. `rustfmt` must not touch it.
//
// Baseline measured 2026-09-26 on dev b0472d7 / oo v0.60.0: see the order.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use ring::signature::{self, Ed25519KeyPair, KeyPair, UnparsedPublicKey};

const NEW_DOMAIN: &str = "refine-commit:v1:";

struct Ws {
    _scratch: nlang_interpreter::ScratchDir,
    root: PathBuf,
    ws: PathBuf,
}

impl Ws {
    fn new(tag: &str) -> Self {
        let scratch = nlang_interpreter::ScratchDir::new(&format!("sig-commit-{tag}"));
        let root = scratch.path().to_path_buf();
        let ws = root.join("ws");
        fs::create_dir_all(&ws).unwrap();
        Ws { _scratch: scratch, root, ws }
    }

    fn fixture(tag: &str, name: &str) -> Self {
        let w = Ws::new(tag);
        let f = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name);
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

    fn layout(&self) -> String {
        fs::read_to_string(self.ws.join(".oo/format")).unwrap().trim().to_string()
    }

    fn object(&self, caid: &str) -> PathBuf {
        let d = caid.rsplit(':').next().unwrap();
        self.ws.join(".oo/objects/sha256").join(&d[..2]).join(&d[2..])
    }

    fn root_of_head(&self) -> String {
        let ins = self.ok(&["inspect", &self.head()]);
        ins.lines().find_map(|l| l.strip_prefix("root:")).unwrap().trim().to_string()
    }

    /// A refine of the current root onto itself; returns `refine`'s output.
    fn refine_here(&self, sign: bool) -> (String, i32) {
        let root = self.root_of_head();
        let mut args = vec!["refine", "--source", &root, "--target", &root, "-m", "r"];
        if sign {
            args.push("--sign");
        }
        self.oo(&args)
    }

    /// A committed `a: 1`, then a refine of its root onto itself. HEAD is the
    /// refine commit; returns the refine command's output.
    fn refine(&self, sign: bool) -> String {
        fs::write(self.ws.join("p.n"), "a: 1\n").unwrap();
        self.ok(&["evolve", "p.n"]);
        self.ok(&["commit", "-m", "base"]);
        let (o, rc) = self.refine_here(sign);
        assert_eq!(rc, 0, "setup `oo refine`: {o}");
        o
    }

    fn head_body(&self) -> String {
        let text = fs::read_to_string(self.object(&self.head())).unwrap();
        text.splitn(2, '\n').nth(1).unwrap().trim().to_string()
    }

    /// Write a self-consistent commit with `body` and point HEAD at it.
    fn mint(&self, body: &str) -> String {
        let caid = value_caid(body);
        let p = self.object(&caid);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, format!("{} commit\n{}", nlang_interpreter::store_codec::FRAME, body)).unwrap();
        fs::write(self.ws.join(".oo/HEAD"), format!("{caid}\n")).unwrap();
        caid
    }

    fn pubkey(&self) -> String {
        self.ok(&["identity"]).lines().next().unwrap().trim().to_string()
    }

    fn line(&self) -> String {
        let (o, rc) = self.oo(&["log"]);
        assert_eq!(rc, 0, "VOID READING: `oo log` did not open: {o}");
        authority_line(&o)
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

/// The D74 address of a commit body, by the store's value decoder.
fn value_caid(body: &str) -> String {
    let doc = format!("{}\n{}", nlang_interpreter::store_codec::FRAME, body);
    let v = nlang_interpreter::store_codec::decode_value(&doc)
        .unwrap_or_else(|e| panic!("VOID READING: the body does not decode: {e}\n{body}"));
    v.content_hash().to_string()
}

/// The body with its `authority: { … }` field removed.
fn strip_authority(body: &str) -> String {
    let at = body.find(" authority: {").unwrap_or_else(|| panic!("VOID READING: no authority: {body}"));
    let end = at + body[at..].find('}').unwrap() + 1;
    format!("{}{}", &body[..at], &body[end..])
}

fn quoted_after<'a>(body: &'a str, key: &str) -> &'a str {
    let at = body.find(key).unwrap_or_else(|| panic!("VOID READING: no `{key}` in: {body}")) + key.len();
    let rest = &body[at..];
    &rest[..rest.find('"').unwrap()]
}

fn stored_sig(body: &str) -> (String, String) {
    (
        quoted_after(body, "signer_pubkey_hex: \"").to_string(),
        quoted_after(body, "signature_hex: \"").to_string(),
    )
}

fn verifies(pk_hex: &str, sig_hex: &str, payload: &[u8]) -> bool {
    let (Ok(pk), Ok(sig)) = (hex::decode(pk_hex), hex::decode(sig_hex)) else {
        return false;
    };
    UnparsedPublicKey::new(&signature::ED25519, pk).verify(payload, &sig).is_ok()
}

fn new_payload(body: &str) -> Vec<u8> {
    format!("{NEW_DOMAIN}{}", value_caid(&strip_authority(body))).into_bytes()
}

fn old_payload(source: &str, target: &str) -> Vec<u8> {
    format!("refine:{source}:{target}").into_bytes()
}

/// Does the stored signature sign the whole commit (the pinned definition)?
fn signs_the_commit(body: &str) -> bool {
    let (pk, sig) = stored_sig(body);
    verifies(&pk, &sig, &new_payload(body))
}

/// Sign an unsigned refine body per the pinned definition with a fresh key.
fn sign_as_pinned(unsigned: &str) -> (String, String) {
    let rng = ring::rand::SystemRandom::new();
    let pkcs8 = Ed25519KeyPair::generate_pkcs8(&rng).unwrap();
    let kp = Ed25519KeyPair::from_pkcs8(pkcs8.as_ref()).unwrap();
    let payload = format!("{NEW_DOMAIN}{}", value_caid(unsigned));
    let pk = hex::encode(kp.public_key().as_ref());
    let sig = hex::encode(kp.sign(payload.as_bytes()).as_ref());
    let at = unsigned.rfind(" } }").unwrap_or_else(|| panic!("VOID READING: not a refine body: {unsigned}"));
    let body = format!(
        "{} authority: {{ signer_pubkey_hex: \"{pk}\" signature_hex: \"{sig}\" }}{}",
        &unsigned[..at],
        &unsigned[at..]
    );
    (body, pk)
}

/// A line with the signer's key and the writer's word normalised away.
fn shape(line: &str, key: &str) -> String {
    line.replace(key, "<key>")
        .replace("writer recorded: unverified", "writer recorded: <word>")
        .replace("writer recorded: verified", "writer recorded: <word>")
}

/// The same, with the writer's-word suffix removed too (for legacy lines,
/// which never carry it — D74 ②).
fn bare_shape(line: &str, key: &str) -> String {
    let s = shape(line, key);
    match s.find(" (writer recorded: <word>)") {
        Some(at) => format!("{}{}", &s[..at], &s[at + " (writer recorded: <word>)".len()..]),
        None => s,
    }
}

const L6_SIGNER: &str = "8047eac6051c05302ca7a99433f70bd5e029609f63b08c1f3f0d7d9361bc0b23";
const L5_SIGNER: &str = "79b908e3eb5fc19f5f6645906765ae76e5c2963950c4753c8c9c5b05e6616de4";

// ── Controls: the two oracles, both poles, no engine in the loop ─────────

/// The new-form oracle accepts a signature made per the pinned definition and
/// rejects it once one byte of the signed value changes.
#[test]
fn c1_the_whole_commit_oracle_has_two_poles() {
    let w = Ws::new("c1");
    w.refine(false);
    let (body, _) = sign_as_pinned(&w.head_body());
    assert!(signs_the_commit(&body), "the oracle rejects a signature made by its own definition");
    let altered = replace_once(&body, "message: \"r\"", "message: \"s\"");
    assert!(!signs_the_commit(&altered), "the oracle accepts a signature over a different commit");
}

/// The old-form oracle accepts the v0.60.0 fixture's real signature and
/// rejects it against a different target.
#[test]
fn c2_the_old_form_oracle_has_two_poles() {
    let w = Ws::fixture("c2", "layout6_signed_repo");
    let root = w.root_of_head();
    let (pk, sig) = stored_sig(&w.head_body());
    assert_eq!(pk, L6_SIGNER, "VOID READING: not the fixture's signer");
    assert!(verifies(&pk, &sig, &old_payload(&root, &root)), "the old-form oracle rejects a real v0.60.0 signature");
    let other = root.replace(':', ";");
    assert!(!verifies(&pk, &sig, &old_payload(&root, &other)), "the old-form oracle accepts anything");
}

// ── Guards (green on v0.60.0, must stay green) ───────────────────────────

/// A genuine signed refine still names its signer, in `refine` and in `log`.
#[test]
fn g1_a_genuine_signature_still_names_its_signer() {
    let w = Ws::new("g1");
    let out = w.refine(true);
    let key = w.pubkey();
    assert!(out.contains(&key[..16]), "`oo refine --sign` does not say who signed: {out}");
    assert!(w.line().contains(&key[..16]), "`oo log` does not say who signed");
}

/// The v0.60.0 fixture's old-form signature still re-verifies and names its
/// signer (D76 甲: shown for what it signs, not dropped).
#[test]
fn g2_an_old_form_signature_is_still_shown() {
    let w = Ws::fixture("g2", "layout6_signed_repo");
    assert!(w.line().contains(&L6_SIGNER[..16]), "an old-form signature lost its signer: {}", w.line());
}

/// The legacy fixture's signature still names its signer (Q-058 r4, D76 甲).
#[test]
fn g3_a_legacy_signature_is_still_shown() {
    let w = Ws::fixture("g3", "layout5_signed_repo");
    assert!(w.line().contains(&L5_SIGNER[..16]), "a legacy signature lost its signer: {}", w.line());
}

// ── Reds ─────────────────────────────────────────────────────────────────

/// A fresh store's `refine --sign` signs the whole commit, per the pinned
/// definition. Baseline: the stored signature is old-form.
#[test]
fn r1_a_signature_signs_the_commit() {
    let w = Ws::new("r1");
    w.refine(true);
    let body = w.head_body();
    assert!(signs_the_commit(&body), "the stored signature does not sign the commit: {body}");
}

/// m1: flip the writer's word; the signature must not be shown as it was.
/// Baseline: `refine authority: <key> (writer recorded: verified)`.
#[test]
fn r2_a_flipped_word_does_not_keep_its_signer() {
    let w = Ws::new("r2");
    w.refine(true);
    let key = w.pubkey();
    let genuine = w.line();
    let body = w.head_body();
    let (from, to) = if body.contains("authority_status: \"unverified\"") {
        ("authority_status: \"unverified\"", "authority_status: \"verified\"")
    } else {
        ("authority_status: \"verified\"", "authority_status: \"unverified\"")
    };
    w.mint(&replace_once(&body, from, to));
    let t = w.line();
    assert_ne!(shape(&t, &key), shape(&genuine, &key), "a word the signer never signed is shown under their key: {t}");
}

/// m2: change the message; the signature must not be shown as it was.
#[test]
fn r3_a_changed_commit_does_not_keep_its_signer() {
    let w = Ws::new("r3");
    w.refine(true);
    let key = w.pubkey();
    let genuine = w.line();
    w.mint(&replace_once(&w.head_body(), "message: \"r\"", "message: \"someone else wrote this\""));
    let t = w.line();
    assert_ne!(shape(&t, &key), shape(&genuine, &key), "a changed commit keeps the signer's endorsement: {t}");
}

/// m3: transplant a signature into another repo's unsigned refine with the
/// same source and target.
#[test]
fn r4_a_transplanted_signature_does_not_verify() {
    let a = Ws::new("r4a");
    a.refine(true);
    let key = a.pubkey();
    let genuine = a.line();
    let signed = a.head_body();
    let at = signed.find(" authority: {").unwrap();
    let auth = &signed[at..at + signed[at..].find('}').unwrap() + 1];

    let b = Ws::new("r4b");
    b.refine(false);
    let unsigned = b.head_body();
    let end = unsigned.rfind(" } }").unwrap_or_else(|| panic!("VOID READING: not a refine body: {unsigned}"));
    b.mint(&format!("{}{}{}", &unsigned[..end], auth, &unsigned[end..]));
    let t = b.line();
    assert_ne!(shape(&t, &key), shape(&genuine, &key), "a signature moved into another commit still verifies: {t}");
}

/// A reader verifies a whole-commit signature it did not write: a body signed
/// per the pinned definition by a key the engine has never seen.
/// Baseline: `signature did not verify`.
#[test]
fn r5_a_reader_verifies_the_pinned_form() {
    let w = Ws::new("r5");
    w.refine(false);
    let (body, pk) = sign_as_pinned(&w.head_body());
    w.mint(&body);
    let t = w.line();
    assert!(t.contains(&pk[..16]), "a signature made per the definition is not shown as whose: {t}");
}

/// D76 甲: an old-form signature on a new-form commit is shown differently
/// from a whole-commit one. Baseline: both lines have the same shape.
#[test]
fn r6_old_form_and_whole_commit_can_be_told_apart() {
    let old = Ws::fixture("r6a", "layout6_signed_repo");
    let fresh = Ws::new("r6b");
    fresh.refine(true);
    let a = shape(&old.line(), L6_SIGNER);
    let b = shape(&fresh.line(), &fresh.pubkey());
    assert_ne!(a, b, "a signature over sources and targets reads like one over the commit: {a} / {b}");
}

/// D76 甲 on a legacy commit: its old-form signature is not shown the way a
/// whole-commit signature is (the writer's-word suffix, which D74 ② keeps off
/// legacy lines, is removed before comparing). Baseline: identical.
#[test]
fn r7_a_legacy_signature_is_shown_for_what_it_signs() {
    let old = Ws::fixture("r7a", "layout5_signed_repo");
    let fresh = Ws::new("r7b");
    fresh.refine(true);
    let a = bare_shape(&old.line(), L5_SIGNER);
    let b = bare_shape(&fresh.line(), &fresh.pubkey());
    assert_ne!(a, b, "a legacy signature reads like one over the commit: {a} / {b}");
}

/// D76 ③: an explicit migrate advances a `layout=6` store to what a fresh
/// store declares, without moving HEAD, and the cost names the engines it
/// locks out (v0.59.0 through v0.60.0 open layout 6). Baseline: "already
/// current".
#[test]
fn r8_a_layout6_store_is_migrated_on_request() {
    let w = Ws::fixture("r8", "layout6_signed_repo");
    let fresh = Ws::new("r8f");
    fresh.refine(false);
    let head = w.head();
    let (o, rc) = w.oo(&["migrate", "--grant", "migrate"]);
    assert_eq!(rc, 0, "migrate: {o}");
    assert_eq!(w.layout(), fresh.layout(), "migrate did not reach the declaration a fresh store has: {o}");
    assert_ne!(w.layout(), "layout=6", "a fresh store still declares layout=6: {o}");
    assert!(o.contains("v0.59.0") && o.contains("v0.60.0"), "the cost must name v0.59.0 through v0.60.0: {o}");
    assert_eq!(w.head(), head, "migrate moved HEAD");
    assert!(w.line().contains(&L6_SIGNER[..16]), "the old-form signature stopped reading after migrate");
}

/// D76 ③: a migrated store receives whole-commit signatures.
#[test]
fn r9_a_migrated_store_signs_the_commit() {
    let w = Ws::fixture("r9", "layout6_signed_repo");
    let (o, rc) = w.oo(&["migrate", "--grant", "migrate"]);
    assert_eq!(rc, 0, "migrate: {o}");
    let (o, rc) = w.refine_here(true);
    assert_eq!(rc, 0, "refine after migrate: {o}");
    let body = w.head_body();
    assert!(signs_the_commit(&body), "a migrated store still receives the old form: {body}");
}

/// D76 ③ / REAL_02 §5.1.1: an unmigrated `layout=6` store keeps receiving the
/// old form, keeps its declaration, and `refine --sign` names the migrate
/// that would change that. Baseline: everything but the last holds.
#[test]
fn r10_an_unmigrated_store_keeps_the_old_form_and_says_so() {
    let w = Ws::fixture("r10", "layout6_signed_repo");
    let root = w.root_of_head();
    let (o, rc) = w.refine_here(true);
    assert_eq!(rc, 0, "refine in a layout=6 store: {o}");
    assert_eq!(w.layout(), "layout=6", "a write changed the declaration it found");
    let body = w.head_body();
    let (pk, sig) = stored_sig(&body);
    assert!(verifies(&pk, &sig, &old_payload(&root, &root)), "a layout=6 store received a form it cannot declare: {body}");
    assert!(o.contains("migrate"), "`refine --sign` in a layout=6 store does not name the migrate: {o}");
}

// ── Added at acceptance (2026-09-26), repair round R-1 ────────────────────

/// I4 for the one start state D76 adds: migrating a `layout=6` store locks out
/// only engines that opened it before, v0.59.0 and v0.60.0. The delivered
/// sentence goes on: "That includes every engine that opens layout=5 (oo
/// v0.44.0 through v0.60.0)" — v0.44.0…v0.58.0 never opened layout=6, so this
/// migrate does not lock them out, and the sentence says it does. r8 asked
/// only that v0.59.0 and v0.60.0 appear, so it passed. Predicate: every
/// release the sentence names is v0.59.0 or newer.
/// Baseline: green on v0.60.0 ("already current", names none); red on 6da0e09.
#[test]
fn r11_a_layout6_migrate_names_no_engine_it_does_not_lock_out() {
    let w = Ws::fixture("r11", "layout6_signed_repo");
    let (o, rc) = w.oo(&["migrate", "--grant", "migrate"]);
    assert_eq!(rc, 0, "migrate: {o}");
    let mut named = Vec::new();
    for (i, _) in o.match_indices("v0.") {
        let minor: String = o[i + 3..].chars().take_while(|c| c.is_ascii_digit()).collect();
        if let Ok(n) = minor.parse::<u32>() {
            named.push(n);
        }
    }
    assert!(
        named.iter().all(|&n| n >= 59),
        "the cost names engines that never opened layout=6: {o}"
    );
}
