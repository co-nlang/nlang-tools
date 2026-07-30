# 歸屬聲明 / Affiliation claim — #3c-a work order

**Opened** 2026-07-30. **Baseline** `dev 8a8238e` (engine v0.3.0).
**Probes** `crates/oo/tests/affiliation_claim_probe_test.rs` — pre-committed and
calibrated before this order was written. Workspace at baseline: **1671 passed,
0 failed, 13 ignored** (10 of those ignored are this arc's reds; 3 are the
standing ones).

---

## 1. The problem

An operator runs several machines. REAL_02 §4.1.1 gives every workspace its own
node key and REAL_02 §220 forbids that key from being the operator's — so each
machine is an independent stranger to everyone else, including to its own
sibling. The operator has no way to say "these are mine" and no way to take it
back.

An **affiliation claim** is a signed statement by an operator that a node is
theirs. It rides inside the advert body, so the node signs it too.

### What it is not

* **Not 委任.** ORDER_01 §7.4 already means delegating administrative authority
  over a subpath to a `@Voter` with an explicit weight. No authority moves here.
* **Not 背書.** SPEC_13 §179 already means a governance authority vouching for
  the correctness of a `CAID_target`. This is about an identity relation, not
  the truth of a value.
* **Not Sybil resistance.** REAL_01 §7.6 row 5: that needs an external physical
  anchor and the lattice cannot supply one. An attacker simply does not sign, so
  the mechanism constrains only those who opt in. What it buys is Sybil
  *legibility* for an honest operator who chooses to disclose.
* **Not a trust root.** Deciding *which* operators matter, and what changes for
  a peer affiliated with one, is **#3c-b**. Nothing in this arc may consult a
  trust list or change any policy. See §7.

---

## 2. Normative wire format

The advert body gains an **optional** field. `required` in `oodp.rs` is a
presence check, so its absence stays legal and nothing about an unaffiliated
advert changes.

```
affiliation: {{
    operator_key: "<64 lowercase hex>",   ;; Ed25519 public key
    signature:    "<128 lowercase hex>",  ;; Ed25519 over the payload below
    expires:      <integer, seconds since the epoch>
}}
```

**Signed payload**, a new domain — the engine's third, after `oodp-advert:v1:`
(node key) and `refine:` (operator key, `authority.rs`):

```
oodp-affiliation:v1:<node_id>:<expires>
```

`node_id` and `expires` are both inside the payload, and both are load-bearing:

* without `node_id`, a genuine claim transfers to any node (**R6**)
* without `expires`, the holder extends its own claim (**R5**)

The domain carries `:v1:` because `refine:` not carrying one is a defect we do
not repeat.

**Maximum lifetime: 30 days** (`MAX_AFFILIATION_LIFETIME_SECS = 30 * 24 * 3600`,
same style as `STALE_SKEW_SECS` / `DISCOVER_STALE_SECS`). A claim whose
`expires` is beyond `now + MAX` is not honoured (**R7**). Without a ceiling,
ruling 4 buys nothing: an operator could issue a hundred-year claim and call it
short-lived.

**Spelling is normative here and the probe asserts it literally.** The standing
rule is "pin the property, not one spelling", and a wire format is its
exception: interoperability is a claim about bytes, and a second implementation
reading the spec must produce the same ones.

---

## 3. Scope — five deliverables

### 3.1 Minting: `oo node affiliate`

Produces a claim for *this workspace's* node id, signed with the **operator**
key (`OO_IDENTITY` / `~/.oo/identity`), and persists it so that serving can use
it later. Prints the claim.

