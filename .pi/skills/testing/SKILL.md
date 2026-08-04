---
name: testing
description: Write and run tests in the tod Rust CLI project. Use when writing unit tests, integration tests, mocking Todoist API calls, or debugging test failures.
---

# Testing

## Test locations

- Unit tests: inline at the bottom of each source file in `#[cfg(test)] mod tests`
- CLI/integration tests: `tests/*.rs` using `assert_cmd` + `tempfile::tempdir()` for isolated config files

## Test fixtures and mocks

- Shared test fixtures: `src/test/fixtures.rs`
- Canned JSON API responses: `src/test/responses.rs` (`ResponseFromFile`)
- Mock Todoist API calls with `mockito::Server::new_async()`, point the config at it via `.with_mock_url(server.url())`

## Running tests

```bash
./scripts/test.sh           # Full verification: format, check, clippy, tests, forgotten-strings
cargo test -- <filter>      # Run specific tests
```

## Test cleanup

Running tests can leave stray `tests/*.testcfg` files behind:

```bash
./scripts/testcfg_clean.sh
```
