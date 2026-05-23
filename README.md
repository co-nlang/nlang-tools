# n/ Tools: The Ouroboros Engine (v0.1.0)

> **The Ouroboros Engine** — A high-performance Rust implementation of the `n/` language, featuring lattice-based convergence, content-addressed evaluation, and federated truth discovery.

[![License: MIT/Apache-2.0](https://img.shields.io/badge/License-MIT%2FApache--2.0-blue.svg)](LICENSE-MIT)
[![Rust Version](https://img.shields.io/badge/rustc-1.75+-orange.svg)](https://www.rust-lang.org/)

`nlang-tools` is the primary implementation of the **n/ (n-slash)** language. It realizes the **Ouroboros** architecture, where computation is defined as the monotonic merging of geometric structures (Combos) within a globally consistent lattice.

---

## 🏗️ Repository Structure

This repository is a Rust Workspace containing the core engine crates:

- **[`crates/parser`](crates/parser)**: AST definition and Pest-based grammar. Features semantic canonicalization for deterministic CAID generation.
- **[`crates/interpreter`](crates/interpreter)**: The core Unification Engine. Implements the Trinity Isomorphism, MBU (Mover Billing Unit) resource accounting, and LADD (Lattice-Aware Distributed Discovery) routing.
- **[`crates/oo`](crates/oo)**: The Ouroboros CLI. Your entry point for running, evolving, formatting, and serving n/ universes.
- **[`docs/`](docs)**: Internal architecture documents and implementation status.
- **[`tests/`](tests)**: Comprehensive test suite, including federation and logical entropy verification.

---

## 🚀 Quick Start

### Build the Toolchain
```bash
cargo build --release
```

### Start the REPL
```bash
./target/release/oo repl
```

### Evolve and Commit a Universe
```bash
# Define some truth
echo 'name: "Alice", age: 30' > profile.n
./target/release/oo evolve profile.n

# Check status (bits entropy)
./target/release/oo status

# Commit to the object store
./target/release/oo commit -m "Genesis profile"
```

---

## 🧩 Advanced Features

1.  **MBU Accounting**: Every evaluation is resource-bounded. Infinite recursion safely collapses to `#divergent` when MBU limits are hit.
2.  **Geometric Reflection**: Introspect structures at runtime using `~%Reflection./keys`, `/type_of`, and `/is_cocoon`.
3.  **LADD Routing**: Discover and assemble truth across nodes using `~%Discovery./fetch`. Conflicting fragments are resolved via **Gravity** (Information Density).
4.  **Semantic Formatting**: `oo fmt` ensures that structurally identical code produces the same CAID by reordering fields and normalizing syntax.

---

## 🧪 Testing

```bash
# Run the full suite (Rust + n/ Integration)
cargo test
```

---

## 📜 License

Dual-licensed under [MIT](LICENSE-MIT) and [Apache-2.0](LICENSE-APACHE).

*"Lattice initialized. Convergence is inevitable."*
