// A name spread in but not seen.
// Order: nlang-tools/docs/a_name_spread_in_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-078.  No new ruling: SPEC_03 §3.1,
// SPEC_04 §2.1 and §3.1, SYNTAX_05 §4 #6 already decide every cell here.
//
// ── What is wrong (v0.79.0; every cell the same back to v0.27.0) ─────────
//
// `...X` lays X's fields flat into the container it is written in (SPEC_03
// §3.1), and a bare name is looked up in the fields of the container first
// (SPEC_04 §2.1). Inside a Combo neither half holds:
//   * a sibling does not see a name spread in:
//       `{ ...{ j: 5 }, q: j }.q`  ⟹ `_`   (`.j` ⟹ 5)
//       `{ ...~%Math, r: add (1, 2) }.r`  ⟹ `_`   (import is spread)
//       `{ j: 100, t: { ...{ j: 5 }, q: j } }.t.q`  ⟹ 100
//       `{ ...{ j: 5 }, j: 7, q: j }.q`  ⟹ 7   (`.j` ⟹ ⊥ #conflict)
//   * a spread source does not see its container:
//       `{ s: { j: 5 }, ...s }.j`  ⟹ `_`   (the spread never happens)
//       `{ n: 1, ...{ k: n } }.k`  ⟹ `_`
//   * a spread of a container built with a spread drops what it brought in:
//       `{ a: { ...{ j: 5 } }, ...a }.j`  ⟹ `_`
//   * a container spreading itself is `{}`, not ⊥ #divergent:
//       `{ s: { ...s } }.s`  ⟹ `{}`   (the top level answers #divergent)
// A committed field reads the same `_`. At the top level a sibling sees a
// spread name, but a spread source written above its definition is dropped:
//       `...s` then `s: { j: 1 }`  ⟹ `_.j` is `_`  (the other order: 1)
//
// The delivery may NOT edit this file. `rustfmt` must not touch it.

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

struct Ws {
    _scratch: nlang_interpreter::ScratchDir,
    root: PathBuf,
    ws: PathBuf,
}

impl Ws {
    fn new(tag: &str) -> Self {
        let scratch = nlang_interpreter::ScratchDir::new(&format!("name-spread-in-{tag}"));
        let root = scratch.path().to_path_buf();
        let ws = root.join("ws");
        fs::create_dir_all(&ws).unwrap();
        Ws { _scratch: scratch, root, ws }
    }

    /// Run `oo`, killing it after 60 s: a cycle must answer, not hang.
    fn oo(&self, args: &[&str]) -> (String, i32) {
        let mut child = Command::new(env!("CARGO_BIN_EXE_oo"))
            .args(args)
            .current_dir(&self.ws)
            .env("OO_IDENTITY", self.root.join("identity"))
            .env("OO_NODE_HOME", self.root.join("node-home"))
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
            if start.elapsed() > Duration::from_secs(60) {
                let _ = child.kill();
                let _ = child.wait();
                return (format!("TIMEOUT after 60 s: oo {}", args.join(" ")), -1);
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let o = child.wait_with_output().expect("output");
        (
            format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)).trim().to_string(),
            o.status.code().unwrap_or(-1),
        )
    }

    fn eval(&self, e: &str) -> String {
        self.oo(&["eval", e]).0
    }

    /// Evolve then commit one source file in this universe.
    fn commit(&self, src: &str) -> (String, i32) {
        fs::write(self.ws.join("a.n"), src).unwrap();
        let (o, rc) = self.oo(&["evolve", "a.n"]);
        if rc != 0 {
            return (o, rc);
        }
        self.oo(&["commit", "-m", "a"])
    }

    /// The root digest the last commit wrote.
    fn root(&self) -> String {
        let head = fs::read_to_string(self.ws.join(".oo").join("HEAD")).unwrap_or_default();
        let (o, _) = self.oo(&["inspect", head.trim()]);
        o.lines()
            .find(|l| l.contains("root:"))
            .and_then(|l| l.rsplit(':').next())
            .unwrap_or("")
            .trim()
            .to_string()
    }
}

fn is(o: &str, want: &str) -> bool {
    o == want
}

fn has(o: &str, want: &str) -> bool {
    !o.starts_with("_|_") && o.contains(want)
}

/// ⊥ with this cause.
fn bot(o: &str, cause: &str) -> bool {
    o.starts_with("_|_") && o.contains(&format!("%cause: {cause}"))
}

