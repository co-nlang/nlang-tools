# `#discover` — the service index on the wire — work order

Arc opened 2026-07-28 against **v0.2.50** (`top 75e2932`). Scope decided by the
project owner after the measurements in §1: **service index only**. Kademlia
routing tables (REAL_02 §4.1) are *not* in this arc — see §5.

Deliver on `dev`. Do not tag, do not touch `top`, do not write spec files.

---

## 1. What was measured before this order was written

All numbers below are from the released v0.2.50 binary, two real workspaces,
real sockets. Reproduce any of them before you start; if one disagrees, stop
and report rather than coding around it.

| # | Fact | How it was measured |
| :-- | :--- | :--- |
| M1 | `#discover` on the wire returns `#not_implemented` | live request to `oo node serve`; `oodp.rs:328` |
| M2 | REAL_02 defines no `#discover` packet. APP_05 §2.3/§5.1 do define one, but it queries a **geometric pattern** and its response carries **no peer and no address** — it cannot answer "who serves this CAID" | full spec scan |
| M3 | **A verified advertisement changes nothing** | see below |
| M4 | The advert directory is process memory only; zero bytes on disk | `.oo/` of a node that accepted an advert contains only an empty `objects/` |
| M5 | `peer_adverts` (written by the wire, read by nobody) and `gbb_registry` (written locally, read by routing) are disjoint maps | source scan |
| M6 | No k-buckets, no XOR distance anywhere | `MAX_ROUTING_HOPS = 16` is gravitational weight — L3, not L2 |
| M7 | **`%hops` has existed in the response envelope since the packet-format arc, is documented at REAL_02 §3.2 ("路由跳數；直答為 0"), and is passed `0` at every one of its ten call sites** | source scan |

M3, in full, one CAID, one machine A:

```
control 1   A serves it over the wire            → %result present     PASS
measure     B holds A's verified signed advert   → ⊥ #conflict         FAIL
control 2   B with an explicit ./connect to A    → { found_via: … }    PASS
```

Both controls are non-empty; the middle line is the finding. REAL_02 §4.2.6
declares this scope, so the engine is conformant — this arc is what makes the
directory load-bearing.

Two gaps found while measuring, both of which this arc must close because
relaying makes them load-bearing:

*   **`PeerAdvert` does not retain `signature` or `ts`.** Verbatim relay is
    impossible with the current record.
*   **`ttl` is a required field whose value is never checked**
    (`field_as_i64(&cv, "ttl").unwrap_or(0)`). §4.2's `& ..15` is unenforced.

---

## 2. Rulings carried into this order

**R-a — §4.2.5's structural guarantee cannot survive discovery, and that is
not a defect to hide.** §4.2.5 buys anti-amplification and anti-reflection
*structurally*: "a valid signature can never name a third party". `#discover`
**is** the operation of naming a third party. Therefore:

*   the identity, port, services, timestamp and hop budget of a relayed record
    stay **self-authenticating** — they are inside the signature;
*   the **host** and the **distance travelled** cannot be — the first was the
    relayer's observation, the second is the relayer's count. Neither is signed.

A relayed record is therefore **a self-authenticating object wrapped in
asserted pointers**. The seam of discussion 025 runs *through the record*. The
wire format must make that visible in its shape (§3.3), not bury it in prose.

**R-b — amplification moves from structural to budgeted.** §4.2.5 closed the
amplifier by construction. A small `#discover` returning a large `%peers` list
is an amplifier. It must be closed by an explicit, measured bound instead
(§3.6). State the ratio; do not hand-wave it.

**R-c — `%from` on `#discover` is a claim, not authentication.** REAL_02 §3.2
left this "still undecided"; it is decided here. Nothing may branch on it.
A discover answer is a re-broadcast of public signed records; making the answer
depend on who asks buys no security and creates a partition surface.

**R-d — `ttl` is a lattice quantity, and this layer cannot compute it. Treat
it as a declared relay bound and claim nothing more.**

This ruling was drafted twice and both drafts were wrong. It said "duration"
first, then "hop count". The author of the design supplied what it actually
was, and the spec bears it out:

*   §4.1's first routing filter is $MASA_{overlap} = MASA_Q \sqcap MASA_{N_i}$
    — **every hop is a meet**, and meet descends the lattice order
    ($A \sqcap B \sqsubseteq A$).
