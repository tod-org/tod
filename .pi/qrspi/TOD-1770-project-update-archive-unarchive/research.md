# Research Findings

## Q1: How do existing task update functions structure their POST requests and handle responses, and how does `create_project` differ by deserializing a full `Project` response instead of returning `"✓"`?

### Findings

- All four task update functions (`update_task_priority` at `src/todoist/mod.rs:491`, `update_task_content` at `:553`, `update_task_deadline` at `:568`, `update_task_labels` at `:610`) follow an identical pattern:
  1. Build a `serde_json::Value` body via `json!({ "field": value })`
  2. Format the URL as `format!("{TASKS_URL}{task_id}")`
  3. Call `request::post_todoist(config, &url, body, spinner).await?` — note the `;` that discards the returned `String`
  4. Return `Ok("✓".into())` — a hardcoded success string

- `create_project` (`src/todoist/mod.rs:662-674`) differs only in post-request handling: it binds the response to `let json = ...` and calls `Project::from_json(&json)` to deserialize the API response into a `Project` struct. The function returns `Result<Project, Error>` instead of `Result<String, Error>`.

- Both patterns use the exact same `request::post_todoist` (`src/todoist/request.rs:33`) — there is no difference in how the POST request is structured or sent. The divergence is exclusively in what happens with the response string.

- `create_task` (`src/todoist/mod.rs`) follows the same deserialization pattern as `create_project`: bind response, call `Task::from_json(&json)`. Same for `move_task_to_project`, `move_task_to_section`, `create_section`, and `create_comment`.

- 10 functions in `src/todoist/mod.rs` discard the response and return `"✓"`: the four task updates above, plus `add_task_label`, `update_task_due_natural_language`, `update_task_description`, `complete_task`, `delete_task`, and `delete_project`.

- The CLI callers of functions that return `"✓"` don't inspect the value — e.g., `src/commands/list_commands.rs` calls `update_task_priority` in `future::join_all(handles).await` and drops the `Result<String, Error>`.

### Response handling summary

| Function | Line (`mod.rs`) | Returns | Handles response |
|---|---|---|---|
| `update_task_priority` | 491 | `Result<String, Error>` | Discards, returns `"✓"` |
| `create_project` | 662 | `Result<Project, Error>` | `Project::from_json(&json)` |
| `delete_project` | 650 | `Result<String, Error>` | Discards, returns `"✓"` |

---

## Q2: What writable fields does the Todoist REST API accept for `POST /projects/{id}`, how do those map to the existing `Project` struct fields, and does the `Project` struct currently derive both `Serialize` and `Deserialize`?

### Findings

- The codebase uses the v1 API (`PROJECTS_URL = "/api/v1/projects"` at `src/todoist/mod.rs:40`). The v1 `POST /api/v1/projects/{id}` accepts these writable fields:

| API field | Type | `Project` struct field | `src/projects.rs` line |
|---|---|---|---|
| `name` | string | `name` | 29 |
| `description` | string | `description` | 36 |
| `color` | string/numeric | `color` | 23 |
| `is_favorite` | boolean | `is_favorite` | 28 |
| `view_style` | string | `view_style` | 34 |
| `child_order` | integer | `child_order` | 22 |
| `is_collapsed` | boolean | `is_collapsed` | 42 |

- All 7 writable API fields map directly to existing `Project` struct fields.

- **`Project` derives both `Serialize` and `Deserialize`** at `src/projects.rs:18`:
  ```rust
  #[derive(PartialEq, Eq, Serialize, Deserialize, Clone, Debug)]
  ```

- Fields on `Project` that are **not writable** via `POST /projects/{id}`: `id`, `can_assign_tasks`, `created_at`, `updated_at`, `is_archived`, `is_deleted`, `is_frozen`, `default_order`, `parent_id`, `inbox_project`, `is_shared`.

- `create_project` (`src/todoist/mod.rs:662`) sends only 3 fields in its body: `name`, `description`, `is_favorite`. No other project mutation function exists in the API layer.

