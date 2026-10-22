// A hidden facet taken for a name.
// Order: nlang-tools/docs/a_hidden_facet_taken_for_a_name_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-099.  D115: a hidden type `~@a`
// and a hidden logic `~/a` take identity axes 7 and 8 (REAL_03 §6.2), names
// without the prefix; a store declaring encoding ≤8 keeps the v0.99.0 rule,
// new stores declare encoding 9. SYNTAX_05 §4 #1: `a`, `@a`, `/a` and their
// hidden versions coexist and do not overwrite one another.
//
// ── What is wrong (v0.50.0 through v0.99.0) ─────────────────────────────
//
// `~@T` and `~/f` are parsed as private data keys named `@T` and `/f`.
// `{ ~a: 2, ~@a: 3, k: a }.k` reads `3`: the exact `~a` lookup in a scope
// frame never matches, and the prefix fallback finds `@a` first. `~@T`
// prints as `~"@T"`, which does not read back, so committing it fails, and a
// root `~/f` fails the same way. The address is axis 6 with the prefixed
// name, not axis 7 or 8.
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
        let scratch = nlang_interpreter::ScratchDir::new(&format!("hidden-facet-{tag}"));
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

    /// Evolve `src`, then commit: "" when both succeed.
    fn commit(&self, src: &str) -> String {
        let (rc, o) = self.evolve(src);
        if rc != 0 {
            return format!("evolve rc={rc}: {}", o.chars().take(200).collect::<String>());
        }
        let (o, rc) = self.oo(&["commit", "-m", "a"]);
        if rc != 0 {
            return format!("commit rc={rc}: {}", squash(&o).chars().take(200).collect::<String>());
        }
        String::new()
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

/// `oo eval` from an empty directory (no universe).
fn eval_here(e: &str) -> String {
    let w = Ws::new("eval");
    w.eval(e)
}

fn conflict(got: &str) -> String {
    if got.starts_with("_|_") && got.contains("#conflict") { "_|_ #conflict".into() } else { got.into() }
}

/// Two hidden facets of one name, and what each name reads.
const PAIRS: [(&str, &str, &str); 6] = [
    ("data then type, a", "~a: 2, ~@a: 3, k: a", "2"),
    ("type then data, a", "~@a: 3, ~a: 2, k: a", "2"),
    ("data then type, @a", "~a: 2, ~@a: 3, k: @a", "3"),
    ("data then type, ~@a", "~a: 2, ~@a: 3, k: ~@a", "3"),
    ("data then logic, a", "~a: 2, ~/a: (x -> 0), k: a", "2"),
    ("data then logic, /a", "~a: 2, ~/a: (x -> 0), k: /a 5", "0"),
];

/// r1 — Each facet of one name reads its own, in place and committed.
#[test]
fn r1_facets_of_one_name_read_their_own() {
    let mut bad = Vec::new();
    for (label, fields, want) in PAIRS {
        check(&mut bad, &format!("{label}: in place"), eval_here(&format!("{{ {fields} }}.k")), want);
        let w = Ws::new("r1");
        check(&mut bad, &format!("{label}: commit"), w.commit(&format!("t: {{ {fields} }}\n")), "");
        check(&mut bad, &format!("{label}: committed"), w.eval("_.t.k"), want);
    }
    verdict(bad, PAIRS.len() * 3);
}

/// r2 — Hidden facets are written and read back: at the root, in a field,
/// and the generic example of SPEC_05 §5.1. Outside, they stay hidden.
#[test]
fn r2_hidden_facets_commit_and_read_back() {
    let mut bad = Vec::new();
    let cells: [(&str, &str, &str); 5] = [
        ("root type", "~@T: @int\nr: 5 & @T\n", "5"),
        ("root type refuses", "~@T: @int\nr: \"s\" & @T\n", "_|_ #conflict"),
        ("root logic", "~/f: (x -> x + 1)\nr: /f 2\n", "3"),
        ("field type", "t: { ~@T: @int, r: 5 & @T }\nr: t.r\n", "5"),
        ("generic", "@Box: {\n    ~@T: @any\n    value: ~@T\n}\n@IntBox: @Box & { ~@T: @int }\nok: { value: 42 } & @IntBox\nr: ok.value\n", "42"),
    ];
    for (label, src, want) in cells {
        let w = Ws::new("r2");
        check(&mut bad, &format!("{label}: commit"), w.commit(src), "");
        check(&mut bad, &format!("{label}: _.r"), conflict(&w.eval("_.r")), want);
    }
    let w = Ws::new("r2-outside");
    check(&mut bad, "outside: commit", w.commit("t: { ~@T: @int, ~/f: (x -> x), v: 1 }\n"), "");
    check(&mut bad, "outside: _.t", w.eval("_.t"), "{ v: 1 }");
    let v = w.eval("_.t.~@T");
    check(&mut bad, "outside: _.t.~@T", v.contains("#private_access_violation").to_string(), "true");
    let v = w.eval("_.t.~/f");
    check(&mut bad, "outside: _.t.~/f", v.contains("#private_access_violation").to_string(), "true");
    verdict(bad, 5 * 2 + 4);
}

const P1: &str = "t: { ~/f: (x -> x + 1), r: /f 2 }\n";

impl Ws {
    fn decl(&self) -> String {
        fs::read_to_string(self.ws.join(".oo").join("objects.format")).unwrap_or_default().trim().to_string()
    }

    fn at_encoding_8(tag: &str) -> Ws {
        let w = Ws::new(tag);
        let oo = w.ws.join(".oo");
        fs::create_dir_all(&oo).unwrap();
        fs::write(oo.join("objects.format"), "encoding=8\n").unwrap();
        fs::write(oo.join("format"), "layout=9\n").unwrap();
        w
    }
}

/// r3 — New stores declare encoding 9 and write axes 7 and 8. A store that
/// declares 8 keeps the v0.99.0 rule (its root is the one v0.99.0 verifies);
/// `oo migrate` names the engines it locks out, and what was written before
/// still reads after it.
#[test]
fn r3_new_stores_declare_9_and_old_stores_keep_their_rule() {
    let mut bad = Vec::new();
    let w = Ws::new("r3-new");
    check(&mut bad, "new: commit", w.commit(P1), "");
    check(&mut bad, "new: declares", w.decl(), "encoding=10"); // AMENDED 2026-10-22 for Q-100 (D116): encoding 10
    check(&mut bad, "new: root", w.root(), "21e6e90ff86296bc70ceb297610288322aa287b970f69a79ffa97879a6298621");
    check(&mut bad, "new: _.t.r", w.eval("_.t.r"), "3");

    let w = Ws::at_encoding_8("r3-old");
    check(&mut bad, "old: commit", w.commit(P1), "");
    check(&mut bad, "old: declares", w.decl(), "encoding=8");
    check(&mut bad, "old: root", w.root(), "f42079d4bcc8f79ce6a4b4802cdc87cd352233cff5558cd4c24ce525daa77b15");
    check(&mut bad, "old: _.t.r", w.eval("_.t.r"), "3");
    let (o, rc) = w.oo(&["migrate", "--grant", "migrate"]);
    check(&mut bad, "migrate rc", rc.to_string(), "0");
    check(&mut bad, "migrate names the lockout", o.contains("unopenable by oo v0.98.0 through v0.100.0").to_string(), "true"); // AMENDED 2026-10-22 for Q-100 (D116): encoding 10
    check(&mut bad, "migrated declares", w.decl(), "encoding=10"); // AMENDED 2026-10-22 for Q-100 (D116): encoding 10
    check(&mut bad, "after migrate: _.t.r", w.eval("_.t.r"), "3");
    let (_, rc) = w.oo(&["log"]);
    check(&mut bad, "after migrate: log", rc.to_string(), "0");
    check(&mut bad, "after migrate: commit", w.commit("u: { ~@T: @int, r: 5 & @T }\n"), "");
    check(&mut bad, "after migrate: _.u.r", w.eval("_.u.r"), "5");
    verdict(bad, 15);
}

/// g1 — Roots that hold no hidden type or logic do not move (v0.99.0).
const UNTOUCHED: [(&str, &str); 4] = [
    ("x: 0\n", "b74d775816f6b5be6cc152577b7556f6c8fdcc63433b0e31068cc2484a94ae0f"),
    ("t: { ~p: 3, y: p }\n", "1b1a1daa0d332554a2e85939fa416ca5d71877423db753a1bdbc2e339b30398e"),
    ("t: { @T: @int, /f: (x -> x), v: 1 }\n", "4271b205b49c56a4326585ab6295f51798e2af3fae27210fce1318ff8c44cc14"),
    ("~p: 3\ny: p\n", "f8b00ed33b9a22c0118f0f8176b5d89b506db8c45060e56cde7d82f2299cc166"),
];

#[test]
fn g1_roots_without_hidden_facets_do_not_move() {
    let mut bad = Vec::new();
    for (src, root) in UNTOUCHED {
        for at8 in [false, true] {
            let w = if at8 { Ws::at_encoding_8("g1-8") } else { Ws::new("g1") };
            let label = format!("{}{}", src.trim().replace('\n', "; "), if at8 { " (encoding 8)" } else { "" });
            check(&mut bad, &format!("{label}: commit"), w.commit(src), "");
            check(&mut bad, &format!("{label}: root"), w.root(), root);
        }
    }
    verdict(bad, UNTOUCHED.len() * 4);
}

/// g2 — The six facets of one name, all present, read their own (green
/// today: the exact names are found first).
#[test]
fn g2_six_facets_read_their_own() {
    let mut bad = Vec::new();
    let six = "a: 1, ~a: 2, @a: @int, ~@a: @str, /a: (x -> 10), ~/a: (x -> 20)";
    for (read, want) in [("a", "1"), ("~a", "2"), ("5 & @a", "5"), ("\"s\" & ~@a", "\"s\""), ("/a 0", "10"), ("~/a 0", "20")] {
        check(&mut bad, read, eval_here(&format!("{{ {six}, k: {read} }}.k")), want);
    }
    verdict(bad, 6);
}
