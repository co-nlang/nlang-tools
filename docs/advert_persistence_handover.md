# Durable peer directory — work order

Persist the advertisement directory across restarts, rebuild the Kademlia
bucket index from it on load, and bind the observed half of each record to the
node that observed it.

This arc exists because the `kademlia_table` arc's delivery persisted the
routing state as a side effect and acceptance reverted it. The pin left behind
(`p4_nothing_persisted`) says why in its own failure message: durable OODP
state "carries GC, migration, a REAL_02 §5.1 clause, and the fact that
incumbent-first stops being reset by a restart. Arriving as a side effect is
how persistence arrives unaudited." This is that arc.

---

## 1. What was measured before this order was written

Measured on v0.2.53 unless stated.

* **The routing table is a derived index.** `routing.rs` says so in its own
  header — "second index over accepted `#advertise` records". `record_peer_advert`
  (lib.rs:572) writes the directory and the index together; the index holds
  `node_id` strings and looks the record up in the directory to answer
  `#find_node` (routing.rs:154–160). Nothing in the index is not recoverable
  from the directory plus an ordering.
* **The self id is seeded lazily.** `Ouroboros::init` builds
  `RoutingIndex::new([0u8; 20])` (lib.rs:463) and the first accepted advert
  replaces it with the real node id (lib.rs:583–584). A table loaded from disk
  is meaningless under a different self id: every bucket index is
  `leading_zeros(XOR(self, peer))`, so changing self renumbers every bucket.
* **The revert is clean and marked.** lib.rs:459–463 carries the acceptor's
  note naming `.oo/oodp_index.json` and pointing at `routing.rs`. There is no
  live load path.
* **Node identity is keyed by workspace path.**
  `Identity::node_key_path(workspace)` =
  `{OO_NODE_HOME|~/.oo}/nodes/<sha256 of the absolute workspace path>`
  (value.rs:1756–1762, lib.rs:540). Copying a workspace tree to another path
  therefore *is* another node — v0.2.48's ruling, and the mechanism a probe
  uses to construct a copy without a second process.
* **The reverted design's cost, from the v0.2.52 record**: 150 accepted
  adverts produced a 185,854-byte index and **14.38 MB of cumulative writes**,
  because the whole file was rewritten on every accept. That is 1,239 bytes
  per record and O(N²) in writes — the 77× amplification the revert named.
* **Two pins assert the absence of what this arc adds** (found by the standing
  mechanical check, §6.3).

---

## 2. Rulings carried into this order

Decided by the owner, 2026-07-29.

### R1 — only the signed face travels

A persisted record has two halves, and they are the two strata of discussion
025 as they appear inside one record (the `#discover` arc already split them
on the wire, REAL_02 §4.2.5):

* **self-authenticating** — the verbatim `%ad` source, which carries
  `node_id`, `public_key`, `services`, `listen_port`, `capacity`, `ts`, `ttl`
  and the signature. True regardless of who holds it. **Travels.**
* **asserted** — `observed_host`, `hops`, `received_at`, and `addr` (which is
  derived from `observed_host`). These are claims by *this* node about its own
  receipt. **Does not travel.**

"Travels" means: survives being copied to another node. It does **not** mean
"survives a restart". A restart in place is the same node, so a node restores
its own observations; that is not travel, it is memory.

Mechanically: the file records the `node_id` it belongs to. On load,

* signed records load unconditionally;
* asserted fields load **only if** the file's `node_id` equals this node's;
* on mismatch the record loads without an address, and the node must
  re-observe before it can dial. It can still **relay** such a record, because
  relay emits the verbatim signed source and marks `%observed_host`
  separately — a copy may repeat what it was told and may not claim to have
  seen it.

On mismatch there is also no `received_at`, so the incumbent ordering falls
back to the signed `ts`. That is the only ordering a copy is entitled to.

### R2 — the durable file is named for what is actually durable

The bucket index is not persisted. The directory is, and the index is rebuilt
on load by replaying records in `received_at` order (falling back to `ts`
per R1). Rebuilding reproduces incumbent-first exactly, because incumbent-first
is a function of insertion order and nothing else.

So the file is a **peer directory**, and it is named that. `.oo/peers/` —
not `.oo/routing/buckets.dat`, which REAL_02 §5.1 sketches as a blueprint.
Spec closure corrects §5.1; the acceptor does that, not the delivery.

