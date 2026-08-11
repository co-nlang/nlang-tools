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
