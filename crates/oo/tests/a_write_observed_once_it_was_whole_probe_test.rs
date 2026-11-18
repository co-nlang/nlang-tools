// A write observed once it was whole.
// Order: nlang-tools/docs/a_write_observed_once_it_was_whole_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-127 (root 1, 丙; no new ruling):
// the fields of one write are a set (SYNTAX_03 §2 #1); a field that
// observes an effect stores what it observed (D110, D46 ②), and evolve
// still observes; what a definition read, and the labels it read, are
// recorded as it reads, not predicted from its text.
//
// ── What is wrong (v0.129.0) ────────────────────────────────────────────
//
//   Evolve decides from the text of a definition what it will read and
//   which labels it will carry (`predict_effect`, Q-125's arrival order).
//   Where the prediction misses, the same fields store another root in
//   another order: `a: [p \ 1]`, `m: [a]` with `p` an IO name, readers
//   first `0736aa3c…`, name first `74d93c63…` (six forms). A name written
//   first and pinned pure afterwards leaves its readers' answers, so the
//   root is not the root of the pure name written with them (`a: [p]`).
//   A pure chain written readers first walks each reader's recipe
//   (N=2000 40 s, name first 3 s). A list holding a record with an effect
//   does the effect twice at commit; a readers-first conflict with HEAD
//   is taken by evolve and reported by commit.
//
// The other half: every arrival of an effect Q-125 measured is observed
// once, at evolve; `oo run` reads the whole write; the roots no change
// touches stay.
//
// Every `oo` here runs under a 2 GB address-space cap and a kill inside
// the probe (60 s; 120 s for the cost cell).
//
// The delivery may NOT edit this file. `rustfmt` must not touch it.

#![allow(dead_code)]

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const KILL_AFTER: Duration = Duration::from_secs(60);
const P: &str = "p: ~%Env.get \"Q127_UNSET\"";
const E: &str = "~%Io./append_file (\"a.log\", \"x\")";

struct Ws {
    _scratch: nlang_interpreter::ScratchDir,
    root: PathBuf,
    ws: PathBuf,
}

impl Ws {
    fn new(tag: &str) -> Self {
        let scratch = nlang_interpreter::ScratchDir::new(&format!("observed-whole-{tag}"));
        let root = scratch.path().to_path_buf();
        let ws = root.join("ws");
        fs::create_dir_all(&ws).unwrap();
        Ws { _scratch: scratch, root, ws }
    }

