// Forgetting, once it is allowed to happen at all (2026-07-28, pre-committed
// by work order: docs/local_gc_handover.md).
//
// ── What is on the floor ─────────────────────────────────────────────────
//
// Measured on v0.2.52, five commits and one `#squash`:
//
//     before squash   10 objects, 10 reachable, 0% garbage
//     after squash    11 objects,  4 reachable, 60% of bytes unreachable
//                     …and the store had grown
//
// `#squash` is the one operation that makes objects unreachable (the rollback
// arc established that), and nothing has ever reclaimed them. The store only
// grows.
//
// The `0% before` line is not decoration. The first walker written to produce
// these numbers did not understand digests serialised as byte arrays, found
// one object, and reported *100% garbage*. A measurement that finds nothing
// and a measurement that finds everything-is-garbage look identical from the
// outside; the control is what tells them apart. Same family as reading
// `content_digest` when the field is `digest`.
//
// ── Why the root set is small enough to get right ────────────────────────
// `evolve` writes no objects — only `commit` does — and `staged` inlines its
// whole value, naming no CAID. So the roots are HEAD, its parent chain, and
// each commit's root tree. Nothing else in `.oo/` makes an object reachable.
//
// ── The ruling this arc turns on ─────────────────────────────────────────
// After a `#rollback` and a commit, whether the abandoned head counts as a
// root is worth half the store: as a root, 0% is collectable; not a root,
// 50%. The owner ruled: **not a root**.
//
// That follows the rollback arc's own line — details may be lost, the fact
// may not — because the fact is the digest in `CommitMeta.abandoned`, and the
// digest survives. But the price is not hidden: `oo log` will name content the
// store no longer holds, and rolling forward stops being possible. R3 pins
// both halves — the collection AND the honest log line.
//
// ── Forgetting does not happen by itself ─────────────────────────────────
// Discussion 025 argued global GC is impossible in principle: no global HEAD,
// therefore no global reachability root. Local explicit collection is the only
// kind there can be, and that is structural rather than cautious. R5 holds the
// engine to it — nothing collects as a side effect of anything — and R6 puts
// it behind the same capability gate as `#squash`, because deleting bytes is
// at least as consequential as making them unreachable.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

// ── harness ─────────────────────────────────────────────────────────────

fn fresh_dir(tag: &str) -> nlang_interpreter::ScratchDir {
    nlang_interpreter::ScratchDir::new(&format!("gc-{tag}"))
}

fn oo_raw(dir: &Path, args: &[&str]) -> (String, bool) {
    let out = Command::new(env!("CARGO_BIN_EXE_oo"))
        .args(args)
        .current_dir(dir)
        .env("OO_IDENTITY", dir.join("identity-for-tests"))
        .env("OO_NODE_HOME", dir.join("node-home-for-tests"))
        .output()
        .unwrap();
    (
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout).trim(),
            String::from_utf8_lossy(&out.stderr).trim()
        ),
        out.status.success(),
    )
}

fn oo(dir: &Path, args: &[&str]) -> String {
    oo_raw(dir, args).0
}

fn write(dir: &Path, name: &str, src: &str) {
    fs::write(dir.join(name), src).unwrap();
}

fn workset_snapshot(dir: &Path) -> Vec<u8> {
    let mut ps = nlang_interpreter::injections::paths(dir).unwrap();
    ps.sort();
    let mut out = Vec::new();
    for p in ps {
        out.extend(fs::read(&p).unwrap());
        out.push(0);
    }
    out
}

fn objects_dir(dir: &Path) -> PathBuf {
    dir.join(".oo").join("objects").join("sha256")
}

/// Every object in the store: digest → byte length.
fn store_map(dir: &Path) -> BTreeMap<String, u64> {
    let mut out = BTreeMap::new();
    let root = objects_dir(dir);
    let Ok(top) = fs::read_dir(&root) else {
        return out;
    };
    for a in top.flatten() {
        if !a.path().is_dir() {
            continue;
        }
        let pre = a.file_name().to_string_lossy().to_string();
        for b in fs::read_dir(a.path()).unwrap().flatten() {
            let rest = b.file_name().to_string_lossy().to_string();
            out.insert(format!("{pre}{rest}"), b.metadata().unwrap().len());
        }
    }
    out
}

