# `#advertise` on the wire — work order

Arc opened 2026-07-27, on top of **v0.2.49** (`top 7e951fa`, `dev 0608d29`).
Probes are pre-committed: `crates/oo/tests/advertise_wire_probe_test.rs`,
**9 red (`#[ignore]`d) + 10 pin**, calibrated on v0.2.49.

---

## 1. What was measured before this order was written

All commands run against the v0.2.49 debug binary.

**M1 — a node cannot serve its own public key.**

```
$ oo node id
hash:sha256:v2:_:…:8a2add2bb7e0c2c18c75bb997157e3b04008d4b9ce63f911412d5bf7e718e6f9
path: …/nodes/734d970cd322d15a…

$ printf '{{ %%op: #fetch, %%hash: "hash:…8a2add2b…", %%from: "x" }}\n' | nc 127.0.0.1 19551
{"%hops":0,"%source":"hash:…8a2add2b…","%status":"#not_found"}
```

The key is on disk. It is not an object. Nothing can resolve `node_id` → key.

**M2 — the specified packet cannot carry it either.** REAL_02 §4.2 and §7.1 give
`ServiceAdvertisement` the fields `node_id: @caid`, `signature: b""`, and no
public key. Ed25519 does not verify against a hash.

> REAL_02 §7.2 obligation #1 — "verify `signature` matches the public key
> corresponding to `node_id`" — **has never been satisfiable**. Same shape as the
> v0.2.46 whitelist no value could satisfy: an unsatisfiable check is not a
> stricter check, it is no check.

**M3 — there is no address anywhere in REAL_02.** `%source` is an *identifier*;
§4.1's routing buckets never say what a bucket entry holds. v0.2.49 replaced
`source_id = format!("node:{}", port)` with the node id — correct, a port is not
an identity — but the port had been doing two jobs, and fixing the identity job
left the *location* job homeless.

**M4 — the wire is a faithful carrier of values.** A value with a string, a
float, a tag list and an int survives `Value → serde_json → wire → Value` with
the client's CAID recomputation matching (otherwise `⊥ #caid_mismatch`):

```
$ oo run f.n            # B fetches from A over OODP
out: {{ pkg: "demo", ratio: 0.5, tags: [#a, #b], version: 3 }}  ;; %effect: #io
```

This is what licenses signing over a **CAID** rather than over a canonical byte
string: the system already has exactly one canonical encoding and it survives
the wire.

**M5 — CAID sensitivity, measured directly:**

| | effect on the CAID |
| :-- | :-- |
| field order (`{{a,b}}` vs `{{b,a}}`) | **none** — a Combo is an unordered record |
| an extra field | **changes it** |
| list order (`["a","b"]` vs `["b","a"]`) | **changes it** |

Consequences, both binding on the implementation: the body may be reconstructed
in any field order, and `services` **must not** be sorted, deduped or otherwise
normalised before hashing.

**M6 — the precedent already existed.** `authority.rs`'s `AuthorityInfo` carries
`signer_pubkey_hex` — the key itself, inline. The operator path solved this on
day one; the wire spec just never followed.

---

## 2. Rulings carried into this order

**Q1 — the address is half observed and half claimed.**

| part | source |
| :-- | :-- |
| host | **observed**, from the connection the advertisement arrived on |
| port | **claimed**, inside the signature — a listening port cannot be observed, and the source port is ephemeral and is not it |

An advertisement can therefore only ever describe *the machine you are already
talking to*. A signed claim can never name a third party, so the reflection
vector closes structurally instead of by a check. A NAT'd node simply cannot
advertise a reachable address — that is true, and must not be papered over.

**Q3 — `%status: #rejected` + a mandatory `%reason`.** The status set stays small
and stable across ops; the discrimination lives in `%reason`. Collapsing four
rejections into one `#conflict` would be v0.2.48's `#timeout` mistake one level
down.

**Q2 — GPP is deferred to its own discussion (disc 026). Nothing in this arc
touches REAL_02 §7, APP_02 §6, APP_05 §5 or SPEC_15 §7.**

---

## 3. Design

### 3.1 The advertisement

The signed **body** — the advertisement before `signature` is added:

```nlang
{{
    node_id:     "hash:sha256:v2:…"   ;; CAID of the public key bytes
    public_key:  "<64 hex>"           ;; raw Ed25519 public key
    services:    ["hash:…", …]        ;; may be empty; order is significant (M5)
    listen_port: 8080
    capacity:    10
    ts:          1785130396           ;; unix seconds
    ttl:         15
}}
```