    /// Run `oo` capped at 2 GB, killed after `kill`: (output, rc, seconds).
    fn oo_for(&self, args: &[&str], kill: Duration) -> (String, i32, f64) {
        let mut child = Command::new("sh")
            .arg("-c")
            .arg("ulimit -v 2000000; exec \"$0\" \"$@\"")
            .arg(env!("CARGO_BIN_EXE_oo"))
            .args(args)
            .current_dir(&self.ws)
            .env("OO_IDENTITY", self.root.join("identity"))
            .env("OO_NODE_HOME", self.root.join("node-home"))
            .env_remove("Q127_UNSET")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("oo runs");
        let start = Instant::now();
        loop {
            if child.try_wait().expect("wait").is_some() {
                break;
            }
            if start.elapsed() > kill {
                let _ = child.kill();
                let _ = child.wait();
                return ("DID NOT RETURN".into(), -1, start.elapsed().as_secs_f64());
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let secs = start.elapsed().as_secs_f64();
        let o = child.wait_with_output().expect("output");
        let text = format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
        let rc = o.status.code().unwrap_or(-1);
        if text.contains("panicked") {
            return ("PANIC".into(), rc, secs);
        }
        if text.contains("stack overflow") || text.contains("memory allocation") || (rc != 0 && text.trim().is_empty()) {
            return ("CRASHED".into(), rc, secs);
        }
        (text.trim().to_string(), rc, secs)
    }

    fn oo(&self, args: &[&str]) -> (String, i32) {
        let (o, rc, _) = self.oo_for(args, KILL_AFTER);
        (o, rc)
    }

    fn file(&self, src: &str) -> String {
        let n = fs::read_dir(&self.ws).map(|d| d.count()).unwrap_or(0);
        let f = format!("f{n}.n");
        fs::write(self.ws.join(&f), format!("{src}\n")).unwrap();
        f
    }

    /// Evolve `src` from a new file: the exit code, or the crash.
    fn evolve(&self, src: &str) -> String {
        let f = self.file(src);
        let (o, rc) = self.oo(&["evolve", &f]);
        if o == "DID NOT RETURN" || o == "CRASHED" || o == "PANIC" { o } else { format!("e{rc}") }
    }

    fn commit(&self) -> String {
        let (o, rc) = self.oo(&["commit", "-m", "a", "--grant", "pin", "--grant", "effect_override"]);
        if o == "DID NOT RETURN" || o == "CRASHED" || o == "PANIC" { o } else { format!("c{rc}") }
    }

    fn eval(&self, e: &str) -> String {
        let s = self.oo(&["eval", e]).0;
        squash(&s)
    }

    fn root(&self) -> String {
        let head = fs::read_to_string(self.ws.join(".oo").join("HEAD")).unwrap_or_default();
        if head.trim().is_empty() {
            return String::new();
        }
        let (o, _) = self.oo(&["inspect", head.trim()]);
        o.lines()
            .find(|l| l.contains("root:"))
            .and_then(|l| l.rsplit(':').next())
            .unwrap_or("")
            .trim()
            .to_string()
    }

    /// Bytes the effect has written so far.
    fn done(&self) -> u64 {
        fs::metadata(self.ws.join("a.log")).map(|m| m.len()).unwrap_or(0)
    }
}

fn squash(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn check(bad: &mut Vec<String>, label: &str, got: String, want: &str) {
    if got != want {
        bad.push(format!("  {label}\n    want {want}\n    got  {}", got.chars().take(200).collect::<String>()));
    }
}

fn verdict(bad: Vec<String>, n: usize) {
    assert!(bad.is_empty(), "{} of {} wrong:\n{}", bad.len(), n, bad.join("\n"));
}

/// One write of `src` (its lines in the given order), committed: the
/// exit codes and the root.
fn written(src: &str) -> String {
    let w = Ws::new("written");
    let e = w.evolve(src);
    let c = w.commit();
    format!("{e} {c} {}", w.root())
}

fn reversed(src: &str) -> String {
    src.lines().rev().collect::<Vec<_>>().join("\n")
}

/// r1 — The fields of one write are a set: written readers first or name
/// first, the same fields store one root, whatever operator the reader
/// applies to an IO name (no prediction of the label it will carry).
#[test]
fn r1_one_write_one_root_whatever_the_operator() {
    let mut bad = Vec::new();
    let mut n = 0;
    for f in ["[p \\ 1]", "[p < 1]", "(p,)", "[...p]", "[(q -> p) 1]", "[p == 1 ? 1 : 2]", "{ x: p, y: [p] }"] {
        let readers = format!("a: {f}\nm: [a]\n{P}");
        let first = written(&readers);
        let last = written(&reversed(&readers));
        check(&mut bad, &format!("a: {f}, m: [a], readers first / name first"), last, &first);
        n += 1;
    }
    verdict(bad, n);
}

/// r2 — A pin that makes the name pure stores the root of the pure name
/// written with the same readers, in either order of the first write.
#[test]
fn r2_a_pin_to_pure_is_the_pure_root() {
    let mut bad = Vec::new();
    let mut n = 0;
    for r in [
        "a: [{ x: p }]",
        "a: [p]",
        "a: { x: p }",
        "a: [b]\nb: [p]",
        "a: b + \"\"\nb: c + \"\"\nc: [p]",
        "a: {{ v: p }}",
        "a: p",
        "a: [p, 1]\nm: [a]",
    ] {
        let pure = written(&format!("{r}\np: 7"));
        for (ord, src) in [("readers first", format!("{r}\n{P}")), ("name first", format!("{P}\n{r}"))] {
            let w = Ws::new("pin");
            let e = w.evolve(&src);
            fs::write(w.ws.join("pin.n"), "p: 7\n").unwrap();
            let p = w.oo(&["evolve", "--pin", "--grant", "pin", "pin.n"]).1;
            let c = w.commit();
            check(&mut bad, &format!("{r:?}, {ord}, then p pinned to 7"), format!("{e} {c} {}", w.root()), &format!("e0 c0 {}", pure.split(' ').last().unwrap_or("")));
            if p != 0 {
                bad.push(format!("  {r:?}, {ord}: the pin exited {p}"));
            }
            n += 1;
        }
    }
    verdict(bad, n);
}

/// g2 — `oo run` observes the whole write, as `oo evolve` does: readers
/// first reads what name first reads. (Guard: v0.129.0 gets these through
/// Q-125's static order; without it, `run` must still land the write in
/// the order it settles.)
#[test]
fn g2_run_reads_the_whole_write() {
    let mut bad = Vec::new();
    let mut n = 0;
    for r in ["a: [b.v]\nb: { v: c }\nc: [p]", "a: [b]\nb: [c]\nc: [p]", "a: b.v\nb: { v: p }"] {
        let w = Ws::new("run");
        let rf = w.file(&format!("{r}\n{P}"));
        let nf = w.file(&format!("{P}\n{}", reversed(r)));
        let a = squash(&w.oo(&["run", "--ephemeral", "--observe", "a", &rf]).0);
        let b = squash(&w.oo(&["run", "--ephemeral", "--observe", "a", &nf]).0);
        check(&mut bad, &format!("oo run {r:?}, readers first / name first"), a, &b);
        n += 1;
    }
    verdict(bad, n);
}

/// r4 — The order a write is written in does not change what it costs:
/// a pure chain written readers first costs what it costs name first
/// (within three times).
#[test]
fn r4_the_order_does_not_change_the_cost() {
    let n = 2000;
    let chain: Vec<String> = (0..n).map(|i| format!("k{i}: k{} + 1", i + 1)).collect();
    let rf = format!("{}\nk{n}: 0", chain.join("\n"));
    let nf = format!("k{n}: 0\n{}", chain.iter().rev().cloned().collect::<Vec<_>>().join("\n"));
    let w = Ws::new("cost");
    let (fr, ff) = (w.file(&rf), w.file(&nf));
    let (_, rr, tr) = w.oo_for(&["run", "--ephemeral", "--observe", "k0", &fr], Duration::from_secs(120));
    let (_, rn, tn) = w.oo_for(&["run", "--ephemeral", "--observe", "k0", &ff], Duration::from_secs(120));
    assert!(rr == 0 && rn == 0, "N={n}: run exited {rr} / {rn}");
    assert!(tr < 3.0 * tn.max(0.5), "N={n} pure chain: readers first {tr:.2} s, name first {tn:.2} s");
}

/// r5 — An effect is done once: a list holding a record whose field does
/// an effect, committed and read, writes one `x`.
#[test]
fn r5_an_effect_is_done_once() {
    let mut bad = Vec::new();
    let mut n = 0;
    for src in [
        format!("a: {{ c: {E} }}\nm: [a]"),
        format!("a: {{ c: {E}\n y: p }}\nm: [a]\np: 7"),
        format!("p: 7\nm: [a]\na: {{ c: {E}\n y: p }}"),
        // Guards: v0.129.0 does these once too.
        format!("k: {{{{ v: {E} }}}}\na: {{ ...k\n y: p }}\np: 7"),
        format!("a: [b]\nb: {{{{ v: c }}}}\nc: [{E}]"),
        format!("k: {{{{ v: {E} }}}}"),
    ] {
        let w = Ws::new("once");
        let f = w.file(&src);
        let e = w.oo(&["evolve", "--grant", "effect_override", &f]).1;
        let c = w.commit();
        for q in ["_.a", "_.m", "_.k"] {
            let _ = w.eval(q);
        }
        check(&mut bad, &format!("{src:?}"), format!("e{e} {c} {}", w.done()), "e0 c0 1");
        n += 1;
    }
    verdict(bad, n);
}

/// r6 — A write that meets HEAD into ⊥ is refused at evolve, in either
/// order (Q-122: evolve judges what it writes).
#[test]
fn r6_a_conflict_with_head_is_refused_at_evolve() {
    let mut bad = Vec::new();
    let mut n = 0;
    for (ord, src) in [("readers first", "h.k: g + 1\nh.z: 1\ng: 2"), ("name first", "g: 2\nh.k: g + 1\nh.z: 1")] {
        let w = Ws::new("head");
        let pre = w.evolve("h: { k: 0, r: 6 }");
        let c0 = w.commit();
        let e = w.evolve(src);
        check(&mut bad, &format!("HEAD h: {{ k: 0, r: 6 }}; {ord}"), format!("{pre} {c0} {e}"), "e0 c0 e1");
        n += 1;
    }
    verdict(bad, n);
}

/// r7 — A reader of an IO name stores what it read (D110, D46 ②: the cell
/// observes an effect), even when the answer has lost the label: `[p % 2]`
/// is `[⊥]`. v0.129.0 stored the recipe because it predicted `%` pure —
/// in one write, and with the name committed before.
#[test]
fn r7_a_reader_of_an_effect_stores_what_it_read() {
    let mut bad = Vec::new();
    let src = format!("a: [p % 2]\nm: [a]\n{P}");
    check(&mut bad, "a: [p % 2], m: [a], readers first", written(&src), "e0 c0 d99b0b4b6b96298080d169b8fed8200063e309aafec4bb40f7a028ff2fcbaeed");
    check(&mut bad, "the same, name first", written(&reversed(&src)), "e0 c0 d99b0b4b6b96298080d169b8fed8200063e309aafec4bb40f7a028ff2fcbaeed");
    // The name committed first, the reader in the next write: nothing in
    // this write observes, so what the reader read decides alone — by name,
    // and by a path's first segment.
    for (pre, r, want) in [
        (P.to_string(), "a: [p % 2]", "28589556ed6474c05f281b000d00b6f054413016592301b744be6534305d582a"),
        ("p: { v: ~%Env.get \"Q127_UNSET\" }".to_string(), "a: [p.v % 2]", "119f63ff6323b2e45447ae6eadbf980fe572eae9f9d397c0b2c7bee8097c604a"),
    ] {
        let w = Ws::new("two");
        let a = w.evolve(&pre);
        let b = w.commit();
        let c = w.evolve(r);
        let d = w.commit();
        check(&mut bad, &format!("{pre:?} committed, then {r:?}"), format!("{a} {b} {c} {d} {}", w.root()), &format!("e0 c0 e0 c0 {want}"));
    }
    verdict(bad, 4);
}

/// g1 — Every arrival of an effect Q-125 measured is observed once, at
/// evolve: HEAD holds a pure `g`; one write reaches `g` through a helper,
/// a record, a list, a meet, a map, a pipe, a ternary, a table, recursion
/// — and makes `g` an effect.
#[test]
fn g1_every_arrival_is_observed_once() {
    let g = "g: (s -> ~%Io./append_file (\"a.log\", s))";
    let mut bad = Vec::new();
    let mut n = 0;
    for (head, k) in [
        ("ap: (h -> h \"x\")", "ap g"),
        ("r: { f: g }", "r.f \"x\""),
        ("", "((x -> x) g) \"x\""),
        ("m: (u -> (v -> g))", "(m 1 2) \"x\""),
        ("", "({ f: g } & { z: 1 }).f \"x\""),
        ("", "(~%List./map ((h -> h), [g])).0 \"x\""),
        ("fs: [g]", "fs.0 \"x\""),
        ("mk: (h -> { f: h })", "(mk g).f \"x\""),
        ("ap: (h -> h h)", "(ap (y -> g)) \"x\""),
        ("", "((g, 1)).0 \"x\""),
        ("", "(sel == 1 ? g : (s -> s)) \"x\"\nsel: 1"),
        ("", "(/pick 3) \"x\"\n/pick: (n -> n == 0 ? g : /pick (n - 1))"),
        ("", "(r.a) \"x\"\nr: { a: g }"),
    ] {
        let w = Ws::new("arrive");
        let h = w.evolve(&format!("g: (s -> s)\n{head}"));
        let hc = w.commit();
        let f = w.file(&format!("k: {k}\n{g}"));
        let e = w.oo(&["evolve", &f]).1;
        let ev = w.done();
        let c = w.commit();
        let _ = w.eval("_.k");
        check(&mut bad, &format!("HEAD {head:?}; k: {k:?}"), format!("{h} {hc} e{e} {ev} {c} {}", w.done()), "e0 c0 e0 1 c0 1");
        n += 1;
    }
    verdict(bad, n);
}
