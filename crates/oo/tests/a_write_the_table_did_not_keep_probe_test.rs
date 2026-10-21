// A write the table did not keep.
// Order: nlang-tools/docs/a_write_the_table_did_not_keep_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-098.  SPEC_07 §1: a constraint
// written more than once is one branch whose bodies meet, "whether they are
// written in one literal, through `&`, or brought together by later
// evolves"; D101: a table has one normal form, and the root's evolves one
// after another produce the same bytes.
//
// ── What is wrong (v0.79.0 through v0.98.0) ─────────────────────────────
//
// Evolve `@{ @int }: 0`, then evolve `x: 3`, then commit. Both evolves exit
// 0, `oo status` no longer lists `x: 3`, and `_.x` reads `_`. The same two
// writes in one file, or with a commit between them, keep `x`. Once the
// working set is a table its data keys are branches (D100), and the member
// an evolve writes is rebuilt by looking each written key up in the working
// set as a plain field: the key is not found and the write is dropped.
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
        let scratch = nlang_interpreter::ScratchDir::new(&format!("table-did-not-keep-{tag}"));
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

impl Ws {
    /// Evolve each step from its own file, then commit: "" when all succeed.
    fn steps(&self, steps: &[&str]) -> String {
        for (i, s) in steps.iter().enumerate() {
            let (rc, o) = self.evolve(s);
            if rc != 0 {
                return format!("evolve {} rc={rc}: {}", i + 1, o.chars().take(200).collect::<String>());
            }
        }
        String::new()
    }

    fn commit_now(&self) -> String {
        let (o, rc) = self.oo(&["commit", "-m", "a"]);
        if rc != 0 {
            return format!("commit rc={rc}: {}", squash(&o).chars().take(200).collect::<String>());
        }
        String::new()
    }

    fn status(&self) -> String {
        squash(&self.oo(&["status"]).0)
    }
}

/// The same writes, one evolve per step and in one file: the second must be
/// what the first is (root, working set) and read `want`.
fn same_as_one_file(bad: &mut Vec<String>, label: &str, steps: &[&str], read: &str, want: &str) {
    let apart = Ws::new(&format!("{label}-apart"));
    let together = Ws::new(&format!("{label}-together"));
    check(bad, &format!("{label}: evolves"), apart.steps(steps), "");
    check(bad, &format!("{label}: one file"), together.steps(&[&steps.join("\n")]), "");
    check(bad, &format!("{label}: status"), apart.status(), &together.status());
    check(bad, &format!("{label}: commit"), apart.commit_now(), "");
    check(bad, &format!("{label}: one file commit"), together.commit_now(), "");
    let r = together.root();
    check(bad, &format!("{label}: root"), apart.root(), if r.is_empty() { "(no root)" } else { &r });
    let got = apart.eval(read);
    if want.starts_with("_|_") {
        let ok = got.starts_with("_|_") && got.contains("#conflict");
        check(bad, &format!("{label}: {read}"), if ok { want.to_string() } else { got }, want);
    } else {
        check(bad, &format!("{label}: {read}"), got, want);
    }
}

const TABLE: &str = "@{ @int }: 0";

/// What the second evolve writes, the name it is read by, and its answer.
const AFTER: [(&str, &str, &str, &str); 8] = [
    ("named", "x: 3", "_.x", "3"),
    ("quoted", "\"q\": 3", "_.q", "3"),
    ("dotted", "t.b: 3", "_.t.b", "3"),
    ("record", "t: { b: 3 }", "_.t.b", "3"),
    ("private", "~p: 3\ny: p", "_.y", "3"),
    ("two", "x: 3\ny: 4", "_.y", "4"),
    ("arrow", "f: (z -> z + 1)", "_.f 4", "5"),
    ("recipe", "n: 2\nx: n + 1", "_.x", "3"),
];

#[test]
fn r1_a_write_after_a_table_is_kept() {
    let mut bad = Vec::new();
    for (label, w, read, want) in AFTER {
        same_as_one_file(&mut bad, label, &[TABLE, w], read, want);
    }
    verdict(bad, AFTER.len() * 7);
}

/// The same constraint written as a pattern key and as a plain key.
const SAME: [(&str, &str, &str, &str, &str); 4] = [
    ("clash", "@{ \"b\" }: 2", "b: 3", "_.b", "_|_ #conflict"),
    ("agree", "@{ \"b\" }: 3", "b: 3", "_.b", "3"),
    ("integer", "@{ 4 }: 2", "4: 3", "_.4", "_|_ #conflict"),
    ("held", "@{ @int }: 0\nb: 3", "b: 2", "_.b", "_|_ #conflict"),
];

#[test]
fn r2_one_constraint_meets_in_either_order() {
    let mut bad = Vec::new();
    for (label, first, second, read, want) in SAME {
        same_as_one_file(&mut bad, label, &[first, second], read, want);
    }
    // The order the two were written in does not move the root.
    let w = Ws::new("clash-reverse");
    check(&mut bad, "clash reversed", w.steps(&["b: 3", "@{ \"b\" }: 2"]), "");
    check(&mut bad, "clash reversed commit", w.commit_now(), "");
    let reversed = w.root();
    let w = Ws::new("clash-forward");
    check(&mut bad, "clash forward", w.steps(&["@{ \"b\" }: 2", "b: 3"]), "");
    check(&mut bad, "clash forward commit", w.commit_now(), "");
    check(&mut bad, "clash forward root", w.root(), &reversed);
    verdict(bad, SAME.len() * 7 + 5);
}

