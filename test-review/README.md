# test-review

Analyze test quality using mutation testing and LLM-powered suggestions.

## Overview

`test-review` runs mutation testing on your codebase to measure how effective your tests are at catching bugs. Unlike code coverage, which only measures what code is executed, mutation testing measures whether your tests actually *verify* the code's behavior.

**How it works:**
1. Creates small modifications ("mutants") to your code (e.g., changing `>` to `>=`)
2. Runs your test suite against each mutant
3. Reports which mutants "survived" (tests didn't catch the bug)
4. Optionally uses an LLM to suggest tests that would catch surviving mutants

## Supported Project Types

| Language | Mutation Tool | Property Testing | Snapshot Testing |
|----------|--------------|------------------|------------------|
| Rust | cargo-mutants | proptest | insta |
| Python | mutmut | hypothesis | syrupy |

## Installation

```bash
# Install test-review
cargo install --path .

# Install mutation testing tool for your language
cargo install cargo-mutants  # Rust
pip install mutmut           # Python
```

## Usage

```bash
# Run mutation testing on current directory
test-review

# Run on a specific project
test-review /path/to/project

# Run with LLM suggestions for surviving mutants
test-review --suggest

# Run on specific package (Rust workspace)
test-review -p my-crate

# Output as JSON for automation
test-review --format json

# Check if required tools are installed
test-review check

# Show recommended tools for project
test-review info
```

## Output Example

```
=== Test Review Report ===
Project: /home/user/my-project (Rust)

## Mutation Testing Results

  Total mutants:  42
  Killed:         38 (90.5%)
  Survived:       4

  Mutation Score: 90.5%

### Surviving Mutants

  1. src/lib.rs:45
     replace > with >=
  2. src/lib.rs:67
     replace + with -

## Assessment: Grade A

  Excellent test coverage! 90.5% of mutations were caught by tests.

### Recommended Improvements

  - 4 mutations survived - add tests for these code paths
  - 2 comparison operator mutations survived - add boundary condition tests
```

## Understanding Mutation Score

| Score | Grade | Interpretation |
|-------|-------|----------------|
| 90-100% | A | Excellent - tests thoroughly verify behavior |
| 80-89% | B | Good - most edge cases covered |
| 70-79% | C | Moderate - tests catch obvious bugs |
| 60-69% | D | Below average - many gaps in coverage |
| <60% | F | Poor - tests provide little confidence |

## LLM Suggestions

With `--suggest`, test-review uses your configured LLM to analyze surviving mutants and suggest specific tests:

```bash
test-review --suggest --model claude-fast
```

This generates suggestions like:

```
## Test Suggestions

  1. [high] Boundary Test - src/lib.rs
     Add test for when input equals boundary value

     #[test]
     fn test_boundary_equality() {
         assert_eq!(process(BOUNDARY), expected);
     }
```

## Automation

For CI/CD or overnight runs:

```bash
# JSON output for parsing
test-review --format json > results.json

# Exit with error if score below threshold
test-review --format json | jq -e '.mutation_results.score >= 80'
```

## Tips for Improving Mutation Score

1. **Property-based testing** - Use `proptest` (Rust) or `hypothesis` (Python) to test many inputs
2. **Boundary conditions** - Test at exact boundaries, not just above/below
3. **Error paths** - Test that errors are raised for invalid inputs
4. **Assertions** - Verify actual values, not just that functions return
