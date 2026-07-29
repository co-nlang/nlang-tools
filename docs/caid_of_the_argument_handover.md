# The CAID of x is the CAID of x — work order

`~%Discovery./identify x` returns the CAID of the argument pack `apply_morphism`
builds, not of `x`. SPEC_13 §6.1 says it returns 「指定節點的**內在 CAID**」 and
REAL_02 §4.2 says an advertisement signature commits to `CAID(本體)`. Neither
holds today.

**Breaking**: the signature payload moves. A v0.2.55 node and a node built from
this arc will not verify each other's advertisements.

---

## 1. What was measured, on v0.2.55

* **The print → parse → eval round trip preserves the CAID.** 18 values, 17
  identical (the exception is a lazy container, whose `to_nlang` prints
  unforced Thunks as Rust `Debug` — the standing L1 ledger item, unrelated).
  So **every** part of the discrepancy comes from the argument pack, and none
  from the round trip. This is what makes the arc small.

* **The discrepancy, measured over 17 shapes** (2026-07-29): 12 hash
  `{{0: v}}`; 2 (tuples) hash `v` itself; 3 (combos with named fields,
  including an advertisement body) hash `{{0: v, …v's named fields lifted}}`.
  For the advertisement body: `identify` gives `db5dc4d2…`, the value's own
  CAID is `706000a8…`.

* **The calling convention is ambiguous, and that is the root.**
  `~%Math./add 1 2` and `~%Math./add (1, 2)` both give `3`;
  `identify (1,2)` and `identify {{0:1, 1:2}}` give the same CAID. A builtin
  cannot recover its argument, because `apply_morphism` flattens
  "applied to a pair" and "applied to two things" into one shape.

* **`%arg` is read and never set.** `lib.rs:1395`'s `is_arg_pack` checks
  `ac.contains_key("%arg")`, and nothing anywhere writes that key. The hook
  exists; nobody connected it.

* **LADD keys are process memory.** `gbb_registry` is never persisted and never
  sent on the wire. Moving those keys costs nothing.

* **The naive unwrap is already shipped, and it is losing data.**
  `engine.save` (engine.rs:128) does exactly what §2 rejects:
  `c.get_field("0").cloned().unwrap_or(arg)`. For a wrapped argument that is
  right; for an argument that was already pack-shaped it takes the first
  element. Measured:

  ```
  ~%Discovery./identify_and_store (1, 2)   stores 1, returns 1's address
  ~%Discovery./identify_and_store {{0: 9}} stores 9, returns 9's address
  oo inspect <that address>                prints 1
  ```

  **The store confirms it saved something it did not save.** This is worse
  than the CAID discrepancy — it is silent data loss on the write path, and it
  is the exact failure mode the marker prevents. It is also the strongest
  argument for the ruling: the alternative was not hypothetical, it was
  already in the tree.

* **Four probe suites sign through the language surface.**
  `advertise_wire`, `discover_index`, `kademlia_table` and `advert_persistence`
  each have a `caid_of` helper that shells out to
  `oo eval ~%Discovery./identify …`. This is load-bearing for the arc's shape —
  see §6.2.

---

## 2. Ruling

Owner, 2026-07-29: **set the marker and remove the ambiguity**, rather than
fix the one symptom.

`apply_morphism` sets `%arg` on the pack **only in the branch that wraps** —
the `else` that places the argument into a positional slot. A tuple, or any
argument already shaped like a pack, takes the other branch, is not wrapped,
and carries no marker.

So the two cases become distinguishable, with no residue:

| what was applied | pack | `%arg`? | the argument is |
|---|---|---|---|
| `f v` (v not pack-shaped) | `{{0: v, …lifted}}` | **yes** | slot `0` |
| `f (a, b)` | `{{0: a, 1: b}}` | no | the pack itself |
| `f {{0: x}}` | `{{0: x}}` | no | the pack itself |

A builtin that consumes **the whole argument** reads slot `0` when `%arg` is
present and the pack itself when it is not.

---

## 3. Design

### 3.1 The marker

* Set in `apply_morphism`'s wrapping branch only. **Not** in the `is_arg_pack`
  branch, and **not** unconditionally — that would erase the very distinction
  this arc exists to restore.
* `%`-prefixed, so it lands in `ComboVal`'s `meta` bucket. Note that
  `fields()` still surfaces it with the prefix restored, so every site that
  iterates the pack must be checked (§3.3).

### 3.2 The three whole-argument builtins

`disc.rs` — `disc.identify` (262), `disc.advertise` (267–268, hash **and**
`compute_mass`), `disc.find` (312–315, same pair). Each unwraps first, then
hashes. Mass is a property of the value, not of how it was passed.

Consequences, all of them intended:

