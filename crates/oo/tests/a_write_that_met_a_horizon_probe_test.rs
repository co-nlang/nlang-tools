// A write that met a horizon.
// Order: nlang-tools/docs/a_write_that_met_a_horizon_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-121.  D127 (user, 2026-11-03):
// a write that meets a coordinate the universe holds as a horizon — or a
// horizon written over a value — keeps both. The horizon stays the snapshot
// it is; reading the coordinate answers it, `%caid` unchanged. Nothing is
// lost and nothing is decided that a reader could not decide.
//
// ── What is wrong (v0.121.0) ────────────────────────────────────────────
//
//   `/f: (x -> …)`, `r: /f 5000` committed, then `r: 7`: evolve takes it,
//   commit answers `already in HEAD`, the 7 is gone. `r: 7` committed, then
//   `r: /f 5000`: the root holds only `r: #blur …`, the 7 is gone. The same
//   for a closed definition that runs out of fuel, a call, `H + 1`, a list
//   element. `h: { a: (3000 fields & 3000 fields).k2999 }` (reads as a
//   horizon), then `h: { a: 7 }`: refused as `#conflict at h.a` — the check
//   worked the definition out for free, which no reader can.
//
// The other half: a decided value is judged as before (`r: /f 50`, then
// `r: 7` is refused); a direct `/f 5000 & 7` still answers the horizon; the
// snapshot's `%caid` does not move; small roots do not move.
//
// Every `oo` here runs under a 3 GB address-space cap and a kill inside
// the probe (90 s).
//
// The delivery may NOT edit this file. `rustfmt` must not touch it.


#![allow(dead_code)]

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const KILL_AFTER: Duration = Duration::from_secs(90);

struct Ws {
    _scratch: nlang_interpreter::ScratchDir,
    root: PathBuf,
    ws: PathBuf,
}

impl Ws {
    fn new(tag: &str) -> Self {
        let scratch = nlang_interpreter::ScratchDir::new(&format!("met-a-horizon-{tag}"));
        let root = scratch.path().to_path_buf();
        let ws = root.join("ws");
        fs::create_dir_all(&ws).unwrap();
        Ws { _scratch: scratch, root, ws }
    }

