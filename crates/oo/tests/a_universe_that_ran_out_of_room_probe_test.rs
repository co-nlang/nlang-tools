// A universe that ran out of room.
// Order: nlang-tools/docs/a_universe_that_ran_out_of_room_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-094.  D113 (2026-10-20): the
// universe's own merge (staging a write, folding members, commit, the
// observation view) is bookkeeping, not an observation; it spends no fuel.
// A coordinate is a horizon only when its own definition is one; what it
// stores and reads does not depend on how many other coordinates there are.
//
// ── What is wrong (v0.95.0; v0.94.0 the same) ───────────────────────────
//
// One evolve of K literal fields `aI: I`:
//   K=1000  ⟹ `a999` is stored as `#blur #fuel_exhausted`
//   K=1200  ⟹ 402 coordinates of the root are horizons; `_.b` of `b: 5`
//              reads one (v0.93.0 reads `5`; v0.92.0 is clean at K=1000)
// 600 aliases `bI: aI` ⟹ 260 horizons; seven root spreads of 200 fields
// ⟹ 408. The universe's merge meets coordinate by coordinate in one
// engine-level context: 10000 fuel, 5 per meet.
//
// These cells are large on purpose. Each takes 10–40 s.
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
        let scratch = nlang_interpreter::ScratchDir::new(&format!("out-of-room-{tag}"));
        let root = scratch.path().to_path_buf();
        let ws = root.join("ws");
        fs::create_dir_all(&ws).unwrap();
        Ws { _scratch: scratch, root, ws }
    }

    /// Run `oo`, killing it after 180 s: (output, rc).
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
            if start.elapsed() > Duration::from_secs(180) {
                let _ = child.kill();
                let _ = child.wait();
                return (format!("TIMEOUT after 180 s: oo {}", args.join(" ")), -1);
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let o = child.wait_with_output().expect("output");
        (
            format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)).trim().to_string(),
            o.status.code().unwrap_or(-1),
        )
    }

    /// Evolve `src` from a new file, then commit: "" when both succeed.
    fn commit(&self, src: &str) -> String {
        let n = fs::read_dir(&self.ws).map(|d| d.count()).unwrap_or(0);
        let f = format!("f{n}.n");
        fs::write(self.ws.join(&f), src).unwrap();
        let (o, rc) = self.oo(&["evolve", &f]);
        if rc != 0 {
            return format!("evolve rc={rc}: {}", squash(&o).chars().take(200).collect::<String>());
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

fn literals(k: usize) -> String {
    (0..k).map(|i| format!("a{i}: {i}\n")).collect()
}

// ── Red ──────────────────────────────────────────────────────────────────

/// r1 — A thousand literal fields in one evolve all read as themselves.
/// v0.95.0: `a999` reads `#blur { %cause: #fuel_exhausted … }`.
#[test]
fn r1_the_thousandth_literal_is_itself() {
    let mut bad = Vec::new();
    let w = Ws::new("r1");
    check(&mut bad, "commit 1000 literals", w.commit(&literals(1000)), "");
    for (e, want) in [("_.a0", "0"), ("_.a998", "998"), ("_.a999", "999")] {
        check(&mut bad, e, w.eval(e), want);
    }
    verdict(bad, 4);
}

/// r2 — Twelve hundred literals and one more: every coordinate reads as
/// itself, in one evolve and when the last one is written later.
/// v0.95.0: `_.b` and `_.a1199` read a horizon; the root holds 402.
#[test]
fn r2_one_more_literal_after_twelve_hundred() {
    let mut bad = Vec::new();
    let w = Ws::new("r2-one");
    check(&mut bad, "commit 1200 literals and b", w.commit(&format!("{}b: 5\n", literals(1200))), "");
    for (e, want) in [("_.b", "5"), ("_.a1199", "1199"), ("_.a800", "800")] {
        check(&mut bad, &format!("one evolve: {e}"), w.eval(e), want);
    }
    let w = Ws::new("r2-later");
    check(&mut bad, "commit 1200 literals", w.commit(&literals(1200)), "");
    check(&mut bad, "then b", w.commit("b: 5\n"), "");
    for (e, want) in [("_.b", "5"), ("_.a1199", "1199")] {
        check(&mut bad, &format!("b later: {e}"), w.eval(e), want);
    }
    verdict(bad, 9);
}

/// r3 — Readings: six hundred aliases read what they name; seven root
/// spreads of two hundred fields read their sources. v0.95.0: 260 and 408
/// horizons.
#[test]
fn r3_readings_in_a_large_universe() {
    let mut bad = Vec::new();
    let w = Ws::new("r3-alias");
    let src: String = (0..600).map(|i| format!("a{i}: {i}\n")).chain((0..600).map(|i| format!("b{i}: a{i}\n"))).collect();
    check(&mut bad, "commit 600 aliases", w.commit(&src), "");
    for (e, want) in [("_.b0", "0"), ("_.b400", "400"), ("_.b599", "599")] {
        check(&mut bad, e, w.eval(e), want);
    }
    let w = Ws::new("r3-spread");
    let mut src = String::new();
    for j in 0..7 {
        let fields: Vec<String> = (0..200).map(|i| format!("f{j}_{i}: {i}")).collect();
        src.push_str(&format!("c{j}: {{ {} }}\n", fields.join(", ")));
    }
    for j in 0..7 {
        src.push_str(&format!("...c{j}\n"));
    }
    check(&mut bad, "commit seven spreads", w.commit(&src), "");
    for (e, want) in [("_.f0_0", "0"), ("_.f4_92", "92"), ("_.f6_199", "199")] {
        check(&mut bad, e, w.eval(e), want);
    }
    verdict(bad, 8);
}

/// r4 — Past two thousand: every meet below the roots is the universe's
/// too. A first reference that took only the outer merge off the meter left
/// `a2000` onward as `#fuel_exhausted` (10000 fuel / 5 per meet); r1–r3 did
/// not see it. v0.95.0: a horizon (or the 180 s cap).
#[test]
fn r4_past_two_thousand() {
    let mut bad = Vec::new();
    let w = Ws::new("r4");
    check(&mut bad, "commit 2100 literals", w.commit(&literals(2100)), "");
    for (e, want) in [("_.a1999", "1999"), ("_.a2000", "2000"), ("_.a2099", "2099")] {
        check(&mut bad, e, w.eval(e), want);
    }
    verdict(bad, 4);
}

// ── Guards (green today, must stay green) ─────────────────────────────────

/// g1 — A definition that hits a horizon by itself still stores it: a chain
/// of 200 `+` in one record is `#max_depth_exceeded`, root unchanged. D113
/// takes the merge off the meter, not the definitions.
#[test]
fn g1_a_definition_of_its_own_horizon_keeps_it() {
    let mut bad = Vec::new();
    let w = Ws::new("g1");
    let chain: Vec<String> = (1..=200).map(|i| format!("a{i}: a{} + 1", i - 1)).collect();
    check(&mut bad, "commit", w.commit(&format!("x: {{ a0: 1, {} }}.a200\n", chain.join(", "))), "");
    check(&mut bad, "root", w.root(), "76f95246e0b7d99197b06d1c8d0d13367bfca44d0da97e48cebb708f541cf212");
    let x = w.eval("_.x");
    check(&mut bad, "_.x", x.starts_with("#blur { %cause: #max_depth_exceeded").to_string(), "true");
    verdict(bad, 3);
}

/// g2 — Small universes keep every byte.
#[test]
fn g2_small_roots_do_not_move() {
    let mut bad = Vec::new();
    let cases: [(String, &str); 3] = [
        ("x: 0\n".to_string(), "b74d775816f6b5be6cc152577b7556f6c8fdcc63433b0e31068cc2484a94ae0f"),
        (literals(100), "d1df8cf8a5ef3e5b697c4863e6dd04b9d9a8d028d19b90124e826ac1950c3d04"),
        ("c: { b: 2 }\n...c\n".to_string(), "5d15b3a675337d6649a2790e5965f9de6644fba7590bf89544e5436542680d43"),
    ];
    for (i, (src, want)) in cases.iter().enumerate() {
        let w = Ws::new(&format!("g2-{i}"));
        check(&mut bad, &format!("commit {i}"), w.commit(src), "");
        check(&mut bad, &format!("root {i}"), w.root(), want);
    }
    verdict(bad, 6);
}

// ── Added at acceptance (2026-10-20), repair round R-1 ────────────────────
//
// Delivery ea174fa equals q094-ref. Three of the universe's own meets do not
// go through `unify_universe` and still spend the shared fuel: refine's
// check, a `~%Config` write staged over what is there, and a spread landed
// over keys already staged. Whether a refinement is accepted, whether a
// coordinate stores itself, whether a clash is refused, depends on how many
// other coordinates there are.

/// The full CAID on the `root:` line of HEAD.
fn root_caid(w: &Ws) -> String {
    let head = fs::read_to_string(w.ws.join(".oo").join("HEAD")).unwrap_or_default();
    let (o, _) = w.oo(&["inspect", head.trim()]);
    o.lines()
        .find_map(|l| l.split_once("root:").map(|(_, r)| r.trim().to_string()))
        .unwrap_or_default()
}

/// r5 — refine's monotonicity check is the universe's meet of two roots. A
/// refinement by one more coordinate is accepted at 2100 coordinates as at
/// ten; the other way round is refused at both. Delivery: accepted at 1990,
/// refused as `new ⋢ old` from 2000 on (v0.92.0 the same).
#[test]
fn r5_refine_does_not_count_the_coordinates() {
    let mut bad = Vec::new();
    for k in [10, 2100] {
        let w = Ws::new(&format!("r5-{k}"));
        check(&mut bad, &format!("{k}: commit old"), w.commit(&literals(k)), "");
        let old = root_caid(&w);
        check(&mut bad, &format!("{k}: commit new"), w.commit("b: 5\n"), "");
        let new = root_caid(&w);
        let (o, rc) = w.oo(&["refine", "-s", &new, "-t", &old, "-m", "back"]);
        check(&mut bad, &format!("{k}: old onto new: rc"), rc.to_string(), "1");
        check(&mut bad, &format!("{k}: old onto new: refused as such"), o.contains("refinement fails geometric monotonicity").to_string(), "true");
        let (o, rc) = w.oo(&["refine", "-s", &old, "-t", &new, "-m", "forth"]);
        check(&mut bad, &format!("{k}: new onto old: {}", squash(&o).chars().take(80).collect::<String>()), rc.to_string(), "0");
    }
    verdict(bad, 10);
}

/// r6 — A `~%Config` write staged over 2100 coordinates leaves every one of
/// them itself, and the root is the root of the literals alone (`~%Config`
/// is session state). Delivery: `a2000` onward stored as `#fuel_exhausted`.
#[test]
fn r6_a_config_write_over_a_large_staging() {
    let mut bad = Vec::new();
    let w = Ws::new("r6");
    check(&mut bad, "commit", w.commit(&format!("{}~%Config.fuel: 10000\n", literals(2100))), "");
    check(&mut bad, "root", w.root(), "47a731009320466efc60440b8f0406dec948f20aa8df4a3de80d3de159a5992d");
    for (e, want) in [("_.a1999", "1999"), ("_.a2000", "2000"), ("_.a2099", "2099")] {
        check(&mut bad, e, w.eval(e), want);
    }
    verdict(bad, 5);
}

/// r7 — A spread landed over keys already staged meets each key as the
/// universe's bookkeeping. With the staged fuel at 1000, a spread of 300
/// agreeing keys over 300 literals leaves them themselves; a clash at the
/// 251st key is refused by key, as one at the 6th is. Delivery: `a199`
/// onward stored as horizons, and the clash at `a250` committed.
#[test]
fn r7_a_spread_over_what_is_staged() {
    let mut bad = Vec::new();
    let lits: String = (0..300).map(|i| format!("a{i}: {i}\n")).collect();
    let src = |clash: Option<usize>| {
        let fields: Vec<String> = (0..300).map(|i| format!("a{i}: {}", if Some(i) == clash { 7 } else { i })).collect();
        format!("~%Config.fuel: 1000\n{lits}c: {{ {} }}\n...c\n", fields.join(", "))
    };
    let w = Ws::new("r7-agree");
    check(&mut bad, "agree: commit", w.commit(&src(None)), "");
    for (e, want) in [("_.a0", "0"), ("_.a199", "199"), ("_.a299", "299")] {
        check(&mut bad, &format!("agree: {e}"), w.eval(e), want);
    }
    for at in [5, 250] {
        let w = Ws::new(&format!("r7-clash{at}"));
        let o = w.commit(&src(Some(at)));
        check(&mut bad, &format!("clash at a{at}: evolve refused"), o.starts_with("evolve rc=1").to_string(), "true");
        check(&mut bad, &format!("clash at a{at}: names the key"), o.contains(&format!("#conflict at a{at}")).to_string(), "true");
        check(&mut bad, &format!("clash at a{at}: no HEAD"), w.root(), "");
    }
    verdict(bad, 10);
}

// ── Added at acceptance of R-1 (2026-10-20) ───────────────────────────────

/// r8 — A source that lands later is checked for agreement with what its
/// keys hold as the universe's bookkeeping. With the staged fuel at 1000, a
/// later `c` whose 251st key disagrees is refused by key, as one whose 6th
/// does; one that agrees lands, and a later write keeps it. Reference
/// q094-r1ref 07cb5c9: the clash at `a250` commits (`_.c.a250` 7 beside
/// `_.a250` 250). The delivery found this one.
#[test]
fn r8_a_later_source_is_checked_without_counting() {
    let mut bad = Vec::new();
    let fuel = "~%Config.fuel: 1000\n";
    let lits: String = (0..300).map(|i| format!("a{i}: {i}\n")).collect();
    let c = |clash: Option<usize>| {
        let fields: Vec<String> = (0..300).map(|i| format!("a{i}: {}", if Some(i) == clash { 7 } else { i })).collect();
        format!("{fuel}c: {{ {} }}\n", fields.join(", "))
    };
    for at in [5, 250] {
        let w = Ws::new(&format!("r8-clash{at}"));
        check(&mut bad, &format!("clash at a{at}: seed"), w.commit(&format!("{fuel}{lits}...c\n")), "");
        let seed = w.root();
        let o = w.commit(&c(Some(at)));
        check(&mut bad, &format!("clash at a{at}: evolve refused"), o.starts_with("evolve rc=1").to_string(), "true");
        check(&mut bad, &format!("clash at a{at}: names the key"), o.contains(&format!("#conflict at a{at}")).to_string(), "true");
        check(&mut bad, &format!("clash at a{at}: root"), (w.root() == seed).to_string(), "true");
    }
    let w = Ws::new("r8-agree");
    check(&mut bad, "agree: seed", w.commit(&format!("{fuel}{lits}...c\n")), "");
    check(&mut bad, "agree: c", w.commit(&c(None)), "");
    check(&mut bad, "agree: x", w.commit("x: 1\n"), "");
    check(&mut bad, "agree: root", w.root(), "a77efa7c31e4ebd35a151b4f86c5cd9fcfba4335ee304199fbc14d1e7b7ce0d2");
    for (e, want) in [("_.a250", "250"), ("_.c.a250", "250"), ("_.x", "1")] {
        check(&mut bad, &format!("agree: {e}"), w.eval(e), want);
    }
    verdict(bad, 15);
}
