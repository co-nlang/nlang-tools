# Work order — the engine stops forging the governance root

Arc opened 2026-07-27, after v0.2.44.
Acceptor: project brain. Implementer: model #3.
Expected classification: **破壞性(Layer 1)** — newly initialized universes address differently.

---

## 0. The headline, measured on v0.2.44

Six fresh repositories, the same one-line source, six processes:

```
=== root CAID ===                    === plain value CAID (control) ===
be91ecfe…  7b48ed60…  611724cb…      hash:sha256:v2:…:681781ef…   ×6
5c39042f…  57564041…  51a0fe12…      (one value, six times)
```

**Six different universes from one source.** Content addressing works for values
and does not work for the universe.

Enumerating every leaf of two roots from separate processes — **2,588 leaf paths,
exactly one differs**:

```
/Combo/system/Official/Combo/data/architects/Atom[0]/Str
      0314c0f8b917ddbca7eed9231a50657860 …  201ff3cc…  5e8b16e7…
      99a031cd…  dc6dc995…  eebbb4b8…
```

That is `hex(Identity::new_random().public_key)` — a fresh Ed25519 key minted at
every `Ouroboros::init` (`lib.rs:372/396`) and written into the universe by
`root_with_system()` (`lib.rs:1065–1067`).

**The hash is correct. The content really is different. What is wrong is that a
random number is part of the universe.**

## 1. It is worse than a random number — read the spec

* **ORDER_01 §117**: 「**信任根**:`~%Official.architects` 定義為 `@{ [@~%Governance.@Voter] }`(Voter 之**集合**)。」
* **ORDER_01 §88**: 「`~%Official` 內容更新必須通過 **RFC 流程**。」
* **SPEC_10 §93**: 「`signer` **必須存在於**當前紀元的 `~%Official.architects` 集合中。」

The engine mints a **string**, not a set; its content is a **local random
self-appointment**, not a governance trust root; and it appears at engine start,
not through an RFC. `~%Official` is **not** in SPEC_13 §3.1's genesis seed list
(`~%List`, `~%Math`, `~%Logic`, `~%Engine`, `~%Discovery`), and the
`~%Official.blacklist` that ORDER_01 §46/§91 refers to does not exist.

The engine is **minting a local forgery of a global governance object and
appointing itself its sole member.** The CAID nondeterminism is the symptom.

And the appointment is load-bearing: `Ouroboros::init` also inserts the same key
into `architect_registry` (`lib.rs:375/399`), so `bootstrap_exempt` is false and
`oo refine` demands `--sign` — which always succeeds, because the signer is the
sole architect. **An authority check that passes because the checker appointed
itself is a lying audit surface** (v0.2.41 `#squash` precedent).

## 2. Rulings

### A — `~%Official.architects` leaves the universe root

The whitelist is a claim about *this workspace's trust configuration*, not
semantic content of the universe. Discussion 025's two strata put it in the
**assertion layer** — where `.oo/architects.json` already lives (SPEC_08 §6.3) —
not the self-authenticating object layer.

Root becomes universe-wide deterministic, satisfying **SPEC_13 §4.1.2 義務 #1**
(「相同的內容在相同的格式版本下,產生的 CAID **必須**全宇宙唯一且決定」).

Removal is **functionally inert inside the engine**: `verify_refine_authority`
takes `architect_registry`, never the root field (`authority.rs:30–34`). The
field is display-only. Measured: after removal `~%Official.architects` observes
as `⊥ #missing_key` (closed cocoon) — an honest "this engine does not hold the
global governance object."

### B — the engine stops appointing itself

Drop the self-insert at `lib.rs:375/399`. A fresh repository then has an **empty**
registry, `bootstrap_exempt` is true, and `oo refine` succeeds unsigned.

This **is** a relaxation: previously refused, now accepted. It is the right one,
because the previous refusal was theatre — the only thing it required was a
signature the same process could always produce. Replacing "always passes because
I signed it myself" with "there is no authority configured here" is the honest
form.

Note for the record: SPEC_10 §93's own exemption is keyed on **Epoch < 0**, not
on an empty local registry. The engine has no epoch model. That gap is ledgered,
not closed here.

### C — an unverified refine must be recorded as unverified *(acceptor's corollary)*

Ruling B creates a state the engine must not be silent about. A reader of history
must be able to tell a refine whose authority was verified from one where no
authority existed to verify against. Otherwise this arc removes one lying audit
surface and leaves a silent one — see v0.2.41 (「無法憑檢視查驗的審計面不成其為
審計面」) and REAL_03 §6.6 條款四.

**Where it goes is fixed by measurement, not preference:**

`Commit::content_hash` (`value.rs:2125`) hashes selected fields — parent digest,
root digest, kind tag, `RefineInfo`'s **source/target digests only**, and
`format!("{:?}", self.meta)`.

* Adding a field to **`RefineInfo`** does **not** move any commit CAID. ← use this
* Adding a field to **`CommitMeta`** **does**, because the Debug string carries it.

`CommitMeta` already survives this only because of a **hand-written `Debug`**
(`value.rs:1429`) that omits `abandoned` when `None` — that is what keeps
pre-v0.2.41 commits bit-stable, verified here by reading a v0.2.40-built
repository with v0.2.44 (log verifies, further commits succeed). **Do not touch
`CommitMeta`.**

## 3. Scope

**IN**

1. `root_with_system()` no longer mints `architects` into `~%Official`.
2. `Ouroboros::init` / `new_in_memory` no longer self-insert into
   `architect_registry`. `load_architects` (assertion layer) is unchanged.
3. `RefineInfo` records whether authority was verified or the refine proceeded
   unverified; `oo log` / the refine report shows it, distinguishably.
4. The probes in §4.

