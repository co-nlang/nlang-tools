// A contradiction computed before the meet.
// Order: nlang-tools/docs/a_contradiction_computed_before_the_meet_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-106.  D119: in a value context
// (arithmetic operands, `==`/`!=` operands, the `~%Math` arithmetic
// builtins) an operand that is not yet a value — a type, a range, a
// complement — answers `_`, as `_ + 1` does. A bottom is absorbing under
// refinement; an answer that a later refinement lifts was never one.
//
// ── What is wrong (v0.106.0) ────────────────────────────────────────────
//
//   `@int + 1`, `(1..) * 2`, `!0 - 1`, `~%Math./add (@int, 1)` ⟹ ⊥ #conflict
//   `(1..) == 5` ⟹ `#false`, and `5 == 5` ⟹ `#true`
// so a derived field is frozen before the meet that would answer it:
//   one file `n: @int`, `r: n + 1`, `n: 5` ⟹ `_.r` is ⊥; `n: 5` first ⟹ `6`
//   `h: { k: @int, r: k + 1 }`, then commit `h: { k: 5 }` ⟹ ⊥ (one file `6`)
//
// The delivery may NOT edit this file. `rustfmt` must not touch it.


#![allow(dead_code)]

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
        let scratch = nlang_interpreter::ScratchDir::new(&format!("before-the-meet-{tag}"));
        let root = scratch.path().to_path_buf();
        let ws = root.join("ws");
        fs::create_dir_all(&ws).unwrap();
        Ws { _scratch: scratch, root, ws }
    }

    /// Run `oo`, killing it after 120 s: (output, rc).
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
            if start.elapsed() > Duration::from_secs(120) {
                let _ = child.kill();
                let _ = child.wait();
                return (format!("TIMEOUT after 120 s: oo {}", args.join(" ")), -1);
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let o = child.wait_with_output().expect("output");
        (
            format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)).trim().to_string(),
            o.status.code().unwrap_or(-1),
        )
    }

    /// Evolve `src` from a new file: (rc, output).
    fn evolve(&self, src: &str) -> (i32, String) {
        let n = fs::read_dir(&self.ws).map(|d| d.count()).unwrap_or(0);
        let f = format!("f{n}.n");
        fs::write(self.ws.join(&f), src).unwrap();
        let (o, rc) = self.oo(&["evolve", &f]);
        (rc, squash(&o))
    }

    /// Evolve each source as its own file, then commit: the evolve exit codes
    /// and the commit's ("e0 e1 c0").
    fn writes(&self, srcs: &[&str]) -> String {
        let mut out = Vec::new();
        for s in srcs {
            out.push(format!("e{}", self.evolve(s).0));
        }
        let (_, rc) = self.oo(&["commit", "-m", "a"]);
        out.push(format!("c{rc}"));
        out.join(" ")
    }

    /// `oo eval`, the trailing effect note dropped, whitespace collapsed.
    fn eval(&self, e: &str) -> String {
        let s = self.oo(&["eval", e]).0;
        let s = match s.find(";; %effect") {
            Some(i) => s[..i].trim().to_string(),
            None => s,
        };
        squash(&s).chars().take(120).collect()
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

    /// Copy `other`'s working-set members in: two writes that did not see
    /// each other, as two concurrent evolves leave them.
    fn take_members_of(&self, other: &Ws) {
        let from = other.ws.join(".oo").join("injections");
        let to = self.ws.join(".oo").join("injections");
        for e in fs::read_dir(&from).unwrap() {
            let p = e.unwrap().path();
            fs::copy(&p, to.join(p.file_name().unwrap())).unwrap();
        }
    }
}

