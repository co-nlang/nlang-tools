# The meter reads two — recon & work order

2026-08-11. Baseline `nlang-tools` `dev` at the v0.18.0 tie-back, clean tree.
D2, deferred from the D1 arc (*every level doubles the universe*) and named
there as: fixing D1 does not fix this, and fixing this alone would turn
ordinary nesting into `#fuel_exhausted`.

---

## §1 What is on the floor

SPEC_08 §107 defines `%fuel` as bounding 「單次收斂觀測涉及的**節點展開與
遞迴總量**」 — the total node expansion and recursion of one converging
observation. Measured minimum viable fuel:

| shape | min fuel |
|---|---:|
| `nest=2` | **2** |
| `nest=8` | **2** |
| `nest=30` | **2** |
| `nest=120` | **2** |
| `nest=200` | **2** |

Constant. It bounds neither of the two things it is defined to bound.

Nor is it only depth. Every fixture below had its **value verified**, so none
of them is measuring a program that quietly did nothing:

| operation | value | min fuel |
|---|---|---:|
| observe `a: 1` | `1` | 2 |
| `1 + 1` | `2` | 2 |
| `(x -> x + 1) 1` | `2` | 2 |
| `5 \|> inc` | `6` | 2 |
| eight nested applications | `9` | 2 |
| eight-stage pipe chain | `13` | 2 |
| **lift `inc` over 20 elements** | `[2,3,…,21]` | **2** |
| `{x:1} & {y:2}` | both fields | 4 |

**Eight real applications cost what one atom costs. The meter reads two.**

## §2 Mechanism, and why the fix is not a list of call sites

`force_recursive` — the function that performs the nesting walk — calls
`check_resources(0)` at both of its sites (`lib.rs:3236`, `3261`). It takes
part in depth accounting (`ctx.depth += 1`) and in the timeout check, and
charges no fuel.

It is not alone. The same function opens with

```rust
if c.pending_spreads.is_empty() && value_is_fully_solid_combo(c) {
    return val;                          // no charge
}
```

and `value_is_fully_solid_combo` is **itself a recursive walk of the entire
subtree** — work that grows with the structure, performed before any charge,
and belonging to no row of the billing table. Repairing the two zeroes leaves
this one. That is why the probes pin a **property** (more work must cost more)
rather than a list of sites: a probe naming the sites would go green while the
next unbilled path stayed free.

## §3 Why this is not a performance matter

REAL_01 §9 is a **[Core Requirement]** and states its own purpose:

> 為了確保在視界邊緣產生一致的 **`#blur` CAID**，引擎必須遵循 MBU 能階計費。

The bill decides **where the horizon falls**; the horizon is part of the blur's
CHS (REAL_03 §7.3); O42 made the blur's identity **content-addressed**. So two
engines that bill differently mint **different CAIDs for the same program at
the same declared horizon**.

⟹ A meter that does not turn is not only a missing safety bound. It is an
interop hazard on the identity we fixed two versions ago, and it is invisible
today only because a second implementation does not exist.

## §4 The two schedules do not match

REAL_01 §9.1, normative:

| operation | MBU |
|---|---:|
| 投影展開 (subspace expansion) | 1 |
| 算子應用 (operator application) | 10 |
| 譜校準 (spectral calibration) | 25 |
| 正交合併 (orthogonal merge) | 5 |
| 算子升寫 (lifting) | 5 + E_inner |
| FFI | 50+ |

Measured at the observation boundary: application **2**, merge **4**, lifting
over 20 elements **2**. The code does contain non-zero charges (`unify.rs` 10,
combo `10 + 2n`, path 5, deref 32 or `1 + len`), but they are not what an
observation actually pays, and none of those numbers is documented anywhere.

## §5 Headroom

Real conformance vectors consume **1–7** units against a default of **10,000**
(measured on six L2 vectors). The deepest nesting anywhere in the corpus is
**9**; the longest operator run is **7**. An honest bill cannot plausibly break
existing code — which is why the guard against overcharging is C0 (ordinary
programs still evaluate at the default budget), not a smaller default.

This engine's nesting work is ~n^1.5..n² (post-D1: nest 50 → 0.167 s, 100 →
0.735 s, 200 → 2.152 s). Billing implementation visits would therefore cost
~10,000 for a 100-level nest — the entire default budget — and would make the
bill an artifact of one engine's data structures. See ruling 1.

---

## §6 Rulings (approved 2026-08-11)

