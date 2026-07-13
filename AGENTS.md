# Project Instructions

## Responses
- Keep responses concise
- Ask clarifying questions when instructions are unclear

## Code standards
- Follow Rust idioms; avoid `unwrap`/`expect` outside tests unless justified
- Use the existing `Error` type (src/errors.rs) for error handling, not ad-hoc error types
- No `dbg!`, `TODO`, `FIXME` comments (CI rejects these)
- New business logic should have tests
- `mockito` is used for mocking API calls
- Run `cargo fmt --all` and `cargo clippy --all-features -- -D warnings` and `cargo nextest run --all-features` after code changes

## Commits and PRs

- Code comments should describe what and why but not how
- `main` is the base branch when reviewing code
- The repo uses conventional commits
