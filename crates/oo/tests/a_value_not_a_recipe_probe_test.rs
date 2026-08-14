// A value, not a recipe (Q-010b, pre-committed by work order:
// docs/a_value_not_a_recipe_handover.md).
//
// ── The claim ────────────────────────────────────────────────────────────
//
// O35 = A: the durable form is a VALUE, and reading it is DECODING, not
// evaluation. Three things follow, and they move identity, so they are one
// epoch or none:
//
//   1. FORCE AT COMMIT (O35, O51). `k1: 1 + 2` is stored today as
//      `Thunk{expr: 1+2, closure: […]}` — a recipe. Commit is the
//      solidification boundary; what goes into history is the observation,
//      not the program that has not been run.
//
//   2. THE CLOSURE CAPTURES ONLY FREE VARIABLES (O49-ii, SPEC_05 §3.3 MUST).
//      Forcing does NOT delete the closure — it renames it: `Thunk.closure`
//      becomes the morphism's `%closure`, and it is LOAD-BEARING (a morphism
//      that has left its defining scope has no root and no path to rebuild
//      from). The defect is that it mirrors the WHOLE scope: ~16 B per
//      irrelevant neighbour, and the field's own value inside it.
//
//   3. THE ROOT CARRIES `system`'s DIGEST, NOT ITS CONTENT (O50).
//      Measured at v0.20.0: `system` is 61,912 B of a 72,555 B root — 85%.
//      Storing the digest means adding a builtin no longer moves every
//      historical root: an old root keeps pointing at the old table.
//
// ── What must NOT move ───────────────────────────────────────────────────
//
// `.oo/staged` keeps its Thunks (O51: forcing happens at commit, not at
// evolve). Q-010a's guarantees are restated here as pins, because an arc that
// rewrites the write path is exactly where they would silently regress.
//
// ── Why recursion is the control, not a corner case ──────────────────────
//
// A recursive morphism's self-reference IS a free variable of its body.
// "Capture only free variables" keeps it; an implementation that reaches for
// "drop the self-reference" breaks `fact`. C2 exists so that this is caught by
// a probe rather than by a user. (An earlier draft of the ruling said "must
// not contain itself" — measurement retired that wording: the criterion has to
// name the property, not the symptom.)
//
// ── Probe integrity ──────────────────────────────────────────────────────
//
// Reds are `#[ignore]`d and MUST be red at the baseline for the reason stated
// in each. The delivery may remove `#[ignore]` and nothing else in this file.
// C0 runs first: every "X is absent from the store" assertion below is vacuous
// if the scan finds nothing.

use std::path::Path;
use std::process::Command;