/// Run every `(expression, check, expectation)` and report all misses at once.
fn table(w: &Ws, cases: &[(&str, fn(&str, &str) -> bool, &str)]) {
    let mut wrong = Vec::new();
    for (e, check, want) in cases {
        let o = w.eval(e);
        if !check(&o, want) {
            wrong.push(format!("`{e}` => {o}   (wanted {want})"));
        }
    }
    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
}

/// Each `(source, expression, want)` in a fresh universe: commit, then read.
fn committed(cases: &[(&str, &str, &str)]) -> Vec<String> {
    let mut wrong = Vec::new();
    for (i, (src, e, want)) in cases.iter().enumerate() {
        let w = Ws::new(&format!("c{i}"));
        let (o, rc) = w.commit(src);
        if rc != 0 {
            wrong.push(format!("commit of `{}` failed: {o}", src.replace('\n', "; ")));
            continue;
        }
        let o = w.eval(e);
        if o != *want {
            wrong.push(format!("after `{}`: `{e}` => {o}   (wanted {want})", src.replace('\n', "; ")));
        }
    }
    wrong
}

fn root_of(tag: &str, src: &str) -> Result<String, String> {
    let w = Ws::new(tag);
    let (o, rc) = w.commit(src);
    if rc != 0 {
        return Err(format!("commit of `{}` failed: {o}", src.replace('\n', "; ")));
    }
    Ok(w.root())
}

// ── Red on v0.79.0 ────────────────────────────────────────────────────────

/// SPEC_03 §3.1 + SPEC_04 §2.1: a name spread into a container is a field of
/// it, so its fields and its children find it by bare name — in every kind of
/// container and every place a container is written.
#[test]
fn r1_a_sibling_reads_a_spread_name() {
    let w = Ws::new("r1");
    table(&w, &[
        ("{ ...{ j: 5 }, q: j }.q", is, "5"),
        ("{ q: j, ...{ j: 5 } }.q", is, "5"),
        ("{ ...{ j: 5 }, q: j + 1 }.q", is, "6"),
        ("{ ...{ j: 5 }, a: { b: j } }.a.b", is, "5"),
        ("{ a: { ...{ j: 5 }, q: j } }.a.q", is, "5"),
        ("{ j: 100, t: { ...{ j: 5 }, q: j } }.t.q", is, "5"),
        ("{{ ...{ j: 5 }, q: j }}.q", is, "5"),
        ("{ ...{{ j: 5 }}, q: j }.q", is, "5"),
        ("{ ...#ok, q: %val }.q", is, "#ok"),
        ("{ ...{ g: (x -> x + 1) }, r: g 1 }.r", is, "2"),
        ("{ ...{ g: (x -> x + 1) }, r: /g 1 }.r", is, "2"),
        ("(x -> { ...{ j: x }, q: j }.q) 5", is, "5"),
        ("{ ...{ j: 5 }, q: j } |> (c -> c.q)", is, "5"),
        ("{ @{ @int }: j, ...{ j: 5 } } 3", is, "5"),
        ("{ @{ @int }: \"n\", ...{ j: 5 }, q: j }.q", is, "5"),
        ("{ ...{ j: 5 }, q: j }", has, "q: 5"),
    ]);
}

/// SYNTAX_05 §4 #6: import is spread. A module spread into a Combo is read by
/// bare name there, as it is at the top level.
#[test]
fn r2_an_import_inside_a_combo() {
    let w = Ws::new("r2");
    table(&w, &[
        ("{ ...~%Math, r: add (1, 2) }.r", is, "3"),
        ("{ ...~%Math, r: /add (1, 2) }.r", is, "3"),
    ]);
}

/// SPEC_03 §3.1 collision merge: a key both written and spread in is the meet,
/// and a sibling reads the meet — not one half of it.
#[test]
fn r3_a_sibling_reads_the_meet() {
    let w = Ws::new("r3");
    table(&w, &[
        ("{ ...{ j: 5 }, j: 7, q: j }.q", bot, "#conflict"),
        ("{ ...{ j: 5 }, ...{ j: 7 }, q: j }.q", bot, "#conflict"),
        ("{ ...{ j: 5 }, j: @int, q: j }.q", is, "5"),
    ]);
}

