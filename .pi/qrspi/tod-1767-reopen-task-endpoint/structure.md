# Structure Outline

## Approach

Mirror `complete_task` (`POST /tasks/{id}/close`) with `reopen_task` (`POST /tasks/{id}/reopen`),
omitting the config-mutation side effects. Every layer — HTTP, API client, CLI handler, dispatch,
docs — follows the exact same patterns as `Complete`.

---

## Phase 1: API Client

Add `reopen_task()` to `src/todoist/mod.rs` with a unit test. This is independently testable via
`cargo test` — it mocks the HTTP 204 response and verifies the function returns `Ok("✓")`.

**Files**: `src/todoist/mod.rs`

**Key changes**:
- `pub async fn reopen_task(config: &Config, task_id: &str, spinner: bool) -> Result<String, Error>`
  — placed after `complete_task` (line ~645), before `delete_task`. POSTs `Value::Null` to
  `{TASKS_URL}{task_id}/reopen`, no side effects, returns `Ok("✓".into())`.

**Test** (in `#[cfg(test)]` block):
- `test_reopen_task` — placed after `test_complete_task`, before `test_move_task_to_project`.
  Mock returns 204 No Content, no body, asserts with `mock.assert_async().await` (following the
  canonical 204 pattern from `test_archive_project`).

**Verify**:
```bash
cargo test reopen_task
```
Check: test passes, mock asserts the correct URL path `/api/v1/tasks/{id}/reopen` was POSTed.

---

## Phase 2: CLI Surface

Wire the new API function into the CLI — struct, enum variant, handler, and dispatch. After this
phase, `tod task reopen` is a working end-to-end command.

**Files**: `src/commands/task_commands.rs`, `src/commands/mod.rs`

**Key changes**:

*`src/commands/task_commands.rs`*:
- `pub struct Reopen {}` — unit struct, `#[derive(Parser, Debug, Clone)]`, placed after `Complete`
  struct (before `Comment`).
- `TaskCommands::Reopen(Reopen)` variant — `#[clap(alias = "r")]`, placed after `Complete` variant
  (before `Comment`).
- `pub async fn reopen(config: Config, _args: &Reopen, json: bool) -> Result<String, Error>`
  — placed after `complete()` (line ~327), before `comment()`. Matches `complete()` exactly:
  reads `config.next_task()`, calls `todoist::reopen_task(&config, &task.id, true).await?`,
  returns `format::green_string("Task reopened successfully")` or JSON-serialized task.

*`src/commands/mod.rs`*:
- `TaskCommands::Reopen(args)` match arm in `task_command()` — placed after `Complete` arm,
  before `Comment` arm. Pattern: `let result = task_commands::reopen(config.clone(), args, cli.json).await;`
  wrapped in `build_command_result(result, &config)`.

**Verify**:
```bash
scripts/test.sh
```
Manual check (requires Todoist API token):
```bash
tod task next          # get a task
tod task complete      # complete it
tod task reopen        # reopen it — should print "Task reopened successfully"
tod task reopen -j     # should print the reopened task as JSON
tod task reopen        # no next task — should error with "There is nothing to reopen."
```

---

## Phase 3: Documentation

Update `docs/usage.md` to document the new subcommand and alias.

**Files**: `docs/usage.md`

**Key changes**:
- Add `reopen     (r) Reopen the last completed task` to the task subcommands listing
  (after `complete`, before existing commands or `help`)
- Add usage example `# Reopen the last completed task` / `tod task reopen` after the
  existing `tod task complete` example

**Verify**:
```bash
grep "reopen" docs/usage.md
```
Check: help listing line and usage example both present.

---

## Testing Checkpoints

| After Phase | What should be true |
|-------------|---------------------|
| 1 | `cargo test reopen_task` passes; `cargo check` compiles (unused function warning expected) |
| 2 | `scripts/test.sh` passes fully; `tod task reopen` works end-to-end with a live token |
| 3 | `docs/usage.md` lists `reopen` with alias `(r)` and has a usage example |
