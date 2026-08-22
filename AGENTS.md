# Project Instructions

## Code standards

- Use the existing `Error` type (`src/errors.rs`) for error handling
- No `dbg!`, `TODO`, `FIXME`, `DEBUG:`, or `FIXTURE:` strings anywhere in `.rs` files — `scripts/test.sh` greps for these and fails the build

## Project structure

- `src/main.rs` — entry point, `CommandResult` struct, `output_result()` output gateway
- `src/errors.rs` — `Error` type (`{message, source}`) with `Serialize` derive for JSON output
- `src/format.rs` — Terminal color utilities
- `src/input.rs` — inquire-based terminal prompts with mock support. Guards for JSON/non-interactive modes belong at the `fetch_*` call sites in `src/commands/mod.rs`, not inside `input.rs`.
- `docs/usage.md` — User-facing command examples; keep in sync with CLI changes

### Todoist API
- `src/todoist/` — REST API client (`mod.rs`) and HTTP layer (`request.rs`). Base URL: `https://api.todoist.com` with `/api/v1/` prefix (see `TODOIST_URL` in `request.rs`).

### Business logic
- `src/projects.rs` — `Project` struct, CRUD operations, task scheduling
- `src/lists.rs` — Multi-task operations (view, process, label, timebox, etc.)
- `src/tasks/mod.rs` — `Task` struct + formatting

### Config
- `src/config/` — `Config` struct + serialization (`mod.rs`), file I/O (`file.rs`), project CRUD (`projects.rs`)

### CLI dispatch
- `src/commands/` — Dispatch via clap `Subcommand` enum (`mod.rs`) and command handlers for auth, config, list, project, reminder, section, shell, task, test

## Pre-commit checklist

Run `scripts/test.sh` before committing — it covers `cargo fmt --check`, `cargo check`, `cargo clippy`, `cargo test`, and forbidden-string grep.

## QRSPI workflow

Features follow the QRSPI pipeline: `/1_spec` → `/2_clarify` → `/3_design`
→ `/4_research` → `/5_plan` → `/6_implement` (see `~/AGENTS.md`).
Artifacts live in `.pi/qrspi/<issue-id>/`. Each phase gate requires
explicit user approval before proceeding to the next.

## Project skills

| Skill | When to use |
|-------|-------------|
| `architecture` | Planning features that touch output, serialization, or cross-cutting concerns |
| `docs-sync` | After implementing a feature that changes the CLI surface area |
| `testing` | Writing unit/integration tests, mocking Todoist API calls, debugging test failures |

## GitHub conventions

- Use `gh issue view <N>` or `gh pr view <N>` to fetch issue/PR content. Never use web_search for repo issues.
- Issue templates live in `.github/ISSUE_TEMPLATE/` — `feature_request.md` for features, `bug_report.md` for bugs

## Docs

- When adding or changing CLI commands, check `docs/usage.md` for example output that references the changed subcommands and keep it up to date.

## Error handling

- All errors use the `Error` type (`src/errors.rs`)
- Use `Error::new(source, message)` for ad-hoc errors where `source` is a lowercase function or feature name (e.g. `"json_mode"`, `"fetch_project"`, `"config_check"`)
- Use `From` impls for wrapped library errors (e.g. `"io"`, `"serde_json"`, `"reqwest"`, `"oneshot"`)
- The `Display` impl applies `format::red_string` to messages and `format::yellow_string` to the source. Callers constructing error messages must not pre-apply `format::*_string` coloring — `Display` owns color output.

## Commits and PRs

- Branch naming: `type/short-description`.
- Commit format follows Conventional Commits (enforced by `.commitlint.config.mjs`).
- Create PRs with `gh pr create --title "type: description" --body "..." --base main`.
- To fix multiple non-conforming commits: `GIT_SEQUENCE_EDITOR` interactive rebase with `exec git commit --amend` lines, then `git push --force-with-lease`.
