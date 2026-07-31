# 直接觀察來源性 / Direct observation provenance — work order

**Opened 2026-07-31.** **Baseline:** `nlang-tools` branch `dev`, HEAD
`62e6cd55fa06170c7c45fccf0c1153617b3f38a3` (`tie-back v0.7.1 (top
13d0899; content already on dev)`). **Classification:** receiver-local observation
state; no release classification is made at order opening because this arc does
not change the wire, signed bytes, language surface, or release metadata.

The acceptor-owned probe is
`crates/oo/tests/direct_observation_provenance_probe_test.rs`. It was created
before this handover and calibrated against the current engine. It contains two
live controls and nine ignored reds. The delivery boundary is removal of those
nine `#[ignore]` attributes only.

The workspace run immediately before the probe was added was **1721 passed, 0
failed, 3 ignored**. The complete opening run with the precommitted probe was
**178 test-result blocks: 1723 passed, 0 failed, 12 ignored**; the probe itself
was **2 passed, 0 failed, 9 ignored**. The `nlang-tools` worktree contained only
the new probe before this document was written. The superproject remains dirty
in unrelated paths; no such path is part of this order.

---

## 1. The defect: two observation facts are one record

A peer advertisement contains a signed, self-authenticating half and a
receiver-local observation half. The current engine does not record how the
receiver learned the advertisement.

### 1.1 Direct acceptance

The direct `#advertise` accept ladder is in
`crates/interpreter/src/oodp.rs:897-918`. It has the one fact that a relay
cannot provide: the host is the peer address on the connection carrying the
advertisement. The current construction records that host as
`observed_host`, derives `addr`, and sets `%hops` to zero.

This is the fact the future admission arc will need, but it is not currently
labelled as direct.

### 1.2 Relayed acceptance

`oo node discover` consumes a `#discover` response in
`crates/oo/src/main.rs:578-608`. It connects to the relayer, not to the node
named by the signed advertisement, and writes the relayer-provided
`%observed_host` and `%hops` into a local `PeerAdvert`.

The relayer's host and hop count are assertions by the relayer. They are not
covered by the advertised node's signature. A response claiming `%hops: 0` is
still relayed when it arrived through `#discover`; zero is not a proof of local
observation.

### 1.3 The overwrite

`record_peer_advert` in
`crates/interpreter/src/lib.rs:669-699` currently performs unconditional
last-wins replacement by `node_id`:

```rust
if let Ok(mut dir) = self.peer_adverts.write() {
    dir.insert(node_id.clone(), advert.clone());
}
```

Therefore a relayed copy can replace a direct observation, and a direct
observation cannot be recovered from the durable record after that replacement.
The durable encoder/loader in
`crates/interpreter/src/peers.rs:79-190,193-293` persists observer-local fields
but has no provenance field. A restart or an old record consequently cannot say
whether a host was actually observed here.

The active remote-source writer in
`crates/interpreter/src/builtins/disc.rs:90-147` is deliberately not part of
this order. Provenance must be corrected before any future admission code reads
this state.

---

## 2. Vocabulary and authority boundary

The receiver-local state has exactly three semantic values:

* **`direct`** — this receiver accepted a valid `#advertise`, and the host was
  derived from the connection carrying that advertisement;
* **`relayed`** — this receiver learned the signed advertisement through a
  `#discover` response; the host and hop count remain relay assertions;
* **`unknown`** — provenance is absent, legacy, or was cleared because a record
  was copied to a different owner.

These values answer only **how this receiver learned this advertisement**. They
are not a trust root, a correctness verdict, a ranking preference, or a claim
that an address is safe to dial.

The signed advertisement and the `#discover` response remain unchanged. No
provenance key may enter the signed body, the node signature payload, or the
wire response. The current direct/relay distinction must be recorded by the
receiver at the path where it is known, not reconstructed later from:

* `node_id`;
* `%hops == 0`;
* a non-empty host or address;
* `received_at`;
* a TCP connection to a relayer; or
* an unsigned wire spelling.

Provenance belongs to the exact verified signed advertisement identity (the
body identity/CAID or an equivalent exact-ad key), not merely to `node_id`. One
node may publish different signed advertisements over time by changing its
port, services, capacity, timestamp, or affiliation data. A direct observation
of an older advertisement must not bless a newer relayed advertisement from the
same node.

---

## 3. Required representation and merge semantics

The implementation may use an enum such as
`ObservationProvenance::{Direct, Relayed, Unknown}` or an equivalent durable
representation. An optional receiver-local `provenance` value in the existing
peer-directory record is the expected shape. It must not create a new top-level
`.oo/` path or bump `.oo/format`.

For one exact signed advertisement, merge precedence is:

