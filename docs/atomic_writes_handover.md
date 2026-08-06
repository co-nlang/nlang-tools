# Durable writes must not tear — work order

**Opened** 2026-08-06. **Baseline** `dev 39249d5` (engine v0.10.0, tag on
`top b094fdd`; spec `v0.10.0-draft.1`).
**Probes** `crates/oo/tests/atomic_write_probe_test.rs` — acceptor-owned,
**to be written and calibrated before this order is handed off** (see §5.4).
Workspace at baseline: **1752 passed, 0 failed, 3 ignored** (180 blocks);
the 3 ignored are standing, none belong to this arc.

---

## 1. The defect

**Every durable write in the engine is a non-atomic in-place rewrite.**

`std::fs::write` opens with `O_TRUNC` and then writes. Between those two
steps the file on disk is **shorter than both its old and its new content**,
so a concurrent reader sees a truncated file.

**〔量〕Measured tear rate.** A 400-field universe (`.oo/staged` = 94,413
bytes), 60 `oo evolve` cycles, a reader loop parsing the file continuously:

```
讀取 63,769 次 · 解析失敗 12 次 · 失敗率 0.02%
```

Per *read* that is tiny; per *write* it is **0.2 failures**, i.e. a reader
running across 60 writes expects ~12 failures and `P(zero) ≈ 6e-6`.
**The window is real and reachable, not theoretical.**

**〔讀〕Where.** `crates/interpreter/src/storage.rs` has **zero** occurrences
of `rename` or `create_new`. The writes:

| 路徑 | 出處 |
| :-- | :-- |
| `.oo/staged` | `universe.rs:500` `save_staged` |
| `.oo/pin_pending` · `.oo/effect_pending` | `universe.rs:511` / `:521` |
| `.oo/abandoned` | `universe.rs:633` |
| `.oo/HEAD` | `storage.rs:243` `set_head` |
| `.oo/objects/sha256/**` | `storage.rs:263` `write_object` |

**And it already has a user-visible symptom.** The decode failure message
says *"object present for … but cannot be decoded (**integrity unknown**)"*
— **"integrity unknown" is exactly what a partial write looks like.**
The engine has the error path; it does not know this is one of its causes.

### 1.1 The signature is deterministic

**〔量〕`.oo/staged` keeps the same inode across writes:**

```
寫入 1 → inode 304362      寫入 4 → inode 304362
寫入 2 → inode 304362      寫入 5 → inode 304362
寫入 3 → inode 304362
```

In-place rewrite reuses the inode. **A temp-file-plus-rename write installs a
new inode every time.** That gives this arc a **race-free red** (§5).

### 1.2 A TOCTOU the fix removes without aiming at it

```rust
fn write_object(&self, hash: &ContentHash, content: String) -> Result<()> {
    let path = self.hash_to_path(hash);
    if !path.exists() {                 // ← check
        …
        fs::write(&path, content)?;     // ← use
    }
```

Two writers can both see `!exists` and both write. Content-addressing makes
that benign *for the content* (both write the same bytes), but the check is
not atomic. **`rename` is atomically idempotent, so the correct form of the
fix deletes this race as a side effect.** Worth stating so nobody later
"optimises" the check back in.

---

## 2. Why this one, and why now

* **It is a data-corruption class, not a design disagreement.** Everything
  else in `meta/oo/`'s queue needs a ruling first; this needs none.
* **It does not decide the concurrency model.** temp+rename fixes **torn
  reads**. It does **not** fix **lost updates** — see §3.3. Deliberately.
* **Today's exposure is low** because people run one `oo` at a time. That is
  a usage habit, not a mechanism, and the control plane's whole purpose is to
  remove it (`meta/oo/control_plane.md` §0).

---

## 3. What is being asked for

### 3.1 Every durable write is atomic (MUST)

Write to a temporary file **in the same directory** as the target, flush and
`fsync` it, then `rename` it over the target. Same-directory matters:
`rename` is only atomic within one filesystem.

Covers all six paths in §1. **The probe pins the property, not the spelling**
— a helper, a crate, or hand-rolled all satisfy it.

### 3.2 Nothing is left behind (MUST)

A failed or interrupted write must not leave a temp file that a later reader
or a directory scan can mistake for content.