fn squash(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn check(bad: &mut Vec<String>, label: &str, got: String, want: &str) {
    if got != want {
        bad.push(format!("  {label}\n    want {want}\n    got  {got}"));
    }
}

fn verdict(bad: Vec<String>, n: usize) {
    assert!(bad.is_empty(), "{} of {} wrong:\n{}", bad.len(), n, bad.join("\n"));
}

/// `oo eval` from an empty directory (no universe), a ⊥ reduced to its cause.
fn eval_here(e: &str) -> String {
    let w = Ws::new("eval");
    bottom(&w.eval(e))
}

fn bottom(got: &str) -> String {
    for cause in ["#conflict", "#no_matching_branch"] {
        if got.starts_with("_|_") && got.contains(cause) {
            return format!("_|_ {cause}");
        }
    }
    got.into()
}

/// The root a single evolve of `src` commits.
fn root_of(src: &str) -> String {
    let w = Ws::new("root");
    let got = w.writes(&[src]);
    if got != "e0 c0" {
        return format!("not committed: {got}");
    }
    w.root()
}

fn table(bad: &mut Vec<String>, cells: &[(&str, &str)]) {
    for (e, want) in cells {
        check(bad, e, eval_here(e), want);
    }
}

/// The readings of `q` after `w1` then `w2` written as one file, as two
/// evolves and one commit, and as two commits; and the three roots.
fn three_ways(w1: &str, w2: &str, q: &str) -> (Vec<String>, Vec<String>) {
    let mut got = Vec::new();
    let mut roots = Vec::new();
    for ways in [vec![vec![format!("{w1}\n{w2}")]], vec![vec![w1.to_string(), w2.to_string()]], vec![vec![w1.to_string()], vec![w2.to_string()]]] {
        let w = Ws::new("three");
        let mut rc = Vec::new();
        for commit in &ways {
            let srcs: Vec<&str> = commit.iter().map(|s| s.as_str()).collect();
            rc.push(w.writes(&srcs));
        }
        got.push(format!("{} | {}", rc.join(" "), bottom(&w.eval(q))));
        roots.push(w.root());
    }
    (got, roots)
}

/// r1 — D119: a value context answers `_` when an operand is not yet a
/// value — a type, a range, a complement — on either side, for each
/// operator, `==`/`!=`, and the `~%Math` arithmetic builtins (which answer
/// `_` for `_` too, as `_ + 1` does).
#[test]
fn r1_a_value_context_answers_top_for_what_is_not_yet_a_value() {
    let mut bad = Vec::new();
    let cells = [
        ("@int + 1", "_"),
        ("1 + @int", "_"),
        ("@num * 2", "_"),
        ("@float - 1", "_"),
        ("@int / 2", "_"),
        ("@int % 2", "_"),
        ("@str + 1", "_"),
        ("(1..) + 1", "_"),
        ("(..9) - 1", "_"),
        ("(1..9) * 2", "_"),
        ("!0 + 1", "_"),
        ("@int + @int", "_"),
        ("(@int | 3) + 1", "_"),
        ("@int == 5", "_"),
        ("5 != @int", "_"),
        ("(1..) == 5", "_"),
        ("(1..9) != 5", "_"),
        ("!0 == 1", "_"),
        ("~%Math./add (@int, 1)", "_"),
        ("~%Math./add (_, 1)", "_"),
        ("~%Math./mul (2, 1..)", "_"),
        ("~%Math./sqrt (@num)", "_"),
        ("~%Math./lt (@int, 3)", "_"),
        ("~%Math./max (!0, 3)", "_"),
        ("{ n: @int, r: n + 1 }.r", "_"),
        ("({ n: @int, r: n + 1 } & { n: 5 }).r", "6"),
        ("({ n: 1.., r: n * 2 } & { n: 5 }).r", "10"),
        ("({ n: @int, r: n == 5 } & { n: 5 }).r", "#true"),
        ("({ n: @int, r: ~%Math./add (n, 1) } & { n: 5 }).r", "6"),
    ];
    table(&mut bad, &cells);
    verdict(bad, cells.len());
}

/// r2 — The other half: values, contradictions and anchors answer as
/// before. A bottom of values stays a bottom; an anchor stays sealed
/// (`SPEC_01` §2.6); a union distributes; a partial builtin stays partial.
#[test]
fn r2_values_and_contradictions_answer_as_before() {
    let mut bad = Vec::new();
    let cells = [
        ("5 + 1", "6"),
        ("\"a\" + \"b\"", "\"ab\""),
        ("(1 | 2) + 1", "2 | 3"),
        ("_ + 1", "_"),
        ("_ == 5", "_"),
        ("5 == 5", "#true"),
        ("5 != 5", "#false"),
        ("\"a\" + 1", "_|_ #conflict"),
        ("{ a: 1 } + 1", "_|_ #conflict"),
        ("{ a: 1 } == 1", "_|_ #conflict"),
        ("5 & 7", "_|_ #conflict"),
        ("({ n: @int, r: n & 7 } & { n: 5 }).r", "_|_ #conflict"),
        ("({ n: @int, r: n + \"a\" } & { n: 5 }).r", "_|_ #conflict"),
        ("~%Math./add (2, 1)", "3"),
        ("~%Math./add (\"a\", 1)", "_|_ #conflict"),
        ("(~%Math./add (1)) 2", "3"),
    ];
    table(&mut bad, &cells);
    let w = Ws::new("anchor");
    check(&mut bad, "#_ + 1", w.eval("#_ + 1").split(" ;;").take(2).collect::<Vec<_>>().join(" ;;"), "_|_ ;; %cause: #arithmetic_on_anchor");
    check(&mut bad, "1 / 0", w.eval("1 / 0").split(" ;;").take(2).collect::<Vec<_>>().join(" ;;"), "_|_ ;; %cause: #numerical_error");
    verdict(bad, cells.len() + 2);
}

/// One file of `lines` in the given order: the reading of `q` and the root.
fn one_file(lines: &[&str], q: &str) -> (String, String) {
    let w = Ws::new("order");
    let rc = w.writes(&[&lines.join("\n")]);
    (format!("{rc} | {}", bottom(&w.eval(q))), w.root())
}

/// r3 — A file is the meet of its lines: every line order gives one
/// reading and one root. Contradictions after the meet stay contradictions
/// in every order.
#[test]
fn r3_a_file_reads_the_same_in_every_line_order() {
    let mut bad = Vec::new();
    let mut n = 0;
    let cases: [(&[&str], &str, &str); 11] = [
        (&["n: @int", "r: n + 1", "n: 5"], "_.r", "e0 c0 | 6"),
        (&["n: 1..", "r: n + 1", "n: 5"], "_.r", "e0 c0 | 6"),
        (&["n: @int", "r: n * 2", "n: 5"], "_.r", "e0 c0 | 10"),
        (&["n: @int", "r: n == 5", "n: 5"], "_.r", "e0 c0 | #true"),
        (&["n: @int", "r: ~%Math./add (n, 1)", "n: 5"], "_.r", "e0 c0 | 6"),
        (&["n: @int", "m: n + 1", "r: m + 1", "n: 5"], "_.r", "e0 c0 | 7"),
        (&["n: @int", "r: (x -> x + 1) n", "n: 5"], "_.r", "e0 c0 | 6"),
        (&["n: @int", "r: { z: n + 1 }", "n: 5"], "_.r.z", "e0 c0 | 6"),
        (&["n: @int", "r: n & 7", "n: 5"], "_.r", "e0 c0 | _|_ #conflict"),
        (&["n: @int", "r: n + \"a\"", "n: 5"], "_.r", "e0 c0 | _|_ #conflict"),
        (&["h: { k: @int, r: k + 1 }", "h: { k: 5 }"], "_.h.r", "e0 c0 | 6"),
    ];
    for (lines, q, want) in cases {
        let mut perms: Vec<Vec<&str>> = vec![lines.to_vec()];
        let k = lines.len();
        for i in 0..k {
            let mut p = lines.to_vec();
            let x = p.remove(i);
            p.push(x);
            perms.push(p);
            let mut p = lines.to_vec();
            p.reverse();
            perms.push(p);
        }
        let mut roots = Vec::new();
        for p in &perms {
            let (got, root) = one_file(p, q);
            check(&mut bad, &format!("{}: {q}", p.join("; ")), got, want);
            roots.push(root);
            n += 1;
        }
        roots.sort();
        roots.dedup();
        check(&mut bad, &format!("{}: one root over every order", lines.join("; ")), roots.len().to_string(), "1");
        n += 1;
    }
    verdict(bad, n);
}

/// r4 — One file, two evolves, two commits: one reading, one root (Q-103),
/// at the root, in a record's first layer, below it, in a list.
#[test]
fn r4_three_ways_one_reading_one_root() {
    let mut bad = Vec::new();
    let cases = [
        ("n: @int\nr: n + 1", "n: 5", "_.r", "6", "f6dc16ae774bef4a5fb10d214847b2c84718d416654e9e11f3570fd92ef10db9"),
        ("n: 1..\nr: n + 1", "n: 5", "_.r", "6", "f6dc16ae774bef4a5fb10d214847b2c84718d416654e9e11f3570fd92ef10db9"),
        ("n: @int\nr: { z: n + 1 }", "n: 5", "_.r.z", "6", "f2f0a75760817dea5ddb9bf9f55c9ed1f617cdfd762cf55fc1b96c19e80d42e2"),
        ("n: @int\nr: [n + 1]", "n: 5", "_.r", "[6]", "5a933514b5074692293caab774ccb532a9c96c7d44ea75539d40aa44897fb5c6"),
        ("n: @int\nr: ~%Math./add (n, 1)", "n: 5", "_.r", "6", "652d1e77f5b9511322774d381be71919376c294d6b1c38b10db3b7e621119a12"),
        ("n: @int\nr: n == 5", "n: 5", "_.r", "#true", "2b8505cfc16b0404307e8b2917714bd851cb3e27aea68fe2fde8c791357cfd4a"),
        ("h: { k: @int, r: k + 1 }", "h: { k: 5 }", "_.h.r", "6", "bcb96b882977ae9d757df79203c77c816533425241393a1341161503f56eda37"),
        ("h: { k: @int, s: { r: ^.k + 1 } }", "h: { k: 5 }", "_.h.s.r", "6", "700b8c1709ae64c5fc11426096a77f5a5bac0d4f204534e77b3a9f3c38f3bd6c"),
        ("h: { k: !0, r: k * 2 }", "h: { k: 3 }", "_.h.r", "6", "0617bd58d5ab015711b30fa91b87705b87fd82a7bd9b2642143a3439b577336e"),
    ];
    let mut n = 0;
    for (w1, w2, q, want, root) in cases {
        let (got, roots) = three_ways(w1, w2, q);
        for (way, g) in ["one file", "two evolves", "two commits"].iter().zip(got) {
            let rc = if *way == "two evolves" { "e0 e0 c0" } else if *way == "one file" { "e0 c0" } else { "e0 c0 e0 c0" };
            check(&mut bad, &format!("{w1} / {w2}, {way}: {q}"), g, &format!("{rc} | {want}"));
            n += 1;
        }
        for (way, r) in ["one file", "two evolves", "two commits"].iter().zip(roots) {
            check(&mut bad, &format!("{w1} / {w2}, {way}: root"), r, root);
            n += 1;
        }
    }
    verdict(bad, n);
}

/// r5 — Three writes grouped five ways: one reading, one root.
#[test]
fn r5_three_writes_every_grouping_one_root() {
    let mut bad = Vec::new();
    let mut n = 0;
    for (a, b, c, q, want) in [
        ("n: @int", "r: n + 1", "n: 5", "_.r", "6"),
        ("h: { k: @int, r: k + 1 }", "h: { j: 1 }", "h: { k: 5 }", "_.h.r", "6"),
    ] {
        let groupings: Vec<(&str, Vec<Vec<String>>)> = vec![
            ("one file", vec![vec![format!("{a}\n{b}\n{c}")]]),
            ("three evolves", vec![vec![a.into(), b.into(), c.into()]]),
            ("three commits", vec![vec![a.into()], vec![b.into()], vec![c.into()]]),
            ("one, then two", vec![vec![a.into()], vec![format!("{b}\n{c}")]]),
            ("two, then one", vec![vec![format!("{a}\n{b}")], vec![c.into()]]),
        ];
        let mut roots = Vec::new();
        for (label, commits) in &groupings {
            let w = Ws::new("r5");
            for commit in commits {
                let srcs: Vec<&str> = commit.iter().map(|s| s.as_str()).collect();
                w.writes(&srcs);
            }
            check(&mut bad, &format!("{a} / {b} / {c}, {label}: {q}"), bottom(&w.eval(q)), want);
            n += 1;
            roots.push(w.root());
        }
        roots.sort();
        roots.dedup();
        check(&mut bad, &format!("{a} / {b} / {c}: one root over five groupings"), roots.len().to_string(), "1");
        n += 1;
    }
    verdict(bad, n);
}

/// r6 — A contradiction is still committed and still said (D33, D56): the
/// commit names where a bottom landed and why. A derivation over a type is
/// no longer one, so its commit says nothing.
#[test]
fn r6_a_contradiction_still_commits_and_says_where() {
    let mut bad = Vec::new();
    let mut n = 0;
    for (src, say) in [
        ("r: 1 + \"a\"", Some("#conflict at r")),
        ("n: 5\nr: n + \"a\"", Some("#conflict at r")),
        ("c: c + 1", Some("#divergent")),
        ("h: { k: 1 & 2 }", Some("#conflict at h.k")),
        ("n: @int\nr: n + 1", None),
        ("h: { k: @int, r: k + 1 }", None),
        ("n: 1..\nr: n == 5", None),
    ] {
        let w = Ws::new("r6");
        let (rc, _) = w.evolve(src);
        let (out, crc) = w.oo(&["commit", "-m", "a"]);
        check(&mut bad, &format!("{src}: rc"), format!("e{rc} c{crc}"), "e0 c0");
        let said = match say {
            Some(s) => out.contains(s).to_string(),
            None => (!out.contains("#conflict") && !out.contains("#divergent")).to_string(),
        };
        check(&mut bad, &format!("{src}: commit says {:?}", say.unwrap_or("nothing")), said, "true");
        n += 2;
    }
    verdict(bad, n);
}

/// r7 — Scale: 400 derived fields over a type, filled by a later write,
/// as one file and as two commits, read the filled value in time.
#[test]
fn r7_many_derived_fields_read_the_filled_value_in_time() {
    let mut bad = Vec::new();
    let big = format!("h: {{ k: @int, {} }}", (0..400).map(|i| format!("f{i}: k + {i}")).collect::<Vec<_>>().join(", "));
    let roots = format!("n: @int\n{}", (0..400).map(|i| format!("r{i}: n + {i}")).collect::<Vec<_>>().join("\n"));
    for (label, ways, q) in [
        ("record, one file", vec![vec![format!("{big}\nh: {{ k: 5 }}")]], "_.h.f399"),
        ("record, two commits", vec![vec![big.clone()], vec!["h: { k: 5 }".to_string()]], "_.h.f399"),
        ("root, one file", vec![vec![format!("{roots}\nn: 5")]], "_.r399"),
        ("root, two commits", vec![vec![roots.clone()], vec!["n: 5".to_string()]], "_.r399"),
    ] {
        let w = Ws::new("r7");
        let start = Instant::now();
        let mut rc = Vec::new();
        for commit in &ways {
            let srcs: Vec<&str> = commit.iter().map(|s| s.as_str()).collect();
            rc.push(w.writes(&srcs));
        }
        let took = start.elapsed();
        check(&mut bad, &format!("{label}: writes"), rc.join(" "), if ways.len() == 1 { "e0 c0" } else { "e0 c0 e0 c0" });
        check(&mut bad, &format!("{label}: {q}"), w.eval(q), "404");
        check(&mut bad, &format!("{label}: within 60 s"), (took < Duration::from_secs(60)).to_string(), "true");
    }
    verdict(bad, 12);
}

/// g1 — Roots that never held a value context over a type do not move
/// (v0.106.0).
#[test]
fn g1_roots_without_this_form_do_not_move() {
    let mut bad = Vec::new();
    let cases = [
        ("x: 0", "b74d775816f6b5be6cc152577b7556f6c8fdcc63433b0e31068cc2484a94ae0f"),
        ("r: 1 + 2", "a16d383953ae796e965293a07d6643371ec06fcfa61337ea649cf41851e0f15a"),
        ("n: 5\nr: n + 1", "f6dc16ae774bef4a5fb10d214847b2c84718d416654e9e11f3570fd92ef10db9"),
        ("r: 1 + \"a\"", "7674df6e0cf1aadcedf26afd44b62186155cc717799a1864be4e90d634611f97"),
        ("c: c + 1", "3f0a62e4ac00a7517e258bee6afbc348f33167cb1cd42e9bcefa419645c2da34"),
        ("n: 5\nr: n & 7", "fba672c829ec60c00b70cb6090e8b95ba38b16c96cd99eb6773bad45a146822f"),
        ("h: { k: 5, r: k + 1 }", "87b57f307c108e2fd303c78c69168e38a838b8828a1c02bdd3b96e536cf998a6"),
        ("h: { k: @int, r: k }", "9247b8a31920ff61cd5ae14c35c682bdf0d372d28c68e53d5036a6b80de3043e"),
        ("r: (1 | 2) + 1", "fda956e1d47ac9011a7c617026ffbe70ffbe72550b26ae0d8bb12fea4dadb9b0"),
    ];
    for (src, root) in cases {
        check(&mut bad, src, root_of(src), root);
    }
    verdict(bad, cases.len());
}
