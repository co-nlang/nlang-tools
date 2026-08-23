# Work order — the store trust boundary

**Arc**: `.oo/` is the engine's, not the language's.
**Date**: 2026-07-26. **Baseline**: v0.2.41 (`top 0ab489e`).
**Probes (pre-committed, calibrated)**: `crates/oo/tests/store_boundary_probe_test.rs`
— 15 red (`#[ignore]`, each verified red *for the right reason*) + 5 pin (green now,
must stay green).

**Probe modification rights belong to the acceptor.** Your only permitted edit to
that file is removing `#[ignore]`. If a gate looks wrong, report it — do not
adjust it. (Two of the fifteen were vacuous on the first pass and were repaired
by the acceptor before this order was written; the note is in the file.)

---

## 1. What was measured

All of the following were run end-to-end against the v0.2.41 release binary, not
inferred from reading code.

### A4 — the sharpest one

```
$ oo rollback <c1>                  → Error: #privileged_required
$ oo run a4.n -o w                  → #true          (writes .oo/HEAD)
$ oo log                            → tip is now c1
$ ls .oo/abandoned                  → absent
```

`a4.n` is one line and needs no privilege:

```
w: ~%Io./write_file(".oo/HEAD", "hash:sha256:v1:<c1>")
```

The same effect as the capability-gated operation, through the back door, and it
leaves **less** audit trace than the legitimate path leaves. The capability
lattice built across v0.2.38 / v0.2.40 / v0.2.41 is decorative for as long as
this holds.

### A5 — a forged audit claim, sealed into a CAID

An unprivileged program wrote `.oo/abandoned`; the next ordinary commit consumed
it and recorded an abandonment of a commit that never existed. Permanent, because
commit metadata is hashed.

### A2 — the refine trust root

An unprivileged program wrote `.oo/architects.json`. `Ouroboros::open`
(`lib.rs:381`) merges it into `architect_registry`, which is the *only* gate in
`verify_refine_authority` (`authority.rs:54`) on who may sign a `#refine`.

### A3 — measured, and deliberately OUT of scope

The bytes of an object were edited in place, leaving the filename (= the CAID
digest) untouched. The engine loaded it unverified: the universe root became a
value that was never committed, and evolving the true `x: 1` then answered
`#conflict`. CAID is the identity of a value, but `read_object` returns whatever
sits at the path.

**This is the next arc, not this one** (user ruling). It carries its own decision
— verify the digest only, or the `lattice_sketch` too; REAL_03 §9.2 leaves a
back-compat door open. Do not touch `storage.rs` read paths here.

### A1 — a dead morphism

`~%Official./add_architect` answers `_|_ #conflict` to every input. The builtin
does `oo.force(arg, ctx).to_string_plain()` on the *whole* argument; apply hands
it `{0: str}`, and `to_string_plain()` of a Combo is the literal `"{...}"`
(length 5 ≠ 64). Identical class to v0.2.39's `check_oml`: the registry-level
contract and the apply-level contract disagree, with no test on the seam.

Fail-safe, but it means the morphism has no body — and `save_architects` is its
only caller, so **the forgeable file is the only live path into the trust root.**

---

## 2. Rulings

**R-A (user).** Sandbox model: the language layer cannot touch the store. State
the scope honestly in the spec — this closes the language-level attack; it does
not defend against someone with shell access, who can equally replace the `oo`
binary. REAL_01 §7.2's out-of-store keys / HSM stay ledgered, not built.

**R-B (user).** Authorization layer this arc (A1 / A2 / A4 / A5). CAS read
integrity is the next arc.

**R-C (acceptor).** The rule is a **reserved path component**, not a string
prefix and not a `base_dir` comparison:

> A path handed to a filesystem-touching builtin is refused iff, after resolving
> `.`, `..` and symlinks, **any component equals exactly `.oo`**.

Rationale: one sentence to specify; no `base_dir` dependency (so it holds for
`oo eval` outside a workspace); no TOCTOU window; covers other workspaces' stores
for free; and component-exact matching makes the `.oo_peer_a` prefix trap
structurally impossible. Cost: a user directory literally named `.oo` becomes
unreachable — the same deal `.git` gets, and REAL_01 §4 already reserves the name.

**R-D (acceptor).** The boundary is **unconditional**. `--privileged` does not
unlock it, nor does any `--grant`. The capability lattice governs §6.2 lattice
and history operations; it does not govern the store's physical bytes. If
privilege opened this door, a privileged program could forge the very audit
records §6.2 requires, and the guarantee would close on itself.

