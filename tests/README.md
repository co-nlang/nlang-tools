# n/ Language Test Corpus

This directory contains the **n/-language test corpus** — tests written *in* n/
and executed by `oo test`.

> **Authority note (2026-07-11)**: the authoritative engine test suite is the
> **Rust suite** (`cargo test --workspace`: 667 passed / 0 failed / 3 ignored,
> 106 suites), which includes the pre-committed acceptance probes
> (`*_probe_test.rs` / `*_redline_test.rs` — never weaken those). This corpus
> complements it as end-to-end language-level material; parts of it predate the
> 2026-07 SYNTAX_01–12 finalization (see *Known stale entries* below).

## Directory Structure

```
tests/
├── README.md
├── lib/                  # Test utilities and assertions (test.n)
├── unit/                 # Unit tests for individual features
├── integration/          # Integration tests (pipelines, morphism composition)
├── static/               # Static analysis tests (oo test --static-only)
├── pending/              # Deferred material (known-unsupported constructs)
└── *.n                   # Loose engine-dev era corpus (historical; being triaged)
```

## Running Tests

```bash
# All corpus tests
cargo run -p oo -- test tests/

# By category
cargo run -p oo -- test tests/unit/
cargo run -p oo -- test --static-only tests/static/
cargo run -p oo -- test tests/integration/

# Pattern matching
cargo run -p oo -- test --pattern "arithmetic" tests/
```

The test runner exits with code 1 if any test fails.

## Writing Tests

Tests are fields named with the `test_` prefix:

```n
test_addition: 1 + 1 == 2
test_combo_meet: ({ a: 1 } & { b: 2 }) = { a: 1, b: 2 }
test_range_membership: (5 & 1..10) == 5
```

Evaluation rules:

- `#true` / `#pass` → pass; `#false` / `#fail` → fail
- any non-Bottom value → pass; `_|_` (Bottom) → fail

Comparison family reminder (SYNTAX_06): use `==`/`!=` for collapsed atomic
comparison (absorbs `_|_`/`_`), and `=`/`<=` for set-family clean booleans.

Assertions via the test library:

```n
assert_eq: (x -> (y -> x == y))
test_example: /assert_eq (1 + 1) 2
```

## Corpus status (2026-07-12 cleanup — the old 11-stale backlog is CLEARED)

`oo test tests/unit/` = **65 passed / 0 failed**; `tests/integration/` = **7 / 0**.
`tests/pending/` holds deferred material and is EXPECTED to fail/parse-error
(old-grammar loose files moved there: builtin_test / genesis_seeds /
sprint1_verify / federation_test / federation_test_tcp; plus test_canonical —
blocked on engine gaps below).

Engine gaps exposed by this cleanup (measured, ledgered in nlang-spec
`meta/ENGINE_SYNC.md`; the corpus works around them and the workarounds are
marked in-file):

- **G1 combo equality**: identical bound combos compare `#false` in BOTH cmp
  families (`x: {a:1}, y: {a:1}` → `x == y` and `x = y` both `#false`), while
  atoms/lists are fine and `normalize_union` dedupes equal combos correctly —
  the leak is in the cmp evaluation path, not `PartialEq`. Corpus workaround:
  field-wise atom assertions.
- **G2 `/`-prefixed curried defs**: `/add: (x -> (y -> x + y))` breaks EVERY
  application form (⊥ #conflict) — the dispatch packaging swallows the inner
  morphism as a rule keyed `y`. Bare-name defs (`add: …`, referenced `/add`
  via prefix-alternates) work in all forms. Corpus rule: **define curried
  morphisms with bare names, body-parenthesized** (see lib/test.n header).
- **G3 fuel-exhaustion cause**: runaway morphisms report `%type` `#conflict`
  (cause refinement queued since L2-17).
- **G4 union-dedupe × navigation**: when ALL union branches survive a meet and
  dedupe to one combo, display shows the single combo but path navigation gets
  the pre-dedupe union → `#invalid_path`. Corpus workaround: conflict-kill
  distinguishers so a single branch survives.

New tests follow the finalized SYNTAX_01–12 spelling.
