# A limit you cannot catch — recon & work order

2026-08-11. Baseline `nlang-tools` `fc6d333` (v0.17.1 tie-back), clean tree.
Sibling of the evaluator-side arc *a_limit_you_cannot_choose* (W4″, v0.15.0/
v0.17.0): the same §2.7.3 distinction, now on the parser.

---

## §0 How this arc came to the front

It was ledgered as "the native stack ceiling, 130 ok / 140 abort,
`max_unification_depth: 20` avoids it." Two of those three claims did not
survive measurement:

* **It is the PARSER, not the evaluator.** `oo fmt` — which does no evaluation
  at all — aborts at the same depth. The evaluator's ceiling (v0.17.0,
  `HARD_RECURSION_LIMIT = 400`) is healthy: with the depth knob raised, a
  200-level nest evaluates to a clean `⊥ #stack_overflow` in both profiles.
* **No knob avoids it.** `max_unification_depth ∈ {default, 20, 256}` all abort
  at debug depth 130 — the crash is upstream of the universe being read.
* The "130/140" figures were read at 125 (below the wall) and extrapolated.

And there is a second, nearer wall the ledger did not mention at all: an
exponential-time one, reachable at depth 24.

---

## §1 What is on the floor — three defects, all remote

### A — exponential backtracking (time)

`n.pest` `primary` orders `tuple` before `"(" ~ expr ~ ")"`. `tuple` requires a
comma after the first `expr`, so every level of `(…)` grouping parses its whole
interior once as a failed tuple, then again as a grouping. Cost 2^depth.

| input | depth | wall |
|---|---:|---:|
| `(((…1…)))` grouping | 20 | 12.2 s |
| `(((…1…)))` grouping | 22 | 48.9 s |
| `(((…1…)))` grouping | 24 | >120 s |
| `(1,)` nested (tuple arm hits directly) | 1000 | **0.044 s** |
| `[…]`, `{…}`, `{{…}}`, `<<…>>` (no competing arm) | 1000–1200 | ≤0.4 s |

Counterfactual armed: one character (`,`) separates the 49 s case from the
0.044 s case. Inner width scales the constant only (18 levels: width 1 → 3.07 s,
width 4 → 4.96 s). **`~%Config.timeout` does not cover it** — measured 49 s with
`timeout:1000ms` set; the operator's only time knob cannot reach the parse
stage.

### B — no depth ceiling (crash)

The parser recurses on the native stack (a 64 MiB thread, `parser/src/lib.rs`
`with_parser_stack`) with no depth guard anywhere — `grep` finds no depth
counter in the parser. Deep nesting of any form overflows it:

| form | debug cliff | release cliff | per-frame |
|---|---:|---:|---:|
| `{{a: …}}` plain | 130 ok / 131 abort | 1335 / 1336 | 49–516 KB |
| cocoon / combo / scope | ~130 | ~1336 | — |

The symptom is `exit 134`, `thread '…' has overflowed its stack`, no `⊥`, no
`%cause`, no line number. This is a live violation of **ERROR_CODES §2.7.3**'s
first MUST — "the implementation MUST have a recursion ceiling of its own,
strictly below what it can survive" — written last week for the evaluator and
never applied to the parser.

### The wire — both are remote, and one packet is enough

`oo node serve` (`main.rs run_serve`): `read_line` with **no byte cap**, the
line handed to `parse_expr_only`, the accept loop **single-threaded**, each
connection handled inline. Measured, single `%`-correct payload:

```
{%op: #advertise, %ad: ((((…22 levels…))))}     69 bytes
  → server blocked >90 s (still backtracking 2^22)
  → a concurrent legit find_node client: timed out at 40 s (starved)
```