- **No `update_project` function exists** in `src/todoist/mod.rs`. The codebase can create (`:662`), list (`all_projects`), and delete (`:650`) projects via the API, but cannot update them. The existing `rename` (`src/projects.rs:173`) is local-only (config, not API).

---

## Q3: How does `handle_response` in `src/todoist/request.rs` handle 204 No Content or empty-body responses, and how do callers like `delete_project`, `update_task_priority`, and `complete_task` handle responses that carry no meaningful JSON body?

### Findings

- `handle_response` (`src/todoist/request.rs:164-190`) has **no special-case logic for HTTP 204**. On any 2xx status (`status.is_success()`, line 169), it unconditionally calls `response.text().await?` (`:170`) and returns `Ok(json_string)`. For a true 204 with an empty body, this returns `Ok("")`.

- The 10 callers that discard the response body are unaffected by this — they only check `?` for error propagation and then return `"✓"`. Key examples:
  - `update_task_priority` (`src/todoist/mod.rs:491`) — `request::post_todoist(...).await?; Ok("✓".into())`
  - `delete_project` (`src/todoist/mod.rs:650`) — `request::delete_todoist(...).await?; Ok("✓".into())`
  - `complete_task` (`src/todoist/mod.rs:625`) — `request::post_todoist(...).await?; Ok("✓".into())`

- `complete_task` (`:625`) is notable as the only caller that passes `Value::Null` as the body. `post_todoist` (`request.rs:58-61`) branches on `Value::Null` to send `client.send().await?` without a JSON body.

- **If a caller that parses the response** (e.g., `create_project`, `move_task_to_project`) received an empty body, `serde_json::from_str("")` would fail with a parse error — but this scenario doesn't occur because those endpoints return full JSON objects.

- Tests for task updates (`src/todoist/mod.rs`, around `test_update_task_priority` at `:990`) stub the mock server with status 204 plus a body from a fixture file. The body is present from the mock but the actual Todoist API returns true 204s for these endpoints. The tests assert `Ok(String::from("✓"))` regardless.

### Response flow

```
caller → post_todoist/delete_todoist → handle_response
  → status.is_success()? → response.text().await? → Ok(String)
  → caller either: (a) discards String and returns "✓", or
                    (b) passes String to Type::from_json(&json)
```

---

## Q4: What is the complete pattern for adding a new subcommand to `ProjectCommands`?

### Findings

Four locations must be touched, all in `src/commands/`:

**1. Struct definition** — `src/commands/project_commands.rs` (e.g., `Empty` at `:118-124`):
```rust
#[derive(Parser, Debug, Clone)]
pub struct Empty {
    #[arg(short, long)]
    /// Project to empty
    project: Option<String>,
}
```
Convention: field args are `Option<...>` when an interactive prompt fallback is supported.

**2. Enum variant** — `src/commands/project_commands.rs:8-40` (`ProjectCommands` enum):
```rust
#[clap(alias = "e")]
/// (e) Empty a project by putting tasks in other projects
Empty(Empty),
```
Convention: single-letter alias matching the doc comment's parenthetical.

**3. Match arm** — `src/commands/mod.rs:192-230` (`project_command()` function):
```rust
ProjectCommands::Empty(args) => {
    let mut config = fetch_config(cli, tx).await?;
    let result = project_commands::empty(&mut config, args).await;
    Ok(build_command_result(result, &config))
}
```
All arms use `let mut config = fetch_config(cli, tx).await?`, pass `&mut config`, and wrap results with `build_command_result`. Some arms pass `cli.json` as a third argument; others don't.

**4. Handler function** — `src/commands/project_commands.rs` (e.g., `empty` at `:213`):
```rust
pub async fn empty(config: &mut Config, args: &Empty) -> Result<String, Error> { ... }
```
Two signature shapes exist:
- Without `json`: `fn foo(config: &mut Config, args: &Foo) -> Result<String, Error>` (4 of 7 handlers: `remove`, `rename`, `empty`, `delete`)
- With `json`: `fn foo(config: &mut Config, args: &Foo, json: bool) -> Result<String, Error>` (3 of 7: `create`, `list`, `import`)