#[test]
fn r3_every_later_evolve_is_kept() {
    let mut bad = Vec::new();
    same_as_one_file(&mut bad, "three", &[TABLE, "x: 3", "y: 4"], "_.x", "3");
    same_as_one_file(&mut bad, "three-y", &[TABLE, "x: 3", "y: 4"], "_.y", "4");
    same_as_one_file(&mut bad, "four", &[TABLE, "x: 3", "@{ 5 }: 1", "y: 4"], "_.y", "4");
    verdict(bad, 3 * 7);
}

/// Two evolves that never make the working set a table: v0.98.0's roots.
const UNTOUCHED: [(&str, &str, &str, &str); 6] = [
    ("plain", "x: 3", "y: 4", "9f4688480a4a9ce2cf664796eaa3d8073ba7dcc656281718d69ac4f7f4389cc1"),
    ("nested table", "t: { @{ \"b\" }: 2 }", "t.b: 3", "2fa352ca8b18f80ebc86f640b2483072eeda01879cad92402e9071c0f1e9136d"),
    ("key then pattern", "b: 3", "@{ \"b\" }: 2", "b80f5fb08691d64667263b37687392cbafc6c313e3a02360bed1124a4e7d848e"),
    ("spread", "c: { b: 2 }", "r: { a: 1, ...c }", "f6be66e9989ce17b864992b6223061d8503f196a74b26c6311f2921d7e30ab13"),
    ("private", "~p: 3", "y: p", "f8b00ed33b9a22c0118f0f8176b5d89b506db8c45060e56cde7d82f2299cc166"),
    ("recipe", "n: 2", "x: n + 1", "b24cde4e3af92bc6d8d6dadf162bb0fc90027f4785e15883e9416f59dcfcb07d"),
];

#[test]
fn g1_roots_that_never_held_a_table_do_not_move() {
    let mut bad = Vec::new();
    for (label, a, b, root) in UNTOUCHED {
        let w = Ws::new(&format!("g1-{label}"));
        check(&mut bad, &format!("{label}: evolves"), w.steps(&[a, b]), "");
        check(&mut bad, &format!("{label}: commit"), w.commit_now(), "");
        check(&mut bad, &format!("{label}: root"), w.root(), root);
    }
    verdict(bad, UNTOUCHED.len() * 3);
}

/// What the second evolve's member says, when the working set is not a table.
const MEMBERS: [(&str, &str, &str, &str); 3] = [
    ("plain", "x: 3", "y: 4", "{ y: 4 }"),
    ("private", "x: 1", "~p: 3\ny: p", "{ y: { ~%__nlang_thunk: #true ~%__nlang_expr: p ~%__nlang_closure: [] } ~p: 3 }"),
    ("nested table", "t: { @{ \"b\" }: 2 }", "t.b: 3", "{ t: { %morphism: #true %rules: { \"{d53dd013218efa2667fc05d0271345858a6de19a8030731c5f780de3819461ee}\": {{ %bodies: {{ \"{70a0915b4366ca84e66fd2c6b8b5061ff74d5b003fb63f9bd94410d92a774fe2}\": {{ %pattern: \"b\" %val: { ~%__nlang_thunk: #true ~%__nlang_expr: 3 ~%__nlang_closure: [{ ~%__nlang_frame: [0, { b: { ~%__nlang_thunk: #true ~%__nlang_expr: 3 ~%__nlang_closure: [] } }] }] } _: _ }} \"{d493a417b7ba5eeccdfbc6279341cb5c8ea787cbbde39c8f99ef01c9786a07ae}\": {{ %pattern: \"b\" %val: { ~%__nlang_thunk: #true ~%__nlang_expr: 2 ~%__nlang_closure: [{ ~%__nlang_frame: [1, { %morphism: #true %rules: { \"{d53dd013218efa2667fc05d0271345858a6de19a8030731c5f780de3819461ee}\": {{ %pattern: \"b\" %val: { ~%__nlang_thunk: #true ~%__nlang_expr: 2 ~%__nlang_closure: [] } _: _ }} } }] }] } _: _ }} }} %pattern: \"b\" }} } } }"),
];

fn members(w: &Ws) -> Vec<(std::time::SystemTime, String)> {
    let dir = w.ws.join(".oo").join("injections");
    let mut out: Vec<_> = fs::read_dir(&dir)
        .map(|d| d.filter_map(|e| e.ok()).collect::<Vec<_>>())
        .unwrap_or_default()
        .into_iter()
        .map(|e| {
            let t = e.metadata().and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH);
            let body = fs::read_to_string(e.path()).unwrap_or_default();
            let body = body.split_once("\n\n").map(|(_, b)| b).unwrap_or("");
            (t, squash(body))
        })
        .collect();
    out.sort();
    out
}

#[test]
fn g2_members_that_never_held_a_table_do_not_change() {
    let mut bad = Vec::new();
    for (label, a, b, want) in MEMBERS {
        let w = Ws::new(&format!("g2-{label}"));
        check(&mut bad, &format!("{label}: first"), w.steps(&[a]), "");
        let before: Vec<String> = members(&w).into_iter().map(|(_, b)| b).collect();
        check(&mut bad, &format!("{label}: second"), w.steps(&[b]), "");
        let mut after: Vec<String> = members(&w).into_iter().map(|(_, b)| b).collect();
        for m in &before {
            if let Some(i) = after.iter().position(|x| x == m) {
                after.remove(i);
            }
        }
        check(&mut bad, &format!("{label}: member"), after.join(" | "), want);
    }
    verdict(bad, MEMBERS.len() * 3);
}