Two sources of truth would be the alternative, and the directory and the index
could then disagree with no way to tell which was right.

### R3 — automatic removal is allowed here, and the boundary must be written down

SPEC_08 §6.2 ruled two days ago that forgetting must not happen automatically.
That clause governs **making a recorded fact's content unreachable**: a
committed object is named by a commit, and collecting it makes a rollback
irreversible. None of that holds for an advertisement. It was never committed,
no recorded fact points at it, and dropping it makes nothing irreversible.

So automatic removal is legal here — and the spec must say where the boundary
runs, because otherwise the next reader will take §6.2's red line to cover all
of `.oo/`, which it does not.

**This arc exercises that permission only for superseded records** (see §5).

---

## 3. Design

### 3.1 The file

`.oo/peers/directory` — one record per line, append-only.

Line format is the delivery's choice with one constraint: the verbatim `%ad`
source must round-trip **byte for byte**, because relay emits it verbatim and
a re-serialisation would break signatures (REAL_02 §4.2, `discover_index` §3.3).

The file's first line is a header carrying at least the owning `node_id` and a
format tag. A file whose header does not parse is treated as absent, not as an
error: an unreadable cache is a cold start, and refusing to boot because a
cache is damaged would be the engine asserting that a cache is load-bearing.

### 3.2 Writing

Append one line per **accepted** advert — after the signature ladder passes,
at the same point `record_peer_advert` is called today. Nothing that fails
verification is ever written.

Full-file rewrites happen only at compaction (§3.4). The gate is a measured
bound, not an adjective: see R6.

**The serving process must say what it wrote.** Byte counts cannot be observed
from outside: file size distinguishes neither an append from a rewrite of the
same content, nor a rewrite from a compaction. The node is the only thing that
knows, and it is already this arc's observation surface — so these three lines
are part of the deliverable, spelled exactly, because the probes parse them:

```
OODP Peers: append <bytes> bytes (<live> live)
OODP Peers: compact <bytes> bytes (<live> live)
OODP Peers: loaded <n> records, skipped <k> damaged
```

Same shape as the kademlia arc's repair: the observability the order specifies
needs no second process. That is also the satisfiability check on this order —
every gate below reads either the file, the serving node's own log, or a
restart of it, and none requires a view the running node cannot provide.

### 3.3 Loading

At `Ouroboros::init`, if `.oo/peers/directory` exists:

1. read the header; on mismatch or damage, start empty;
2. replay lines in file order, last-wins per `node_id`;
3. apply R1's identity check per record;
4. seed `RoutingIndex` with this node's id and insert the surviving records in
   `received_at` order (or `ts` on mismatch).

A line that does not parse is **skipped**, counted, and reported on the serving
node's own log. The rest of the file still loads. One damaged line must not
cost the whole directory, and the count must be visible — a silent skip is how
a directory quietly becomes empty.

Note the ordering problem this creates and solve it: today the self id is
seeded by the first accepted advert. On load there may be records but no
advert yet, so the loader must obtain the node identity itself. Do not mint
one — `node_identity()` is lazy on purpose (P5 of the node-identity arc: an
ordinary command must not create a key). If no node key exists yet, load the
records but leave the index unseeded, exactly as a fresh process does today.

### 3.4 Compaction

When the file holds more than **2×** the live record count, rewrite it with
only the surviving records. Superseded lines are dropped — a newer signed
advertisement from the same node replaces an older one, which is replacement,
not forgetting.

Compaction is the only full-file write. It is automatic (R3) and requires no
capability.

### 3.5 What this changes about incumbent-first, deliberately

A full bucket keeps its incumbents (v0.2.52). Until now that lock was reset by
every restart, which was a real if accidental mitigation. **After this arc it
survives restarts.** An attacker who wins the race to fill a bucket keeps it
until the record is superseded or the file is removed by hand.

This is a cost the arc accepts, not an oversight. It is named here, it is named
in the spec at SPEC_15 §7.1 where the cost model lives, and it is the reason
the v0.2.52 story commit called incumbent-first "a race condition, not a
defence". Do not add eviction-by-ping to compensate: pinging inside the insert
path lets a stranger choose when this node dials out, which is why v0.2.52
refused it.

---

## 4. Deliverables

