// The operator gets a name (2026-07-27, pre-committed by work order:
// docs/identity_persistence_handover.md).
//
// ── The headline, measured on v0.2.45 ────────────────────────────────────
//
// Three processes, ONE workspace, three signatures:
//
//   91f3ff6e…   7b2cf863…   10132278…
//
// The engine mints `Identity::new_random()` at every `Ouroboros::init` and
// never writes it anywhere. `.oo/` holds `HEAD` and `objects` and no key
// material at all. So the signer of a `#refine` is a party that exists for
// the duration of one process and is never seen again.
//
// ── The door with no key ─────────────────────────────────────────────────
// v0.2.45 removed the engine's self-appointment, which was correct, and in
// doing so revealed that the honest configuration is unreachable. Measured
// on a repository with a HEAD:
//
//   architects.json = [pubkey observed from process 1]
//     `oo refine --sign`   →  Error: signer 9cb94f64… not in architect_registry
//     `oo refine`          →  Error: missing %authority on non-bootstrap refine
//
// Both directions refused. There is no value you can write into that file
// that this engine will ever present, because it presents a different one
// every time. SPEC_10 §93's 權威判定 branch has never once been satisfied:
// before v0.2.45 self-appointment made it always true, after v0.2.45 having
// no key makes it always false. The only reachable configuration is the
// empty whitelist, where everything is `unverified`.
//
// A whitelist that cannot be satisfied is not a stricter check than one that
// cannot fail. It is the same check.
//
// ── The builtin that persistence would arm ───────────────────────────────
// `~%Official./sign_refine` is reachable from an ordinary n/ program with no
// privilege flag, and returns a real Ed25519 signature by the engine's key,
// effect `#io`. Measured: three plain `oo eval` invocations, three
// signatures, no grant of any kind.
//
// Today that is harmless only because the key is worthless. `~%Official` has
// exactly one key and the language has NO interface that constructs a refine
// commit, so the builtin's only possible use is to hand the signature to the
// program — which can write it to a file or send it over a socket. The
// moment the key becomes stable and declared, that is "any n/ program,
// including one fetched from a peer, can obtain an architect's signature
// authorising an arbitrary CAID → CAID redirect".
//
// Retirement follows the v0.2.42 precedent (`/add_architect`): the language
// surface must not own the refine trust root. Its cost is real and is paid
// here — `/sign_refine` was the module-liveness control for TWO existing
// probes, so this file's author (the acceptor) rewrites those controls to
// assert `~%Official` itself, and says so out loud rather than leaving a
// false red for the delivery to trip over.
//
// ── Where a secret lives ─────────────────────────────────────────────────
// Discussion 025 split `.oo/` into self-authenticating OBJECTS (verify, no
// permissions) and asserted POINTERS (authenticate, nothing to verify
// against). A private key is neither. What it needs is concealment, and
// concealment is the one property the CAID/lattice framework has nothing to
// say about. So the question is not which subdirectory of `.oo/` — it is
// whether the key belongs in that tree at all, and it does not: `.oo/`
// is the thing that gets served to peers and copied with the repository.
// Relying on "the serve path happens not to read that file" is exactly the
// reasoning v0.2.44 punished when `remote_fetch` never recomputed an address.
//
// Ruling: the identity is the OPERATOR's, at `~/.oo/identity`, overridable
// by `OO_IDENTITY` for compartmentalisation. ORDER_01 §7.1 agrees — the
// trust root is a set of `@Voter`s carrying `weight` and a human `alias`.
// That is a person. Not a process, and not a workspace.
//
// ── Why minting is allowed when appointing was not ───────────────────────
// v0.2.45 forbade the engine from minting AUTHORITY. A keypair is a NAME,
// and names are self-minted in n/ — a CAID needs nobody's permission either.
// Authority arrives by declaration, and declaration is always out of band.
// The engine may make names; it may not make declarations. Same degree
// separation, one layer up.
//
// ── Anti-vacuity ─────────────────────────────────────────────────────────
// Inherited from the previous arc and not relaxed: every comparison first
// proves both sides well-formed and non-empty, and every red first proves
// the operation under test actually happened. R5 and R7 in particular create
// the file they are about, so that neither can pass by the file's absence.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

const SRC: &str = "v: { hello: \"world\" }\n";

