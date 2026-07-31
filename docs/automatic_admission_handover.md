# Automatic admission + hard cap — opening handover

> 開單日：2026-07-31。這是 provenance v0.8.0 之後的新弧；本提交只開工單與
> acceptor-owned probe，不實作准入、來源上限、驅逐或新的線上格式。
>
> Engine release anchor: `nlang-tools` `top` story `5a030f3` (`v0.8.0`),
> post-release `dev` tie-back `f15558c`.
> Spec release anchor: `nlang-spec` `top` story `90f1440`
> (`v0.8.0-draft.1`), post-tag `local` sync `7a2b9d4`.
> Superproject pointer: `cc89c9b`.

## 1. Opening baseline

Measured on post-release `nlang-tools/dev` `f15558c`, with the opening tree
still clean and before this order/probe was written:

| Gate | Result |
|---|---:|
| Full workspace | **1732 passed / 0 failed / 3 ignored** |
| Full-workspace result blocks | **178** |
| Direct-observation provenance | **11/11** |
| Advert persistence | **19/19** |
| Advertise wire | **19/19** |
| Discover index | **17/17** |
| Affiliation claim | **20/20** |
| Discovery trust | **20/20** |
| Connect consent | **9/9** |
| Local GC | **17/17** |
| Kademlia table | **17/17** |
| Peer fetch verification | **12/12** |
| Conformance | **143/143** |
| Genesis | **11/11** |
| `cargo fmt --all -- --check` | pass |

The release binary reports `oo v0.8.0`; the release spec is
`v0.8.0-draft.1`. No fmt, `.oo/format`, CAID, wire-byte, or existing owner-suite
baseline is allowed to move while opening this arc.

## 2. The question

A receiver may now know, in receiver-local metadata, whether an exact signed
advertisement was seen directly, learned through a relay, or loaded without a
known observation:

```text
direct > relayed > unknown
```

The next question is deliberately narrower than general peer trust:

> When may a receiver turn one of those records into an active remote fetch
> source, and how many such automatic sources may consume the receiver's time?

This is not a claim that a signed advertisement is truthful. `services`,
`capacity`, and the affiliation claim remain signed assertions. The source
still has to return bytes whose CAID verifies. The new decision governs whether
the receiver may spend connection/fetch time on the asserted address.

## 3. Existing facts the delivery must consume

### 3.1 Two different maps

`Ouroboros` has two separate maps:

- `peer_adverts: RwLock<HashMap<String, PeerAdvert>>` — the durable signed
  advertisement directory and routing input;
- `peers: RwLock<HashMap<String, Peer>>` — the process-local active fetch-source
  map, whose entries are `Peer::Local` or `Peer::Remote(String)`.

`#discover` currently writes only the first map. `~%Discovery./connect` is the
only active remote-source writer and remains the manual-consent path. The
source map currently has no node ID, provenance, admission class, timestamp,
TTL, persistence, source cap, or eviction metadata. Caller-selected names are
not unique addresses: two names pointing to one address are two map entries.

The delivery must not silently define a cap as `peers.len()` without first
choosing the cap domain. In particular, `MAX_DISCOVER_PEERS = 8` and Kademlia
`K = 20` are unrelated bounds and are not candidate source-cap values.

### 3.2 Durable and derived inputs

The durable peer record contains the verbatim signed `%ad` plus receiver-local
observer fields. Same-owner restart restores the observer half; owner mismatch
(copy) clears it and decodes the provenance as `unknown`. Missing or unknown
provenance is conservative `unknown`. `verified_operator_key` is derived from
the verbatim advertisement at load/refresh and is not itself a durable
authority decision.

The workspace trust root is the closed-data file `.oo/discovery.n`:

```nlang
affiliation_roots: ["<64 lowercase hex Ed25519 operator key>"]
```

`Ouroboros::init` loads it once. A matching root alone currently creates no
source and makes no network call. `refresh_affiliations` refreshes claims, not
the roots; a running engine does not reload `discovery.n`.

### 3.3 Cost calibration

The existing measured source costs remain the budget that a cap must protect:

- no-source fetch floor: about **0.040 s**;
- one blackholed remote source: about **5.05 s**;
- three blackholed sources: about **15.09 s**;
- 150 silent source members: about **12.5 minutes** for a sequential fetch scan.

These are connection/fetch costs, not advertisement or signature costs. The
opening probe must not infer a source cap from the discovery response cap or a
routing bucket cap. The numerical cap is still an explicit policy choice.

## 4. Eligibility contract for reconnaissance and delivery

The opening reds express the following proposed contract. The implementation
must not weaken it by checking only `node_id` or only the last durable line.
An automatically admitted source must be tied to the **current exact signed
advertisement** and require all of:

1. the advertisement still passes the existing signature, identity, literal,
   TTL, and freshness ladder;
2. its exact-ad provenance is `direct`;
3. its directly observed host and signed `listen_port` form the source address;
4. its affiliation claim verifies now and is not expired;
5. the derived operator key is in this workspace's loaded
   `affiliation_roots`;
6. a newer/different signed advertisement for the same `node_id` is evaluated
   on its own provenance and claim; an old direct observation cannot bless it.

Therefore `relayed`, `unknown`, `%hops: 0`, a non-empty relay host, a TCP
connection to a relay, a copied observer half, an absent/invalid/expired claim,
and an unrooted claim are all ineligible. An ineligible or revoked candidate
must not leave a stale automatic source behind.

