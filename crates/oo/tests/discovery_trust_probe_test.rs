// 歸屬信任根 / affiliation trust roots — #3c-b2/② (2026-07-31).
// Pre-committed by work order: docs/discovery_trust_handover.md
//
// ── The false pointer this arc corrects ───────────────────────────────────
//
// SPEC_13 §4.1.2 and REAL_02 §4.2.8 currently point discovery trust at
// `~/.oo/authorized_keys`. REAL_01 §7.0.2 says that file belongs to the dormant
// SERVICE face: it names issuers of long-lived privilege tokens and is paired
// with a CRL. It cannot also mean "whose affiliation claims may express this
// node's admission consent". Those are different questions in REAL_01 §7.6.
//
// The active precedent is the workspace assertion layer: `.oo/architects.json`
// is the refine-authority list. But the lists must not be merged — governance,
// service-token issuance, package-blacklist authority and affiliation admission
// answer four different questions. REAL_02 §5.1 already reserves the honest
// home for the fourth one: `.oo/discovery.n`, "發現節點與信任設定".
//
// ── What this arc builds, and what it deliberately does not ───────────────
//
// This arc makes `.oo/discovery.n` real with one closed data field:
//
//     affiliation_roots: ["<64 lowercase hex Ed25519 public key>"]
//
// A root means only: a valid affiliation claim signed by this operator MAY be
// read by the next arc as consent to admit the claimed node. It grants no
// refine authority, package authority, service token, language capability or
// degree-0 correctness preference.
//
// This arc stores and exposes the roots. It DOES NOT consume them: no automatic
// dial, no peer-source insertion, no routing/directory preference, no advert
// acceptance change. That is ③ automatic admission + cap, a different thing.
//
// ── Reconnaissance measurements on v0.6.0 ─────────────────────────────────
//
// * `.oo/discovery.n` has zero engine/code references; only the spec blueprint.
// * A language write to the candidate path is already refused with
//   `#store_boundary`, and the file does not appear. The existing component-
//   exact `.oo` boundary buys this arc its language isolation for free.
// * Malformed `.oo/architects.json` is silently swallowed by
//   `load_architects(...).unwrap_or_else(|_| empty)`: `oo status` exits 0 and
//   prints "Universe is static". That is fail-safe but not honest — absence and
//   unreadability collapse. R4–R7 forbid copying that precedent.
// * `Ouroboros.peers` is process-local and unbounded; `disc.connect` is its only
//   active remote writer. `PeerAdvert.verified_operator_key` is already derived
//   from signed `%ad` and never persisted. The future admission test therefore
//   has all inputs, but this arc leaves that seam untouched (P4).
//
// ── Configuration/CLI contract ────────────────────────────────────────────
//
// Missing file and explicit `affiliation_roots: []` both mean an empty set.
// Malformed, unreadable, non-canonical or unknown input is a NAMED error — never
// silently empty. The file is parsed as closed literal DATA, not evaluated as
// n/ code. The trusted out-of-band surface is:
//
//     oo node trust list
//     oo node trust add <operator-key>
//     oo node trust remove <operator-key>
//
// `list` prints canonical keys one per line, sorted. Empty prints no keys and
// must not create the file. Add/remove may edit the assertion layer because the
// local operator already owns it; no capability is inferred from the file.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

const CONFIG: &str = "discovery.n";
const FIELD: &str = "affiliation_roots";
const KEY_A: &str = "1111111111111111111111111111111111111111111111111111111111111111";
const KEY_B: &str = "2222222222222222222222222222222222222222222222222222222222222222";

#[derive(Debug)]
struct RunResult {
    text: String,
    ok: bool,
}

fn fresh_dir(tag: &str) -> PathBuf {
    let mut d = std::env::temp_dir();
    d.push(format!(
        "nlang-discovery-trust-{}-{}-{}",
        tag,
        std::process::id(),
        DIR_SEQ.fetch_add(1, Ordering::SeqCst)
    ));
    fs::remove_dir_all(&d).ok();
    fs::create_dir_all(&d).unwrap();
    d
}

fn oo_cmd_with_home(dir: &Path, home: &Path) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_oo"));
    c.current_dir(dir)
        .env("HOME", home)
        .env("OO_IDENTITY", dir.join("identity-for-tests"))
        .env("OO_NODE_HOME", dir.join("node-home-for-tests"));
    c
}

fn oo_cmd(dir: &Path) -> Command {
    oo_cmd_with_home(dir, &dir.join("home-for-tests"))
}