fn object_path(dir: &Path, digest: &str) -> PathBuf {
    objects_dir(dir).join(&digest[..2]).join(&digest[2..])
}

fn digest_of_caid(caid: &str) -> String {
    caid.rsplit(':').next().unwrap().trim().to_string()
}

fn head_digest(dir: &Path) -> String {
    let h = fs::read_to_string(dir.join(".oo").join("HEAD")).unwrap();
    digest_of_caid(h.trim())
}

/// Every digest a JSON object refers to.
///
/// Digests appear **both** as 64-hex strings and as byte arrays. A walker that
/// understands only one of those finds nothing and calls the whole store
/// garbage — which is how the first version of this function behaved, and why
/// `reachable_before_any_squash_is_everything` exists as a control.
///
/// `follow_abandoned` exists so the probes can measure the ruling of R-b
/// rather than assume it.
fn refs_of(v: &serde_json::Value, follow_abandoned: bool, out: &mut Vec<String>) {
    match v {
        serde_json::Value::Object(m) => {
            for (k, x) in m {
                if k == "abandoned" && !follow_abandoned {
                    continue;
                }
                if k == "__nlang_system_digest" {
                    refs_of_standard_digest(x, out);
                } else if k == "digest" {
                    match x {
                        serde_json::Value::String(s) if s.len() == 64 => out.push(s.clone()),
                        serde_json::Value::Array(a) => {
                            let hex: String = a
                                .iter()
                                .map(|b| format!("{:02x}", b.as_u64().unwrap_or(0)))
                                .collect();
                            if hex.len() == 64 {
                                out.push(hex);
                            }
                        }
                        other => refs_of(other, follow_abandoned, out),
                    }
                } else {
                    refs_of(x, follow_abandoned, out);
                }
            }
        }
        serde_json::Value::Array(a) => {
            for x in a {
                refs_of(x, follow_abandoned, out);
            }
        }
        serde_json::Value::String(s) if s.starts_with("hash:sha256:") => {
            out.push(digest_of_caid(s));
        }
        _ => {}
    }
}

fn refs_of_standard_digest(v: &serde_json::Value, out: &mut Vec<String>) {
    match v {
        serde_json::Value::String(s)
            if s.len() == 64
                && s.bytes()
                    .all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f' | b'A'..=b'F')) =>
        {
            out.push(s.to_lowercase())
        }
        serde_json::Value::Object(m) => {
            for value in m.values() {
                refs_of_standard_digest(value, out);
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                refs_of_standard_digest(value, out);
            }
        }
        _ => {}
    }
}

/// The set the engine must keep, computed independently of the engine.
fn reachable(dir: &Path, follow_abandoned: bool) -> BTreeSet<String> {
    let all = store_map(dir);
    let mut seen = BTreeSet::new();
    let mut stack = vec![head_digest(dir)];
    while let Some(d) = stack.pop() {
        if seen.contains(&d) || !all.contains_key(&d) {
            continue;
        }
        seen.insert(d.clone());
        let Ok(bytes) = fs::read(object_path(dir, &d)) else {
            continue;
        };
        let Ok(json) = nlang_interpreter::store_codec::object_json_view(&bytes) else {
            continue;
        };
        let mut r = Vec::new();
        refs_of(&json, follow_abandoned, &mut r);
        // D52: new commits store `parent: None`. The predecessor lives on
        // the commit circle (`commit:` + `parents:`). Dual-walk: JSON
        // `parent` when present (already in `r`), else the circle covering.
        // R-b: a digest in this commit's `abandoned` is a record, not a
        // covering predecessor — skip it unless we are measuring the
        // abandoned-as-root counterfactual.
        let mut skip = BTreeSet::new();
        if !follow_abandoned {
            if let Some(arr) = json
                .pointer("/meta/abandoned")
                .or_else(|| json.get("abandoned"))
                .and_then(|v| v.as_array())
            {
                for x in arr {
                    if let Some(s) = x.as_str() {
                        let hex = s.rsplit(':').next().unwrap_or(s);
                        if hex.len() == 64 {
                            skip.insert(hex.to_lowercase());
                        }
                    }
                }
            }
        }
        if let Some(p) = commit_pred_from_circles(dir, &d, &skip) {
            r.push(p);
        }
        stack.extend(r);
    }
    seen
}