Admission must be lazy with respect to the network: accepting or loading an
eligible record must not dial the source. The first dial is the existing fetch
source scan. Existing byte-level fetch verification must continue to skip a
lying source and continue scanning.

## 5. Opening probe

`crates/oo/tests/automatic_admission_probe_test.rs` is acceptor-owned. The
controls are live at opening; the reds are `#[ignore]` until delivery. Delivery
may remove only those ignore attributes—no assertion, fixture, helper, or
control-line edits.

Controls establish non-empty, computing fixtures before any absence claim:

- `c0_direct_rooted_signed_fixture_is_live`: a real node key, real signed
  affiliation claim, workspace root, direct receipt, a live fetch payload, and
  a counting fake peer all exist;
- `c1_relay_and_legacy_records_are_live`: the relay path produces a real signed
  record and the same durable file can be reduced to a conservative legacy
  record without making the fixture empty.

Opening reds cover:

- `r1_direct_rooted_admission_inserts_without_eager_dial`: direct + valid +
  rooted becomes a source, but receipt itself does not contact it; the first
  unnamed fetch does and retrieves the computing payload;
- `r2_same_owner_restart_reconstructs_eligibility`: the source is reconstructed
  from the durable signed/observer halves after restart;
- `r3_copy_clears_direct_observation_for_admission`: copying a workspace cannot
  carry direct eligibility into a new owner;
- `r4_relayed_zero_hops_is_not_admitted`: relay provenance wins over a claimed
  zero hop count;
- `r5_unknown_legacy_record_is_not_admitted`: missing provenance never defaults
  to direct;
- `r6_unrooted_claim_is_not_admitted`: a valid claim by an operator outside the
  workspace roots is not consent;
- `r7_expired_claim_is_not_admitted`: expiry is re-evaluated and does not leave
  a stale automatic source;
- `r8_newer_relayed_ad_does_not_inherit_old_direct`: exact-ad identity prevents
  a newer relayed advertisement for the same node from inheriting an old direct
  observation or source slot;
- `r9_automatic_remote_cap_is_three_incumbent_first`: one unrooted candidate,
  three rooted low-capacity incumbents, and two rooted high-capacity late
  candidates establish that the automatic-only cap is exactly three and that
  late capacity does not evict the incumbents; manual and local sources remain
  outside that cap.

The existing `connect_consent`, `peer_fetch_verification`, `advert_persistence`,
`direct_observation_provenance`, `affiliation_claim`, `discovery_trust`, and
Kademlia suites remain the regression owners for manual connect, local sources,
fetch verification, persistence, provenance, claim/root semantics, and routing.
The opening probe does not duplicate or weaken those suites.

### Cap policy — decided 2026-07-31

The three cap choices are now explicit and are part of this arc's acceptance
contract:

1. **number: 3 automatic remote-source slots** — the measured cost of one
   silent source is about 5.05 seconds, so three sequential automatic sources
   bound the measured worst case at about 15.1 seconds. This number is derived
   from the time budget, not from discovery's `8` or Kademlia's `K = 20`.
2. **domain: automatic remote sources only** — a slot is consumed only by a
   source admitted from the current exact signed advertisement after direct
   provenance, the existing signature/identity/literal/freshness/TTL ladder,
   a currently valid affiliation claim, and a loaded affiliation root all
   succeed. Manual `connect` remotes and local sources do not consume these
   slots. The durable advertisement directory is not the capped collection.
3. **overflow: incumbent-first, no eviction** — the first eligible candidates
   observed or reconstructed occupy free slots. Later eligible candidates are
   not admitted while all three slots are occupied; `capacity`, freshness, or
   another preference does not evict an incumbent. An incumbent that becomes
   invalid, expired, revoked, or is replaced by an ineligible exact
   advertisement may be removed by revalidation. Automatic backfill after such
   removal and restart tie-breaking are separate questions and are not pinned
   by the opening cap red.

The implementation must represent the automatic admission class explicitly (or
with a sidecar); `peers.len()`, `peer_adverts.len()`, the discovery response cap,
and the Kademlia bucket cap are not substitutes for this domain. Admission
remains lazy with respect to the network: filling a slot must not dial its
source.

The acceptor-owned probe now contains a cap calibration red. It must establish
live manual/local and automatic fixtures before asserting that exactly the
incumbent set, rather than merely fewer than three sources, is active.

## 6. Scope gates

In scope:

- consuming released provenance and affiliation-root facts;
- an active-source admission seam;
- no-eager-dial behavior;
- exact-ad replacement/revalidation;
- restart/copy/legacy behavior;
- the later, explicitly chosen automatic-source cap.

Out of scope:

- changing signed advertisement bytes, CAIDs, `%discover` response shape, or
  `.oo/format`;
- changing manual `./connect` capability semantics;
- making relay assertions trustworthy;
- adding automatic dialing on receipt;
- inventing a source preference/ranking based on degree-0 correctness;
- persistence of the process-local source map unless separately chosen;
- runtime trust-root reload unless separately chosen;
- fixing the provenance ledger items (lock atomicity, duplicate-line replay,
  relay-only restart coverage) opportunistically.

## 7. Delivery and acceptance boundary

The acceptor owns this probe. Model #3 may implement the engine only after the
cap policy is explicit. A delivery diff that changes this file beyond removing
`#[ignore]` is rejected. Acceptance reruns the owner suite, full workspace,
conformance, genesis, and the unchanged provenance/trust/consent/fetch owners.
No version bump or tag belongs to this opening arc.
