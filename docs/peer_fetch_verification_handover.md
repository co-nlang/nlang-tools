# Work order — peer-fetch address verification (REAL_03 §6.6 across every read path)

Arc opened 2026-07-26, immediately after v0.2.43.
Acceptor: project brain. Implementer: model #3.

---

## 0. The headline, measured verbatim on v0.2.43

A peer that answers every request with the same 61 bytes:

```
$ cat fabricated.json
{"Atom":[{"Str":"ATTACKER_CONTROLLED_NEVER_EXISTED"},0,null]}
```

A program that asks for an address which has never existed:

```nlang
conn: ~%Discovery./connect { 0: "Hostile", 1: "tcp://127.0.0.1:9934" }
got:  ~%Discovery./fetch   { 0: "Hostile", 1: "hash:sha256:v1:0000000000000000000000000000000000000000000000000000000000000000" }
```

```
$ oo run probe2.n --observe got
"ATTACKER_CONTROLLED_NEVER_EXISTED"
```

`remote_fetch` (`crates/interpreter/src/lib.rs:2249`) opens a socket, writes the
requested CAID, reads bytes, `serde_json::from_slice`, `Ok(val)`. The requested
hash is used to *ask* and never again. **Any CAID can be made to resolve to any
content by any peer you have connected to.**

Measured twice: once with fabricated bytes under a never-existing address (above),
and once with a genuine object served under a neighbouring address — asked for
`hash:sha256:v1:a80f42…`, received the content of `hash:sha256:v1:b80f42…`,
`"PEER_B_REAL_VALUE"`, no complaint.

v0.2.43 hardened the local store — the one place where you at least own the bytes —
and left the network path, where you own nothing, entirely unverified. The story
commit was "A path is not an identity". A socket is a path.

---

## 1. Spec basis — this is a compliance gap, not a spec change

REAL_03 §6.6 was committed 2026-07-26 (yesterday, `local 506cf37`). Its clauses
already cover every defect below. Nothing in this arc requires a new normative
clause; the engine simply does not implement the one that exists.

* **條款一(重算義務)**: 「以 CAID 取得內容的**每一條路徑**,必須於解碼後重算該內容的位址,並與**請求的** CAID 比對。」
* **§6.6 實作註記** names the hot paths verbatim, and 「**對等取用**」 is one of them.
* **條款三(三種結果必須可分)**: 「**尤其不得**將 `#caid_mismatch` 或 `#object_undecodable` 報告為「不存在」——那將使『庫完好』與『庫被竄改』在觀測上無法區分。」
* **條款四(消費端不得丟棄裁決)**: 「偵測若被呼叫端沉默丟棄則等同未偵測。」

And the property this arc restores is asserted as fact by **REAL_02 §3.1**:

> 來源不影響收斂結果——相同的 hash 無論從哪裡取得,內容必然相同。

The engine currently makes that sentence false.

**One floor under another.** SPEC_13 §7.2 requires an elaborate defence against the
Semantic Eclipse Attack: 隨機跳出 (fetch from nodes *outside* your trust lattice at
a 1/64 rate), 幾何張力, `#semantic_isolation`. That defence only hardens anything if
out-of-lattice bytes authenticate themselves. Without §6.6 on the network path,
隨機跳出 is not a defence — it is a channel handed to untrusted nodes.

---

## 2. Ruling Q1 — peers are 對等 at degree 0, 偏序 at degree ≥1

The question raised at arc open was whether two peers are equals or ordered.
Both, at different degrees, and the spec already says which is which:

* **偏序在「哪個 CAID」.** SPEC_13 §7.1: the trust poset $T$ applies 「當**別名衝突**發生時」 and picks 「優先坍縮至 `official` 提供的 **CAID**」. It resolves *names to identities* — degree ≥1, a question of authority and meaning.
* **對等在「這個 CAID 的位元組」.** Once a CAID is in hand, who hands you the bytes is irrelevant, because degree-0 verification is **self-authenticating**. The least-trusted peer returning correctly-addressed bytes gives you an object byte-identical to the most-trusted peer's.

