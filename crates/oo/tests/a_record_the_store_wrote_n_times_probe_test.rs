// A record the store wrote N times.
// Order: nlang-tools/docs/a_record_the_store_wrote_n_times_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-097.  D114 (2026-10-21): in the
// durable form a closure frame is written once and shared by reference, as
// it is in memory; stores declare encoding 8; an encoding-7 store keeps the
// inline form until `oo migrate`; identity bytes and addresses do not move.
//
// ── What is wrong (v0.36.0 through v0.97.0) ─────────────────────────────
//
// Every field of a record is a thunk in the working set and savepoints, and
// every thunk's `~%__nlang_closure` writes the whole record's frame again.
// A record of N fields is N² bytes: 400 literal fields make a 12 MB member
// and a 1.8 GB commit; a root holding 200 readings `xI: d.b` is a 3 MB file.
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
        let scratch = nlang_interpreter::ScratchDir::new(&format!("written-n-times-{tag}"));
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


fn record(n: usize, prefix: &str) -> String {
    let fields: Vec<String> = (0..n).map(|i| format!("{prefix}{i}: {i}")).collect();
    format!("{{ {} }}", fields.join(", "))
}

/// Bytes under `.oo/<dir>`, and whether any file there holds `needle`.
fn bytes_under(w: &Ws, dir: &str, needle: &str) -> (u64, bool) {
    fn walk(p: &std::path::Path, needle: &str, n: &mut u64, hit: &mut bool) {
        if let Ok(rd) = fs::read_dir(p) {
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, needle, n, hit);
                } else if let Ok(b) = fs::read(&p) {
                    *n += b.len() as u64;
                    if !needle.is_empty() && String::from_utf8_lossy(&b).contains(needle) {
                        *hit = true;
                    }
                }
            }
        }
    }
    let (mut n, mut hit) = (0, false);
    walk(&w.ws.join(".oo").join(dir), needle, &mut n, &mut hit);
    (n, hit)
}

fn root_file_bytes(w: &Ws) -> u64 {
    let r = w.root();
    if r.len() < 3 {
        return 0;
    }
    fs::metadata(w.ws.join(".oo").join("objects").join("sha256").join(&r[..2]).join(&r[2..])).map(|m| m.len()).unwrap_or(0)
}

// ── Red ──────────────────────────────────────────────────────────────────

/// r1 — A record of 400 literal fields is written once, not 400 times: the
/// working-set member under 200 KB and the savepoints under 300 KB after
/// one evolve (v0.97.0: 11.9 MB and 14.9 MB). Two disjoint records of 400
/// fields at one coordinate commit and read both. The answers do not move.
#[test]
fn r1_a_record_is_written_once() {
    let mut bad = Vec::new();
    let w = Ws::new("r1-one");
    let (rc, _) = w.evolve(&format!("r: {}\n", record(400, "f")));
    check(&mut bad, "evolve 400", rc.to_string(), "0");
    let (m, _) = bytes_under(&w, "injections", "");
    let (s, _) = bytes_under(&w, "savepoints", "");
    check(&mut bad, &format!("member bytes {m} under 200000"), (m < 200_000).to_string(), "true");
    check(&mut bad, &format!("savepoint bytes {s} under 300000"), (s < 300_000).to_string(), "true");
    let (o, rc) = w.oo(&["commit", "-m", "a"]);
    check(&mut bad, &format!("commit: {}", squash(&o).chars().take(60).collect::<String>()), rc.to_string(), "0");
    check(&mut bad, "_.r.f399", w.eval("_.r.f399"), "399");
    let w = Ws::new("r1-two");
    check(&mut bad, "two records", w.commit(&format!("r: {}\nr: {}\n", record(400, "f"), record(400, "g"))), "");
    check(&mut bad, "_.r.f399", w.eval("_.r.f399"), "399");
    check(&mut bad, "_.r.g399", w.eval("_.r.g399"), "399");
    verdict(bad, 8);
}