1. `.oo/peers/directory` — append on accept, load at init, compaction at 2×.
2. Header with owning `node_id`; R1's per-record identity split on load.
3. `RoutingIndex` rebuilt on load in `received_at` (or `ts`) order.
4. Damaged-line skip with a counted, logged report.
5. Un-`#[ignore]` the reds in `crates/oo/tests/advert_persistence_probe_test.rs`.
6. Update the two pins listed in §6.3 — **these are scheduled to change**.

---

## 5. Out of scope — do not deliver

* **Age-based expiry.** R3 makes it legal, and it is still not this arc. There
  is no staleness constant anywhere in the spec, and `ttl` is not one — it is a
  lattice quantity, not a duration (REAL_02 §4.2.7). Inventing a number inside
  an implementation arc is how a constant becomes canon without a ruling.
  Compaction here drops **superseded** records only.
* Persisting the bucket index itself (R2).
* Eviction by ping (§3.5).
* Any change to the signature ladder, to `#fetch`, or to `#discover` relay.
* Anything under `.oo/objects/`.
* Spec edits and CHANGELOG entries — acceptance does those.

---

## 6. Gates

`crates/oo/tests/advert_persistence_probe_test.rs`, pre-committed with this
order. **The probes belong to the acceptor**: delivery removes `#[ignore]` and
changes nothing else. If a probe looks wrong, say so and stop — do not adjust
it to fit. That has happened twice and both times the order was at fault.

### Control — must be green before and after

`c0_a_node_that_learned_nothing_writes_nothing` — a node that serves but
accepts no advert leaves no `.oo/peers/` behind. Leads the file: every scan
below asks "what is in the directory", and a loader that silently fails makes
every one of them pass by having nothing.

### Reds — must go green

| # | Name | What it holds |
|---|---|---|
| R1 | `restart_in_place_restores_the_directory` | advertise 30, kill, restart same workspace, `#find_node` returns them |
| R2 | `the_file_appears_where_declared_and_nowhere_else` | `.oo/peers/directory` exists; no other new entry under `.oo/` |
| R3 | `restart_in_place_restores_the_observed_host` | same node restores its own observation; the verbatim `%ad` survives byte for byte |
| R4 | `a_copy_gets_the_signed_half_only` | tree copied to another path: the signed record is still relayed, signature intact |
| R4b | `a_copy_does_not_inherit_an_observation` | the copy emits **no** `%observed_host` for a connection it never accepted |
| R5 | `the_rebuilt_index_matches_an_insertion_replay` | after reload, `closest(target, k)` equals a brute-force replay over all 60 records — whole table, not sampled |
| R6 | `writes_are_linear_not_quadratic` | writes are **non-zero** and 150 adverts cost **< 1 MB** (the reverted design cost 14.38 MB) |
| R7 | `a_superseded_record_is_replaced_after_reload` | second advert from the same `node_id` wins; the older one does not come back |
| R8 | `compaction_triggers_and_shrinks_the_file` | the 2× threshold is crossed and the live set is unchanged |
| R9 | `one_damaged_line_does_not_cost_the_directory` | corrupt a middle line: the rest loads, and the skip count reaches the serve log |

R4 and R4b are a pair on purpose. R4 alone would go green under a design where
*everything* travels; R4b alone would go green under one where nothing does.
Only both together pin the split. (Lesson from the named-parameter arc: a
probe whose defect shows as "two cases are indistinguishable" must be made
pair-discriminating or it is a hollow green.)

### Pins — must stay green

| # | Name | What it holds |
|---|---|---|
| P1 | `unsigned_adverts_never_reach_the_directory` | a **computing** body with a tampered signature is refused and never written |
| P2 | `gc_does_not_touch_the_peer_directory` | `oo gc --grant gc` leaves it alone |
| P3 | `advertising_writes_no_objects` | the directory is durable state, not CAS content |
| P4 | `fetch_is_untouched` | `#fetch` still independent of `%from` |
| P5 | `find_node_answers_are_unchanged_within_one_process` | a live node's answers do not move |
| P6 | `an_unknown_entry_under_oo_is_tolerated` | an unrecognised `.oo/` entry breaks nothing — the invariant behind §6.2 |
| P7 | `the_store_format_marker_is_not_bumped` | `.oo/format` is an invariant here, not a target |

### 6.0 What calibration changed in this order

