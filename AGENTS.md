# Project Instructions

## Responses
- Keep responses concise
- Ask clarifying questions when instructions are unclear

## Code standards
- Tool chain is pinned via `rust-toolchain.toml`
- Use the existing `Error` type (`src/errors.rs`) for error handling
- `.unwrap()` and `.expect()` should only be used in test cases unless it is justified with code comments.
- No `dbg!`, `TODO`, `FIXME`, `DEBUG:`, or `FIXTURE:` strings anywhere in `.rs` files — `scripts/test.sh` greps for these and fails the build
- New business logic should have tests

## Tests
- Unit tests live inline at the bottom of each source file in `#[cfg(test)] mod tests`, not in separate files
- CLI/integration tests live in `tests/*.rs` using `assert_cmd` + `tempfile::tempdir()` for isolated config files
- Mock Todoist API calls with `mockito::Server::new_async()`, then point the config at it via `.with_mock_url(server.url())`
- Shared test fixtures are in `src/test/fixtures.rs`; canned JSON API responses are in `src/test/responses.rs` (`ResponseFromFile`)
- Running tests can leave stray `tests/*.testcfg` files behind; clean with `./scripts/testcfg_clean.sh`

## Commands
- Run `./scripts/test.sh` to run checks 
- Format with `cargo fmt --all` before committing

## Commits and PRs
- Code comments should describe what and why but not how
- `main` is the base branch when reviewing code
- Conventional Commits are enforced by `commitlint` (`.commitlint.config.mjs`); header/body max line length is 250, all lowercase
