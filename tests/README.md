# n/ Language Test Suite

This directory contains the official test suite for the n/ language.

## Directory Structure

```
tests/
├── README.md              # This file
├── lib/
│   └── test.n            # Test utilities and assertions
├── unit/                 # Unit tests for individual features
│   ├── test_arithmetic.n
│   ├── test_logic.n
│   └── test_combo.n
├── integration/          # Integration tests
│   └── test_pipeline.n
└── static/              # Static analysis tests
    └── test_violations.n
```

## Running Tests

### Run all tests
```bash
cargo run -p oo -- test tests/
```

### Run specific test category
```bash
# Unit tests only
cargo run -p oo -- test tests/unit/

# Static analysis tests
cargo run -p oo -- test --static-only tests/static/

# Integration tests
cargo run -p oo -- test tests/integration/
```

### Run with pattern matching
```bash
cargo run -p oo -- test --pattern "arithmetic" tests/
```

## Writing Tests

### Test Naming Convention

Tests should be named with the `test_` prefix:

```n
test_description: expression
```

Examples:
```n
test_addition: 1 + 1 = 2
test_combo_meet: { a: 1 } & { b: 2 } = { a: 1, b: 2 }
```

### Test Evaluation Rules

- `#true` or `#pass` → Test passes
- `#false` or `#fail` → Test fails
- Any non-Bottom value → Test passes
- `_|_` (Bottom) → Test fails

### Using Test Library

Import the test library for assertions:

```n
/assert_eq: x y -> x = y
/assert_gt: x y -> x > y

test_example: /assert_eq (1 + 1) 2
```

## Test Categories

### Unit Tests
Test individual language features in isolation:
- Arithmetic operations
- Logic operations
- Combo/Cocoon operations
- Path navigation

### Integration Tests
Test feature interactions:
- Pipeline chains
- Morphism composition
- Complex expressions

### Static Tests
Test static analysis detection:
- Type conflicts
- Infinite recursion patterns
- Environment dependencies
- Randomness injection

## CI Integration

The test framework exits with code 1 if any test fails:

```bash
cargo run -p oo -- test tests/ || echo "Tests failed"
```