**Therefore:**

* On verification failure, `~%Discovery./fetch` **continues to the remaining peers**.
  Continuing is safe precisely because peers are equals at degree 0 — a verified
  answer is the same answer whoever gives it. Aborting would hand any single
  malicious node a denial capability over every CAID.
* The failure is **never silently dropped** (條款四). It surfaces even when another
  peer subsequently answers correctly.
* If no source produces bytes that verify, the result is `⊥ (%cause: #caid_mismatch)`,
  distinct from absence.

**Config knob: rejected.** A knob may govern availability policy (how many peers,
timeouts, whether to quarantine a lying peer). It must not govern whether
verification happens or whether the verdict surfaces — those are MUSTs, and a
setting that switches off a MUST is not a setting. Separately, making a
*language-visible* outcome depend on engine-local configuration is exactly the
R2 ledger problem.

**Measurable consequence — verification is what makes the unordered peer set safe.**
`Ouroboros::peers` is a `HashMap` (`lib.rs:302`); `peers.values()` iterates in an
order that differs between processes. Today, two peers holding different content
for one CAID produce a **nondeterministic** result. After this arc the result is
deterministic, because only correctly-addressed bytes survive and those are
identical by definition. R6 below is that gate.

---

## 3. Ruling Q2 — scope: D1–D5

Every defect below was measured on v0.2.43. Two carry paired discriminators.

### D1 — network read path is entirely unverified `[條款一]`

`crates/interpreter/src/lib.rs:2249` `remote_fetch`. Measured: §0.
Callers: `disc.rs:139` (named peer), `disc.rs:160`, `disc.rs:325` (peer sweep).

### D2 — `NDP Miss` reported for a corrupt object `[條款三, verbatim]`

`crates/oo/src/main.rs:225`. Measured: tampered B's root object in place
(17 bytes replaced, length preserved), then requested its CAID over NDP.

```
NDP Request for CAID: hash:sha256:v1:b80f42…
NDP Miss:             hash:sha256:v1:b80f42…
```

The store is corrupt; the console says the object is not there. Serving 0 bytes
is correct — v0.2.43's local verification held and no lying bytes went on the
wire. What is wrong is the **report**: corruption is announced as absence, so the
one person who could repair it never learns. This is the sentence §6.6 條款三
points at.

### D3 — corrupt ≡ absent at the language surface `[條款三]`

`disc.rs:133 / 146 / 158 / 312 / 323`, all of the form `if let Ok(val) = …`.
Measured through a local peer:

```
fetch(corrupt object)  →  _|_ (%cause: #conflict)
fetch(absent CAID)     →  _|_ (%cause: #conflict)
```

Character-for-character identical. An n/ program can never distinguish "the
lattice does not hold this" from "my peer's copy is lying."

### D4 — shadow scan truncates silently; tampering buys a shorter audit `[條款四]`

`crates/interpreter/src/universe.rs:861` (`Err(_) => break`) and `:865`
(`Err(_) => { current = commit.parent; continue; }`).

Paired discriminator, measured on identical repos:

```
untampered                          → Shadow: 2 historical commit(s) …
after editing 3 bytes in one        → Shadow: 1 historical commit(s) …
  commit object ("d1" → "dX")
```

Same confident wording, no error, no warning. This is the `#refine` precedent of
v0.2.43 repeated one call site over: **tampering buys silence in the audit
report.** Note the audit direction — under-reporting shadow-affected commits
means a refine that rewrites history is presented as if it rewrites less.

### D5 — shadow report swallows its own read `[條款四]`

`crates/oo/src/main.rs:417` `if let Ok(commit) = engine.store.get_commit(&hash)`,
reading back the commit just written. Minor, but the same discard.

### Explicitly OUT of scope

