# GitHub Copilot Instructions

This document guides GitHub Copilot’s code suggestions, reviews, and commit messages for this repository.  
Copilot should follow these conventions when generating or completing code, comments, tests, CI, and commit messages.

---

## General Guidelines
- Prefer clear, maintainable, **idiomatic Rust** over clever one-liners.
- Keep naming precise and self-documenting; avoid abbreviations that obscure meaning.
- Code must **compile cleanly** (no warnings) and pass tests and clippy.
- Prefer `PathBuf` over `String` for filesystem paths.
- Debug prints start with `DEBUG:` (e.g., `eprintln!("DEBUG: …")`).
- Avoid unnecessary complexity; refactor deeply nested logic into small, pure functions.

---

## Code Review Suggestions (what to propose)
- Suggest idiomatic Rust improvements (iterators, pattern matching, `?` operator, `Option`/`Result` ergonomics).
- Identify and refactor overly deep nesting (extract helpers, early returns, guard clauses).
- Call out unclear/ambiguous names; propose descriptive function/variable names.
- Remove dead code, unused imports, and tighten visibility (`pub` only when needed).
- Prefer composition over conditional flags; keep modules cohesive.

---

## Rust Conventions
- Prefer `?` for propagation; **use `.expect("clear message")` instead of `.unwrap()`** when a panic is acceptable.
- Use clippy-friendly idioms; derive `Debug`, `Clone`, `PartialEq` where it aids testing and ergonomics.
- Use `chrono` + `chrono-tz` for time; avoid time-of-day flakiness in tests.
- Use structured error enums (`thiserror`) over panics for recoverable failures.
- Tests executed with `cargo nextest`.

### Example (good)
```rust
use std::{fs, path::PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read config at {0}: {1}")]
    Read(PathBuf, #[source] std::io::Error),
    #[error("invalid config json: {0}")]
    Parse(#[source] serde_json::Error),
}

pub fn load_config(path: &PathBuf) -> Result<Config, ConfigError> {
    let data = fs::read_to_string(path).map_err(|e| ConfigError::Read(path.clone(), e))?;
    let cfg: Config = serde_json::from_str(&data).map_err(ConfigError::Parse)?;
    Ok(cfg)
}
```

### Example (avoid)
```rust
fn load_config(path: &str) -> Config {
    let data = std::fs::read_to_string(path).unwrap();
    serde_json::from_str(&data).unwrap()
}
```

---

## Error Handling & Messaging
- **Prefer `expect` over `unwrap`** and include descriptive context.
- Print **consistent, descriptive** errors for unexpected cases; only auto-fix safe, deterministic scenarios.
- Centralize error reporting via a small helper so messages are uniform.

### Standardized error reporter (example)
```rust
/// Prints a standardized error message with context and source.
/// Use for unexpected errors you cannot recover from here.
pub fn report_error(context: &str, err: &(dyn std::error::Error)) {
    eprintln!("ERROR: {context}: {err}");
    let mut src = err.source();
    while let Some(cause) = src {
        eprintln!("caused by: {cause}");
        src = cause.source();
    }
}
```

---

## Documentation & Comments
- **Doc comments (`///`)** on public items and on **complex functions** (at the top of the function).
- **Inline/dev comments (`//`)** explain non-obvious decisions and trade-offs (the “why,” not the obvious “what”).
- Comments should be **adjacent to the function(s) they describe**, not at call sites.
- Keep comments crisp; update them if behavior changes.

### Doc comment example
```rust
/// Parses a user config file from `path`.
///
/// # Errors
/// Returns `ConfigError` when the file cannot be read or the JSON is invalid.
pub fn load_config(path: &PathBuf) -> Result<Config, ConfigError> { /* … */ }
```

---

## Tests
- **Every new function** should get tests; any new feature or bug fix must include tests reproducing/fixing behavior.
- Use `assert_cmd` for CLI and `mockito` for HTTP.
- Mark slow or external-dependency tests with `#[ignore]`; keep unit tests fast.
- Avoid time fragility; inject a `TimeProvider` or use fixed/mocked time.

---

## GitHub Actions / CI
- Prefer **reusable workflows**; avoid duplicate runs on `push` and `pull_request`.
- Always set `concurrency` to prevent overlapping runs.
- Cache Rust builds with `Swatinem/rust-cache`.
- Follow naming patterns already used here: `release_build_test.yaml`, `release_upload.yaml`, `release_publish.yaml`.

---

## Commit Messages (Conventional Commits)
All commit messages (including generated ones) **must** follow **[Conventional Commits v1.0.0](https://www.conventionalcommits.org/en/v1.0.0/)**:
- **all lowercase**
- **type** is one of:  
  `[build, chore, ci, docs, feat, fix, perf, refactor, revert, style, test]`  
- Optional **scope** in parentheses after type (component/module), e.g. `feat(config): …`
- Message: short imperative summary; body (optional) explains why; footer for breaking changes or issue refs.

### Good
```
feat(config): support pathbuf for config path
fix(parser): handle empty lines in csv import
ci(workflow): add concurrency to prevent overlapping runs
test(cli): add coverage for invalid regex error
refactor(tasks): split nested loop into helper functions
```

### Avoid
```
Update files
Fix stuff
Refactor
```

---

## Pull Requests
- Include: summary, rationale (“why”), and a brief list of changes.
- Link issues and note breaking changes clearly.
- Ensure tests/docs updated; check clippy and fmt.
- Prefer small, focused PRs over large mixed changes.

---

## Checklist (what Copilot should auto-enforce)
- [ ] Use `expect` over `unwrap` with clear messages.
- [ ] Print consistent, descriptive errors via the standard helper.
- [ ] Add tests for **every** new function and for all fixes/features.
- [ ] Functions and modules have appropriate `///` docs; non-obvious logic has `//` comments.
- [ ] Comments live beside the functions they describe.
- [ ] Use `PathBuf` for paths; avoid unnecessary `pub`.
- [ ] Commit messages follow **Conventional Commits** (types from the allowed list; lowercase; optional scope).
- [ ] CI jobs are not duplicated; concurrency is set; rust-cache enabled.

---