* The key it signs with **must** be the key `oo identity` reports (REAL_01
  §7.5.2 可宣告性 — reporting X while signing with Y makes the operator publish
  a key that never signs, and the failure surfaces much later as "signer not in
  the registry", pointing at the wrong cause). **R1**.
* Minting a claim **is** an actual signing need, so minting the operator key
  here is permitted under §7.5.2 惰性.
* Default expiry: `now + MAX_AFFILIATION_LIFETIME_SECS`. An option to choose a
  shorter one is welcome; a longer one must be refused at mint time.

### 3.2 Where the claim lives

Beside the node key it is about: `{OO_NODE_HOME|~/.oo}/nodes/…`, **not** in the
workspace `.oo/`.

* **Constraint.** `kademlia_table_probe_test.rs:1149` pins the `.oo/` top level
  to an allow-list of `objects`, `format`, `peers`. A new entry there breaks a
  green pin in an untouched suite. This is not a reason to hide the file — it is
  the reason the node home is the right home: the claim is about the node
  identity, which already lives there.
* The claim is public material only (a public key, a signature, an integer), so
  it is not a secret. It still does not belong in the workspace, which exists in
  order to be copied.

### 3.3 Serving: `oo node advertise` attaches it

If a claim exists and has not expired, it goes into the advert body before the
node signs. **R2**.

* **The operator private key is never needed to serve.** This is the single
  property that keeps REAL_02 §220 intact: if serving needed it, an affiliated
  node would be a machine holding the operator's signing power, and copying the
  workspace would copy it. **R10** removes the identity file after minting and
  requires that serving still works, and that serving does not re-mint it.

### 3.4 Verification on all three paths

A claim arrives three ways, and verifying it on some but not all recreates
exactly the defect v0.2.54's acceptance repaired — *a signature record nobody
checks is not a signature record*.

| path | where | probe |
| :-- | :--- | :--- |
| direct `#advertise` | `oodp.rs` accept ladder | **R3, R4, R5, R6, R7** |
| relayed in a `#discover` answer | `verify_relayed_entry` | **R8** |
| loaded from `.oo/peers/directory` | `peers.rs` load | **R9** |

A claim is honoured only if **all** of: the operator signature verifies over
`oodp-affiliation:v1:<node_id>:<expires>` for *this advert's* `node_id`;
`expires` is in the future; and `expires <= now + MAX`.

**Additive only (ruling 3).** A claim is degree ≥1 — asserted, not verifiable
by the receiver in the sense that a CAID is. It must never cause anything to be
accepted that would otherwise be refused, and a broken claim must **not** reject
the advert: the node's own signature is still valid, so who it is was never in
doubt; only the affiliation is unproven. **P2, P3, R4, R5, R6, R7, R9.**

### 3.5 Observation: `oo node peers`

`oo node` today is serve / id / advertise / discover / find-node — there is no
way to see what the node believes about a peer, so without this the arc's result
cannot be observed and the order would be unsatisfiable.

Required semantics (the probe pins these, not a column layout):

* every known peer's node id is recoverable from the output
* the `operator_key` of a **verified** affiliation appears in that peer's entry
* an unverified, expired, over-long, mismatched or absent claim contributes **no
  operator string anywhere in the output**

The verdict is **derived, never stored** — see §4.

---

## 4. What must NOT change

Measured at reconnaissance, 2026-07-30, on v0.3.0:

* **The durable format does not change at all.** `PeerAdvert.ad_source` already
  holds the verbatim `%ad`, so the claim is *already* persisted with no code.
  The verified operator is a derived fact, and v0.2.54 ruled that derived state
  is rebuilt at load rather than stored (that is why the bucket index is not in
  the file). Recompute it; do not add a field. `decode_record_line` would
  tolerate a new key, and `FORMAT_TAG` uses `starts_with` — so **P6** is a real
  pin, and if the delivery decides to store the verdict anyway, P6 is what makes
  that a decision instead of a side effect.
* **No new objects, no universe movement.** SPEC_13 §4.1.2 obligation #3: an
  operator key is engine-local, non-deterministic state and must not be minted
  into the universe. **P4, P5.**
* **The operator private key never enters `.oo/`** (REAL_01 §7.5.1). **P7.**
* **Ordinary work still does not mint a node key** (REAL_01 §7.5.4 — reading at
  open is allowed, minting is not). Adding a CLI that signs must not turn
  `oo status` into a key-minting operation. **P8.**
* **The node signature still governs.** The claim must never become a path
  around the check that was already there. **P3.**
* **Compute the body CAID the way the engine already does** — through
  `~%Discovery./identify` (`identify_caid`), not a bare `content_hash()`. These
  differ; that is breaking-change entry #7 and it is not reopened here.

---

## 5. Probes

```
cargo test --test affiliation_claim_probe_test              # 2 controls + 8 pins, green now
cargo test --test affiliation_claim_probe_test -- --ignored # 10 reds, all red now
```

**Probe modification rights belong to the acceptor.** The delivery removes
`#[ignore]` and nothing else. If a probe looks wrong, say so in the report and
leave it failing — a red that turns out to be miscalibrated is a finding, and
three of them were, on this arc alone (§6).

At baseline every red fails at an honest, legible assertion, and **every red
that asserts an absence fails first at a presence guard**, so none of them can
pass by the observation surface staying broken.

---

## 6. Traps found at calibration — recorded so they are not re-derived

* **R5 was green at baseline, for the wrong reason.** It asserted only that an
  extended expiry is *not* reported. `oo node peers` does not exist, its output
  is a CLI error, the error does not contain the operator key, and so the
  absence held for a reason with nothing to do with expiry. Fixed by adding an
  in-window control claim that must be reported in the same run. The general
  form is now a standing rule: **every red that asserts an absence must assert a
  presence in the same run.**
* **P4's own premise was false.** `oo run` writes no objects — a store only has
  content after `evolve` + `commit`. The baseline was zero, so "the count did
  not change" would have held even with a broken walker. The `before > 0` guard
  caught it. (Note: `advert_persistence_probe_test.rs`'s P3 compares object
  counts with no such guard. Not touched here; recorded for the test-tidying
  arc.)
* **CLI spellings.** `oo node advertise` / `discover` take `--to`, not
  `--peer`. `oo node id` prints a bare CAID plus a `path:` line, not a quoted
  atom.
* **A CLI error is non-empty**, so `assert!(!out.is_empty())` does not prove a
  command ran. R8 now also rejects `error:` / `Usage:`.

---

## 7. Out of scope — #3c-b

Do not implement, and do not leave a hook for:

* a trust root / list of operators the node cares about
* any behaviour change based on affiliation: routing admission or eviction
  preference, directory retention, ranking, connection policy
* revocation lists of any kind (ruling 4: short lifetime plus renewal is the
  revocation mechanism)

SPEC_13 §4.1.2 obligation #3 already fixes where that trust root will live when
it comes — the assertion layer, out of band, beside `~/.oo/authorized_keys`,
never in `~%Official` — and that it is a *third* list, distinct from
`architect_registry` (governance) and REAL_02 §6.2's root-of-trust (package
blacklists, still unimplemented). Three lists is the honest answer because they
answer three different questions, but writing that down is #3c-b's job.

