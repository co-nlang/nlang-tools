# The meter reads two — delivery record

2026-08-11. Implementation against `the_meter_reads_two_recon.md`; no
acceptor probe body was changed.

## Billing decomposition

REAL_01 §9.1 is represented by one engine-owned MBU schedule. The prices used
by the current language evaluator are these:

| fixture / semantic operation | charge |
|---|---:|
| path segment, `#ref` dereference, cocoon node opened by `force_recursive`, and every subspace inspected by the solid-combo fast path | subspace expansion: 1 each |
| direct `f x` application and each `|>` stage | operator application: 10 each |
| list pipe | its pipe application (10) + lifting base (5) + the inner application (10) for every list element |
| `&` and recursive/distributed lattice intersections; `|` and its runtime absorption comparisons | orthogonal merge: 5 per semantic merge |
| runtime union flattening and recursive orthocomplement traversal | subspace expansion: 1 per runtime member |

Thus `pipe_chain(k)` receives one 10-MBU application charge at every stage;
`nested_apply(k)` receives one at every syntactic application; `lift_over(n)`
receives `10 + 5 + n*10` before incidental path/subspace work; and a merge
chain receives 5 for every executed lattice merge. A nesting cocoon is billed
by semantic nodes/subspaces, not by this implementation's recursive frames.

The old `10 + 2n` charges for combo, tuple, and poset literals were removed:
their construction is bounded by the supplied AST, so it is not a dynamic MBU
operation. The generic AST walker retains a zero-cost resource check only for
depth, stack, and timeout gates.

`Spectral calibration` (25) and `FFI` (50+) are retained as named schedule
rows, but this interpreter has no language-level evaluator dispatch for either
operation. Poset rank derivation is over a literal relation AST and is not
treated as spectral calibration. A future operation that exposes either path
must bind to these rows before it can run under an observation horizon.

## Completeness audit

The audit covered the previously free nesting walk and solid-combo fast path,
deferred-spread expansion, recursive combo/union merge, union normalization,
orthocomplement traversal, path/ref navigation, and force memo hits.

Two implementation details that could otherwise change the semantic horizon
were corrected:

* A force-memo entry now stores the MBU it consumed; a hit debits that same
  amount. Cache warmth can no longer make a later observation reach a different
  blur horizon.
* Merge billing sits in the recursive lattice operation rather than only at a
  surface `&`; dynamic union distribution and spread collisions therefore do
  not get free additional merges.

The remaining unmetered loops are either bounded by the already-supplied AST
(literal construction, rank construction, static lists/tuples) or are outside
the language observation evaluator (CLI/store/network management). They do
not mint a language `#blur` and are not substitutes for the future FFI row.

## macOS finding — not a Linux test

The store-boundary comparisons in `builtins/fs_guard.rs` are byte-/component-
exact: `Component::Normal(s) == ".oo"`, `paths_eq`'s component-vector equality,
and `is_node_key_dir`'s component-prefix equality. There is no case-folding
comparison in the engine.

On a case-sensitive filesystem `.OO` is a distinct name and these exact
comparisons intentionally differ from `.oo`. On a case-insensitive macOS
volume, `.OO` can resolve to the same directory as `.oo`; the outcome then
depends on what spelling `canonicalize` returns for the existing ancestor. If
it returns the on-disk `.oo` spelling the current guard refuses it; if it
preserves the supplied `.OO` spelling the exact post-resolution comparison can
miss it. Linux cannot establish either macOS behavior, so no vacuous test was
added. This needs a real case-insensitive APFS/HFS test before relying on the
boundary's case behavior.

## §10 repair record

Acceptance round 1 confirmed that semantic, cache-independent MBU billing is
correct: a warm memo must pay the same MBU as a cold evaluation, because cache
warmth must not move a fuel horizon or its `#blur` CAID.

`Ouroboros::force_memo_hit_count()` is therefore the new diagnostic observable.
It is a monotonic, per-engine counter of successfully served force-memo
entries. It is deliberately not reset by invalidation, and it does not affect
fuel, result values, or CAIDs. An acceptor can snapshot it before and after an
observation to distinguish a hit from a miss without reintroducing a
cache-dependent billing schedule.

The public specifications now state the O41 boundary: REAL_01 §9.1's
"operator application" means morphism application; AST-bounded traversal,
literal arithmetic, and structural construction are not MBU-billable unless
they trigger a charged semantic operation. SPEC_09's stale `timeout: 1000`
row and the matching engine comments are corrected to the O41 genesis default
`#_`.

For the final p2 literal pin, the current fixture was measured without
changing its test: the first digest selected by its present `find("hash:")`
helper is `hash:sha256:v1:3e731e0788ac0f47dec9db218007fe87ad3831687fea2b2dfc4adb54d83fd102`
(`/add` in the displayed root); the `v` member's own blur digest is
`hash:sha256:v1:de65bce3aa8a3e59c5bb70a55b95ab858dbcb89fb6b4901b41c57f307d8576bc`.
The acceptor should pin the former if retaining that helper, or explicitly
select `v` before pinning the latter.

## Validation and known changed contracts

* `cargo test -p oo --test the_meter_reads_two_probe_test --no-fail-fast --quiet`
  passes: 12 passed, 0 failed, 0 ignored.
* Union/absorption regression tests pass.
* The schedule remains a Layer-1 horizon/CAID change. §10's acceptor-side
  updates have already re-pointed the five non-memo contracts. The four
  remaining failures are exactly the old fuel-delta memo instruments:
  `stage4_acceptance_test::stage4_memo_reduces_fuel_on_second_observe` and
  `stage5_redline_test::{stage5_r1_memo_survives_unrelated_evolve,
  stage5_r2_related_evolve_still_invalidates,stage5_r3_c0_survives_any_evolve}`.
  This delivery did not edit those test bodies, as §10.6 requires; they can be
  re-pointed to `force_memo_hit_count()` by the acceptor.
* `run-conformance.py` passes 143/143; `genesis_test` passes 11/11.
