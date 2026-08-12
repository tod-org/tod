# Implementation Plan

## Overview

Add `POST /tasks/{id}/reopen` (Todoist REST API — returns 204 No Content) to the API client, expose it via `tod task reopen` (alias `r`), mirroring the exact patterns from `complete_task` (`POST /tasks/{id}/close`) with no config-mutation side effects.

---

## Phase 1: API Client

### Changes

#### 1. Add `reopen_task()` function
**File**: `src/todoist/mod.rs`
**Action**: modify — insert new function after `complete_task` (after line 645, before the blank line preceding `/// Deletes a task by ID.` on line 646)

```rust
/// Reopens a completed task by its ID. Does not return a new task (the API yields no data).
pub async fn reopen_task(config: &Config, task_id: &str, spinner: bool) -> Result<String, Error> {
    let url = format!("{TASKS_URL}{task_id}/reopen");

    request::post_todoist(config, &url, Value::Null, spinner).await?;
    // No side effects — reopening doesn't change what's "next"
    // API does not pass back a task
    Ok("✓".into())
}
```

Placement: between the closing `}` of `complete_task` (line ~645) and the `/// Deletes a task by ID.` doc comment of `delete_task` (line ~647). One blank line before and after the new function.

#### 2. Add `test_reopen_task` test
**File**: `src/todoist/mod.rs`
**Action**: modify — insert new test after `test_complete_task` (the `}` on line ~1160), before `test_move_task_to_project` (the `#[tokio::test]` on line ~1162)

```rust
    #[tokio::test]
    async fn test_reopen_task() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/api/v1/tasks/6Xqhv4cwxgjwG9w8/reopen")
            .with_status(204)
            .create_async()
            .await;

        let config = test::fixtures::config().await.with_mock_url(server.url());
        let task = test::fixtures::today_task().await;
        let response = reopen_task(&config, &task.id, false)
            .await
            .expect("Did not reopen task");
        mock.assert_async().await;
        assert_eq!(response, String::from("✓"));
    }
```

Key: follows the canonical 204 pattern from `test_archive_project_hits_api` — `.with_status(204)`, no `.with_body()`, `mock.assert_async().await`.

### Verification

#### Automated
- [x] `cargo test reopen_task` passes — test mocks 204, asserts `Ok("✓")`, verifies URL path
- [x] `cargo check` compiles (unused function warning is expected at this phase)

#### Manual
- None for this phase

---

## Phase 2: CLI Surface

### Changes

#### 1. Add `Reopen` struct
**File**: `src/commands/task_commands.rs`
**Action**: modify — insert after `Complete` struct (line ~96-98), before `Comment` struct (line ~100-107)

```rust
#[derive(Parser, Debug, Clone)]
/// Reopens the last completed task.
pub struct Reopen {}
```

#### 2. Add `Reopen` variant to `TaskCommands` enum
**File**: `src/commands/task_commands.rs`
**Action**: modify — insert in enum after `Complete` variant (line ~33-35), before `Comment` variant (line ~37-39)

```rust
    #[clap(alias = "r")]
    /// (r) Reopen the last completed task
    Reopen(Reopen),
```

#### 3. Add `reopen()` handler function
**File**: `src/commands/task_commands.rs`
**Action**: modify — insert after `complete()` function (after the `}` on line ~215), before `comment()` function (the `/// Adds a comment...` line ~218)

```rust
/// Reopens the stored next task.
pub async fn reopen(config: Config, _args: &Reopen, json: bool) -> Result<String, Error> {
    match config.next_task() {
        Some(task) => {
            todoist::reopen_task(&config, &task.id, true).await?;

            if json {
                Ok(serde_json::to_string(&task)?)
            } else {
                Ok(format::green_string("Task reopened successfully"))
            }
        }
        None => Err(Error::new(
            "task_reopen",
            "There is nothing to reopen. A task must first be marked as 'next'.",
        )),
    }
}
```

#### 4. Add dispatch arm in `task_command()`
**File**: `src/commands/mod.rs`
**Action**: modify — insert in `task_command()` match block after `TaskCommands::Complete` arm (line ~252-256), before `TaskCommands::Comment` arm (line ~257-261)

```rust
        TaskCommands::Reopen(args) => {
            let config = fetch_config(cli, tx).await?;
            let result = task_commands::reopen(config.clone(), args, cli.json).await;
            Ok(build_command_result(result, &config))
        }
```

### Verification

#### Automated
- [x] `scripts/test.sh` passes fully (fmt, check, clippy, test, forbidden-string grep)

#### Manual
- [x] `tod task next` — fetches a task
- [x] `tod task complete` — completes it
- [x] `tod task reopen` — prints "Task reopened successfully" (green)
- [x] `tod task reopen -j` — prints the reopened task as JSON
- [x] `tod task reopen` (with no completed task) — errors with "There is nothing to reopen."
- [x] `tod task reopen -h` — shows `(r) Reopen the last completed task`
- [x] `tod t r` — alias works

---

## Phase 3: Documentation

### Changes

#### 1. Update task subcommands listing
**File**: `docs/usage.md`
**Action**: modify — in the `tod task -h` output block (~line 57-70), insert `reopen` line after `complete`, before `help`

**Find** (the existing block lines):
```
  complete   (o) Complete the last task fetched with the next command
  help       Print this message or the help of the given subcommand(s)
```

**Replace with**:
```
  complete   (o) Complete the last task fetched with the next command
  reopen     (r) Reopen the last completed task
  help       Print this message or the help of the given subcommand(s)
```

#### 2. Add usage example
**File**: `docs/usage.md`
**Action**: modify — after the `tod task complete` usage lines (~line 137-138), add a reopen example

**Find**:
```
# Complete the last "next task" and get another
tod task complete && tod task next
```

**Replace with**:
```
# Complete the last "next task" and get another
tod task complete && tod task next

# Reopen the last completed task
tod task reopen
```

### Verification

#### Automated
- [x] `grep "reopen" docs/usage.md` returns both the help listing line and the usage example

#### Manual
- None — doc-only change

---

## Testing Checkpoints

| After Phase | What should be true |
|-------------|---------------------|
| 1 | `cargo test reopen_task` passes; `cargo check` compiles (unused function warning expected) |
| 2 | `scripts/test.sh` passes fully; `tod task reopen` works end-to-end with a live token |
| 3 | `docs/usage.md` lists `reopen` with alias `(r)` and has a usage example |