/// r2 — A root holding a record of 200 readings `xI: d.b`: the same address
/// as v0.97.0, a root file under 100 KB (v0.97.0: 2993617 bytes).
#[test]
fn r2_a_reading_record_keeps_its_address_in_fewer_bytes() {
    let mut bad = Vec::new();
    let w = Ws::new("r2");
    let fields: Vec<String> = (0..200).map(|i| format!("x{i}: d.b")).collect();
    check(&mut bad, "commit", w.commit(&format!("d: {{ b: 3 }}\nq: {{ {} }}\n", fields.join(", "))), "");
    check(&mut bad, "root", w.root(), "60d3f35a5b033490b1f0475e468ce324b28d444bdd793528b9104c4e4a8230f0");
    let b = root_file_bytes(&w);
    check(&mut bad, &format!("root file {b} bytes under 100000"), (b > 0 && b < 100_000).to_string(), "true");
    check(&mut bad, "_.q.x199", w.eval("_.q.x199"), "3");
    verdict(bad, 4);
}

/// r3 — New stores declare encoding 8. A store that declares encoding 7
/// keeps the inline form (no `~%__nlang_frame` written) and reads as before;
/// `oo migrate` names the engines it locks out, moves it to 8, and after it
/// shared frames are written and both old and new read.
#[test]
fn r3_an_old_store_keeps_its_form_until_migrate() {
    let mut bad = Vec::new();
    let w = Ws::new("r3-new");
    check(&mut bad, "new store", w.commit("r: { y: 1, z: y }\n"), "");
    let decl = fs::read_to_string(w.ws.join(".oo").join("objects.format")).unwrap_or_default();
    check(&mut bad, "new store declares", decl.trim().to_string(), "encoding=9"); // AMENDED 2026-10-22 for Q-099 (D115): encoding 9
    let (_, shared) = bytes_under(&w, "", "__nlang_frame");
    check(&mut bad, "new store shares frames", shared.to_string(), "true");

    let w = Ws::new("r3-old");
    let oo = w.ws.join(".oo");
    fs::create_dir_all(&oo).unwrap();
    fs::write(oo.join("objects.format"), "encoding=7\n").unwrap();
    fs::write(oo.join("format"), "layout=9\n").unwrap();
    check(&mut bad, "old store: commit", w.commit("r: { y: 1, z: y }\n"), "");
    let (_, shared) = bytes_under(&w, "", "__nlang_frame");
    check(&mut bad, "old store writes inline", shared.to_string(), "false");
    check(&mut bad, "old store: _.r.z", w.eval("_.r.z"), "1");
    let (o, rc) = w.oo(&["migrate", "--grant", "migrate"]);
    check(&mut bad, "migrate rc", rc.to_string(), "0");
    check(&mut bad, "migrate names the lockout", o.contains("v0.95.0 through v0.99.0").to_string(), "true"); // AMENDED 2026-10-22 for Q-099 (D115): encoding 9
    let decl = fs::read_to_string(oo.join("objects.format")).unwrap_or_default();
    check(&mut bad, "migrated declares", decl.trim().to_string(), "encoding=9"); // AMENDED 2026-10-22 for Q-099 (D115): encoding 9
    check(&mut bad, "after migrate: commit", w.commit("u: { y: 2, w: y }\n"), "");
    let (_, shared) = bytes_under(&w, "", "__nlang_frame");
    check(&mut bad, "after migrate shares frames", shared.to_string(), "true");
    check(&mut bad, "after migrate: _.r.z", w.eval("_.r.z"), "1");
    check(&mut bad, "after migrate: _.u.w", w.eval("_.u.w"), "2");
    verdict(bad, 13);
}

// ── Guards (green today, must stay green) ─────────────────────────────────