fn fresh(tag: &str) -> nlang_interpreter::ScratchDir {
    let d = nlang_interpreter::ScratchDir::new(&format!("notarecipe-{tag}"));
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

/// Every file under `.oo/objects`. Never truncated.
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

fn root_object(dir: &Path) -> (std::path::PathBuf, String) {
    let (p, b) = objects(dir)
        .into_iter()
        .max_by_key(|(_, b)| b.len())
        .expect("no objects at all — see C0");
    (p, String::from_utf8_lossy(&b).into_owned())
}

/// The balanced JSON value that follows `"<key>":`, starting at `from`.
///
/// CALIBRATION NOTE (2026-08-14). The first draft bounded a field by "cut at
/// the next sibling key". That is wrong here and it made R2 pass at the
/// baseline — the closure MIRRORS THE ENCLOSING SCOPE, so every sibling key
/// also appears *inside* the field being sliced, and the cut landed in the
/// middle of the mirror. The whole point of R2 is that the mirror is there;
/// a slicer that trips over it cannot measure it. Brace matching only.
fn field_slice(json: &str, key: &str, from: usize) -> Option<String> {
    let pat = format!("\"{key}\"");
    let i = json[from..].find(&pat)? + from;
    let rest = &json[i + pat.len()..];
    let start = rest.find(|c: char| c == '{' || c == '[' || !c.is_whitespace() && c != ':')?;
    let body = &rest[start..];
    let (open, close) = match body.chars().next()? {
        '{' => ('{', '}'),
        '[' => ('[', ']'),
        // A scalar: take up to the next comma at depth 0.
        _ => return Some(body.split(',').next()?.to_string()),
    };
    let mut depth = 0i32;
    let mut in_str = false;
    let mut esc = false;
    for (n, c) in body.char_indices() {
        if esc {
            esc = false;
            continue;
        }
        match c {
            '\\' if in_str => esc = true,
            '"' => in_str = !in_str,
            _ if in_str => {}
            c if c == open => depth += 1,
            c if c == close => {
                depth -= 1;
                if depth == 0 {
                    return Some(body[..=n].to_string());
                }
            }
            _ => {}
        }
    }
    None
}

/// Every maximal run of exactly `n` lowercase hex characters. "Maximal" so a
/// 128-char run is not reported as two 64-char ones.
fn hex_runs(s: &str, n: usize) -> Vec<String> {
    let b = s.as_bytes();
    let hex = |c: u8| c.is_ascii_digit() || (b'a'..=b'f').contains(&c);
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if !hex(b[i]) {
            i += 1;
            continue;
        }
        let start = i;
        while i < b.len() && hex(b[i]) {
            i += 1;
        }
        if i - start == n {
            out.push(s[start..i].to_string());
        }
    }
    out
}

fn app_slice(root: &str) -> String {
    field_slice(root, "app", 0).expect("no `app` field in the root object — fixture drifted")
}

const SPREAD: &str = "app: { k1: 1 + 2, f: x -> x + v1, v1: 10, v2: 20, v3: 30 }\n";

// ── C0 ── the shelf is not empty ─────────────────────────────────────────
#[test]
fn c0_the_store_actually_has_objects() {
    let d = committed("c0", SPREAD);
    let objs = objects(&d);
    assert!(objs.len() >= 2, "expected root + commit, found {}", objs.len());
    let (_, root) = root_object(&d);
    assert!(
        root.len() > 1000 && root.contains("\"app\""),
        "root object is {} bytes and may not be the root; every absence \
         assertion below would be vacuous",
        root.len()
    );
}

// ── C1 ── the values survive ─────────────────────────────────────────────
#[test]
fn c1_the_committed_values_still_read_back() {
    let d = committed("c1", SPREAD);
    let (path, _) = root_object(&d);
    let caid = {
        let file = path.file_name().unwrap().to_string_lossy().to_string();
        let dir = path.parent().unwrap().file_name().unwrap().to_string_lossy().to_string();
        format!("hash:sha256:v1:{dir}{file}")
    };
    let out = oo(&d, &["inspect", &caid]);
    for want in ["k1", "f", "v1"] {
        assert!(
            out.contains(want),
            "`{want}` did not survive the round trip: {}",
            &out[..out.len().min(400)]
        );
    }
    assert!(
        !out.contains("caid_mismatch") && !out.contains("undecodable"),
        "the stored root no longer hashes to its own address: {}",
        &out[..out.len().min(300)]
    );
}

// ── C2 ── recursion still works ──────────────────────────────────────────
// THE control for the free-variable analysis. A recursive morphism's self
// reference is a free variable of its body; an implementation that reaches for
// "drop the self-reference" passes R3 and breaks this.
#[test]
fn c2_a_recursive_morphism_still_computes() {
    let d = fresh("c2");
    let out = oo(
        &d,
        &["eval", "{ fact: n -> (n <= 1) ? 1 : n * (fact (n - 1)) }.fact 5"],
    );
    assert!(
        out.contains("120"),
        "`fact 5` no longer gives 120. A recursive morphism's own name IS a \
         free variable of its body — narrowing the capture must keep it. Got: \
         {}",
        out.trim()
    );
}