fn run_command(mut command: Command, args: &[&str]) -> RunResult {
    let out = command.args(args).output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    RunResult {
        text: format!("{}{}", stdout, stderr).trim().to_string(),
        ok: out.status.success(),
    }
}

fn oo_raw(dir: &Path, args: &[&str]) -> RunResult {
    run_command(oo_cmd(dir), args)
}

fn oo_raw_with_home(dir: &Path, home: &Path, args: &[&str]) -> RunResult {
    run_command(oo_cmd_with_home(dir, home), args)
}

fn oo(dir: &Path, args: &[&str]) -> String {
    let out = oo_raw(dir, args);
    assert!(out.ok, "oo {args:?} failed: {}", out.text);
    out.text
}

fn init(dir: &Path) {
    let out = oo_raw(dir, &["status"]);
    assert!(out.ok, "control: `oo status` failed: {}", out.text);
    assert!(
        dir.join(".oo").join("format").exists(),
        "control: status did not initialize a store"
    );
}

fn config_path(dir: &Path) -> PathBuf {
    dir.join(".oo").join(CONFIG)
}

fn valid_config(keys: &[&str]) -> String {
    let body = keys
        .iter()
        .map(|k| format!("    \"{k}\""))
        .collect::<Vec<_>>()
        .join(",\n");
    if body.is_empty() {
        format!("{FIELD}: []\n")
    } else {
        format!("{FIELD}: [\n{body}\n]\n")
    }
}

fn write_config(dir: &Path, text: &str) {
    fs::create_dir_all(dir.join(".oo")).unwrap();
    fs::write(config_path(dir), text).unwrap();
}

fn object_count(dir: &Path) -> usize {
    fn walk(path: &Path, n: &mut usize) {
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    walk(&p, n);
                } else {
                    *n += 1;
                }
            }
        }
    }
    let mut n = 0;
    walk(&dir.join(".oo").join("objects"), &mut n);
    n
}

fn commit_same_source(dir: &Path) {
    fs::write(dir.join("same.n"), "same: { anchor: 1 }\n").unwrap();
    oo(dir, &["evolve", "same.n"]);
    let out = oo(dir, &["commit", "-m", "same"]);
    assert!(
        out.contains("Commit successful"),
        "commit did not happen: {out}"
    );
}

fn head_commit(dir: &Path) -> String {
    let c = oo(dir, &["log"])
        .lines()
        .find_map(|line| line.strip_prefix("commit ").map(|s| s.trim().to_string()))
        .unwrap_or_default();
    assert!(c.starts_with("hash:sha256:"), "no HEAD commit in {dir:?}");
    c
}

fn object_path(dir: &Path, caid: &str) -> PathBuf {
    let digest = caid.rsplit(':').next().expect("CAID digest");
    assert!(digest.len() >= 2, "short CAID digest: {caid}");
    dir.join(".oo")
        .join("objects")
        .join("sha256")
        .join(&digest[..2])
        .join(&digest[2..])
}

fn root_digest(dir: &Path) -> String {
    let path = object_path(dir, &head_commit(dir));
    let commit: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).unwrap_or_else(|e| panic!("{path:?}: {e}")))
            .unwrap();
    let digest = &commit["root"]["digest"];
    let hex = if let Some(s) = digest.as_str() {
        s.to_string()
    } else if let Some(bytes) = digest.as_array() {
        bytes
            .iter()
            .map(|b| format!("{:02x}", b.as_u64().expect("digest byte")))
            .collect()
    } else {
        panic!("commit root has no usable digest: {}", commit["root"]);
    };
    assert!(
        hex.len() == 64 && hex.chars().all(|c| c.is_ascii_hexdigit()),
        "root digest is not 64 hex: {hex:?}"
    );
    hex
}

fn list_lines(text: &str) -> Vec<&str> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect()
}

// ── controls: prove the harness and the pre-existing node surface are live ─

#[test]
fn control_store_initialization_is_live() {
    let d = fresh_dir("control-store");
    init(&d);
    assert!(d.join(".oo").join("objects").is_dir());
    assert!(!oo(&d, &["status"]).is_empty());
}

#[test]
fn control_existing_node_surface_is_live() {
    let d = fresh_dir("control-node");
    init(&d);
    let peers = oo_raw(&d, &["node", "peers"]);
    assert!(
        peers.ok,
        "control: existing `oo node peers` failed: {}",
        peers.text
    );
}

// ── pins: green on v0.6.0, must remain green after delivery ───────────────

