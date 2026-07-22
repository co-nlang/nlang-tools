# Work order — `%super` derived reflection + `%type`→`%name` retirement (B5 / R1)

> Pre-committed probes: `crates/interpreter/tests/type_super_probe_test.rs`
> (8 reds `#[ignore]`, verified red-for-right-reason 2026-07-22; 6 pins green).
> Conformance: `conformance/L2/93..96` (red until delivery).
> **Deliverer removes `#[ignore]` only — never edit a probe assertion or a
> conformance vector. Probe/vector rights belong to the acceptor.**

## 0. Ruling (R1, user-adjudicated 2026-07-22)

`@T`'s content fields `%super`/`%predicate` (SPEC_03 §4 isomorphism table,
SPEC_05 §3.2) were an early import of a traditional (C-style) type system,
before n/ was understood as a pure structural lattice. In C the type tells
you how to interpret bytes in a fixed space; n/ is the lattice directly, so
type-checking already happens by `&` (SPEC_05 §5 generics — nothing to add
there). What a lattice value genuinely needs is the **reverse of monotone
convergence**: convergence goes DOWN (`&` refines), but "how is this datum
handled" is a lookup UP the hierarchy. `%super` is that upward link.

- **`%super`** = a DERIVED reflection view (like `%kind`, SPEC_05 §4), holding
  the **SPEC_09 §2.1 hierarchy tree** immediate parent. Load-bearing.
- **`%predicate`** = RETIRED. In the structural core the constraint IS the
  combo, checked by `&`; a separate `%predicate` (the `P_instance ⊑ P_type`
  subsumption) only means anything in a NOMINAL layer = **R2**, which is
  ledgered (a candidate future mechanism for cross-engine custom-type-design
  exchange), NOT built here. `%predicate` must remain an ordinary open-miss.
- **`%type` "Name" payload** = RETIRED (the last `%type` fossil; the cocoon
  arc 2026-07-19 retired the rest). The type NAME reflection spelling
  converges to **`%name`** (unifying with stdlib type nodes).

## 1. Deliverable

Three changes, all on the OBSERVABLE reflection surface; the structural
type-check machinery (`type_constraint_meet`, `is_type_constraint_combo`,
generic `&`) stays behaviourally identical.

### 1a. `%super` — derived hierarchy back-link (the load-bearing piece)

Reading `.%super` on a **type value** (`is_type_constraint_combo`) returns
its immediate parent type marker, per the SPEC_09 §2.1 tree:

| type | `%super` | | type | `%super` |
| :-- | :-- | :-- | :-- | :-- |
| `@any` | — (⊤, open-miss `_`) | | `@str` | `@any` |
| `@unit` | `@any` | | `@list` | `@any` |
| `@bool` | `@any` | | `@combo` | `@any` |
| `@num` | `@any` | | `@record` | `@combo` |
| `@complex` | `@num` | | `@morphism` | `@any` |
| `@float` | `@complex` † | | `@type` | `@any` |
| `@int` | `@num` | | `@caid` | `@any` |
| `@u8..@u256`, `@i8..@i256` | `@int` | | *user field-type* | `@combo` |

- The returned value is a real type marker (`%kind: #type`), so the chain
  composes: `((@int).%super).%super = @any`.
- `@any` has no super → honest open-miss `_` (do NOT fabricate a self-loop).
- A non-type value (plain datum `42`) open-misses `_` — `%super` is
  type-only.
- **† float discrepancy (flagged for spec closure, NOT a deliverer
  decision):** §2.1 tree puts `@float` under `@complex`; §2.3 table lists
  `@float → @num` (a non-immediate ancestor). The user pointed `%super` at
  §2.1, so the map above uses `@float → @complex`. `@float` is deliberately
  kept OUT of the red gates; implement per the tree, the acceptor reconciles
  §2.3 wording at closure.

Mechanism hint: `.%super` is DERIVED (not a stored field) — intercept it in
the meta-read path (near `crates/interpreter/src/lib.rs` ~1680–1705, where
type-marker navigation resolves) with a `name → parent-name` table, minting
`TypeConstraint::marker_value(parent)`. A user field-type (type-constraint
combo whose name is not a builtin) → `@combo`.