/// Independent of the engine walk: directory is truth. Finds the first
/// other `commit:` digest reachable along `parents:` from the circle that
/// names `digest`.
fn commit_pred_from_circles(dir: &Path, digest: &str, skip: &BTreeSet<String>) -> Option<String> {
    let sp = dir.join(".oo").join("savepoints");
    let rd = fs::read_dir(&sp).ok()?;
    let mut nodes: BTreeMap<String, (Vec<String>, Option<String>)> = BTreeMap::new();
    for e in rd.flatten() {
        let p = e.path();
        if !p.is_file() {
            continue;
        }
        let name = p.file_name()?.to_str()?.to_string();
        if name == "LOG" || name.starts_with('.') {
            continue;
        }
        let text = fs::read_to_string(&p).ok()?;
        let parents =
            nlang_interpreter::store_codec::parse_savepoint_parents(&text).unwrap_or_default();
        let commit = nlang_interpreter::store_codec::parse_savepoint_commit(&text);
        nodes.insert(name, (parents, commit));
    }
    let start = nodes
        .iter()
        .find(|(_, (_, c))| c.as_deref() == Some(digest))?
        .0
        .clone();
    let mut seen = BTreeSet::new();
    seen.insert(start.clone());
    let mut q: VecDeque<String> = nodes.get(&start)?.0.clone().into();
    while let Some(pid) = q.pop_front() {
        if !seen.insert(pid.clone()) {
            continue;
        }
        let Some((parents, commit)) = nodes.get(&pid) else {
            continue;
        };
        if let Some(d) = commit {
            if d != digest && !skip.contains(d) {
                return Some(d.clone());
            }
        }
        q.extend(parents.iter().cloned());
    }
    None
}

fn bytes_of(dir: &Path, set: &BTreeSet<String>) -> u64 {
    let all = store_map(dir);
    set.iter().filter_map(|d| all.get(d)).sum()
}

/// A workspace with `n` generations committed.
fn repo_with_history(tag: &str, n: usize) -> nlang_interpreter::ScratchDir {
    let d = fresh_dir(tag);
    oo(&d, &["run", "--help"]);
    for i in 1..=n {
        write(&d, "u.n", &format!("gen{i}: {{ n: {i} }}\n"));
        oo(&d, &["evolve", "u.n"]);
        let out = oo(&d, &["commit", "-m", &format!("gen{i}")]);
        assert!(
            out.contains("hash:"),
            "LIVENESS: generation {i} did not commit: {out}"
        );
    }
    d
}

fn head_commit_caid(dir: &Path) -> String {
    oo(dir, &["log"])
        .lines()
        .find_map(|l| l.strip_prefix("commit ").map(|s| s.trim().to_string()))
        .unwrap_or_default()
}

fn oldest_commit_caid(dir: &Path) -> String {
    oo(dir, &["log"])
        .lines()
        .filter_map(|l| l.strip_prefix("commit ").map(|s| s.trim().to_string()))
        .last()
        .unwrap_or_default()
}

fn root_digest(dir: &Path) -> String {
    let c = head_commit_caid(dir);
    assert!(c.starts_with("hash:sha256:"), "no HEAD commit in {dir:?}");
    let p = object_path(dir, &digest_of_caid(&c));
    let commit: serde_json::Value = nlang_interpreter::store_codec::object_json_view(
        &fs::read(&p).unwrap_or_else(|e| panic!("{p:?}: {e}")),
    )
    .unwrap();
    let dg = &commit["root"]["digest"];
    let hex = if let Some(s) = dg.as_str() {
        s.to_string()
    } else if let Some(a) = dg.as_array() {
        a.iter()
            .map(|b| format!("{:02x}", b.as_u64().unwrap()))
            .collect()
    } else {
        panic!("commit root has no usable digest: {}", commit["root"]);
    };
    assert_eq!(hex.len(), 64, "root digest is not 64 hex: {hex:?}");
    hex
}

// ════════════════════════════════════════════════════════════════════════
//  CONTROL — green at v0.2.52 and after; it is what makes the reds mean
//  anything at all
// ════════════════════════════════════════════════════════════════════════