* `~%Discovery./identify x` returns `x`'s CAID — SPEC_13 §6.1.
* LADD advertise/query keys become the value's CAID, so **the LADD address
  space and the CAS address space coincide**. An n/ program can finally compute
  where a value lives.
* The advertisement signature payload becomes `"oodp-advert:v1:" ++ CAID(body)`
  — REAL_02 §4.2, literally.

### 3.3 Every other site that reads the whole argument

Thirteen more: `diff.rs` ×6, `disc.rs` ×3 helpers, `query.rs` ×1, `toml.rs` ×1.
Each reads operands out of slots, not the pack itself — **verify that, one by
one, and say so in the delivery record**. If any reads the pack, it needs the
same unwrap. A `%arg` leaking into a diff or a TOML dump is the failure mode.

### 3.4 What must not change

`identify_caid` / `identify_caid_src` in `oodp.rs` keep going through the
morphism. They do not need to bypass it once the morphism is correct, and
routing them around it would leave the language surface wrong while making the
protocol right — which is the split this ruling rejected.

---

## 4. Deliverables

1. `%arg` set in `apply_morphism`'s wrapping branch only.
2. The three `disc.rs` whole-argument builtins unwrap before hashing and
   before `compute_mass`.
2a. **`engine.save` unwraps by the marker, not by `get_field("0")`.** It is
   losing data today (§1) and must stop.
3. The other thirteen sites checked and reported (§3.3).
4. Un-`#[ignore]` the reds in `crates/oo/tests/caid_of_the_argument_probe_test.rs`.

## 5. Out of scope

* Any change to `oodp.rs`'s signing/verifying helpers.
* Making `to_nlang` round-trip lazy containers (standing L1 ledger item).
* Anything about `%reason`, peers, or storage.
* Spec, ERROR_CODES, CHANGELOG — acceptance.

## 6. Gates

`crates/oo/tests/caid_of_the_argument_probe_test.rs`, pre-committed.
**Probes belong to the acceptor**: remove `#[ignore]`, change nothing else.

### Reds

| # | Name | Holds |
|---|---|---|
| R1 | `identify_returns_the_caid_of_the_value` | scanned over many shapes: `./identify v` equals `v`'s own CAID, for every shape that is not pack-shaped |
| R2 | `a_tuple_is_still_its_own_value` | `./identify (1,2)` is the tuple's CAID — the marker did not erase the other branch |
| R3 | `a_combo_with_slot_zero_is_still_itself` | `./identify {{0: 5}}` is that combo's CAID, not `5`'s |
| R4 | `the_ladd_key_is_the_cas_address` | advertise a value, then find it by the CAID the store uses |
| R5 | `the_signature_commits_to_the_body_caid` | probe recomputes `CAID(body)` from the wire bytes with no help from `./identify`, signs with it, and the node accepts |
| R6 | `storing_a_tuple_stores_the_tuple` | `identify_and_store (1,2)` must not store `1` — the shipped naive unwrap |
| R7 | `storing_a_slot_zero_combo_stores_the_combo` | same for `{{0: 9}}` |

### Pins

| # | Name | Holds |
|---|---|---|
| P1 | `multi_argument_builtins_are_unchanged` | `add 1 2` = 3, `add (1,2)` = 3, curried `add 1` then `2` = 3 |
| P2 | `the_marker_never_reaches_a_user_visible_value` | no observation anywhere prints `%arg`; a combo passed through a morphism comes back without it |
| P3 | `store_round_trip_is_unchanged` | a value's CAS address does not move |
| P4 | `diff_and_toml_do_not_see_the_marker` | the two whole-argument iterators over operands are unaffected |

**Calibration removed a red.** `a_v0255_advertisement_is_refused` asserted
that the pack's CAID and the body's CAID differ — which is true *today*, so it
was green at baseline for the wrong reason. Breaking-ness is measured at
acceptance with two binaries (§7), not asserted by a probe that cannot build
the other one.

### 6.1 No pin is scheduled to change

The mechanical grep (`grep -rn "identify" crates/*/tests/*.rs`) found no probe
asserting the current pack-CAID behaviour. Nothing is scheduled to change.

### 6.2 The four `caid_of` helpers must **not** be edited

`advertise_wire`, `discover_index`, `kademlia_table` and `advert_persistence`
sign through `oo eval ~%Discovery./identify`. Because this arc moves the
language surface and the protocol **together**, those helpers stay correct
without being touched, and all four suites must stay green unedited.

**This is a gate, not a note.** If the delivery finds itself editing a
`caid_of`, the fix has drifted apart — the protocol has moved and the language
surface has not, which is precisely the split the ruling rejected. Report it
and stop.

### 6.3 Classification

