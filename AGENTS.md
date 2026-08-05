# Project Instructions

## Code standards

- Use the existing `Error` type (`src/errors.rs`) for error handling
- No `dbg!`, `TODO`, `FIXME`, `DEBUG:`, or `FIXTURE:` strings anywhere in `.rs` files — `scripts/test.sh` greps for these and fails the build
- New business logic should have tests covering both happy and sad paths

## Project structure

- `src/main.rs` — entry point, `CommandResult` struct, `output_result()` output gateway
- `src/errors.rs` — `Error` type (`{message, source}`) with `Serialize` derive for JSON output
- `src/format.rs` — Terminal color utilities
- `src/input.rs` — inquire-based terminal prompts with mock support (`config.mock_string`, `config.mock_select`). Guards for JSON/non-interactive modes belong at the `fetch_*` call sites in `src/commands/mod.rs`, not inside `input.rs`.

### Todoist API
- `src/todoist/mod.rs` — REST API client. Key exports: `all_tasks_by_*`, `quick_create_task`, `create_task`, `complete_task`, `update_task_*`, `all_projects`, `all_labels`, `all_comments`, `all_sections_by_project`
- `src/todoist/request.rs` — HTTP layer (GET/POST/DELETE via reqwest)

### Business logic
- `src/projects.rs` — `Project` struct, CRUD operations, task scheduling
- `src/lists.rs` — Business-logic functions for multi-task operations (view, process, label, timebox, etc.). The clap arg structs live in `src/commands/list_commands.rs`.
- `src/tasks/mod.rs` — `Task` struct + formatting

### Config
- `src/config/mod.rs` — `Config` struct + serialization (`#[serde(deny_unknown_fields)]`)
- `src/config/file.rs` — File I/O: `Config::load()`, `save()`, `create()`, `reload()`, `touch_file()`
- `src/config/projects.rs` — Project CRUD on the config's `projectsv1` field

### CLI dispatch
- `src/commands/mod.rs` — Dispatch via clap `Subcommand` enum. Defines the `Cli` struct (including `--json`/`-j` flag) and `fetch_*` helpers: `fetch_string`, `fetch_project`, `fetch_filter`, `fetch_priority`, `fetch_project_or_filter`, `maybe_fetch_labels`.
- `src/commands/task_commands.rs` — Task subcommand handlers
- `src/commands/list_commands.rs` — List/view subcommand handlers
- `src/commands/project_commands.rs` — Project subcommand handlers
- `src/commands/config_commands.rs` — Config subcommand handlers
- `src/commands/auth_commands.rs` — Auth subcommand handlers
- `src/commands/reminder_commands.rs` — Reminder subcommand handlers
- `src/commands/section_commands.rs` — Section subcommand handlers
- `src/commands/shell_commands.rs` — Shell completion handler
- `src/commands/test_commands.rs` — Manual API test handler

## GitHub conventions

- Fetch issue/PR content with `gh issue view <N>` or `gh pr view <N>`. Use `--json` for labels and comments. Do not use web_search for repo issues — GitHub issue pages are not indexed for web search.
- Issue templates live in `.github/ISSUE_TEMPLATE/` — use `feature_request.md` for features, `bug_report.md` for bugs

## Error handling

- All errors use the `Error` type (`src/errors.rs`)
- Use `Error::new(source, message)` for ad-hoc errors where `source` is a lowercase function or feature name (e.g. `"json_mode"`, `"fetch_project"`, `"config_check"`)
- Use `From` impls for wrapped library errors (e.g. `"io"`, `"serde_json"`, `"reqwest"`, `"oneshot"`)
- The `Display` impl applies `format::red_string` to messages and `format::yellow_string` to the source. Callers constructing error messages must not pre-apply `format::*_string` coloring — `Display` owns color output.

## Commits and PRs

- Never push directly to `main` — all changes go through pull requests and are merged into `main`
- Branch naming: `type/short-description` (e.g. `fix/error-coloring`, `feat/add-foo`)
- Push the branch to origin before creating a PR: `git push -u origin <branch>`
- Create PRs with `gh pr create --title "type: description" --body "..." --base main`
- Commit format follows Conventional Commits (lowercase, 250-char line limit). See `.commitlint.config.mjs` for enforced rules.