/// Before anything makes objects unreachable, **everything** is reachable.
///
/// A walker that silently finds nothing would report a pristine store as 100%
/// garbage — and every red below would then "pass" by deleting the universe.
/// This control is the only thing standing between those two readings.
#[test]
fn reachable_before_any_squash_is_everything() {
    let d = repo_with_history("control", 4);
    let all = store_map(&d);
    let live = reachable(&d, false);
    assert!(
        all.len() >= 4,
        "HARNESS: store too small to mean anything: {}",
        all.len()
    );
    assert_eq!(
        live.len(),
        all.len(),
        "the walker reached {} of {} objects in a store where nothing has been \
         orphaned — it is not finding references, and every measurement built \
         on it would be fiction",
        live.len(),
        all.len()
    );
}

// ════════════════════════════════════════════════════════════════════════
//  RED — must fail on v0.2.52, for the reason stated
// ════════════════════════════════════════════════════════════════════════

/// R1 — GC removes exactly what is unreachable.
///
/// Baseline: `oo gc` does not exist.
#[test]
fn r1_gc_removes_exactly_the_unreachable_set() {
    let d = repo_with_history("r1", 5);
    let base = oldest_commit_caid(&d);
    oo(&d, &["squash", &base, "--grant", "squash"]);

    let before = store_map(&d);
    let live = reachable(&d, false);
    let dead: BTreeSet<String> = before
        .keys()
        .filter(|k| !live.contains(*k))
        .cloned()
        .collect();
    assert!(
        !dead.is_empty(),
        "HARNESS: the squash orphaned nothing, so there is nothing to collect"
    );

    let out = oo(&d, &["gc", "--grant", "gc"]);
    let after = store_map(&d);
    let removed: BTreeSet<String> = before
        .keys()
        .filter(|k| !after.contains_key(*k))
        .cloned()
        .collect();

    assert_eq!(
        removed, dead,
        "GC removed a different set than the unreachable one\n{out}"
    );
    let freed: u64 = before
        .iter()
        .filter(|(k, _)| dead.contains(*k))
        .map(|(_, v)| *v)
        .sum();
    assert!(
        out.contains(&dead.len().to_string()),
        "the report does not state how many objects were removed: {out}"
    );
    assert!(
        out.contains(&freed.to_string()) || out.contains(&format!("{}", freed / 1024)),
        "the report does not state the {freed} bytes freed: {out}"
    );
    // `bytes_of` is the same figure computed from the live side; they must agree.
    let total: u64 = before.values().sum();
    assert_eq!(
        total - bytes_of(&d, &live),
        freed,
        "the probe's own two byte figures disagree — fix the probe before \
         reading anything into the engine's"
    );
}

/// P5 — nothing reachable is ever removed, and the universe still works.
///
/// **Moved from the reds at calibration.** It passed at v0.2.52 for the wrong
/// reason — nothing removes anything yet — and a probe that is green because
/// the feature is absent has measured nothing. It belongs here, as an active
/// pin: R1 forces removal to happen, and this forbids removing the wrong
/// things. The pair is what makes the sweep falsifiable; the labels just have
/// to say which half is which.
#[test]
fn p5_nothing_reachable_is_ever_removed() {
    let d = repo_with_history("r2", 5);
    let base = oldest_commit_caid(&d);
    oo(&d, &["squash", &base, "--grant", "squash"]);

    let live = reachable(&d, false);
    let root_before = root_digest(&d);
    oo(&d, &["gc", "--grant", "gc"]);

    let after = store_map(&d);
    for keep in &live {
        assert!(
            after.contains_key(keep),
            "GC removed a REACHABLE object {keep}"
        );
    }
    assert_eq!(root_digest(&d), root_before, "the root moved during a GC");

    // …and the universe is still usable.
    write(&d, "u.n", "after_gc: { n: 7 }\n");
    oo(&d, &["evolve", "u.n"]);
    let out = oo(&d, &["commit", "-m", "after gc"]);
    assert!(
        out.contains("hash:"),
        "the universe stopped committing after GC: {out}"
    );
}

