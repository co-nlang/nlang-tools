# Work order — a privilege that leaves no trace

Arc opened 2026-07-27, after v0.2.46.
Acceptor: project brain. Implementer: model #3.
Expected classification: **增量** — but see §5.5. The last arc's work order got
this wrong by asking the wrong question, so it is decided at acceptance from a
root-CAID measurement, not assumed here.

This arc is **not** the queue item as it was written. The queue said *operator
declaration — REAL_01 §7.2 privilege tokens, lifecycle, CRL*. Measurement says
that lock has no door (§2). What it found instead is a normative clause the
engine has never satisfied.

---

## 0. The headline, measured on v0.2.46

```
oo evolve s.n --grant effect_override:io     ← capability presented here
oo commit -m "no capability at this stage"   ← and never again
```

with `s.n` = `v: (~%Effect./runPure (~%Time.now _))`:

```
committed universe:  v: 1785130396317
commit object:       kind = Standard
                     meta = {author, timestamp, message}
```

A nondeterministic IO observation, laundered into an ordinary integer, sitting
in history with nothing to say it was ever privileged.

Control — the same source with no grant at evolve:

```
committed universe:  v: _|_ (%cause: #privileged_required)
```

So the gate is real, and the difference between those two universes is exactly
one discharge.

And the laundering is *thorough*: `identify_and_store` of a discharged `42` and
of a hand-typed `42` return the **same CAID**, `c3b42fab…`. That is **correct** —
content addressing means the value is the value — and it is precisely why
SPEC_08 §6.2 puts the audit on the **Commit** and forbids it in the value.

### The spec already said all of this

* **§6.2 表**: `#effect_override` → 審計標籤 **`#privileged_effect`**.
* **§6.2 透明度**: 「由特權操作產生的 Commit **必須**在元資訊中標註其干預性質」.
* **§6.2 授權時點** (2026-07-26): 「若一個特權操作跨越多個階段(如「演化期標記、提交期套用」),則**每一個實際施加效果的階段都必須各自出示能力**」.

The discharge is applied at evolve and **fixed into history at commit**. Commit
is a stage that applies the effect, and it presents nothing. This is the v0.2.40
`#pin` finding one operation across — and `#pin`'s shipped design is the answer.

## 1. The surface the new marker would be built on is forgeable

Measured on v0.2.46, an ordinary commit with no capability of any kind:

```
$ oo commit -m "pin"
$ oo log
commit hash:sha256:v1:f67e68bb…
    pin
    Date: …
```

which is, to the byte, what a genuine `#pin` commit with no message renders.

v0.2.41 ruled 「無法憑檢視查驗的審計面不成其為審計面」 and repaired the
auto-generated squash **message**; the marker **format** was left, and
`main.rs:369` says so in a comment. Adding a third marker to this surface would
be building on sand, so the format is in scope.

## 2. Why the queue item itself was not built

REAL_01 §7.1/§7.2 specify a three-part token, issuance by an ADMIN token or HSM,
≤24h validity, and a CRL reloaded every 5 minutes. All `[Core Requirement]`, all
absent. Measured:

* The **only** path to a privileged operation is the CLI, run by whoever owns the workspace.
* `oo serve` is **read-only** — one bare CAID in, one stored value out. REAL_01 §2's JSON-RPC API is entirely unimplemented, so §2.6's auth has no surface either.
* `oo repl` accepts no grants at all.
* SPEC_08 §6.3.3 已明文 declares that out-of-band tampering gets **no guarantee** — and the local operator is exactly an out-of-band-capable party (can edit `.oo/`, can replace the binary).

So a token system would guard a door that does not exist, against the one party
it cannot restrain. Worse, **§7.2's ≤24h bearer token is weaker than what ships
today**: a flag typed per invocation cannot leak, because it does not persist.

Meanwhile the mechanism the engine actually uses — per-invocation `--grant`,
SPEC_08 §6.1.4's capability lattice — **appears nowhere in REAL_01 §7**. The spec
describes an unimplemented worse mechanism and omits the implemented better one.

