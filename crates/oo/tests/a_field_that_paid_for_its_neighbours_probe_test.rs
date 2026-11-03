// A field that paid for its neighbours.
// Order: nlang-tools/docs/a_field_that_paid_for_its_neighbours_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-120.  When the universe forces a
// value for its own bookkeeping — projecting a commit, deciding whether a
// write clashes, whether a member is already at HEAD, how a definition is
// stored — it charged every cell of that value to one meter. What one field
// came to depended on how many other fields the same write held. SPEC_10
// §2.3.1 (D113): a coordinate is a horizon only because its own definition
// hit one; what it stores, reads, takes or refuses does not change with how
// many other coordinates there are. The universe's own meets spend no fuel.
//
// ── What is wrong (v0.120.0) ────────────────────────────────────────────
//
//   `h: { k0: 0, …, k3999: 3999 }` committed: 667 fields are stored as
//   `#blur #fuel_exhausted`, `_.h.k3999` ⟹ `#blur` (the record evaluated
//   directly answers `3999`); 8000 fields, 4667. Two 2000-field records in
//   one commit store 667, in two commits none. A `z: 1 & 2` after 3999
//   fields is not reported by the commit (after 499 it is). A write
//   `h: { k3999: 7 }` over the committed record is taken (over 10 or 2100
//   fields it is refused by name). The same file of 2100 lines committed
//   twice says `Nothing to commit` (1900 lines: `already in HEAD …`). A
//   record of 4000 fields a call built is stored as its answer with a
//   `%closure` snapshot (1000 fields: as its recipe, `(x -> …) 1`).
//   Beside `r: (500 fields & 500 fields).k499 + n`, the write `h: { z: 1 }`
//   is refused as `#conflict at h` (100 fields: taken), while `_.h.r` reads
//   `504`: deciding whether `r` prints back worked it out twice on one meter.
//
// The other half: an observation is still charged on one meter. A meet the
// user writes, a reading of a stored recipe and the same recipe read again
// in place, a definition that runs out on its own — all answer what they
// answered. The roots of small writes do not move.
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
        let scratch = nlang_interpreter::ScratchDir::new(&format!("neighbours-{tag}"));
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

// ── r1 ── every field stores what it is, however many it has beside it ────

/// One write (or several, each evolved and committed in turn); then each
/// `(read, want)`; and the committed root stores no fuel horizon.
fn stores(label: &'static str, srcs: Vec<String>, reads: Vec<(String, String)>) -> Box<dyn Fn() -> Vec<String> + Send + Sync> {
    Box::new(move || {
        let mut bad = Vec::new();
        let w = Ws::new("r1");
        for (i, s) in srcs.iter().enumerate() {
            check(&mut bad, &format!("{label}: write {i}"), w.write(s), "e0 c0");
        }
        check(&mut bad, &format!("{label}: fields stored as a fuel horizon"), w.stored_out_of_fuel().to_string(), "0");
        for (q, want) in &reads {
            check(&mut bad, &format!("{label}: {q}"), short(&w.eval(q)), want);
        }
        bad
    })
}

#[test]
fn r1_every_field_stores_what_it_is_however_many_it_has_beside_it() {
    let mut bad = Vec::new();
    let r = |q: &str, w: &str| (q.to_string(), w.to_string());
    let n = cells(
        &mut bad,
        vec![
            stores("4000 atoms", vec![format!("h: {}", rec(4000))], vec![r("_.h.k3999", "3999"), r("_.h.k2000", "2000"), r("_.h.k0", "0")]),
            stores("8000 atoms", vec![format!("h: {}", rec(8000))], vec![r("_.h.k7999", "7999"), r("_.h.k4000", "4000")]),
            stores(
                "4000 strings",
                vec![format!("h: {{ {} }}", fields(4000, |i| format!("k{i}: \"s{i}\"")))],
                vec![r("_.h.k3999", "\"s3999\"")],
            ),
            stores(
                "hidden facet and 4000 fields",
                vec![format!("h: {{ ~@T: @int, {} }}", fields(4000, |i| format!("k{i}: {i}")))],
                vec![r("_.h.k3999", "3999")],
            ),
            stores("a list holding 4000 fields", vec![format!("l: [{}]", rec(4000))], vec![r("_.l.0.k3999", "3999")]),
            stores("4000 fields a call built", vec![format!("h: (x -> {}) 1", rec(4000))], vec![r("_.h.k3999", "3999")]),
            stores(
                "8 nested records of 1000",
                vec![format!("h: {{ {} }}", fields(8, |j| format!("r{j}: {}", rec(1000))))],
                vec![r("_.h.r7.k999", "999"), r("_.h.r0.k999", "999")],
            ),
            stores(
                "two of 2000 in one commit",
                vec![format!("a: {}\nb: {{ {} }}", rec(2000), fields(2000, |i| format!("j{i}: {i}")))],
                vec![r("_.a.k1999", "1999"), r("_.b.j1999", "1999"), r("_.b.j0", "0")],
            ),
            stores(
                "a field added to a committed 4000",
                vec![format!("h: {}", rec(4000)), "h: { z: 1 }".into()],
                vec![r("_.h.k3999", "3999"), r("_.h.z", "1")],
            ),
            // the control: the same record, small.
            stores("10 atoms", vec![format!("h: {}", rec(10))], vec![r("_.h.k9", "9")]),
        ],
    );
    verdict(bad, n);
}