/// P1 — the reserved configuration path is already behind the unconditional
/// store boundary. The ordinary-file write is the same-run presence control.
#[test]
fn p1_language_boundary_already_covers_discovery_config() {
    let d = fresh_dir("p1");
    init(&d);

    let control = oo(&d, &["eval", "~%Io./write_file(\"control.txt\", \"live\")"]);
    assert!(
        control.contains("true"),
        "ordinary write control failed: {control}"
    );
    assert_eq!(fs::read_to_string(d.join("control.txt")).unwrap(), "live");

    let denied = oo(
        &d,
        &[
            "eval",
            "~%Io./write_file(\".oo/discovery.n\", \"affiliation_roots: []\")",
        ],
    );
    assert!(
        denied.contains("store_boundary"),
        "language write reached the assertion layer: {denied}"
    );
    assert!(!config_path(&d).exists());
}

/// P2 — ordinary engine work must not manufacture a trust declaration or a
/// signing identity. Absence is paired with `.oo/format` and HEAD presence.
#[test]
fn p2_ordinary_work_does_not_create_trust_or_keys() {
    let d = fresh_dir("p2");
    init(&d);
    commit_same_source(&d);
    let peers = oo_raw(&d, &["node", "peers"]);
    assert!(peers.ok, "control: node peers failed: {}", peers.text);

    assert!(d.join(".oo").join("format").exists());
    assert!(d.join(".oo").join("HEAD").exists());
    assert!(
        !config_path(&d).exists(),
        "ordinary work manufactured trust"
    );
    assert!(
        !d.join("identity-for-tests").exists(),
        "ordinary work minted operator key"
    );
    assert!(
        !d.join("node-home-for-tests").exists(),
        "ordinary work minted node key"
    );
}

/// P3 — local trust configuration is an assertion, never universe content.
/// Both sides are committed and non-empty before comparison.
#[test]
fn p3_discovery_config_never_reaches_the_universe_root() {
    let a = fresh_dir("p3-a");
    let b = fresh_dir("p3-b");
    init(&a);
    init(&b);
    write_config(&a, &valid_config(&[KEY_A]));
    commit_same_source(&a);
    commit_same_source(&b);

    assert!(
        object_count(&a) > 0 && object_count(&b) > 0,
        "comparison has an empty side"
    );
    let ar = root_digest(&a);
    let br = root_digest(&b);
    assert!(
        !ar.is_empty() && !br.is_empty(),
        "comparison has an empty root"
    );
    assert_eq!(
        ar, br,
        "workspace trust configuration entered universe content"
    );
}

/// P4 — merely naming an affiliation root has no network/source effect in this
/// arc. ③ may consume it only after a verified claim and under a separate cap.
#[test]
fn p4_a_root_alone_creates_no_peer_source_or_network_identity() {
    let d = fresh_dir("p4");
    init(&d);
    write_config(&d, &valid_config(&[KEY_A]));

    let peers = oo_raw(&d, &["node", "peers"]);
    assert!(peers.ok, "control: node peers failed: {}", peers.text);
    assert!(
        peers.text.is_empty(),
        "a bare trust root manufactured a peer: {}",
        peers.text
    );
    assert!(
        !d.join(".oo").join("peers").exists(),
        "a bare root created the peer directory"
    );
    assert!(
        !d.join("identity-for-tests").exists(),
        "reading trust minted operator identity"
    );
    assert!(
        !d.join("node-home-for-tests").exists(),
        "reading trust minted node identity"
    );
}

/// P5 — the four authority questions remain physically separate. This arc must
/// not rewrite the refine list or instantiate the dormant service-face list.
#[test]
fn p5_other_authority_lists_remain_separate() {
    let d = fresh_dir("p5");
    init(&d);
    let architects = d.join(".oo").join("architects.json");
    fs::write(&architects, "[]").unwrap();
    let before = fs::read(&architects).unwrap();
    write_config(&d, &valid_config(&[KEY_A]));

    oo(&d, &["status"]);
    assert_eq!(
        fs::read(&architects).unwrap(),
        before,
        "discovery trust rewrote governance trust"
    );
    assert!(
        config_path(&d).exists(),
        "presence control: discovery config vanished"
    );
    assert!(
        !d.join("home-for-tests")
            .join(".oo")
            .join("authorized_keys")
            .exists(),
        "local discovery trust instantiated the dormant service-token whitelist"
    );
}

// ── red gate: ignored at baseline, delivery removes only #[ignore] ─────────

