---
name: rust-checks
description: Run Rust verification checks in the tod project. Use when verifying code compiles, passes linting, or is ready to commit.
---

# Rust Checks

## Single verification command

```bash
./scripts/test.sh
```

This runs in order: `cargo fmt --check` → `cargo check` → `cargo clippy` → `cargo test` → forgotten-strings grep. If any step fails, the script exits.

## Individual checks

```bash
cargo fmt --all        # Format code (run before committing)
cargo check            # Fast compile check (no codegen)
cargo clippy           # Lint with warnings-as-errors
cargo test             # Run all tests
```

## Forgotten strings

`scripts/test.sh` greps for `dbg!`, `TODO`, `FIXME`, `DEBUG:`, and `FIXTURE:` in `.rs` files and fails the build if any are found.