* **REAL_02 §3.2 packet format.** Measured: NDP implements none of it. The spec
  says Request is an n/ Cocoon `{{ %op %hash %from }}` and Response carries
  `%status: #success | #not_found | #conflict`, `%result`, `%source`, `%hops`.
  The engine sends a bare CAID line and returns bare JSON. That is a real and
  large compliance gap; it is a protocol rewrite, not read-path verification.
  Ledgered, its own arc. **Do not start it here.**
* **Trust-poset ordering of peers** (SPEC_13 §7.1). Unimplemented; out of scope.
  This arc must not introduce an ordering — see §2, degree 0 does not need one.
* **`#semantic_eclipse` / 隨機跳出** (SPEC_13 §7.2). Out of scope.
* **`⊥ #conflict` as the cause for a plain absent CAID.** Pre-existing and
  arguably wrong (`#missing_key` would read better), but changing it moves a
  language-visible outcome with no clause behind it. Leave it. Only the
  *mismatch* case gains a new cause.

---

## 4. What to change

### 4.1 Verify on the network path (D1)

`remote_fetch` currently returns `Result<Value, BottomCause>`. After
deserialisation it must recompute the address of the decoded value and compare
against the requested hash, with the same version-dependent scope REAL_03 §6.6
條款二 fixes for the local store — **reuse `storage::value_address_matches`, do
not write a second comparator.** A second implementation of "is this the right
address" is a second thing to keep in sync, and the two would drift.

Three outcomes, same three as `StoreReadError`:

| outcome | meaning |
| :--- | :--- |
| verified | recomputed address equals requested address |
| mismatch | decoded, address differs — the peer is lying |
| undecodable | bytes arrived, decode failed — integrity undecidable |

"Peer sent nothing / connection failed" stays distinct from all three, and is
the only one that behaves like absence.

### 4.2 A new distinguishable cause (D1, D3)

Add `BottomCause::CaidMismatch` **at the tail** of the enum (append-only, exactly
as `StoreBoundary` was added in v0.2.42 — the ordinal is part of the wire form).
`as_tag()` → `"caid_mismatch"`; display `#caid_mismatch`; obstruction degree 1,
matching `StoreBoundary`.

Reaches the language surface from `disc.fetch` and `disc.find` when no source
produced verifying bytes and at least one source produced *mismatching* bytes.
If every source simply lacked the object, the existing absent behaviour is
unchanged.

Do **not** add a separate cause for undecodable; fold it into `#caid_mismatch`
at the language surface but keep the distinction in the incident record (§4.4).
Rationale: `#object_undecodable` is a store-read outcome in REAL_03 §8; the
language surface needs "these bytes are not this identity", and splitting it
buys a second tag with no clause requiring it. Raise it with the acceptor if
you disagree — do not decide it silently.

### 4.3 Continue, do not abort (D1, ruling Q1)

In every peer sweep (`disc.rs` named-peer, unnamed sweep, and `disc.find`),
a mismatching peer is skipped and the sweep continues. The final `⊥` carries
`#caid_mismatch` if any source mismatched, and the pre-existing cause if all
sources merely lacked it.

The named-peer form (`fetch { 0: "PeerName", 1: caid }`) has exactly one source;
mismatch there is immediately `⊥ #caid_mismatch`.

### 4.4 Surface the verdict (D1, D3, 條款四)

`nlang-interpreter` contains no `eprintln!` anywhere — the library is silent by
design and the CLI prints. Keep that.

Add to `Ouroboros`:

```rust
pub integrity_log: RwLock<Vec<IntegrityIncident>>,
```

recording, per incident: the requested CAID, the source (peer name / address /
`local`), and which of {mismatch, undecodable} occurred. `oo run` / `oo evolve` /
`oo repl` print accumulated incidents to **stderr** after evaluation, whether or
not the fetch ultimately succeeded — that is the whole point of 條款四.

Keep the record minimal. It is not a general diagnostics framework and must not
become one in this arc.

