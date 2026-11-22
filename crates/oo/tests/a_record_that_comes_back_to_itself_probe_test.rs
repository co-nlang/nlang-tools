// Q-135 probe: a record that comes back to itself.
//
// When the universe forces a value for its own bookkeeping (the scratch
// write, observing a write, the late bottoms, the commit's projection and
// bottom check), each cell it passes has its own budget (SPEC_10 §2.3.1,
// Q-120): what one cell comes to does not depend on how many others the walk
// holds. A cell is a cell as written. A record that holds a way back to
// itself by two paths — a field reading its sibling, a closure's frame, a
// list — was walked once per path: 2^(256 / cycle length) cells, each with
// a fresh budget. (v0.121.0 to v0.134.0: `a: { e: x, x: a }` does not
// return from commit, and from v0.123.0 not from evolve either; v0.120.0
// walked it on one meter.)
//
// r1 — a record with two ways back to itself evolves and commits; written
//      in one evolve or split, in either order, it stores one root; its
//      fields read what was written.
// r2 — the walk costs the records written, not the ways round them: eight
//      ways back, a cycle of 5 records and one of 129 each evolve and commit
//      within 20 s.
// g1 — what is stored and read does not change: shapes with one way back,
//      a record read twice, records calls make, recursion, bottoms three
//      levels down, keep the root and readings v0.134.0 gives.
//
// R-1 (D132: a late bottom is counted per cell). A walk that left a thunk
// once it had walked it walked the next way to it again: in a ring where
// every record has two ways to the next, 2^L. And a bottom read along every
// path is a coordinate per path, as many as the paths.
//
// r3 — evolve costs the cells written, however many ways each record has
//      to the next: rings of 64 records with two ways (a field, its
//      sibling, a list, a closure's frame, itself), 32 with four, 16 through
//      a call; a chain of 2000 records each holding a bottom; each evolve
//      within 20 s. A ring with two ways that closes over a bottom is
//      refused as the ring with one way is.
// r4 — D132: a write is refused when a cell it did not write reads a bottom
//      it did not read, at the coordinate the cell reads it (the shortest
//      way); a cell that already read that bottom gains nothing. The commit
//      lists each bottom once, at its own cell.
//
// Not here: reading such a record whole (`oo eval '{ a: { e: x, x: a } }.a'`)
// copies the value at each level of one observation and runs out of memory
// — v0.120.0 does too; the Inbox row on values not shared. Committing a
// ring where every record has two ways: the commit's projection forces each
// pure cell whole to see whether it is ⊥, and an unshared value of such a
// ring is 2^L (the same Inbox row; R-1 scope, user 2026-11-22).

use std::fs;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const ENV: &str = "~%Env.get \"Q135_A\"";
// r3 / r4: what the reference (q135-r1-ref) answers; the coordinate is the
// shortest way the cell reads the bottom.
const WANT_CLOSE_ONE: &str = "e0 e1 | Error: Evolution Conflict in \"f1.n\": #conflict at n4.w0.w0.w0.w0.w0.z";
const WANT_CLOSE_TWO: &str = "e0 e1 | Error: Evolution Conflict in \"f1.n\": #conflict at n4.w0.w0.w0.w0.w0.z";
const WANT_LATE_RECORD: &str = "e0 c0 e1 | Error: Evolution Conflict in \"f1.n\": #conflict at a.w.z";
const WANT_LATE_SECOND: &str = "e0 c0 e1 | Error: Evolution Conflict in \"f1.n\": #conflict at a.x.z";
const WANT_LATE_RING: &str = "e0 c0 e1 | Error: Evolution Conflict in \"f1.n\": #conflict at n4.w0.w0.w0.w0.w0.z";
const WANT_LATE_NAME: &str = "e0 c0 e1 | Error: Evolution Conflict in \"f1.n\": #conflict at q";
const WANT_LATE_NONE: &str = "e0 c0 e0 | ";
const WANT_LATE_SHORT: &str = "e0 c0 e1 | Error: Evolution Conflict in \"f1.n\": #conflict at a.p.z";
const WANT_LIST_RECORD: &str = "c.z";