**Ruling Q1**: tokens are scoped to the **service surface** (REAL_01 §1.2/§2),
marked as not applying before that surface exists; the per-invocation grant is
written into §7 as the normative mechanism for the local surface. This is the
second *measurement vetoes the arc* precedent (`#ext:` was the first).

**Ruling Q2**: `#effect_override` follows §6.2 literally — commit **re-presents**
the capability and the Commit is marked `#privileged_effect`.

## 3. What is deliberately NOT reopened

A rollback with no subsequent commit leaves no record anywhere (measured: HEAD
moved, `.oo/` mentions nothing). That is **not** a gap. §6.2 R1 says the record
is written by the next commit because 「該時點即分歧真正進入鏈之時」 — nothing
entered the chain, so there is nothing to record. Measured, read, left alone.

## 4. Scope

### D1 — commit re-presents the capability

Mirror `#pin`'s shipped two-step, which is the design §6.2's 授權時點 clause was
written from:

* **evolve**, when a discharge **actually occurs**, records the fact in the assertion layer beside staged — the same home and shape as `.oo/pin_pending` (`universe.rs:445–464`).
* **commit**, when that record is present, **requires** `--grant effect_override:<tags>` and refuses with `#privileged_required` otherwise.
* The record is **intent, not authorization** (§6.2 意圖≠授權). Its presence means *you must ask*; it never means *you may*. Since v0.2.42 the language layer cannot write `.oo/` at all, and out-of-band writers are outside §6.3.3's scope — so this is exactly as strong as `pin_pending`, no more and no less. Say that in the code comment rather than implying more.

### D2 — the marker

* Tag: **`#privileged_effect`**, the name §6.2 already gives it.
* Home: **`CommitMeta`**, a new `Option` field. **Not** `CommitKind` — a commit can be both a pin and a discharge, and kinds are exclusive.
* **`Commit::content_hash` hashes `format!("{:?}", self.meta)`.** `CommitMeta` stays bit-stable across versions only because its **hand-written `Debug`** (`value.rs:1429`) omits `abandoned` when `None`. The new field **must** follow that pattern or every existing commit CAID moves. P1 pins the JSON side; the `Debug` side is yours to keep.
* The marker is a statement about the **commit**, not about every coordinate in it: it means *this commit fixed privileged-discharged content into history*. Do not try to attribute it per-path; §6.2 作用範圍最小化 governs where the *effect* may be applied, which is already the grant's business.
* **It must reflect the discharge, not the grant** (P7): `--grant effect_override:io` over content that discharged nothing must leave an unmarked commit. Key it on an actual discharge event during evaluation — `ctx.had_nondistrib_event` is the existing precedent for that shape.

### D3 — `oo log`, and unforgeable markers

* The marker must appear in `oo log` (R3). A verdict that exists only inside a stored object is not an audit surface — v0.2.44 spent an arc on that exact point.
* **Audit markers must not be reproducible by a commit message** (R4). Which side changes — a marker prefix, message quoting, anything — is your call; the gate reads whatever the engine prints for a *real* pin, writes exactly that as a message, and requires the two to differ. Fix it for `pin`, `squash` and `abandoned` as well as the new one; they share the renderer and the same defect.

### D4 — retire `--grant commit`

`privilege.commit` has **zero** consumer sites. SPEC_08 §6.2 retired `#commit` on
2026-07-26 (「量測顯示該描述無對應閘位」); the CLI spelling outlived the concept
and gates nothing. Reject the spelling with a clear error naming the retirement.
Keep the `Privilege::commit` field only if something still needs it — measurement
says nothing does, and the struct comment calling `pin`/`rollback`/`squash`
"declared but inert slots" is stale since v0.2.40/41 and should be corrected too.

### Out of scope

* The service surface itself, and everything in REAL_01 §2.
* Token issuance, expiry, CRL — Q1 puts them behind that surface.
* `#rollback` with no subsequent commit (§3 above).
* `.oo/audit.log` (REAL_01 §7.3). It is the same question as tokens: a log inside `.oo/` is an assertion nobody can verify, and §6.3.3 already disclaims out-of-band tampering. Deferred with tokens, and named in the spec closure so it is not silently dropped.

## 5. Probes — pre-committed, do not modify