**R-E (acceptor).** Reads are blocked as well as writes. The ruling says the
language cannot *touch* the store, and a uniform boundary is what the spec can
state in one line. A read carve-out is cheap to add later and expensive to reason
about now.

R-C, R-D and R-E are acceptor calls, stated explicitly so the user can veto any
of them before you start. Confirm they still stand if this order has been sitting.

---

## 3. What to build

### 3.1 The boundary check

One helper, used by every filesystem-touching builtin. Suggested home:
`crates/interpreter/src/builtins/mod.rs` or a small `fs_guard.rs`.

```
fn crosses_store_boundary(raw: &str) -> bool
```

Requirements:

1. **Resolve before judging.** `sub/../.oo/HEAD` must be caught (measured: it
   overwrote HEAD at baseline). Absolute paths must be caught. Symlinks must be
   caught — `innocent -> .oo`, then `innocent/HEAD`.
2. The target usually does not exist yet, so `fs::canonicalize` on the whole path
   will fail. Canonicalize the **nearest existing ancestor**, then append the
   remaining components with `.` / `..` normalized. Do not normalize `..` purely
   lexically across a symlinked ancestor.
3. **Compare by component.** `std::path::Component::Normal(s)` where `s == ".oo"`.
   Rust's `Path::starts_with` is already component-wise; the trap is only if
   someone reaches for `str::starts_with`. Pin
   `pin_dot_oo_is_matched_by_component_never_by_prefix` guards this and covers
   `.oo_peer_a`, `.oomisc`, `foo.oo`, `.ooo`.
4. Refusing the store directory itself, not only paths under it.

### 3.2 The refusal

New `BottomCause` variant, **appended at the tail** (the enum's existing comments
already mandate this — the serialized form is the variant name and old objects
must keep deserializing):

```rust
/// Filesystem access from the language layer to a path inside the engine
/// store (`.oo`). Unconditional — no capability unlocks it.
/// SPEC_08 §6.3; TAG_REGISTRY #store_boundary.
StoreBoundary,
```

with `as_tag()` → `"store_boundary"`.

Return `_|_` with this cause. **Not `#none` and not `#false`.** v0.2.41's lesson
was that an audit face you cannot tell apart from an ordinary outcome is not an
audit face; a refused `exists` must be distinguishable from "the file is not
there". `red_exists_on_store_is_refused_not_answered_false` pins exactly this,
with `#false` on a genuinely absent path as its control.

Include the offending path in the ⊥ message.

### 3.3 Call sites — the complete list

The filesystem surface was enumerated untruncated; there is no process-spawning
builtin (`process.rs` is `exit`/`pid` only), so this list is closed.

| builtin | file | note |
|---|---|---|
| `io.read_file` | `builtins/io.rs:11` | |
| `io.write_file` | `builtins/io.rs:23` | |
| `io.exists` | `builtins/io.rs:40` | must be ⊥, not `#false` |
| `io.append_file` | `builtins/io.rs:50` | |
| `csv.read_csv` | `builtins/csv.rs:157` | |
| `disc.connect` | `builtins/disc.rs:76` | takes a peer *base* dir, then `ObjectStore::init` creates and reads `<base>/.oo/objects`. Judge the path as handed in. `remote:`-prefixed values are not filesystem paths — leave them alone. |

### 3.4 A1 — retire `~%Official./add_architect`

Remove the morphism from `~%Official` (`lib.rs:1036`) and the
`engine.add_architect` builtin (`builtins/engine.rs:300`). Do **not** repair it:
it is the front door to precisely the trust root this arc is closing the back
door on, and REAL_01 §7.2's own answer is out-of-band provisioning from
`~/.oo/authorized_keys`.

Full-tree scan already done — the only references are its own definition and
historical `docs/worknotes/phase-*.md`, which are a record and must not be edited.
No conformance vector, no corpus file, no test depends on it.

Keep `ObjectStore::load_architects` (REAL_01 §7.2 mandates loading a whitelist)
and keep `save_architects` for the future CLI provisioning path.

`red_add_architect_is_off_the_language_surface` requires `#missing_key`, with
`/sign_refine` still resolving as its control — so "retired" is checkable rather
than an indistinguishable shade of bottom.

---

## 4. What NOT to touch

