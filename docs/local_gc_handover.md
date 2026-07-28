# Local GC + store format version — work order

Arc opened 2026-07-28 against **v0.2.52** (`top 1ccce5e`). Scope: **reclaiming
bytes in one workspace, and marking the store's format so a future change can
be detected.** Durable OODP state (the routing table, the advert directory) is
the *next* arc and depends on this one — see §5.

Deliver on `dev`. Do not tag, do not touch `top`, do not write spec files.

---

## 1. What was measured before this order was written

| # | Fact | How |
| :-- | :--- | :--- |
| M1 | **One `#squash` leaves 7 of 11 objects unreachable — 60% of bytes — and the store grows** (1,343,886 → 1,345,264 B) | live: five commits, then squash |
| M2 | **`.oo/` carries no format version marker at all** | `fmt_version` occurs **0** times in `storage.rs` |
| M3 | Durable surface is nine paths: `.oo/{objects/, HEAD, architects.json, staged, pin_pending, effect_pending, abandoned}` and `~/.oo/{identity, nodes/}` | source scan |
| M4 | **After `#rollback` + commit, whether the abandoned HEAD counts as a root is worth 50% of the store** — as a root: 0% reclaimable; not a root: **50%** | live |
| M5 | **`evolve` writes no objects; only `commit` does.** `staged` holds a fully inlined value and names no CAID | live: 0 objects after init and after evolve, 2 after commit |

M5 is why the root set is small enough to get right. M1's control matters as
much as M1: **before** the squash, 10 of 10 objects were reachable and 0% was
garbage — which is what proves the reachability walker used to produce these
numbers is not simply failing to find things.

> **A near miss worth repeating.** The first version of that walker did not
> understand digests serialised as byte arrays and reported *100% unreachable*.
> The `0% before squash` control is what caught it. This is the same family as
> the `content_digest`/`digest` slip: a measurement that finds nothing looks
> exactly like a measurement that found everything is garbage.

---

## 2. Rulings carried into this order

**R-a — the roots are `HEAD`, its parent chain, and each commit's root value
tree. Nothing else.** `staged` inlines its content (M5) so it makes nothing in
the object store reachable; it must nonetheless never be deleted.
`architects.json`, `pin_pending` and `effect_pending` hold assertions and
intents, not object references.

