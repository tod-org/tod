# Project Instructions

## Code standards

- Use the existing `Error` type (`src/errors.rs`) for error handling
- No `dbg!`, `TODO`, `FIXME`, `DEBUG:`, or `FIXTURE:` strings anywhere in `.rs` files — `scripts/test.sh` greps for these and fails the build
- New business logic should have tests covering both happy and sad paths

## Project structure

- `src/todoist/mod.rs` — REST API client (~1,270L). Key exports: `all_tasks_by_*`, `quick_create_task`, `create_task`, `complete_task`, `update_task_*`, `all_projects`, `all_labels`, `all_comments`, `all_sections_by_project`
- `src/todoist/request.rs` — HTTP layer (GET/POST/DELETE via reqwest)
- `src/commands/mod.rs` — CLI dispatch via clap `Subcommand` enum
- `src/commands/task_commands.rs` — Task subcommand handlers
- `src/commands/list_commands.rs` — List/view subcommand handlers
- `src/config/mod.rs` — Config struct + serialization
- `src/errors.rs` — Error type (`{message, source}`)
- `src/tasks/mod.rs` — Task struct + formatting
- `src/format.rs` — Terminal color utilities
- Discovery shortcuts: `grep 'pub async fn' src/todoist/mod.rs` (API surface), `grep '#\[derive' src/` (trait impls)

Note: `.github/copilot-instructions.md` contains GitHub Copilot-specific guidance. AGENTS.md takes precedence when they conflict.

## GitHub conventions

- Issue templates live in `.github/ISSUE_TEMPLATE/` — use `feature_request.md` for features, `bug_report.md` for bugs
- Labels: `feature` (new capabilities), `improvement` (changes to existing code), `bug` (something broken), `doc`, `dev-test`, `discuss`
- Create issues with: `gh issue create --title "..." --label "feature" --body "..." --repo tod-org/tod`

## Error handling

- All errors use the `Error` type (`src/errors.rs`)
- Use `Error::new(source, message)` for ad-hoc errors where `source` is a lowercase module or crate name (e.g. `"io"`, `"serde_json"`, `"reqwest"`, `"oneshot"`)
- Use `From` impls for wrapped library errors
- The `Display` impl applies `format::red_string` to messages and `format::yellow_string` to the source. Callers constructing error messages must not pre-apply `format::*_string` coloring — `Display` owns color output.

## Commands

- Run `./scripts/test.sh` as the single verification command — it runs format, check, clippy, tests, and forgotten-strings grep in one pass. Do not run `cargo test` separately before it.

## Commits and PRs

- Never push directly to `main` — all changes go through pull requests and are merged into `main`
- Branch naming: `type/short-description` (e.g. `fix/error-coloring`, `feat/add-foo`)
- Create PRs with `gh pr create --title "type: description" --body "..." --base main`
- Commit format follows Conventional Commits (lowercase, 250-char line limit). See `.commitlint.config.mjs` for enforced rules.
