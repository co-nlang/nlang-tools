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
/assert_eq: x y -> x == y
test_example: /assert_eq (1 + 1) 2
```

## Known stale entries (corpus triage backlog, 2026-07-11)

`oo test tests/unit/` currently reports **45 passed / 11 failed** — all 11 are
engine-dev era expectations that predate current semantics, concentrated in:

- `test_canonical.n` (2), `test_entropy.n` (3) — old `%bits`/canonical-order expectations
- `test_federation.n` (3), `test_ladd.n` (2) — old gravity/fetch fixtures
  (the LADD/federation behavior itself is covered green in the Rust suite)
- `test_reflection.n` — evolves to Conflict under finalized grammar

These are **corpus maintenance items, not engine regressions** (tracked in
`docs/feature-roadmap.md` backlog). New tests should follow the finalized
SYNTAX_01–12 spelling.