// ── C3 ── a captured name still reaches the body ─────────────────────────
#[test]
fn c3_a_capturing_morphism_still_applies() {
    let d = fresh("c3");
    let out = oo(&d, &["eval", "({ y: 10, f: x -> x + y }.f) 5"]);
    assert!(
        out.contains("15"),
        "a morphism that captured `y` no longer applies — the closure is not \
         optional (SPEC_05 §3.3: 承重, 不得省略). Got: {}",
        out.trim()
    );
}

// ── P1 ── the working set stays lazy ─────────────────────────────────────
// O51: forcing happens at COMMIT. `.oo/staged` is not a CAS object and keeps
// its Thunks. An arc that moves forcing into the write path can take this out
// without meaning to.
#[test]
fn p1_staged_still_holds_thunks() {
    let d = fresh("p1");
    std::fs::write(d.join("u.n"), SPREAD).unwrap();
    oo(&d, &["evolve", "u.n"]);
    let staged = std::fs::read_to_string(d.join(".oo").join("staged"))
        .expect("`.oo/staged` is gone after evolve — the fixture, not the property");
    assert!(
        staged.contains("Thunk"),
        "`.oo/staged` no longer holds a Thunk. Forcing belongs at the commit \
         boundary, not at evolve (O51)"
    );
}

// ── P2 ── the closure is not deleted ─────────────────────────────────────
// O49 as first worded said the closure is not durable content. Measurement
// retired that: SPEC_05 §3.3 now makes it MUST-keep. This pin is the guard
// against a delivery that reads the older ruling.
#[test]
fn p2_a_stored_morphism_still_has_its_closure() {
    let d = committed("p2", SPREAD);
    let (_, root) = root_object(&d);
    let app = app_slice(&root);
    assert!(
        app.contains("closure") || app.contains("%closure"),
        "the stored morphism has no closure at all. It is load-bearing: a \
         morphism that has left its defining scope has no root and no path to \
         rebuild an environment from (SPEC_05 §3.3)"
    );
}

// ── P3 ── Q-010a's guarantees, restated ──────────────────────────────────
// An arc that rewrites the write path is exactly where the previous arc's
// properties regress silently.
#[test]
fn p3_the_format_2_guarantees_still_hold() {
    let a = committed("p3a", SPREAD);
    let b = committed("p3b", SPREAD);

    let (_, ra) = root_object(&a);
    let (_, rb) = root_object(&b);

    assert!(!ra.contains("\"span\""), "`span` is back on disk (Q-010a R1)");
    assert_eq!(
        ra.matches('\n').count(),
        0,
        "the object is being pretty-printed again (Q-010a R4)"
    );
    assert_eq!(
        ra, rb,
        "two runs of identical source produced different bytes — the \
         canonical serialization regressed (Q-010a R2)"
    );
}

// ── R1 ── what is committed is an observation, not a program ─────────────
#[test]
fn r1_a_committed_expression_is_stored_forced() {
    let d = committed("r1", SPREAD);
    let (_, root) = root_object(&d);
    let app = app_slice(&root);

    // The existence half: a morphism must still be there. This arc forces
    // values; it does not delete operators. Without this, "no Thunk" would
    // also pass on a store that lost everything.
    assert!(
        app.contains("morphism") || app.contains("closure"),
        "no operator survived in the store — R1 would then be passing because \
         the store is empty of the things it is asserting about"
    );

    let k1 = app
        .find("\"k1\"")
        .map(|i| app[i..(i + 200).min(app.len())].to_string())
        .unwrap_or_default();
    assert!(
        !k1.contains("Thunk"),
        "`k1: 1 + 2` is still stored as a Thunk — a recipe, not a value. \
         Commit is the solidification boundary (O35 = A, O51): what enters \
         history is the observation. Stored as: {k1}"
    );
}

