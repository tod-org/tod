# Project Instructions

## Code standards

- Use the existing `Error` type (`src/errors.rs`) for error handling
- `.unwrap()` and `.expect()` should only be used in test cases unless it is justified with code comments.
- No `dbg!`, `TODO`, `FIXME`, `DEBUG:`, or `FIXTURE:` strings anywhere in `.rs` files — `scripts/test.sh` greps for these and fails the build
- New business logic should have tests covering both happy and sad paths

## Error handling

- All errors use the `Error` type (`src/errors.rs`)
- Use `Error::new(source, message)` for ad-hoc errors where `source` is a lowercase module or crate name (e.g. `"io"`, `"serde_json"`, `"reqwest"`, `"oneshot"`)
- Use `From` impls for wrapped library errors
- The `Display` impl applies `format::red_string` to messages and `format::yellow_string` to the source. Callers constructing error messages must not pre-apply `format::*_string` coloring — `Display` owns color output.

## Commands

- Run `./scripts/test.sh` as the single verification command — it runs format, check, clippy, tests, and forgotten-strings grep in one pass. Do not run `cargo test` separately before it.
- Format with `cargo fmt --all` before committing (included in `./scripts/test.sh`)

## Commits and PRs

- Never push directly to `main` — all changes go through pull requests and are merged into `main`
- Conventional Commits are enforced by `commitlint` (`.commitlint.config.mjs`); header and body lines are capped at 250 characters, all lowercase
- Branch naming: `type/short-description` (e.g. `fix/error-coloring`, `feat/add-foo`)
- Create PRs with `gh pr create --title "type: description" --body "..." --base main`