`crates/oo/tests/privileged_effect_audit_probe_test.rs`, calibrated at baseline:
**5 red (`#[ignore]`), 7 green.** Delivery removes **only** the `#[ignore]`
attributes. Every invocation pins `OO_IDENTITY` (v0.2.46), so the suite never
touches the developer's real `~/.oo/`.

| | gate | baseline failure |
| :-- | :--- | :--- |
| R1 | commit of discharged content needs the capability **(paired: granted commit must succeed, and the value must not be ⊥)** | `a discharge entered history with no capability presented at that stage` |
| R2 | the commit object is marked **(paired: an ordinary commit is not)** | `carries no marker: {"kind":"Standard",…}` |
| R3 | `oo log` shows the marker | `audit lines were ["authorised"]` |
| R4 | markers are not forgeable by a message | `left: ["pin"] right: ["pin"]` |
| R5 | `--grant commit` retired **(paired: pin/rollback/squash/effect_override still accepted)** | still accepted |

| | pin | must stay green |
| :-- | :--- | :--- |
| P1 | an ordinary commit's meta has exactly `author`/`message`/`timestamp` | commit CAID stability |
| P2 | golden value CAID unmoved | §6.2 幾何指紋 |
| P3 | `runPure` still gated **(paired both ways)** | the thing being measured |
| P4 | the `pin` marker survives the R4 format change | D3 blast radius |
| P5 | ordinary work needs no capability | the gate fires on discharge only |
| P6 | root CAID deterministic across 3 workspaces | v0.2.45 §4.1.2 #1 |
| P7 | a grant without a discharge marks nothing | D2's "fact not flag" |

**Anti-vacuity**: `repo_with_discharge()` refuses to build a fixture unless the
ungated form is proven to be refused first. The discharged value is an ordinary
integer, so "no marker" and "nothing to mark" are indistinguishable unless the
pair is measured together.

## 6. Acceptance

1. Diff purity — only files this order names.
2. Numbers: workspace, this probe 12/12, `pin_probe` / `history_ops` / `runpure_cli` / `selective_discharge` (they share the audit renderer and the capability lattice), conformance 143/143, genesis 11/11.
3. Adversarial, at minimum: a commit that is *both* a pin and a discharge; two discharges in one staged batch; a discharge staged by one process and committed by another; `--privileged` (full grant) as the authorising form; a repository built by v0.2.46 committed to by the new binary and re-read by v0.2.46.
4. A/B against v0.2.46 — the five reds must be red on the previous binary for the reasons recorded above.
5. **Classification decided by measurement**: does an ordinary commit's CAID move, and does the universe root move? The scan-is-clean argument does not settle this — that was the last arc's mistake.

## 7. Delivery record (delivery side)

- **Tip**: `c16079a`.
- **D1** `runPure` success → `Ouroboros::note_privileged_discharge`; evolve →
  `effect_pending` + `.oo/effect_pending` (intent only). Commit requires
  `effect_override` grant when pending; refuses with `#privileged_required`.
- **D2** `CommitMeta.privileged_effect: Option<bool>` (serde skip if None);
  custom `Debug` omits absent optionals (ordinary commit digests stable).
  Set only from `effect_pending`, never from grant alone.
- **D3** `oo log`: markers `pin` / `squash` / `privileged_effect` /
  `abandoned …`; messages always `message: …` (unforgeable by R4).
- **D4** `--grant commit` rejected with retirement error; `Privilege::all()`
  no longer sets `commit`; comment updated.
- **Probe**: only 5 `#[ignore]` removed.
- **Numbers**: privileged_effect_audit **12/12** · pin **15/15** · history_ops
  **15/15** · runpure_cli **6/6** · selective_discharge **15/15** ·
  workspace **1515/0/3** · conf **143/143** · genesis **11/11**.
- **Classification measurement**: ordinary commit meta still exactly
  `author`/`message`/`timestamp` (P1); root digests deterministic (P6);
  value CAIDs unmoved (P2). **Recommend 增量** — optional fields omitted from
  Debug/serde when None.
- **Not touched**: nlang-spec, tokens/CRL, rollback-without-commit.