---

## Q5: How are project API operations tested?

### Findings

**Mockito infrastructure:**
- `mockito = "1.7.2"` is the test dependency
- `get_base_url()` (`src/todoist/request.rs:278-281`) returns `config.mock_url` during `cfg!(test)`, transparently routing all HTTP to the mock server
- `Config::with_mock_url(url)` (`src/config/mod.rs:801`), `Config::mock_select(index)` (`:814`), `Config::with_mock_string(str)` (`:807`), and `Config::default_test()` (`:761`) support test configuration

**Standard mock pattern** (used in all project tests):
```rust
let mut server = mockito::Server::new_async().await;
let mock = server.mock("METHOD", "/api/v1/path")
    .with_status(200)
    .with_header("content-type", "application/json")
    .with_body(ResponseFromFile::Projects.read().await)
    .create_async().await;
let config = test::fixtures::config().await.with_mock_url(server.url());
// ... exercise code ...
mock.assert();  // or mock.assert_async().await;
```

**Project-related JSON fixtures** (`tests/responses/`):
| File | Purpose | Key detail |
|---|---|---|
| `Project.json` | Single project | `id: "123"`, `name: "Doomsday"`, `parent_id: "5678"` |
| `Projects.json` | Project list (existing) | `id: "123"` — matches the pre-seeded fixture project in config |
| `NewProjects.json` | Project list (new ID) | `id: "890"` — deliberately different, tests "new project not in config" import path |

Fixture loading via `ResponseFromFile` enum (`src/test/responses.rs:8-46`). Project variants have no value replacements (no `INSERTDATE`).

**Unit tests for deserialization** (`src/projects.rs`):
- `test_project_from_json_valid` (`:1366`) — deserializes `Project.json`, asserts `name == "Doomsday"`
- `test_project_from_json_invalid` (`:1373`) — `Project::from_json("not json")` returns `Err`
- `test_project_response_from_json_valid` (`:1379`) — deserializes `Projects.json` paginated response
- Negative order tests (`:1392`, `:1414`) — inline JSON, asserts negative `child_order` / `default_order` preserved

**Integration tests for project API operations** (`src/projects.rs`):
- `test_list` (`:690`) — mocks `GET /projects?limit=200`, asserts formatted project list string
- `test_import*` (`:774-892`) — mocks `GET /projects?limit=200` with `NewProjects.json`, tests import by name, by ID, and not-found errors
- `test_remove_auto` (`:921`) — verifies config cleanup when API no longer returns a tracked project
- `test_project_delete` (`:1179`) — mocks `DELETE /projects/123`, asserts `"✓"` and mock was called

**Integration tests for project commands** (`src/commands/project_commands.rs:273-406`):
- `delete_force_skips_confirmation_prompt_for_non_empty_project` (`:273`)
- `delete_cancels_when_user_selects_cancel_for_non_empty_project` (`:315`)
- `delete_confirms_and_removes_project_when_user_selects_delete` (`:349`)
- `rename_uses_name_flag_without_prompt` (`:406`) — no HTTP mock needed (local-only rename)

---

## Q6: How does `projects::create` bridge between calling the API and updating local config, and what does `projects::rename` reveal about local-only vs. API-backed operations?

### Findings

**`projects::create` — the bridge** (`src/projects.rs:86-97`):
1. `todoist::create_project(config, &name, description, is_favorite, true).await?` — POST to Todoist API, returns `Project` with server-assigned `id`
2. `add(config, &project).await?` — calls `config.add_project(project.clone())` then `config.save().await`
3. Formats output: either `serde_json::to_string(&project)` (JSON mode) or `"Created project {name} and added to config"`

Flow: **API write first → config write second**. If the API call fails, local config is never mutated.

**`projects::add`** (`src/projects.rs:154-158`): bundles `config.add_project` + `config.save`. Also used by `import` and `maybe_add_project`.