/// R3 — abandoned-head content is collected, and the log says so.
///
/// Both halves are the ruling: the bytes go (R-b), and the fact stays —
/// visibly marked, not silently broken.
///
/// Baseline: `oo gc` does not exist.
#[test]
fn r3_abandoned_content_is_collected_and_the_log_admits_it() {
    let d = repo_with_history("r3", 3);
    let target = oldest_commit_caid(&d);
    oo(&d, &["rollback", &target, "--grant", "rollback"]);
    write(&d, "u.n", "after: { n: 99 }\n");
    oo(&d, &["evolve", "u.n"]);
    oo(&d, &["commit", "-m", "after rollback"]);

    let log_before = oo(&d, &["log"]);
    let abandoned_line = log_before
        .lines()
        .find(|l| l.trim().starts_with("abandoned "))
        .unwrap_or_else(|| panic!("LIVENESS: no abandoned line to test:\n{log_before}"))
        .trim()
        .to_string();
    let abandoned_caid = abandoned_line
        .split_whitespace()
        .nth(1)
        .unwrap()
        .to_string();
    let abandoned_digest = digest_of_caid(&abandoned_caid);

    // Following `abandoned` keeps strictly more alive than not following it —
    // otherwise this test is measuring nothing.
    let with = reachable(&d, true);
    let without = reachable(&d, false);
    assert!(
        with.len() > without.len(),
        "HARNESS: the abandoned head reaches nothing extra, so the ruling has \
         no consequence to measure here"
    );

    oo(&d, &["gc", "--grant", "gc"]);
    let after = store_map(&d);
    assert!(
        !after.contains_key(&abandoned_digest),
        "the abandoned head's own object survived — it is not a root (R-b)"
    );

    let log_after = oo(&d, &["log"]);
    assert!(
        log_after.contains(&abandoned_caid),
        "the abandoned line vanished — the fact must survive even when the \
         content does not:\n{log_after}"
    );
    assert!(
        log_after.contains("collected"),
        "the log names content the store no longer holds and does not say so:\n{log_after}"
    );
}

/// P6 — uncommitted work survives a collection.
///
/// **Moved from the reds at calibration**, for the same reason as P5.
///
/// `staged` inlines its value and names no CAID, so a sweep driven purely by
/// reachability leaves it intact by luck rather than by design. This pins the
/// outcome, not the luck.
#[test]
fn p6_staged_work_survives_collection() {
    let d = repo_with_history("r4", 4);
    let base = oldest_commit_caid(&d);
    oo(&d, &["squash", &base, "--grant", "squash"]);

    write(&d, "u.n", "pending: { n: 42 }\n");
    oo(&d, &["evolve", "u.n"]);
    let staged_before = workset_snapshot(&d);
    assert!(
        !staged_before.is_empty(),
        "nothing in the working set after evolve"
    );

    oo(&d, &["gc", "--grant", "gc"]);

    let staged_after = workset_snapshot(&d);
    assert_eq!(staged_before, staged_after, "GC modified the working set");
    let out = oo(&d, &["commit", "-m", "staged survived"]);
    assert!(
        out.contains("hash:"),
        "staged work no longer commits after GC: {out}"
    );
}

/// R5 — nothing collects by itself.
///
/// Discussion 025: forgetting must not happen automatically, and that is
/// structural rather than cautious. Every ordinary command must leave a
/// garbage-holding store byte-identical.
///
/// Baseline: v0.2.52 collects nothing anywhere, so this is **green at
/// baseline for the right reason and must stay green** — it is listed among
/// the reds only because it can start failing the moment GC exists.
#[test]
fn r5_no_command_collects_by_itself() {
    let d = repo_with_history("r5", 4);
    let base = oldest_commit_caid(&d);
    oo(&d, &["squash", &base, "--grant", "squash"]);

    let before = store_map(&d);
    let live = reachable(&d, false);
    assert!(
        before.len() > live.len(),
        "HARNESS: no garbage present, so 'nothing collected it' proves nothing"
    );

    write(&d, "u.n", "idle: { n: 1 }\n");
    for args in [
        vec!["log"],
        vec!["status"],
        vec!["run", "u.n"],
        vec!["evolve", "u.n"],
        vec!["commit", "-m", "tick"],
        vec!["log"],
    ] {
        oo(&d, &args);
        let now = store_map(&d);
        for (k, _) in &before {
            assert!(
                now.contains_key(k),
                "`oo {}` collected object {k} — forgetting must never happen \
                 as a side effect",
                args.join(" ")
            );
        }
    }
}

