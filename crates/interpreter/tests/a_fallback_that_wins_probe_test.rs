// A fallback that wins (Q-027, pre-committed by work order:
// docs/a_fallback_that_wins_handover.md).
//
// ── The claim ────────────────────────────────────────────────────────────
//
// When the engine cannot tell what a piece of data is, that data must not win
// because of it.
//
// Measured 2026-08-16 on v0.23.1: every field of a durable peer record is
// parsed with the same idiom, `…and_then(as_T).unwrap_or(X)`, and the same
// idiom falls in opposite directions depending on what the value happens to
// mean in that field:
//
//   ttl        -> 0   => ttl == 0 means "do not forward"          LOSES  (safe)
//   services   -> []  => matches no service query                 LOSES
//   provenance -> Unknown, with a comment "never promote to direct" LOSES
//   ts         -> 0   => epoch, and received_at falls back to ts   WINS   (bug)
//   received_at-> ts  => the PRIMARY sort key                      WINS   (bug)
//   admission_seq -> 0 => the SECONDARY sort key, ahead of all     WINS   (bug)
//
// `provenance` is the only one anyone chose the direction for; it says so in
// a comment. The rest is the same idiom meeting different field semantics,
// and which way it falls is a coincidence. REAL_02 §4.3.5.1 has the phrase
// for this: an accidental property is not a property.
//
// Measured behaviourally: five candidates, three seats, identical
// received_at, differing only in admission_seq — one absent, one unparseable
// ("abc"), three with real numbers 6/7/8. Seated: [absent, unparseable, 6].
// The record that cannot say when it arrived took a seat ahead of two that
// can. REAL_02 §5.1.2 forbids exactly that.
//
// ── Ruling A (user, 2026-08-16) ──────────────────────────────────────────
//
// Present-but-unparseable sorts LAST, and the record is still kept. The
// signed half may be perfectly intact; what is damaged is our own asserted
// half, and a peer should not vanish because our bookkeeping broke.
//
// ── What this file cannot witness ────────────────────────────────────────
//
// Nothing here reaches the entropy-failure path (§3.2/§3.3) — SystemRandom
// does not fail on demand. That half needs an injectable source and a unit
// test the delivery supplies. Four greens here do NOT mean this arc holds.
//
// ── Probe integrity ──────────────────────────────────────────────────────
//
// Reds are `#[ignore]`d and MUST be red at the baseline for the reason stated
// in each. The delivery may remove `#[ignore]` and nothing else in this file.
// C0 runs first: an assertion about load order is vacuous if nothing loaded.
//
// No signatures, processes or ports: `peers::load` does not verify signatures
// (that is `verify_loaded`, a separate step), so a directory can be written
// by hand and handed to the real parser and the real comparator.

use nlang_interpreter::peers;
use nlang_interpreter::PeerAdvert;

const OWNER: &str = "hash:sha256:v1:0000000000000000000000000000000000000000000000000000000000000001";

/// One durable record. `seq` and `recv` are written verbatim into the JSON,
/// so a test can put a string, a null, or nothing at all where a number
/// belongs.
fn record(tag: &str, recv: &str, seq: &str) -> String {
    let node_id = format!("hash:sha256:v1:{tag:0>64}");
    let mut fields = vec![
        format!("\"ad\":\"{{{{ node_id: \\\"{node_id}\\\" }}}}\""),
        format!("\"node_id\":\"{node_id}\""),
        format!("\"public_key\":\"{:0>64}\"", tag),
        "\"services\":[]".to_string(),
        "\"listen_port\":9000".to_string(),
        "\"capacity\":10".to_string(),
        "\"ts\":1700000000".to_string(),
        "\"ttl\":15".to_string(),
        "\"observed_host\":\"127.0.0.1\"".to_string(),
        "\"hops\":0".to_string(),
        "\"addr\":\"127.0.0.1:9000\"".to_string(),
        "\"provenance\":\"direct\"".to_string(),
    ];
    if !recv.is_empty() {
        fields.push(format!("\"received_at\":{recv}"));
    }
    if !seq.is_empty() {
        fields.push(format!("\"admission_seq\":{seq}"));
    }
    format!("{{{}}}", fields.join(","))
}

/// Write a directory owned by OWNER, load it, and return the records in the
/// engine's own admission order.
fn loaded_in_order(tag: &str, records: &[String]) -> Vec<PeerAdvert> {
    let dir = nlang_interpreter::ScratchDir::new(&format!("fallback-{tag}"));
    let path = peers::directory_path(&dir);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut text = format!("# {} node_id={OWNER}\n", peers::FORMAT_TAG);
    for r in records {
        text.push_str(r);
        text.push('\n');
    }
    std::fs::write(&path, text).unwrap();

    let (by_id, _, _, _) = peers::load(&dir, Some(OWNER));
    let mut v: Vec<PeerAdvert> = by_id.into_values().collect();
    v.sort_by(|a, b| peers::cmp_admission_order(a, b, Some(OWNER)));
    v
}