1. **Bill in semantic units, not implementation visits.** Charge per REAL_01
   §9.1 operation (subspace expansion, operator application, merge, lifting),
   not per `force_recursive` entry. §9 exists so that blur CAIDs agree across
   engines; billing this engine's recursion would make the bill depend on
   frame-cloning behaviour that a linear implementation would not have. A
   depth-n nest bills ~n.

2. **The engine's schedule must BE the spec's schedule** (§4). The engine
   adopts §9.1's numbers; §9.1 is a Core Requirement and the engine's current
   numbers are undocumented.
   * **This is breaking.** Changing the schedule moves where horizons fall,
     hence which observations become `#blur`, hence their CAIDs. Recorded as a
     Layer 1 breaking entry with the 90-day clock restarting (previous: #11,
     v0.17.0). Practical corpus impact is nil (§5), and saying so is not the
     same as pretending the change is not breaking.

3. **Completeness, in two halves** (both to the spec):
   * **(a) the rule** — no billable operation may be charged zero. An
     operation is **billable when its cost is not bounded by the size of the
     already-evaluated AST**; if how much work it does depends on what happens
     at runtime (how deep it recurses, how many nodes it walks), it is billable.
   * **(b) its falsifiable form** — an observation performing strictly more
     billable operations MUST NOT cost strictly less.

   (a) is the rule and (b) is how anyone can prove you followed it. Without
   (b), (a) is taxonomy: §9.1 prices six operations and never says the list is
   exhaustive, so an engine can do unbounded work in operations that appear on
   no row and remain conformant. That is exactly what happened.

**Ride-alongs (no ruling needed, approved as same-arc cleanup):** SIGPIPE
panic; the three stale `#[ignore]`s; `mod advert_debug` shipping in
`oodp.rs`; the macOS case-insensitive-filesystem check (§9).

---

## §7 Probes (pre-committed, calibrated on the v0.18.0 tree)

`crates/oo/tests/the_meter_reads_two_probe_test.rs`. Acceptor-owned; the
delivery may only remove `#[ignore]`.

| id | asserts | baseline |
|---|---|---|
| **C0** | ordinary programs still evaluate **at the default budget**, with the measured values | green |
| **C1** | exhaustion still produces `#blur` + `%cause: #fuel_exhausted` | green |
| **C2** | a fuel-side blur is still reproducible (O42) | green |
| **P1** | an atom still costs ≤ 20 | green |
| **P2** | the corpus's deepest shape still fits in a tenth of the default | green |
| **R1** | depth is billed: `nest(200) > nest(2)` | **red**: 2 vs 2 |
| **R2** | width is billed: lift over 200 > lift over 1 | **red**: 2 vs 2 |
| **R3** | every pipe stage billed, and **uniformly** | **red**: `[2,2,2,2]` |
| **R4** | an application costs ≥ §9.1's 10, uniformly | **red**: marginals `[0,0,0]` |
| **R5** | four independent families each grow the bill | **red**: `nesting` 2 vs 2 |
| **R6** | closing a pipe early does not panic | **red**: exit 101, broken-pipe panic |
| **R7** | no draft module, no stale "Known Issue/Defect" ignores | **red**: `oodp.rs` |

Design notes:

* **C0 is the control the arc hangs on.** Every red is "this costs more than
  that", and the cheapest way to make a meter turn is to charge until nothing
  finishes.
* **R3/R4 assert uniformity, not only monotonicity** — an engine that billed
  the first stage only would satisfy a strict inequality and fail here.
* **R4 asserts `>= 10`, not `== 10`, and the probe says why**: no fixture adds
  an application and nothing else (a nested `(x -> x + 1) (…)` also introduces
  a morphism value; a named form adds a path reference priced at 1). Exact
  correspondence to §9.1 is carried by the delivery's written decomposition and
  the spec closure. **If the marginal cannot reach 10, report it** — do not
  adjust the probe.
* **R5 runs its control first, inside the loop**: the small case must complete
  at the default budget, so a failure is about cost and not about a broken
  fixture.
* **R7 is a source scan, so its control runs first** (≥50 `.rs` files found,
  and `check_resources` located) — a walker that silently found nothing would
  otherwise satisfy every "no violations" assertion. It also excludes its own
  file, which quotes both offending strings in its prose; this was caught in
  calibration, when the probe reported itself.
* **R6's fixture is deliberately large** (20,000 fields): a 300-line file fits
  inside the 64 KiB pipe buffer, exits 0, and proves nothing. The acceptor's
  first attempt at this measurement made exactly that mistake.

---

## §8 Work order

