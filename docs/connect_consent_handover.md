# 撥號需要同意 / Dialling needs consent — the gate

**Opened** 2026-07-30. **Baseline** `dev 160eac4` (engine v0.5.0).
**Probes** `crates/oo/tests/connect_consent_probe_test.rs` — pre-committed and
calibrated before this order was written. Workspace at baseline: **1696 passed,
0 failed, 8 ignored** (5 of those ignored are this arc's reds; 3 are standing).

**Classification: 破壞性 (Layer 1).** A program that works today stops working
without a flag. That is a language-surface semantic incompatibility, so the
ORDER_00 §5.1.4 clock restarts, and it restarts nine days after entry #7 did.
It is also a security repair; the classification does not change for that.

---

## 1. The defect

REAL_02 §4.2.6 says the fetch source set 「只由操作者顯式建立(`./connect`)」 —
is built only by the operator, explicitly. **That sentence describes an
intention the mechanism does not enforce.** `~%Discovery./connect` is a plain
language-layer builtin carrying `EffectTag::IO` and nothing else.

Measured on v0.5.0 — an ordinary program, no `--grant`, no `--privileged`:

```
added: ~%Discovery./connect ("stranger", "tcp://192.0.2.9:8080")
  → #true  ;; %effect: #io
a following ~%Discovery./fetch → 5.05 s, ⊥ #peer_timeout
```

The 5.05 s is the proof that the dial happened. So any program — including one
fetched from a peer and evaluated — can make this engine connect out to an
address of its choosing and tell that address which CAID it wants.

The containment that exists is **accidental**: the source set lives in process
memory, so a program's `./connect` dies with the process. Nobody chose that. It
is the same species as the `#discover` hash seed the sampling arc replaced.

---

## 2. Why this arc had to come before the one it was cut from

§4.2.6 also defers a ruling: whether a *discovered* peer may become a fetch
source, which it calls a consent question (同意權問題) to be settled on its own
rather than as a side effect of discovery. **That ruling cannot be made while
nothing gates who may add a source at all** — "under what consent may discovery
add a source" presupposes that consent exists.

### 2.1 What is at risk, and what is not

**Not integrity.** SPEC_13 §6.1.1 and REAL_03 §6.6: every source is a read
path, bytes are re-addressed and compared, and a failing source **must** be
skipped while the scan continues. A malicious source cannot hand you a wrong
answer.

What a source gets is:

| | nature | bounded? |
| :-- | :--- | :--- |
| the right to spend your time — **5 s per silent member, per fetch**, sequential, and §6.1.1 forbids bailing out | availability | cappable; a silent member is detectable |
| knowledge of which CAID you want | disclosure | **neither capped nor revocable** |

So the axis is **not security against convenience**. It is who may spend your
time and who may learn what you are looking for — which is why §4.2.6 called it
consent rather than trust. You do not need to trust a source for correctness;
verification handles that. You consent to *paying* it and to *telling* it.

Measured, linear: 1 blackholed source 5.05 s, 3 → 15.09 s. A closed local port
returns in 0.06 s, so only a dropped-packet address shows the cost.

---

## 3. Scope

### 3.1 The gate is on the remote form only

`~%Discovery./connect` with a `tcp://` address requires a capability. The local
form does **not**.

That is a ruling, not an omission: a local `ObjectStore` dials nobody and tells
nobody, so neither consent cost applies, and SPEC_08 §6.3 already governs the
path it opens (`crosses_store_boundary` refuses a store directory). **P1** holds
the local form open.

### 3.2 The capability

* **Name: `connect`** — matching the morphism. Existing grants are
  `effect_override[:tag…]`, `pin`, `rollback`, `squash`, `gc`; the CLI's own
  error message is the authoritative list and it must gain this one.
* **Per invocation, out of band** (SPEC_08 §6.1.2 / §6.1.4, REAL_01 §7.0.1):
  `oo run prog.n --grant connect`. **Not stored, not inferrable from `.oo/`.**
  §6.1.2's no-backdoor ruling applies verbatim. **R3, R5.**
* **A flag spelling is an interface**, so the probe asserts the word literally,
  for the same reason a wire format is pinned literally.

### 3.3 The refusal

* `⊥ %cause: #privileged_required` — the code already exists and ERROR_CODES
  already documents it as a horizon capability. **R1.**
* **It must name the missing capability.** SPEC_08 §6.1.4 separates "this
  operation is not authorised" from "your coverage is insufficient"; this is the
  former, and a diagnostic that does not say which word to pass leaves the
  operator guessing. The existing `runPure` refusal sets the pattern — it names
  `effect_override`. **R2.**

### 3.4 The gate precedes the effect

Returning `⊥` after having already opened the connection would satisfy R1 and
still give the address what it wanted: a packet and a query. This is SPEC_08
§6.1.2's "authority must be presented at the moment the privileged effect is
applied", one layer down. **R4 measures it in seconds** rather than inspecting,
because what must not happen is a syscall: floor 0.040 s, dial 5.05 s,
threshold 2 s.

### 3.5 Not in scope

* The affiliation trust root, automatic admission of discovered peers, a cap on
  the source set, `discovery.n` persistence. Those are the next two arcs.
* **This arc must not make the directory a fetch source in any way.** §4.2.6's
  MUST NOT stands, and **P2 pins it for the first time** — see §4.

---

## 4. What must NOT change

* **The directory is still not a fetch source (P2).** §4.2.6 has forbidden this
  since 2026-07-28 and **nothing has ever pinned it**: `discover_index`'s
  `p2_fetch_untouched` guards the *server* answering `#fetch`, not the client's
  source set. So the MUST NOT could have regressed silently at any point, and
  one of the things this arc buys is that it no longer can.
* **The local form needs no grant (P1).**
* **Serving `#fetch` is untouched (P4).** This arc changes who may *become* a
  source, not what a node answers when asked.
* **A failing source must not abort the scan** (SPEC_13 §6.1.1). Not a pin in
  this file — see §5, it has an owner.

---

## 5. Probes, and the two suites that must change

```
cargo test --test connect_consent_probe_test              # 1 control + 3 pins, green now
cargo test --test connect_consent_probe_test -- --ignored # 5 reds, all red now
```

**Probe modification rights belong to the acceptor**, with one enumerated
exception below. The delivery removes `#[ignore]` from this file and nothing
else in it.

### 5.1 SCHEDULED TO CHANGE — enumerated, not quiet

Gating the remote form turns two green suites red, because their programs call
`./connect` with `tcp://` and no grant. **The delivery is authorised to add
`--grant connect` to exactly these call sites and to change nothing else about
them:**

| suite | sites | form |
| :-- | :-- | :--- |
| `node_identity_probe_test.rs` | `fetch_from` (one helper, used at 3 call sites) | `tcp://127.0.0.1:{port}` |
| `peer_fetch_verification_probe_test.rs` | 9 `./connect` calls | `format!("tcp://127.0.0.1:{}", port)` |

Two suites are **not** affected and must not be touched:
`store_boundary_probe_test.rs` (2 calls) and
`universe_determinism_probe_test.rs` (1 call) both use local paths.

**These probes model an operator who consented, so passing the grant makes them
more faithful, not less.** But it is an edit to a probe, so it is enumerated
here rather than discovered in a diff, and acceptance will verify that nothing
else in those files moved.

**In particular**, `peer_fetch_verification::red_one_honest_peer_among_liars_is_found_every_time`
owns SPEC_13 §6.1.1's skip-and-continue property. Adding a grant to it must not
weaken what it asserts. That is why this file has no P3: proving §6.1.1 needs
two client-side sources, which needs the form this arc gates, so it could not be
green both before and after — a pin that needs the thing it pins is not a pin.

### 5.2 Calibration record

* **P2's first version was vacuous.** It used a local `./connect` to the
  holder's `.oo` as its control, which SPEC_08 §6.3 refuses — so the pin passed
  because *nothing* worked. It now controls with a raw wire `#fetch`, which the
  holder serves and which needs no source set on the asking side, and it also
  asserts the advertisement was accepted and the directory is readable. Third
  time this trap has been caught; the standing rule is that a probe asserting an
  absence must assert a presence in the same run.
* Reds at baseline: R1/R2 return `#true  ;; %effect: #io`; R3/R5 hit
  `unknown grant SPEC 'connect'`; R4 reports **5.07 s** against a 2 s threshold.
* Pins green across four consecutive runs.

---

## 6. Acceptance measurements (acceptor's, not probes)

1. **Diff purity** — `#[ignore]` removals in this file; in the two enumerated
   suites, only added `--grant connect`; nothing else anywhere. No `git add -A`.
2. **Independent re-run** of the workspace, plus conformance and genesis.
3. **The breaking surface, stated as a number**: how many programs in `tests/`,
   `examples/` and the conformance corpus call `./connect` with a remote
   address. The corpus had zero at baseline; confirm it still does, because a
   conformance vector that needs a CLI flag would be a spec change smuggled in.
4. **The refusal costs nothing**: a refused `./connect` followed by a fetch must
   stay near the 0.040 s floor, not merely under R4's 2 s threshold.
5. **`--privileged` still implies it** or explicitly does not — whichever the
   delivery chooses, it must be stated, because `--privileged` is documented as
   the full §6 grant and a new capability silently outside it would make that
   sentence false.

---

## 7. Ledger — known and deliberately not fixed here

* The peer directory has **no size cap and no eviction**: `record_peer_advert`
  inserts by `node_id`, so it grows with every distinct identity that ever
  advertised, bounded only by SPEC_15 §7.1's minting price. This is why option
  (ii) of the deferred ruling — admit the whole directory behind one flag — was
  rejected: 150 entries were measured at 172,091 B, and 150 silent members at
  5 s each is 12.5 minutes per fetch with no early exit permitted.
* `random_below` returns 0 if `SystemRandom::fill` fails, silently degrading
  `#discover` sampling to a fixed prefix (REAL_02 §4.3.5.1 forbids it; the
  engine is still silent).
* `#success` with no `%result` is still recorded as an integrity incident.
* `advert_persistence`'s object-count comparison has no non-empty baseline
  guard; `to_nlang` prints unforced Thunks as `Debug`; `reader.read_line` is
  unbounded; `free_port()` is TOCTOU; `routing_id_from_digest` zero-pads.

---

## 8. Delivery record (delivery side)

### Built

- **`Privilege.connect`** on the capability lattice; `--grant connect` on the
  CLI; **`--privileged` includes `connect`** (full §6 grant remains complete).
- **`disc.connect` with `tcp://`**: requires `ctx.privilege.connect` **before**
  any peer-table write. Refusal:
  `⊥ #privileged_required` with message naming `connect`.
- **Local form** (filesystem path): still ungated (P1).
- **Probe suites (authorized + one more of the same class)**:
  | suite | edit |
  | --- | --- |
  | `node_identity` | `fetch_from` → `--grant connect` |
  | `peer_fetch_verification` | `run_observe` → `--grant connect` |
  | `oodp_packet_format` | `fetch_from` → same (order undercounted; same surface) |
- **`wire_says_why`**: already uses `--privileged`; no probe edit.
- Directory still not a fetch source (P2). Spec / CHANGELOG not edited.

### Breaking surface

- Language: remote `./connect` without grant → `#privileged_required`.
- Conformance corpus: **0** remote `./connect` vectors (confirmed).
- `tests/pending/federation_test_tcp.n` still has one `tcp://` connect (pending,
  not conformance).

### Acceptance

1. Diff purity: only `#[ignore]` removals in this probe file; grant flags only
   in the three helpers above.
2. Workspace **1701/0/3** · conf **143/143** · genesis **11/11**.
3. Refused connect timing: **~0.03 s** wall (floor; no dial).
4. `--privileged` **does** imply `connect`.

### Numbers

| Suite | Result |
| --- | --- |
| connect_consent | **9/9** |
| node_identity | **13/13** |
| peer_fetch_verification | **12/12** |
| oodp_packet_format | **13/13** |
| wire_says_why | **16/16** |
| workspace | **1701 / 0 / 3** |
| conf | **143/143** |
| genesis | **11/11** |

### Left

Directory size cap / discovery admission (next arcs). Ledger §7 unchanged.
