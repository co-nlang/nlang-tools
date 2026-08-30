// A history written twice.
// Rulings: nlang-spec/meta/oo/STATUS.md D52, D53, D54
//          (design: meta/oo/commit.md 1.7.8, 1.7.9; they amend 1.7.2, 1.7.7)
// Recon:   nlang-tools/docs/a_history_written_twice_recon.md (+ appendix 1)
// Order:   nlang-tools/docs/a_history_written_twice_handover.md
//
// -- What this arc is ----------------------------------------------------
//
// Two records both claim to be the history and neither can find the
// other. A circle's frame carries `parents:` and a working set and no
// CAID at all; a `Commit` carries parent/root/meta/kind/refine_info and
// no circle id. Measured: three evolves and three commits leave three
// circles holding `{ x: 1 }`, `{ y: 2 }`, `{ z: 3 }` beside a commit
// chain holding none of that.
//
// So arc A item A2's own wording -- "history traversal moves onto the
// circle chain" -- cannot be followed today: the circle chain holds no
// history. This arc puts the missing edge in.
//
// -- The three rulings that shape it -------------------------------------
//
// D52: a circle records "I became commit C"; `Commit.parent` is no longer
// SET (never removed -- `None` contributes zero bytes, commit.md 1.7.7, so
// existing CAIDs do not move). Traversal follows the commit edge.
//
// D53: the minted graph and the drawn graph need not be the same shape,
// and the drawn one must be a pure function of the minted one. This is
// what makes the commit's own circle correct rather than a cost: a commit
// is an EVENT, and 1.11 already ruled that events are not idempotent, so
// it earns its own circle. That circle has one parent and one child and
// carries one fact, so a drawing may contract it into a label on an edge.
// The same thing is a node in one graph and an edge in the other.
//
// D54: the minted graph's nodes are CIRCLES. A commit is an annotation,
// not a node. Under D52 the commits have no edges among themselves at
// all, so they carry labels, not structure -- and labels belong to the
// drawing. H1 cannot decide this (it is equal either way; see the recon
// appendix Q14), so it was ruled, not derived.
//
// -- The topology, and why it is an instrument and not a metaphor --------
//
// A history DAG is a 1-dimensional CW complex, so rank H1 = E - V + C,
// and an independent cycle is exactly one fork that later merged. D50 is
// the ruling that made holes possible at all: singular predecessors give
// a chain, and a chain has H1 = 0 always.
//
// Verified on real engine output, not on paper: seed a fork, then merge
// it with a sequential no-op. Before, V=3 E=2 C=1, H1 = 0. After, V=4
// E=4 C=1, H1 = 1, and the merging circle's frame reads
// `parents: aaaa... bbbb...`.
//
// Contracting a pass-through circle is an edge contraction, hence a
// homotopy equivalence, hence H1 is preserved. G3 is that invariant
// pointed at the one thing this arc could get wrong: if the commit's
// circle is wired with two parents instead of one, H1 jumps and a merge
// that never happened appears in the history.
//
// -- Out of scope, do not touch ------------------------------------------
//
//   * `oo log --graph`. D53's red line is about drawings, but the drawing
//     itself is a later arc. Recon appendix Q17: the probes here read the
//     directory, so nothing needs `--graph` to go red.
//   * Compare-and-swap and retry (Q-016). Recon Q12: none of the four
//     candidates solved it in passing.
//   * `oo run`/`eval` not seeing the committed universe (Q-018).
//   * The `message` field of a bottom not being covered by its CAID.
//     Measured 2026-08-31, in the Inbox, marked interrupt-candidate. It
//     is a SERVICE-plane problem (a peer can serve arbitrary text at a
//     verified address); the local face is covered by REAL_01 6.3.3.
//   * Garbage-collecting circles. Still collides with D43.
//
// -- Probe integrity ------------------------------------------------------
//
// Reds are `#[ignore]`d and MUST be red at the baseline for the reason
// stated in each. The delivery may remove `#[ignore]` and NOTHING else in
// this file -- `rustfmt` included. A fmt pass makes "the rest of the file
// is untouched" a false sentence.
//
// R1 and R2 are deliberately spelling-agnostic. R1 asks that SOME circle
// frame contain HEAD's digest; it does not pin `commit:` as the key, the
// separator, or the CAID prefix. R2 asks that the second commit's stored
// bytes NOT contain the first commit's digest; it does not pin `parent: _`
// as the spelling of absence. If you need a different on-disk shape, both
// still hold. Say so in the report if they do not.
//
// NOT PROBED, stated so no one mistakes silence for coverage:
//   * D53's actual red line, "the drawn graph's E-V+C equals the minted
//     graph's". There is no drawn graph this arc (no `--graph`), so there
//     is nothing to compare against and the assertion would be vacuously
//     green. G3 arms the half that exists. The real comparison lands in
//     the arc that builds the drawing.
//   * Mixed chains across engine versions. A repo whose commits were
//     written by v0.40.0 (with `parent` set) must still log in full under
//     the new engine. That cannot be armed inside `cargo test`, because
//     the binary under test is the new one and cannot produce old-shaped
//     commits. It is measured at ACCEPTANCE with the real v0.40.0 tag
//     binary building the repo and the new binary reading it, and the
//     order asks for the answer in writing (S7).
//   * Crash windows, including the new one D52 opens (`set_head` written,
//     commit circle not yet minted). Every finding about crash windows so
//     far came from reconstructing state by hand; a probe that fakes a
//     crash pins the faking. The order asks for the traversal to start at
//     HEAD, which closes it by construction, and for that to be stated.

