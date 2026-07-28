# Kademlia routing table + `#find_node` — work order

Arc opened 2026-07-28 against **v0.2.51** (`top d2e3d28`). Scope: **the table
itself and one query op**. Iterative lookup is not in this arc — see §5.

Deliver on `dev`. Do not tag, do not touch `top`, do not write spec files.

---

## 1. What was measured before this order was written

| # | Fact | How |
| :-- | :--- | :--- |
| M1 | **The engine has no routing-table machinery at all** — no XOR distance, no buckets, no `closest`. The only `bucket` is a stats histogram; the only `xor` is a math builtin | full crate scan |
| M2 | `ContentHash.digest` is **32 bytes / 256 bits**. §4.1's "前 160 bit" is the **first 20 bytes** | measured on a real `node id` |
| M3 | §4.1 calls the field `content_digest`; the field is `digest` | source vs spec |
| M4 | **A probe can synthesise identities in-process** using the engine's own `Identity::node_id_caid()` — `nlang-interpreter` is a normal dependency of `oo`. **0.280 ms/key** | measured, 20,000 keys in 5.59 s |
| M5 | 20,000 synthetic ids reach **bucket 15**, and **10 buckets overflow k=20**. Distribution `[9944, 5044, 2601, 1136, 647, 303, 163, 73, 37, 26, 11, 9, 2, 2]` | measured |

M4 and M5 are why this arc is allowed to exist. The objection at arc-open was
that k-buckets are **unfalsifiable on two or three local nodes** — a probe
cannot tell a real bucket structure from a flat list. M5 removes that: 5.6
seconds of key generation fills ten buckets past capacity, and the correct
answer to `closest(target, k)` can be **brute-forced over the whole inserted
set** and compared. Structure that no measurement can hold accountable is
exactly what this protocol exists to refuse; here, measurement can.

**Stated limit**: random sampling cannot reach buckets ≥16 (probability 2⁻¹⁶).
Table *population* at depth is therefore untestable. The **query path is not**
— `%target` is an arbitrary 160-bit value, so a probe can take a known id, flip
low bits, and exercise `closest` at any depth. Do not confuse the two when
reporting coverage.

### 1.1 The grinding cost, measured

`node_id = sha256(public key)` and a key costs 0.280 ms. Occupying one slot in
a given victim's bucket *i* costs 2^(i+1) keys in expectation:

| bucket | cost to take all k=20 slots |
| :-- | :--- |
| 10 | **≈ 11 seconds** |
| 20 | ≈ 3.3 hours |
| 30 | ≈ 140 days |

This is not a defect introduced here — it is Kademlia's standing assumption
that ids are hard to choose, meeting a machine that generates 3,500 of them a
second. It is written down because **SPEC_15 §7.1's attack-cost model has been
an open TODO since discussion 026**, and this is the first time the numbers
exist. It also connects to ORDER_00 §1.1: an internal mechanism cannot supply
Sybil resistance, so a routing table is an amplifier of that fact, not a cure.

**Incumbent-first (§3.4) is what converts the table from a target into a
race**: grinding buys nothing against a bucket that is already full. The
attacker must be *early*, not merely rich. Say this plainly; do not let it
read as a security claim it is not.

---

## 2. Rulings carried into this order

**R-a — `#find_node` is a new op and must not reuse `#discover`.** "Who is
near this id" and "who serves this CAID" are different questions over
different tables. Sharing an op is the disease this project has spent three
arcs pulling apart (`ttl`, `@GPP_Response`, `身分`).

**R-b — the routing table may be populated *only* by authenticated
advertisements.** Standard Kademlia learns from every message because it has
no authentication; ours has signed adverts, and `%from` is an unsigned claim.
Populating from `%from` would let anyone insert any id — the eclipse attack
handed over for free. Insertion happens **after** the §4.2.2 ladder passes,
and nowhere else.

**R-c — one record store, two indexes.** The advert directory (keyed by
`node_id`, answering §4.3) and the bucket index (answering this op) are two
views of the *same* accepted records. Do not create a second source of truth.

**R-d — incumbent-first.** Bucket full → the new peer is **not inserted**, and
the drop is logged. No network I/O in the insert path: ping-and-replace would
make accepting an advertisement able to dial outward at a stranger's timing,
which is a new amplification vector to buy a refinement this arc does not need.

**R-e — `%target` is a 160-bit id, never a CAID.** 40 lowercase hex
characters, exactly. A CAID as `%target` → `#conflict`. Mapping a CAID to a
target (take its first 20 bytes) is the caller's business; letting the op
accept both is how the two questions get confused again.

---

## 3. Design

### 3.1 The table

*   **Self id** = first **20 bytes** of `ContentHash.digest` of this node's
    `node_id` (M2). Raw bytes, not hex characters.
*   **160 buckets.** Bucket index *i* = the number of leading **zero bits** of
    `XOR(self, peer)`. Equivalently: peers sharing exactly *i* leading bits.
    `i == 160` means the peer *is* self and must never be stored (§3.5).
*   **k = 20** per bucket.
*   **Insertion** only from an accepted `#advertise` (R-b), after all five
    checks pass.