struct Ws {
    _scratch: nlang_interpreter::ScratchDir,
    root: std::path::PathBuf,
    ws: std::path::PathBuf,
}

impl Ws {
    fn new(tag: &str) -> Self {
        let scratch = nlang_interpreter::ScratchDir::new(&format!("comes-back-{tag}"));
        let root = scratch.path().to_path_buf();
        let ws = root.join("ws");
        fs::create_dir_all(&ws).unwrap();
        Ws { _scratch: scratch, root, ws }
    }

    /// Run `oo` capped at 2 GB, killed after `kill`: (output, rc). The output
    /// goes to a file, not a pipe (a printed value can pass 64 KB).
    fn oo(&self, args: &[&str], kill: Duration) -> (String, i32) {
        let out_path = self.root.join("out.txt");
        let out = fs::File::create(&out_path).expect("output file");
        let err = out.try_clone().expect("output file");
        let mut cmd = Command::new("sh");
        cmd.arg("-c")
            .arg("ulimit -v 2000000; exec \"$0\" \"$@\"")
            .arg(env!("CARGO_BIN_EXE_oo"))
            .args(args)
            .current_dir(&self.ws)
            .env("OO_IDENTITY", self.root.join("identity"))
            .env("OO_NODE_HOME", self.root.join("node-home"))
            .env("Q135_A", "alpha")
            .env_remove("OO_Q117_SHADOW")
            .stdin(Stdio::null())
            .stdout(Stdio::from(out))
            .stderr(Stdio::from(err));
        let mut child = cmd.spawn().expect("oo runs");
        let start = Instant::now();
        loop {
            if child.try_wait().expect("wait").is_some() {
                break;
            }
            if start.elapsed() > kill {
                let _ = child.kill();
                let _ = child.wait();
                return ("DID NOT RETURN".into(), -1);
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let status = child.wait().expect("wait");
        let text = String::from_utf8_lossy(&fs::read(&out_path).unwrap_or_default()).to_string();
        let rc = status.code().unwrap_or(-1);
        if text.contains("panicked") {
            return ("PANIC".into(), rc);
        }
        if text.contains("stack overflow") || text.contains("memory allocation") || (rc != 0 && text.trim().is_empty()) {
            return ("CRASHED".into(), rc);
        }
        (text, rc)
    }

    fn code(o: &str, rc: i32, letter: &str) -> String {
        if o == "DID NOT RETURN" || o == "CRASHED" || o == "PANIC" { o.to_string() } else { format!("{letter}{rc}") }
    }

    fn file(&self, name: &str, src: &str) {
        fs::write(self.ws.join(name), format!("{src}\n")).unwrap();
    }

    /// The committed root's address, last 16 hex digits.
    fn root_tail(&self) -> String {
        let head = fs::read_to_string(self.ws.join(".oo").join("HEAD")).unwrap_or_default();
        let (o, _) = self.oo(&["inspect", head.trim()], Duration::from_secs(60));
        o.lines()
            .find(|l| l.contains("root:"))
            .and_then(|l| l.split_whitespace().last())
            .map(|r| r[r.len().saturating_sub(16)..].to_string())
            .unwrap_or_else(|| "-".into())
    }

    /// Evolve each file in turn, then commit, each within `kill`: the rcs.
    fn steps(&self, files: &[String], kill: Duration) -> Vec<String> {
        let mut rcs = Vec::new();
        for (k, src) in files.iter().enumerate() {
            let f = format!("f{k}.n");
            self.file(&f, src);
            let (o, rc) = self.oo(&["evolve", &f], kill);
            rcs.push(Ws::code(&o, rc, "e"));
        }
        let (o, rc) = self.oo(&["commit", "-m", "a"], kill);
        rcs.push(Ws::code(&o, rc, "c"));
        rcs
    }

    /// Evolve each file in turn, each within `kill`: the rc letter and what
    /// it printed, squashed.
    fn evolve_each(&self, files: &[String], kill: Duration) -> Vec<(String, String)> {
        let mut out = Vec::new();
        for (k, src) in files.iter().enumerate() {
            let f = format!("f{k}.n");
            self.file(&f, src);
            let (o, rc) = self.oo(&["evolve", &f], kill);
            out.push((Ws::code(&o, rc, "e"), squash(&o)));
        }
        out
    }

    fn read(&self, q: &str) -> String {
        squash(&self.oo(&["eval", "--", q], Duration::from_secs(60)).0)
    }
}

fn squash(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn sha16(s: &str) -> String {
    hex::encode(ring::digest::digest(&ring::digest::SHA256, s.as_bytes()).as_ref())[..16].to_string()
}

fn check(bad: &mut Vec<String>, label: &str, got: String, want: &str) {
    if got != want {
        bad.push(format!("  {label}\n    want {want}\n    got  {}", got.chars().take(200).collect::<String>()));
    }
}

fn verdict(bad: Vec<String>, n: usize) {
    assert!(bad.is_empty(), "{} of {} wrong:\n{}", bad.len(), n, bad.join("\n"));
}

fn all_ok(rcs: &[String]) -> String {
    rcs.iter().map(|r| if r.ends_with('0') && r.len() == 2 { "ok".to_string() } else { r.clone() }).collect::<Vec<_>>().join(" ")
}

// ── r1 ── a record with two ways back to itself evolves and commits ────

/// r1 — Records that come back to themselves by two ways (a field reading
/// its sibling, a closure's frame, a list, two hops in one record, a record
/// a call makes, an effect on the way, a bottom beside the way back). Each
/// is written in one evolve and, where it has two records, split in both
/// orders: every evolve and the commit return within 20 s, the orders store
/// one root, and the fields read what was written. (v0.134.0: the evolve
/// does not return, or runs out of the 2 GB cap.)
#[test]
fn r1_a_record_with_two_ways_back_evolves_and_commits() {
    type Shape = (&'static str, Vec<Vec<String>>, Vec<(&'static str, String)>);
    let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    let shapes: Vec<Shape> = vec![
        ("a field reading its sibling",
         vec![s(&["a: { e: x, x: b }\nb: { y: a, f: 1 }"]), s(&["a: { e: x, x: b }", "b: { y: a, f: 1 }"]), s(&["b: { y: a, f: 1 }", "a: { e: x, x: b }"])],
         vec![("_.b.f", "1".into()), ("_.a.x.f", "1".into()), ("_.a.e.y.x.f", "1".into())]),
        ("a closure's frame",
         vec![s(&["a: { e: (n -> x), x: b }\nb: { y: a, f: 1 }"]), s(&["a: { e: (n -> x), x: b }", "b: { y: a, f: 1 }"]), s(&["b: { y: a, f: 1 }", "a: { e: (n -> x), x: b }"])],
         vec![("_.b.f", "1".into()), ("_.a.x.f", "1".into()), ("_.a.x.y.x.f", "1".into())]),
        ("itself",
         vec![s(&["a: { e: x, x: a, v: 1 }"])],
         vec![("_.a.v", "1".into()), ("_.a.x.v", "1".into()), ("_.a.e.x.v", "1".into())]),
        ("two hops in one record",
         vec![s(&["a: { e: x, x: y, y: a, v: 2 }"])],
         vec![("_.a.v", "2".into()), ("_.a.y.v", "2".into())]),
        ("a list",
         vec![s(&["a: { e: x, x: [b], v: 4 }\nb: [a]"]), s(&["a: { e: x, x: [b], v: 4 }", "b: [a]"]), s(&["b: [a]", "a: { e: x, x: [b], v: 4 }"])],
         vec![("_.a.v", "4".into())]),
        ("a record a call makes",
         vec![s(&["f: (n -> { v: n, back: r })\nr: { e: x, x: f 1 }"]), s(&["f: (n -> { v: n, back: r })", "r: { e: x, x: f 1 }"]), s(&["r: { e: x, x: f 1 }", "f: (n -> { v: n, back: r })"])],
         vec![("_.r.x.v", "1".into()), ("_.r.x.back.x.v", "1".into())]),
        ("an effect on the way",
         vec![vec![format!("a: {{ e: x, x: b }}\nb: {{ y: a, f: {ENV} }}")], vec!["a: { e: x, x: b }".into(), format!("b: {{ y: a, f: {ENV} }}")]],
         vec![("_.b.f", "\"alpha\" ;; %effect: #io".into()), ("_.a.x.f", "\"alpha\" ;; %effect: #io".into())]),
        ("a bottom beside the way back",
         vec![s(&["a: { e: x, x: a, z: 1 & 2 }"])],
         vec![("_.a.z", "_|_ ;; %cause: #conflict".into())]),
    ];
    let mut bad = Vec::new();
    let mut cells = 0;
    let t = Duration::from_secs(20);
    for (shape, orders, reads) in &shapes {
        let mut roots = Vec::new();
        for (k, files) in orders.iter().enumerate() {
            let w = Ws::new("r1");
            let rcs = w.steps(files, t);
            let order = if orders.len() == 1 { String::new() } else { format!(", order {}", k + 1) };
            check(&mut bad, &format!("{shape}{order}: each step within 20 s"), all_ok(&rcs), &vec!["ok"; rcs.len()].join(" "));
            cells += 1;
            if rcs.iter().all(|r| r.ends_with('0') && r.len() == 2) {
                roots.push(w.root_tail());
                for (q, want) in reads {
                    check(&mut bad, &format!("{shape}{order}: {q}"), w.read(q), want);
                    cells += 1;
                }
            }
        }
        if roots.len() > 1 {
            check(&mut bad, &format!("{shape}: one root in every order"), format!("{}", roots.iter().all(|r| r == &roots[0])), "true");
            cells += 1;
        }
    }
    verdict(bad, cells);
}

// ── r2 ── the walk costs the records written ───────────────────────────

/// r2 — Eight fields each reading the record they are in; a cycle of 5
/// records and one of 129, the head reading its sibling: each evolves and
/// commits within 20 s and reads `v`. (v0.134.0: 2^(256 / cycle length)
/// cells; none returns.)
#[test]
fn r2_the_walk_costs_the_records_written() {
    let ring = |l: usize| {
        let mut f = vec!["a: { e: x, x: b1, v: 1 }".to_string()];
        for i in 1..l {
            f.push(format!("b{i}: {{ y: b{} }}", i + 1));
        }
        f.push(format!("b{l}: {{ y: a }}"));
        f.join("\n")
    };
    let eight = format!("a: {{ {}, v: 1 }}", (0..8).map(|i| format!("p{i}: a")).collect::<Vec<_>>().join(", "));
    let shapes = [("eight ways back", eight), ("a cycle of 5 records", ring(4)), ("a cycle of 129 records", ring(128))];
    let mut bad = Vec::new();
    for (shape, src) in &shapes {
        let w = Ws::new("r2");
        let rcs = w.steps(&[src.clone()], Duration::from_secs(20));
        check(&mut bad, &format!("{shape}: evolve and commit within 20 s"), all_ok(&rcs), "ok ok");
        check(&mut bad, &format!("{shape}: _.a.v"), w.read("_.a.v"), "1");
    }
    verdict(bad, shapes.len() * 2);
}

// ── g1 ── what is stored and read does not change ──────────────────────

/// g1 — Shapes the walk passes without a second way back keep the root and
/// the readings v0.134.0 gives: rcs, the root's last 16 hex digits, and
/// each reading (a long one as its length and digest). Observing a value
/// with one way back whole still unrolls it to the depth horizon: the walk
/// that stops at a way back is the universe's bookkeeping, not a reading. Recursion meets the same expression in another frame at
/// each level (`f (n - 1)`): that is not a way back, and the bottom three
/// calls down is still found.
#[test]
fn g1_what_is_stored_and_read_does_not_change() {
    let hz = (0..300).map(|i| format!("k{i}: k{} + 1", i + 1)).collect::<Vec<_>>().join("\n") + "\nk300: 5";
    let blur = "#blur { %cause: #max_depth_exceeded, %caid: \"hash:sha256:v1:6a974c80646fcfce482fd9a5721a94ba4de5e99bdcefeae96e8493ee6d5809e9\" }";
    let conflict = "_|_ ;; %cause: #conflict ;; Incompatible types: 1 vs 2";
    let shapes: Vec<(&str, String, &[&str], String)> = vec![
        ("one way back", "a: { x: b, v: 1 }\nb: { y: a, f: 2 }".into(), &["_.a.x.f", "_.a.x.y.v"], "e0 c0 49a0bd0c6b191bff | 2 | 1".into()),
        ("a: b, b: [a]", "a: b\nb: [a]".into(), &["_.a", "_.b"], "e0 c0 c37f8fd2632bd278 | [_|_ ;; %cause: #divergent] | [_|_ ;; %cause: #divergent]".into()),
        ("a record read twice", "b: { v: 1 }\na: { p: b, q: b, r: [b, b] }".into(), &["_.a.q.v", "_.a.r"], "e0 c0 147da3e91eb315c1 | 1 | [{ v: 1 }, { v: 1 }]".into()),
        ("records a call makes", "f: (n -> { v: n, w: g n })\ng: (m -> m + 1)\nr: f 1\ns: f 2".into(), &["_.r.w", "_.s.w"], "e0 c0 8b3cf377373efc8e | 2 | 3".into()),
        ("recursion, a record per level", "f: (n -> (n <= 0) ? 0 : { v: n, w: f (n - 1) })\nr: f 3".into(), &["_.r.w.v", "_.r.w.w.w"], "e0 c0 608853bfefaec673 | 2 | 0".into()),
        ("recursion, the same argument", "f: (n -> { v: n, w: f n })\nr: f 1".into(), &["_.r.v", "_.r.w.w.v"], "e0 c0 f261b342870d23a6 | 1 | 1".into()),
        ("a bottom three calls down", "f: (n -> (n <= 0) ? 1 & 2 : { w: f (n - 1) })\nr: f 3".into(), &["_.r.w.w", "_.r.w.w.w"], format!("e0 c0 131e78f8603922d0 | {{ w: {conflict} }} | {conflict}")),
        ("a bottom three records down", "r: { w: { w: { w: 1 & 2 } } }".into(), &["_.r.w.w.w"], format!("e0 c0 1687c1b38e4790a1 | {conflict}")),
        ("one record, two frames", "mk: (n -> { v: n, k: (m -> v) })\nr: [mk 1, mk 2]\ns: [(r 0).k 0, (r 1).k 0]".into(), &["_.s"], "e0 c0 4fea0ef378106dd8 | [1, 2]".into()),
        ("the depth horizon", hz, &["_.k0"], format!("e0 c0 ffaac0c92aef280c | {blur}")),
        ("one way back, read whole", "a: { x: b, v: 1 }\nb: { y: a }".into(), &["#_.a"], "e0 c0 7eb4ca6eea22fe49 | 2559 317bce1e1f01a103".into()),
        ("recursion, the same argument, read whole", "f: (n -> { v: n, w: f n })\nr: f 1".into(), &["#_.r"], "e0 c0 f261b342870d23a6 | 3187 22d4e31692f39c73".into()),
    ];
    let mut bad = Vec::new();
    for (label, src, reads, want) in &shapes {
        let w = Ws::new("g1");
        let rcs = w.steps(&[src.clone()], Duration::from_secs(60));
        let mut got = format!("{} {}", rcs.join(" "), w.root_tail());
        for q in reads.iter() {
            // `#q`: a reading too long to spell here, as its length and digest.
            match q.strip_prefix('#') {
                Some(q) => {
                    let r = w.read(q);
                    got.push_str(&format!(" | {} {}", r.len(), sha16(&r)));
                }
                None => got.push_str(&format!(" | {}", w.read(q))),
            }
        }
        check(&mut bad, label, got, want);
    }
    verdict(bad, shapes.len());
}

// ── r3 ── every record has two ways to the next ────────────────────────

/// A ring of `l` records, each with `m` edges of one kind to the next.
fn ring_of(kind: &str, l: usize, m: usize) -> String {
    let edges = |i: usize, t: &str| -> Vec<String> {
        match kind {
            "a field" => (0..m).map(|j| format!("w{j}: {t}")).collect(),
            "its sibling" => std::iter::once(format!("x: {t}")).chain((1..m).map(|j| format!("e{j}: x"))).collect(),
            "a list" => (0..m).map(|j| format!("l{j}: [{t}]")).collect(),
            "a closure's frame" => std::iter::once(format!("x: {t}")).chain((1..m).map(|j| format!("k{j}: (u -> x)"))).collect(),
            "itself" => std::iter::once(format!("x: {t}")).chain((1..m).map(|j| format!("s{j}: n{i}"))).collect(),
            "a call" => (0..m).map(|j| format!("c{j}: id {t}")).collect(),
            _ => unreachable!(),
        }
    };
    let mut f = vec!["id: (x -> x)".to_string()];
    for i in 0..l {
        let mut fields = vec![format!("v: {i}")];
        fields.extend(edges(i, &format!("n{}", (i + 1) % l)));
        f.push(format!("n{i}: {{ {} }}", fields.join(", ")));
    }
    f.join("\n")
}

/// r3 — Rings where every record has two (or four) ways to the next, and a
/// chain of 2000 records each holding a bottom: each evolves within 20 s.
/// A ring with two ways, its far half written first with a bottom in it,
/// refuses the near half as the ring with one way does. (v0.134.0 and the
/// first delivery: the rings do not return from 5 records on; the chain
/// from about 1000.)
#[test]
fn r3_evolve_costs_the_cells_written() {
    let mut shapes: Vec<(String, String)> = Vec::new();
    for kind in ["a field", "its sibling", "a list", "a closure's frame", "itself"] {
        shapes.push((format!("64 records, two ways by {kind}"), ring_of(kind, 64, 2)));
    }
    shapes.push(("32 records, four ways by a field".into(), ring_of("a field", 32, 4)));
    shapes.push(("16 records, two ways by a call".into(), ring_of("a call", 16, 2)));
    let chain = (0..2000).map(|i| format!("k{i}: {{ z: 1 & 2, n: k{} }}", i + 1)).collect::<Vec<_>>().join("\n") + "\nk2000: { z: 1 & 2 }";
    shapes.push(("2000 records, each holding a bottom".into(), chain));
    let mut bad = Vec::new();
    for (shape, src) in &shapes {
        let w = Ws::new("r3");
        let out = w.evolve_each(&[src.clone()], Duration::from_secs(20));
        check(&mut bad, &format!("{shape}: evolve within 20 s"), out[0].0.clone(), "e0");
    }
    // the ring closes over a bottom its far half holds
    let near = "n0: { v: 0, w0: n1, w1: n1 }\nn1: { v: 1, w0: n2, w1: n2 }\nn2: { v: 2, w0: n3, w1: n3 }";
    let far = "n3: { v: 3, w0: n4, w1: n4 }\nn4: { v: 4, w0: n0, w1: n0, z: 1 & 2 }";
    let one = "n3: { v: 3, w0: n4 }\nn4: { v: 4, w0: n0, z: 1 & 2 }";
    let near_one = "n0: { v: 0, w0: n1 }\nn1: { v: 1, w0: n2 }\nn2: { v: 2, w0: n3 }";
    for (shape, files, want) in [
        ("one way, the ring closes over a bottom", vec![one.to_string(), near_one.to_string()], WANT_CLOSE_ONE),
        ("two ways, the ring closes over a bottom", vec![far.to_string(), near.to_string()], WANT_CLOSE_TWO),
    ] {
        let w = Ws::new("r3-close");
        let out = w.evolve_each(&files, Duration::from_secs(20));
        check(&mut bad, shape, format!("{} {} | {}", out[0].0, out[1].0, out[1].1), want);
    }
    verdict(bad, shapes.len() + 2);
}

// ── r4 ── a late bottom is counted per cell (D132) ──────────────────────

/// r4 — D132. A write is refused where a cell it did not write comes to read
/// a bottom it did not read: through a record (`a.w.z`), a second field of a
/// record that already read it by the first (`a.x.z`), a ring that closes
/// over its own bottom (the shortest way round), a record that reads it by
/// two ways (the shorter is reported). A write that reads no
/// bottom lands. The commit lists each bottom once, at its own cell: not
/// once more for every cell that reads it. (v0.134.0: the ring is reported
/// at a coordinate going round to the depth horizon; the commit lists
/// `a.w.z` beside `c.z`, and 5151 bottoms for a chain of 100 records each
/// holding one.)
#[test]
fn r4_a_late_bottom_is_counted_per_cell() {
    let cases: [(&str, [&str; 2], &str); 6] = [
        ("a record comes to read a bottom", ["a: { w: b }\nc: { z: 1 & 2 }", "b: c"], WANT_LATE_RECORD),
        ("a second field of a record that read it", ["a: { w: c, x: b }\nc: { z: 1 & 2 }", "b: c"], WANT_LATE_SECOND),
        ("a ring closes over its own bottom", ["n3: { v: 3, w0: n4 }\nn4: { v: 4, w0: n0, z: 1 & 2 }", "n0: { v: 0, w0: n1 }\nn1: { v: 1, w0: n2 }\nn2: { v: 2, w0: n3 }"], WANT_LATE_RING),
        ("a name read before it is written", ["q: c", "p: c\nc: \"x\" + 1"], WANT_LATE_NAME),
        ("a record reads it two ways, the shorter reported", ["a: { q: { r: b }, p: b }", "b: { z: 1 & 2 }"], WANT_LATE_SHORT),
        ("a write that reads no bottom", ["n3: { v: 3, w0: n4 }\nn4: { v: 4, w0: n0, z: 1 & 2 }", "n0: { v: 0, w0: n1 }\nn1: { v: 1, w0: n2 }\nn2: { v: 2, w0: q }\nq: { v: 9 }"], WANT_LATE_NONE),
    ];
    let mut bad = Vec::new();
    for (label, files, want) in cases {
        let w = Ws::new("r4");
        let f: Vec<String> = files.iter().map(|x| x.to_string()).collect();
        w.file("f0.n", &f[0]);
        let (o0, rc0) = w.oo(&["evolve", "f0.n"], Duration::from_secs(20));
        let (o1, rc1) = w.oo(&["commit", "-m", "a"], Duration::from_secs(20));
        w.file("f1.n", &f[1]);
        let (o2, rc2) = w.oo(&["evolve", "f1.n"], Duration::from_secs(20));
        let got = format!("{} {} {} | {}", Ws::code(&o0, rc0, "e"), Ws::code(&o1, rc1, "c"), Ws::code(&o2, rc2, "e"), squash(&o2));
        check(&mut bad, label, got, want);
    }
    // the commit's list
    let lists = [
        ("a record that reads a bottom", "a: { w: c }\nc: { z: 1 & 2 }".to_string(), WANT_LIST_RECORD.to_string()),
        ("100 records each holding a bottom", (0..100).map(|i| format!("k{i}: {{ z: 1 & 2, n: k{} }}", i + 1)).collect::<Vec<_>>().join("\n") + "\nk100: { z: 1 & 2 }", "101".to_string()),
    ];
    for (label, src, want) in &lists {
        let w = Ws::new("r4-list");
        w.file("f0.n", src);
        let (_, rc0) = w.oo(&["evolve", "f0.n"], Duration::from_secs(20));
        let (o, rc) = w.oo(&["commit", "-m", "a"], Duration::from_secs(20));
        let listed: Vec<&str> = o.split_whitespace().collect::<Vec<_>>().windows(3).filter(|x| x[0] == "#conflict" && x[1] == "at").map(|x| x[2]).collect();
        let got = if label.starts_with("100") { format!("{}", listed.len()) } else { listed.join(" ") };
        check(&mut bad, &format!("commit lists, {label} (rc {rc0} {rc})"), got, want);
    }
    verdict(bad, cases.len() + lists.len());
}