use std::path::Path;
use std::process::Command;

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

fn oo_ok(dir: &Path, args: &[&str]) -> bool {
    let mut c = Command::new(env!("CARGO_BIN_EXE_oo"));
    c.current_dir(dir)
        .env("OO_IDENTITY", dir.join("identity-for-tests"))
        .env("OO_NODE_HOME", dir.join("node-home-for-tests"))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    c.args(args).status().expect("oo runs").success()
}

fn scratch(tag: &str) -> nlang_interpreter::ScratchDir {
    nlang_interpreter::ScratchDir::new(&format!("twice-{tag}"))
}

fn write(d: &Path, name: &str, body: &str) {
    std::fs::write(d.join(name), body).expect("write source");
}

/// Every regular file under `.oo/savepoints/` that is not `LOG` and does
/// not start with a dot.
fn circles(d: &Path) -> Vec<std::path::PathBuf> {
    let dir = d.join(".oo").join("savepoints");
    let mut out: Vec<std::path::PathBuf> = match std::fs::read_dir(&dir) {
        Ok(rd) => rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_file())
            .filter(|p| {
                let n = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
                n != "LOG" && !n.starts_with('.')
            })
            .collect(),
        Err(_) => Vec::new(),
    };
    out.sort();
    out
}

/// The ids named on a circle's `parents:` line. Empty for a root, and
/// empty for a v0.38/v0.39 frame that has no such line at all.
fn parents_of(p: &Path) -> Vec<String> {
    let text = match std::fs::read_to_string(p) {
        Ok(t) => t,
        Err(_) => return Vec::new(),
    };
    text.lines()
        .take_while(|l| !l.trim_start().starts_with('{'))
        .find(|l| l.trim_start().starts_with("parents:"))
        .map(|l| {
            l.trim()
                .trim_start_matches("parents:")
                .split_whitespace()
                .map(|s| s.to_string())
                .collect()
        })
        .unwrap_or_default()
}

/// rank H1 = E - V + C of the MINTED graph, whose nodes are circles and
/// whose edges are `parents:` entries (D54: a commit is an annotation, so
/// the commit edge is NOT counted here). Dangling predecessors -- ids no
/// file carries -- are not edges; they have no second endpoint.
fn h1(d: &Path) -> i64 {
    let files = circles(d);
    let names: Vec<String> = files
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
        .collect();
    let mut edges: Vec<(usize, usize)> = Vec::new();
    for (i, f) in files.iter().enumerate() {
        for par in parents_of(f) {
            if let Some(j) = names.iter().position(|n| *n == par) {
                edges.push((i, j));
            }
        }
    }
    let v = names.len();
    let e = edges.len();
    // Components by union-find on the undirected graph.
    let mut up: Vec<usize> = (0..v).collect();
    fn find(up: &mut Vec<usize>, x: usize) -> usize {
        let mut x = x;
        while up[x] != x {
            up[x] = up[up[x]];
            x = up[x];
        }
        x
    }
    for (a, b) in &edges {
        let (ra, rb) = (find(&mut up, *a), find(&mut up, *b));
        if ra != rb {
            up[ra] = rb;
        }
    }
    let mut roots = std::collections::BTreeSet::new();
    for i in 0..v {
        let r = find(&mut up, i);
        roots.insert(r);
    }
    e as i64 - v as i64 + roots.len() as i64
}