/// The `tag` each record was built with, in load order.
fn tags(v: &[PeerAdvert]) -> Vec<String> {
    v.iter()
        .map(|a| a.node_id.rsplit(':').next().unwrap().trim_start_matches('0').to_string())
        .collect()
}

// ── C0 — control, runs first ─────────────────────────────────────────────

#[test]
fn c0_parseable_sequence_numbers_sort_in_order() {
    let v = loaded_in_order(
        "c0",
        &[
            record("3", "1700000000", "8"),
            record("1", "1700000000", "6"),
            record("2", "1700000000", "7"),
        ],
    );
    assert_eq!(
        v.len(),
        3,
        "control failed: a hand-written directory did not load. \
         Every probe in this file is vacuous until this passes."
    );
    assert_eq!(
        tags(&v),
        vec!["1", "2", "3"],
        "control failed: parseable sequence numbers did not sort by value"
    );
}

// ── C1 — control: legacy records must not be demoted by this arc ─────────
// REAL_02 §5.1.2: absent means written by an older engine, and MAY sort
// first. Only *unparseable* loses that.

#[test]
fn c1_an_absent_sequence_number_still_sorts_first() {
    let v = loaded_in_order(
        "c1",
        &[
            record("2", "1700000000", "6"),
            record("1", "1700000000", ""), // absent — the v0.9.0 shape
            record("3", "1700000000", "7"),
        ],
    );
    assert_eq!(v.len(), 3, "C0 must pass first");
    assert_eq!(
        tags(&v)[0],
        "1",
        "an absent admission_seq is a legacy record and may still sort first; \
         got {:?}",
        tags(&v)
    );
}

// ── P1 — red: unparseable must not inherit the legacy priority ───────────
// Baseline red: `.and_then(as_u64).unwrap_or(0)` gives absent and
// unparseable the same 0, and 0 sorts ahead of every real number.

#[test]
#[ignore = "Q-027: absent and unparseable admission_seq both fall to 0 (work order §3.1)"]
fn p1_an_unparseable_sequence_number_sorts_last() {
    let v = loaded_in_order(
        "p1",
        &[
            record("2", "1700000000", "6"),
            record("9", "1700000000", "\"abc\""), // present, unparseable
            record("3", "1700000000", "7"),
        ],
    );
    assert_eq!(v.len(), 3, "C0 must pass first");
    assert_eq!(
        tags(&v).last().map(String::as_str),
        Some("9"),
        "a record that cannot say when it arrived must sort after every record \
         that can. Order was {:?}",
        tags(&v)
    );
}

// ── P2 — red: the same for the primary key ───────────────────────────────
// Baseline red: `received_at` falls back to `ts`, so an unparseable arrival
// time silently becomes a real, early one.

#[test]
#[ignore = "Q-027: unparseable received_at falls back to ts (work order §3.1)"]
fn p2_an_unparseable_arrival_time_sorts_last() {
    let v = loaded_in_order(
        "p2",
        &[
            record("2", "1700000005", "6"),
            record("9", "null", "7"), // present, unparseable
            record("3", "1700000009", "8"),
        ],
    );
    assert_eq!(v.len(), 3, "C0 must pass first");
    assert_eq!(
        tags(&v).last().map(String::as_str),
        Some("9"),
        "an unparseable received_at must not become an early one. Order was {:?}",
        tags(&v)
    );
}

// ── P3 — the other half of ruling A: demoted, not dropped ────────────────
// P1 and P2 are both satisfied by throwing the damaged record away, and that
// would violate the ruling: the signed half may be intact, and a peer must
// not vanish because our own bookkeeping broke. Green today and must stay
// green — it constrains the *fix*, not the current behaviour.

#[test]
fn p3_a_record_with_an_unparseable_field_is_still_kept() {
    let v = loaded_in_order(
        "p3",
        &[
            record("2", "1700000000", "6"),
            record("9", "1700000000", "\"abc\""),
            record("8", "null", "7"),
        ],
    );
    assert_eq!(
        v.len(),
        3,
        "a record whose ordering field is unparseable must be demoted, not \
         dropped (ruling A). Kept: {:?}",
        tags(&v)
    );
    let t = tags(&v);
    assert!(
        t.contains(&"9".to_string()) && t.contains(&"8".to_string()),
        "both damaged records must still be present; got {t:?}"
    );
}