Written before the probes existed, this section listed **eleven** reds. Three
of them would have been green at v0.2.53 for the wrong reason, and calibration
is what found that — running the reds and reading *why* each failed, not
merely *that* it failed:

* **"cumulative writes < 1 MB"** — the engine writes zero bytes today, so the
  bound passed on nothing. Rewritten as R6, which asserts writes are non-zero
  **and** bounded. This is the standing rule "a comparison must first prove
  both sides non-empty" applied to a bound.
* **"`.oo/format` is not bumped"** — nothing moves it today. It is an
  invariant, so it became P7.
* **"a previous release still opens this store"** — the file does not exist at
  baseline, so there is nothing to hand an older binary. Cross-version is an
  acceptance measurement (§7); the invariant that makes it work is P6.

Calibration also found **four reds failing on a precondition rather than on
the target**: R3/R4/R4b/R7 asked `#discover` with `%target: ["a-name"]`, and
`%target` is a single **CAID** string — `#discover` asks who serves a CAID, and
the engine refuses an unparseable target. Those four were red, and red for
nothing. They now mint a service CAID and fail on the restart, which is what
they are for.

**A red that fails is not yet a calibrated red.** It has to fail on the
sentence it claims to hold.

### 6.1 Scheduled to change — **not** invariants

Two existing pins assert the absence of exactly what this arc adds. Both must
be edited, and both edits are authorised here. Listing them is the point: a pin
whose content is "X has not happened yet" is a countdown timer, and this arc is
the expiry.

* `crates/oo/tests/kademlia_table_probe_test.rs::p4_nothing_persisted` —
  asserts routing leaves **no** durable state under `.oo/`. Rewrite as an
  allow-list including `peers`, keeping the property (nothing *undeclared*
  appears), not the old absolute.
* `crates/oo/tests/local_gc_probe_test.rs::p4_no_undeclared_durable_state` —
  add `peers` to the allow-list.

Nothing else in either file may move.

### 6.2 Why `.oo/format` is not bumped

The marker exists so an engine that cannot read a layout refuses by name. A
v0.2.53 engine reading a store with `.oo/peers/` is not in that position: it
ignores a file it does not know and works correctly (R10 measures this in both
directions). Bumping would make it refuse a store it can read — a false
refusal, which REAL_03 §6.6's new clause was landed one arc ago to forbid: a
verdict must be true, and "I cannot read this" said about something readable
teaches an operator to ignore the marker.

### 6.3 The mechanical check that found §6.1

Standing rule, from this arc's predecessor: **when an arc adds a durable file
or an op, grep the existing pins for what asserts its absence.** Run before
writing the order, not after:

```
grep -rn '\.oo/' crates/*/tests/*.rs | grep -iE 'allow|assert|read_dir|contains'
```

Two hits, both in §6.1. The rule exists because writing the lesson down did not
stop it recurring one arc later; the check has to be mechanical.

---

## 7. Acceptance numbers to have ready

* workspace / conformance / genesis / `advert_persistence` / `kademlia_table` /
  `discover_index` / `local_gc` counts
* cumulative bytes written for 150 accepted adverts, against 14.38 MB
* bytes per record, against 1,239
* file size before and after compaction, with the live count both times
* the damaged-line skip count as it appears in the serve log
* both cross-version directions (R10), stated as what was run, not as "works"

---

## 8. Ledger — not this arc

Carried forward, still open:

1. `reader.read_line` is unbounded (`#fetch` shares it; capping needs a spec
   ruling on object size).
2. **Two CAID paths disagree** — `~%Discovery./identify x` returns the CAID of
   the argument pack `apply_morphism` builds, not of `x`; measured 2026-07-29,
   violates SPEC_13 §6.1 and REAL_02 §4.2. Rides the delegation arc, because
   fixing it changes the signature payload.
3. `mod advert_debug` ships with `println!` and a stale comment.
4. `free_port()` is TOCTOU in every network probe suite.
5. `routing_id_from_digest` zero-pads a short digest silently.
6. REAL_02 §3.2 wants unknown-op and malformed distinguishable; both are
   `#conflict`. Rides the delegation arc.
7. `oo <cmd> | head` panics on SIGPIPE, engine-wide.

---

## 9. Delivery record (delivery side)

Fill in on return: what was built, what was measured, what was left, anything
noticed and not fixed. An empty record has happened twice; both were caught.