The full advertisement is that plus `signature: "<128 hex>"`.

### 3.2 What is signed

```
payload   = "oodp-advert:v1:" ++ CAID(body).to_string()
signature = Ed25519(node_private_key, payload)
```

Two properties fall out and both are wanted:

* **Domain separation.** The `oodp-advert:v1:` prefix keeps a signature over an
  advertisement from being replayable as a signature over anything else that
  hashes values (`refine:` in `authority.rs` is separated the same way).
* **The signature commits to a value, not to an encoding.** Because the payload
  is a content address, a peer may re-encode the advertisement (JSON ↔ cocoon,
  any field order) without breaking the signature. Most protocols need canonical
  serialisation to get this; content addressing gives it for free.

`CAID(body)` is the engine's ordinary `Value::content_hash()` of the parsed body.
Receiver side: parse `%ad`, **remove exactly the `signature` field**, hash the
rest.

### 3.3 The request

```nlang
{{ %op: #advertise, %from: "<node_id>", %ad: { …advertisement… } }}
```

Both the existing encodings (n/ cocoon, JSON envelope) continue to be accepted;
`%ad` is an object/combo in either.

### 3.4 Verification, in this order

| # | check | on failure |
| :-- | :--- | :--- |
| 1 | `%ad` present, a combo, all required fields present incl. `public_key` and `signature` | `#rejected` `%reason: #malformed` |
| 2 | `CAID(public_key bytes) == %ad.node_id` | `#rejected` `%reason: #identity_mismatch` |
| 3 | `%from == %ad.node_id` | `#rejected` `%reason: #identity_mismatch` |
| 4 | signature verifies against `public_key` over §3.2's payload | `#rejected` `%reason: #bad_signature` |
| 5 | `|now − ts| ≤ 60s` | `#rejected` `%reason: #stale` |
| — | otherwise | `#success` |

Order is normative and gates R3: an engine that only asks "does the signature
verify against the key in the packet" accepts every forgery, because the forger
supplies both halves. The identity binding must be checked, and it must be
**named** — `#identity_mismatch`, never `#bad_signature`.

`CAID(public_key bytes)` is `Identity::node_id_caid()`'s computation:
`Value::Atom(AtomKind::Bytes(pk)).content_hash()`.

### 3.5 What is *not* verified

`services` is a **claim**. A node cannot know what a peer holds. Accepting an
advertisement that lists a CAID the advertiser does not have is correct
behaviour (R8) — the lie costs the liar a wasted round trip later, when `#fetch`
either produces a self-authenticating object or does not. An engine that refused
here would be asserting knowledge it does not have.

`capacity` likewise. Signing a claim does not make it true; it makes the liar
attributable.

### 3.6 The peer directory

In-process, keyed by `node_id`, holding: public key, services, `addr` =
observed host + claimed port, capacity, ttl, received-at.

**It is written and read by nothing.** No fetch path consults it in this
version. That is deliberate; routing is the discover arc. Do not wire it into
`disc.fetch`'s peer sweep — P5 and the scope rules below both forbid it.

### 3.7 Log lines (R7 reads these)

```
OODP Advert: <node_id> addr=<host>:<listen_port> services=<n> ttl=<n>
OODP Advert rejected: #<reason> from=<%from|?> (<detail>)
```

`addr=` must carry the observed host and the claimed port. Logging the
connection's source address elsewhere is fine; recording it *as the peer's
address* is not.

### 3.8 CLI

```
oo node advertise --to <host:port> [--service <caid>]... [--listen-port <n>]
```

Sends this workspace's signed advertisement and prints the peer's `%status` and,
when rejected, `%reason`. `--listen-port` defaults to 8080 (matching
`oo node serve`). Zero `--service` is legal: an advertisement with an empty
service list is a liveness announcement.

---

## 4. Deliverables

* **D1** — `#advertise` served: parse `%ad`, the §3.4 ladder, `#success`.
* **D2** — `%reason` on responses, present **iff** `%status` is `#rejected`.
* **D3** — advertisement construction + signing (§3.1–§3.2), sender side.
* **D4** — the peer directory (§3.6) and its log lines (§3.7).
* **D5** — `oo node advertise` (§3.8).
* **D6** — all 9 reds pass with **only** `#[ignore]` removed; all 10 pins stay green.