#[test]
#[ignore = "delivery gate: expose an empty workspace-local trust set"]
fn red_list_surface_exists_and_missing_means_empty_without_creation() {
    let d = fresh_dir("r1");
    init(&d);
    let control = oo_raw(&d, &["node", "peers"]);
    assert!(
        control.ok,
        "control: node surface is absent: {}",
        control.text
    );

    let listed = oo_raw(&d, &["node", "trust", "list"]);
    assert!(listed.ok, "`oo node trust list` is absent: {}", listed.text);
    assert!(
        listed.text.is_empty(),
        "missing config is not an empty set: {}",
        listed.text
    );
    assert!(
        !config_path(&d).exists(),
        "listing an absent root created durable state"
    );
}

#[test]
#[ignore = "delivery gate: add and list one canonical affiliation root"]
fn red_add_persists_the_reserved_literal_config() {
    let d = fresh_dir("r2");
    init(&d);
    let added = oo_raw(&d, &["node", "trust", "add", KEY_A]);
    assert!(added.ok, "trust add is absent: {}", added.text);
    assert!(
        added.text.contains("added") && added.text.contains(KEY_A),
        "add was not observable: {}",
        added.text
    );

    let listed = oo_raw(&d, &["node", "trust", "list"]);
    assert!(listed.ok, "trust list failed: {}", listed.text);
    assert_eq!(list_lines(&listed.text), vec![KEY_A]);
    let text = fs::read_to_string(config_path(&d)).expect("trust add wrote no discovery.n");
    assert!(
        text.contains(FIELD) && text.contains(KEY_A),
        "wrong config shape: {text:?}"
    );
}

#[test]
#[ignore = "delivery gate: roots are a sorted removable set"]
fn red_add_remove_round_trip_is_exact_and_sorted() {
    let d = fresh_dir("r3");
    init(&d);
    assert!(oo_raw(&d, &["node", "trust", "add", KEY_B]).ok);
    assert!(oo_raw(&d, &["node", "trust", "add", KEY_A]).ok);

    let both = oo_raw(&d, &["node", "trust", "list"]);
    assert!(both.ok, "trust list failed: {}", both.text);
    assert_eq!(
        list_lines(&both.text),
        vec![KEY_A, KEY_B],
        "roots are not canonical/sorted"
    );

    let removed = oo_raw(&d, &["node", "trust", "remove", KEY_A]);
    assert!(removed.ok, "trust remove failed: {}", removed.text);
    assert!(removed.text.contains("removed") && removed.text.contains(KEY_A));
    let left = oo_raw(&d, &["node", "trust", "list"]);
    assert!(left.ok);
    assert_eq!(list_lines(&left.text), vec![KEY_B]);
    let text = fs::read_to_string(config_path(&d)).unwrap();
    assert!(!text.contains(KEY_A) && text.contains(KEY_B));
}

#[test]
#[ignore = "delivery gate: malformed configuration is named, never silently empty"]
fn red_malformed_config_is_a_loud_named_error() {
    let d = fresh_dir("r4");
    init(&d);
    write_config(&d, "affiliation_roots: [\n");
    let out = oo_raw(&d, &["status"]);
    assert!(
        !out.ok,
        "malformed discovery.n was silently treated as empty: {}",
        out.text
    );
    assert!(
        out.text.contains(CONFIG),
        "error did not name the file: {}",
        out.text
    );
    assert!(
        out.text.to_lowercase().contains("parse"),
        "error did not name parsing: {}",
        out.text
    );
}

#[test]
#[ignore = "delivery gate: impossible Ed25519 names are rejected"]
fn red_short_key_is_a_loud_named_error() {
    let d = fresh_dir("r5");
    init(&d);
    write_config(&d, &valid_config(&["aa"]));
    let out = oo_raw(&d, &["status"]);
    assert!(
        !out.ok,
        "short key was silently treated as an empty root: {}",
        out.text
    );
    assert!(
        out.text.contains(CONFIG) && out.text.contains("64"),
        "unhelpful key error: {}",
        out.text
    );
}

#[test]
#[ignore = "delivery gate: public-key spelling has one canonical form"]
fn red_uppercase_key_is_rejected_not_silently_normalized() {
    let d = fresh_dir("r6");
    init(&d);
    let upper = "AB".repeat(32);
    write_config(&d, &valid_config(&[&upper]));
    let out = oo_raw(&d, &["status"]);
    assert!(
        !out.ok,
        "uppercase key was accepted or ignored: {}",
        out.text
    );
    assert!(
        out.text.contains(CONFIG) && out.text.to_lowercase().contains("lowercase"),
        "error did not name the canonical form: {}",
        out.text
    );
}