*   §3.1's mass is $m(C) = \mathrm{Tr}(P_C)$ — an integer, the rank of a
    projection.
*   A quantity that is an integer, descends under meet, and is naturally
    bounded above is exactly the shape of `ttl: @int & ..15`.

**Which is why it has no unit.** A lattice rank is not measured in seconds or
in hops. And nothing decrements it: *the mathematics* decrements it. Because
it is recomputed at each node from content-addressed data, the original `ttl`
is **self-authenticating** — a degree-0 quantity in the sense of discussion
025, requiring no trust in any relayer.

Two consequences, and the second is why this arc must be modest here:

1.  **A hop count is not that quantity.** It is the degenerate shadow left when
    the distance cannot be computed — an *asserted pointer*, degree ≥1, on the
    wrong side of the 025 seam. The distance cannot be computed on this layer
    for the reason M5 records: $d_L$ lives in `gbb_registry` and `ttl` lives in
    `peer_adverts`, and the two maps have never met. **The field kept the name
    of the mechanism and lost the mechanism** — the same disease discussion 026
    diagnosed, third instance in this arc.

2.  **The monotonicity was deliberately abandoned.** §4.3 step 3 sends a query
    with no common MASA to a random jump, and §4.4 makes spectral random jumps
    mandatory — but a node with no common MASA is exactly one §4.1 already
    filtered to $W_i = 0$. The random jump leaves the descending chain **by
    construction**, and §7.3 says why: it is the Semantic Eclipse defence.
    A self-authenticating monotone budget and the ability to escape a captured
    neighbourhood cannot both be had. The spec chose escape, and then needed a
    non-monotone safety net — which is what `MAX_ROUTING_HOPS = 16` is.

