# Research Findings

## Q1: Trace the flow of `complete_task` from CLI entry point through to HTTP request

### Layers (top to bottom)

| Layer | File | Function/Span | Key Detail |
|-------|------|---------------|------------|
| Entry | `src/main.rs:117-119` | `run_command()` → `commands::select_command(cli, tx)` | Wraps in `CommandResult` |
| Top dispatch | `src/commands/mod.rs:135-136` | `select_command()` match arm | `Commands::Task(command) => task_command(command, &cli, &tx).await` |
| Task dispatch | `src/commands/mod.rs:253-257` | `task_command()` match arm | `TaskCommands::Complete(args) => task_commands::complete(config.clone(), args, cli.json).await` |
| Command handler | `src/commands/task_commands.rs:202-214` | `complete()` | Fetches `config.next_task()`, calls `todoist::complete_task`, returns `✓` or JSON-serialized task |
| API client | `src/todoist/mod.rs:485-498` | `complete_task()` | Builds URL `{TASKS_URL}{task_id}/close`, POSTs `Value::Null` body, then runs side effects (see Q4) |
| HTTP layer | `src/todoist/request.rs:36-67` | `post_todoist()` | Builds reqwest `Client::new().post(...)` with Bearer auth + `X-Request-Id`; sends `Value::Null` as empty body via `.send()` |
| Response | `src/todoist/request.rs:153-176` | `handle_response()` | On success, reads `response.text()` and returns it as `Result<String, Error>` |

### Return types at each layer

- `task_commands::complete()` → `Result<String, Error>` (`src/commands/task_commands.rs:202`)
- `todoist::complete_task()` → `Result<String, Error>` (`src/todoist/mod.rs:485`)
- `request::post_todoist()` → `Result<String, Error>` (`src/todoist/request.rs:41`)
- `request::handle_response()` → `Result<String, Error>` (`src/todoist/request.rs:158`)

### Structs involved

- **`Complete`**: `src/commands/task_commands.rs:96-98` — unit struct (no fields), derives `Parser`
- **`Config`**: `src/config/mod.rs:56-126` — passed through from dispatch; holds `next_task: Option<Task>`
- **`Task`**: `src/tasks/mod.rs` — the stored next task's ID is used in the API URL

### Key observation

`complete_task` is unique among task commands in that it does not take `--project` or `--filter` — it always operates on the stored next task from `Config::next_task()`.

---

## Q2: How `post_todoist` handles HTTP 204 No Content responses

### Response handling

`src/todoist/request.rs:153-176` — the `handle_response()` function:

```rust
if status.is_success() {
    let json_string = response.text().await?;
    debug::maybe_print(config, &format!("{method} {url}\nresponse: {json_string}"));
    Ok(json_string)
}
```

- HTTP 204 No Content is a success status, so it enters the `is_success()` branch
- `response.text().await?` reads the (empty) response body → returns `Ok("")` (empty `String`)
- There is **no special-casing** for 204 vs 200 — both go through the same `.text()` path
- Callers that don't need the body simply discard it with `?` (e.g., `request::post_todoist(...).await?;`)

### What callers receive

- `post_todoist()` returns `Result<String, Error>` — on 204, it's `Ok("")` (empty string)
- Functions like `complete_task` (`src/todoist/mod.rs:487`), `archive_project` (`src/todoist/mod.rs:553`), `update_task_priority` (`src/todoist/mod.rs:566`) use `request::post_todoist(...).await?;` to check for errors and ignore the body
- Functions like `create_task` (`src/todoist/mod.rs:198`) use `request::post_todoist(...).await?` and then parse the body: `Task::from_json(&json)` — these would panic or error on empty 204 responses, but they target endpoints that return 200 with JSON

---

## Q3: How `archive_project` and `unarchive_project` handle 204 — differences from `complete_task`

### archive_project

`src/todoist/mod.rs:548-557`:
```rust
pub async fn archive_project(config: &Config, project_id: &str, spinner: bool) -> Result<String, Error> {
    let url = format!("{PROJECTS_URL}/{project_id}/archive");
    let body = json!({});
    request::post_todoist(config, &url, body, spinner).await?;
    Ok("✓".into())
}
```

### unarchive_project

`src/todoist/mod.rs:560-569`: Identical pattern to `archive_project` but with `/unarchive` URL.

### complete_task

`src/todoist/mod.rs:485-498`:
```rust
pub async fn complete_task(config: &Config, task_id: &str, spinner: bool) -> Result<String, Error> {
    let url = format!("{TASKS_URL}{task_id}/close");
    request::post_todoist(config, &url, Value::Null, spinner).await?;
    if !cfg!(test) {
        maybe_run_command(config.task_complete_command.as_deref(), config)?;
        config.reload().await?.clear_next_task().save().await?;
    }
    Ok("✓".into())
}
```

