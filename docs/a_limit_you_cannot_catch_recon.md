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
* the depth constant is **measured**, not guessed: model #3 confirms the chosen
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
