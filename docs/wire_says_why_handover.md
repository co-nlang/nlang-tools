# The wire says why — work order

Give every non-success OODP response a `%reason`, return `#not_implemented`
for an op this node does not serve, and stop the client from recording
protocol-level answers as integrity verdicts.

REAL_02 §3.2 requires four things to be distinguishable and gives three codes.
It says so about itself, and adds: **「在裁定之前,本條不得被引用為已達成的保證。」**
This is that ruling.

---

## 1. What was measured, on v0.2.54, before this order was written

### 1.1 `#conflict` carries five meanings

| case | `%status` |
|---|---|
| held + intact | `#success` |
| **held but corrupt on the peer's disk** | `#conflict` |
| not held | `#not_found` |
| **unknown op** | `#conflict` |
| **known op, required field missing** | `#conflict` |
| **malformed line** | `#conflict` |
| **unparseable CAID string** | `#conflict` |

Only the second is an integrity verdict. The other four are "your request was
wrong" or "I do not do that".

### 1.2 The client records all of them as integrity incidents

`oodp.rs` ~1390: `#conflict` and `#not_implemented` both call
`record_integrity(Mismatch)`; any unrecognised status calls
`record_integrity(Undecodable)`. All three return `BottomCause::CaidMismatch`.

Measured end to end against a stub peer:

```
peer answers #teapot            → integrity #undecodable: … source=tcp://…   ⊥ #caid_mismatch
peer answers #not_implemented   → integrity #mismatch:    … source=tcp://…   ⊥ #caid_mismatch
```

**A peer newer than this client is recorded as serving undecodable objects.**
That is the worst possible direction: every future protocol extension makes
older clients accuse newer nodes of corruption. And a peer that honestly says
"I do not serve that op" is recorded as having failed an integrity check.

Both are REAL_03 §6.6 `裁決必須為真` violations, same family as the `oo inspect`
false verdict repaired in v0.2.53.

### 1.3 The collapse also corrupts the multi-source verdict

`disc.rs` ~208: `Err(BottomCause::CaidMismatch) => saw_mismatch = true`. Since
every peer failure collapses to that cause, a peer that merely does not
implement the op sets `saw_mismatch`, and the scan's final answer becomes
`⊥ #caid_mismatch` instead of "nobody has it".

**ERROR_CODES §`#caid_mismatch` already forbids this**: 「純粹『無人持有』不適用此碼」.
So this is a conformance violation, not only a design wart.

### 1.4 The mechanism for saying why already exists, and is fenced off

`#rejected` carries `%reason` today (measured: `#advertise` with no `%ad` →
`#rejected` + `%reason: #malformed`). §3.2 also states the principle —
「狀態集合維持小而跨 op 穩定,可分性住在 `%reason`」 — and then blocks it:
`%reason` appears **當且僅當** `%status` 為 `#rejected`.

The spec argues for the fix and forbids it in the same section.

---

## 2. Rulings carried into this order

Owner, 2026-07-29.

### R1 — `%reason` opens to every non-success status, and an op this node does not serve gets `#not_implemented`

Both of §3.2's proposed fixes, because they answer different halves. The
status set stays small and stable across ops; distinguishability lives in
`%reason`; and `#not_implemented` goes back to meaning what it says.

### R2 — a peer speaking a language this client does not know is not a broken peer

An unrecognised `%status` gets its own cause and **is not an integrity
incident**. A node newer than you is not a node that has been corrupted.

---

## 3. Design

### 3.1 Reason vocabulary

`%reason` is a tag. Small, stable, and shared across ops.

| `%status` | `%reason` | means |
|---|---|---|
| `#not_implemented` | `#unknown_op` | this node does not recognise the name |
| `#conflict` | `#caid_mismatch` | **held, and address verification failed** — the only integrity verdict on this table |
| `#conflict` | `#malformed` | the packet did not parse |
| `#conflict` | `#missing_field` | op recognised, a required field was absent |
| `#conflict` | `#unparseable_caid` | `%hash` / `%target` was not a CAID |
| `#rejected` | (existing §4.2.2 set) | unchanged |