// ── r2 ── a ⊥ is reported however many fields precede it ──────────────────

#[test]
fn r2_a_conflict_is_reported_however_many_fields_precede_it() {
    let mut bad = Vec::new();
    let cell = |n: usize| -> Box<dyn Fn() -> Vec<String> + Send + Sync> {
        Box::new(move || {
            let mut bad = Vec::new();
            let w = Ws::new("r2");
            let src = format!("h: {{ {}, z: 1 & 2 }}", fields(n - 1, |i| format!("k{i}: {i}")));
            check(&mut bad, &format!("N={n}: evolve"), w.evolve(&src).1.to_string(), "0");
            let (said, rc) = w.commit();
            check(&mut bad, &format!("N={n}: commit"), format!("rc={rc} {said}"), "rc=0 #conflict at h.z");
            check(&mut bad, &format!("N={n}: _.h.z"), short(&w.eval("_.h.z")), "_|_ #conflict");
            bad
        })
    };
    let n = cells(&mut bad, vec![cell(500), cell(4000), cell(16000)]);
    verdict(bad, n);
}

// ── r3 ── a write that clashes is refused however large the record ───────

#[test]
fn r3_a_write_that_clashes_is_refused_however_large_the_record() {
    let mut bad = Vec::new();
    // (label, first write, the clashing write, coordinate, read, want)
    let cell = |label: &'static str, first: String, clash: String, at: String, q: String, want: String| -> Box<dyn Fn() -> Vec<String> + Send + Sync> {
        Box::new(move || {
            let mut bad = Vec::new();
            let w = Ws::new("r3");
            check(&mut bad, &format!("{label}: first write"), w.write(&first), "e0 c0");
            let (o, rc) = w.evolve(&clash);
            let named = o.contains(&format!("#conflict at {at}"));
            check(&mut bad, &format!("{label}: the clashing write"), format!("rc={rc} named={named}"), "rc=1 named=true");
            check(&mut bad, &format!("{label}: {q} after"), short(&w.eval(&q)), &want);
            bad
        })
    };
    let on = |n: usize| {
        cell("committed record", format!("h: {}", rec(n)), format!("h: {{ k{}: 7 }}", n - 1), format!("h.k{}", n - 1), format!("_.h.k{}", n - 1), (n - 1).to_string())
    };
    let alias = |n: usize| {
        cell("a recipe that reads it", format!("c: {}\nr: c", rec(n)), format!("r: {{ k{}: 7 }}", n - 1), format!("r.k{}", n - 1), format!("_.c.k{}", n - 1), (n - 1).to_string())
    };
    let n = cells(&mut bad, vec![on(10), on(2100), on(4000), on(16000), alias(10), alias(16000)]);
    verdict(bad, n);
}

// ── r4 ── a member HEAD already holds is named however many fields ───────

#[test]
fn r4_a_member_head_already_holds_is_named_however_many_fields() {
    let mut bad = Vec::new();
    let cell = |n: usize| -> Box<dyn Fn() -> Vec<String> + Send + Sync> {
        Box::new(move || {
            let mut bad = Vec::new();
            let w = Ws::new("r4");
            let src = (0..n).map(|i| format!("k{i}: {i}")).collect::<Vec<_>>().join("\n");
            check(&mut bad, &format!("N={n}: first"), w.write(&src), "e0 c0");
            check(&mut bad, &format!("N={n}: again, evolve"), w.evolve(&src).1.to_string(), "0");
            let (o, rc) = w.oo(&["commit", "-m", "b"]);
            let head = fs::read_to_string(w.ws.join(".oo").join("HEAD")).unwrap_or_default();
            let named = o.contains("already in HEAD") && o.contains(head.trim());
            check(&mut bad, &format!("N={n}: again, commit"), format!("rc={rc} names HEAD={named}"), "rc=1 names HEAD=true");
            bad
        })
    };
    let n = cells(&mut bad, vec![cell(10), cell(1900), cell(2500), cell(16000)]);
    verdict(bad, n);
}