// ── harness ─────────────────────────────────────────────────────────────

fn fresh_dir(tag: &str) -> PathBuf {
    let mut d = std::env::temp_dir();
    d.push(format!(
        "nlang-ident-{}-{}-{}",
        tag,
        std::process::id(),
        DIR_SEQ.fetch_add(1, Ordering::SeqCst)
    ));
    fs::remove_dir_all(&d).ok();
    fs::create_dir_all(&d).unwrap();
    d
}

/// An identity-file path OUTSIDE any `.oo` component, so that protection of
/// it can never be an accident of the v0.2.42 store boundary matching the
/// literal name `.oo`.
fn ident_path(tag: &str) -> PathBuf {
    fresh_dir(tag).join("operator").join("key")
}

struct Run {
    out: String,
    ok: bool,
}

impl Run {
    fn has(&self, s: &str) -> bool {
        self.out.contains(s)
    }
}

/// Every invocation in this file pins `OO_IDENTITY`, so the suite can never
/// read or write the developer's real `~/.oo/`.
fn oo(dir: &Path, ident: &Path, args: &[&str]) -> Run {
    let out = Command::new(env!("CARGO_BIN_EXE_oo"))
        .args(args)
        .current_dir(dir)
        .env("OO_IDENTITY", ident)
        .output()
        .unwrap();
    Run {
        out: format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout).trim(),
            String::from_utf8_lossy(&out.stderr).trim()
        ),
        ok: out.status.success(),
    }
}

/// A repository with `SRC` evolved and committed, so refines are not
/// bootstrap-exempt by way of a missing HEAD.
fn repo(tag: &str, ident: &Path) -> PathBuf {
    let d = fresh_dir(tag);
    fs::write(d.join("s.n"), SRC).unwrap();
    oo(&d, ident, &["evolve", "s.n"]);
    let r = oo(&d, ident, &["commit", "-m", "x"]);
    assert!(r.has("Commit successful"), "repo() failed to commit: {}", r.out);
    d
}

fn caid(seed: char) -> String {
    format!("hash:sha256:v1:{}", std::iter::repeat(seed).take(64).collect::<String>())
}

fn object_path(dir: &Path, c: &str) -> PathBuf {
    let d = c.rsplit(':').next().unwrap();
    dir.join(".oo")
        .join("objects")
        .join("sha256")
        .join(&d[..2])
        .join(&d[2..])
}

/// `oo refine --sign` in its own process. Returns the run and, when it
/// succeeded, the commit CAID it printed.
fn refine_signed(dir: &Path, ident: &Path, s: char, t: char, msg: &str) -> (Run, Option<String>) {
    let (sc, tc) = (caid(s), caid(t));
    let r = oo(dir, ident, &["refine", "-s", &sc, "-t", &tc, "--sign", "-m", msg]);
    let commit = r
        .out
        .lines()
        .find_map(|l| l.strip_prefix("Refine commit: ").map(|x| x.trim().to_string()));
    (r, commit)
}

/// The signer recorded IN the commit object — not what any CLI claims.
/// Panics rather than returning a value that could compare equal to another
/// failure; a comparison that cannot fail has not been made.
fn signer_of(dir: &Path, commit: &str) -> String {
    let p = object_path(dir, commit);
    let j: serde_json::Value =
        serde_json::from_slice(&fs::read(&p).unwrap_or_else(|e| panic!("{p:?}: {e}"))).unwrap();
    let hex = j["refine_info"]["authority"]["signer_pubkey_hex"]
        .as_str()
        .unwrap_or_else(|| panic!("commit {commit} carries no signer_pubkey_hex: {j}"))
        .to_string();
    assert!(
        hex.len() == 64 && hex.chars().all(|c| c.is_ascii_hexdigit()),
        "signer is not a 64-hex ed25519 public key: {hex:?}"
    );
    hex
}

fn head_commit(dir: &Path, ident: &Path) -> String {
    let c = oo(dir, ident, &["log"])
        .out
        .lines()
        .find_map(|l| l.strip_prefix("commit ").map(|s| s.trim().to_string()))
        .unwrap_or_default();
    assert!(c.starts_with("hash:sha256:"), "no HEAD commit in {dir:?}");
    c
}