`%reason` is **required** on every non-`#success` response this node emits.
`#not_found` carries `%reason: #not_held` for uniformity — a client must never
have to infer from absence.

### 3.2 Client-side causes

| what the peer said | `%cause` | integrity incident? |
|---|---|---|
| `#conflict` + `%reason: #caid_mismatch` | `#caid_mismatch` | **yes** (unchanged) |
| `#conflict` + any other `%reason` | `#peer_refused` | no |
| `#conflict` + **no** `%reason` (an older peer) | `#peer_refused` | **no** — see below |
| `#not_implemented` | `#peer_not_implemented` | no |
| unrecognised `%status` | `#peer_unknown_status` | no |
| `#not_found` | `#missing_key` | no (unchanged) |

**An unexplained refusal is not evidence of corruption.** Treating a
`%reason`-less `#conflict` from a v0.2.54 peer as an integrity incident would
keep today's false accusations alive for exactly the peers least able to
defend themselves. The cost is real and is accepted: a genuine peer-side
integrity detection from an older peer now reads as a refusal. It is bounded,
because the client re-verifies every byte it accepts anyway (REAL_03 §6.6) —
what is lost is the peer's *report*, not the client's own check.

### 3.3 The multi-source scan

`saw_mismatch` must be set **only** by `#caid_mismatch`. A source that refused,
did not implement, or spoke an unknown dialect is skipped and the scan
continues (REAL_02 §6.1.1 — sources are peers at degree 0 and a failing one
must not abort the scan). When no source verified and none mismatched, the
answer is `⊥ #missing_key`, not `⊥ #caid_mismatch`.

### 3.4 Two new error codes

`#peer_not_implemented`, `#peer_unknown_status`, `#peer_refused` — three, on
the `#peer_timeout` pattern: the `peer_` prefix marks a condition of the other
end, whose remedy points away from the reader's own program. Acceptance writes
ERROR_CODES; **delivery does not touch the spec**.

---

## 4. Deliverables

1. `%reason` on every non-success response, per §3.1.
2. Unknown op → `#not_implemented` + `%reason: #unknown_op`.
3. Client causes per §3.2; integrity incidents only for `#caid_mismatch`.
4. `saw_mismatch` set only by `#caid_mismatch` (§3.3).
5. Un-`#[ignore]` the reds in `crates/oo/tests/wire_says_why_probe_test.rs`.
6. Update the one pin in §6.1 — **scheduled to change**.

## 5. Out of scope — do not deliver

* Any new `%status` value. The set stays `#success` / `#not_found` /
  `#conflict` / `#not_implemented` / `#rejected`.
* Signing, delegation, node endorsement.
* The CAID-path fix (`~%Discovery./identify` returns the argument pack's CAID,
  measured 2026-07-29) — a separate breaking arc.
* Spec edits, ERROR_CODES entries, CHANGELOG. Acceptance does those.

## 6. Gates

`crates/oo/tests/wire_says_why_probe_test.rs`, pre-committed with this order.
**Probes belong to the acceptor**: delivery removes `#[ignore]` and nothing
else. If a probe looks wrong, say so and stop — that has happened twice and
both times the order was at fault.

### Control

`c0_a_wellformed_request_is_still_answered` — leads the file. Every red below
asks "what does the server say when something is wrong", and a server that
answered nothing at all would satisfy most of them.

### Reds

| # | Name | Holds |
|---|---|---|
| R1 | `unknown_op_is_not_implemented` | `#not_implemented` + `%reason: #unknown_op` |
| R2 | `corrupt_and_unknown_op_are_distinguishable` | the §3.2 MUST, as a single assertion over both |
| R3 | `every_non_success_carries_a_reason` | all seven measured cases, scanned, none without `%reason` |
| R4 | `the_reason_names_which_conflict_it_is` | corrupt → `#caid_mismatch`; missing field → `#missing_field`; garbage → `#malformed` |
| R5 | `not_implemented_is_not_an_integrity_incident` | stub peer; `⊥ #peer_not_implemented`, integrity log empty |
| R6 | `an_unknown_status_is_not_an_integrity_incident` | stub peer answers `#teapot`; `⊥ #peer_unknown_status`, log empty |
| R7 | `an_unexplained_conflict_is_a_refusal` | stub peer answers `#conflict` with no reason; `⊥ #peer_refused`, log empty |
| R8 | `a_scan_past_a_refusing_peer_says_missing` | refusing peer + peer without the object → `⊥ #missing_key`, not `#caid_mismatch` |