    /// Run `oo` capped at 3 GB, killed after 90 s: (output, rc). A run
    /// that is killed or dies without a word says so.
    fn oo(&self, args: &[&str]) -> (String, i32) {
        let mut child = Command::new("sh")
            .arg("-c")
            .arg("ulimit -v 3000000; exec \"$0\" \"$@\"")
            .arg(env!("CARGO_BIN_EXE_oo"))
            .args(args)
            .current_dir(&self.ws)
            .env("OO_IDENTITY", self.root.join("identity"))
            .env("OO_NODE_HOME", self.root.join("node-home"))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("oo runs");
        // Drain both pipes while waiting: a root printed past the pipe's
        // buffer would otherwise block the child until the kill.
        let drain = |r: Option<Box<dyn std::io::Read + Send>>| {
            std::thread::spawn(move || {
                let mut buf = Vec::new();
                if let Some(mut r) = r {
                    let _ = r.read_to_end(&mut buf);
                }
                buf
            })
        };
        let out = drain(child.stdout.take().map(|r| Box::new(r) as Box<dyn std::io::Read + Send>));
        let err = drain(child.stderr.take().map(|r| Box::new(r) as Box<dyn std::io::Read + Send>));
        let start = Instant::now();
        let status = loop {
            if let Some(st) = child.try_wait().expect("wait") {
                break st;
            }
            if start.elapsed() > KILL_AFTER {
                let _ = child.kill();
                let _ = child.wait();
                return ("DID NOT RETURN".into(), -1);
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        let (out, err) = (out.join().unwrap_or_default(), err.join().unwrap_or_default());
        let text = format!("{}{}", String::from_utf8_lossy(&out), String::from_utf8_lossy(&err));
        let rc = status.code().unwrap_or(-1);
        if text.contains("panicked") {
            return ("PANIC".into(), rc);
        }
        if text.contains("stack overflow") || text.contains("memory allocation") || (rc != 0 && text.trim().is_empty()) {
            return ("CRASHED".into(), rc);
        }
        (text.trim().to_string(), rc)
    }

    /// Evolve `src` from a new file: (output, rc).
    fn evolve(&self, src: &str) -> (String, i32) {
        let n = fs::read_dir(&self.ws).map(|d| d.count()).unwrap_or(0);
        let f = format!("f{n}.n");
        fs::write(self.ws.join(&f), src).unwrap();
        self.oo(&["evolve", &f])
    }

    /// `oo commit`: (what it says before `Commit successful`, rc).
    fn commit(&self) -> (String, i32) {
        let (o, rc) = self.oo(&["commit", "-m", "a"]);
        let said = o.split("Commit successful").next().unwrap_or("").trim().to_string();
        (squash(&said), rc)
    }

    /// Evolve then commit `src`; "e0 c0" when both succeed.
    fn write(&self, src: &str) -> String {
        let (eo, er) = self.evolve(src);
        if eo == "DID NOT RETURN" || eo == "CRASHED" || eo == "PANIC" {
            return eo;
        }
        let (co, cr) = self.commit();
        if co == "DID NOT RETURN" || co == "CRASHED" || co == "PANIC" {
            return co;
        }
        format!("e{er} c{cr}")
    }

    /// `oo eval`, the trailing effect note dropped, whitespace collapsed.
    fn eval(&self, e: &str) -> String {
        let s = self.oo(&["eval", e]).0;
        let s = match s.find(";; %effect") {
            Some(i) => s[..i].trim().to_string(),
            None => s,
        };
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

    /// The committed root as `oo inspect` prints it.
    fn stored(&self) -> String {
        let head = fs::read_to_string(self.ws.join(".oo").join("HEAD")).unwrap_or_default();
        if head.trim().is_empty() {
            return String::new();
        }
        let (o, _) = self.oo(&["inspect", head.trim()]);
        let caid = o.lines().find(|l| l.starts_with("root:")).and_then(|l| l.split_whitespace().last()).unwrap_or("").to_string();
        self.oo(&["inspect", &caid]).0
    }

    /// The committed root object's bytes, as the store holds them.
    fn root_object(&self) -> String {
        let r = self.root();
        if r.len() < 3 {
            return String::new();
        }
        let p = self.ws.join(".oo").join("objects").join("sha256").join(&r[..2]).join(&r[2..]);
        fs::read_to_string(p).unwrap_or_default()
    }

    /// How many fields the committed root stores as a fuel horizon.
    fn stored_out_of_fuel(&self) -> usize {
        self.stored().matches("#fuel_exhausted").count()
    }

    /// The first line the committed root prints for `name`, cut to 12 bytes.
    fn stored_form(&self, name: &str) -> String {
        let s = self.stored();
        let key = format!("  {name}:");
        s.lines().find(|l| l.starts_with(&key)).map(|l| l.trim().chars().take(12).collect()).unwrap_or_default()
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

/// A ⊥ or a horizon reduced to its cause.
fn short(got: &str) -> String {
    for cause in ["#conflict", "#fuel_exhausted", "#max_depth_exceeded"] {
        if (got.starts_with("_|_") || got.starts_with("#blur")) && got.contains(cause) {
            return format!("{} {cause}", got.split_whitespace().next().unwrap_or(""));
        }
    }
    got.into()
}

fn fields(n: usize, f: impl Fn(usize) -> String) -> String {
    (0..n).map(f).collect::<Vec<_>>().join(", ")
}

/// `{ k0: 0, …, k{n-1}: n-1 }`
fn rec(n: usize) -> String {
    format!("{{ {} }}", fields(n, |i| format!("k{i}: {i}")))
}

const FUEL: &str = "#blur #fuel_exhausted";

/// Run each cell in its own workspace, four at a time; a cell pushes what
/// is wrong.
fn cells(bad: &mut Vec<String>, run: Vec<Box<dyn Fn() -> Vec<String> + Send + Sync>>) -> usize {
    let n = run.len();
    let per = n.div_ceil(4).max(1);
    let mut out: Vec<Vec<String>> = vec![Vec::new(); n];
    std::thread::scope(|s| {
        for (o, r) in out.chunks_mut(per).zip(run.chunks(per)) {
            s.spawn(move || {
                for (o, f) in o.iter_mut().zip(r) {
                    *o = f();
                }
            });
        }
    });
    for o in out {
        bad.extend(o);
    }
    n
}


const F: &str = "/f: (x -> x == 0 ? 0 : /f (x - 1))";
const W: &str = "77777";
const DEPTH: &str = "#blur #max_depth_exceeded";

/// A closed definition that reads as a fuel horizon at the default fuel.
fn fu() -> String {
    format!("({} & {{ {} }}).k2999", rec(3000), fields(3000, |i| format!("j{i}: {i}")))
}

/// (name, the horizon's source, what goes before it, its reading)
fn horizons() -> Vec<(&'static str, String, String, &'static str)> {
    vec![
        ("depth", "/f 5000".to_string(), format!("{F}\n"), DEPTH),
        ("fuel", fu(), String::new(), FUEL),
    ]
}

/// Each step is evolved then committed, or (`evolve_only`) evolved only and
/// committed once at the end. "e0 … c0" when all succeed.
fn steps(w: &Ws, srcs: &[String], evolve_only: bool) -> String {
    let mut out = Vec::new();
    for s in srcs {
        out.push(format!("e{}", w.evolve(s).1));
        if !evolve_only {
            let (said, rc) = w.commit();
            out.push(format!("c{rc}{}", if said.contains("already in HEAD") { ":HEAD" } else { "" }));
        }
    }
    if evolve_only {
        let (said, rc) = w.commit();
        out.push(format!("c{rc}{}", if said.contains("already in HEAD") { ":HEAD" } else { "" }));
    }
    out.join(" ")
}

fn kept_cell(label: String, srcs: Vec<String>, evolve_only: bool, marker_write: &'static str, horizon_mark: String, q: &'static str, want: &'static str) -> Box<dyn Fn() -> Vec<String> + Send + Sync> {
    Box::new(move || {
        let mut bad = Vec::new();
        let w = Ws::new("kept");
        let rcs = steps(&w, &srcs, evolve_only);
        let all_ok = rcs.split(' ').all(|x| x == "e0" || x == "c0");
        check(&mut bad, &format!("{label}: every step"), all_ok.to_string(), "true");
        let obj = w.root_object();
        check(&mut bad, &format!("{label}: the root holds the write"), obj.contains(marker_write).to_string(), "true");
        check(&mut bad, &format!("{label}: the root holds the horizon"), obj.contains(horizon_mark.as_str()).to_string(), "true");
        check(&mut bad, &format!("{label}: {q}"), short(&w.eval(q)), want);
        if !all_ok {
            bad.push(format!("    ({label}: steps were {rcs})"));
        }
        bad
    })
}

/// What in the root object shows that the horizon is kept: its snapshot, or
/// its definition.
fn mark(name: &str) -> String {
    if name == "depth" { "max_depth_exceeded|/f 5000".into() } else { "fuel_exhausted|k2999".into() }
}

fn holds_either(obj: &str, m: &str) -> bool {
    m.split('|').any(|x| obj.contains(x))
}

// ── r1 ── a write over a horizon is kept ────────────────────────────────

#[test]
fn r1_a_write_over_a_horizon_is_kept() {
    let mut bad = Vec::new();
    let mut run: Vec<Box<dyn Fn() -> Vec<String> + Send + Sync>> = Vec::new();
    for (name, h, pre, want) in horizons() {
        let shapes: Vec<(&str, Vec<String>, bool, &'static str)> = vec![
            ("two commits", vec![format!("{pre}r: {h}"), format!("r: {W}")], false, "_.r"),
            ("two evolves", vec![format!("{pre}r: {h}"), format!("r: {W}")], true, "_.r"),
            ("one file", vec![format!("{pre}r: {h}\nr: {W}")], false, "_.r"),
            ("in a record", vec![format!("{pre}h: {{ a: {h} }}"), format!("h: {{ a: {W} }}")], false, "_.h.a"),
            ("a union holding it", vec![format!("{pre}r: {h} | 1"), format!("r: {W}")], false, "_.r"),
            ("a list element", vec![format!("{pre}l: [{h}]"), format!("l: [{W}]")], false, "_.l.0"),
            ("a call's answer", vec![format!("{pre}r: (x -> x) ({h})"), format!("r: {W}")], false, "_.r"),
            ("arithmetic on it", vec![format!("{pre}r: {h} + 1"), format!("r: {W}")], false, "_.r"),
        ];
        for (label, srcs, ev, q) in shapes {
            let m = mark(name);
            let mm = m.clone();
            let label = format!("{name}, {label}");
            let srcs2 = srcs.clone();
            run.push(Box::new(move || {
                let mut bad = Vec::new();
                let w = Ws::new("r1");
                let rcs = steps(&w, &srcs2, ev);
                let all_ok = rcs.split(' ').all(|x| x == "e0" || x == "c0");
                check(&mut bad, &format!("{label}: every step (got {rcs})"), all_ok.to_string(), "true");
                let obj = w.root_object();
                check(&mut bad, &format!("{label}: the root holds the write"), obj.contains(W).to_string(), "true");
                check(&mut bad, &format!("{label}: the root holds the horizon"), holds_either(&obj, &mm).to_string(), "true");
                check(&mut bad, &format!("{label}: {q}"), short(&w.eval(q)), want);
                bad
            }));
            let _ = m;
        }
    }
    let n = cells(&mut bad, run);
    verdict(bad, n);
}

// ── r2 ── a horizon over a value keeps the value ────────────────────────

#[test]
fn r2_a_horizon_over_a_value_keeps_the_value() {
    let mut bad = Vec::new();
    let mut run: Vec<Box<dyn Fn() -> Vec<String> + Send + Sync>> = Vec::new();
    for (name, h, pre, want) in horizons() {
        let shapes: Vec<(&str, Vec<String>, bool, &'static str)> = vec![
            ("two commits", vec![format!("{pre}r: {W}"), format!("r: {h}")], false, "_.r"),
            ("two evolves", vec![format!("{pre}r: {W}"), format!("r: {h}")], true, "_.r"),
            ("one file", vec![format!("{pre}r: {W}\nr: {h}")], false, "_.r"),
            ("in a record", vec![format!("{pre}h: {{ a: {W} }}"), format!("h: {{ a: {h} }}")], false, "_.h.a"),
        ];
        for (label, srcs, ev, q) in shapes {
            let m = mark(name);
            let label = format!("{name}, {label}");
            run.push(Box::new(move || {
                let mut bad = Vec::new();
                let w = Ws::new("r2");
                let rcs = steps(&w, &srcs, ev);
                let all_ok = rcs.split(' ').all(|x| x == "e0" || x == "c0");
                check(&mut bad, &format!("{label}: every step (got {rcs})"), all_ok.to_string(), "true");
                let obj = w.root_object();
                check(&mut bad, &format!("{label}: the root holds the value"), obj.contains(W).to_string(), "true");
                check(&mut bad, &format!("{label}: the root holds the horizon"), holds_either(&obj, &m).to_string(), "true");
                check(&mut bad, &format!("{label}: {q}"), short(&w.eval(q)), want);
                bad
            }));
        }
    }
    let n = cells(&mut bad, run);
    verdict(bad, n);
}

// ── r3 ── whether a write clashes is judged on what a reader reads ──────

#[test]
fn r3_a_clash_is_judged_on_what_a_reader_reads() {
    let mut bad = Vec::new();
    // `h.a` reads as a fuel horizon; the check must not work it out for free.
    let cell = |n: usize, first: bool| -> Box<dyn Fn() -> Vec<String> + Send + Sync> {
        Box::new(move || {
            let mut bad = Vec::new();
            let w = Ws::new("r3");
            let def = format!("({} & {{ {} }}).k{}", rec(n), fields(n, |i| format!("j{i}: {i}")), n - 1);
            let (a, b) = if first { (format!("h: {{ a: {def} }}"), format!("h: {{ a: {W} }}")) } else { (format!("h: {{ a: {W} }}"), format!("h: {{ a: {def} }}")) };
            check(&mut bad, &format!("N={n} first={first}: first write"), w.write(&a), "e0 c0");
            // What a reader reads of the definition alone, from the root.
            let alone = Ws::new("r3-def");
            let _ = alone.write(&format!("h: {{ a: {def} }}"));
            let def_reads = short(&alone.eval("_.h.a"));
            let (o, rc) = w.evolve(&b);
            // A reader who can decide sees the clash; when the definition reads
            // as a horizon nobody can, and both are kept (D127).
            let want = if def_reads.starts_with("#blur") { "rc=0" } else { "rc=1" };
            check(&mut bad, &format!("N={n} first={first}: the definition reads {def_reads}; the second write"), format!("rc={rc}"), want);
            if rc == 1 && want == "rc=0" {
                bad.push(format!("    (said {})", o.chars().take(80).collect::<String>()));
            }
            bad
        })
    };
    let n = cells(&mut bad, vec![cell(10, true), cell(10, false), cell(3000, true), cell(3000, false)]);
    verdict(bad, n);
}

// ── g1 ── a decided value is judged as before ───────────────────────────

#[test]
fn g1_a_decided_value_is_judged_as_before() {
    let mut bad = Vec::new();
    let cell = |label: &'static str, a: String, b: &'static str, rc_want: &'static str, q: &'static str, want: &'static str| -> Box<dyn Fn() -> Vec<String> + Send + Sync> {
        Box::new(move || {
            let mut bad = Vec::new();
            let w = Ws::new("g1");
            check(&mut bad, &format!("{label}: first"), w.write(&a), "e0 c0");
            check(&mut bad, &format!("{label}: second"), format!("rc={}", w.evolve(b).1), rc_want);
            let _ = w.commit();
            check(&mut bad, &format!("{label}: {q}"), short(&w.eval(q)), want);
            bad
        })
    };
    let mut run = vec![
        cell("/f 50 then 7", format!("{F}\nr: /f 50"), "r: 7", "rc=1", "_.r", "0"),
        cell("/f 50 then 0", format!("{F}\nr: /f 50"), "r: 0", "rc=0", "_.r", "0"),
        cell("7 then /f 50", format!("{F}\nr: 7"), "r: /f 50", "rc=1", "_.r", "7"),
        cell("H then _", format!("{F}\nr: /f 5000"), "r: _", "rc=0", "_.r", DEPTH),
    ];
    // The same horizon written again is the one HEAD holds.
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        let w = Ws::new("g1-again");
        let src = format!("{F}\nr: /f 5000");
        check(&mut bad, "same horizon twice: first", w.write(&src), "e0 c0");
        check(&mut bad, "same horizon twice: evolve", format!("rc={}", w.evolve(&src).1), "rc=0");
        let (o, rc) = w.oo(&["commit", "-m", "b"]);
        check(&mut bad, "same horizon twice: commit", format!("rc={rc} {}", o.contains("already in HEAD")), "rc=1 true");
        bad
    }));
    run.push(Box::new(|| {
        let mut bad = Vec::new();
        let e = Ws::new("g1-eval");
        let w = |x: &str| format!("({{ {F}, r: {x} }}).r");
        check(&mut bad, "direct /f 5000 & 7", short(&e.eval(&w("/f 5000 & 7"))), DEPTH);
        check(&mut bad, "direct 7 & /f 5000", short(&e.eval(&w("7 & /f 5000"))), DEPTH);
        check(&mut bad, "direct /f 50 & 7", short(&e.eval(&w("/f 50 & 7"))), "_|_ #conflict");
        let u = e.eval(&w("/f 5000 | 7"));
        check(&mut bad, "direct /f 5000 | 7 keeps both", (u.contains("7") && u.contains("#max_depth_exceeded")).to_string(), "true");
        bad
    }));
    let n = cells(&mut bad, run);
    verdict(bad, n);
}

// ── g2 ── the snapshot does not move ────────────────────────────────────

#[test]
fn g2_the_snapshot_does_not_move() {
    let mut bad = Vec::new();
    for (name, h, pre, _) in horizons() {
        let w = Ws::new("g2");
        check(&mut bad, &format!("{name}: first"), w.write(&format!("{pre}r: {h}")), "e0 c0");
        let before = w.eval("_.r.%caid");
        let _ = w.write("r: 7");
        check(&mut bad, &format!("{name}: %caid after the write"), w.eval("_.r.%caid"), &before);
    }
    let w = Ws::new("g2-pin");
    check(&mut bad, "depth: write", w.write(&format!("{F}\nr: /f 5000")), "e0 c0");
    check(&mut bad, "depth: %caid", w.eval("_.r.%caid"), "\"hash:sha256:v1:e84c5a978ff81efffa3df3d9f8a5406af9c3de2df5274eb469fbbc1765973ecd\"");
    let chain = (0..4000).map(|_| "1").collect::<Vec<_>>().join(" + ");
    let s = Ws::new("g2-spread");
    check(&mut bad, "spread: write", s.write(&format!("big: {chain}\np: {{ b: 1, ...big }}\nm: big & {{ }}")), "e0 c0");
    check(&mut bad, "spread: p.%caid == big.%caid", s.eval("_.p.%caid == _.big.%caid"), "#true");
    check(&mut bad, "meet: m.%caid == big.%caid", s.eval("_.m.%caid == _.big.%caid"), "#true");
    verdict(bad, 8);
}

// ── g3 ── the roots of small writes do not move ─────────────────────────

#[test]
fn g3_the_roots_of_small_writes_do_not_move() {
    let mut bad = Vec::new();
    for (src, want) in [
        ("x: 0", "b74d775816f6b5be6cc152577b7556f6c8fdcc63433b0e31068cc2484a94ae0f"),
        ("h: { a: 1, b: a + 1 }", "2284caf43b29be83c7b90aee891418b43fcd91b8495819d95f70ff43f54a0e5e"),
        ("h: { a: 1, b: 2 }", "3525d353584d8f2523cedad36edec0385aa208f09003f8c6db60ea5d327e0006"),
        ("t: { @{ @int }: 1, _: 0 }", "b6130220f9d82a6d1a5686b41c622853a844aa649ac056e81c4871b525c3e582"),
        ("f: (x -> x + 1)", "258a1a08adfd0340cdbe103e4f4851bea107a67e298e72d6679badc0595bfd1f"),
    ] {
        let w = Ws::new("g3");
        check(&mut bad, &format!("{src}: write"), w.write(&format!("{src}\n")), "e0 c0");
        check(&mut bad, &format!("{src}: root"), w.root(), want);
    }
    verdict(bad, 10);
}