**OUT — ledgered, do not start**

* **Migrating existing universes.** An old repo's root came from the store and
  still contains `architects`; commits made on top of it carry it forward, so
  existing universes stay nondeterministic. In scope here: **newly initialized**
  universes. Migration is its own arc.
* **Identity persistence — a SEQUEL, not a rival.** Read this carefully; an
  earlier draft of this order was misleading. Persistence *instead of* ruling A
  would make the root stable per repository and still different between
  repositories, so it does not satisfy SPEC_13 §4.1.2. But **once A lands the
  identity is not in the root at all**, and persistence stops having any
  bearing on addressing. A is persistence's prerequisite, not its competitor.

* **Who fills the empty whitelist — the operator, and the spec already says
  where.** REAL_01 §7.2 (`[Core Requirement]`) specifies that the engine loads
  a public-key whitelist from **`~/.oo/authorized_keys`** — the *home*
  directory, i.e. the operator — issued out of band by an ADMIN token or an
  HSM, with a lifecycle and a revocation list. The engine reads none of that.
  It reads the *workspace* file `.oo/architects.json` (project-level trust) and
  otherwise invents a per-process self-appointment.

  The missing party in this whole picture is the human, and the spec put them
  in `~`. So an empty registry after ruling B does not mean "no architect
  exists"; it means **nobody has declared one yet**, which is the honest state
  of a cold start.

  The shape is already settled precedent: **SPEC_08 §6.1.2 ruling P1** — a
  privilege cannot be self-granted from inside a program; it arrives through a
  trusted out-of-band channel. Architecthood is that ruling one layer down, and
  the engine's self-appointment is exactly the self-grant P1 forbids. Whatever
  the sequel arc builds, it must arrive through that channel and **must not**
  reintroduce a language-surface writer (`~%Official./add_architect` was retired
  for this reason in v0.2.42).

  Ordering, so the sequel is not attempted piecemeal:
  `A (identity out of the root)` → `persistence (a stable key in the assertion
  layer)` → `operator declaration (~/.oo/authorized_keys or a CLI path)` → an
  authority check that can actually fail.

* **Cold start has two spec-level answers already, and the engine uses
  neither**: constitutionally, ORDER_00's hardcoded genesis-refine list with
  SPEC_10 §93's `Epoch < 0` exemption; locally, REAL_01 §7.2's operator
  whitelist. The engine has no epoch model and no `~/.oo` reader. Ledgered.
* Epoch model; `~%Official.blacklist`; turning `architects` into a real Voter
  set; the global `~%Official` as a discovered object.
* Storage weight (indentation 3.3×, 77 KB genesis duplication, ~2 KB per
  recursive type). All measured, all separate.

## 4. Probes — pre-committed, do not modify

`crates/oo/tests/universe_determinism_probe_test.rs`. The implementer removes
`#[ignore]` and nothing else; if a probe looks wrong, report and stop.

| gate | asserts | baseline |
| :--- | :--- | :--- |
| R1 | one source, N processes → exactly one root CAID | red |
| R2 | no leaf path of the root differs across processes | red |
| R3 | `~%Official.architects` is not minted into the universe | red |
| R4 | a fresh repository has no architect; refine needs no signature | red |
| R5 | verified and unverified refines are distinguishable | red |
| R6 | engine B's root CAID resolves against engine A's store | red |
| R7 | `CommitMeta`'s Debug omission keeps old commit digests stable | green — pins an invariant nothing pinned |
| P1–P5 | value CAIDs, `/sign_refine`, provisioned `--sign`, cross-version read, genesis seeds | green, must stay green |

**Anti-vacuity is the theme of this file.** Every comparison gate first asserts
that both sides are well-formed and non-empty — a 64-hex digest, a leaf count in
the thousands — because this arc exists partly because the acceptor's own
v0.2.44 stability script compared `None` to `None` across 143 vectors and
reported a perfect score. A comparison that cannot fail has not been made.

## 5. Acceptance

1. Diff purity — nothing outside §3.
2. Workspace / conformance (143) / genesis (11) / this probe file.
3. **Address movement, stated exactly.** Root CAIDs of newly initialized
   universes **will** move — that is the point, and it is why this is 破壊性.
   Everything else must not: value CAIDs, genesis seed CAIDs, and the commit
   digests of repositories built by earlier engines.
4. Adversarial pass, including a re-run of the reds against a v0.2.44 worktree.

Classification and CHANGELOG are the acceptor's step. **Do not touch `nlang-spec`.**

---

## 6. Delivery record (delivery side)

- **Tip**: `db01d24`.
- **A** `root_with_system`: no longer mints `architects` into `~%Official`
  (module keeps `/sign_refine` only).
- **B** `Ouroboros::init` / `new_in_memory`: empty registry by default; load only
  from `.oo/architects.json`. No self-insert of the process key.
- **C** `RefineInfo.authority_status`: `"verified"` | `"unverified"` (serde
  optional; not hashed beyond source/target). Set from `Valid` / `Exempt`.
  Displayed by `oo log` and the refine report.
- **Authority**: empty-registry membership skip only under `bootstrap_exempt`
  (crypto still checked); non-empty whitelist without the signer still refuses.
- **Probe**: only 7 `#[ignore]` removed (R1–R6 + R5b).
- **Gates**: universe_determinism probe **12/12**; workspace **1487/0/3**;
  conf **143/143**; genesis **11/11**; release clean of new warnings.
- **Address movement (expected)**: newly initialized root CAIDs change (破壊性).
  Value CAIDs / genesis seeds / `CommitMeta` Debug omission (R7) unchanged.
- **Not touched**: nlang-spec, identity persistence, epoch model, migration of
  old repos that still carry `architects` in stored roots.