In a **debug** build the deep request *aborts the whole server process*. A
90-byte packet at depth 30 hangs the node effectively forever. `oodp.rs`'s
`MAX_AD_DEPTH = 8` cannot help: it walks the **already-parsed** AST
(`ensure_literal_body`, after `extract_ad_expr_and_source` has parsed), and the
process is dead or hung before the walk. The lock is on the inside of a door
with no lock of its own — and this is the same door a prior acceptor repair
already hardened (the engine used to *evaluate* an unauthenticated `%ad`,
arbitrary effect before any of §3.4's five checks).

---

## §2 Why pest's `set_call_limit` is not the fix for B

pest 2.7 exposes `set_call_limit(Option<NonZeroUsize>)`. Its own source
(`parser_state.rs`): the count is "a running total over all non-terminal
rules," incremented on entry, **never decremented**. It is the parser's
**fuel** (total work), not its **depth**.

Total rule calls do not track stack depth:

* a 130-level chain → ~800 total calls, stack depth 130 → **crashes**;
* a 200-field flat combo → ~800–1000 total calls, stack depth 1 → **safe**.

The two are indistinguishable to a call counter. Any limit low enough to catch
the deep crash also rejects legitimate large-but-shallow files (the corpus's
own files run to hundreds of fields). So `set_call_limit` is the wrong measure
for B. It is a legitimate *optional* backstop for A (it bounds total work, so no
crafted input runs unboundedly) — but A's root-cure is left-factoring, which
keeps the common case linear without ever false-rejecting a large file.

**A Rust stack overflow aborts** (guard-page SIGSEGV → `abort()`); it is not a
panic and `catch_unwind` does not catch it. So the ceiling cannot be enforced
by running and recovering — it MUST be checked **before** the parser recurses.
That is why B's fix is a gate ahead of the parse, not a number inside it.

---

## §3 Prior art (why "set a value" is a floor, and where the ceiling is)

Universal footgun: recursive-descent + native stack + adversarial nested input.

* **Billion laughs / XML entity expansion** — the canonical parser DoS; the
  consensus answer is a hard limit, because deep nesting is only ever
  adversarial.
* **JSON depth bombs** — Go `encoding/json` `maxNestingDepth = 10000`; Rust
  `serde_json` default `RECURSION_LIMIT = 128`. *(Tie-back: O42's first delivery
  added `disable_recursion_limit()` and re-opened exactly this crash on the data
  path. Same lesson, unlearned on the grammar path because pest ships no
  default.)*
* **Clang** `-fbracket-depth` default **256**, clean "maximum nesting level
  exceeded"; GCC/Ruby/PHP added nesting limits after crash reports.

Two problems, two root-cures:

* **A (exponential):** left-factor the ambiguous choice (chosen), or packrat
  memoization (CPython's PEG parser does this; pest does not). A depth limit is
  *not* a fix for A — depth 100 of `(…)` is still 2^100.
* **B (depth crash):** the true root-cure is to stop using the native stack —
  an explicit heap stack / trampoline, ceiling → available memory. For n/ this
  means **replacing pest** (pest owns the recursion; we cannot inject a heap
  stack). That is a project, not a bugfix, so **the pre-parse depth gate is the
  correct scope here** and the pest replacement is named as deferred (§7).

The parser's ceiling is **defence** (a crash fence); the evaluator's identical
`#stack_overflow` ceiling is **semantics** (the §2.7.3 incapacity boundary).
Same tag, two identities.

---

## §4 Rulings (approved 2026-08-11)

1. **A — left-factor the grammar.** Reorder/refactor so `(…)` is parsed once:
   consume `( expr`, then branch on the next token (`,` → tuple, `)` →
   grouping). Grouping becomes linear. **MUST preserve** the three values:
   `(x)` grouping/identity, `(x,)` 1-tuple, `(x, y)` 2-tuple (C2/P1/P2).

2. **B — a pre-parse depth gate.** Before pest recurses, reject input nested
   past a single conservative constant as `⊥ #stack_overflow`:
   * **single constant**, below the *debug* native cliff (~130 across forms)
     with generous margin — recommend **100** (the deepest file in the whole
     corpus nests 9; a 10× margin). It is an implementation fact, not a knob:
     it MUST NOT appear in the SPEC_09 §6 closed knob table, and no operator
     value may move it.
   * report **`⊥ #stack_overflow`**, NOT `#max_depth_exceeded` (policy) and NOT
     a `#blur` (an aborted parse has no addressable snapshot). Strict and Blur
     strategies both ⊥.
   * the gate must be a **depth** measure (not `set_call_limit`, §2). A linear
     pre-scan of nesting depth is the mechanism; it MUST be string/comment
     aware so brackets inside a string literal do not count (else a legitimate
     string of parens is wrongly refused). Covering every parse entry point in
     the parser crate (one gate) protects fmt, eval, run, serve and discovery
     uniformly.

3. **Byte cap — the wire read.** Bound the single-line read at the serve
   entry (align with the existing `MAX_DISCOVER_RESPONSE_BYTES = 64 KiB`);
   a line over the cap is refused **`#request_too_large`** (new reason) before
   being buffered whole. Orthogonal to A/B, same door.

Arc shape: **A + B + byte cap in one arc** — one door, one violated MUST, one
delivery. A and B are two different things (grammar ambiguity vs. absent
ceiling) with their own probes, which is why they earn separate reds, but they
ship together. Deferred: pest replacement (§7) and D2 (evaluator fuel meter on
the nesting path).

---

## §5 Probes (pre-committed, calibrated at baseline `fc6d333`)

`crates/oo/tests/a_limit_you_cannot_catch_probe_test.rs`. Acceptor-owned; the
delivery may only remove `#[ignore]`.

| id | asserts | baseline |
|---|---|---|
| **C0** | the parser still parses a shallow program | green |
| **C1** | a 40-level nest still evaluates to its value | green |
| **C2** | `(x)` / `(x,)` / `(x,y)` keep three distinct values (the A control) | green |
| **C3** | a legit wire find_node is served with no attacker (attributes R4) | green |
| **P1** | grouping is identity: `(1) == 1` | green |
| **P2** | a 1-tuple is not its element: `(1,) != 1` | green |
| **R1** | 24-level `(…)` finishes == 7 within budget *(ruling A)* | **red**: >12 s timeout |
| **R2** | 2000-level nest → exit 0 + `#stack_overflow`, no crash/blur/max_depth *(B)* | **red**: exit 134, abort |
| **R3** | 128 KiB request → `#request_too_large` *(byte cap)* | **red**: read whole, `#missing_field` |
| **R4** | node serves a legit client DURING a hostile deep request *(A+B, wire)* | **red**: legit starved / node felled |

Calibration run (baseline, `cargo test`): C0–C3, P1–P2 green (6/6); R1–R4 red
for the intended reason each (see the failure strings). R1 asserts the value
(== 7) so completion alone cannot satisfy it and a low depth gate that rejects
24 would FAIL it — forcing genuine linearization. R4 pins the **property** (the
node stays reachable), not a depth number, so no future optimisation outruns it.

Design notes carried from prior arcs, honoured here:
* the control the arc hangs on (C0) is first and forbids buying safety by
  breaking the parser;
* every "absence of bad" red also asserts a "presence of good" in the same run
  (R2 asserts the `#stack_overflow` value is present; R4 asserts the legit reply
  is present) — no vacuous greens;
* the adversarial payload (R4) is a **computing load** wrapped in a well-formed
  advertise envelope, not a malformed shape (v0.2.50 rule);
* R4 pins the property, not the number the old wall stood at (D1's lesson).

---

## §6 Acceptance criteria

Standard protocol: diff purity; probe-integrity proof (the delivery removed
only `#[ignore]`, no probe body edited); independent whole-workspace re-run;
repeat stability ×5; the R4 adversarial case; cross-version where applicable.
Additionally:

* **conformance 143/143 and genesis 11/11 must stay green** — the depth gate
  must reject nothing legitimate (corpus max depth 9 ≪ 100).
* the depth constant is **measured**, not guessed: model #4 confirms the chosen
  value sits with margin below the debug cliff for the worst syntactic form.
* `set_call_limit`, if used at all, is an A-backstop set high enough to never
  false-reject a large file — it is **not** the depth gate.

## §7 Deferred (named, not smuggled in)

* **pest replacement** — a hand-written heap-recursive parser would raise B's
  ceiling from ~130 to available memory and make the limit a policy choice
  rather than a survival necessity. Self-contained arc; the depth gate is the
  floor until then.
* **D2** — the evaluator charges no fuel on the nesting path (`force_recursive`
  calls `check_resources(0)`); `fuel: 20` completes a 14-level nest doing ~2^14
  units. Separate arc; fixing it alone would turn ordinary nesting into
  `#fuel_exhausted`.

---

# §8 Repair round 1 — the gate only covers brackets (2026-08-11)

Delivery by model #4. **Diff purity: clean.** Probe integrity: **intact** —
exactly four `#[ignore]` removals, no probe body edited. Grammar left-factoring
(A), pre-parse scan (B) and byte cap all present and well built; the parser's
own unit tests pass; all ten first-round probes green.

**Verdict: not accepted.** R2 and R4 went green while the node can still be
felled by one packet, because **every fixture in round one nested brackets**,
and the delivered gate counts delimiters. The delivery satisfied what was
specified. The specification was too narrow — that is an acceptor miss (§8.4).

## §8.1 Defect 1 — `!` walks past the gate and abort()s a serving node

`n.pest:66` `unary_expr = { (unary_op ~ unary_expr) | … }` is **right-recursive**
and `unary_op` is `!` — no delimiter, so the pre-parse scan counts nothing.

| measured on the delivered tree | result |
|---|---|
| `oo fmt` `!`×2022 (debug) | **exit 134**, `'<unknown>'` (parser thread) |
| `oo fmt` `!`×60000 (release) | **exit 134** |
| wire `{%op:#advertise, %ad: !…×8000 x}` | **serve process ABORTS**; attacker gets an empty reply; `kill -0` fails; `serve.log` ends in "has overflowed its stack" |

This is the arc's own DoS, reached by changing which character repeats.

**Grammar audit — `!` is the only one.** Every level of the precedence chain
(`expr → morphism → ternary → pipe → join → cmp → meet → add → mul → infix →
apply`) is a **loop** (`X ~ (op ~ X)*`); recursion reaches `expr` only through
`primary`, i.e. through a bracket. `unary_expr`'s right recursion is the sole
unbounded non-bracket parser recursion. Counter-checked: `x.a.a…`×8000
(postfix loop) is fine.

## §8.2 Defect 2 — flat source builds a deep AST and kills the walkers

Flat operator chains produce an AST as deep as the chain. The walkers
(`to_nlang` / `canonicalize`) recurse with no ceiling. Measured, `oo fmt`,
debug:

| chain | cliff |
|---|---|
| `1+1+…` | **7971 ok / 7972 crash** |
| `x@t@t…` | 7971 / 7972 |
| `x\|>f\|>f…` | 7971 / 7972 |
| `x&y&y…` | 7971 / 7972 |

One shared walker, one number, and the crash is in **`oo-main`**, not the
parser. `oo run` on identical input is **clean** (the evaluator's
`HARD_RECURSION_LIMIT` catches it) — which is what makes the formatter's
silence a gap rather than a policy. Not remotely reachable (a request whose
`%ad` is a deep unary/operator tree is rejected by `parse_nlang_request` /
`ensure_literal_body`'s allow-list before any deep walk), so this half is a
local-tooling defect, not a second DoS.

Corpus reality check: the longest operator run in any `.n` file in the repo is
**7**; the deepest bracket nesting is **9**.

## §8.3 Rulings (approved 2026-08-11) — two guards, two stages

**The gate is not one constant. It is two guards in two places**, and both
report the same `#stack_overflow` (same incapacity, two stages):

1. **Make `unary_expr` iterative.** `unary_op* ~ operand`, folding the run into
   nested `ExprKind::Unary` **iteratively** in `parse_expr` (a recursive fold
   would relocate the crash, not remove it). Same technique as ruling A: remove
   the recursion, do not cap it. **Consequence to state plainly:** once this
   lands, brackets are the only unbounded recursion in pest, and *only then* is
   the existing pre-parse delimiter scan complete for its purpose.
   MUST preserve `!` semantics (C4/P3).

2. **Add a post-parse AST-depth check.** After a successful parse and before any
   consumer walks the tree, reject trees deeper than a single constant with the
   same `ParserNestingLimitExceeded` / `⊥ #stack_overflow`.
   * It **MUST use an explicit stack** (or an explicit worklist). A recursive
     depth check is the crash it exists to prevent.
   * The constant is an implementation fact, not a knob: not in the SPEC_09 §6
     table, unmovable by any operator value.
   * **Recommended 1000**, and it must be *measured*, not adopted: ≈8x below the
     measured 7971 debug walker cliff and ≈140x above the deepest real corpus
     chain (7). #4 must confirm the chosen value sits with margin below the
     debug cliff **for the worst form after change 1** — a run of `!`×N now
     reaches the walkers instead of the parser, and `Unary`'s walker frame has
     not been measured.
   * Placing it once, in the parser crate at the parse entry points, covers
     fmt / evolve / run / repl / eval / wire uniformly. The pre-parse scan
     stays as it is: it guards pest, this guards everyone downstream.

Scope: **both merged into this arc** (user ruling, 2026-08-11). Defect 2 is
strictly speaking the formatter rather than the parser, i.e. outside ruling B's
literal wording, and is merged because it is the same class — deep input kills a
tool — and is cheap alongside change 1.

## §8.4 Acceptor miss, recorded

Recon §1 wrote "deep nesting of **any form**" but measured only `{{}}`, `{}` and
the scope fixture — three bracket families. R2 and R4 inherited that blind spot,
so the first delivery could be complete against the probes and incomplete
against the ruling.

The rule this restates, now failed twice (D1's R2 was the first): **when an arc
exists to push a wall out, the probes must reach past the NEW wall, and they
must pin the PROPERTY rather than one family of input.** R5/R6 are written that
way — they assert "deep input never kills the process; it either does the job or
reports a bottom", assert a shallow case in the same run so a too-low ceiling
cannot pass, and R6 sweeps all four operator families that share the walker.

Mechanical check added for future arcs: **when a ruling says "input of kind K",
enumerate the grammar productions that can produce K and cover each family** —
here, "deep input" had four bracket families, one prefix family and one
operator-chain family, and only the first was probed.

## §8.5 Repair probes (acceptor-owned, calibrated on the delivered tree)

| id | asserts | on delivery |
|---|---|---|
| **C4** | unary + chains keep meaning and formatting | green |
| **P3** | `!` is still orthocomplement; `!1` still `#conflict` | green |
| **R5** | `!`×8000 survivable locally **and** a bang-chain request does not fell a serving node (shallow `!`×200 green in the same run) | **red**: exit 134 in `'<unknown>'` |
| **R6** | `+`/`@`/`\|>`/`&` ×9000 survivable in `oo fmt` (200-term chain green in the same run) | **red**: exit 134 in `'oo-main'` |

Twelve green / two red on the delivered tree.

## §8.6 Acceptance criteria for the repair

Everything in §6, plus:

* **all 16 probes green** (10 first-round + C4/P3 + R5/R6).
* the AST-depth constant is **measured after change 1**, against the worst form
  including a `!` run, with the margin stated.
* `!` semantics unchanged (C4/P3), and `oo run` behaviour on deep operator
  chains unchanged — it is already clean and must stay clean.
* conformance 143/143, genesis 11/11, whole workspace re-run, ×5 stability.
* **verify the deep-AST drop path**: a tree at the ceiling is built and then
  dropped; Rust's recursive drop glue can overflow on its own. Formatting at
  7971 succeeds today, so drop is safe at least that far — confirm the chosen
  ceiling stays inside that.

---

# §9 Repair round 2 — the fence was set below a shipped guarantee (2026-08-11)

Round 1 is **accepted on its own terms**: probe integrity intact (only the two
`#[ignore]`s removed, R5/R6 bodies verbatim), grammar made iterative, the AST
fold iterative, the depth check on an explicit worklist, all 14 arc probes
green, and the gate boundary clean (4096 parses, 4097 reports, both exit 0).

The `PARSER_AST_DEPTH_LIMIT = 4096` constant is **correct and stays**. Its
stated justification was accurate and the acceptor's first reading of it was
wrong: `flat_chain(4000)` does exist and is used throughout
`blur_spread_probe_test` (9 sites), `effect_meta_probe_test` and others. Only
the word "conformance" was imprecise (they are interpreter probes). Verified
independently: 4096 is safe for the worst form **after** the unary change —
`!`x4096 and `+`x4096 both format cleanly in debug, `!`x4097 reports
`⊥ #stack_overflow`, and the evaluator still separates policy from incapacity
(4000 → `#max_depth_exceeded`, 9000 → `#stack_overflow`).

## §9.1 What the workspace run found

Two regressions against **already-cut** versions, neither visible from the arc's
own probes.

**(a) `limit_you_cannot_choose` (v0.17.0) — repaired by the acceptor.**
`chain(5000)` is now refused at parse, so C1 went red (nothing staged) and —
worse — **R2 stayed green vacuously**: all three of its assertions are
ABSENCES (`!#max_depth_exceeded`, `!#blur`, `!crashed`), and an empty universe
satisfies all three. The evaluator's `HARD_RECURSION_LIMIT`, which that probe
exists to exercise, was no longer being reached at all.

Repair: both fixtures 5000 → `EVALUATOR_REACHING_CHAIN = 1000` (under the parser
ceiling, over the default depth policy 256 and over `HARD_RECURSION_LIMIT` 400);
measured after the change, C1 reports `#max_depth_exceeded` and R2 reports
`#stack_overflow` **from the evaluator**. R2 additionally hardened with a
presence assertion, per the standing rule that a red asserting absence must
assert a presence in the same run. Counterfactual armed: restoring 5000 turns
the new assertion red and names the cause. 10/10 green.

**(b) `nesting_doubles_the_universe` (v0.17.1) — needs the delivery.**
The pre-parse fence of 100 refuses a 120-level nest, and that arc's R5 pins 120
**deliberately**, chosen just under the measured native cliff. This is a
capability regression against the immediately preceding release.

## §9.2 Why no small correction works

The fence must be **above 120** (shipped guarantee) and **below 131** (measured
debug native cliff): a ten-level window, on a cliff that moves 10x between build
profiles.

| profile | per level | cliff at 64 MiB |
|---|---:|---:|
| debug | 504 KB | 130 |
| release | 49 KB | 1335 |

No constant is safe there. The acceptor's recommendation of 100 was made
without grepping the tree for assertions pinning nesting depth — **the very rule
§8.4 added, violated in the same round by its author.**

## §9.3 Ruling A (approved 2026-08-11) — promise first, then size the stack

Choose the ceiling as a **language promise**, then size the parser thread stack
so the worst profile clears it with margin. (This is the "guaranteed minimum
nesting depth" shape raised in §3 — ISO-C translation limits — now forced.)

1. **`PARSER_NESTING_LIMIT` 100 → 256.** The number Clang's `-fbracket-depth`
   has defaulted to for years. Still an implementation fact, not a knob: not in
   the SPEC_09 §6 table, unmovable by any operator value.

2. **`PARSER_STACK_BYTES` 64 MiB → 512 MiB.** Measured requirement: a 3x margin
   at fence 256 needs ~378 MiB in debug; 512 MiB gives ~4x there and ~31x in
   release. On Linux a thread stack is reserved address space committed lazily,
   and the parser already spawns a thread with an explicit stack size on every
   parse — this changes the size argument of an mmap that already happens, not
   the number of syscalls. (Measured for context: `oo fmt` on a trivial file is
   4.2 ms end to end, dominated by process start, so thread spawn is not a hot
   path.)

3. **These two are one change.** With the fence at 256 and the stack at 64 MiB
   the fence guards nothing — the cliff is at 131, so deep input walks past the
   fence and aborts. R7 is written so it cannot be satisfied by moving the
   constant alone.

4. **Spawn failure must not panic.** A 512 MiB reservation can fail under a low
   `RLIMIT_AS` or a constrained container, where `with_parser_stack`'s
   `.expect("failed to spawn parser thread")` would abort the process — trading
   one crash for another. Spawn failure must surface as a clean error.

`PARSER_AST_DEPTH_LIMIT` stays **4096**, untouched.

## §9.4 Repair probe

`R7 the_promised_nesting_depth_is_real` — acceptor-owned, added and calibrated
on the round-1 tree (**red**: "256 levels were refused rather than parsed").

* **Part 1** — 256 levels must PARSE (`oo fmt` exits 0 and emits `{{`). Cannot
  pass with the fence alone: at 64 MiB the cliff is 131 and this aborts.
* **Part 2** — 120 levels must still EVALUATE, restating v0.17.1's guarantee
  **inside this arc**, so this arc can never lower a shipped capability without
  its own probes going red. Round 1 lacked that cross-arc guard; the regression
  was caught by the workspace run, which is luck compared to a probe that names
  the promise.

Fifteen probes total: 14 green, R7 red.

## §9.5 Standing rules earned

* **A ceiling is chosen against the tree's pinned assertions, not against the
  corpus alone.** "Corpus max depth is 9, so 100 is generous" ignored that a
  probe from the previous version pins 120. Grep the tree for the quantity
  before choosing its limit — the §8.4 rule, now shown to bind the acceptor too.
* **A ceiling and the resource it protects are one decision.** A fence chosen
  without sizing the stack is either useless (above the cliff) or a capability
  cut (below what shipped). Pick the promise, then buy the room.
* **When an arc introduces a limit, restate the previous version's guarantee as
  a probe inside the new arc.** Otherwise the only thing standing between a new
  ceiling and a silent capability regression is a full workspace run.

## §9.6 Acceptance criteria

Everything in §6 and §8.6, plus:

* all 15 probes green, and `nesting_doubles_the_universe` 8/8 and
  `limit_you_cannot_choose` 10/10 green **in the same workspace run**;
* the 512 MiB figure **verified by measurement**, not adopted: confirm the debug
  native cliff after the change is ≥3x the fence for the worst syntactic form;
* confirm parse latency on a small file is unchanged (thread spawn cost);
* spawn-failure path produces a clean error, not a panic.

---

# §10 Acceptance — round 2 accepted (2026-08-11)

**Verdict: accepted.** All three rulings landed, both cross-arc regressions are
closed, and every §9.6 criterion is verified by measurement.

## §10.1 Probe integrity

`#[ignore]` count is zero; R7's body is verbatim (rustfmt reflow only); the
acceptor's `limit_you_cannot_choose` edits are intact. The delivery added its
own unit tests (drop path, spawn failure, whitespace between unary prefixes) —
those are the delivery's, not the acceptor's, and are additive.

## §10.2 Results

| gate | result |
|---|---|
| arc probes | **15/15** |
| whole workspace | **1869 / 0 / 3** |
| repeat stability | **5 x 15/15** |
| `nesting_doubles_the_universe` (v0.17.1) | 8/8 |
| `limit_you_cannot_choose` (v0.17.0) | 10/10 |
| cross-version | new reads/evolves an old repo; **old still reads what new wrote**; `(7)`, `(7,)`, `!#true` identical across binaries |

## §10.3 The margin, measured

§9.6 required the 512 MiB figure be verified rather than adopted. The fence
blocks every source-level path to the cliff, so this needed a diagnostic spike
(one trial per process — a stack overflow aborts and cannot be bisected
in-process, which is the arc's own thesis turned into a test-harness
constraint). Spike applied, measured, removed; the tree is back to the
delivery.

Worst form (cocoon chain), full parser-thread closure (pest parse + `parse_expr`):

| parser stack | max depth | per level |
|---:|---:|---:|
| 64 MiB | 123 | 532 KB |
| 128 MiB | 248 | 528 KB |
| 256 MiB | 498 | 526 KB |
| **512 MiB (shipped)** | **997** | 525 KB |

Linear, and consistent with the independent 504 KB/level figure from §9.2.
**Fence 256 against a cliff of 997 = 3.89x margin**, meeting the >=3x
requirement. The first spike attempt measured only `NParser::parse` and gave
18 KB/level — an eightfold underestimate; the recursion that costs is
`parse_expr`, and both run on that thread.

Cost of the reservation, measured: parse latency on a trivial file **3.8 ms**
(4.2 ms at 64 MiB — no regression), and peak RSS while parsing a 256-level nest
is **141 MB**, confirming the 512 MiB is reserved address space committed
lazily, not memory taken.

## §10.4 Adversarial, live

Against a real `oo node serve`, debug build:

| payload | before this arc | now |
|---|---|---|
| `%ad` with 2000 nested parens | node blocked >90 s; concurrent client starved | **0.01 s**, `#stack_overflow` |
| `%ad` with 8000 `!` | serve process **aborted** | **0.01 s**, `#stack_overflow` |
| 128 KiB single line | read whole, answered `#missing_field` | **`#request_too_large`, 20/20** |
| legit `find_node` after each | — | served, node alive, zero overflow lines in the log |

**A measurement error of the acceptor's, recorded because it nearly became a
false defect report.** A first pass showed the oversized request getting
`Connection reset by peer` and 0/20 replies delivered, which looked like "the
refusal never reaches the peer". The server log showed 20/20 correct refusals,
so the fault was on the measuring side: the harness wrapped the whole recv loop
in one `except`, so an RST discarded the reply that **had already arrived**.
(The server closes with unread data in its receive buffer, which makes the
kernel send RST rather than FIN.) With the loop fixed to keep what it already
read — exactly what the probe's own Rust client does — the result is **20/20
delivered**. No defect; a broken instrument.

## §10.5 Open, ledgered, not fixed here

`with_parser_stack_using` maps **both** spawn failure and a **panic inside the
parser thread** to `ParserNestingLimitExceeded`, i.e. to `#stack_overflow`. The
spawn-failure half is what §9.3 asked for. The panic half is new and is a
mislabel: a genuine stack overflow **aborts** and never reaches `join()`, so a
failed join means an internal parser bug, and reporting a bug as an incapacity
is the same class of lie this arc exists to remove (ERROR_CODES §2.7.1/§2.7.3).
It also makes parser panics silent. Small (propagate the panic instead of
folding it), out of the accepted scope, and recorded here for the next parser
arc.

## §10.6 What this arc bought

* grouping parses **linearly** — `(…)` x24 went from >120 s to milliseconds,
  and the exponential is gone at the root (left-factored, not capped);
* the parser has an implementation ceiling that **reports** instead of
  aborting, satisfying ERROR_CODES §2.7.3's first MUST on the parse path for
  the first time;
* deep ASTs built from flat source no longer kill the walkers;
* the wire read is bounded and refuses by size;
* the promised nesting depth is now a **stated number with measured room**
  (256 against a 997 cliff) rather than an accident of build profile.