---

## 8. Acceptance measurements (acceptor's, not probes)

1. **Diff purity** — no probe edits beyond removing `#[ignore]`; no `git add -A`.
2. **Independent re-run** of the whole workspace, plus conformance and genesis.
3. **Interoperability with v0.3.0, both directions.** An unaffiliated v0.3.0
   node and this build must still exchange `#advertise` with `#success` both
   ways. Reconnaissance measured a nested unknown field as accepted on v0.3.0,
   so this arc is expected to be **incremental** — but it is measured, not
   assumed, and if it is breaking that is entry #8 and the 90-day clock moves.
4. **Directory size.** Reconnaissance measured +265 bytes per advert (462 →
   727, +57%). v0.2.54's number moves: 150 adverts, 131,568 B → ≈206,000 B,
   still far under R6's 1 MB bound in that suite. Re-measure for the
   ENGINE_SYNC record.
5. **`MAX_DISCOVER_PEERS = 8` still binds before the 64 KiB response budget** —
   it does today at 727 bytes per advert; confirm the delivery has not made an
   advert large enough to change that.

---

## 9. Ledger — known and deliberately not fixed here

* `#success` with no `%result` is still recorded as an integrity incident;
  needs a ruling on malformed *responses*.
* Unknown advert fields are accepted, relayed verbatim and persisted verbatim,
  bounded only by 64 KiB per request and the per-identity minting price of
  SPEC_15 §7.1. Pre-existing, not introduced by this arc — but this arc invites
  larger adverts, so §7.1's pricing should be revisited when #3c-b lands.
* `to_nlang` prints unforced Thunks as Rust `Debug`; `reader.read_line` is
  unbounded; `free_port()` is TOCTOU; `routing_id_from_digest` zero-pads.