// ── R2 ── the closure carries only what the body needs ───────────────────
#[test]
fn r2_the_closure_captures_only_free_variables() {
    let d = committed("r2", SPREAD);
    let (_, root) = root_object(&d);
    let app = app_slice(&root);

    let f = field_slice(&app, "f", 0).expect("no `f` in the store — fixture drifted");

    // Existence half: `v1` IS a free variable of `x -> x + v1` and MUST stay.
    assert!(
        f.contains("v1"),
        "`v1` is not in the morphism's closure, but it is a free variable of \
         its body — narrowing must not drop what the body needs"
    );

    let leaked: Vec<&str> = ["v2", "v3"].into_iter().filter(|v| f.contains(v)).collect();
    assert!(
        leaked.is_empty(),
        "the closure carries names the body never mentions: {leaked:?}. \
         It mirrors the whole enclosing scope, so an operator's size grows \
         with neighbours that have nothing to do with it (~16 B each, \
         measured v0.20.0). SPEC_05 §3.3: 僅捕捉自由變數 (MUST)"
    );
}

// ── R3 ── the root names the builtin table, it does not carry it ─────────
#[test]
fn r3_the_root_carries_the_digest_of_system_not_its_body() {
    let d = committed("r3", SPREAD);
    let (_, root) = root_object(&d);

    // Existence half: the root must still HAVE a system slot. "No Math" would
    // otherwise pass on a root that lost the field entirely.
    assert!(
        root.contains("system"),
        "the root has no `system` slot at all — the assertion below would be \
         passing for the wrong reason"
    );
    let inlined: Vec<&str> = ["math.add", "list.map", "string.len"]
        .into_iter()
        .filter(|n| root.contains(n))
        .collect();
    assert!(
        inlined.is_empty(),
        "the builtin table is inlined into the root ({inlined:?} found). \
         Measured at v0.20.0: 61,912 B of a 72,555 B root — 85%. With the \
         digest instead, adding a builtin stops moving every historical root: \
         an old root keeps naming the old table (O50)"
    );
}

// ── R4 ── an unknown builtin table is refused by name ────────────────────
// The digest is only honest if the engine can say it does not have that table.
// Without this, R3 could be satisfied by a root that names a table nobody
// checks.
// ACCEPTOR-REWRITTEN (repair round 3). The first instrument took the ROOT's
// `system` slot as `field_slice(root, "system", 0)` — the FIRST occurrence.
// That is the mirror mistake R2 was calibrated against, made a second time in
// the same file: nested combos each carry their own `"system":{}`, and the
// root's own slot is the LAST of seven, not the first. It passed two rounds
// only because the delivery happened to compact the empty ones away, so the
// instrument was measuring a coincidence of the implementation it was testing.
//
// The property does not mention field order or the sentinel's spelling: the
// root names EXACTLY ONE standard-library table by digest, and corrupting
// that name must produce a refusal that says which table is missing.
#[test]
fn r4_an_unresolvable_table_digest_is_refused_by_name() {
    let d = committed("r4", SPREAD);
    let (path, root) = root_object(&d);

    let digests = hex_runs(&root, 64);
    assert_eq!(
        digests.len(),
        1,
        "expected exactly one 64-hex table digest in the root, found {}. \
         Zero means there is no digest to be unable to resolve; more than one \
         means this instrument cannot tell which is the table, and it must \
         stop rather than guess: {digests:?}",
        digests.len()
    );

    let zeroed = "0".repeat(64);
    let broken = root.replace(&digests[0], &zeroed);
    assert_ne!(broken, root, "the digest was not substituted");
    std::fs::write(&path, &broken).unwrap();

    let out = oo(&d, &["log"]);
    assert!(
        out.contains(&zeroed),
        "a root naming a standard-library table this engine does not have was \
         not refused BY NAME — the message has to say which table is missing, \
         or the digest is decoration rather than a dependency. Got: {}",
        out.trim()
    );
}
