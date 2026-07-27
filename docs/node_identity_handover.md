# Work order — the node introduces itself by its port number

Arc opened 2026-07-27, after v0.2.48.
Acceptor: project brain. Implementer: model #3.
Expected classification: decided at acceptance. `%from` is **additive**
(measured: the v0.2.48 parser tolerates unknown fields, so a new client still
talks to a v0.2.48 node). What makes it breaking is D5 — retiring the legacy
bare-CAID request, which v0.2.48's own spec clause dated to this arc.

**Spec closure is NOT in scope.** The previous work order put it in delivery's
scope and the implementer ended up writing the CHANGELOG entry that classified
their own change — the shape v0.2.45 named 「由被檢查者供給檢查名單者,其檢查
恆真」. §4 lists what the spec will say so the delivery knows the target; the
acceptor writes it.

---

## 0. The headline, measured on v0.2.48

```rust
let source_id = format!("node:{}", port);     // main.rs:235
```

**A node introduces itself by its listening port.** Two nodes on port 19831 on
different machines are the same `%source`; one node restarted on another port is
a different one. `%from` is absent entirely, so a request carries no notion of
who is asking.

And `ladd.rs`'s `node_caid` is a misnomer — `disc.advertise` fills it with
`arg.content_hash()`, the CAID of the **advertised value**. Nothing in the engine
holds a node's own identity.

## 1. The constraint that settles what a node identity is made of

Two fresh workspaces, same source, measured:

```
a root: 5a2ec0a175ec4f089c6c5d9f6d939507…
b root: 5a2ec0a175ec4f089c6c5d9f6d939507…
```

v0.2.45 made that determinism a virtue. It also makes the universe **unusable as
a node address**: every node serving the same universe would occupy the same DHT
slot.

> The property that makes a universe federatable is exactly the property that
> disqualifies it as a node address. Content addressing answers *what*; a node
> address has to answer *which of the many holders* — different questions by
> construction.

So the node identity is a **keypair**, not a derivation from content. REAL_02
§4.2's `signature: b""` needs one anyway.

**Spec gap found**: §4.1 says 「節點 ID = **CAID 的**內容指紋(content_digest)
前 160 bit」 and never says what that CAID addresses. This arc answers it: the
CAID of the node's **public key**.

## 2. Rulings

**Q1 — the workspace path is part of the node's identity.**
The key lives at `~/.oo/nodes/<digest of the workspace's absolute path>`.

Not in `.oo/`: v0.2.46 already ruled that a secret must not live inside a
shareable artifact, and `.oo/objects` is precisely the thing designed to be
served and copied. That ruling applies verbatim here; it is not reopened.

Path-derived, because **the engine cannot distinguish a moved workspace from a
copied one** — it sees only that the path changed. Making the path part of the
identity means a copy is structurally a different node: no detection, no
heuristic, nothing to ask the operator. The cost is that `mv` gives the workspace
a new node identity. With no DHT yet that costs nothing today; later it means
peers re-learn it.

**Q2 — every workspace that touches the network is a node.**
Consistent with 節點 ≙ `.oo/`: a workspace that only fetches is a node that
happens not to be listening. Mint lazily at first network use (fetch or serve),
exactly as v0.2.46 mints the operator key at first signing need. `%from` is then
always present rather than a field that is sometimes empty.

**Q3 (acceptor, not asked) — the node key is independent of the operator key.**
Different questions: the operator key answers *who authorises* (governance,
`#refine`); the node key answers *which machine answered*. Signing the node key
with the operator key — "this node is operated by X" — is the natural extension
when advertisements need to be trusted, and it is **not** this arc.

## 3. Scope

### D1 — the node keypair

* Path: `~/.oo/nodes/<hex digest of the workspace absolute path>`. Provide an override for tests — **`OO_NODE_HOME`**, an absolute directory that replaces `~/.oo`; relative must be refused, as `OO_IDENTITY` is.
* Reuse everything v0.2.46/v0.2.47 already learned, because the same mistakes are available here: PKCS#8 only, public key derived on load; `create_new` + `mode(0o600)` so the file is never world-readable even briefly and a concurrent first mint yields **one** key with every process reporting the key on disk; parent directory moded only when the engine **creates** it; an unreadable key file is **refused, never overwritten**.
* Lazy: minted at first network use. `oo run` / `evolve` / `commit` on a workspace that never touches the network must not mint one.

### D2 — `node_id`

`node_id` = the CAID of the node's public key; the DHT address is its leading
160 bits (REAL_02 §4.1). Say in the code comment what the CAID addresses — that
is the gap this arc closes.

### D3 — `%from` on requests, and `%source` on responses

* Every OODP request carries `%from: <node_id>`.
* `%source` becomes the node id. Two ports on one workspace ⟹ the same `%source`; two workspaces ⟹ different.
* **`%from` is a claim, not an authentication.** Nothing may depend on it: it is unsigned, and any peer can put any value there. Serving, verification and every outcome must be identical whatever `%from` says. Say so in the code comment, and expect a pin that forges it.

### D4 — `oo node id`

Prints this workspace's node id and the path its key lives at — the same shape as
`oo identity`, and the surface R1 reads. It must print the id that `%from`
actually carries.

### D5 — retire the legacy bare-CAID request

v0.2.48 accepted it as a **declared, dated** transition surface and the spec says
its removal happens here. A bare CAID now gets an envelope answering `#conflict`
(as any other malformed request does) rather than being served.

### Out of scope

* `#advertise` / `#discover` on the wire, `ServiceAdvertisement`, its `signature`, and Kademlia routing (REAL_02 §4). The node now *has* the key those need; using it is the next arc.
* Delegation (node key signed by the operator key) — Q3.
* The control surface (REAL_01 §2 / §8.4).

## 4. What the spec will say (acceptor writes this — do not edit the spec)

* **REAL_02 §4.1**: what the node's CAID addresses (the public key), closing the gap.
* **REAL_02 §3.2**: `%from` is carried; it is a claim and unauthenticated; the transition-period bare-CAID form is removed as dated.
* A clause on where the node key lives and why the path is part of the identity, cross-referencing REAL_01 §7.5's secret-not-in-a-shareable-artifact rule.
* **REAL_01 §7.5**: a sentence distinguishing the operator key from the node key, so the two are not read as one.

## 5. Probes

`crates/oo/tests/node_identity_probe_test.rs` — written and calibrated by the
acceptor before delivery starts. Delivery removes **only** `#[ignore]`.

Expected shape: R1 is the headline and is **paired both ways** — a copy at a
different path must be a *different* node, and the same workspace across
processes must be the *same* node. An engine that minted a fresh key every time
would pass the first half; one that hard-coded a constant would pass the second.

## 6. Acceptance

1. Diff purity — probes are the acceptor's; if one is wrong, **say so instead of accommodating it**.
2. Numbers, re-run by the acceptor.
3. Adversarial, at minimum: a copied workspace served alongside its original; a forged `%from`; a corrupt node key file; concurrent first mint; `OO_NODE_HOME` relative; a workspace whose path contains spaces or non-ASCII; the operator key and node key must not be confusable (one must not be usable as the other).
4. A/B against v0.2.48.
5. Classification by measurement — value CAIDs, universe root and commit CAIDs must be untouched; the protocol question is D5.