1. **Bill the nesting walk and the solid-combo fast path** (§2), in semantic
   units per ruling 1.
2. **Adopt REAL_01 §9.1's schedule** (ruling 2) and **write down the
   decomposition**: for each probe fixture, which operations were charged and
   at what price. The acceptance needs this to check ruling 2, since R4 can
   only pin a floor.
3. **Audit for other unbilled paths** against ruling 3(a)'s criterion — any
   operation whose cost is not bounded by the size of the already-evaluated
   AST. `value_is_fully_solid_combo` is one; find the rest rather than fixing
   the two named zeroes.
4. **Ride-alongs**: SIGPIPE must not panic; delete `mod advert_debug`; remove
   the three stale `#[ignore]`s — and for `lazy_stress_test`, give it the same
   64 MiB thread builder every other interpreter test uses (its "Known Issue"
   is that missing builder, not an engine defect: the same 50-deep chain
   answers `149` through the CLI and 200 answers a clean `#max_depth_exceeded`).
5. **Do not change default `%fuel`.** §5 says the headroom is ~1400x. If the
   delivery finds a real program that no longer fits, report it — raising the
   default to hide an overcharge would defeat C0's purpose.

Standing constraints: probes are acceptor-owned (remove `#[ignore]` only); no
`git add -A`; existing tests that break must be **reported, not edited**;
English commit messages.

## §9 Named, not smuggled in

* **macOS case-insensitive filesystem**: `.oo` path comparison is byte-exact
  and has never been tested on a case-insensitive filesystem; the first Mac
  user will find out. **Not probeable on Linux** — a test here would pass
  vacuously. The delivery must produce a written finding (which comparisons
  are byte-exact, which would differ) rather than a green test.
* **Parser-thread panic mislabel** (v0.18.0 §10.5): `with_parser_stack_using`
  maps a panic inside the parser thread to `#stack_overflow`. Next parser arc.
* **pest replacement**: the real ceiling-raiser, already deferred in v0.18.0.

---

# §10 Acceptance round 1 — repair required (2026-08-11)

Probe integrity **intact**: exactly seven `#[ignore]` removals, no probe body
edited. Arc probes **12/12**. conformance **143/143**, genesis **11/11**,
workspace **1874/0** with the nine reported failures reproduced exactly.

The delivery contains one finding worth more than the arc asked for, and one
error of direction. Its own characterisation of the nine — "all pin an old
low-fuel threshold, old blur CAID, or fuel-cheaper cache hit" — is **wrong for
five of them**, and that mischaracterisation is what this section exists to
correct.

## §10.1 What the delivery got right, including something we did not ask for

**Cache-independent billing.** A memo hit now debits the MBU it originally
consumed. Before, a warm cache left more fuel, so the horizon — and therefore
the `#blur` CHS, and therefore its CAID — depended on whether the value had
been observed before. `stage5_r1`'s own precondition (`warm > cold`) is the
proof that cache state was observable in the fuel account. This is the O42
defect class (identity depending on a runtime reading) in a place nobody had
looked, and the delivery found and closed it unprompted.

## §10.2 Arithmetic — a spec ambiguity, resolved by O41, not by this arc

Measured: `1+1+…` at 10 terms and at 100 terms both cost 2. Literal arithmetic
is unbilled, and the delivery says so explicitly — it removed the old
`10 + 2n` charges on combo/tuple/poset construction as AST-bounded.

The acceptor first read this as "the completeness rule was applied in reverse"
and prepared to require the charges back. **That was wrong**, and the ruling
that settles it already existed:

> **O41 (user, 2026-08-09)**: every horizon knob defaults to `#_` **except
> `%fuel`** — it is 「唯一一個『拿掉之後觀測不再必定終止』的旋鈕」.

That is `%fuel`'s job description on the record: it guarantees **termination**,
not CPU fairness. Work whose extent is fixed by the supplied AST always
terminates, so it needs no charge; what can fail to terminate — recursion,
self-reference, unbounded lifting — is billed. The delivery's reading is the
correct one.

It also completes a picture across three arcs, with no gap left:

| bound | what it limits | in the CHS? |
|---|---|---|
| parser fences (v0.18.0) | how big and deep the **source** may be | n/a |
| **`%fuel`** | what the source may **amplify into** | **yes** |
| `%timeout` | wall clock | **no** |