/// g1 — Addresses do not move (D114 Q2): five roots as v0.97.0 gives them,
/// one of them holding a reference that sorts before its frame's definition.
#[test]
fn g1_addresses_do_not_move() {
    let mut bad = Vec::new();
    let readings = |n: usize| {
        let f: Vec<String> = (0..n).map(|i| format!("x{i}: d.b")).collect();
        format!("d: {{ b: 3 }}\nr: {{ {} }}\n", f.join(", "))
    };
    let chain = {
        let f: Vec<String> = (1..50).map(|i| format!("f{i}: f{}", i - 1)).collect();
        format!("r: {{ f0: 0, {} }}\n", f.join(", "))
    };
    let cases = [
        ("lit 50", format!("r: {}\n", record(50, "f")), "b8d6b5ce6e077004e1076117d72e9f1f808af34f787d4f27fe39c9e0876856bd"),
        ("readings 50", readings(50), "2ea74a11c8cbd90a62c021f918c80668693a65f9c6b7ee5da75a53f1ae7bc452"),
        ("chain 50", chain, "336a3085d16e367fd059a150d568b6522ec819ce5338cf4e77cbc88b636b2728"),
        ("mixed", "d: { b: 3 }\nc: { b: 3, k: 1 }\n...c\nr: { y: 1, z: y, w: d.b, f: (x -> x + y) }\nq: r.z + 1\n".to_string(), "0e3d2ca3313acda02873703785fc4c90bd5cc5f779c0972fc0e09cf7b7224b88"),
        ("readings 200", readings(200), "3dd138529716114bf5fb953de2eb6a358348de32b6c5b0889b7a2b07358bffbd"),
    ];
    for (name, src, root) in cases {
        let w = Ws::new(&format!("g1-{}", name.replace(' ', "-")));
        check(&mut bad, &format!("{name}: commit"), w.commit(&src), "");
        check(&mut bad, &format!("{name}: root"), w.root(), root);
    }
    verdict(bad, 10);
}

/// g2 — Reading is as before (a pruned frame changes these): a chain of
/// sibling readings reads through; a sibling is read in its own literal
/// only; a sibling written later is taken and the reading does not see it.
#[test]
fn g2_readings_are_as_before() {
    let mut bad = Vec::new();
    let w = Ws::new("g2-chain");
    let f: Vec<String> = (1..200).map(|i| format!("f{i}: f{}", i - 1)).collect();
    check(&mut bad, "chain: commit", w.commit(&format!("r: {{ f0: 0, {} }}\n", f.join(", "))), "");
    check(&mut bad, "chain: _.r.f199", w.eval("_.r.f199"), "0");
    let w = Ws::new("g2-literal");
    check(&mut bad, "in place: commit", w.commit("r: { z: y } & { y: 5 }\n"), "");
    check(&mut bad, "in place: _.r.z", w.eval("_.r.z"), "_");
    let w = Ws::new("g2-later");
    check(&mut bad, "later: seed", w.commit("r: { z: y }\n"), "");
    check(&mut bad, "later: sibling", w.commit("r: { y: 5 }\n"), "");
    check(&mut bad, "later: _.r.y", w.eval("_.r.y"), "5");
    check(&mut bad, "later: _.r.z", w.eval("_.r.z"), "_");
    let w = Ws::new("g2-dotted");
    check(&mut bad, "dotted: seed", w.commit("r: { z: y }\n"), "");
    check(&mut bad, "dotted: r.y", w.commit("r.y: 5\n"), "");
    let w = Ws::new("g2-coord");
    check(&mut bad, "coordinate agree: seed", w.commit("d: { b: 3 }\nr: { b: d.b }\n"), "");
    check(&mut bad, "coordinate agree: spread", w.commit("c: { b: 3 }\nr: { ...c }\n"), "");
    check(&mut bad, "coordinate agree: _.r.b", w.eval("_.r.b"), "3");
    verdict(bad, 13);
}