- **`storage.rs` read paths.** A3 is the next arc.
- **Anything under `Universe`'s own `.oo/` access.** The boundary is on the
  language surface. The engine reaches the store through `ObjectStore`/`Universe`
  and must be unaffected — `pin_engine_writes_are_unaffected` and
  `pin_gated_history_operations_still_work` guard this.
- **`crates/interpreter/tests/io_p34_test.rs`.** It writes to ordinary temp paths
  and is the regression guard that the boundary does not over-block.
- **`crates/oo/tests/pin_probe_test.rs:366`.** It demonstrates the v0.2.40
  escalation by writing `.oo/pin_pending` from n/. After this arc that write is
  refused, so the probe's *mechanism* changes even though its assertion still
  holds. Report it; the acceptor will update it. Do not edit it.
- **`tests/pending/federation_test.n`.** Uses `../.oo_peer_a`, which is a
  different directory name and must keep working.
- **`Identity::new_random()`.** See §5.
- **The `abandoned` file's lifecycle.** Persisting until the next commit is by
  design (v0.2.41 ruling R1), not a defect.

---

## 5. Ledgered, do not fix here

**The identity is regenerated every engine start** (`lib.rs:354` and `377`, both
`Identity::new_random()`). The local pubkey differs every process, so a signature
made in one run is unverifiable in the next: refine authority is self-authorizing
within a process and impossible across processes. Real, and larger than A2, but
it is the REAL_01 §7 / REAL_02 identity-persistence build.

**Byte-reclamation GC.** The store already accumulates — every `oo eval` and
`oo run -o` leaves objects behind (measured: a `_|_` from a failed call was found
sitting in `objects/`). GC must come *after* this arc: a GC that trusts
`.oo/HEAD` can be told to delete live history.

**Cross-workspace stores.** R-C covers them as a side effect of the component
rule. That is a bonus, not a claim — a program that can write another workspace's
store through some path the engine never sees is outside the model R-A chose.

---

## 6. Acceptance

Report when all of these hold. Do not report partial completion as done.

1. All 15 red gates green with `#[ignore]` removed — and **only** `#[ignore]`
   removed.
2. All 5 pins still green.
3. Workspace suite: no new failures against the baseline recorded below.
4. Conformance 143/143, genesis 11/11.
5. `cargo build --release` clean of new warnings.

Baseline four numbers are recorded in `meta/ENGINE_SYNC.md` (spec repo) for
v0.2.41 and restated in the acceptance thread.

State plainly anything you could not do, and anything you changed outside the
scope above — including anything you had to touch to make a probe pass. A
concession made silently to satisfy a probe is the failure mode this protocol
exists to catch (v0.2.39: a delivery widened the CAID parser to accept a bare
`_` because an acceptor probe asked for it; the probe was wrong, and the
widening reached 13 call sites including disk reads).

---

## 7. Delivery record (delivery side)

- **Tip**: `ae205b4`.
- **Boundary**: `crates/interpreter/src/builtins/fs_guard.rs` —
  `crosses_store_boundary` / `store_boundary_refusal`. Resolve nearest existing
  ancestor via `canonicalize`, append remaining with `.`/`..` normalized;
  refuse iff any `Component::Normal == ".oo"`.
- **Cause**: `BottomCause::StoreBoundary` (enum tail) → tag `store_boundary`.
- **Call sites**: `io.{read,write,exists,append}_file`, `csv.read_csv`,
  `disc.connect` (filesystem peers only; `tcp://` / `remote:` skipped).
- **A1**: removed `~%Official./add_architect` morphism + `engine.add_architect`
  builtin. `load_architects` / `save_architects` kept.
- **Probe**: only 15 `#[ignore]` removed.
- **Gates**: store_boundary probe **20/20**; genesis **11/11**; conformance
  **143/143**; release build clean of *new* warnings (pre-existing unused
  import / mut warnings unchanged).
- **Workspace**: **1449/1/3** — the single failure is exactly the predicted
  out-of-scope probe:
  `crates/oo/tests/pin_probe_test.rs::pin_intent_file_is_not_authority`
  (precondition: "unprivileged program CAN write `.oo/pin_pending`"). Work
  order §4: report, do **not** edit. Acceptor to update that probe's
  mechanism.
- **Not touched**: `storage.rs` read paths (A3 next arc); engine/Universe
  `.oo` access; `io_p34_test`; identity persistence; GC.