/// Hex digest of the universe root at HEAD. Key is `digest`; anything else
/// is a harness failure and panics.
fn root_digest(dir: &Path, ident: &Path) -> String {
    let p = object_path(dir, &head_commit(dir, ident));
    let j: serde_json::Value =
        serde_json::from_slice(&fs::read(&p).unwrap_or_else(|e| panic!("{p:?}: {e}"))).unwrap();
    let dg = &j["root"]["digest"];
    let hex = if let Some(s) = dg.as_str() {
        s.to_string()
    } else if let Some(a) = dg.as_array() {
        a.iter()
            .map(|b| format!("{:02x}", b.as_u64().expect("digest byte")))
            .collect()
    } else {
        panic!("commit root has no usable `digest` field: {}", j["root"]);
    };
    assert!(
        hex.len() == 64 && hex.chars().all(|c| c.is_ascii_hexdigit()),
        "root digest is not a 64-hex string: {hex:?}"
    );
    hex
}

fn provision(dir: &Path, keys: &[&str]) {
    let list = keys
        .iter()
        .map(|k| format!("\"{k}\""))
        .collect::<Vec<_>>()
        .join(",");
    fs::write(dir.join(".oo").join("architects.json"), format!("[{list}]")).unwrap();
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES
// ─────────────────────────────────────────────────────────────────────────

/// R1 — the headline. Two processes, one workspace, one signer.
///
/// Independent of any new CLI on purpose: the pubkey is read out of the
/// commit objects, which exist at baseline too, so this gate measures
/// persistence and not the existence of `oo identity`.
#[test]
fn red_identity_is_stable_across_processes() {
    let ident = ident_path("r1");
    let d = repo("r1", &ident);

    let (ra, ca) = refine_signed(&d, &ident, 'a', 'b', "one");
    let (rb, cb) = refine_signed(&d, &ident, 'c', 'd', "two");

    // Anti-vacuity: both operations must actually have happened, or the
    // comparison below is between two absences.
    let ca = ca.unwrap_or_else(|| panic!("first refine did not run: {}", ra.out));
    let cb = cb.unwrap_or_else(|| panic!("second refine did not run: {}", rb.out));
    assert_ne!(ca, cb, "the two refines produced the same commit");

    let a = signer_of(&d, &ca);
    let b = signer_of(&d, &cb);
    assert_eq!(
        a, b,
        "two processes in one workspace signed as two different parties"
    );
}

/// R2 — the key is persisted, at the operator path, and nowhere in `.oo/`.
#[test]
fn red_identity_persists_at_the_operator_path_and_not_in_the_workspace() {
    let ident = ident_path("r2");
    let d = repo("r2", &ident);

    let (r, c) = refine_signed(&d, &ident, 'a', 'b', "sign");
    c.unwrap_or_else(|| panic!("refine did not run, so nothing was signed: {}", r.out));

    assert!(
        ident.exists(),
        "signing did not persist an identity at {ident:?}"
    );
    let bytes = fs::read(&ident).unwrap();
    assert!(!bytes.is_empty(), "identity file is empty");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(&ident).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "identity file is world-readable: {mode:o}");
    }

    // And no copy of it inside the tree that gets served and copied.
    let mut found = Vec::new();
    walk(&d.join(".oo"), &mut |p| {
        if fs::read(p).map(|b| b == bytes).unwrap_or(false) {
            found.push(p.to_path_buf());
        }
    });
    assert!(
        found.is_empty(),
        "private key material found inside .oo/: {found:?}"
    );
}

fn walk(dir: &Path, f: &mut dyn FnMut(&Path)) {
    if let Ok(rd) = fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, f);
            } else {
                f(&p);
            }
        }
    }
}