*   **Refresh**: a peer already present re-advertising updates its record in
    place. It does **not** consume a slot and does **not** evict anyone, and
    it does **not** change its position among incumbents.
*   **Full bucket** → new peer dropped (R-d), counted, logged.
*   **No persistence.** `.oo/routing/` is not created; REAL_02 §5.1 already
    notes that file is a blueprint.

### 3.2 The request

```nlang
{{ %op: #find_node, %from: @caid, %target: @str }}
```

`%target`: exactly 40 lowercase hex characters. Anything else — wrong length,
uppercase, a CAID, absent — is `#conflict` (R-e).

`%from` is a claim. Nothing branches on it (§4.3.6's rule, unchanged), and in
particular it is **not** inserted into the table (R-b).

### 3.3 The response

Same shape as REAL_02 §4.3.3, deliberately:

```nlang
{{ %status: #success, %source: @caid, %hops: 1,
   %peers: [ {{ %ad: @str, %observed_host: @str }} ] }}
```

*   The **k closest known peers** to `%target` by XOR, **ascending by
    distance**, ties broken by the id itself so the order is total and
    reproducible.
*   `%ad` is the verbatim signed advertisement source, byte-for-byte, exactly
    as §4.3.3 requires. The receiver verifies it with the same ladder.
*   Empty table → `#success` with `%peers: []`.
*   `MAX_FIND_NODE_PEERS = 20` (= k), and the **same 64 KiB body budget** as
    §4.3.5, kept on **both** sides — the client bound added by the v0.2.51
    acceptance repair applies here too. A budget only the honest side keeps is
    not a budget.

### 3.4 Answering is not the same as knowing

`closest` searches the **whole table**, not just the bucket the target falls
in. A target may be closest to peers spread over several buckets, and an
implementation that returns "bucket[i] then stop" is wrong even though it will
look right on most inputs. R5 brute-forces this.

### 3.5 Self

Self is never in the table and never in an answer. A node advertising its own
id to itself is accepted at the `#advertise` layer (that is not this op's
business) but must not be inserted.

### 3.6 Observability

Log, on each accepted advertisement:

```
OODP Routing: +<node_id> bucket=<i> occupancy=<n>/<k>
OODP Routing: bucket <i> full, incumbent kept, dropped <node_id>
```

CLI:

```
oo node routing
```

prints one line per **non-empty** bucket (`bucket <i>: <n>`), then `total: <n>`
and `dropped_full: <n>`. This is an operator diagnostic and a probe's only
direct view of the structure it is asserting about — reading it out of a log
would be brittle. Keep the shapes exact.

---

## 4. Deliverables

1. Routing table (§3.1) as a second index over the accepted-advert records.
2. Insertion + refresh + incumbent-first drop, from `#advertise` only.
3. `#find_node` served (§3.2–§3.5).
4. `#find_node` reply verification on the querying side — the §4.3.6 ladder,
   with `ensure_literal_body` **before any evaluation**, and the 64 KiB client
   bound with `#oversize` naming.
5. `oo node find-node --to <host:port> --target <40 hex>` CLI, printing
   `%status` then one line per accepted peer in the same shape `oo node
   discover` uses, `(host unverified, hops=N claimed)` included.
6. `oo node routing` (§3.6).
7. Remove `#[ignore]` from `crates/oo/tests/kademlia_table_probe_test.rs`.
   **Nothing else in that file may change.**

---

## 5. Out of scope — do not deliver

*   **Iterative lookup / multi-hop.** No recursive FIND_NODE, no lookup
    convergence, no α parallelism. This arc answers one question one hop.
*   **Ping / liveness / eviction by probing** (R-d).
*   **Bucket splitting.** 160 fixed buckets. The splitting refinement changes
    nothing measurable at reachable depths and would ship structure no probe
    here can hold.
*   **Persistence.**
*   **Discovered peers still do not become fetch sources.** Unchanged from
    v0.2.51 and still a consent question.
*   **No new language primitive.** CLI only.
*   **No spec files.** Not REAL_02, not APP_05, not SPEC_15, not CHANGELOG.
    The attack-cost model of §1.1 is the acceptor's to land.

---

## 6. Gates

Probes are pre-committed in `crates/oo/tests/kademlia_table_probe_test.rs`,
calibrated before this order was sent. **You may remove `#[ignore]` and
nothing else.** If a probe looks wrong, report it; do not repair it.

**A note the acceptor owes you**: the v0.2.51 order listed a pin
(`advertise_wire`'s "`#discover` is still unimplemented") that its own scope
made impossible to keep, and the delivery had to update it. That was the work
order's fault, not the delivery's. This order therefore names its
scheduled-to-change pins explicitly: **none this time** — nothing in the
existing suites asserts that `#find_node` is unimplemented, because the op did
not exist to be asserted about. If you find one anyway, that is the same class
of error and you should report it rather than absorb it.

### Reds — must go green

| # | What it holds |
| :-- | :--- |
| R1 | bucket index = leading-zero count of XOR, checked for ≥200 synthetic peers spread over ≥6 buckets |
| R2 | capacity: a bucket offered 100+ candidates holds exactly 20 |
| R3 | incumbent-first, **pairwise**: the 20 retained are the first 20 accepted, and every later one is absent |
| R4 | a re-advertisement by a peer already present refreshes it — consumes no slot, evicts nobody |
| R5 | `closest(target, k)` equals **brute force over every inserted peer**, for 20 targets including deep ones built by flipping low bits of a known id |
| R6 | self is in neither the table nor any answer |
| R7 | a relayed `%ad` from `#find_node` verifies from the packet alone |
| R8 | `%target` that is not 40 lowercase hex — including a valid CAID — is `#conflict` |
| R9 | `%from` on `#find_node` never enters the table (R-b) |
| R10 | the answer is byte-identical under three different `%from` values |
| R11 | **a `#find_node` reply whose `%ad` computes** is refused before evaluation and the effect does not occur |
| R12 | both budgets: ≤20 entries, and a hostile relayer's oversized reply is named `#oversize` client-side |

R5 is the probe this arc exists for. A flat list with correct sorting passes
R1 vacuously and fails R2; a real table that searches only one bucket passes
R2 and fails R5. Neither can be satisfied by a structure that is not the one
specified.

R11 is the standing rule from v0.2.50, at the third remote-input entry point:
**an adversarial case must include a payload that computes, not only payloads
of the wrong shape.**

### Pins — must stay green

| # | What it holds |
| :-- | :--- |
| P1 | the whole `discover_index_probe_test` suite |
| P2 | the whole `advertise_wire_probe_test` suite |
| P3 | universe determinism: two fresh workspaces, same source, same root digest — measured on a node whose routing table has been filled and queried |
| P4 | `peer_fetch_verification` and `oodp_packet_format` unchanged |
| P5 | nothing persisted: no `.oo/routing/`, no new objects in the store |

---

## 7. Acceptance numbers to have ready

1. Full suite before and after.
2. Bucket occupancy histogram after inserting N synthetic peers, next to the
   probe's independently computed expectation.
3. R5's brute-force comparison: how many targets, and the worst-case rank
   disagreement (must be zero).
4. R11's effect check: the exact path, shown absent.
5. P3's two root digests in full, shown equal.
6. Cross-version: a v0.2.51 node answers `#find_node` with **`#conflict`**,
   not `#not_implemented` — measured, and it is not what the first draft of
   this order said. `#find_node` is an *unknown* op to that build, and unknown
   ops fall to `#conflict`. The new client must **handle** that reply rather
   than treat it as malformed, and must not report it as though the old node
   were broken.

   > This exposes something the acceptor will take to the spec, not the
   > delivery: REAL_02 §3.2 requires 缺物 / 腐敗 / 未知 op / 未實作 op to be
   > **distinguishable**, and 腐敗 and 未知 op both answer `#conflict` today.
   > Ledger item 6. Do not change it here.
7. The measured cost of your own probe run (keys generated, wall clock), so
   the suite's runtime is a known quantity rather than a surprise.

---

## 8. Ledger — not this arc

1. `reader.read_line` unbounded (pre-existing, v0.2.48).
2. Two CAID computation paths disagree (`~%Discovery./identify` vs bare
   `content_hash()`), still unexplained.
3. `mod advert_debug` with `println!` still in the engine.
4. §4.1 says `content_digest`; the field is `digest` (M3). Spec-side.
5. SPEC_15 §7.1's attack-cost model, now computable (§1.1). Spec-side.
6. **REAL_02 §3.2 requires 未知 op and 腐敗 to be distinguishable, and they
   are not**: both answer `#conflict` (measured on v0.2.51 — `%op: #find_node`
   and `%op: #unify` both give `#conflict`, as does a malformed line). The
   section's own list of four things that must be tellable apart has three
   codes. Pre-existing, spec-side, and not this arc's to fix — but it is the
   same shape as the `#timeout` finding of v0.2.48, one op-set wider.

---

## 9. Calibration record — measured on v0.2.51 before this order was sent

Pins **5 passed / 0 failed**. Reds **0 passed / 12 failed**, stable across
three consecutive runs. Red suite wall clock ≈ **30 s** (dominated by key
generation and sockets) — budget for it.

| # | Baseline failure |
| :-- | :--- |
| R1, R2, R4, R9 | `oo node routing` does not exist — the probe cannot see the structure it asserts about |
| R3 | `early peer 0 of bucket 0 was not kept` (nothing is kept; there is no table) |
| R5 | answer length 0 vs 20 — `#find_node` returns nothing |
| R6, R7, R8, R10, R12 | `#find_node` → `#conflict` (unknown op) |
| R11 | **liveness**: `unrecognized subcommand 'find-node'` |

Note the fixture change made during calibration: with 220 random ids, bucket 5
is empty about **3% of the time**, which would have made R1 fail three runs in
a hundred for a reason unrelated to the code. The fixture now **draws until the
set covers ≥6 buckets** instead of asserting about the draw afterwards. A gate
that flakes is worse than no gate, and the failure mode would have taught its
next reader to re-run rather than to look.