// ── r5 ── how a definition is stored does not depend on its size ─────────

#[test]
fn r5_how_a_definition_is_stored_does_not_depend_on_its_size() {
    let mut bad = Vec::new();
    // R-4: a record a call built, holding a function that captured the call's
    // names, keeps its recipe. D110: one that reads the universe keeps it.
    let cell = |label: &'static str, n: usize, body: fn(usize) -> String, after: Option<&'static str>, q: String, want: &'static str| -> Box<dyn Fn() -> Vec<String> + Send + Sync> {
        Box::new(move || {
            let mut bad = Vec::new();
            let w = Ws::new("r5");
            check(&mut bad, &format!("{label} N={n}: write"), w.write(&format!("h: (x -> {}) 1", body(n))), "e0 c0");
            check(&mut bad, &format!("{label} N={n}: stored as"), w.stored_form("h"), "h: (x -> {");
            if let Some(a) = after {
                check(&mut bad, &format!("{label} N={n}: then {a}"), w.write(a), "e0 c0");
            }
            check(&mut bad, &format!("{label} N={n}: {q}"), short(&w.eval(&q)), want);
            bad
        })
    };
    // Whether a coordinate written twice keeps both writes' syntax (Q-092)
    // is decided by whether a side reads the universe: decided the same at
    // any size. (Reading it is not pinned: a meet of 4001 fields read in
    // place runs out, §6.)
    let then_w = |n: usize| -> Box<dyn Fn() -> Vec<String> + Send + Sync> {
        Box::new(move || {
            let mut bad = Vec::new();
            let w = Ws::new("r5w");
            check(&mut bad, &format!("reads n, then w N={n}: write"), w.write(&format!("h: (x -> {}) 1", reads_n(n))), "e0 c0");
            check(&mut bad, &format!("reads n, then w N={n}: h: {{ w: 1 }}"), w.write("h: { w: 1 }"), "e0 c0");
            check(&mut bad, &format!("reads n, then w N={n}: stored as"), w.stored_form("h"), "h: (x -> {");
            bad
        })
    };
    fn closure(n: usize) -> String {
        format!("{{ {}, f: (y -> x + y) }}", fields(n, |i| format!("k{i}: {i}")))
    }
    fn reads_n(n: usize) -> String {
        format!("{{ {}, z: n }}", fields(n, |i| format!("k{i}: {i}")))
    }
    let n = cells(
        &mut bad,
        vec![
            cell("closure last", 1000, closure, None, "_.h.f 2".into(), "3"),
            cell("closure last", 4000, closure, None, "_.h.f 2".into(), "3"),
            cell("reads n last", 1000, reads_n, Some("n: 5"), "_.h.z".into(), "5"),
            cell("reads n last", 4000, reads_n, Some("n: 5"), "_.h.z".into(), "5"),
            cell("reads n last", 16000, reads_n, Some("n: 5"), "_.h.z".into(), "5"),
            then_w(10),
            then_w(4000),
        ],
    );
    verdict(bad, n);
}

// ── r6 ── a write beside a costly field is taken ─────────────────────────

#[test]
fn r6_a_write_beside_a_costly_field_is_taken() {
    let mut bad = Vec::new();
    // `r` reads the universe; working it out takes most of a budget. The
    // write touches another field. Whether it is taken is not `r`'s to pay.
    let costly = |n: usize| -> Box<dyn Fn() -> Vec<String> + Send + Sync> {
        Box::new(move || {
            let mut bad = Vec::new();
            let w = Ws::new("r6");
            let a = rec(n);
            let b = format!("{{ {} }}", fields(n, |i| format!("j{i}: {i}")));
            check(&mut bad, &format!("costly N={n}: first"), w.write(&format!("n: 5\nh: {{ r: ({a} & {b}).k{} + n }}", n - 1)), "e0 c0");
            let (o, rc) = w.evolve("h: { z: 1 }");
            check(&mut bad, &format!("costly N={n}: h: {{ z: 1 }}"), format!("rc={rc} {}", if rc == 0 { "" } else { o.as_str() }), "rc=0 ");
            check(&mut bad, &format!("costly N={n}: commit"), w.commit().1.to_string(), "0");
            check(&mut bad, &format!("costly N={n}: _.h.z"), short(&w.eval("_.h.z")), "1");
            check(&mut bad, &format!("costly N={n}: _.h.r"), short(&w.eval("_.h.r")), &(n + 4).to_string());
            bad
        })
    };
    // A record whose last field reads the universe; the write adds a field.
    let wide = |n: usize| -> Box<dyn Fn() -> Vec<String> + Send + Sync> {
        Box::new(move || {
            let mut bad = Vec::new();
            let w = Ws::new("r6");
            check(&mut bad, &format!("wide N={n}: first"), w.write(&format!("h: {{ {}, z: n }}", fields(n, |i| format!("k{i}: {i}")))), "e0 c0");
            let (o, rc) = w.evolve("h: { w: 1 }");
            check(&mut bad, &format!("wide N={n}: h: {{ w: 1 }}"), format!("rc={rc} {}", if rc == 0 { "" } else { o.as_str() }), "rc=0 ");
            check(&mut bad, &format!("wide N={n}: commit"), w.commit().1.to_string(), "0");
            bad
        })
    };
    let n = cells(&mut bad, vec![costly(100), costly(500), costly(1000), wide(10), wide(4000)]);
    verdict(bad, n);
}