### Differences

| Aspect | `archive_project` / `unarchive_project` | `complete_task` |
|--------|----------------------------------------|-----------------|
| POST body | `json!({})` (empty object) | `Value::Null` (null body) |
| Side effects | None | Shell command hook + config reload/clear/save |
| Test guard | None | Side effects guarded by `!cfg!(test)` |
| Config mutation | No | Yes — clears `next_task` via reload→clear→save chain |

Both patterns ignore the HTTP response body (just checking for errors with `?`). The key structural difference is that `complete_task` has post-request side effects gated behind `!cfg!(test)`, while `archive_project`/`unarchive_project` are pure API wrappers.

---

## Q4: Side effects of `complete_task` on config state

All side effects live in `todoist::complete_task()` at `src/todoist/mod.rs:485-498`. They execute **after** the API call succeeds, and are gated by `!cfg!(test)`:

### 1. Shell command hook

`src/todoist/mod.rs:491`:
```rust
maybe_run_command(config.task_complete_command.as_deref(), config)?;
```

- `maybe_run_command()` is defined at `src/todoist/mod.rs:626-638`
- If `config.task_complete_command` is `Some`, it calls `execute_command(command, tx)` from `src/shell.rs:22-30`
- `execute_command()` spawns a background `tokio::spawn` task that runs `sh -c <command>` with stdout/stderr suppressed
- Errors are reported asynchronously via the `UnboundedSender<Error>` channel; the function returns `Ok(())` immediately without waiting
- The shell command is configured via `tod config edit` → `task_complete_command` field (`src/config/mod.rs:82`)

### 2. Config reload → clear → save chain

`src/todoist/mod.rs:492`:
```rust
config.reload().await?.clear_next_task().save().await?;
```

This is a 3-step chained operation:

- **`reload()`**: `src/config/file.rs:63-68` — loads config fresh from disk, preserving `internal` (async error channel) and `time_provider` from the in-memory instance
- **`clear_next_task()`**: `src/config/mod.rs:406-411` — sets `next_task: Option<Task>` to `None`, returning a new `Config` via struct update syntax
- **`save()`**: `src/config/file.rs:34-56` — serializes config as pretty-printed JSON, truncates the file, writes, flushes, and syncs to disk

### What is NOT done

- `increment_completed()` (`src/config/mod.rs:414-435`) is **not** called by `complete_task` — the completed count is not incremented on task completion
- The CLI-layer `task_commands::complete()` (`src/commands/task_commands.rs:202-214`) does **no side effects** — it delegates entirely to `todoist::complete_task`

---

## Q5: TaskCommands enum structure and dispatch

### Enum definition

`src/commands/task_commands.rs:18-41`:
```rust
#[derive(Subcommand, Debug, Clone)]
pub enum TaskCommands {
    QuickAdd(QuickAdd),   // alias = "q"
    Create(Create),       // alias = "c"
    Edit(Edit),           // alias = "e"
    Next(Next),           // alias = "n"
    Complete(Complete),   // alias = "o"
    Comment(Comment),     // alias = "m"
}
```

Each variant wraps a unit-like struct that derives `Parser` (`src/commands/task_commands.rs:44-116`). The structs hold CLI flags as optional fields (e.g., `project: Option<String>`, `filter: Option<String>`, `content: Option<String>`). `Complete` is the only variant with no fields (`pub struct Complete {}`).

### Dispatch

`src/commands/mod.rs:236-265` — the `task_command()` function matches on `TaskCommands`:

```rust
TaskCommands::QuickAdd(args) => task_commands::quick_add(&config, args, cli.json).await,
TaskCommands::Create(args)  => task_commands::create(config.clone(), args, cli.json).await,
TaskCommands::Edit(args)    => task_commands::edit(config.clone(), args, cli.json).await,
TaskCommands::Next(args)    => task_commands::next(config.clone(), args, cli.json).await,
TaskCommands::Complete(args)=> task_commands::complete(config.clone(), args, cli.json).await,
TaskCommands::Comment(args) => task_commands::comment(config.clone(), args, cli.json).await,
```

All arms fetch config via `fetch_config(cli, tx).await?`, wrap result in `build_command_result(result, &config)`, and return `CommandResult`.

### Steps to add a new variant

1. **Define the struct** in `src/commands/task_commands.rs` deriving `Parser` with CLI flags
2. **Add variant** to `TaskCommands` enum with a clap alias
3. **Write handler function** (`pub async fn ...`) in `src/commands/task_commands.rs`
4. **Add match arm** in `task_command()` at `src/commands/mod.rs:236-265`
5. The `Commands::Task(TaskCommands)` arm at `src/commands/mod.rs:136` routes all `TaskCommands` variants through `task_command()`, so the top-level dispatch doesn't need changes