/// A spread source is an expression written inside its container, so it
/// resolves names there (SPEC_04 §2.1, private ones included, §3.1 #1) —
/// sibling fields, names other spreads bring in, wherever they are written.
#[test]
fn r4_a_spread_source_reads_its_container() {
    let w = Ws::new("r4");
    table(&w, &[
        ("{ s: { j: 5 }, ...s }.j", is, "5"),
        ("{ ...s, s: { j: 5 } }.j", is, "5"),
        ("{ s: { j: 5 }, ...s, q: j }.q", is, "5"),
        ("{ q: j, ...s, s: { j: 5 } }.q", is, "5"),
        ("{ n: 1, ...{ k: n } }.k", is, "1"),
        ("{ ~h: 3, ...{ k: h } }.k", is, "3"),
        ("{ ...{ j: 5 }, ...{ k: j } }.k", is, "5"),
        ("{ ...{ j: 5, k: j + 1 }, q: k }.q", is, "6"),
        ("{ ...{ s: { j: 1 } }, ...s, q: j }.q", is, "1"),
        ("{ ...s, ...{ s: { j: 1 } }, q: j }.q", is, "1"),
    ]);
}

/// Spreading a container that was itself built with a spread carries the
/// fields that spread brought in.
#[test]
fn r5_a_spread_carries_what_its_source_brought_in() {
    let w = Ws::new("r5");
    table(&w, &[
        ("{ a: { ...{ j: 5 } }, ...a }.j", is, "5"),
        ("{ ...{ ...{ j: 5 } } }.j", is, "5"),
        ("{ ...{ ...{ j: 5 } }, q: j }.q", is, "5"),
        ("{ ...{ ...{ j: 5 } } }", has, "j: 5"),
    ]);
}

/// SPEC_03 §3.1 circular spread: a container spreading itself or an ancestor
/// is ⊥ #divergent — and answers, it does not hang or crash.
#[test]
fn r6_a_container_spreading_itself_is_divergent() {
    let w = Ws::new("r6");
    table(&w, &[
        ("{ s: { ...s } }.s", bot, "#divergent"),
        ("{ a: { ...b }, b: { ...a } }.a", bot, "#divergent"),
    ]);
}

/// The committed face: a field committed with a spread keeps what it reads.
#[test]
fn r7_a_committed_container_reads_its_spread_names() {
    let mut wrong = committed(&[
        ("t: { ...{ j: 5 }, q: j }\n", "_.t.q", "5"),
        ("t: { s: { j: 5 }, ...s, q: j }\n", "_.t.q", "5"),
        ("t: { s: { j: 5 }, ...s }\n", "_.t.j", "5"),
        ("u: { ...~%Math, r: add (1, 2) }\n", "_.u.r", "3"),
        ("w: { a: { ...{ j: 5 } }, ...a }\n", "_.w.j", "5"),
    ]);
    // The same file run without a universe.
    let w = Ws::new("r7run");
    fs::write(w.ws.join("v.n"), "t: { ...{ j: 5 }, q: j }\n").unwrap();
    let (o, _) = w.oo(&["run", "--observe", "t.q", "v.n"]);
    if o != "5" {
        wrong.push(format!("`run --observe t.q` => {o}   (wanted 5)"));
    }
    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
}

/// SPEC_03 §3.1 timing: where a top-level spread source is defined in the file
/// must not change the result — the value, and so the root.
#[test]
fn r8_a_top_level_spread_source_may_come_later() {
    let mut wrong = committed(&[
        ("...s\ns: { j: 1 }\n", "_.j", "1"),
        ("...s\n...{ s: { j: 1 } }\nq: j\n", "_.q", "1"),
    ]);
    // A spread at the top is its fields written there: a collision is the
    // refusal `j: 7` then `j: 5` gets, wherever the spread or its source is
    // written, and against the committed root too.
    for (i, src) in ["...s\ns: { j: 1 }\nj: 2\n", "j: 7\n...{ j: 5, k: 1 }\n"].iter().enumerate() {
        let w = Ws::new(&format!("r8c{i}"));
        fs::write(w.ws.join("a.n"), src).unwrap();
        let (o, rc) = w.oo(&["evolve", "a.n"]);
        if rc == 0 || !o.contains("#conflict") {
            wrong.push(format!("evolve `{}` => rc={rc} {o}   (wanted a refusal naming #conflict)", src.replace('\n', "; ")));
        }
    }
    let w = Ws::new("r8r");
    let _ = w.commit("j: 7\n");
    fs::write(w.ws.join("b.n"), "...{ j: 5, k: 1 }\n").unwrap();
    let (o, rc) = w.oo(&["evolve", "b.n"]);
    if rc == 0 || !o.contains("#conflict") {
        wrong.push(format!("after committing `j: 7`, evolve `...{{ j: 5, k: 1 }}` => rc={rc} {o}   (wanted a refusal naming #conflict)"));
    }
    for (i, (a, b)) in [
        ("s: { j: 1 }\n...s\n", "...s\ns: { j: 1 }\n"),
        ("...{ s: { j: 1 } }\n...s\nq: j\n", "...s\n...{ s: { j: 1 } }\nq: j\n"),
    ]
    .iter()
    .enumerate()
    {
        match (root_of(&format!("o{i}a"), a), root_of(&format!("o{i}b"), b)) {
            (Ok(x), Ok(y)) if x == y && !x.is_empty() => {}
            (x, y) => wrong.push(format!(
                "`{}` root {:?} vs `{}` root {:?}   (wanted one root)",
                a.replace('\n', "; "),
                x,
                b.replace('\n', "; "),
                y
            )),
        }
    }
    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
}