**Calibration moved one gate.** `a_reasoned_caid_mismatch_still_accuses` was
written as a red and is green at baseline for the wrong reason: today *every*
`#conflict` accuses, so "the reasoned one accuses" holds trivially. It is P7.
R7 and P7 are still a pair — R7 alone is satisfiable by never accusing anyone,
P7 alone by accusing everyone. R7 is the red; P7 is what stops the repair
going too far.

### Pins

| # | Name | Holds |
|---|---|---|
| P1 | `the_status_set_did_not_grow` | exactly the five values appear across every case |
| P2 | `a_held_object_still_arrives` | `#success` + value, unchanged |
| P3 | `not_found_is_still_not_found` | absence is not conflict |
| P4 | `advertise_rejection_is_unchanged` | `#rejected` + `%reason` per §4.2.2 |
| P5 | `silence_is_still_never_an_answer` | seven adversarial inputs, none gets 0 bytes |
| P7 | `a_reasoned_caid_mismatch_still_accuses` | stub peer answers `#conflict` + `%reason: #caid_mismatch`; `⊥ #caid_mismatch` **and** one integrity incident — the guard on R7 |
| P6 | `a_computing_payload_is_still_refused` | an advert body that evaluates but fails the ladder is still rejected — the standing rule that adversarial cases at a remote-input entry point must include a payload that computes |

### 6.1 Scheduled to change — not an invariant

`crates/oo/tests/advertise_wire_probe_test.rs::p4_unknown_and_retired_forms`
asserts unknown op → `#conflict`. This arc changes exactly that. Update the
unknown-op assertion to `#not_implemented`; the retired bare-CAID form and the
garbage line stay `#conflict` and must keep their assertions. Nothing else in
that file moves.

Found by the standing mechanical check:
`grep -rn "status_of" crates/oo/tests/*.rs | grep -E '"conflict"|not_implemented'`.

### 6.2 Classification note for acceptance

Expected **增量**, and the measurement that decides it: today's client
collapses `#not_implemented` and `#conflict` into the same result, so a
v0.2.54 client sees no behaviour change when a v0.2.55 node starts returning
`#not_implemented` for an unknown op. `%reason` is a field older clients
ignore. Acceptance must run this both ways rather than assume it.

## 7. Acceptance numbers to have ready

* workspace / conformance / genesis / wire_says_why / advertise_wire / oodp /
  discover_index / advert_persistence counts
* the seven-case status+reason matrix, before and after, in full
* the two stub-peer measurements from §1.2, re-run
* both cross-version directions
* the integrity log after a scan that met a refusing peer — it must be empty

## 8. Ledger — not this arc

1. `reader.read_line` unbounded (`#fetch` shares it).
2. **`~%Discovery./identify x` returns the CAID of the argument pack**, not of
   `x` — SPEC_13 §6.1 and REAL_02 §4.2. Next arc, breaking.
3. **apply's builtin calling convention is ambiguous**: measured
   `~%Math./add 1 2` ≡ `~%Math./add (1, 2)` ≡ 3, and `identify (1,2)` ≡
   `identify {{0:1, 1:2}}`. A builtin cannot recover its argument. Language
   design; the root under item 2.
4. `mod advert_debug` ships with `println!`.
5. `free_port()` is TOCTOU in every network probe suite.
6. `routing_id_from_digest` zero-pads silently.
7. `oo <cmd> | head` panics on SIGPIPE.
8. **Node endorsement is unspecified** — the spec says only that the operator
   and node keys must differ. Measured non-breaking (an unknown field inside a
   signed body is accepted), so it is an 增量 arc of its own. It must not be
   called 委任: ORDER_01 §7.4 and APP_05 already use that word for two other
   things.

## 9. Delivery record (delivery side)

Fill in on return: what was built, what was measured, what was left, anything
noticed and not fixed.
