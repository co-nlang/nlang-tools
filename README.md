# n/ Tools: The Ouroboros Engine (v0.2.0)

> **The Ouroboros Engine** — A high-performance Rust implementation of the `n/` language, featuring lattice-based convergence, call-by-observation lazy evaluation, content-addressed evaluation, and federated truth discovery.

[![License: MIT/Apache-2.0](https://img.shields.io/badge/License-MIT%2FApache--2.0-blue.svg)](LICENSE-MIT)
[![Rust Version](https://img.shields.io/badge/rustc-1.75+-orange.svg)](https://www.rust-lang.org/)

`nlang-tools` is the primary implementation of the **n/ (n-slash)** language. It realizes the **Ouroboros** architecture, where computation is defined as the monotonic merging of geometric structures (Combos) within a globally consistent lattice.

Versioning: the engine shares its `major.minor` with the [language spec](https://github.com/co-nlang/nlang-spec) (`meta/VERSIONING.md` there); a bare-core release (no pre-release tag) certifies REAL_05 conformance. **v0.2.0 is the first bare-core release: REAL_05 Level 2 conformance 45/45** (`conformance/` corpus + `scripts/run-conformance.py` in the spec repo).

---

## 🏗️ Repository Structure

This repository is a Rust Workspace containing the core engine crates:

- **[`crates/parser`](crates/parser)**: AST definition and Pest-based grammar, fully synced to the finalized SYNTAX_01–12 / SPEC_14 grammar. Golden-AST vectors + seeded fuzz roundtrip guard against silent deformation; semantic canonicalization backs deterministic CAID generation.
- **[`crates/interpreter`](crates/interpreter)**: The core Unification Engine. Call-by-observation laziness (thunks, live refs, observation memo, per-coordinate incremental invalidation), the Trinity Isomorphism, resource-bounded observation, and LADD (Lattice-Aware Distributed Discovery) routing.
- **[`crates/oo`](crates/oo)**: The Ouroboros CLI — run, eval, evolve, commit, refine, fmt, serve, and `lint` (contextuality linter, Tier 1).
- **[`docs/`](docs)**: Architecture, implementation status, roadmap; `docs/worknotes/` holds per-feature work orders and acceptance records.
- **[`tests/`](tests)**: n/-language test corpus (the authoritative Rust suite lives in each crate's `tests/`).

---

## 🚀 Quick Start

### Build the Toolchain
```bash
cargo build --release
```

### Evaluate expressions
```bash
# Lattice meet: ranges are closed interval sets — not loops
./target/release/oo eval '5 & 1..10'      # → 5   (membership)
./target/release/oo eval '1..10 & 5..20'  # → 5..10 (intersection)
./target/release/oo eval '(1 | 7) & 1..3' # → 1   (superposition distributes)
```

### Evolve and Commit a Universe
```bash
echo 'name: "Alice", age: 30..40' > profile.n
./target/release/oo evolve profile.n

# Refine a declared possibility space monotonically
echo 'age: 33' > refine.n
./target/release/oo evolve refine.n

./target/release/oo status
./target/release/oo commit -m "Genesis profile"
```

### Start the REPL
```bash
./target/release/oo repl
```

---

## 🧩 Advanced Features

1.  **Call-by-Observation**: fields hold thunks; values solidify only when observed. Live references (`<<path>>`) dereference against the *current* universe at observation time. Observation memo + per-coordinate invalidation give incremental convergence (GUIDE_03 §11).
2.  **Two comparison families**: atomic `==`/`!=` absorb `_|_`/`_` (lattice extremes propagate); set-family `=`,`<`,`<=`,`>`,`>=` return clean booleans (`_|_` is the empty set, an operand — not a black hole).
3.  **Resource-bounded observation**: every evaluation is fuel/depth/timeout-bounded; exhaustion collapses honestly to `#blur` horizons or `_|_` per the observation strategy.
4.  **Contextuality Linter**: `oo lint` computes the context graph, clique number ω(G), and K4/K5 candidate sites (Tier 1: graph-theoretic, no obstruction claims).
5.  **LADD Routing**: discover and assemble truth across nodes via `~%Discovery`; conflicts resolve by information density.
6.  **Semantic Formatting**: `oo fmt` normalizes structurally identical code to the same CAID.

---

## 🧪 Testing

```bash
# Full Rust suite: 718 passed / 0 failed / 3 ignored (109 suites)
cargo test --workspace
```

Probe/redline test files (`*_probe_test.rs`, `*_redline_test.rs`) are pre-committed acceptance probes promoted to permanent regression guards — do not weaken them.

---

## 📜 License

Dual-licensed under [MIT](LICENSE-MIT) and [Apache-2.0](LICENSE-APACHE).

*"Lattice initialized. Convergence is inevitable."*