/// A morphism that spreads its own recursive call reads itself from the
/// container it is defined in — and is not a circular spread. Depth must not
/// cost more than depth: 40 levels answer, like 3.
#[test]
fn r9_a_container_spreading_a_recursive_call() {
    let w = Ws::new("r9");
    let g = "g: (n -> ~%Math./gt (n, 0) ? { ...(g (n - 1)), k: 1 } : { z: 0 })";
    let mut wrong = Vec::new();
    for d in [3, 40] {
        let e = format!("{{ {g}, r: g {d} }}.r");
        let o = w.eval(&e);
        if !(has(&o, "k: 1") && o.contains("z: 0")) {
            wrong.push(format!("`{e}` => {o}   (wanted k: 1 and z: 0)"));
        }
    }
    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
}

/// An effect spread in runs when it is read — once — whoever reads it.
#[test]
fn r10_a_sibling_reading_a_spread_effect_runs_it_once() {
    let w = Ws::new("r10");
    let o = w.eval(r#"{ ...{ w: ~%Io./append_file ("log.txt", "a") }, q: w }.q"#);
    let log = fs::read_to_string(w.ws.join("log.txt")).unwrap_or_default();
    assert!(o == "#true  ;; %effect: #io" && log == "a", "`.q` => {o}, log {log:?}   (wanted #true with #io, log \"a\")");
}

// ── Guards: green on v0.79.0, must stay green ────────────────────────────

/// `^` counts containers; a spread adds fields, not a container.
/// (Mutation: give a container's fields an extra frame for what spreads
/// brought in, instead of the one they have ⟹ `^.j` reads the container
/// itself ⟹ red.)
#[test]
fn g1_caret_counts_containers() {
    let w = Ws::new("g1");
    table(&w, &[
        ("{ ...{ j: 5 }, q: ^.j }.q", is, "_"),
        ("{ y: 1, a: { ...{ j: 5 }, b: ^.y } }.a.b", is, "1"),
        ("{ ...{ j: 5 }, a: { b: ^.j } }.a.b", is, "5"),
    ]);
    let wrong = committed(&[("t: { ...{ j: 5 }, p: ^.j }\n", "_.t.p", "_")]);
    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
}

/// Lexical scope stays lexical: a spread-in field keeps the scope it was
/// written in; a private field of a spread source does not leak (SPEC_03
/// §3.1 spread isolation); `&` does not share scope; an outer name is still
/// found when nothing nearer defines it; mutual references stay as they are.
/// (Mutation: spread a source's private fields regardless of where the spread
/// is written ⟹ red.)
#[test]
fn g2_lexical_scope_stays_lexical() {
    let w = Ws::new("g2");
    table(&w, &[
        ("{ k: 2, ...{ k: 1, q: k } }.q", is, "1"),
        ("{ ...{ ~h: 3, k: h } }.k", is, "3"),
        ("{ ...{ ~h: 3, k: 1 }, q: h }.q", is, "_"),
        ("{ ...{ ~h: 3 } }", is, "{}"),
        ("({ ...{ j: 5 } } & { q: j }).q", is, "_"),
        ("{ j: 100, t: { ...{ k: 5 }, q: j } }.t.q", is, "100"),
        ("{ j: k, k: j }.j", is, "_"),
        ("{ ...{ j: k }, ...{ k: j } }.j", is, "_"),
        ("{ ..._, q: 1 }.q", is, "1"),
        ("{ ...{ j: 5 }, j: 7 }.j", bot, "#conflict"),
    ]);
}

/// Values whose meaning does not change keep their address: no spread at all,
/// spreads whose fields are already solid, and spreads whose source reads only
/// itself. (Mutation: give every spread source the container it is written in,
/// whether it reads outside itself or not ⟹ the last two move ⟹ red.)
#[test]
fn g3_unchanged_values_keep_their_root() {
    let mut wrong = Vec::new();
    for (i, (src, want)) in [
        ("t: { j: 5, q: j }\n", "f13c087abd2c252692689bc8fb710ee2e1c49f6f520188a1dabe3e422f3912c3"),
        ("t: { ...{ j: 5 }, q: 1 }\n", "8a60092ad7227919afeebc0a9c520a3909d5e10de58e3e6bb7befa1d50fd7683"),
        ("t: { a: 1, ...{ b: 2 } }\n", "cbae84692c1ed07da0af21770cea6cbf8e071a9d1ad06e4b719f0fc201f9968a"),
        ("...{ j: 5 }\nq: j\n", "f03a7d869c4fad4909bf230faee612aeafd305144165f22efaaa79ecc7334cd3"),
        ("s: { j: 5 }\n...s\n", "cb50248338128f5f460cc9dd0594239e799e39bd05c53ce47b779ede525f258b"),
        ("t: { ...{ j: 5, k: j } }\n", "065a2b711806d400167410c9f34f314a8b8a3dab82d7516608ee3a4528d97262"),
        ("t: { k: 2, ...{ k: 1, q: k } }\n", "6dfd21e6eb4ccb9bfb9742d20817304309081b6aeabf4b77d925d97ebfda7aff"),
    ]
    .iter()
    .enumerate()
    {
        match root_of(&format!("g3{i}"), src) {
            Ok(r) if r == *want => {}
            other => wrong.push(format!("`{}` => {:?}   (wanted {want})", src.replace('\n', "; "), other)),
        }
    }
    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
}

/// The top level keeps its refusals and its no-ops: a collision through a
/// spread is still an evolve conflict, a self-spread still commits as
/// #divergent, a source that is never defined is still nothing, and a spread
/// that agrees still lands. (Mutation: refuse a top-level spread whose source
/// is not defined ⟹ red.)
#[test]
fn g4_top_level_refusals_and_no_ops() {
    let mut wrong = Vec::new();
    let w = Ws::new("g4");
    fs::write(w.ws.join("a.n"), "...{ j: 5 }\nj: 7\n").unwrap();
    let (o, rc) = w.oo(&["evolve", "a.n"]);
    if rc == 0 || !o.contains("#conflict") {
        wrong.push(format!("evolve `...{{ j: 5 }}; j: 7` => rc={rc} {o}   (wanted a refusal naming #conflict)"));
    }
    wrong.extend(committed(&[
        ("s: { ...s }\n", "_.s", "_|_  ;; %cause: #divergent"),
        ("...nothing\nk: 1\n", "_.k", "1"),
        ("j: 5\n...{ j: 5, k: 1 }\n", "_.k", "1"),
    ]));
    // Agreement with the committed root still lands.
    let w = Ws::new("g4r");
    let _ = w.commit("j: 7\n");
    let (o, rc) = w.commit("...{ j: 7, k: 1 }\n");
    let k = w.eval("_.k");
    if rc != 0 || k != "1" {
        wrong.push(format!("after committing `j: 7`, `...{{ j: 7, k: 1 }}` => rc={rc} {o}; `_.k` => {k}   (wanted 1)"));
    }
    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
}

/// An effect spread in runs only when read, and once. (Mutation: settle what
/// a spread brings in when the container expands ⟹ the unread effect runs ⟹
/// red.)
#[test]
fn g5_an_effect_spread_in_runs_when_read() {
    let mut wrong = Vec::new();
    for (i, (e, want, log_want)) in [
        (r#"{ ...{ w: ~%Io./append_file ("log.txt", "a") }, q: 1 }.q"#, "1", ""),
        (r#"{ ...{ w: ~%Io./append_file ("log.txt", "a") }, q: 1 }.w"#, "#true  ;; %effect: #io", "a"),
        (r#"{ w: ~%Io./append_file ("log.txt", "a") }.w"#, "#true  ;; %effect: #io", "a"),
    ]
    .iter()
    .enumerate()
    {
        let w = Ws::new(&format!("g5{i}"));
        let o = w.eval(e);
        let log = fs::read_to_string(w.ws.join("log.txt")).unwrap_or_default();
        if o != *want || log != *log_want {
            wrong.push(format!("`{e}` => {o}, log {log:?}   (wanted {want}, log {log_want:?})"));
        }
    }
    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
}