/// R3 — the first authority check that can succeed.
///
/// PAIRED. A gate that only asserted "verified" could be passed by an engine
/// that verifies everything, which is the v0.2.45 defect wearing the other
/// mask. So the same test also requires that a whitelist NOT containing the
/// signer still refuses. Membership has to decide, in both directions.
#[test]
fn red_a_provisioned_whitelist_can_finally_verify() {
    let ident = ident_path("r3");
    let d = repo("r3", &ident);

    // Learn this operator's public key the only way that exists at baseline.
    let (r0, c0) = refine_signed(&d, &ident, 'a', 'b', "learn");
    let c0 = c0.unwrap_or_else(|| panic!("bootstrap refine did not run: {}", r0.out));
    let mine = signer_of(&d, &c0);
    assert!(r0.has("unverified"), "empty-whitelist refine should be unverified: {}", r0.out);

    // (i) My own key, provisioned out of band.
    provision(&d, &[&mine]);
    let (r1, c1) = refine_signed(&d, &ident, 'c', 'd', "verified");
    let c1 = c1.unwrap_or_else(|| panic!("refine under my own whitelist was refused: {}", r1.out));
    assert!(
        r1.has("verified") && !r1.has("unverified"),
        "a signature by a whitelisted key must be recorded verified: {}",
        r1.out
    );
    assert_eq!(signer_of(&d, &c1), mine, "a different party signed");

    // (ii) DISCRIMINATOR: somebody else's key, same everything else.
    let foreign: String = std::iter::repeat('f').take(64).collect();
    assert_ne!(foreign, mine);
    provision(&d, &[&foreign]);
    let (r2, c2) = refine_signed(&d, &ident, 'e', '0', "refused");
    assert!(
        c2.is_none(),
        "a whitelist that does not contain the signer accepted the refine: {}",
        r2.out
    );
    assert!(
        r2.has("not in architect_registry"),
        "refusal must name the membership failure: {}",
        r2.out
    );
}

/// R4 — `/sign_refine` off the language surface.
///
/// PAIRED with two controls, because "retired" must be distinguishable from
/// "the whole module vanished" and from "the system root broke".
#[test]
fn red_sign_refine_is_off_the_language_surface() {
    let ident = ident_path("r4");
    let d = repo("r4", &ident);

    // CONTROL 1: the system root is alive.
    let fuel = oo(&d, &ident, &["eval", "~%Config.fuel"]);
    assert!(fuel.has("10000"), "control: ~%Config.fuel broke: {}", fuel.out);

    // CONTROL 2: `~%Official` is still mounted. It becomes an EMPTY module,
    // which is the honest cold-start shape: the spec names the trust root
    // `~%Official.architects` (ORDER_01 §117) and nobody has declared one.
    //
    // The assertion is `{{`, not "not bottom", and the difference is the
    // whole point. Measured: a module removed from the (open) system root
    // evaluates to `_`, not `_|_` — so "not bottom" would pass for a module
    // that had vanished entirely, and would not be a control at all.
    let module = oo(&d, &ident, &["eval", "~%Official"]);
    assert!(
        module.has("{{"),
        "control: ~%Official must stay mounted as a closed combo, not vanish \
         into Top: {}",
        module.out
    );

    let got = oo(
        &d,
        &ident,
        &[
            "eval",
            r#"~%Official./sign_refine { source_caids: { 0: "x" }, target_caids: { 0: "y" } }"#,
        ],
    );
    assert!(
        got.has("missing_key"),
        "a retired morphism must be absent, not a dead one: {}",
        got.out
    );
    assert!(
        !got.has("signature_hex"),
        "the language layer still hands out operator signatures: {}",
        got.out
    );
}

/// R5 — the identity file is unreachable from the language layer, by virtue
/// of being the identity file and not by the accident of its name.
///
/// The path deliberately contains no `.oo` component, so the v0.2.42 store
/// boundary does not cover it for free. The probe writes the file itself, so
/// baseline redness is "the language read it", never "it was not there".
#[test]
fn red_identity_file_is_unreadable_from_the_language_layer() {
    let ident = ident_path("r5");
    fs::create_dir_all(ident.parent().unwrap()).unwrap();
    fs::write(&ident, b"NOT-A-REAL-KEY").unwrap();

    // CONTROL: a sibling in the same directory stays readable, so a refusal
    // is about this path and not about this directory.
    let sibling = ident.parent().unwrap().join("notes.txt");
    fs::write(&sibling, b"readable").unwrap();

    let d = fresh_dir("r5w");
    let ctl = oo(
        &d,
        &ident,
        &["eval", &format!("~%Io./read_file(\"{}\")", sibling.display())],
    );
    assert!(
        !ctl.has("_|_"),
        "control: an ordinary sibling file must stay readable: {}",
        ctl.out
    );

    let got = oo(
        &d,
        &ident,
        &["eval", &format!("~%Io./read_file(\"{}\")", ident.display())],
    );
    assert!(
        got.has("_|_"),
        "the language layer can read the operator's private key: {}",
        got.out
    );
    assert!(
        !got.has("NOT-A-REAL-KEY"),
        "key bytes reached the language layer: {}",
        got.out
    );
}