### 4.5 Report corruption as corruption (D2)

`main.rs:225`: distinguish `StoreReadError::NotFound` from the other two.
`NDP Miss` stays only for genuine absence. Corruption prints a distinct line
naming the CAID and the outcome.

**The wire stays as it is** — still 0 bytes to the peer. Whether the protocol
should carry `%status: #conflict` is the REAL_02 §3.2 arc, not this one.

### 4.6 Do not truncate the shadow scan silently (D4, D5)

`universe.rs:861/865`: split on `StoreReadError` exactly as the v0.2.43 `refine`
repair did (`universe.rs:816`, read it first — same shape, same reasoning).

* `NotFound` / non-`StoreReadError`: current behaviour (opaque, REAL_03 §9.1).
* `CaidMismatch` / `ObjectUndecodable`: the scan's result is no longer complete.
  It must not be printed as though it were.

Ruling on what "not silently" means here: **the refine must not be blocked** —
a corrupt object elsewhere in history is not grounds to refuse an unrelated
refine — but the shadow report must state that the scan was truncated and at
which commit, and the incident goes in the integrity log (§4.4). An audit
surface that cannot be distinguished from a complete one is not an audit
surface (v0.2.41 `#squash` precedent).

`main.rs:417` (D5): same split; a failed read-back of the commit just written is
reported, not swallowed.

---

## 5. Probes — pre-committed, do not modify

`crates/oo/tests/peer_fetch_verification_probe_test.rs`, written and calibrated
by the acceptor **before** this order was issued.

**Probe modification rights belong to the acceptor.** The implementer removes
`#[ignore]` and nothing else. If a probe looks wrong, say so and stop — do not
adjust it. Two arcs in a row (v0.2.39, v0.2.42) cost a revert because a delivery
accommodated an acceptor error instead of reporting it; v0.2.43's delivery
reported one instead, at zero cost. Report.

| gate | asserts | baseline |
| :--- | :--- | :--- |
| R1 | fabricated bytes under a never-existing CAID are not returned | red |
| R2 | a real object served under the wrong CAID is not returned | red |
| R3 | corrupt and absent are distinguishable at the n/ surface | red |
| R4 | NDP serve does not report a corrupt object as `Miss` | red |
| R5 | a tampered commit does not silently shorten the shadow report | red |
| R6 | one honest peer among liars is found deterministically | red |
| P1–P6 | honest fetch, local fetch, absence, `inspect`, untampered shadow, connect | green, must stay green |

Every red gate asserts **first** that the operation actually happened (the
hostile peer logged the request; the tampered object is on disk and decodable;
the baseline shadow report is non-empty) and only then asserts the invariant.
Five arcs running, the recurring calibration failure has been a gate that goes
red because the operation never ran.

R6 is the one gate with a probabilistic baseline: two lying peers and one honest
peer, 10 runs, `HashMap` iteration order varying per process. Probability of a
vacuous baseline pass ≈ 3⁻¹⁰. After the fix it is deterministic.

---

## 6. Acceptance

1. **Diff purity** — nothing outside the read paths named in §3. In particular
   no peer ordering, no packet-format work, no new config knob.
2. **Four numbers** re-run from a clean build: workspace, conformance (143),
   genesis (11), plus this arc's probe file.
3. **Address stability** — no existing CAID moves. Measure genesis + the full
   conformance corpus before and after and compare verbatim. This arc is
   read-side only; any movement means something else changed.
4. **Adversarial pass** by the acceptor, including the paired discriminators
   re-run against a v0.2.43 worktree to confirm the gates are still red at
   baseline after any repair.

Classification is expected to be **修正** (an existing normative clause was not
implemented), not 增量 — but that is the acceptor's call at closure, and the new
`BottomCause` tail may move it. Do not write the CHANGELOG entry; spec closure is
the acceptor's step. (v0.2.43's delivery committed `ERROR_CODES.md` itself; the
content was right and the order was ambiguous. This order is not: **do not touch
`nlang-spec`.**)