/// The 64-hex digest `.oo/HEAD` currently names, without the prefix.
fn head_digest(d: &Path) -> String {
    let raw = std::fs::read_to_string(d.join(".oo").join("HEAD")).expect("HEAD exists");
    raw.trim().rsplit(':').next().unwrap_or("").to_string()
}

/// Concatenated bytes of every object file whose name matches `digest`.
fn object_text(d: &Path, digest: &str) -> String {
    let (a, b) = digest.split_at(2);
    let p = d.join(".oo").join("objects").join("sha256").join(a).join(b);
    std::fs::read_to_string(p).unwrap_or_default()
}

// ---------------------------------------------------------------- R1 ----
//
// RED at the baseline: nothing on disk records which circle became which
// commit. Measured 2026-08-31 -- three evolves and three commits leave
// three frames reading `parents: <id>` and a working set, and no CAID
// appears in any of them. This is the whole of the arc's premise, and if
// it is green before the work starts, the premise is wrong.

#[test]
#[ignore]
fn r1_a_commit_leaves_a_circle_that_names_it() {
    let s = scratch("r1");
    let d = s.path();
    write(d, "a.n", "x: 1\n");
    assert!(oo_ok(d, &["evolve", "a.n"]), "evolve must succeed");
    assert!(oo_ok(d, &["commit", "-m", "c1"]), "commit must succeed");

    let head = head_digest(d);
    assert!(!head.is_empty(), "HEAD must name a commit");

    let naming: Vec<_> = circles(d)
        .into_iter()
        .filter(|p| std::fs::read_to_string(p).unwrap_or_default().contains(&head))
        .collect();

    assert!(
        !naming.is_empty(),
        "after a commit, some circle must name it. HEAD is {head}; \
         the {} circle frame(s) present name no commit at all",
        circles(d).len()
    );
}

// ---------------------------------------------------------------- R2 ----
//
// RED at the baseline: today a second commit's stored bytes carry
// `parent: <first commit's hash>`. D52 says stop SETTING it. Asserted on
// the object bytes rather than on `oo inspect`'s `parent:` line, because
// Q13 says that CLI line may be rewritten to name the source circle
// instead -- pinning the CLI would go red for the wrong reason.

#[test]
#[ignore]
fn r2_a_new_commit_does_not_name_its_predecessor() {
    let s = scratch("r2");
    let d = s.path();
    write(d, "a.n", "x: 1\n");
    assert!(oo_ok(d, &["evolve", "a.n"]));
    assert!(oo_ok(d, &["commit", "-m", "c1"]));
    let first = head_digest(d);

    write(d, "b.n", "y: 2\n");
    assert!(oo_ok(d, &["evolve", "b.n"]));
    assert!(oo_ok(d, &["commit", "-m", "c2"]));
    let second = head_digest(d);
    assert_ne!(first, second, "the second commit must be a new object");

    let bytes = object_text(d, &second);
    assert!(!bytes.is_empty(), "the second commit object must be readable");
    assert!(
        !bytes.contains(&first),
        "D52: a new commit must not name its predecessor, but the object \
         for {second} contains {first}"
    );
}

// ---------------------------------------------------------------- G1 ----
//
// GREEN today and must stay green. Carried forward from the previous arc:
// D51 must NOT be read as "every successful evolve mints a circle". A
// sequential no-op adds nothing. If this goes red, the directory grows
// without bound under repeated no-ops -- the fork-bomb reading.
//
// This arc puts a SECOND minting site in (the commit's own circle), which
// is exactly the kind of change that reopens it.

#[test]
fn g1_a_sequential_no_op_mints_nothing() {
    let s = scratch("g1");
    let d = s.path();
    write(d, "a.n", "x: 1\n");
    assert!(oo_ok(d, &["evolve", "a.n"]));
    let after_first = circles(d).len();

    for _ in 0..20 {
        assert!(oo_ok(d, &["evolve", "a.n"]), "a no-op evolve still succeeds");
    }

    assert_eq!(
        circles(d).len(),
        after_first,
        "twenty sequential no-op evolves must mint nothing beyond the first"
    );
}