| incoming | existing | required result |
| :-- | :-- | :-- |
| direct | direct | same-class update may win |
| direct | relayed | direct wins |
| direct | unknown | direct wins |
| relayed | direct | existing direct remains, including its host/address |
| relayed | relayed | same-class update may win |
| relayed | unknown | relayed wins |
| unknown | direct | existing direct remains |
| unknown | relayed | existing relayed remains |
| unknown | unknown | current last-wins behaviour may remain |

For a different signed advertisement with the same `node_id`, the incoming
record starts with its own provenance. It may replace the current record under
the existing current-record policy, but it must not inherit the old
advertisement's direct status.

A direct record for one exact advertisement must therefore not be replaced by a
relayed copy of that same advertisement. This rule protects both the direct
host and the direct provenance. Same-class relayed updates may retain the
existing last-wins behaviour for their asserted host and hop count.

---

## 4. Persistence, restart, copy, and legacy rules

The existing signed-half/observer-half split remains authoritative:

* the verbatim signed `%ad` and its signed fields travel in a copied workspace;
* `observed_host`, `addr`, `hops`, `received_at`, and provenance are local
  observations and do not travel as observations.

Required persistence behaviour:

1. A same-owner restart restores both `direct` and `relayed` provenance.
2. A missing provenance key decodes as `unknown`, never as `direct`.
3. A copied workspace retains the signed advertisement but clears
   `observed_host`, `addr`, `hops`, `received_at`, and provenance, just as it
   clears the other observer-local fields today.
4. A copied record may be relayed, and that new local receipt may set provenance
   to `relayed`; it may not make the copied record `direct`.
5. A later direct observation may set the copied record to `direct`.
6. Existing `received_at` fallback and owner comparison behaviour must not
   change unless calibration identifies a collision owned by this order.

This is a local metadata addition, not a new persistence authority. The signed
bytes must round-trip unchanged because relaying a re-serialised advertisement
would invalidate its signature.

---

## 5. Probe gates

The probe uses real `oo node serve`, raw OODP requests, a real signed computing
advertisement, isolated `OO_IDENTITY` and `OO_NODE_HOME`, and a controlled fake
relayer. It inspects the canonical durable record only where that is the
specified local observation surface.

The controls run before every absence claim that could otherwise pass through a
missing server, empty directory, or failed network setup.

### 5.1 Controls — live now and after delivery

| test | property |
| :-- | :-- |
| `c0_direct_and_relayed_advertisements_are_both_live` | direct `#advertise` and relayed `#discover` both reach the durable directory; both paths are real and non-empty |
| `c1_restart_and_copy_still_discriminate_the_observer_half` | same-owner restart restores the observed host, while a copied workspace retains the signed ad but emits no host it did not observe |

Opening calibration: **2 passed, 0 failed, 9 ignored**.

### 5.2 Reds — precommitted, currently ignored

| # | test | required property | calibrated current failure |
| :-- | :-- | :-- | :-- |
| R1 | `r1_direct_receipt_records_direct_provenance` | direct receipt stores explicit `direct` | signed ad, direct host/address, and `hops: 0` are present, but `provenance` is absent |
| R2 | `r2_relayed_receipt_records_relayed_provenance` | `#discover` receipt stores explicit `relayed`, even though the receiver connected to a relayer | signed ad is preserved, but `provenance` is absent, so the record is not distinguishable from direct |
| R3 | `r3_direct_is_authoritative_for_the_same_signed_advertisement` | direct wins over relay in both direct→relay and relay→direct orderings | both arrival orders are exercised; the current last-wins directory has no provenance, and in direct→relay the asserted host `198.51.100.20` replaces the direct `127.0.0.1` host |
| R4 | `r4_a_different_signed_ad_does_not_inherit_direct_provenance` | a newer signed ad from the same node starts with its own `relayed` state | the newer ad and relay host `198.51.100.22` are selected correctly, but no provenance is stored |
| R5 | `r5_a_forwarded_relayed_observation_stays_relayed` | a second relay remains `relayed` | the second relay's host `198.51.100.24` and `hops: 2` are present, but provenance is absent |
| R6 | `r6_zero_and_one_claimed_hops_are_both_relayed` | relay claims with `%hops: 0` and `%hops: 1` are both `relayed` | both values and both documentation-range hosts are exercised before assertion; neither record has provenance |
| R7 | `r7_restart_preserves_direct_and_relayed_provenance` | same-owner restart preserves direct; relayed records remain relayed | restart first restores the direct host, then the later relay overwrites it with `198.51.100.27`; both records lack provenance |
| R8 | `r8_copy_clears_local_provenance_before_a_later_relay` | copy clears local provenance and a later relay can set only `relayed` | copy-side host clearing passes; the later relay stores `198.51.100.29` but no provenance |
| R9 | `r9_legacy_missing_provenance_is_conservative` | missing optional field is `unknown`, not direct, and a later relay can set `relayed` | after the field is removed from a direct record, the same-ad relay preserves signed bytes and stores `198.51.100.30`, but no provenance |