### 1b. `%name` — name reflection (retire the `%type` payload)

The marker's internal payload field is minted as `%type` today
(`crates/interpreter/src/dispatch.rs:108`). Rename the mint to `%name`, and
follow the reader `get_type_constraint_name` (`type_constraint.rs:257`) to
read `%name`. Then:

- `(@int).%name` → `"int"` (plain field nav on the renamed slot).
- `(@int).%type` → `_` (ordinary open-miss; the fossil spelling is gone).
- `@int` display → `{{ %kind: #type, %name: "int" }}` (no `%type` leak).

### 1c. `%predicate` — stays retired

No code. Confirm `(@int).%predicate` remains an ordinary open-miss `_`
(pin `pin_predicate_stays_retired`).

## 2. Red gates (must flip green) + pins (must stay green)

`type_super_probe_test.rs`:
- reds (8, `#[ignore]`): `red_super_numeric_chain`,
  `red_super_fixed_width_to_int`, `red_super_record_to_combo`,
  `red_super_chain_navigable`, `red_super_user_type_is_combo`,
  `red_name_reflection_via_name`, `red_type_payload_retired`,
  `red_display_no_type_leak`.
- pins (6, green now): `pin_any_top_has_no_super`,
  `pin_atom_super_out_of_scope`, `pin_predicate_stays_retired`,
  `pin_kind_unchanged`, `pin_structural_membership_unchanged`,
  `pin_generic_specialization_unchanged`.

Conformance `conformance/L2/`: `93-type-super-int-num`,
`94-type-super-fixed-width`, `95-type-name-reflection`,
`96-type-top-no-super` (red until delivery → 135/135 after).

## 3. Baselines (measured 2026-07-22 @ v0.2.32 head, probes added)

| suite | baseline (pre-delivery) | target (post-delivery) |
| :-- | :-- | :-- |
| `type_super_probe_test` | 6 pass / 8 ignored-red | 14 pass / 0 ignored |
| workspace | 1313 / 0 / 11 | 1321 / 0 / 3 |
| conformance | 132 / 135 (L2-93/94/95 red; L2-96 already green) | 135 / 135 |
| corpus | 75 / 0 | 75 / 0 (unchanged) |

Retirement due-diligence (red line): tree-wide `%type` grep done — the only
remaining live `%type` is this marker payload; corpus (`.n`) has no `.%type`
dependency; cocoon/blur `.%type` pass-through behaviour is unaffected (that
path is not a meta-read and does not touch the marker payload).

## 4. NOT in scope

`%predicate` implementation (R2); nominal name-based subtyping (R2); the
§2.1/§2.3 float wording (acceptor, at closure); atom `%kind` inference;
stored-universe migration (marker payload rename is a one-time CAID-neutral
internal change — markers are re-minted per observation, not stored under
their payload spelling; verify no CAID shift in acceptance).

## 5. Delivery record

*(model #3 fills in: commits, any engine repairs, deviations.)*

## 6. Acceptance (acceptor)

- [ ] diff purity: only `#[ignore]` removed from probes; no probe/vector edit.
- [ ] 4 numbers re-run on delivered head: probes 14/0/0, workspace 1321/0/3,
      conformance 135/135, corpus 75/0.
- [ ] adversarial: `%super` chain to `@any`; `@any`/atom open-miss honest;
      `%type` gone everywhere (display + read); `%predicate` still `_`;
      generic `&` and membership unchanged; no CAID shift on type markers.
- [ ] spec closure: SPEC_05 §3.2 rewrite (R1: %super derived / %predicate
      retired / %name), SPEC_03 §4 table (drop %predicate from `@T` row),
      SPEC_02 §30, REAL_04 §1 (drop %predicate mention), §2.3 float wording,
      R2 ledger entry (nominal + cross-engine exchange), CHANGELOG, GLOSSARY
      (%super / %name).