#[test]
#[ignore = "delivery gate: closed data shape catches misspelled policy"]
fn red_unknown_field_is_not_silently_ignored() {
    let d = fresh_dir("r7");
    init(&d);
    write_config(&d, "affiliation_rootz: []\n");
    let out = oo_raw(&d, &["status"]);
    assert!(
        !out.ok,
        "unknown field silently became an empty trust root: {}",
        out.text
    );
    assert!(
        out.text.to_lowercase().contains("unknown") && out.text.contains("affiliation_rootz"),
        "error did not identify the unknown field: {}",
        out.text
    );
}

#[test]
#[ignore = "delivery gate: a node trust decision cannot leak to sibling workspaces"]
fn red_roots_are_workspace_local_even_under_one_home() {
    let a = fresh_dir("r8-a");
    let b = fresh_dir("r8-b");
    let shared_home = fresh_dir("r8-shared-home");
    // Presence controls under the SAME HOME: both workspaces initialize and the
    // existing node surface answers before the new surface is tested.
    for d in [&a, &b] {
        let status = oo_raw_with_home(d, &shared_home, &["status"]);
        assert!(
            status.ok,
            "control: status failed under shared HOME: {}",
            status.text
        );
        assert!(d.join(".oo").join("format").exists());
        let peers = oo_raw_with_home(d, &shared_home, &["node", "peers"]);
        assert!(
            peers.ok,
            "control: node peers failed under shared HOME: {}",
            peers.text
        );
    }
    write_config(&a, &valid_config(&[KEY_A]));

    let al = oo_raw_with_home(&a, &shared_home, &["node", "trust", "list"]);
    let bl = oo_raw_with_home(&b, &shared_home, &["node", "trust", "list"]);
    assert!(
        al.ok && bl.ok,
        "trust list surface absent: A={} B={}",
        al.text,
        bl.text
    );
    assert_eq!(list_lines(&al.text), vec![KEY_A]);
    assert!(bl.text.is_empty(), "A's consent leaked into B: {}", bl.text);
    assert!(
        !config_path(&b).exists(),
        "listing B copied A's declaration into it"
    );
}

#[test]
#[ignore = "delivery gate: trust management is configuration only, not identity or admission"]
fn red_trust_management_mints_no_keys_and_admits_no_peers() {
    let d = fresh_dir("r9");
    init(&d);
    commit_same_source(&d);
    let before = object_count(&d);
    assert!(before > 0, "harness: object comparison starts empty");
    let format_path = d.join(".oo").join("format");
    let format_before =
        fs::read_to_string(&format_path).expect("presence control: no format marker");
    assert!(
        !format_before.trim().is_empty(),
        "presence control: empty format marker"
    );

    let added = oo_raw(&d, &["node", "trust", "add", KEY_A]);
    assert!(added.ok, "trust management surface absent: {}", added.text);
    assert_eq!(
        object_count(&d),
        before,
        "trust configuration wrote universe objects"
    );
    assert_eq!(
        fs::read_to_string(&format_path).unwrap(),
        format_before,
        "an optional assertion-layer file bumped the store format"
    );
    assert!(
        !d.join("identity-for-tests").exists(),
        "trust add minted operator identity"
    );
    assert!(
        !d.join("node-home-for-tests").exists(),
        "trust add minted node identity"
    );
    assert!(
        !d.join(".oo").join("peers").exists(),
        "trust add admitted or persisted a peer"
    );
}

#[test]
#[ignore = "delivery gate: CLI validation refuses bad input without manufacturing a file"]
fn red_invalid_cli_key_is_rejected_before_any_write() {
    let d = fresh_dir("r10");
    init(&d);
    let control = oo_raw(&d, &["node", "peers"]);
    assert!(control.ok, "control: node surface absent: {}", control.text);

    let out = oo_raw(&d, &["node", "trust", "add", "aa"]);
    assert!(!out.ok, "invalid key was accepted: {}", out.text);
    assert!(
        out.text.contains("64") && out.text.to_lowercase().contains("lowercase"),
        "invalid-key refusal did not explain the required form: {}",
        out.text
    );
    assert!(
        !config_path(&d).exists(),
        "failed add still manufactured discovery.n"
    );
}

#[test]
#[ignore = "delivery gate: unreadable configuration is distinct from absence"]
fn red_config_path_that_cannot_be_read_as_a_file_is_loud() {
    let d = fresh_dir("r11");
    init(&d);
    fs::create_dir(config_path(&d)).unwrap();
    let out = oo_raw(&d, &["status"]);
    assert!(
        !out.ok,
        "directory-at-config-path silently became an empty set: {}",
        out.text
    );
    assert!(
        out.text.contains(CONFIG),
        "read error did not name discovery.n: {}",
        out.text
    );
}