/// R6 — `oo identity` exists and tells the truth.
///
/// PAIRED: printing a well-formed hex string is worthless unless it is the
/// key that signing actually uses, so the printed value is checked against
/// the signer recorded in a commit object.
#[test]
fn red_oo_identity_prints_the_key_that_signing_uses() {
    let ident = ident_path("r6");
    let d = repo("r6", &ident);

    let shown = oo(&d, &ident, &["identity"]);
    assert!(shown.ok, "`oo identity` failed: {}", shown.out);
    let printed = shown
        .out
        .split(|c: char| !c.is_ascii_hexdigit())
        .find(|w| w.len() == 64)
        .unwrap_or_else(|| panic!("`oo identity` printed no 64-hex public key: {}", shown.out))
        .to_string();

    let (r, c) = refine_signed(&d, &ident, 'a', 'b', "check");
    let c = c.unwrap_or_else(|| panic!("refine did not run: {}", r.out));
    assert_eq!(
        signer_of(&d, &c),
        printed,
        "`oo identity` printed a key that is not the one signing uses"
    );
}

/// R7 — a damaged identity file is never silently replaced.
///
/// Losing an operator's key by overwriting it is unrecoverable, and doing so
/// silently would turn "my signatures stopped verifying" into a mystery. The
/// baseline red is exact: `OO_IDENTITY` is ignored, so the corrupt file is
/// stepped over and a fresh key is used without a word.
#[test]
fn red_a_corrupt_identity_file_is_refused_not_overwritten() {
    let ident = ident_path("r7");
    fs::create_dir_all(ident.parent().unwrap()).unwrap();
    let corrupt: &[u8] = b"this is not a pkcs8 document";
    fs::write(&ident, corrupt).unwrap();

    let d = repo("r7", &ident);
    let (r, c) = refine_signed(&d, &ident, 'a', 'b', "corrupt");

    assert!(
        c.is_none(),
        "a corrupt identity was stepped over and something else signed: {}",
        r.out
    );
    assert!(
        r.out.to_lowercase().contains("identity"),
        "the refusal must name the identity file: {}",
        r.out
    );
    assert_eq!(
        fs::read(&ident).unwrap(),
        corrupt,
        "the engine overwrote a key it could not read"
    );
}