// ── Added at acceptance (2026-10-21), repair round R-1 ────────────────────
//
// Delivery eda60b3 equals q097-ref 287fcff. Whether frames are shared was
// set wherever a store was opened, so `~%Discovery./connect` to a local
// peer decided the form this universe's working set is written in: an
// encoding-7 universe connected to an encoding-8 peer writes shared frames
// into its own member and savepoint (v0.97.0 then fails to commit them:
// "thunk frame is not a combo"), and an encoding-8 universe connected to an
// encoding-7 peer writes inline.

/// r4 — The form a universe writes is its own store's, whatever else the
/// same evolve opens. Own encoding 7 + a peer of 8: no shared frame in the
/// universe; own 8 + a peer of 7: shared frames as always.
#[test]
fn r4_a_peer_does_not_choose_the_form() {
    let mut bad = Vec::new();
    for (own, peer, want) in [("7", "8", "false"), ("8", "7", "true")] {
        let w = Ws::new(&format!("r4-{own}-{peer}"));
        let p = w.root.join("peer");
        for (dir, enc) in [(w.ws.join(".oo"), own), (p.join(".oo"), peer)] {
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join("objects.format"), format!("encoding={enc}\n")).unwrap();
            fs::write(dir.join("format"), "layout=9\n").unwrap();
        }
        let src = format!("p: ~%Discovery./connect(\"pp\", \"{}\")\nr: {{ y: 1, z: y }}\n", p.display());
        let (rc, _) = w.evolve(&src);
        check(&mut bad, &format!("own {own}, peer {peer}: evolve"), rc.to_string(), "0");
        let (_, shared) = bytes_under(&w, "", "__nlang_frame");
        check(&mut bad, &format!("own {own}, peer {peer}: universe shares frames"), shared.to_string(), want);
        let (o, rc) = w.oo(&["commit", "-m", "a"]);
        check(&mut bad, &format!("own {own}, peer {peer}: commit {}", squash(&o).chars().take(60).collect::<String>()), rc.to_string(), "0");
        check(&mut bad, &format!("own {own}, peer {peer}: _.r.z"), w.eval("_.r.z"), "1");
    }
    verdict(bad, 8);
}

/// r5 — Reading a root by address in the same evolve does not change the
/// form either. `ObjectStore::get_value` builds an in-memory engine for a
/// root; its fresh ephemeral store is encoding 8. Own encoding 7: `q: _{…}.x`
/// beside a record writes no shared frame; own 8: shared as always. The
/// acceptor's first R-1 reference set the form there too and failed this.
#[test]
fn r5_reading_a_root_by_address_does_not_choose_the_form() {
    let mut bad = Vec::new();
    for (own, want) in [("7", "false"), ("8", "true")] {
        let w = Ws::new(&format!("r5-{own}"));
        let oo = w.ws.join(".oo");
        fs::create_dir_all(&oo).unwrap();
        fs::write(oo.join("objects.format"), format!("encoding={own}\n")).unwrap();
        fs::write(oo.join("format"), "layout=9\n").unwrap();
        check(&mut bad, &format!("own {own}: seed"), w.commit("x: 1\n"), "");
        let root = w.root();
        let (rc, _) = w.evolve(&format!("q: _{{sha256:{root}}}.x\nr: {{ y: 1, z: y }}\n"));
        check(&mut bad, &format!("own {own}: evolve"), rc.to_string(), "0");
        let (_, shared) = bytes_under(&w, "", "__nlang_frame");
        check(&mut bad, &format!("own {own}: universe shares frames"), shared.to_string(), want);
        let (o, rc) = w.oo(&["commit", "-m", "b"]);
        check(&mut bad, &format!("own {own}: commit {}", squash(&o).chars().take(60).collect::<String>()), rc.to_string(), "0");
        check(&mut bad, &format!("own {own}: _.q"), w.eval("_.q"), "1");
    }
    verdict(bad, 10);
}