---

## 5. Out of scope — do not deliver

* **GPP / REAL_02 §7 / APP_02 §6.** Deferred to disc 026. No `gpp_proof`, no
  `fingerprint_commitment`, no trust levels.
* **`#discover`, Kademlia, routing tables.** P3 pins `#discover` as still
  explicitly unimplemented.
* **Reading the peer directory** from any fetch path.
* **Any language-layer surface.** No new `~%Discovery./…` morphism, no new
  system-module member. The wire op and the CLI only — this is what keeps the
  universe root untouched, and the root is measured at acceptance (§7).
* **Persisting the directory to `.oo/`.** Pointers are the assertion stratum
  (SPEC_08 §6.3) and that is a separate decision.
* **Spec changes and CHANGELOG entries.** Spec closure is the acceptor's, per
  the standing rule from v0.2.48: a delivery that writes the entry classifying
  its own change is the v0.2.45 shape — the checked party supplying the
  checklist.

---

## 6. Gates

`crates/oo/tests/advertise_wire_probe_test.rs`. Calibrated on v0.2.49: every red
fails naming `#not_implemented`; every pin passes.

| red | what it decides |
| :-- | :--- |
| R1 | a correctly signed advertisement is accepted |
| R2 | a body altered after signing is `#bad_signature` — pairwise against the unaltered control |
| R3 | a **valid** signature under a key whose CAID ≠ the claimed `node_id` is `#identity_mismatch`, **not** `#bad_signature` |
| R4 | an hour-old `ts` is `#stale`; the same advertisement fresh is `#success` |
| R5 | missing `%ad`, scalar `%ad`, and unsigned `%ad` are all `#malformed` |
| R6 | after a valid advertisement from the same key, one without `public_key` is still refused — no remembered/derived/fetched fallback |
| R7 | the recorded address is observed-host + claimed-port, and is **not** the ephemeral source port |
| R8 | advertising a CAID the advertiser does not hold is `#success` |
| R9 | `%from ≠ %ad.node_id` is refused, not silently resolved |

| pin | what it protects |
| :-- | :--- |
| P1 | `#fetch` still serves, and `#not_found` is still distinguishable |
| P2 | `%from` stays a **claim** on `#fetch` — five spellings incl. absent, all `#success` |
| P3 | `#discover` still `#not_implemented` |
| P4 | unknown `%op`, bare CAID and garbage all still `#conflict` |
| P5 | local LADD unchanged **in both directions** (its `#semantic_eclipse` verdict is pinned, not endorsed) |
| P6 | the node private key still refused at the language boundary |
| P7 | an advertisement adds no objects to the receiver's store (store proved non-empty first) |
| P8 | advertising does not move the workspace's own state |
| P9 | the node id is stable and advertising does not rotate it |
| P10 | the operator key and the node key stay two different keys |

The probes are the acceptor's. **Change nothing in that file but the nine
`#[ignore]` lines.** If a gate looks wrong, say so and stop — do not accommodate
it. (v0.2.39 and v0.2.42 both had deliveries quietly bend to an acceptor error;
the standing rule exists because the root cause was mine both times.)

---

## 7. Acceptance numbers to have ready

* `cargo test -p oo --test advertise_wire_probe_test` — 19/19, no `#[ignore]`.
* Full workspace suite; conformance; genesis.
* **The universe root digest, before and after.** Classification is not
  pre-committed here: the v0.2.46 lesson is that a tree-wide scan governs
  whether *programs* break, not whether *addresses* move. Report the root digest
  from a fresh workspace on v0.2.49 and on the delivery; the acceptor decides
  the CHANGELOG class from that plus the cross-version matrix.
* Cross-version: a v0.2.49 client's `#fetch` against the new node, and the new
  client's `#fetch` against a v0.2.49 node. Both must still work — this arc adds
  an op, it does not change `#fetch`.

---

## 8. Ledger items observed while measuring — not this arc

* `~%Discovery./advertise` followed by `~%Discovery./find` on the same key
  exhausts the 16-hop budget and returns `⊥ #semantic_eclipse`. Only the
  explicit-`target` direct-lookup path resolves today. Pinned as-is by P5.
* `disc.connect` takes positional fields `"0"`/`"1"`; the named spelling
  silently yields `#false` rather than a diagnosable refusal.