Every red was run independently with the complete test name and
`--ignored --exact --nocapture`. No red was accepted because of a failed
startup, empty directory, absent response, or an unexercised comparison side.
R3 and R6 execute both arrival/hop cases before their assertions. R9 checks the
canonical durable record because the current relay client intentionally writes
`services: []` and `ts: 0`; using its stale-filtered service index would test an
unrelated defect.

Probe modification rights belong to the acceptor. Delivery may remove only the
nine `#[ignore]` attributes. If a probe is found to be wrong during delivery,
it must be reported rather than rewritten to make the engine pass.

---

## 6. Existing pins and scheduled changes

No existing pin is scheduled to change at order opening. In particular, leave
these owners untouched:

* advert persistence restart/copy and signed-record verification pins;
* discover-index relay bounds and `%observed_host`-outside-signature pins;
* affiliation claim's three-path verification and trust-root root-alone pins;
* `connect_consent_probe_test.rs::p2_the_directory_is_still_not_a_fetch_source`;
* local-GC and Kademlia durable-state allow-list pins;
* serving `#fetch`, source-verification, and byte-preservation pins;
* packet/wire and `wire_says_why` classification pins.

The untruncated durable-state scan found the existing top-level allow-list
owners in `advert_persistence_probe_test.rs::r2_the_file_appears_where_declared_and_nowhere_else`,
`kademlia_table_probe_test.rs::p4_nothing_persisted`, and
`local_gc_probe_test.rs::p4_no_undeclared_durable_state`. They already account
for the existing `.oo/peers/` directory. This order adds only a field inside
that existing directory and no new top-level path, so no pin's claimed absence
is being retired and no pin is scheduled to change. If implementation
calibration finds a real contradiction, stop and report the exact owning pin;
do not weaken it.

---

## 7. Scope boundary

### In scope

1. receiver-local `direct` / `relayed` / `unknown` provenance;
2. binding provenance to the exact verified signed advertisement;
3. direct-over-relayed merge precedence for the same advertisement;
4. conservative legacy decoding;
5. owner-aware restart and copy semantics;
6. relay forwarding retaining relay provenance;
7. the nine acceptance reds and their calibration record.

### Forbidden in this order

* automatic dialing, automatic fetching, or source-map insertion;
* automatic admission or consumption of affiliation roots/claims;
* source caps, ranking, eviction, or preference policy;
* changing signed advertisement bytes or `#discover` response shape;
* treating provenance as correctness, trust, or address safety;
* changes to Kademlia, CAS, GC, universe roots, identities, language surface,
  `.oo/format`, or unrelated durable state;
* specification closure, CHANGELOG, VERSION, ENGINE_SYNC, release metadata, or
  a release cut.

The next separate arc is **automatic admission + hard cap**. It must consume only
a trustworthy direct-observation state produced by this order; it is not a
hidden deliverable here.

---

## 8. Named future implementation boundary

The opening order does not edit these files. Delivery is expected to stay at
these boundaries:

* `crates/interpreter/src/lib.rs` — provenance type/metadata and exact-ad merge
  logic;
* `crates/interpreter/src/oodp.rs` — mark direct `#advertise` ingestion without
  changing wire format;
* `crates/interpreter/src/peers.rs` — optional provenance encoding/decoding,
  legacy `unknown`, owner-mismatch clearing, and reload precedence;
* `crates/oo/src/main.rs` — mark `node discover` records as relayed while
  preserving relay assertions and wire bytes.

Do not change `crates/interpreter/src/builtins/disc.rs` in this order. Its
current remote-source insertion remains governed by the separate connect and
future admission arcs.

---

## 9. Satisfiability check

The order is satisfiable without crossing into admission:

* direct and relayed inputs are structurally separable at two existing call
  sites, before the records are merged;
* the probe signs a computing body, so success cannot be manufactured from a
  shape-only response;
* same-ad arrival-order cases are both executable, with distinct observed hosts;
* different-ad identity is varied by timestamp and listen port while retaining
  the same node key;
* owner restart and copied workspace are independently observable;
* legacy absence is created by removing one optional durable key, not by making
  the entire directory disappear;
* the expected direct/relay outcomes do not require an impossible “A broken
  while B good” state.

No gate needs automatic dialing, a trust root, or a cap. The future admission
arc therefore remains a genuine subsequent decision rather than an implicit
implementation detail.

---

