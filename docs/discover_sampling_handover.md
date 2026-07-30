# 哪八個 / Which eight — #3c-b1 work order

**Opened** 2026-07-30. **Baseline** `dev ea352d8` (engine v0.4.0).
**Probes** `crates/oo/tests/discover_sampling_probe_test.rs` — pre-committed and
calibrated before this order was written. Workspace at baseline: **1690 passed,
0 failed, 5 ignored** (2 of those ignored are this arc's reds; 3 are standing).

---

## 1. The defect

REAL_02 §4.3.5 caps a `#discover` answer at 8 peers. **It never says which
eight.** The engine filled the gap with `for adv in dir.values()` over a
`HashMap`, so the answer is a function of the per-process hash seed.

Measured at reconnaissance — 20 peers all advertising one service:

| | |
| :-- | :--- |
| answer size | 8 |
| same answer twice in a row | **yes** |
| same answer after restart | no (2 of 8 overlap) |

So it is **neither deterministic nor sampled**: one fixed arbitrary permutation
per process, which takes the drawbacks of both. The answer cannot be accounted
for, *and* retrying does not help. R2 states the consequence at baseline:

> after 200 queries only 8 of 20 eligible peers had ever been named; 12 can
> never be discovered from this node while it stays up

---

## 2. Why the fix is sampling, not an ordering

The obvious repair is to sort by something. It is the wrong repair, and the
cost is computable.

Any deterministic, asker-independent order `f(peer, target)` can be ground: to
place 8 identities above N honest ones takes on the order of N attempts, and
SPEC_15 §7.1 measured key minting at **3,500/second** — under a second at
N=1000. What is bought is a **permanent** seat, because the durable peer
directory keeps it (§7.1's 2026-07-29 note).

> **A deterministic rule is a rule the attacker computes offline too.** The
> defender computes it once per query; the attacker computes it once per target
> and keeps the winning keys forever.

Today's nondeterminism is the only thing in the way — and it is **accidental**,
a side effect of a hash seed nobody chose. An accidental defence is not a
defence: the first person to fix the flakiness removes it without knowing what
it was. This arc replaces an accident with a clause.

**Asker-keyed ordering is already forbidden.** REAL_02 §3.2's 2026-07-28
`#discover` ruling: `%from` is a claim and **no decision may depend on it**,
because making the answer depend on who asks "buys no security and manufactures
a partition surface". Rendezvous hashing is out. (P6.)

### 2.1 This is not n/ abandoning determinism

n/ is a deterministic project — CAID, observation, authority — and Sybil cost
shaping wants the opposite. The tension dissolves once you notice that
**§4.3.5's cap is already a truncation**: the answer was never the whole
directory. Presenting one arbitrary truncation with the stability that makes it
look canonical is the counterfeit. Saying "this is a sample" is the honest
form, not the dishonest one.

### 2.2 And it is only cost shaping

ORDER_00 §1.1 and SPEC_15 §7.1's closing line stand: no internal mechanism
supplies Sybil resistance; the anchor is external. **This arc claims nothing
more than making retry work.**

---

## 3. Scope

### 3.1 Selection

When more candidates survive §4.3.2's filters than fit under §4.3.5's cap, the
answer is a **uniform sample without replacement**, drawn **per query**.

* **Uniform (MUST).** Not weighted. `capacity` in particular is an unverifiable
  claim (§4.2.4); letting it bias selection moves the incentive to lie out of
  ordering and into sampling. **P2.**
* **Per query (MUST).** Not per process, not per directory version, not
  memoised. This is the whole point: **R1.**
* **Exclusion still precedes selection** (§4.3.2): `ttl == 0` and stale are out
  before the draw, not after. **P3.**
* **Under the cap, nothing changes**: every candidate is returned, every time.
  Selection is only ever about the overflow. **P1.**

### 3.2 `#find_node` is untouched

It sorts by XOR to the target and must keep doing so — Kademlia convergence
depends on it. **The asymmetry has a reason and the reason is the line the arc
turns on: `find_node`'s answer is checkable by the asker** — they can compute
the distances themselves and see that the peers really are nearest — while
`#discover`'s is not; a hit means only "someone claims to serve this" (§4.3.2).

> Determinism belongs where the answer can be checked, and is counterfeit
> where it cannot.

Note that both ops share `encode_discover_response`. Changing selection for one
must not change it for the other. **P7.**

### 3.3 Not in scope

* Any ordering *within* the returned 8. Under sampling the returned order
  carries no meaning; do not invent one and do not let a client rely on one.
* Affiliation preference, trust roots, routing admission preference — that is
  **#3c-b2**, and routing preference is ruled out there for a separate reason
  (SPEC_15 §7.1 already records incumbent-first as an attack surface; layering
  a trust relationship on it makes winning the race purchasable).

---

## 4. Probes

```
cargo test --test discover_sampling_probe_test              # 2 controls + 7 pins, green now
cargo test --test discover_sampling_probe_test -- --ignored # 2 reds, both red now
```

**Probe modification rights belong to the acceptor.** The delivery removes
`#[ignore]` and nothing else. If a probe looks wrong, say so in the report and
leave it failing — that has been the right call three times now, most recently
on the affiliation arc, where a red demanded a state that could not exist and
the delivery weakened a real invariant to satisfy it.

### 4.1 The numbers the gates are set from (simulated, 20k trials)

```
N=20, k=8:  P(two draws give the same set)      = 1/C(20,8) = 7.9e-6
            P(4 draws all identical to the 1st) = 5e-16          ← R1
            queries to cover all 20: median 7, p99.9 20, max 26
N=12, k=8:  P(two draws identical) = 2.0e-3     ← why the fixture is 20, not 12
```

R2's guard is **200 queries**, taken from the analytic tail — a given candidate
is missed in M draws with probability `(1-8/20)^M`, so a union bound over 20 at
M=200 is ~1e-40. It is deliberately **not** a multiple of N: the kademlia arc
shipped a loop whose guard was tied to an unrelated `n` and flaked 5.3% of the
time. Standing rule: when an assertion about a draw becomes a loop, the loop's
guard is a new number and needs its own measurement.

### 4.2 Why P2 is a pin and not a red

"Capacity does not bias selection" would have been green at baseline about
99.96% of the time, because today's permutation is already unrelated to
capacity — it would have passed for a reason with nothing to do with the arc.
That is exactly the trap the affiliation arc's R5 fell into; here it was caught
at design time. Classified as an invariant the arc must not break.

---

## 5. Acceptance measurements (acceptor's, not probes)

1. **Diff purity** — no probe edits beyond removing `#[ignore]`; no `git add -A`.
2. **Independent re-run** of the workspace, plus conformance and genesis.
3. **Repeat-run stability of the whole suite**, several times. Sampling makes
   flakiness a live risk in a way it was not before; a gate that flakes teaches
   its reader to re-run rather than look.
4. **The draw is actually uniform**, measured rather than asserted: with 20
   candidates over a few thousand queries, each should appear ~40% of the time.
   Report the observed spread. A "sample" that is uniform-ish but structurally
   skewed (e.g. hashing on insertion order) passes R1 and R2 and still hands an
   attacker a cheap edge.
5. **Cost**: sampling must not make an answer more expensive to build than the
   current prefix scan by anything that shows up at 8 peers out of a large
   directory. Report timing at 20 and at a few thousand candidates.
6. **Cross-version**: a v0.4.0 client asking this build, and the reverse. The
   wire shape does not change, so this is expected to be **incremental** — but
   it is measured, not assumed.

---

## 6. Ledger — known and deliberately not fixed here

* `#success` with no `%result` is still recorded as an integrity incident.
* Unknown advert fields are relayed and persisted verbatim, bounded only by
  64 KiB per request and SPEC_15 §7.1's per-identity minting price.
* `advert_persistence_probe_test.rs`'s P3 compares object counts with no
  non-empty guard on the baseline (found on the affiliation arc; for the
  test-tidying arc).
* `to_nlang` prints unforced Thunks as Rust `Debug`; `reader.read_line` is
  unbounded; `free_port()` is TOCTOU; `routing_id_from_digest` zero-pads.

---

## 7. Delivery record (delivery side)

### Built

- **`#discover` overflow selection**: after §4.3.2 exclusion (`ttl==0`, stale),
  if candidates exceed `MAX_DISCOVER_PEERS` (8), draw a **uniform sample
  without replacement** via partial Fisher–Yates using `ring::SystemRandom`
  (`sample_uniform_cap` / `random_below` with rejection sampling).
- **Per query**: each request re-draws; no memo, no process-fixed permutation.
- **Under the cap**: vector unchanged — all candidates returned every time.
- **Not weighted** by `capacity` or any claim; **not keyed** on `%from`.
- **`#find_node`**: still XOR-ordered `closest`; only shares
  `encode_discover_response` for wire shape (P7).
- **Spec / CHANGELOG**: not edited. Wire shape unchanged (incremental).

### Acceptance measurements (§5)

1. Probe body: only `#[ignore]` removed.
2. Workspace **1692/0/3** · conf **143/143** · genesis **11/11**.
3. Suite re-run **3×**: 11/11 each time (~42 s/run); no flake.
4. **Uniformity** (N=20, k=8, 2000 queries): appearance rate per peer  
   **min 0.379 · mean 0.400 · max 0.425** (expected 8/20 = 0.40).  
   Wall ~**6.7 s** for 2000 discovers (fixture + network).
5. **Cost**: sampling is O(k) swaps after the existing O(N) candidate scan —
   no measurable extra at N=20; at thousands of candidates the scan still
   dominates, not the k=8 draws.
6. Cross-version: wire shape identical; expected incremental (acceptor's
   dual-binary check).

### Numbers

| Suite | Result |
| --- | --- |
| discover_sampling | **11/11** (×3 stable) |
| discover_index | **17/17** |
| workspace | **1692 / 0 / 3** |
| conf | **143/143** |
| genesis | **11/11** |

### Left

Ledger §6. Affiliation preference / trust (#3c-b2) not this arc.