/// R6 — GC is privileged.
///
/// `#squash` merely makes objects unreachable and needs `--grant squash`.
/// Deleting the bytes is at least as consequential.
///
/// Baseline: `oo gc` does not exist, so the subcommand is unrecognised —
/// which is red on the "removes nothing AND names the grant" half.
#[test]
fn r6_gc_requires_its_grant() {
    let d = repo_with_history("r6", 4);
    let base = oldest_commit_caid(&d);
    oo(&d, &["squash", &base, "--grant", "squash"]);
    let before = store_map(&d);

    let (out, ok) = oo_raw(&d, &["gc"]);
    assert!(!ok, "ungranted GC exited successfully: {out}");
    assert_eq!(store_map(&d), before, "ungranted GC removed objects anyway");
    assert!(
        out.contains("gc"),
        "the refusal does not name the missing grant: {out}"
    );

    // …and with the grant it does work, so the refusal is a gate and not a
    // permanent inability.
    oo(&d, &["gc", "--grant", "gc"]);
    assert!(
        store_map(&d).len() < before.len(),
        "granted GC removed nothing"
    );
}

/// R7 — the store announces its format, and an unknown one is refused.
///
/// Baseline: `.oo/format` does not exist and nothing reads it.
///
/// ACCEPTOR EDIT (Q-010a, 2026-08-13). Two literals here were countdown
/// timers, not invariants. The declared format was pinned to `"1"` and the
/// "unknown" format used for the refusal half was `"2"` — the next number,
/// which O48 has now made the *current* one. Both move with the ruling; the
/// property does not, and the property is what this probe is for:
///
///   * the store SAYS what format it is, and
///   * an engine that cannot read that format refuses AND CHANGES NOTHING.
///
/// The unknown-format fixture is now far outside the readable range so that
/// it does not have to be revisited on every bump.
// ACCEPTOR (Q-011, 2026-08-14). The property is unchanged — the store SAYS
// what it is, and an engine that cannot read it refuses AND CHANGES NOTHING.
// What changed is that there are now TWO declarations (O23): `.oo/format`
// carries the layout, `.oo/objects.format` the object encoding. The old probe
// compared the file to a constant named `STORE_FORMAT_VERSION`, which is gone
// — and which by the end had become a lie: it read `3` while the file said
// `layout=2`.
//
// The rewrite pins no literal. Each declaration must exist, must say which
// axis it measures, and must be individually load-bearing: corrupting either
// one alone has to stop the engine.
#[test]
fn r7_both_declarations_are_present_and_each_one_is_enforced() {
    let d = repo_with_history("r7", 2);
    let layout = d.join(".oo").join("format");
    let encoding = d.join(".oo").join("objects.format");

    for (path, axis) in [(&layout, "layout"), (&encoding, "encoding")] {
        let text = fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("{axis} declaration is missing: {e}"));
        assert!(
            text.contains(axis),
            "the {axis} declaration does not say which axis it measures: {text:?}"
        );
    }

    // Each declaration is enforced on its own. `99` is chosen to be outside
    // any readable range on either axis, so it does not need revisiting on a
    // bump.
    for (path, axis) in [(&layout, "layout"), (&encoding, "encoding")] {
        let before = store_map(&d);
        let original = fs::read_to_string(path).unwrap();
        fs::write(path, format!("{axis}=99\n")).unwrap();

        for args in [vec!["log"], vec!["status"], vec!["gc", "--grant", "gc"]] {
            let (out, ok) = oo_raw(&d, &args);
            assert!(
                !ok,
                "`oo {}` proceeded against a store whose {axis} it cannot read: {out}",
                args.join(" ")
            );
            assert!(
                out.contains("99"),
                "the refusal does not name what it could not read: {out}"
            );
        }
        assert_eq!(
            store_map(&d),
            before,
            "an engine that could not read the {axis} declaration still changed the store"
        );
        fs::write(path, original).unwrap();
    }
}