---

## 7. Ledgered during this arc's measurement — not this arc's work

### L1 — stored values carry unforced thunks with source spans

The committed root stores a literal field as an **unforced `Thunk`** carrying its
source span:

```json
"payload": { "Combo": { "data": { "hello": { "Thunk": {
    "expr": { "kind": {"Atom": {"Str": "world"}},
              "span": {"start": 18, "end": 25} },  ← byte offsets into a.n
    …
```

Two consequences, both measured:

1. **The shadow scan can only ever match already-forced fields.** `fv.content_hash()`
   over a thunk cannot equal the forced value's CAID. A literal-valued source
   silently yields an empty shadow report. This is why `shadow_universe()` in the
   probe file goes out of its way to build a forced field — without it R5 would be
   vacuous.

2. **Identity is contaminated by formatting.** Two sources differing only by two
   blank lines and a comment:

   | | fmtA | fmtB |
   | :--- | :--- | :--- |
   | `lattice_sketch` | `///siKXKwpMgAP+fpK+Qx/vNBQD/…` | identical |
   | `content_digest` | `b1 87 94 be 69 cc ff 37 …` | `57 d7 e0 ea 34 01 99 36 …` |

   The spectral feature recognises the two universes as the same geometry. The
   content digest does not. REAL_03 §7.1 specifies the hash traversal as feeding
   「每個節點的型別標籤、名稱與譜特徵」 — a source span is none of the three.
   (Measured against root **value** CAIDs, not commit CAIDs: `CommitMeta` carries
   a timestamp, so commit CAIDs differ unconditionally and prove nothing.)

Its own arc. Adjacent to the ledgered **型別層急切展開** (a two-line recursive type
becomes 260 KB at evolve) — one is too eager, the other too lazy, and both are the
same question: *what exactly is in the store?*

### L2 — NDP implements none of REAL_02 §3.2

Spec: Request is an n/ Cocoon `{{ %op: #discover|#fetch|#advertise, %hash, %from }}`;
Response carries `%status: #success | #not_found | #conflict`, `%result`, `%source`,
`%hops`. Actual: a bare CAID line in, bare JSON out, no status, no source, no hops.
Note that the spec's response *already has* a status vocabulary that could carry a
verification verdict on the wire — which is why §4.5 keeps the wire unchanged here
rather than inventing a third thing.

---

## 8. Delivery record (delivery side)

- **Tip**: see commit (recorded after).
- **D1** `remote_fetch`: deserialize → `value_address_matches` (shared with store)
  → `Ok` / `Err(CaidMismatch)` / empty→`Err(Conflict)`. Incidents on mismatch &
  undecodable.
- **`BottomCause::CaidMismatch`** enum tail; `as_tag` → `caid_mismatch`.
- **D3 / Q1** `disc.fetch` / `disc.find`: continue past lying sources; named peer
  mismatch → immediate `#caid_mismatch`; sweep with only mismatches → same;
  plain absence unchanged (`#conflict`).
- **§4.4** `Ouroboros::integrity_log` + `record_integrity` / `take_integrity_incidents`;
  CLI prints to **stderr** after `run` / `evolve` / `refine`.
- **D2** `oo serve`: `NDP Miss` only for `NotFound`; corruption prints
  `#caid_mismatch` / `#object_undecodable` (wire still 0 bytes).
- **D4** shadow scan: NotFound/opaque → prior behaviour; mismatch/undecodable →
  record + truncate note (refine still succeeds).
- **D5** refine shadow report: failed read-back of just-written commit reported.
- **Probe**: only 6 `#[ignore]` removed.
- **Gates**: peer_fetch probe **12/12**; workspace **1475/0/3**; conf **143/143**;
  genesis **11/11**; release clean of new warnings; `seed_caids_are_stable` ok.
- **Not touched**: nlang-spec, NDP packet format, peer ordering, config knobs.