**〔讀〕This has a specific, measured consequence.** `local_gc_probe_test.rs`'s
`store_map` walks `objects/sha256/<2hex>/` and keys **every file it finds** as
`<2hex><rest>`:

```rust
for b in fs::read_dir(a.path()).unwrap().flatten() {
    let rest = b.file_name().to_string_lossy().to_string();
    out.insert(format!("{pre}{rest}"), b.metadata().unwrap().len());
}
```

⟹ **A leftover temp inside a shard directory becomes a phantom object**, and
that suite's object counts and byte totals move. This is a failure mode
**this arc could introduce**, not a pre-existing one. P1 guards it, and
`local_gc` is on the independent re-run list (§6.2) for this reason
specifically.

### 3.3 Lost updates are explicitly **not** fixed here

Two writers still race:

```
A 讀 → B 讀 → A rename → B rename   ⟹ A 的欄位不見(但不再有半截檔)
```

**〔量〕baseline for the record:** 40 concurrent `oo evolve`, each adding a
distinct field → expected 41 fields, **got 2, lost 39, zero errors.**

**That number is recorded here and is NOT a probe.** A red that this delivery
cannot turn green is a countdown timer, and there are already three standing
`#[ignore]`d probes queued for triage. The lost-update gate belongs to the
CAS-and-retry arc (`meta/oo/control_plane.md` §1.4, W12).

### 3.4 Not in scope

* any locking, `flock`, or single-writer enforcement;
* optimistic concurrency, expected-value comparison, retry loops;
* the savepoint object (`meta/oo/commit.md` arc A) — this arc must not
  restructure what is written, only how;
* control-plane endpoints, service-face tokens;
* the `ScratchDir` / `tempfile` duplication (§7).

---

## 4. Satisfiability check

Done before writing this order, per the standing rule:

* **Is the red reachable?** Yes — §1.1's inode signature is deterministic and
  present today.
* **Can the delivery turn it green?** Yes — rename installs a new inode by
  construction.
* **Is anything asked for that cannot exist?** No. §3.3 is the one thing that
  *could* have been asked for and cannot be delivered here, so it is
  explicitly excluded rather than left implicit.

---

## 5. Probes

**Probe modification rights belong to the acceptor.** The delivery removes
`#[ignore]` and nothing else.

| | what it holds |
| :-- | :--- |
| **C1** | the fixture really rewrites `staged` — the inode is read, the write happens, and the file's *content* changes between reads (a走訪 that silently fails must not let every red pass by "nothing moved") |
| **R1** | `.oo/staged` gets a **new inode on every write** |
| **R2** | `.oo/HEAD` gets a new inode on every write |
| **R3** | `pin_pending` / `effect_pending` / `abandoned` likewise |
| P1 | after any write, **no stray temp file** remains anywhere under `.oo/` |
| P2 | `.oo/format` is not bumped — this changes how bytes land, not what they mean |
| P3 | a workspace written by v0.10.0 still loads and still commits |
| P4 | object content still round-trips: commit → `oo log` → `oo inspect` |

### 5.1 R1–R3 are deterministic, not draws

The inode signature does not depend on timing. Every run is red at baseline
and green after the change. **Do not add a racing reader as a gate** — see
§5.3 for why the race measurement lives on the acceptor's side instead.

### 5.2 `#[cfg(unix)]`

Inodes are a POSIX concept. R1–R3 must be Unix-gated, and the work order
records that **the property is unpinned on Windows** rather than pretending
otherwise. (`resolve_node_home` already has a `USERPROFILE` branch, so
Windows is a supported target on paper.)

### 5.3 Objects cannot be pinned this way — stated, not hidden

`write_object` short-circuits on `path.exists()`, so writing the same object
twice does nothing and the inode never moves. **There is no race-free
signature for the object path.**

⟹ `.oo/objects/**` **is in scope for the fix** (§3.1) but its atomicity is
verified by the acceptor's race measurement (§6.4), not by a probe.
P1 and P4 are what guard it inside the suite.

### 5.4 Calibration is still owed

This order is written **before** the probe file exists, because the recon had
to settle the probe *design* first (a racing reader was the obvious choice and
turned out to be the wrong one). **The probe file must be written, calibrated
and committed before hand-off**: C1 and P1–P4 green at baseline, R1–R3 red at
baseline **and red for the stated reason** (inode stable, not "file missing").