/// R8 — idempotent.
#[test]
fn r8_a_second_gc_frees_nothing() {
    let d = repo_with_history("r8", 5);
    let base = oldest_commit_caid(&d);
    oo(&d, &["squash", &base, "--grant", "squash"]);
    oo(&d, &["gc", "--grant", "gc"]);
    let after_first = store_map(&d);
    let out = oo(&d, &["gc", "--grant", "gc"]);
    assert_eq!(store_map(&d), after_first, "a second GC removed more");
    assert!(
        out.contains('0'),
        "a GC that freed nothing did not say so: {out}"
    );
}

/// R9 — a clean store says so rather than saying nothing.
#[test]
fn r9_a_clean_store_reports_nothing_to_do() {
    let d = repo_with_history("r9", 3);
    let all = store_map(&d);
    assert_eq!(
        reachable(&d, false).len(),
        all.len(),
        "HARNESS: this store already has garbage"
    );
    let out = oo(&d, &["gc", "--grant", "gc"]);
    assert_eq!(
        store_map(&d),
        all,
        "GC removed something from a clean store"
    );
    assert!(!out.is_empty(), "GC on a clean store said nothing at all");
    assert!(
        out.contains('0'),
        "GC on a clean store did not report zero: {out}"
    );
}

/// R10 — a corrupt but **reachable** object is reported, not swept.
///
/// The failure this prevents: an object that cannot be decoded reaches
/// nothing, so a naive mark phase files it under garbage — and the sweep then
/// deletes precisely the object the operator most needed to be told about.
/// Corruption is a REAL_03 §6.6 integrity incident, not a collection.
///
/// Baseline: `oo gc` does not exist.
#[test]
fn r10_a_corrupt_reachable_object_is_reported_not_swept() {
    let d = repo_with_history("r10", 4);
    let live: Vec<String> = reachable(&d, false).into_iter().collect();
    // The root tree of HEAD — reachable by construction, and not the commit
    // itself, so `oo log` still works.
    let victim = root_digest(&d);
    assert!(
        live.contains(&victim),
        "HARNESS: the victim is not reachable"
    );

    let p = object_path(&d, &victim);
    let original = fs::read(&p).unwrap();
    fs::write(&p, b"{ this is not a value }").unwrap();

    let out = oo(&d, &["gc", "--grant", "gc"]);
    assert!(
        p.exists(),
        "GC swept an object it could not decode — corruption became deletion"
    );
    assert_ne!(
        fs::read(&p).unwrap(),
        original,
        "HARNESS: the corruption did not take"
    );
    assert!(
        out.contains(&victim[..16])
            || out.to_lowercase().contains("integrity")
            || out.to_lowercase().contains("undecodable"),
        "GC did not report the undecodable object: {out}"
    );
}

/// R11 — `--dry-run` reports the same and removes nothing.
#[test]
fn r11_dry_run_reports_without_removing() {
    let d = repo_with_history("r11", 5);
    let base = oldest_commit_caid(&d);
    oo(&d, &["squash", &base, "--grant", "squash"]);

    let before = store_map(&d);
    let dry = oo(&d, &["gc", "--grant", "gc", "--dry-run"]);
    assert_eq!(store_map(&d), before, "--dry-run removed objects");

    let dead = before.len() - reachable(&d, false).len();
    assert!(
        dry.contains(&dead.to_string()),
        "--dry-run did not report the {dead} collectable objects: {dry}"
    );
    let real = oo(&d, &["gc", "--grant", "gc"]);
    assert!(
        real.contains(&dead.to_string()),
        "the real run reported a different count than --dry-run:\ndry: {dry}\nreal: {real}"
    );
}

/// R12 — every survivor still verifies against its own address.
///
/// A sweep that rewrites or truncates neighbours would leave a store that
/// looks smaller and reads wrong.
#[test]
fn r12_survivors_still_verify() {
    let d = repo_with_history("r12", 5);
    let base = oldest_commit_caid(&d);
    oo(&d, &["squash", &base, "--grant", "squash"]);
    oo(&d, &["gc", "--grant", "gc"]);

    for (digest, _) in store_map(&d) {
        let caid = format!("hash:sha256:v1:{digest}");
        let out = oo(&d, &["inspect", &caid]);
        assert!(
            !out.contains("caid_mismatch") && !out.contains("undecodable"),
            "surviving object {digest} no longer verifies after GC: {out}"
        );
    }
}

