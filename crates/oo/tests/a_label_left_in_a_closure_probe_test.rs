// A label left in a closure.
// Order: nlang-tools/docs/a_label_left_in_a_closure_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-132 (no new ruling): a name
// written later gives its label to the coordinates that read it (Q-128 M2),
// and the readers of those readers, however far (Q-129). A label is what
// building the value in the working set it ends in gives.
//
// ── What is wrong (v0.131.0) ────────────────────────────────────────────
//
//   A thunk carries the frame it was written in (`capture_free_fields`), and
//   a function is stored with that frame. The label passes reach into the
//   frame only when the thunk's own label moves (`raise_thunk_effects`,
//   `refresh_thunk_effects`) or was not pure (`clear_thunk_effects`), so a
//   label in a frame is left as it was:
//
//   * `a: { e: (n -> n), x: b }`, `b: { f: ~%Env.get … }` evolved `a` then
//     `b` stores another root than one write (`x: b` in the frame of `e` is
//     pure; one write: `#io`). Since v0.127.0.
//   * `a: { c: ~%Env.get …, y: b }`, `b: [p]`, `p` pinned pure: the frame of
//     `y` holds `c: ~%Env.get …` labelled pure, and the root is not the root
//     of `p: 7` written pure.
//   * A record's own label is folded from its data and local fields only;
//     building it takes types, rules and meta fields too (`a: { @T: IO, y: p }`
//     pinned pure stores a pure `a`; written pure, `#io`).
//   * On a cycle the in-place landing and the meet leave different labels in
//     a frame, and `OO_Q117_SHADOW` panics (since v0.128.0); a new name on a
//     cycle is not yet in the reader index when its readers settle, so it
//     keeps the label it had before its readers had theirs.
//
// The other half: a write whose labels were already right keeps its root.
//
// ── R-1 (the first acceptance) ──────────────────────────────────────────
//
//   A function written as a value, not as a record field's thunk (`f: (n ->
//   b)` at the top, in a list, in a chain, on a cycle; an arrow or a pattern
//   arrow), keeps its body as code (`%code`) in a closed record. Building it
//   labels it with what predicting its body gives (`eval.rs`), but the label
//   passes stop at a closed record and the reader index does not read code:
//
//   * `f: (n -> b)`, `b: { f: ~%Env.get … }` evolved `f` then `b` stores a
//     pure `f`; one write, `#io`. Since v0.127.0 (r4).
//   * `f: (n -> p)`, `m: [f]`, `p` pinned pure: `f` keeps `#io` (r5).
//
//   g2: one write of each keeps the root v0.131.0 stores.
//
// Every `oo` here runs under a 2 GB address-space cap and a kill inside
// the probe (60 s).
//
// The delivery may NOT edit this file. `rustfmt` must not touch it.

#![allow(dead_code)]

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const KILL_AFTER: Duration = Duration::from_secs(60);
const P: &str = "p: ~%Env.get \"Q132_UNSET\"";
const V: &str = "~%Env.get \"Q132_UNSET\"";
const K: &str = "~%Env.get \"Q132_K\"";
const E: &str = "~%Io./append_file (\"a.log\", \"x\")";

struct Ws {
    _scratch: nlang_interpreter::ScratchDir,
    root: PathBuf,
    ws: PathBuf,
}

impl Ws {
    fn new(tag: &str) -> Self {
        let scratch = nlang_interpreter::ScratchDir::new(&format!("closure-label-{tag}"));
        let root = scratch.path().to_path_buf();
        let ws = root.join("ws");
        fs::create_dir_all(&ws).unwrap();
        Ws { _scratch: scratch, root, ws }
    }