**R-b — an abandoned HEAD is NOT a root** (owner's ruling, given M4). Its
content becomes collectable.

This follows the rollback arc's own ruling — *details may be lost, the fact
may not* — because the fact is the digest recorded in `CommitMeta.abandoned`,
and that digest survives. But the price is real and must be surfaced, not
buried: **`oo log` prints an `abandoned <caid>` line that, after a GC, names
content the store no longer holds.** The CLI must say so (§3.5). And
**rollback becomes one-way once GC has run** — `oo gc` must say *that*, before
it does anything (§3.4).

**R-c — forgetting never happens automatically.** No GC on commit, on init, on
open, in the background, or as a side effect of anything. Discussion 025
argued that global GC is impossible in principle — there is no global HEAD, so
no global reachability root — which makes local, explicit collection the only
kind there can be. That is a structural fact, not a conservative choice, and
the engine must not soften it.

**R-d — GC is privileged.** `#squash`, which merely makes objects unreachable,
requires `--grant squash` (SPEC_08 §6.2). Deleting the bytes is at least that
consequential, so `oo gc` requires **`--grant gc`** and refuses without it,
removing nothing. Using the same mechanism as every other irreversible
operation is the point: "forgetting must not happen automatically" is then
enforced by the capability lattice rather than by good intentions.

**R-e — the store gets a format version, and an engine that does not
understand it refuses rather than proceeds.** M2 says a future on-disk change
has no way to announce itself today. An engine that opens a store it cannot
read and *proceeds* will either corrupt it or silently misread it; refusing
with a named error is the only honest option. Objects are already
self-describing (CAID carries `v1`/`v2`); what has no version is the *layout*.

---

## 3. Design

### 3.1 Store format version

*   `.oo/format` containing exactly `1` and a newline. Written by `init` (and
    by any engine that opens a store lacking it — see below).
*   On open: absent → treat as version 1 and write it (every store in
    existence today is version 1; refusing them would be refusing everything).
    Present and `1` → proceed. **Present and anything else → refuse the whole
    operation** with a named error naming both versions.
*   The refusal is not advisory. A store of unknown layout must not be read,
    written, or garbage-collected.

### 3.2 Reachability

Mark from the roots of R-a:

1.  `HEAD` → that commit object.
2.  From each commit: its `parent` (transitively) and its `root` digest.
3.  From each value object: every digest it contains, at any depth.

Digests appear in the JSON **both as 64-hex strings and as byte arrays** —
handle both. A walker that handles only one finds nothing and reports
everything as garbage (see §1).

**Not roots**: `CommitMeta.abandoned` (R-b). Everything else that is not
reached is garbage.

### 3.3 Sweep

*   Remove only files under `.oo/objects/` that the mark phase did not reach.
*   Never touch `HEAD`, `staged`, `architects.json`, `pin_pending`,
    `effect_pending`, `abandoned`, `.oo/format`, or anything outside `.oo/`.
*   Remove empty two-hex-digit directories left behind; leave `sha256/` and
    `objects/` in place.
*   **Verify before deleting**: an object that fails to decode is *not*
    thereby garbage — it may be a corrupt but reachable object, which is a
    REAL_03 §6.6 integrity incident and must be **reported, not swept**.
    Sweeping what you cannot read is how a corruption becomes a deletion.

### 3.4 The command

```
oo gc --grant gc
```

Without the grant: refuse, remove nothing, exit non-zero, and say which grant
is missing.

Before removing anything, print the consequence, because R-b makes this
irreversible in a way the operator may not expect:

```
oo gc: 11 objects, 4 reachable, 7 collectable (775,637 bytes)
        3 of them are content of heads abandoned by #rollback — after this,
        `oo log` can name them but not resolve them, and rolling forward is
        no longer possible
```

Then do it, and report what happened:

```
oo gc: removed 7 objects, freed 775,637 bytes
```

*   **Idempotent**: a second run frees nothing and says so.
*   **A clean store**: frees nothing and says so — not silence.
*   `--dry-run` performs the mark phase and the report and removes nothing.

### 3.5 `oo log` after a collection

An `abandoned <caid>` line whose content is gone **must still be printed** —
the fact is what the rollback arc protected — and must be marked:

```
    abandoned hash:sha256:v1:eeba8e5c… (content collected)
```

Determining that requires a store lookup per abandoned entry; that is
acceptable, and **must not** be replaced by a cached flag written at GC time
(a flag is a second source of truth about whether bytes exist, and the bytes
are the truth).

### 3.6 What GC does not defend against

State it in the code and in the report; do not imply otherwise:

*   **Concurrent writers.** The engine does not lock the workspace. A `commit`
    landing while `oo gc` sweeps can lose objects. The command must say it
    expects exclusive use; the engine does not enforce it.
*   **Other workspaces.** Reachability is local by construction (R-c). An
    object collected here may be the only copy anyone had.

---

## 4. Deliverables

1. `.oo/format` (§3.1), with the refusal on unknown versions.
2. Mark-and-sweep (§3.2, §3.3), including the corrupt-object rule.
3. `oo gc` with `--grant gc` and `--dry-run` (§3.4).
4. `oo log` marking unresolvable abandoned entries (§3.5).
5. Remove `#[ignore]` from `crates/oo/tests/local_gc_probe_test.rs`.
   **Nothing else in that file may change.**

---

## 5. Out of scope — do not deliver

*   **Durable OODP state.** The routing table and advert directory stay in
    process memory. That arc comes next and depends on this one: it needs a
    rule for when a peer is forgotten (GC) and for reading an old index
    (format version). Reverting it once was enough.
*   **Global or cross-workspace GC** — impossible in principle (R-c).
*   **Automatic or background GC** (R-c).
*   **Compaction, repacking, delta encoding, loose→pack.** Freeing bytes is
    this arc; storing them better is not.
*   **Locking the workspace** (§3.6 states the limitation instead).
*   No new language primitive. No spec files. No `git add -A`, no tags.

---

## 6. Gates

Probes are pre-committed in `crates/oo/tests/local_gc_probe_test.rs`,
calibrated at v0.2.52 before this order was sent. **You may remove `#[ignore]`
and nothing else.** If a probe looks wrong, report it; do not repair it.

**Pins scheduled to change: none.** Nothing in the existing suites asserts
that objects are never removed. If you find one, that is the countdown-timer
shape the v0.2.51 order got wrong, and you should report it rather than absorb
it.

### Reds — must go green

| # | What it holds |
| :-- | :--- |
| R1 | after a squash, `oo gc --grant gc` removes **exactly** the set the probe independently computes as unreachable — no more, no fewer |
| R3 | abandoned-head content **is** collected (R-b), and `oo log` still prints the line, marked as unresolvable |
| R5 | GC never runs by itself: `init`, `run`, `evolve`, `commit`, `log`, `inspect` and `node serve` all leave a garbage-holding store byte-identical |
| R6 | without `--grant gc`: refuses, removes nothing, exits non-zero, names the missing grant |
| R7 | `.oo/format` exists after init and says `1`; a store marked `2` makes every operation refuse with a named error, and **the store is left untouched** |
| R8 | idempotent: a second GC frees nothing and says so |
| R9 | a store with no garbage: frees nothing and says so |
| R10 | a **corrupt but reachable** object is reported as an integrity incident and **not** swept |
| R11 | `--dry-run` reports the same numbers as the real run and removes nothing |
| R12 | after GC every surviving object still verifies against its own CAID |

**R2 and R4 were moved to the pins at calibration** (now P5 and P6). Both
passed at v0.2.52 — for the wrong reason: nothing removes anything yet, so a
probe asserting "nothing reachable was removed" is satisfied by an engine that
does nothing at all. A gate that is green because the feature is absent has
measured nothing, and this project already learned to file that shape as an
**active pin** rather than a red line.

The pair that makes the sweep falsifiable is therefore **R1 + P5**: R1 forces
removal to happen and P5 forbids removing the wrong things. An implementation
that deletes nothing fails R1; one that deletes everything fails P5. Only the
labels changed; the obligation did not.

R10 is the one that turns a bug into a disaster if missed — sweeping what you
cannot decode deletes exactly the objects a user most needs to be told about.

### Pins — must stay green

| # | What it holds |
| :-- | :--- |
| P1 | `kademlia_table`, `discover_index`, `advertise_wire` suites |
| P2 | universe determinism: two fresh workspaces, same source, same root digest |
| P3 | `#squash` / `#rollback` / `#pin` behave exactly as at v0.2.52 |
| P4 | nothing new persists under `.oo/` except `format` |
| P5 | **nothing reachable is ever removed**, and the universe still evolves and commits afterwards with an unchanged root digest (was R2) |
| P6 | uncommitted `staged` work survives a collection and still commits (was R4) |
| P7 | `history_ops` and `cas_integrity` suites |

### 6.1 Calibration record — measured on v0.2.52 before this order was sent

Control + pins **8 passed / 0 failed**. Reds **0 passed / 9 failed**.

| # | Baseline failure |
| :-- | :--- |
| R1, R8, R9, R10, R11 | `unrecognized subcommand 'gc'` |
| R3 | the abandoned head's object survived (nothing collects it) |
| R6 | granted GC removed nothing |
| R7 | `.oo/format` was never written |
| R12 | *(passes trivially today; see below)* |

The control — `reachable_before_any_squash_is_everything` — is the one that
makes every number above readable. It asserts that in a store where nothing
has been orphaned, the walker reaches **all** of it. Without it, a walker that
silently finds nothing reports a pristine store as entirely garbage, and every
red would then "pass" by deleting the universe.

Two probes changed label at calibration (R2→P5, R4→P6); see above.

---

---

## 7. Acceptance numbers to have ready

1. Full suite before and after.
2. For a store with a known history: objects and bytes before, the probe's
   independently computed reachable set, and what GC actually removed — the
   three must agree exactly.
3. R10's corrupt object: shown reported, shown still on disk.
4. P2's two root digests in full, equal.
5. Cross-version: a v0.2.52 engine opening a store this build has GC'd and
   marked `format: 1` must work normally. Say what a v0.2.52 engine does with
   `.oo/format` present (it has never heard of it).
6. Wall clock of a GC over the largest store you can build cheaply, with the
   object count — so the cost is a known quantity.

---

## 8. Ledger — not this arc

1. `reader.read_line` unbounded (v0.2.48).
2. Two CAID computation paths disagree (`~%Discovery./identify` vs bare
   `content_hash()`), still unexplained.
3. `mod advert_debug` with `println!` still in the engine.
4. `free_port()` is TOCTOU in every network probe suite.
5. `routing_id_from_digest` silently zero-pads a short digest (unreachable
   today).
6. REAL_02 §3.2: unknown op and malformed both answer `#conflict` — four
   things, three codes. Spec-side, ruling deferred.
