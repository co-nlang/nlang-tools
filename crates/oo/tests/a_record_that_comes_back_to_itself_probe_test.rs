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
// Not here: reading such a record whole (`oo eval '{ a: { e: x, x: a } }.a'`)
// copies the value at each level of one observation and runs out of memory
// — v0.120.0 does too; the Inbox row on values not shared.

use std::fs;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const ENV: &str = "~%Env.get \"Q135_A\"";

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