**`projects::rename` — local-only** (`src/projects.rs:173-188`):
- Resolves new name (from param or interactive prompt)
- Creates a new `Project` via struct-update: `Project { name: new_name, ..project.clone() }` — preserves all other fields including `id`
- Removes old project from config via `remove(config, project)` (`config.remove_project` + `config.save`)
- Adds renamed clone via `add(config, &new_project)` (`config.add_project` + `config.save`)
- **No Todoist API call.** Comment: "does not sync to Todoist." CLI help: "(n) Rename a project in config (not in Todoist)"
- Triggers **two sequential disk writes** (remove saves, then add saves again)

**Taxonomy of project operations:**

| Function | `src/projects.rs` | API call | Config write | Direction |
|---|---|---|---|---|
| `create` | 86 | `todoist::create_project` (POST) | `add_project` + `save` | Remote → Local |
| `delete` | 166 | `todoist::delete_project` (DELETE) | `remove_project` + `save` | Remote → Local |
| `remove` | 160 | **none** | `remove_project` + `save` | Local only |
| `rename` | 173 | **none** | `remove_project` + `save` + `add_project` + `save` | Local only |
| `add` | 154 | **none** | `add_project` + `save` | Local only |
| `import` | ~217 | `todoist::all_projects` (GET) | `add` per selected project | Remote → Local |
| `remove_auto` | ~224 | `todoist::all_projects` (GET) | `remove_project` per missing + `save` | Remote → Local |

**`projects::delete`** (`src/projects.rs:166-170`) is the canonical API-backed mutation: calls `todoist::delete_project` first, then `config.remove_project` + `config.save`. **API-first order** mirrors `create`.

**Config storage** (`src/config/mod.rs:64-65`): `projects` field is `Option<Vec<Project>>`, serialized as `"projectsv1"` in config JSON. `add_project` (`src/config/projects.rs:33`) initializes `None` to `Some(vec![project])`. `remove_project` (`:40`) filters by `id` and always sets `Some(...)` — after any mutation, `projects` is never `None`.

---

## Cross-Cutting Observations

- **API-first, config-second pattern**: All remote-backed mutations (`create`, `delete`) call the API before touching local config. If the API call fails, config is unchanged.

- **`String`-return convention**: Functions that don't need to return structured data return `Result<String, Error>` with either `"✓"` (success) or an error. Functions returning structured data return `Result<Project/Task/Section, Error>`.

- **No `update_project` in the API layer**: The codebase has no function for `POST /api/v1/projects/{id}`. The `post_todoist` helper (`src/todoist/request.rs:33`) accepts arbitrary URLs, so the HTTP plumbing is ready without changes.

- **`Project` struct is dual-use**: The same struct is deserialized from API responses (via `Deserialize`) and serialized for config storage / JSON output (via `Serialize`). Both derives are present at `src/projects.rs:18`.

- **Test pattern**: Every project test follows `mockito::Server::new_async()` → `server.mock(...)` → `config.with_mock_url(server.url())` → exercise → `mock.assert()`. No tests use real HTTP.

- **Fixtures convention**: `ResponseFromFile` enum variants share names with JSON files in `tests/responses/`. `Project.json` and `Projects.json` use same project id `"123"` as the pre-seeded fixture project (`src/test/fixtures.rs:97-120`). `NewProjects.json` uses `"890"` to test "project exists in API but not in config" scenarios.

## Open Areas

- The Todoist v1 `POST /projects/{id}` accepts `child_order` as writable but the codebase has no project reordering logic — this field exists on `Project` but is never sent in a write request.
- `description` is sent in `create_project` but the codebase has no way to update it after creation, despite the API supporting it.
- The `rename` function's double-save (remove → save, add → save) creates a brief intermediate state on disk where the project is absent — this could result in data loss if interrupted between the two saves.
- No tests exercise the scenario where `handle_response` receives a genuine empty-body 204 and the response is passed to a JSON deserializer — this path exists but is never targeted in tests because all 204-returning callers discard the body.