and it gives `%timeout`'s exclusion from the six CHS parameters a better
reason than the one recorded in O43 ("the only non-discrete horizon
parameter"): non-discreteness is the symptom. The reason is that **`%fuel`
measures movement on the lattice and `%timeout` measures the machine**, and
what measures the machine must not enter an identity. It is the same reason
SPEC_08 §117 forbids minting a `#blur` on timeout at all.

**Acceptor error recorded**: during this acceptance the acceptor claimed
SPEC_09 declares `%timeout: 1000` as a normative default that the engine
ignores. It does not — O41 changed the genesis default to `#_` and the engine
matches it (verified: genesis `~%Config.timeout` = `#_`, `fuel` = `10000`).
The claim came from reading a **stale table**; see §10.4.

## §10.3 Granularity — no violation, and deliberately left interim

Three of the nine (two controls and a pin, all on the same `<<_.>>` fixture)
failed because exhaustion no longer collapses the whole structural observation
into one blur. Mechanism: `eval.rs:915` went from `check_resources(1)` to
`check_resources(0)`, so the generic AST walk stopped being the thing that ran
out; the horizon is now reached inside each member.

Nobody designed this. **A blur appears wherever `handle_resource_exhausted`
happens to be called**, and moving the charge sites moved the report.

Checked against the spec rather than assumed: **SPEC_08 §3.2.4** says 「當運算
觸及視界邊緣…**節點**會根據 `%strategy` 進行語義坍縮」 — per node — and its
`#blur` row describes keeping the universe 「在『部分觀測』的聯集狀態」. The
new behaviour matches that wording more closely than the old one did. **No
violation**, so this arc does not have to resolve it.

Determinism verified, since the whole arc turns on it: the same program yields
a byte-identical 61-blur result across five separate processes and two working
directories.

Accepted as **interim**. Where an observation reports "how it ended" is being
redesigned separately (`~%Observe`), because that is a different question from
where the meter ticks — today they are one code path, which is why this moved
at all. The three probes have been re-pointed by the acceptor at the property
that survives ("exhaustion still says fuel", and the CAID is the same in every
process) rather than at the interim shape.

## §10.4 Repair list

1. **Restore memo coverage — the only must-fix.** Four tests
   (`stage4_memo_reduces_fuel_on_second_observe`, `stage5_r1/r2/r3`) used the
   fuel delta as their *only* instrument for "did the memo hit". The billing
   change is right, so the instrument is gone and memo correctness — survives
   an unrelated evolve, is invalidated by a related one — is now untested.
   The contract is not obsolete; the instrument is. Provide a
   cache-independent observable (an explicit hit counter or equivalent) and
   re-point those four at it. **Do not** re-couple fuel to cache warmth.

2. **Write ruling (乙) into the spec** so a second implementation does not have
   to guess — the divergence §9 exists to prevent:
   * REAL_01 §9.1: `算子應用` means **morphism application**, not any operator.
   * SPEC_08 §107: add that work whose extent is already fixed by the supplied
     AST is not MBU-billable, with O41's reason (`%fuel` guarantees
     termination) stated, not just asserted.

3. **Fix the stale row that misled the acceptor.** SPEC_09 has two tables that
   disagree: §447 (genesis knobs) says `timeout` → `#_` (O41); §526 (horizon
   parameters) still says `1000`. §526 is O41 residue. Editorial.

4. **Stale engine comments**: `lib.rs:246` and `universe.rs:913` still say
   "Genesis carries `timeout: 1000`". It carries `#_`.

## §10.5 Acceptor-side changes already made

Five tests re-pointed (delivery must not edit these; they are recorded here so
the next reader sees why they moved):

* `name_points_at_remedy::c1`, `::p2`, `knob_that_does_nothing::c1` — observe
  `v` instead of `v.%cause` / `v.%caid`; property preserved (§10.3).
* `cycle_test::test_fuel_exhausted_{strict,blur}_mode` — fixtures changed from
  literal arithmetic to morphism application, because under (乙) arithmetic can
  no longer reach the horizon and the tests had stopped testing exhaustion.

**`p2`'s literal digest is deliberately not re-pinned yet** — a repair round is
open and the schedule change moves every fuel-side blur address. It pins the
cross-process relation for now and **the literal goes back at final
acceptance**; that is an acceptance criterion, not a note.

Workspace after these edits: **1879 passed / 4 failed**, the four being exactly
the memo tests in item 1.

## §10.6 Acceptance criteria for the repair

* all 12 arc probes green; workspace green with **no** test edited by the
  delivery;
* the four memo tests test memo again, through an observable that cannot
  depend on cache warmth;
* `p2`'s literal digest re-pinned to the measured value;
* conformance 143/143, genesis 11/11, ×5 repeat stability, cross-version.
