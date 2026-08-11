# Design: Reopen Task Endpoint

## Overview

Add `POST /tasks/{id}/reopen` (Todoist REST API — returns 204 No Content) to `src/todoist/mod.rs`,
expose it via `tod task reopen` (alias `r`), and follow every existing pattern from `complete_task`
(`POST /tasks/{id}/close`).

---

## Files Changed

| File | What Changes |
|------|-------------|
| `src/todoist/mod.rs` | Add `reopen_task()` function + unit test |
| `src/commands/task_commands.rs` | Add `Reopen` struct, `TaskCommands::Reopen` variant, `reopen()` handler |
| `src/commands/mod.rs` | Add match arm in `task_command()` |
| `docs/usage.md` | Add `reopen` to task subcommands listing + usage example |

---

## 1. API Client: `reopen_task()` (`src/todoist/mod.rs`)

### Placement

Immediately after `complete_task()` (line ~644), before `delete_task()`.

### Signature

```rust
pub async fn reopen_task(config: &Config, task_id: &str, spinner: bool) -> Result<String, Error> {
```

### Body

```rust
let url = format!("{TASKS_URL}{task_id}/reopen");
request::post_todoist(config, &url, Value::Null, spinner).await?;
Ok("✓".into())
```

### Key decisions vs `complete_task`

| Aspect | `complete_task` | `reopen_task` |
|--------|----------------|---------------|
| URL suffix | `/close` | `/reopen` |
| POST body | `Value::Null` | `Value::Null` (same) |
| Side effects | Shell hook + reload/clear/save chain (guarded by `!cfg!(test)`) | **None** — no config mutation |
| Return | `Ok("✓".into())` | `Ok("✓".into())` (same) |

### Why no side effects?

`complete_task` has side effects because completing a task means the user's "next task" is now done — the
config must clear it so they can fetch a new one. Reopening a task doesn't change what's "next" — the user
reopens a previously completed task, which lives in the Todoist backend, not in the local config.

---

## 2. CLI Struct: `Reopen` (`src/commands/task_commands.rs`)

### Enum variant

In `TaskCommands` enum (after `Complete`/`Comment`):

```rust
#[clap(alias = "r")]
/// (r) Reopen the last completed task
Reopen(Reopen),
```

### Struct

After `Comment` struct:

```rust
#[derive(Parser, Debug, Clone)]
/// Reopens the last completed task.
pub struct Reopen {}
```

Same unit-struct pattern as `Complete` — no CLI flags, always operates on `config.next_task()`.

---

## 3. Command Handler: `reopen()` (`src/commands/task_commands.rs`)

### Placement

After `complete()` (line ~327), before `comment()`.

### Body

```rust
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

This is a direct mirror of `complete()` (`src/commands/task_commands.rs:310-327`):
- Reads `config.next_task()`
- Calls the API function with `spinner: true`
- Returns green string in interactive mode, JSON-serialized task in JSON mode
- Error source tag: `"task_reopen"` (parallels `"task_complete"`)

---

## 4. Dispatch (`src/commands/mod.rs`)

### Match arm in `task_command()`

After the `TaskCommands::Complete` arm, before `TaskCommands::Comment`:

```rust
TaskCommands::Reopen(args) => {
    let config = fetch_config(cli, tx).await?;
    let result = task_commands::reopen(config.clone(), args, cli.json).await;
    Ok(build_command_result(result, &config))
},
```

Follows the same pattern as `Complete` and `Comment`: `config.clone()` is passed (because the handler
takes ownership of `Config` to read `next_task()`).

---

## 5. Test (`src/todoist/mod.rs` test module)

### Placement

After `test_complete_task` (line ~1162), before `test_move_task_to_project`.

### Test

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

### Key test decisions

| Aspect | `test_complete_task` (existing) | `test_reopen_task` (new) |
|--------|-------------------------------|-------------------------|
| HTTP status | 200 | **204** (matches real API) |
| Has body | Yes (TodayTask JSON) | **No body** (canonical 204) |
| Assert method | `mock.assert()` | `mock.assert_async().await` (canonical async pattern) |
| Spinner arg | `false` | `false` (test, not interactive) |

The existing `test_complete_task` uses 200 + body out of convention/habit, not correctness. The new test
should follow `test_archive_project` as the canonical 204 pattern (research Q6).

---

## 6. Documentation (`docs/usage.md`)

### Help output block update

The `tod task` subcommands listing currently reads:

```
Commands:
  quick-add  (q) Create a new task using NLP
  create     (c) Create a new task (without NLP)
  edit       (e) Edit an existing task's content
  next       (n) Get the next task by priority
  complete   (o) Complete the last task fetched with the next command
  help       Print this message or the help of the given subcommand(s)
```

Add `reopen`:

```
Commands:
  quick-add  (q) Create a new task using NLP
  create     (c) Create a new task (without NLP)
  edit       (e) Edit an existing task's content
  next       (n) Get the next task by priority
  complete   (o) Complete the last task fetched with the next command
  reopen     (r) Reopen the last completed task
  comment    (m) Add a comment to the last task fetched with the next command
  help       Print this message or the help of the given subcommand(s)
```

### Usage example

After the existing `tod task complete` example:

```
# Reopen the last completed task
tod task reopen
```

---

## Implementation Order

1. `src/todoist/mod.rs` — add `reopen_task()` function
2. `src/todoist/mod.rs` — add `test_reopen_task` test
3. `src/commands/task_commands.rs` — add `Reopen` struct + enum variant
4. `src/commands/task_commands.rs` — add `reopen()` handler
5. `src/commands/mod.rs` — add dispatch arm
6. `docs/usage.md` — update help output + add example
7. Run `scripts/test.sh` to verify
