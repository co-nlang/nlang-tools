# 在席是誰 / Which three hold the seats — work order

**Opened** 2026-07-31. **Baseline** `dev 87d5d25` (engine v0.9.0, tag on
`top 1e719e0`; spec `v0.9.0-draft.1`).
**Probes** `crates/oo/tests/seat_order_probe_test.rs` — acceptor-owned,
pre-committed and calibrated before this order was written.
Workspace at baseline: **1750 passed, 0 failed, 5 ignored** (2 of those ignored
are this arc's reds; 3 are standing).

---

## 1. The defect

REAL_02 §4.2.6.2 caps automatic fetch sources at three and says overflow keeps
the incumbents. Inside one process that is arrival order. **Across a restart it
is not.** The set is rebuilt from the durable directory, sorted by
`received_at` and then by `node_id`, and `received_at` is stored with
**one-second resolution**. Everything arriving inside one second therefore
ties, and the tie is broken by `node_id` ascending — a hash of the peer's
public key.

Measured 2026-07-31, five adverts accepted in one second (durable `received_at`
byte-identical across all five):

```
arrival order:       [2, 0, 3, 4, 1]     (sent in descending node_id)
node_id ascending:   [1, 4, 3, 0, 2]
durable received_at: all five == 1785505650
seats:               [1, 3, 4]           = the three lowest node_id
expected:            [0, 2, 3]           = the first three to arrive
```

Stable across five restarts. **The rebuild is deterministic; it is
deterministically wrong.**

> §4.2.6.3's first draft claimed the opposite — that restarts could disagree —
> because it was written from reading the code rather than running it. The
> clause now carries that correction in its own text. Recording it is cheaper
> than letting the next reader re-derive it.

## 2. Why this is worse than the routing table's hash order

`node_id = sha256(public key)`, and SPEC_15 §7.1 prices minting at a measured
**3,500 keys/second**. The routing table also orders by identity, but its
bucket index is an XOR against **the victim's own id**, so grinding buys a seat
at *one* victim — §7.1's table is priced per target for exactly this reason.

Raw `node_id` ascending is **victim-independent**. The smallest id is smallest
for everyone.

> Grind once, hold a seat on every node that ever hears you in the same second
> as someone else.

This is §4.3.5.1's line — *a deterministic rule is a rule the attacker computes
offline too* — one layer down.

## 3. Why the answer is not sampling

§4.3.5.1 met the same shape (a cap that never said which) with a declared
uniform sample. **That answer does not transfer**, and the reason is the
currency:

* a discovery reply is not a verdict (§4.3.2) and re-drawing costs latency;
* an automatic source spends the operator's **time** and tells someone **what
  they are looking for** (§4.2.6.1).

Re-drawing every restart would unseat honest sources for no reason, and would
hand an attacker a fresh chance at every boot instead of none. **P1 pins that
the rebuild stays deterministic.**

---

## 4. What is being asked for

### B′ — the durable record must make arrival order total (MUST)

The record must carry enough to order arrivals **totally**, and the rebuild
must follow that order. Finer timestamps, a monotonic admission sequence,
anything else — **the probe pins the property, not the spelling** (R1).

**It must be additive.** `decode_record_line` reads key by key, so a new key is
tolerated by an older reader. Re-purposing `received_at` into a different unit
is **not** additive: a v0.9.0 engine would read milliseconds as seconds and
place every record in the year 58000. P3 pins `.oo/format`; P4 pins that a
directory in the v0.9.0 shape still loads and still seats.

### C′ — a tie must not be broken by the peer's identity alone (MUST NOT)

Kept as a red line rather than a gate. Once B′ holds it is unreachable, and a
rule guarding an unreachable state is not something a probe can exercise —
**this arc deliberately ships no probe for it, and says so** rather than
inventing one that passes vacuously. The reason it is still written down: a
future implementation with a coarse clock or a coarse counter can reach the
tie, and at that point the fallback must not be the globally grindable one.

The tiebreak, if an implementation has one at all, should mix in **this
node's** identity so that grinding costs one victim rather than all of them.
Note that this is legitimate here and was **not** legitimate for `#discover`:
§3.2 forbids the **asker** (`%from`) influencing a decision because that
manufactures a partition surface. There is no asker here — the receiver is
allocating its own seats, keyed to its own identity. The two only look alike.

### Not in scope

* the cap value, the eligibility contract, or any §4.2.6.2 clause other than
  seat order;
* wire bytes, CAIDs, `%discover` response shape, `.oo/format`;
* the `#discover` sampling rule (§4.3.5.1) — untouched;
* persistence of the automatic set itself (it stays process-local);
* the standing ledger items in §7.

---

## 5. Probes

```
cargo test --test seat_order_probe_test              # 2 controls + 5 pins, green now
cargo test --test seat_order_probe_test -- --ignored # 2 reds, both red now
```

**Probe modification rights belong to the acceptor.** The delivery removes
`#[ignore]` and nothing else.

| | what it holds |
| :-- | :--- |
| C1 | the fixture overflows and seats are observable |
| C2 | under the cap, every eligible candidate seats |
| **R1** | same-second arrivals keep **arrival** order |
| **R2** | that order survives a **compaction** |
| P1 | the same file seats the same three, restart after restart (ruling D) |
| P2 | arrivals a second apart already keep their order, and still do |
| P3 | `.oo/format` is not bumped |
| P4 | a directory in the v0.9.0 shape still loads and still seats |
| P5 | eligibility untouched — no claim, no seat, however early |

### 5.1 R1 is a construction, not a draw

The candidates are advertised in **descending `node_id`**, so today's fallback
hands the seats to the *last* three to arrive. There is no fixture to redraw
and no probability to quote: it is red every run. (Advertising in a random
order would have been green about one run in ten, when the three lowest ids
happen to be the first three to arrive — the trap the affiliation arc's R5 fell
into, avoided here at design time.)

### 5.2 R2 fails at its precondition today, on purpose

R2 builds on the property R1 is about, so at baseline it fails before reaching
its own assertion and adds nothing independent. That is the honest shape for a
layered property — **do not read a green R2 as separate evidence until R1 is
green.** Its real content is the compaction clause, and that clause requires
the rewrite to have been *announced* in the serve log; a run where compaction
never triggered fails rather than passing empty.

---

## 6. Acceptance measurements (acceptor's, not probes)

1. **Diff purity** — no probe edits beyond removing `#[ignore]`; no `git add -A`.
2. **Independent re-run**: workspace, conformance, genesis, plus the unchanged
   `automatic_admission`, `advert_persistence`, `discovery_trust`,
   `connect_consent`, `direct_observation_provenance` owners.
3. **Repeat-run stability**, several times. Anything touching arrival order and
   a one-second boundary is a flake risk by construction.
4. **Cross-version against v0.9.0, both directions.** No wire bytes change, so
   this is expected to be incremental — measured, not assumed. Include a
   v0.9.0 engine **opening a directory this build wrote** and vice versa; that
   is where an additive-vs-repurposed field would show.
5. **Grind cost restated.** If a tiebreak survives at all, state what it costs
   an attacker per victim, so SPEC_15 §7.1 can be updated with a number rather
   than a claim.

---

## 7. Ledger — known and deliberately not fixed here

* `#success` with no `%result` is still recorded as an integrity incident.
* Unknown advert fields are relayed and persisted verbatim, bounded only by
  64 KiB per request and §7.1's per-identity minting price.
* `advert_persistence_probe_test.rs`'s P3 compares object counts with no
  non-empty guard on the baseline.
* `local_gc` leaves `nlang-gc-r5-*` fixtures behind in the temp directory.
* `to_nlang` prints unforced Thunks as Rust `Debug`; `reader.read_line` is
  unbounded; `free_port()` is TOCTOU; `routing_id_from_digest` zero-pads.
