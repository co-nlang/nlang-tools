# Work order — CAS read integrity

**Arc**: a path is not an identity.
**Date**: 2026-07-26. **Baseline**: v0.2.42 (`top 3aa5710`).
**Probes (pre-committed, calibrated)**: `crates/oo/tests/cas_integrity_probe_test.rs`
— 8 red (`#[ignore]`, each verified red *for the right reason*) + 4 pin (green
now, must stay green).

**Probe modification rights belong to the acceptor.** Your only permitted edit
is removing `#[ignore]`. If a gate looks wrong, report it — do not adjust it.
One of the eight was vacuous on the first pass and was repaired before this
order was written; the note is in the file.

---

## 1. The headline, measured verbatim

```
$ oo inspect hash:sha256:v2:_:/5+TjN...:b0e7f5bd...
CAID:   hash:sha256:v2:_:/5+TjN...:b0e7f5bd...
MASA:   _
Sketch: /5+TjN/DktgqAICg+47dgbiuWQD//+eC...

{ a: 1, b: "two" }      ← before the bytes on disk were edited
{ a: 1, b: "XXX" }      ← after; same command, same CAID, printed above it
```

`read_object` derives a path from the digest and returns whatever is there.
Nothing re-addresses the content. Content addressing exists so that identity
does not depend on where the bytes live; the local store is the one place n/
forgets that.

Three further holes in the same family, all measured:

| what | today |
|---|---|
| tampered **commit** body | `oo log` walks the forged message as genuine |
| CAID with a real digest and a **forged sketch** | resolves to the real object (`hash_to_path` keys on the digest alone) |
| CAID with a **forged masa_ref** | same |

---

## 2. Measured before writing — the things that shaped the arc

### Round-trip stability — the finding that could have vetoed the arc

If stored objects do not reproduce their own address on read-back, switching
verification on bricks every existing store. Ran the **whole conformance
corpus (143 vectors)** into one store and re-addressed every object:
**125/125 parseable objects stable.** Verification is safe to enable.

### `#blur` does not poison addressing

Storage uses the **unsalted** `content_hash()`. `get_horizon_salt()`
(`SystemTime::now`) feeds only `content_hash_with_salt` at `lib.rs:1889`, an
observation-surface `%id`. Checked because it would have been fatal.

### Commits are v1, values are v2

`Commit::content_hash` returns `ContentHash::v1(digest)` — no sketch. Commit
verification is digest-only by construction; REAL_03 §9.2's spectral clause
applies to values.

### The one unreadable object

Of 126 objects, one could not be deserialized at all: **5,091,205 bytes, JSON
depth 646**, past serde_json's 128 default. Source:
`conformance/L2/20-deep-recursion-type.n` — a **shipped, passing vector at
default configuration**. Two lines suffice:

```
@Tree: { v: @int, next: @Tree | () }
out: 1
```

Bisected: the expansion needs neither the meet nor the navigation, only the
type definition. And it splits in two:

| path | size | `next` unrolls | JSON depth | readable |
|---|---|---|---|---|
| `oo evolve` + `commit` | 260 KB | 3 | 20 | yes |
| `oo run` | **5.09 MB** | **128** | **646** | **no** |

The extra 20× is `run_one_shot`'s store-put loop (`main.rs:590`), which
observes every top-level bare-path field and puts it. `observe` forces.

**Two spec clauses say that is wrong**, and one of them names this very type:

> SPEC_04 §158 — 導航保持惰性，**不得**為吸收而強制固化 union(否則遞迴型
> `@Tree | ()` 導航發散)

> SPEC_12 遞迴表 — 結構遞迴 … `#recursive_lazy` … 惰性展開，不觸發發散

Observing `@Tree` itself does not terminate within 5 minutes, so §158's "would
diverge" is not hypothetical, merely unvisited. L2-20 is titled 遞迴型別終止
and observes `t.v`; it tests termination, and the value path is genuinely fine.
**Laziness is the clause nothing tests.**