    /// Run `oo` capped at 2 GB, killed after 60 s, with `env` set: (output, rc).
    fn oo_env(&self, args: &[&str], env: &[(&str, String)]) -> (String, i32) {
        let mut cmd = Command::new("sh");
        cmd.arg("-c")
            .arg("ulimit -v 2000000; exec \"$0\" \"$@\"")
            .arg(env!("CARGO_BIN_EXE_oo"))
            .args(args)
            .current_dir(&self.ws)
            .env("OO_IDENTITY", self.root.join("identity"))
            .env("OO_NODE_HOME", self.root.join("node-home"))
            .env_remove("Q132_UNSET")
            .env_remove("Q132_K")
            .env_remove("OO_Q117_SHADOW")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (k, v) in env {
            cmd.env(k, v);
        }
        let mut child = cmd.spawn().expect("oo runs");
        let start = Instant::now();
        loop {
            if child.try_wait().expect("wait").is_some() {
                break;
            }
            if start.elapsed() > KILL_AFTER {
                let _ = child.kill();
                let _ = child.wait();
                return ("DID NOT RETURN".into(), -1);
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let o = child.wait_with_output().expect("output");
        let text = format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
        let rc = o.status.code().unwrap_or(-1);
        if text.contains("panicked") {
            return ("PANIC".into(), rc);
        }
        if text.contains("stack overflow") || text.contains("memory allocation") || (rc != 0 && text.trim().is_empty()) {
            return ("CRASHED".into(), rc);
        }
        (text.trim().to_string(), rc)
    }

    fn oo(&self, args: &[&str]) -> (String, i32) {
        self.oo_env(args, &[])
    }

    fn file(&self, src: &str) -> String {
        let n = fs::read_dir(&self.ws).map(|d| d.count()).unwrap_or(0);
        let f = format!("f{n}.n");
        fs::write(self.ws.join(&f), format!("{src}\n")).unwrap();
        f
    }

    fn code(o: String, rc: i32, letter: &str) -> String {
        if o == "DID NOT RETURN" || o == "CRASHED" || o == "PANIC" { o } else { format!("{letter}{rc}") }
    }

    /// Evolve `src` from a new file: the exit code, or the crash.
    fn evolve(&self, src: &str) -> String {
        let f = self.file(src);
        let (o, rc) = self.oo(&["evolve", &f]);
        Self::code(o, rc, "e")
    }

    /// Evolve `src` with the Q-117 shadow on: the in-place landing is checked
    /// against the meet, and a difference panics.
    fn evolve_shadow(&self, src: &str) -> String {
        let f = self.file(src);
        let hits = self.root.join("shadow-hits").to_string_lossy().to_string();
        let (o, rc) = self.oo_env(&["evolve", &f], &[("OO_Q117_SHADOW", hits)]);
        Self::code(o, rc, "e")
    }

    /// Evolve `src` pinned.
    fn pin(&self, src: &str) -> String {
        let f = self.file(src);
        let (o, rc) = self.oo(&["evolve", "--pin", "--grant", "pin", &f]);
        Self::code(o, rc, "p")
    }

    fn commit(&self) -> String {
        let (o, rc) = self.oo(&["commit", "-m", "a", "--grant", "pin", "--grant", "effect_override"]);
        Self::code(o, rc, "c")
    }

    fn eval(&self, e: &str) -> String {
        squash(&self.oo(&["eval", e]).0)
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

fn spelled(src: &str) -> String {
    src.replace("%E%", E).replace("%V%", V).replace("%K%", K).replace("p: @P@", P)
}

/// Every order of the lines of `src`.
fn orders(src: &str) -> Vec<String> {
    fn go(rest: Vec<&str>, at: Vec<&str>, out: &mut Vec<String>) {
        if rest.is_empty() {
            out.push(at.join("\n"));
            return;
        }
        for i in 0..rest.len() {
            let mut r = rest.clone();
            let x = r.remove(i);
            let mut a = at.clone();
            a.push(x);
            go(r, a, out);
        }
    }
    let mut out = Vec::new();
    go(src.lines().collect(), Vec::new(), &mut out);
    out
}

/// One write of `src`, committed: the exit codes and the root.
fn written(src: &str) -> String {
    let w = Ws::new("written");
    let e = w.evolve(src);
    let c = w.commit();
    format!("{e} {c} {}", w.root())
}

/// Each line of `src` its own evolve, in that order, then one commit: the
/// exit codes and the root.
fn split(src: &str) -> String {
    let w = Ws::new("split");
    let es: Vec<String> = src.lines().map(|l| w.evolve(l)).collect();
    let c = w.commit();
    format!("{} {c} {}", es.join(" "), w.root())
}

/// `src` (which writes `p` as IO) evolved, then `p: 7` pinned, committed:
/// the exit codes and the root.
fn pinned(src: &str) -> String {
    let w = Ws::new("pinned");
    let e = w.evolve(src);
    let p = w.pin("p: 7");
    let c = w.commit();
    format!("{e} {p} {c} {}", w.root())
}

/// The cycles whose labels settle: a record or a list that comes back
/// through a record holding an IO field or reading one.
const CYCLES: [&str; 12] = [
    "a: { e: %E%, x: b }\nb: { y: a }",
    "a: { e: %V%, x: b }\nb: { y: a }",
    "a: { e: %E%, x: b }\nb: { y: c }\nc: { z: a }",
    "a: { x: b }\nb: { y: a, e: %E% }",
    "a: { e: %E%, x: b }\nb: [a]",
    "a: { e: 1, x: b }\nb: { y: a, f: %V% }",
    "a: { e: (n -> n), x: b }\nb: { y: a, f: %V% }",
    "a: { e: (n -> n), x: b }\nb: { y: a, f: %E% }",
    "a: { e: %E%, x: b }\nb: { y: _.a }",
    "a: [%E%, b]\nb: { y: c }\nc: [a]",
    "a: [%E%, b]\nb: [a]\nm: { p: a, q: b }",
    "a: { e: %V%, x: b }\nb: { y: a }\nc: [b]",
];

/// r1 — The in-place landing is the meet: every cycle in `CYCLES`, every
/// order of its lines, written as one evolve and as one evolve per line,
/// evolves under `OO_Q117_SHADOW` without a panic. (v0.131.0 panics on most:
/// a frame keeps the label it was cleared to, or a new name is not yet a
/// reader when its readers settle.)
#[test]
fn r1_the_in_place_landing_is_the_meet() {
    let mut bad = Vec::new();
    let mut n = 0;
    for shape in CYCLES {
        for src in orders(&spelled(shape)) {
            let w = Ws::new("shadow-one");
            check(&mut bad, &format!("{src:?} one evolve"), w.evolve_shadow(&src), "e0");
            let w = Ws::new("shadow-split");
            let got: Vec<String> = src.lines().map(|l| w.evolve_shadow(l)).collect();
            check(&mut bad, &format!("{src:?} one evolve per line"), got.join(" "), &vec!["e0"; src.lines().count()].join(" "));
            n += 2;
        }
    }
    verdict(bad, n);
}

/// r2 — A write split over evolves stores the root of one write: where no
/// cell is observed before a name it reads is written, writing each line as
/// its own evolve (in the order shown) and committing once stores the root
/// one evolve of the same lines stores. (v0.131.0: a function written before
/// the name its frame reads keeps that name's old label in the frame.)
#[test]
fn r2_a_write_split_over_evolves_stores_the_root_of_one_write() {
    let mut bad = Vec::new();
    let mut n = 0;
    for src in [
        "a: { e: (n -> n), x: b }\nb: { f: %V% }",
        "b: { f: %V% }\na: { e: (n -> n), x: b }",
        "a: { e: (n -> n), x: b }\nb: { f: %E% }",
        "a: { e: (n -> n), x: b }\nb: { y: a, f: %V% }",
        "b: { y: a, f: %V% }\na: { e: (n -> n), x: b }",
        "a: { e: (n -> n), x: b }\nb: { y: a, f: %E% }",
        "a: { e: (n -> n), x: b }\nb: [a, %V%]",
        "a: { e: (n -> n), x: [b] }\nb: { f: %V% }",
        "a: { e: (n -> x), x: b }\nb: { f: %V% }",
        "a: { g: { e: (n -> n), x: b } }\nb: { f: %V% }",
        "a: { e: %E%, x: b }\nb: { y: a }",
        "a: { e: 1, x: b }\nb: { y: a, f: %V% }",
    ] {
        let src = spelled(src);
        let one = written(&src);
        let codes: String = one.split(' ').take(2).collect::<Vec<_>>().join(" ");
        check(&mut bad, &format!("{src:?} one evolve"), codes, "e0 c0");
        let want = format!("{} c0 {}", vec!["e0"; src.lines().count()].join(" "), one.rsplit(' ').next().unwrap_or(""));
        check(&mut bad, &format!("{src:?} one evolve per line"), split(&src), &want);
        n += 2;
    }
    verdict(bad, n);
}

/// r3 — Pinned pure is written pure: a reader of an IO name `p`, written in
/// either order with `p`, then `p: 7` pinned, stores the root of the same
/// readers written with `p: 7` from the start. Covers a record whose frame
/// holds an IO field beside a reader of `p`, and records whose own label
/// comes from an IO type, rule or meta field. (v0.131.0: the frame keeps the
/// IO field cleared to pure; the record's own label drops the IO of its
/// type, rule or meta field.)
#[test]
fn r3_pinned_pure_is_written_pure() {
    let mut bad = Vec::new();
    let mut n = 0;
    for readers in [
        "a: { c: %K%, y: b }\nb: [p]",
        "a: { c: %K%, y: p }",
        "a: { e: (n -> n), c: %K%, y: b }\nb: [p]",
        "a: { @T: %K%\n y: p }\nm: [a]",
        "a: { /r: %K%\n y: p }\nm: [a]",
        "a: { %k: %K%\n y: p }\nm: [a]",
        "a: { @T: %K%\n y: b }\nb: [p]\nm: [a]",
    ] {
        let readers = spelled(readers);
        let pure = written(&format!("{readers}\np: 7"));
        let root = pure.rsplit(' ').next().unwrap_or("").to_string();
        check(&mut bad, &format!("{readers:?} with p: 7"), pure.split(' ').take(2).collect::<Vec<_>>().join(" "), "e0 c0");
        for src in [format!("{readers}\n{P}"), format!("{P}\n{readers}")] {
            check(&mut bad, &format!("{src:?} then p: 7 pinned"), pinned(&src), &format!("e0 p0 c0 {root}"));
            n += 1;
        }
        n += 1;
    }
    verdict(bad, n);
}

/// g1 — A write whose labels were already right keeps its root: one evolve
/// of each, the roots v0.131.0 stores.
#[test]
fn g1_labels_already_right_keep_their_root() {
    let mut bad = Vec::new();
    let mut n = 0;
    for (shape, want) in [
        ("a: { e: (n -> n), x: b }\nb: { f: %V% }", "4551aadaac0772aab9140d7210708eab984a23c45f24667480e43b761b028cd8"),
        ("a: { e: (n -> n), x: b }\nb: { y: a, f: %V% }", "75bd4d2e524c4de7f82aa35ed9ae1bebef6316ab4455e7fa555e2ec82d75321a"),
        ("a: { e: %E%, x: b }\nb: { y: a }", "aa6dde8e7adf869bf49a8f25fda8df4537a487264d8cee7777aa31a962e8227f"),
        ("a: { c: %K%, y: b }\nb: [p]\np: 7", "d525ae901225f9853ff0aeee63eefb966aca4be2a15339666c253f411c5a9ecf"),
        ("a: { @T: %K%\n y: p }\nm: [a]\np: 7", "930d6a7159f3616d9c65ae4658aac1464b1594b85122b736580cf3b44fb2aa7e"),
        ("a: { e: (n -> n), x: b }\nb: { y: a }", "eaeddaeab676fe9791060811635c97f77c289d762c61de7fdea465044b98b5d3"),
        ("a: { x: b }\nb: { y: a }", "628ca75d9dd3e671ab984df798f6eafa258cbda5f39040d17073280a674c5d20"),
        ("f: (n -> n + 1)\nq: f 2", "c4c16816fbb485cd77e6082cf10df069710b7c09331b1ee4bd0d618e620b60a9"),
        ("a: { y: 1 }\nm: [a]", "4e7bccdba70c5b073a0e158413ce6e5fb9e07f400536a2d092dc3c807bd2d8d6"),
        ("a: { @T: %K%\n y: 1 }\nm: [a]", "b09a0d19a8316794f90b4d9e2cffdfc9d59ba6bfd731f8e0bd4c10fe8d48304e"),
    ] {
        let src = spelled(shape);
        check(&mut bad, &format!("{src:?}"), written(&src), &format!("e0 c0 {want}"));
        n += 1;
    }
    verdict(bad, n);
}

// ── R-1 ── a function value is labelled from its code ────────────────────

/// Function values that are not a record field's thunk, reading a name
/// written in the same write: at the top, read through a path, a list or
/// `_.`, in a list, on a cycle, read by a list, in a chain of functions,
/// named by an alias, met with itself; an arrow and a pattern arrow.
const VALUES: [&str; 13] = [
    "f: (n -> b)\nb: { f: %V% }",
    "f: (n -> b)\nb: { f: %E% }",
    "f: (n -> b.f)\nb: { f: %V% }",
    "f: (n -> [b])\nb: { f: %V% }",
    "f: (n -> _.b)\nb: { f: %V% }",
    "l: [(n -> b)]\nb: { f: %V% }",
    "f: (n -> b)\nb: [%V%]",
    "f: (n -> b)\nb: { y: f, f: %V% }",
    "f: (n -> b)\nb: { f: %V% }\nm: [f]",
    "f: (n -> g)\ng: (n -> b)\nb: { f: %V% }",
    "f: (0 -> b)\nb: { f: %V% }",
    "g: f\nf: (n -> b)\nb: { f: %V% }",
    "f: (n -> b) & (n -> b)\nb: { f: %V% }",
];

/// r4 — A function value written before the name its body reads: every
/// shape in `VALUES`, every order of its lines, written as one evolve and as
/// one evolve per line, stores the root one evolve of the lines as listed
/// stores, and evolves under `OO_Q117_SHADOW` without a panic. (v0.131.0:
/// a function written first keeps the label it was built with, pure.)
#[test]
fn r4_a_function_value_is_labelled_from_its_code() {
    let mut bad = Vec::new();
    let mut n = 0;
    for shape in VALUES {
        let listed = spelled(shape);
        let one = written(&listed);
        check(&mut bad, &format!("{listed:?} one evolve"), one.split(' ').take(2).collect::<Vec<_>>().join(" "), "e0 c0");
        let root = one.rsplit(' ').next().unwrap_or("").to_string();
        n += 1;
        for src in orders(&listed) {
            let k = src.lines().count();
            check(&mut bad, &format!("{src:?} one evolve"), written(&src), &format!("e0 c0 {root}"));
            check(&mut bad, &format!("{src:?} one evolve per line"), split(&src), &format!("{} c0 {root}", vec!["e0"; k].join(" ")));
            let w = Ws::new("fn-shadow-one");
            check(&mut bad, &format!("{src:?} one evolve, shadow"), w.evolve_shadow(&src), "e0");
            let w = Ws::new("fn-shadow-split");
            let got: Vec<String> = src.lines().map(|l| w.evolve_shadow(l)).collect();
            check(&mut bad, &format!("{src:?} one evolve per line, shadow"), got.join(" "), &vec!["e0"; k].join(" "));
            n += 4;
        }
    }
    verdict(bad, n);
}

/// r5 — Pinned pure is written pure, for a function value: a reader of an IO
/// name `p` that is a function value (read by a list or a record, so the
/// working set is not a solid record), written in either order with `p`,
/// then `p: 7` pinned, stores the root of the same lines written with `p: 7`
/// from the start. (v0.131.0: the function keeps `#io`.) A working set left
/// a solid record keeps its own label after a pin — Inbox「pin 把一個 IO 的值
/// 換成純的之後，根照帶 `#io`」, not this card.
#[test]
fn r5_a_function_value_pinned_pure_is_written_pure() {
    let mut bad = Vec::new();
    let mut n = 0;
    for readers in [
        "f: (n -> p)\nm: [f]",
        "l: [(n -> p)]\nm: [l]",
        "f: (0 -> p)\nm: [f]",
        "f: (n -> g)\ng: (n -> p)\nm: [f]",
        "f: (n -> b)\nb: { y: p }\nm: [f]",
        "f: (n -> p)\nm: { k: f }",
    ] {
        let pure = written(&format!("{readers}\np: 7"));
        let root = pure.rsplit(' ').next().unwrap_or("").to_string();
        check(&mut bad, &format!("{readers:?} with p: 7"), pure.split(' ').take(2).collect::<Vec<_>>().join(" "), "e0 c0");
        for src in [format!("{readers}\n{P}"), format!("{P}\n{readers}")] {
            check(&mut bad, &format!("{src:?} then p: 7 pinned"), pinned(&src), &format!("e0 p0 c0 {root}"));
            n += 1;
        }
        n += 1;
    }
    verdict(bad, n);
}

/// g2 — One write of a function value keeps its root: the roots v0.131.0
/// stores (the delivery of the first round stores the same).
#[test]
fn g2_one_write_of_a_function_value_keeps_its_root() {
    let mut bad = Vec::new();
    let mut n = 0;
    for (src, want) in [
        ("f: (n -> b)\nb: { f: %V% }", "8800fc11b1777c046a400c01c07c9e9bedb529f4c58cebdf8a3adf77ac562f7e"),
        ("f: (n -> b)\nb: { f: %E% }", "4645e424ffed422db264f0b5a8a2ff5f0e6e03ff95159c4e6e74ab4d76efda5a"),
        ("f: (n -> b.f)\nb: { f: %V% }", "b41147e756a1e0d2046e3d3074170635e03d009e6fae21c5b5c6025f2dbef048"),
        ("f: (n -> [b])\nb: { f: %V% }", "7b524e0d328e277895a21b0a3a90d141a93ee4d51bceef724aa3ec48d65dc3b2"),
        ("f: (n -> _.b)\nb: { f: %V% }", "2b95d5339ad807c4855d1aad4d17abd6cd8c1c6da4a729b3df453fa9f42427ee"),
        ("l: [(n -> b)]\nb: { f: %V% }", "65fc5ba074119f66cedb351085ed0e36ed8c43f3881753b54cbe84c5ad74bc86"),
        ("f: (n -> b)\nb: [%V%]", "09f373970eb504f3f05f5cdaa5d3aa9edce649b485396f2f0159d185bdedbef1"),
        ("f: (n -> b)\nb: { y: f, f: %V% }", "6114996ae74daafa44e81f9542a2779d001f28392516d1c38f2b8cc891e8186f"),
        ("f: (n -> b)\nb: { f: %V% }\nm: [f]", "774757423d740820557f1b4720ae3641b072a456fd44ac6ce0c59f9ea88ef3ba"),
        ("f: (n -> g)\ng: (n -> b)\nb: { f: %V% }", "94fdeb8c3b59db830a23453a839c343255b6650b957f48cc2cfa5f8153c96237"),
        ("f: (0 -> b)\nb: { f: %V% }", "63072700c5563ca14700085a1ac0c144431b4e8a0cb4ac7f60d2eb14e4d527eb"),
        ("f: (n -> p)\nm: [f]\np: 7", "4c9f6f527762f8aa0da62f4d24f8d0f8924d70c0b4c1229d2e17b92c86486c91"),
        ("f: (0 -> p)\nm: [f]\np: 7", "241d15f67921d7a129377d49d5ffec9e724060d706e290b7102b932047b68ca3"),
        ("g: f\nf: (n -> b)\nb: { f: %V% }", "74fb7ef3743335b24783077359d08c90dc58d4f1fb121bb1f48be9ad7d0d6e09"),
        ("f: (n -> b) & (n -> b)\nb: { f: %V% }", "8800fc11b1777c046a400c01c07c9e9bedb529f4c58cebdf8a3adf77ac562f7e"),
    ] {
        let src = spelled(src);
        check(&mut bad, &format!("{src:?}"), written(&src), &format!("e0 c0 {want}"));
        n += 1;
    }
    verdict(bad, n);
}