// ════════════════════════════════════════════════════════════════════════
//  PINS — green at v0.2.52, must stay green
// ════════════════════════════════════════════════════════════════════════

/// P1 — history operations behave as before.
#[test]
fn p1_history_ops_unchanged() {
    let d = repo_with_history("p1", 4);
    let target = oldest_commit_caid(&d);

    let (out, ok) = oo_raw(&d, &["rollback", &target]);
    assert!(
        !ok || out.contains("privileged") || out.contains("grant"),
        "rollback without a grant was allowed: {out}"
    );

    let out = oo(&d, &["rollback", &target, "--grant", "rollback"]);
    assert!(out.contains("hash:"), "granted rollback failed: {out}");
    assert_eq!(head_commit_caid(&d), target, "rollback did not move HEAD");

    write(&d, "u.n", "post: { n: 5 }\n");
    oo(&d, &["evolve", "u.n"]);
    oo(&d, &["commit", "-m", "post"]);
    assert!(
        oo(&d, &["log"]).contains("abandoned"),
        "the abandoned head stopped being recorded"
    );
}

/// P2 — universe determinism: two fresh workspaces, one source, one root.
#[test]
fn p2_universe_determinism() {
    let src = "world: {\n  greet: \"hello\"\n  n: 7\n}\n";
    let mut roots = Vec::new();
    for i in 0..2 {
        let d = fresh_dir(&format!("p2-{i}"));
        oo(&d, &["run", "--help"]);
        write(&d, "u.n", src);
        oo(&d, &["evolve", "u.n"]);
        let out = oo(&d, &["commit", "-m", "p2"]);
        assert!(
            out.contains("hash:"),
            "LIVENESS: p2 commit {i} failed: {out}"
        );
        roots.push(root_digest(&d));
    }
    assert_eq!(roots[0], roots[1], "same source, two roots");
}

/// P3 — `#squash` still compresses history and still needs its grant.
#[test]
fn p3_squash_unchanged() {
    let d = repo_with_history("p3", 5);
    let base = oldest_commit_caid(&d);
    let before = oo(&d, &["log"]).matches("commit ").count();

    let (out, ok) = oo_raw(&d, &["squash", &base]);
    assert!(
        !ok || out.contains("grant") || out.contains("privileged"),
        "ungranted squash was allowed: {out}"
    );

    oo(&d, &["squash", &base, "--grant", "squash"]);
    let after = oo(&d, &["log"]).matches("commit ").count();
    assert!(
        after < before,
        "squash did not compress history: {before} → {after}"
    );
}

/// P4 — nothing durable appears under `.oo/` beyond what this arc declares.
///
/// The pin the kademlia arc got wrong was written about one path instead of
/// the property. This one is written about the property: an allow-list, so a
/// new file has to be added here deliberately.
#[test]
fn p4_no_undeclared_durable_state() {
    let d = repo_with_history("p4", 3);
    write(&d, "u.n", "x: { n: 1 }\n");
    oo(&d, &["evolve", "u.n"]);
    oo(&d, &["log"]);

    let allowed = [
        "objects",
        "HEAD",
        "staged",
        "architects.json",
        "pin_pending",
        "effect_pending",
        "abandoned",
        "format",         // declared by local_gc; carries the LAYOUT axis since Q-011
        "objects.format", // declared by Q-011 (O23): the object-encoding axis
        "savepoints",     // declared by Q-013 (D43): a savepoint is durable, so it
        // outlives commit and is NOT content-addressed (a local id, not a CAID)
        "injections", // declared by Q-014 (D48): the working set is a set of
        // immutable injection files, not a rewritten `.oo/staged`
        "peers", // declared by advert_persistence (scheduled pin update)
    ];
    let mut unexpected = Vec::new();
    for e in fs::read_dir(d.join(".oo")).unwrap().flatten() {
        let n = e.file_name().to_string_lossy().to_string();
        if !allowed.contains(&n.as_str()) {
            unexpected.push(n);
        }
    }
    assert!(
        unexpected.is_empty(),
        "undeclared durable state under `.oo/`: {unexpected:?}. Every file here \
         is something a future engine must migrate and something a GC must \
         decide about — it does not get to arrive as a side effect"
    );
}