/// R8 — an empty closed combo must render as re-parseable source.
///
/// Surfaced BY this arc, not caused by it. Retiring `/sign_refine` makes
/// `~%Official` the first empty closed combo mounted in the system root, so
/// `oo eval ~%Official` stops being hypothetical. Measured on v0.2.45:
///
///   oo eval '{{ }}'   →  {{ }        ← one brace short
///   oo eval '{{ }'    →  parse error: expected field
///
/// `oo fmt` is unaffected — source round-trips fine — so this is the value
/// renderer and fmt v2's freeze is not in question.
///
/// The gate does not prescribe a spelling, only that the rendered form
/// parses back and is idempotent. Choosing `{{ }}` or `{{}}` is delivery's.
#[test]
fn red_empty_closed_combo_renders_as_reparseable_source() {
    let ident = ident_path("r8");
    let d = fresh_dir("r8w");

    fn body(r: &Run) -> String {
        r.out.split(";;").next().unwrap().trim().to_string()
    }

    // CONTROL: the non-empty case already renders both braces, so a failure
    // below is about emptiness and not about closed combos in general.
    let ne = oo(&d, &ident, &["eval", "{{ a: 1 }}"]);
    assert!(
        ne.has("}}"),
        "control: a non-empty closed combo renders both braces: {}",
        ne.out
    );

    let got = oo(&d, &ident, &["eval", "{{ }}"]);
    let rendered = body(&got);
    assert!(
        !rendered.is_empty(),
        "harness: nothing rendered for an empty closed combo: {}",
        got.out
    );

    let again = oo(&d, &ident, &["eval", &rendered]);
    assert!(
        !again.has("expected"),
        "the rendered form does not parse back: {rendered:?} → {}",
        again.out
    );
    assert_eq!(
        body(&again),
        rendered,
        "rendering an empty closed combo is not idempotent"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// PINS — green at baseline, must stay green
// ─────────────────────────────────────────────────────────────────────────

/// P1 — v0.2.45's obligation survives persistence. Four workspaces, four
/// processes, four DIFFERENT identities, one root digest.
#[test]
fn pin_root_caid_is_independent_of_process_workspace_and_identity() {
    let digests: std::collections::BTreeSet<String> = (0..4)
        .map(|i| {
            let ident = ident_path(&format!("p1-{i}"));
            let d = repo(&format!("p1-{i}"), &ident);
            root_digest(&d, &ident)
        })
        .collect();
    assert_eq!(
        digests.len(),
        1,
        "the universe root depends on something local again: {digests:#?}"
    );
}

/// P2 — an empty whitelist still accepts a signed refine, recorded
/// `unverified` (SPEC_10 §93, v0.2.45).
#[test]
fn pin_empty_whitelist_signed_refine_stays_unverified() {
    let ident = ident_path("p2");
    let d = repo("p2", &ident);
    let (r, c) = refine_signed(&d, &ident, 'a', 'b', "u");
    assert!(c.is_some(), "empty-whitelist signed refine was refused: {}", r.out);
    assert!(
        r.has("unverified"),
        "a signature with no whitelist to check it against is not verified: {}",
        r.out
    );
}

/// P3 — an empty whitelist still accepts an UNSIGNED refine (the honest cold
/// start: nobody has declared an authority yet).
#[test]
fn pin_empty_whitelist_unsigned_refine_is_accepted() {
    let ident = ident_path("p3");
    let d = repo("p3", &ident);
    let (s, t) = (caid('a'), caid('b'));
    let r = oo(&d, &ident, &["refine", "-s", &s, "-t", &t, "-m", "u"]);
    assert!(
        r.has("Refine commit: "),
        "bootstrap refine must not require a signature: {}",
        r.out
    );
    assert!(r.has("unverified"), "and must be recorded unverified: {}", r.out);
}

/// P4 — the v0.2.42 store boundary is untouched by anything here.
#[test]
fn pin_store_boundary_still_refuses_dot_oo() {
    let ident = ident_path("p4");
    let d = repo("p4", &ident);
    fs::write(d.join("plain.txt"), b"ok").unwrap();

    let ctl = oo(&d, &ident, &["eval", r#"~%Io./read_file("plain.txt")"#]);
    assert!(!ctl.has("_|_"), "control: an ordinary file must be readable: {}", ctl.out);

    let got = oo(&d, &ident, &["eval", r#"~%Io./read_file(".oo/HEAD")"#]);
    assert!(
        got.has("store_boundary"),
        "the store boundary stopped refusing .oo paths: {}",
        got.out
    );
}

/// P5 — minting is lazy. Running, evolving and committing an ordinary
/// program must not bring a signing key into existence; only asking for one
/// does. Trivially green at baseline (nothing is ever persisted), and it is
/// exactly the property an eager `Ouroboros::init` implementation would
/// break.
#[test]
fn pin_ordinary_work_does_not_mint_an_identity() {
    let ident = ident_path("p5");
    let d = fresh_dir("p5w");
    fs::write(d.join("s.n"), SRC).unwrap();

    oo(&d, &ident, &["run", "s.n"]);
    oo(&d, &ident, &["status"]);
    oo(&d, &ident, &["evolve", "s.n"]);
    let c = oo(&d, &ident, &["commit", "-m", "x"]);
    assert!(c.has("Commit successful"), "harness: commit failed: {}", c.out);
    oo(&d, &ident, &["log"]);

    assert!(
        !ident.exists(),
        "ordinary work minted a signing key at {ident:?}"
    );
}

/// P6 — the module retirement in R4 must not take the rest of `~%Engine`
/// with it. Distinct from R4's control: that one proves `~%Official` is
/// mounted, this one proves the neighbouring module still resolves calls.
#[test]
fn pin_engine_module_still_resolves() {
    let ident = ident_path("p6");
    let d = repo("p6", &ident);
    let got = oo(&d, &ident, &["eval", "~%Engine./check_oml { a: @int, b: @int }"]);
    assert!(
        !got.has("missing_key"),
        "~%Engine regressed alongside ~%Official: {}",
        got.out
    );
}
