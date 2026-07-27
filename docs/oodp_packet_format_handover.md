# Work order — silence means four things

Arc opened 2026-07-27, after v0.2.47.
Acceptor: project brain. Implementer: model #3.
Expected classification: **破壞性(Layer 1 — 協定)**. Decided, not assumed: the
wire format changes, and VERSIONING §5 defines Layer 1 as 語義/文法/**協定**.
A v0.2.47 peer and a post-arc peer cannot talk to each other. See §6.

---

## 0. The headline, measured on v0.2.47

Two engines federate today — that part works. B fetched A's value over TCP and
A logged the request. What does not work is everything the peer has to say
when it cannot hand over the bytes.

```
peer does not have the object   →  0 bytes on the wire  →  client: ⊥ #conflict
peer's copy is corrupt          →  0 bytes on the wire  →  client: ⊥ #conflict
peer accepts and never answers  →  oo hangs until killed (30s ceiling, rc=143)
```

**On the wire, silence means four different things**: I do not have it / I have
it and it is corrupt / I am not going to give it to you / the connection died.
The server *knows* which — it logs `NDP Miss` — and says nothing.

This is REAL_03 §6.6 條款三 (三結果必須可分) one layer out, and v0.2.44 deferred
it in as many words: *"Wire stays 0 bytes (REAL_02 §3.2 arc)."* This is that arc.

### What the wire carries today

| | specified (REAL_02 §3.2) | actual |
| :-- | :--- | :--- |
| request | `{{ %op: #discover\|#fetch\|#advertise, %hash: @caid, %from: @caid }}` | bare CAID + `\n` |
| response | `{ %status: #success\|#not_found\|#conflict, %result, %source, %hops }` | bare JSON value, or nothing |

### And LADD is already here

`crates/interpreter/src/ladd.rs` implements GBB (mass, sketch, masa_ref,
nerve_structure); `disc.advertise` and `disc.find` are real. They write to and
read from `oo.gbb_registry` — **an in-process `RwLock<HashMap>`**. So:

> **LADD is implemented as a local simulation. The packet format is the step
> that would make it distributed.**

That is why L2 was ledgered, and it is why the envelope in D1 must carry `%op`
from day one even though this arc implements only `#fetch`.

## 1. Rulings

**Q1 — node identity is the NEXT arc; this arc does not carry `%from`.**
The value here (distinguishable outcomes, timeout, three-op envelope) does not
depend on knowing who is asking — objects self-authenticate, so `#fetch` has no
use for it. `%from` earns its keep in `#advertise` (who is announcing) and
`#discover` (DHT routing), neither of which ships here. Better to record in the
spec that it arrives with node identity than to ship a field nobody fills.

Node identity also deserves the care the operator identity got in v0.2.46, and
it has its own unresolved problem: **node id in `.oo/` means copying a
repository copies the node's identity**, two nodes claiming one id. (The
container answer — generate at first run, never bake into the image — is a
candidate, not a ruling.) Note that this is the *opposite* allocation from
v0.2.46: the OPERATOR key is per-person and explicitly not per-workspace; the
NODE key is per-`.oo/` precisely because a node **is** a workspace.

**Q2 — the rename lands in this cut.**
The protocol break is happening anyway; the peer should have to follow once, not
twice. Renaming costs nothing on the classification ledger — measured: Layer 1 is
語義/文法/協定, and REAL's normativity is 「引擎互操作協議」, so a local
subcommand spelling does not touch interop, does not spend a breaking entry and
does not move the ORDER_00 §5.1.4 clock. Its only cost is that users retype.

## 2. Scope

### D1 — the OODP envelope

* **Request**: `{{ %op: #fetch, %hash: <caid> }}`. `%op` is **required** and parsed from day one; `#discover` and `#advertise` are accepted as *known* op names and answered with an explicit "not implemented here" status rather than silence — a peer must be able to tell "this node does not do discovery" from "this node ignored me".
* **Response**: `{ %status: #success | #not_found | #conflict, %result: <value|absent>, %source: <str>, %hops: <int> }`.
* `%hops` is `0` for a direct answer. It exists now so routing can fill it later without another break.
* Serialisation: whatever the engine already uses for values on this socket, applied to the envelope as a whole. Do **not** invent a second encoding.
* **No `%from`** (Q1). The spec gets a sentence saying why and when it arrives.

### D2 — the four situations must be distinguishable at the client

| peer state | `%status` | client sees |
| :-- | :--- | :--- |
| has it, verifies | `#success` | the value |
| does not have it | `#not_found` | an absence, **not** a conflict |
| has it, fails address re-verification | `#conflict` | a named integrity verdict |
| answers nothing / dies | (no response) | a timeout, distinct from all three |

The client's existing address re-verification (v0.2.44) stays exactly as it is:
a `#success` whose bytes do not hash to the requested CAID is still
`#caid_mismatch`, **regardless of what the peer claimed**. A peer's `%status` is
a claim, not a verification — say so in the code comment. This is degree 0: the
answer is checked, not trusted.

### D3 — a read timeout (ledger L3)

`remote_fetch` sets `connect_timeout` and then `read_to_end` with no deadline.
Measured: a peer that accepts and never answers hangs `oo` until it is killed.
Set a read timeout; a timed-out fetch is its own outcome, not `#not_found` and
not `#conflict`. Pick a default and say what it is; make it overridable only if
that costs nothing.

### D4 — the rename

The peer command moves under a **noun**: `oo node serve`.

Reasoning, for the record: `oo` carries two families of verbs — the git-shaped
ones that act on the repository you are standing in (evolve, commit, log,
status, refine, rollback, squash, fmt, inspect) and the docker-shaped ones that
act on a thing that is running. Git solved the same collision with `git remote`
/ `git submodule`: core verbs stay flat, everything that is not the working tree
gets a noun. `node` is the right noun because the node **is** the `.oo/`
directory — one workspace, one node, and if you want several you run several
directories.

This also gives the next arcs a home that already reads correctly: `oo node id`
(node identity), and later `oo node discover` / `oo node advertise` as the other
two ops land.

**This is the item most likely to want the author's override** — it is proposed
with reasoning, not settled. Everything else here is measured.

### D5 — spec

* **REAL_02 §3.2**: `oo serve-discovery` is written as a command; it is a **role**. §4.1 defines a discovery node as a Kademlia routing participant, and `%op` shows discovery is one of three ops on one packet format. Rewrite as a role, and name the command that actually exists.
* **REAL_02 §3.2**: record that `%from` is specified but not yet carried, and that it arrives with node identity.
* Wherever the spec implies a peer may answer with silence, replace it with the `%status` it owes.

### Out of scope

* `#discover` and `#advertise` on the wire, and everything in REAL_02 §4 (Kademlia, routing table, `ServiceAdvertisement`). The envelope is built so they are additive.
* Node identity, `%from`, advertisement signatures (Q1).
* The whole control surface (REAL_01 §2 / §8.4). Two separate findings are already ledgered against it and neither is this arc's: its endpoint list shares exactly one name (`commit`) with today's CLI, and §2.2 and §8.4 both define `nlang/observe` and `nlang/subscribe` without cross-referencing.

## 3. Probes

To be written by the acceptor before delivery starts, calibrated red-for-the-
right-reason, as usual. Expected shape: the four-way discriminator is the
headline gate and must be **paired in all four directions** — an engine that
answers `#not_found` to everything passes a one-sided test.

## 4. Acceptance

1. Diff purity.
2. Numbers, re-run by the acceptor.
3. Adversarial, at minimum: a peer that answers a *different* CAID's bytes with `%status: #success` (the v0.2.44 lesson — the claim must not be trusted); a peer that answers `#success` with no `%result`; a truncated envelope; an unknown `%op`; a peer speaking the **old** protocol (must fail cleanly and legibly, not hang).
4. A/B against v0.2.47.
5. Classification: the protocol break is expected. Confirm the blast radius by measurement — value CAIDs, universe root, and commit CAIDs must all be untouched, because none of them are on this path.

## 5. Delivery record (delivery side)

Files touched, the numbers, anything refused and why. If a probe looks wrong,
**say so instead of accommodating it** — that has happened twice and both times
the probe was the defect.

## 6. A note on the clock

This is expected to be **破壞性 #6**. Entries #4 and #5 both landed 2026-07-27,
so the ORDER_00 §5.1.4 window currently starts there. If this cut lands the same
day it costs nothing; if it lands later it resets the window to its own date.
That is a scheduling fact, not an argument for or against — recorded so the
decision is made with it in view rather than discovered afterwards.