---

## Q6: Mockito test patterns for no-body endpoints

### Example: `test_complete_task`

`src/todoist/mod.rs` (test module):
```rust
let mut server = mockito::Server::new_async().await;
let mock = server
    .mock("POST", "/api/v1/tasks/6Xqhv4cwxgjwG9w8/close")
    .with_status(200)           // ← uses 200, not 204
    .with_header("content-type", "application/json")
    .with_body(ResponseFromFile::TodayTask.read().await)  // ← includes a body
    .create_async().await;
// ...
let response = complete_task(&config, &task.id, false).await.expect("Did not complete task");
mock.assert();
assert_eq!(response, String::from("✓"));
```

### Example: `test_delete_task`

Same pattern as `complete_task` — mocks DELETE with `.with_status(200)` and a body, asserts `Ok("✓")`, calls `mock.assert()`.

### Example: `test_archive_project` (canonical 204 pattern)

`src/todoist/mod.rs`:
```rust
let mock = server
    .mock("POST", "/api/v1/projects/123/archive")
    .with_status(204)            // ← correctly uses 204
    // no .with_body()           // ← no body for 204
    .create_async().await;
// ...
let result = archive_project(&config, "123", false).await;
assert_eq!(result, Ok("✓".to_string()));
mock.assert_async().await;       // ← uses assert_async()
```

### Example: `test_unarchive_project`

Identical to `archive_project` — `.with_status(204)`, no body, `.assert_async().await`.

### Observed patterns

| Test | Status Used | Has Body | Assert Method |
|------|-------------|----------|---------------|
| `test_complete_task` | 200 | Yes (TodayTask JSON) | `mock.assert()` |
| `test_delete_task` | 200 | Yes (TodayTask JSON) | `mock.assert()` |
| `test_delete_section` | 200 | Yes (`"null"`) | `mock.assert()` |
| `test_update_task_priority` | 204 | Yes (TodayTask JSON) | `mock.assert()` |
| `test_archive_project` | 204 | No | `mock.assert_async().await` |
| `test_unarchive_project` | 204 | No | `mock.assert_async().await` |

### Inconsistency noted

`test_complete_task` and `test_delete_task` use HTTP 200 with a body, despite the real Todoist API returning 204 No Content for these endpoints. `test_archive_project` is the only test that correctly models 204 with no body. The `assert()` vs `assert_async()` split reflects that `test_archive_project` was written more recently using mockito's async API.

### Common pattern for no-body endpoint tests

1. `let mut server = mockito::Server::new_async().await;`
2. `server.mock("METHOD", "/path").with_status(204).create_async().await;`
3. `let config = test::fixtures::config().await.with_mock_url(server.url());`
4. Call function under test with `false` for spinner
5. `assert_eq!(result, Ok("✓".to_string()));`
6. `mock.assert_async().await;` (or `mock.assert();` for prepopulated mocks)

---

## Cross-Cutting Observations

- **No dedicated 204 handling**: `handle_response()` (`src/todoist/request.rs:153`) treats all success status codes identically — it reads `.text()` regardless of whether a body is expected. 204 just yields an empty string.

- **Side effect test gating**: `complete_task` uses `!cfg!(test)` to skip post-API side effects during tests (`src/todoist/mod.rs:490`). `archive_project` and `unarchive_project` have no side effects, so they need no such guard.

- **Two styles of mockito tests coexist**: older tests use `.create().await` + `mock.assert()` (synchronous), newer ones use `.create_async().await` + `mock.assert_async().await` (async). The `test_archive_project` / `test_unarchive_project` tests are the canonical 204 pattern.

- **Config mutation pattern**: The `reload() → clear_next_task() → save()` chain in `complete_task` follows a functional config update pattern — each step returns a new `Config`, avoiding mutation of the passed reference. `Config` methods like `clear_next_task()` and `set_next_task()` use struct update syntax `Config { field, ..self }`.

- **`Value::Null` body**: `complete_task` uses `Value::Null` for the POST body (`src/todoist/mod.rs:487`), while `archive_project`/`unarchive_project` use `json!({})`. Both are handled correctly by `post_todoist` (`src/todoist/request.rs:60-63`) which checks `Value::Null` and calls `.send()` without attaching JSON.

## Open Areas

- The test for `complete_task` (`src/todoist/mod.rs`) does not test the `!cfg!(test)` side-effect path (shell command hook + config chain). This path is only exercised in integration/e2e runs.
- The `test_complete_task` mock uses HTTP 200 with a body, which doesn't match the real Todoist API's 204 response. The test happens to work because `complete_task` ignores the response body entirely.