**Blast radius of dropping the loop**, measured by disabling it and running
everything: workspace **1450/0/3**, conformance **143/143**, and the unreadable
object disappears. Nothing in the suite or the corpus depends on it.

---

## 3. Rulings

**R-1 (user).** This arc = verification + the forcing store-put loop. The
eager expansion in the **type layer** (the 260 KB at evolve) is **ledgered, not
fixed here** — it reaches type unification and possibly CAIDs and vectors, and
is its own arc.

**R-2 (acceptor).** Fix the loop by **removing it**, not by making it lazy:

* `oo run` is a one-shot **pure** universe (settled by measurement in the `#pin`
  arc; `oo evolve` is the persistent-universe command). A command whose
  contract is "no durable state" writing durable state is a category error,
  independent of laziness.
* Its outputs are orphans by construction — no commit references them, nothing
  enumerates them. It manufactures exactly the garbage the GC arc must sweep.
* Nothing depends on it (measured).
* The capability already exists deliberately as `~%Engine./save`. Making it
  explicit rather than automatic is the ruling the `#squash` arc reached about
  forgetting: the things that matter should be asked for.

Fallback if vetoed: keep the loop but store the staged value **without**
observing — preserves the intent lazily, at the cost of storing thunk-bearing
values whose CAIDs are the lazy ones.

**R-3 (acceptor).** Verify the **full v2 CAID** for values — digest **and**
`lattice_sketch` **and** `masa_ref`. REAL_03 §9.2 offers a digest-only door to
engines *without* spectral support; this engine has it, and §9.2's second
bullet then requires 同時驗證譜特徵與內容指紋的一致性.

**R-4 (acceptor).** **Three outcomes, not a boolean:**

| outcome | meaning |
|---|---|
| `verified` | recomputed address equals the requested one |
| `corrupt` | recomputed address differs — the bytes are lying |
| `unreadable` | cannot deserialize; corruption **cannot be ruled out**, and the engine must say so rather than guess |

Today all three collapse into one string: `run_inspect` maps every `get_value`
error to `"CAID not found in local store"`. A verifier that cannot separate
corruption from a legitimately-undecodable object is not worth switching on —
the same legibility rule the v0.2.41 arc reached about audit faces.

R-2, R-3 and R-4 are acceptor calls, stated so the user can veto any of them.
Confirm they still stand if this order has been sitting.

---

## 4. What to build

### 4.1 Verification

In `crates/interpreter/src/storage.rs`, on the read path used by `get_value`
and `get_commit`:

1. deserialize;
2. recompute the address (`Value::content_hash()` / `Commit::content_hash()`);
3. compare against the **requested** `ContentHash` — for values, all of
   `digest`, `lattice_sketch`, `masa_ref`; for v1 commits, the digest;
4. on mismatch, fail with a distinct, nameable error carrying both addresses.

Deserialization failure is its own error, distinct from both mismatch and
absence.

The error type is `anyhow` at this layer. Make the three cases distinguishable
by the caller — a small enum or three constructors, not three prose strings
that happen to differ.

### 4.2 Surfacing

`run_inspect` (`main.rs:653`) currently flattens every error into
`"CAID not found in local store"`. It must report the three cases distinctly.
Any other caller that maps store errors to a single message needs the same
treatment — enumerate them; do not fix only the one the probes exercise.

Error codes: add to **ERROR_CODES** and REAL_03 §8. Suggested tags —
`#caid_mismatch` (bytes do not match the address) and `#object_undecodable`
(present, cannot be decoded, integrity unknown). Naming is yours to propose;
the *distinctness* is the requirement.

### 4.3 The store-put loop

Remove the loop at `main.rs:590` (see R-2). Keep `~%Engine./save`.

---

## 5. What NOT to touch