## 10. Opening verification record

The following measurements were taken before any engine implementation:

```text
nlang-tools: dev 62e6cd5
workspace before probe: 1721 passed / 0 failed / 3 ignored
workspace with precommitted probe: 1723 passed / 0 failed / 12 ignored
probe controls: 2 passed / 0 failed / 9 ignored
cargo fmt --manifest-path /home/gali/nlang/nlang-tools/Cargo.toml --all -- --check: pass
git diff --check: pass
```

The nine ignored reds were each run independently and all exited non-zero at
the intended provenance/merge assertion. The aggregate ignored run was
`0 passed, 9 failed, 0 ignored, 2 filtered out`; no setup or empty-directory
failure was the reason for a red. Existing pin suites and the full workspace
were rerun after this document was written. Conformance and genesis are
acceptance measurements, not specification edits; they remain outside the
delivery implementation scope.

### Required post-opening commands

Run without truncation:

```text
cargo test --manifest-path /home/gali/nlang/nlang-tools/Cargo.toml \
  -p oo --test direct_observation_provenance_probe_test
cargo test --manifest-path /home/gali/nlang/nlang-tools/Cargo.toml \
  -p oo --test direct_observation_provenance_probe_test -- --ignored
cargo test --manifest-path /home/gali/nlang/nlang-tools/Cargo.toml \
  --workspace --no-fail-fast
```

Run the existing owners unchanged:

```text
cargo test --manifest-path /home/gali/nlang/nlang-tools/Cargo.toml -p oo \
  --test advert_persistence_probe_test
cargo test --manifest-path /home/gali/nlang/nlang-tools/Cargo.toml -p oo \
  --test advertise_wire_probe_test
cargo test --manifest-path /home/gali/nlang/nlang-tools/Cargo.toml -p oo \
  --test discover_index_probe_test
cargo test --manifest-path /home/gali/nlang/nlang-tools/Cargo.toml -p oo \
  --test affiliation_claim_probe_test
cargo test --manifest-path /home/gali/nlang/nlang-tools/Cargo.toml -p oo \
  --test discovery_trust_probe_test
cargo test --manifest-path /home/gali/nlang/nlang-tools/Cargo.toml -p oo \
  --test connect_consent_probe_test
cargo test --manifest-path /home/gali/nlang/nlang-tools/Cargo.toml -p oo \
  --test local_gc_probe_test
cargo test --manifest-path /home/gali/nlang/nlang-tools/Cargo.toml -p oo \
  --test kademlia_table_probe_test
cargo test --manifest-path /home/gali/nlang/nlang-tools/Cargo.toml -p oo \
  --test peer_fetch_verification_probe_test
```

Do not edit those suites as part of opening this order. The post-opening runs
were all green:

| suite | result |
| :-- | :-- |
| `direct_observation_provenance_probe_test` controls | **2 passed / 0 failed / 9 ignored** |
| `advert_persistence_probe_test` | **19/19** |
| `advertise_wire_probe_test` | **19/19** |
| `discover_index_probe_test` | **17/17** |
| `affiliation_claim_probe_test` | **20/20** |
| `discovery_trust_probe_test` | **20/20** |
| `connect_consent_probe_test` | **9/9** |
| `local_gc_probe_test` | **17/17** |
| `kademlia_table_probe_test` | **17/17** |
| `peer_fetch_verification_probe_test` | **12/12** |
| full workspace | **1723 passed / 0 failed / 12 ignored**, 178 result blocks |
| conformance | **143/143** |
| genesis | **11/11** |

The nine ignored reds remain deliberately red until delivery; no existing pin
or probe was edited.

---

## 11. Delivery rules

* Probe modification rights belong to the acceptor; delivery removes only
  `#[ignore]` from this probe.
* The delivery must not edit the specification, CHANGELOG, version metadata,
  release state, or unrelated probes.
* The delivery must not add automatic admission, a source cap, or a dial.
* Use explicit git paths; never `git add -A`, and do not use stash.
* The nlang-tools commit message must be English. The user pushes; this order
  opening does not push.

---

## 12. Ledger — deliberately not fixed here

* `engine.save` still has the previously measured naive unpacking/data-loss
  issue; it is unrelated to observation provenance.
* The relay client writes sparse `services: []` / `ts: 0` rows; R9 avoids making
  that unrelated index behaviour a provenance gate.
* `random_below` still silently returns zero on entropy failure.
* `#success` without `%result` remains an integrity-incident classification
  question.
* `reader.read_line` remains unbounded; `free_port()` remains TOCTOU; and
  `routing_id_from_digest` still zero-pads a short digest.
* Automatic admission and its hard cap remain the next separate arc.

None of these ledger items authorises expanding this order.