// ── g1 ── an observation is still charged on one meter ───────────────────

#[test]
fn g1_an_observation_is_still_charged_on_one_meter() {
    let mut bad = Vec::new();
    let j = |n: usize| format!("{{ {} }}", fields(n, |i| format!("j{i}: {i}")));
    let e = Ws::new("g1-eval");
    // A meet the user writes is the user's observation (D113: "交會途中被強制的定義照常計費").
    check(&mut bad, "(1000 & 1000).k999", short(&e.eval(&format!("({} & {}).k999", rec(1000), j(1000)))), FUEL);
    check(&mut bad, "(100 & 100).k99", short(&e.eval(&format!("({} & {}).k99", rec(100), j(100)))), "99");
    check(&mut bad, "((4000 | 5) & { k0: _ }).k3999", short(&e.eval(&format!("(({} | 5) & {{ k0: _ }}).k3999", rec(4000)))), FUEL);
    check(&mut bad, "4000 directly .k3999", short(&e.eval(&format!("{}.k3999", rec(4000)))), "3999");
    check(&mut bad, "depth: /f 5000", short(&e.eval("({ /f: (x -> x == 0 ? 0 : /f (x - 1)), r: /f 5000 }).r")), "#blur #max_depth_exceeded");
    // A stored recipe reads what the same recipe read in place reads (D110).
    let w = Ws::new("g1-recipe");
    check(&mut bad, "alias: write", w.write(&format!("c: {}\nr: c", rec(4000))), "e0 c0");
    check(&mut bad, "alias: agreeing write", w.write("r: { k3999: 3999 }"), "e0 c0");
    let stored = short(&w.eval("_.r.k3999"));
    let in_place = short(&w.eval("(_.c & { k3999: 3999 }).k3999"));
    check(&mut bad, "alias: read equals read in place", format!("{stored} / {in_place}"), &format!("{FUEL} / {FUEL}"));
    // A definition worked out inside the universe's walk is one observation:
    // `z`'s own comparison runs out, so the commit has no ⊥ to report and
    // reading `z` answers the horizon.
    let big = rec(4000);
    let d = Ws::new("g1-definition");
    check(&mut bad, "definition: evolve", d.evolve(&format!("h: {{ a: 1, z: ({big} = {big}) & #false }}")).1.to_string(), "0");
    let (said, rc) = d.commit();
    check(&mut bad, "definition: commit", format!("rc={rc} said={said}"), "rc=0 said=");
    check(&mut bad, "definition: _.h.z", short(&d.eval("_.h.z")), FUEL);
    verdict(bad, 11);
}

// ── g2 ── the roots of small writes do not move ─────────────────────────

#[test]
fn g2_the_roots_of_small_writes_do_not_move() {
    let mut bad = Vec::new();
    for (src, want) in [
        ("x: 0", "b74d775816f6b5be6cc152577b7556f6c8fdcc63433b0e31068cc2484a94ae0f"),
        ("h: { a: 1, b: a + 1 }", "2284caf43b29be83c7b90aee891418b43fcd91b8495819d95f70ff43f54a0e5e"),
        ("h: { a: 1, b: 2 }", "3525d353584d8f2523cedad36edec0385aa208f09003f8c6db60ea5d327e0006"),
        ("t: { @{ @int }: 1, _: 0 }", "b6130220f9d82a6d1a5686b41c622853a844aa649ac056e81c4871b525c3e582"),
        ("f: (x -> x + 1)", "258a1a08adfd0340cdbe103e4f4851bea107a67e298e72d6679badc0595bfd1f"),
    ] {
        let w = Ws::new("g2");
        check(&mut bad, &format!("{src}: write"), w.write(&format!("{src}\n")), "e0 c0");
        check(&mut bad, &format!("{src}: root"), w.root(), want);
    }
    verdict(bad, 10);
}