So three budgets now coexist: `ttl` (lattice quantity, signed), `fuel_limit`
(the querier's execution budget, APP_05 §2.3), and `MAX_ROUTING_HOPS` (the
engine's hardcoded net). The third exists because the first stopped being
monotone. Reconciling them is spec work, not this delivery's.

**What this arc therefore does, and no more:**

*   `ttl` (**signed**, `0..=15`) is treated as the originator's **declared
    relay bound**. It is **never modified** — it is inside the signature, and
    modifying it would destroy it.
*   `ttl: 0` means **"do not relay me"**. This is meaningful under either
    reading (a rank budget with nothing left to meet; a relay depth of zero),
    which is why it is safe to build on now.
*   `%hops` is emitted because the envelope documents it (M7) and it is useful
    to an operator reading a log. **It is not a check.** Comparing a relay
    count against a rank bound would be comparing two different quantities —
    precisely the category error this arc keeps finding elsewhere — so the
    receiver does **not** gate on `%hops`, and no code may imply that it does.
*   `%hops` is in any case **unverifiable**: a dishonest relayer claims `0`
    forever. R7 pins that limitation so nobody later mistakes it for a defence.

Multi-hop gating belongs to the routing arc, where the real mechanism —
distance recomputed from content-addressed data — is actually available.

**R-e — a bad entry drops the entry, not the response.** SPEC_13 §6.1.1 and
REAL_03 §6.6: sources are peers at degree 0; skip the liar and keep going.
One unverifiable relayed record must not deny service for the verifiable ones
in the same response.

**R-f — freshness has no home in the format, and the engine must not invent
one.** With `ttl` spent on propagation depth, nothing in the advertisement
declares a lifetime. `ts` gives an origin time and nothing else. The receiver
therefore applies **its own** staleness bound, which is exactly the kind of
local availability policy SPEC_13 §6.1.1 permits (it may govern availability;
it may not switch off verification). Use **15 minutes** as the default and
make it visible in the log line (§3.9). Do **not** add a lifetime field to the
advertisement — that is a spec change, and it is the acceptor's.

---

## 3. Design

### 3.1 The request

```nlang
{{ %op: #discover, %from: "<node_id>", %target: "<caid>" }}
```

*   `%target` is **required**. Missing or unparseable → `#conflict`.
*   `%from` per R-c: recorded for observability, never consulted.

### 3.2 What the responder searches

The **advert directory only** — entries whose signed `services` list contains
`%target` as an **exact string match**.

*   Do **not** consult the responder's own object store. A node that holds the
    object but never advertised it to you will not be found here; that is
    `#fetch`'s question, asked directly. Say so in the log, do not paper over it.
*   Do **not** sort, dedupe or normalise `services` before matching — §4.2.1:
    list order is meaningful because it is inside the address.
*   Records excluded **before** the cap is applied: `ttl == 0` ("do not relay
    me", R-d) and older than the local staleness bound (R-f).

A hit means **someone claimed to serve this** (§4.2.4). Nothing more.

### 3.3 The response

```nlang
{{ %status: #success,
   %source: "<responder node_id>",
   %hops:   1,
   %peers:  [ {{ %ad: <verbatim advert body, signature included>,
                 %observed_host: "203.0.113.7" }} ] }}
```

**`%ad` is a JSON string carrying the advertisement as n/ source, byte-for-byte
as it arrived.** The response envelope is JSON, so an n/ combo cannot be nested
inside it as structure; carrying the original source text instead makes
"verbatim" literally checkable rather than a property of a re-encoder. Store
the exact source substring at `#advertise` time (§3.7) and emit it unchanged.

§4.2.1 says the signature commits to the value and not the encoding, so a
re-encoding would still verify — but it would also mean nobody can tell a
faithful relay from a lossy one by looking. Do not re-encode. No field may be
added, removed, reordered, or rewritten — least of all `ttl`.

`%observed_host` is a **sibling of `%ad`, not a field inside it**, and `%hops`
sits in the envelope, outside every `%ad`. This is normative and it is the
point of R-a: **what the signature does not cover must not sit inside what it
does.** Both unsigned fields are the relayer's own assertions, and a reader
must be able to see the trust boundary in the shape of the packet.

`%hops` is `1` on relayed entries and stays `0` on every direct answer, as
REAL_02 §3.2 already documents. It is **observability, not a gate** (R-d);
nothing on either side branches on it.

Empty result → `%status: #success` with `%peers: []`. **Not** `#not_found`.
"Nobody I know of advertises that" is an answer; `#not_found` would collapse it
into "I have no index", and those must stay distinguishable.

### 3.4 Receiver obligations, in this order

For **each** entry of `%peers`, independently (R-e — failure drops the entry):

| # | Check | On failure |
| :-- | :--- | :--- |
| 1 | `%ad` present, is a combo, all §4.2 required fields present | drop entry |
| 2 | `CAID(public_key) == node_id` | drop entry |
| 3 | signature verifies over `"oodp-advert:v1:" ++ CAID(body − signature)` | drop entry |
| 4 | `0 <= ttl <= 15` | drop entry |
| 5 | `ts` within the receiver's staleness bound (R-f) | drop entry |
| — | all pass | usable candidate |

Note what is **absent**: there is no `%from == node_id` check here. §4.2.2 step 3
is about the speaker of an `#advertise`; on a relayed record the speaker is the
relayer, and the record is not about them.

Step 2 must precede step 3, for the reason already given in §4.2.2: an engine
that only asks "does the signature match the key in the packet" accepts every
forgery, because a forger supplies both.

There is deliberately **no check against `%hops`** (R-d). A relay count and a
lattice rank bound are different quantities that happen to share a range;
gating one on the other would look like a defence while being a category
error. The receiver's protection at this layer is its own budget and its own
staleness bound, both local, neither claimed to be more than that.

### 3.5 Body is data, not program — again

§4.2.3 applies unchanged to every `%ad` inside a `%peers` list, on both sides.
The allow-list check (`ensure_literal_body`) **must run before any evaluation**,
exactly as repaired in v0.2.50.

This is the arc's highest-risk surface: a `#discover` response is remote input
that arrives in reply to a request *you* initiated, which is precisely the class
of input people forget to distrust.

### 3.6 Budget (R-b)

*   `MAX_DISCOVER_PEERS = 8` entries per response, applied after the exclusions
    of §3.2.
*   The responder must not emit a response body larger than **64 KiB**; if the
    capped entries would exceed it, emit fewer.
*   The delivery must **report the measured amplification ratio** (response
    bytes ÷ request bytes) at the cap, for a directory holding ≥ 12 matching
    entries. A number, not an assurance.

### 3.7 Record and directory changes

`PeerAdvert` must retain what relay needs:

*   the **verbatim source** of `%ad` — the exact substring of the received
    request line, `signature` included. Not a re-serialisation of the parsed
    value: the point is that a relay can be checked byte-for-byte (§3.3);
*   `ts` and `ttl` (both signed);
*   the `%hops` at which it arrived (`0` for a direct `#advertise`);
*   keep `received_at` for observability and for the staleness bound of R-f.

Exclusion is applied on read (search and relay), not by a background sweeper.
No new thread.

**The directory must not enter the universe.** SPEC_13 §4.1.2 obligation #3:
no engine-local, non-deterministic value may be minted into universe content.
Pin P3 measures this.

### 3.8 `ttl` validation on `#advertise`

Range-check at acceptance: `ttl` outside `0..=15` → `#rejected` with
`%reason: #malformed`. This tightens an existing entry point; pin P1 must show
that no previously-accepted *valid* advertisement is now refused. Note that
`0` is **valid** and means "do not relay me" (R-d).

### 3.9 Log lines

The responder logs, one line per served discover:

```
OODP Discover: target=<caid> matched=<n> capped=<n> excluded=<n no_relay,n stale> from=<%from claim>
```

The querying node logs, one line per response processed:

```
OODP Discover reply: peers=<n> accepted=<n> dropped=<n> (<reason counts>) stale_bound=<secs>
```

`stale_bound` is in the log because R-f made it a local policy, and a local
policy that changes results must be visible to whoever reads the output.

Probes read these; keep the shapes exact.

### 3.10 CLI

```
oo node discover --to <host:port> --target <caid>
```

Mirrors `oo node advertise`. Prints `%status`, then one line per accepted peer:

```
<node_id> <observed_host>:<listen_port> (host unverified, hops=1 claimed)
```

The parenthesis is **required output**, not decoration — it is R-a surfacing at
the only place a human reads it.

---

## 4. Deliverables

1. `PeerAdvert` retains verbatim body, `ts`, `ttl`, arrival `%hops` (§3.7).
2. `ttl` range check on `#advertise` (§3.8).
3. `#discover` served: search, exclusions, cap, response (§3.1–§3.6).
4. `#discover` reply verification on the querying side (§3.4), with the
   `ensure_literal_body` gate ahead of any evaluation (§3.5).
5. `oo node discover` CLI (§3.10).
6. Log lines (§3.9).
7. Remove `#[ignore]` from the probes in
   `crates/oo/tests/discover_index_probe_test.rs` (see §6). **Nothing else in
   that file may change.**

---

## 5. Out of scope — do not deliver

*   **Kademlia**: no k-buckets, no XOR distance, no `FIND_NODE`, no iterative
    lookup, no `.oo/routing/buckets.dat`. Next arc.
*   **Multi-hop relay.** This arc serves `%hops: 1` only. The budget machinery
    is built so the next arc does not have to redesign the packet, but nothing
    here forwards a record it did not receive directly.
*   **Discovered peers must not become fetch sources.** `~%Discovery./fetch`
    and `./find` keep exactly today's source set. Letting a local fetch dial a
    host it learned about is a consent question that deserves its own arc.
    **M3 therefore stays red after this arc** — say so, do not claim otherwise.
*   **No new language primitive.** There is no `~%Discovery./discover`.
    SPEC_13 §6.1/§6.2 do not list one, and adding language surface with no spec
    clause is what the v0.2.50 acceptance had to revert.
*   **No lifetime field on the advertisement** (R-f). Spec change; acceptor's.
*   No persistence of the directory to disk.
*   No changes to `gbb_registry`, LADD routing, or `disc.find`.
*   **No spec files.** Not REAL_02, not APP_05, not CHANGELOG. Spec closure is
    the acceptor's job.
*   No `git add -A`. No tags. No commits on `top`.

---

## 6. Gates

Probes are pre-committed in `crates/oo/tests/discover_index_probe_test.rs`,
written by the acceptor before this order was sent. They are calibrated: every
R is red at v0.2.50 **for the stated reason**, every P is green at v0.2.50.

**You may remove `#[ignore]` and nothing else.** If a probe looks wrong, report
it; do not repair it. A probe changed by the delivery has measured nothing.

### 6.1 Calibration record — measured on v0.2.50 before this order was sent

Workspace baseline: **1568 passed, 0 failed, 15 ignored** (`cargo test
--workspace`). The 15 ignored are this file's 12 reds plus 3 pre-existing.

Pins: `5 passed; 0 failed`. Reds: `0 passed; 12 failed`, each on the assertion
named below.

| # | Baseline failure — the reason it is red |
| :-- | :--- |
| R1 | `%status: #not_implemented`, `%peers` absent |
| R2 | nothing to verify — `#not_implemented` |
| R3 | **liveness**: `unrecognized subcommand 'discover'` |
| R4 | **liveness**: `unrecognized subcommand 'discover'` |
| R5 | **liveness**: the querier never contacted the relayer |
| R6 | the `ttl: 1` record was not relayed — `#not_implemented` |
| R7 | honest half: `#not_implemented` |
| R8 | **`ttl: -1` is accepted with `#success`** — the range check does not exist |
| R9 | **liveness**: twelve advertisements landed and none came back |
| R10 | **liveness**: `unrecognized subcommand 'discover'` |
| R11 | **liveness**: `%from=""` produced no record |
| R12 | **liveness**: the index answered nothing even for a service it holds |

Read the "liveness" rows carefully: those probes fail on their *witness*
assertion, not on their security assertion. That is deliberate. A probe whose
security assertion can be satisfied by a component that never ran has measured
nothing — the failure mode this arc's own R5 exists to prevent, and the one
that let v0.2.50's real defect through.

R8 is the odd one out and the most informative: it is red because a check that
§4.2 already specifies has never existed, not because this arc's feature is
missing.

Note also what P3 caught during calibration: the first draft compared **commit**
CAIDs, which carry timestamps and parents and are supposed to differ. The
obligation of SPEC_13 §4.1.2 #3 is about the **root** the commit points at. The
probe now reads `commit["root"]["digest"]`, as `universe_determinism_probe_test`
does — whose own header records an earlier version of the same mistake.

### Reds — must go green

| # | What it holds |
| :-- | :--- |
| R1 | B, holding A's advert, answers C's `#discover` with a `%peers` list naming A |
| R2 | the relayed `%ad` verifies under A's key at a party that never spoke to A — i.e. relay is genuinely verbatim |
| R3 | a relayed record with a tampered signature is dropped **while a good record in the same response is kept** |
| R4 | a relayed record whose `node_id ≠ CAID(public_key)` is dropped **while a good record in the same response is kept** |
| R5 | **a relayed `%ad` whose body is an expression that would compute** is rejected as `#malformed` **and the effect does not occur** |
| R6 | `ttl: 0` is not relayed, **while `ttl: 1` from the same directory is** |
| R7 | the relay bound binds the honest index and not the wire: an honest node refuses to emit a `ttl: 0` record, **and** a fake relayer hands you that same record and is believed — the limitation, in code |
| R8 | `ttl` outside `0..=15` is `#rejected #malformed` at advertise time |
| R9 | the cap holds: 12 matching entries in the directory, ≤ 8 in the response |
| R10 | `%observed_host` is outside the signature: altering it does not break signature verification, and the CLI still prints `(host unverified, …)` |
| R11 | the outcome is byte-identical under three different `%from` values (R-c) |
| R12 | a target nobody advertises → `#success` with `%peers: []`, not `#not_found` |

R5 exists because of the v0.2.50 lesson, which is now a standing rule:
**an adversarial case at a remote-input entry point must include a payload that
computes, not only payloads of the wrong shape.** The v0.2.50 gate sent
`%ad: 7` — a scalar that evaluates harmlessly — and the arc's real defect walked
straight past it.

R7 and R10 are probes that pin what does **not** work. They exist so that no
later reader mistakes the hop budget for a defence or the relayed host for a
verified address. A gate that only records successes teaches the next person
the wrong thing.

### Pins — must stay green

| # | What it holds |
| :-- | :--- |
| P1 | the whole existing `advertise_wire_probe_test` suite, including that a valid advertisement is still accepted after §3.8 |
| P2 | the whole existing `peer_fetch_verification_probe_test` suite |
| P3 | **universe determinism**: two fresh workspaces, same source, same root digest — measured on nodes that have accepted adverts and served discovers (SPEC_13 §4.1.2 #3) |
| P4 | `~%Discovery./find` and `./fetch` behave exactly as at v0.2.50 |
| P5 | `oodp_packet_format_probe_test` unchanged; `#fetch` and `#advertise` still answer `%hops: 0` |

---

## 7. Acceptance numbers to have ready

Report these as numbers, each with the command that produced it:

1. Full test suite, before and after, both counts.
2. The amplification ratio at the cap (§3.6).
3. M3's three lines re-run — both controls still non-empty, middle line still
   red (it is *supposed* to stay red this arc; a green there means something
   was delivered that was out of scope).
4. R5's effect check: the exact path the payload targeted, shown absent.
5. P3's two root digests, printed in full and shown equal.
6. A cross-version check: a v0.2.50 node and the new node exchanging
   `#advertise` in both directions, and the new node's `#discover` against a
   v0.2.50 node (which must answer `#not_implemented` — and that must be
   handled, not treated as a malformed reply).

Point 6 is not optional. The previous arc's cut record measured cross-version
`#fetch` in both directions; discovery meets more old nodes than fetch does.

---

## 8. Ledger items observed while measuring — not this arc

1. `reader.read_line` is unbounded (pre-existing, v0.2.48).
2. Two CAID computation paths disagree — `~%Discovery./identify` versus a bare
   `content_hash()`. Still unexplained. Do not investigate here.
3. `mod advert_debug` with `println!` is still in the engine (v0.2.50 leftover).
4. `.oo/routing/buckets.dat` appears in REAL_02 §5.1's layout and does not
   exist. Kademlia arc.
5. `@GPP_Response` (APP_05 §5.1) kept the GPP name through discussion 026's
   rename and violates the naming rule landed at APP_02 §0 / REAL_01 §7.6.2.
   Spec-side; acceptor's.
6. REAL_02 §7.4's "TTL 建議值 300 秒" is not merely a different TTL from the
   advertisement's `ttl` — it is a different **kind**: a duration versus a
   count. APP_05 §5.1 puts both in one structure, and its `witness_proof`
   comment ("已驗過且在 TTL 內可省略") means the duration one. Spec-side;
   acceptor's.
7. The advertisement declares no lifetime (R-f). Spec-side; acceptor's.
8. **Three budgets coexist and none of them is the original one.** `ttl`
   (APP_05 §2.2 / REAL_02 §4.2, signed, a lattice quantity), `fuel_limit`
   (APP_05 §2.3, the querier's execution budget) and `MAX_ROUTING_HOPS = 16`
   (hardcoded in `disc.rs`). The third exists because §4.4's mandatory random
   jumps — the Semantic Eclipse defence of §7.3 — break the monotonicity the
   first one relied on. The spec has never said this out loud, and it should:
   the honest self-authenticating budget was traded for the ability to escape
   a captured neighbourhood. Spec-side; acceptor's.
8. Stray `oo node serve` processes from earlier arcs' probe runs are still
   listening on ports 19551–19992 on this machine. A probe that connects to a
   "free" port can hit one. Bind and connect above 21000 for this arc.


---

## 9. Delivery record (delivery side)

- **PeerAdvert**: `ad_source` (verbatim), `ts`, `ttl`, `hops` (0 on direct
  advertise), `observed_host`, `listen_port` retained for relay.
- **#advertise**: `ttl` must be in `0..=15` else `#rejected #malformed` (0 is
  valid — "do not relay me").
- **#discover**: search `peer_adverts.services` exact match; exclude `ttl==0`
  and `|now−ts|>15min` before cap; `MAX_DISCOVER_PEERS=8`, body ≤64KiB;
  response `%hops:1`, `%peers: [{%ad: <verbatim string>, %observed_host}]`.
  Empty → `#success` + `%peers: []` (not `#not_found`). Store not consulted.
- **Client**: `process_discover_reply` / `oo node discover` — §3.4 ladder per
  entry (literal-body first); drop entry not response; CLI prints
  `(host unverified, hops=N claimed)`.
- **Probe**: only 12 `#[ignore]` removed on discover_index. advertise_wire P3
  updated (discover is now served; missing `%target` → `#conflict`).
- **Spec**: not edited.
- **Amplification at cap** (synthetic 8 peers, r9-shaped): response **4139** B /
  request **125** B → **ratio 33.11×** (< 64). r9 gate green.
- **Root digests** (P3 source `world: { greet, n }`):
  `9ecb95b0c6088b348d582c8f0ed03fa499ff7b6d4cb0b71f3eba70bf9f339ae3`
  (quiet == busy shape; directory does not enter the root).
- **M3** (still red by design): holding a verified advert does **not** make
  `disc.fetch` dial that peer — out of scope.
- **Numbers**: discover_index **17/17** · advertise **19/19** · oodp **13/13** ·
  workspace **1580/0/3** · conf **143/143** · genesis **11/11**.