// ---------------------------------------------------------------- G2 ----
//
// GREEN and a red line. `x: 0` is a fully solid universe; its root
// address and object count are the identity this arc must not move.
// Circles are not CAS objects (SPEC_10 3.1 identity MUST NOT), so adding
// a commit edge to a circle must leave both numbers untouched.

#[test]
fn g2_identity_is_a_red_line() {
    let s = scratch("g2");
    let d = s.path();
    write(d, "a.n", "x: 0\n");
    assert!(oo_ok(d, &["evolve", "a.n"]));
    assert!(oo_ok(d, &["commit", "-m", "identity"]));

    let objects = walkdir(&d.join(".oo").join("objects"));
    assert_eq!(objects, 3, "a solid `x: 0` universe is three objects");

    let out = oo(d, &["status"]);
    assert!(
        out.contains("7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911"),
        "the standard root digest must not move; status said:\n{out}"
    );
}

fn walkdir(p: &Path) -> usize {
    let mut n = 0;
    if let Ok(rd) = std::fs::read_dir(p) {
        for e in rd.flatten() {
            let path = e.path();
            if path.is_dir() {
                n += walkdir(&path);
            } else {
                n += 1;
            }
        }
    }
    n
}

// ---------------------------------------------------------------- G3 ----
//
// GREEN and must stay green. D53's invariant, pointed at the one thing
// this arc can get wrong.
//
// A commit's circle has ONE parent -- the tip it committed. If it is
// wired with two (say, both tips of an unmerged fork, or the tip plus
// something else), H1 rises by one and the history grows a hole that
// nothing caused. A hole means "a fork that merged"; inventing one is a
// false statement about what happened.
//
// Today this passes trivially, because a commit mints no circle at all.
// After the work it passes for the real reason. Both are green, and that
// is the point: it is an invariant, not a pin. The order says so.

#[test]
fn g3_committing_does_not_open_a_hole() {
    let s = scratch("g3");
    let d = s.path();
    write(d, "a.n", "x: 1\n");
    assert!(oo_ok(d, &["evolve", "a.n"]));
    write(d, "b.n", "y: 2\n");
    assert!(oo_ok(d, &["evolve", "b.n"]));

    let before = h1(d);
    assert_eq!(before, 0, "a linear history has no holes");

    assert!(oo_ok(d, &["commit", "-m", "c1"]));

    assert_eq!(
        h1(d),
        before,
        "committing must not change H1 = E - V + C of the minted graph: \
         a commit is not a merge"
    );
}

// ---------------------------------------------------------------- G4 ----
//
// GREEN and the one that goes to ZERO if S6 is skipped.
//
// `squash` proves its base is an ancestor of HEAD by walking
// `Commit.parent`. Once D52 stops setting that field, the walk finds
// nothing and every squash fails with "not an ancestor" -- measured in
// the recon, Q1. It is the only one of the five readers whose function
// disappears rather than degrades: `oo log` still prints HEAD, `inspect`
// still prints the object.
//
// So this green is not decoration. If the delivery ships the commit edge
// and forgets the ancestor walk, everything else looks fine and this is
// the assertion that says otherwise.

#[test]
fn g4_squash_still_reaches_its_base() {
    let s = scratch("g4");
    let d = s.path();
    write(d, "a.n", "x: 1\n");
    assert!(oo_ok(d, &["evolve", "a.n"]));
    assert!(oo_ok(d, &["commit", "-m", "c1"]));
    let base = head_digest(d);

    for (src, body) in [("b.n", "y: 2\n"), ("c.n", "z: 3\n")] {
        write(d, src, body);
        assert!(oo_ok(d, &["evolve", src]));
        assert!(oo_ok(d, &["commit", "-m", "more"]));
    }

    let arg = format!("hash:sha256:v1:{base}");
    let out = oo(d, &["squash", &arg, "--grant", "squash"]);
    assert!(
        !out.contains("not an ancestor"),
        "squash must still find its base on the graph; it said:\n{out}"
    );
}