---

## 6. Acceptance measurements (acceptor's, not probes)

1. **Diff purity** — no probe edits beyond removing `#[ignore]`; no `git add -A`.
2. **Independent re-run** — workspace, conformance, genesis, plus
   `local_gc`, `advert_persistence`, `history_ops`, `store_boundary`
   (everything that reads or writes `.oo/` directly).
3. **Repeat-run stability**, several times. This arc changes write ordering;
   anything that was accidentally relying on in-place rewrite will be flaky
   rather than red.
4. **The race measurement, both sides — and it is what closes R1's gap.**
   Re-run §1's reader loop (94 KB staged, 60 writes, ~25 s) before and after.
   **Before: ~12 failures. After: 0 required, observed over at least three
   runs** — a single clean run is not evidence against a 0.02%-per-read event.

   > **A new inode is necessary, not sufficient.** Deleting the file and
   > recreating it also moves the inode and is just as unsafe — it swaps a
   > truncated-file window for an **absent-file** window, and R1–R3 cannot
   > tell the two apart. **So this measurement MUST count missing-file errors
   > as well as parse errors**, or delete-and-recreate reads as a clean pass.
5. **Object tear, same shape** — a commit large enough to have a measurable
   write window, with a reader polling the object path. Report the before
   number even if it is small; if it cannot be driven above zero at baseline,
   **say so** rather than reporting a green that never had a red.
6. **Cross-version against v0.10.0, both directions.** No format change is
   intended, so this is expected to be uneventful — measured, not assumed.
7. **Stray-temp sweep** — after the full workspace run, scan for temp-shaped
   leftovers under every `.oo/` the suite created. **Untruncated.**

---

## 7. Ledger — known and deliberately not fixed here

* **Lost updates under concurrent `evolve`** — §3.3, measured 40 → 2.
* **`ScratchDir` duplicates `tempfile`** (`scratch.rs`): the hand-rolled
  guard does prefix sanitisation + `pid`+`seq` naming + `remove_dir_all`
  then `create_dir_all` (not atomic, predictable name), while
  `ephemeral_store_root()` in the same file is a thin wrapper over
  `tempfile::Builder` — and `tempfile` is now a first-class dependency.
  Test-only, so low severity. **Folded into the engine-test-triage item.**
* **`ScratchDir::keep()` has zero callers** — an unused public escape hatch.
* **The leftover-directory property has no guard and is structurally
  unpinnable** as a unit test (it needs the whole suite's cumulative effect,
  and cargo guarantees no ordering). It belongs on the candidate-retest
  checklist, not in a probe.
* `#success` with no `%result` is still recorded as an integrity incident.
* Unknown advert fields are relayed and persisted verbatim (64 KiB cap).
* `to_nlang` prints unforced Thunks as Rust `Debug`; `reader.read_line` is
  unbounded; `free_port()` is TOCTOU; `routing_id_from_digest` zero-pads.

---

## 8. Delivery record

**Delivered** against open `402da32` / baseline `39249d5` (v0.10.0).

### What landed

* `storage::atomic_write` — same-directory temp (`.partial-*`), `write` +
  `fsync`, then `rename` via `tempfile::persist`. Failure drops the temp.
* Wired for: `.oo/staged`, `pin_pending`, `effect_pending`, `abandoned`,
  `HEAD`, CAS objects (first install), `architects.json`, `.oo/format` mint.
* Probe: three `#[ignore]` attributes removed only.
* **Not** fixed: concurrent lost updates (40→2); no flock / OCC.

### Numbers

| Measurement | Result |
| --- | --- |
| `atomic_write_probe_test` | **8/8** (×3 stable) |
| `local_gc` / `advert_persistence` / `history_ops` / `store_boundary` | all green |
| full workspace | **1760 passed / 0 failed / 3 ignored**, 181 blocks |
| conformance | **143/143** |
| genesis | **11/11** |
| staged race (≈400-field seed, 60 evolves, continuous JSON reader) | **3/3 runs, 0 parse fail, 0 missing** (reads 7780 / 8421 / 9324) |
| `cargo fmt --all -- --check` | pass |

Opening with calibrated probe was effectively 1757/0/6; three reds move into
the pass column (1757 + 3 = 1760; 6 − 3 = 3).