* **The raw read layer's semantics.** `effect_cached_probe_test`'s header
  records why: `#cached` solidification hooks the user-facing fetch-by-CAID
  boundary only, never raw `get_value`, so that commit-root reconstruction,
  refine monotonicity and `content_hash` comparisons stay bit-exact (REAL_04
  determinism). Verification is read-only and does not conflict — but it is the
  same function, so **verify, and change nothing else there**.
* **Addresses.** Verification must not move a single CAID.
  `pin_genesis_addresses_are_unchanged` guards this; so does conformance.
* **The type layer.** Defect 1 is out of scope (R-1). Do not touch type
  unification or the recursive-type expansion itself.
* **`conformance/L2/20-deep-recursion-type.*`.** The vector is correct; it
  tests termination and passes. It is corpus, and corpus is spec.

---

## 6. Performance

Verification hashes on every read, and reads sit on the hot path (universe
load, commit, refine, `disc.fetch`). `content_hash()` also computes the
spectral sketch. **Measure before and after** on a repository with real history
— if the cost is material, report it with numbers rather than silently adding
a cache. A cache keyed on the thing being verified would defeat the point, so
that decision is the acceptor's.

---

## 7. Acceptance

1. All 8 red gates green with `#[ignore]` removed — and **only** `#[ignore]`
   removed.
2. All 4 pins still green.
3. Workspace: no new failures against baseline **1454/0/11**.
4. Conformance **143/143**, genesis **11/11**.
5. `cargo build --release` clean of new warnings.

State plainly anything you could not do, and anything you changed outside the
scope above — **including anything you had to touch to make a probe pass**. A
concession made silently to satisfy a probe is the failure mode this protocol
exists to catch. It has happened twice, and both times the root cause was an
acceptor error the delivery accommodated instead of reporting: v0.2.39 widened
the CAID parser for a badly-shaped probe, and v0.2.42 added a peer-address
scheme this work order's predecessor named by mistake. **Reporting my error
back to me is always the correct move.**

---

## 8. Delivery record (delivery side)

- **Tip**: see commit (recorded after).
- **Verification** (`storage.rs`): `get_value` / `get_commit` deserialize →
  recompute address → compare to requested. Values: full v2 (digest +
  `lattice_sketch` + `masa_ref`). Commits: digest (v1).
- **`StoreReadError`** (distinct, downcastable): `NotFound` / `CaidMismatch`
  (`#caid_mismatch`) / `ObjectUndecodable` (`#object_undecodable`).
- **Surfacing**: `run_inspect` and `run_log` use `format_store_read_error` —
  no flattening to "not found".
- **Store-put loop**: removed from `run_one_shot` (R-2). `~%Engine./save` kept.
- **ERROR_CODES**: `#caid_mismatch` note updated; `#object_undecodable` added
  (spec repo).
- **Probe**: only 8 `#[ignore]` removed.
- **Gates**: cas probe **11/12** (see below); genesis **11/11**; conf **143/143**;
  release build clean of new warnings; workspace **1461/1/3**.

### Acceptor error to report (not accommodated)

**`red_a_run_does_not_force_a_recursive_type`** asserts `biggest < 100_000`.
Measured after this delivery:

1. Store-put is gone: bare `oo run` of the two-line `@Tree` program leaves
   **zero** objects under `.oo/objects`.
2. The probe's own `seeded()` (`evolve`+`commit` of `x: 1`) alone leaves a
   root object of **251 839 bytes** (genesis system modules in the committed
   root). That is the R-1 ledgered type-layer weight, not the 5 MB force loop.
3. Therefore the gate stays red **even though the defect under test is fixed**.
   Raising the threshold or dropping `seeded()` is the acceptor's edit — delivery
   did not touch the probe body.

All other reds (7/7) and all 4 pins are green. The multi-megabyte orphan is gone
(`red_every_object_a_run_leaves_can_be_read_back` passes: all objects depth < 128).

### Performance (§6)

No silent cache added. Informal: 3-commit `oo log` / history ops still
sub-second on a temp store; full workspace ~3 min (same order as pre-arc).
If a production history shows material cost, acceptor decides on a non-defeating
cache strategy.