**破壞性 (Layer 1)**. The signature payload moves, so a v0.2.55 node and this
one reject each other's advertisements. R6 measures it in at least one
direction; acceptance measures both. This is the seventh breaking entry and it
restarts ORDER_00 §5.1.4's clock, which is 2026-07-27 and therefore cheap.

## 7. Acceptance numbers

* the shape scan of R1, in full
* `./identify v` vs the CAS address, for the advertisement body specifically
* both cross-version directions with the actual `%reason` on the rejection —
  this is where the breaking claim is established, with a v0.2.55 binary and
  this one, in both directions
* `identify_and_store` on a tuple and on a slot-0 combo, before and after
* every suite, plus conformance and genesis

## 8. Ledger — not this arc

1. `to_nlang` prints unforced Thunks as Rust `Debug`, so a lazy container does
   not round-trip. Measured again here; unchanged since L1.
2. `#success` with no `%result` is still recorded as an integrity incident
   (wire_says_why's leftover; needs a ruling on malformed *responses*).
3. `reader.read_line` unbounded; `mod advert_debug`; `free_port()` TOCTOU;
   `routing_id_from_digest` zero-pads; `| head` SIGPIPE.
4. **Node endorsement** is unspecified and measured non-breaking — its own
   增量 arc, and it must not be called 委任.

## 9. Delivery record (delivery side)

### Built

- **`%arg` marker**: set only in `apply_morphism`'s wrapping branch (non-
  pack-shaped argument placed in a positional slot). Pack-shaped arguments
  (tuples, `{{0:…}}`) take the other branch and carry no marker. Partial
  morphisms strip `%arg` when materialised so it never becomes a user-visible
  field.
- **`whole_argument(arg)`** (`value.rs`): if `%arg` present → slot `0`, else
  the pack/value itself.
- **Whole-argument builtins** unwrap before hash / mass / store:
  - `disc.identify`
  - `disc.advertise` (+ `compute_mass` / nerve on the unwrapped value)
  - `disc.find` (unwrap; CAID **string** used as query node id / direct
    target so LADD key = CAS address, R4)
  - `engine.save` (`identify_and_store`) — no more unconditional `get_field("0")`
- **`oodp.rs` signing helpers**: unchanged; still go through
  `~%Discovery./identify`, which is now correct (ruling: no split).
- **Four `caid_of` helpers**: not edited (gate §6.2). All four suites green.
- **Spec / CHANGELOG**: not edited. **Breaking** Layer 1 (signature payload).

### §3.3 site-by-site (operand readers — no pack hash)

| Site | Reads | Verdict |
| --- | --- | --- |
| `diff.diff` / `patch` / `is_compatible` | slots `0`,`1` | ok — never hashes pack; `%arg` stays in meta |
| `disc.connect` | slots `0`,`1` | ok |
| `disc.fetch` | slots `0` / `0+1` | ok |
| `query.select` / `where` / `pluck` / `deep_merge` | slots `0`,`1` | ok |
| `toml.parse` / `stringify` | slot `0` as string payload | ok — operand extract, not whole-value CAID |
| `engine.save` | **fixed** via `whole_argument` | was the data-loss path |
| remaining `engine.*` `get_field("0")` | multi-arg Option/Result helpers | multi-slot APIs; not whole-argument CAID consumers (not in the three + save set) |

No site iterated the pack as a value in a way that would emit `%arg` into a
user result (P2/P4 green).

### Measurements

- **R1**: all 13 SHAPES — `identify` = store address (including advert body).
- **Advert body CAID** (was pack `db5dc4d2…` on v0.2.55): now equals own
  value digest  
  `706000a89dc79e0d9a4a538c787eb4db31b848311e80b2c81da52aeb4108ce72`
  (matches the pre-arc measurement of the value's own CAID).
- **R2/R3**: tuple and `{{0: 5}}` keep their own CAIDs (marker did not
  erase the non-wrap branch).
- **R6/R7**: `identify_and_store (1,2)` and `{{0: 9}}` store the whole value.
- Cross-version both ways: acceptance with a v0.2.55 binary (signature payload
  moves; expect mutual `#rejected` / bad_signature). Not run in this delivery
  environment without a pinned older binary on PATH.

### Numbers

| Suite | Result |
| --- | --- |
| caid_of_the_argument | **12/12** |
| advertise_wire | **19/19** (caid_of untouched) |
| discover_index | **17/17** |
| kademlia_table | **17/17** |
| advert_persistence | **19/19** |
| wire_says_why | **16/16** |
| oodp_packet_format | **13/13** |
| local_gc | **17/17** |
| workspace | **1661 / 0 / 3** |
| conf | **143/143** |
| genesis | **11/11** |

### Left

Ledger §8 (lazy `to_nlang`, etc.). Older engine naive `get_field("0")` paths
outside save remain (same class of bug if ever used as whole-argument stores);
not in this arc's three + save set.
