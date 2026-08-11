# Research Findings

## Q1: What is the full set of fields on the `Label` struct in `src/labels.rs`, and how does the Todoist API label response JSON (from `tests/responses/Label.json`) map to those fields? Which fields are optional vs required in the API?

### Findings
- The `Label` struct has five fields (`src/labels.rs:9-13`): `id: String`, `name: String`, `color: String`, `order: Option<u32>`, `is_favorite: bool`.
- `tests/responses/Label.json` contains: `"id": "123"`, `"name": "345"`, `"is_favorite": false`, `"order": null`, `"color": "red"`. All five keys match the struct fields exactly via serde field-name matching.
- `Label.json` represents a single label object (no pagination wrapper). The paginated version is `tests/responses/Labels.json` which wraps the same label object in `{"results": [...], "next_cursor": null}`.
- Based on the struct definition: `id`, `name`, and `color` are required (`String`, no `Option`). `order` is optional (`Option<u32>`, API sends `null` when unset). `is_favorite` is required (`bool`, API always sends it).
- The test fixture at `src/test/fixtures.rs:16-23` (`label()`) constructs a Label with the same values as `Label.json`: `id: "123"`, `name: "345"`, `color: "red"`, `order: None`, `is_favorite: false`.
- `Label` derives `Serialize`, `Deserialize`, `Debug`, `PartialEq`, `Eq` (`src/labels.rs:8`). `LabelResponse` derives only `Deserialize` (`src/labels.rs:17`).
- `Label` implements `Display` (`src/labels.rs:28-31`): outputs just the label name.
- Serde roundtrip tests exist via proptest at `src/labels.rs:100-108` asserting `label_serde_roundtrip`.

## Q2: Trace the flow of `create_project` from CLI to API: how does `src/commands/project_commands.rs` parse arguments, dispatch to `src/projects.rs`, and call `src/todoist/mod.rs`? What does the `request::post_todoist` function expect for URL construction, body format, and response handling?

### Findings
- **CLI parsing** (`src/commands/project_commands.rs:28-37`): `Create` derives `Parser`. Fields: `name: Option<String>` (`-n`), `description: Option<String>` (`-d`), `is_favorite: bool` (`-f`, defaults false).
- **Command handler** (`src/commands/project_commands.rs:167-178`): `create()` destructures args, resolves `name` via `super::fetch_string()` (argument or interactive prompt, with JSON-mode guard), defaults description to `""`, then delegates to `projects::create(config, name, description, *is_favorite, json)`.
- **Dispatch** (`src/commands/mod.rs:220-224`): `project_command()` matches `ProjectCommands::Create(args)` → calls `project_commands::create()`. Returns `CommandResult` via `build_command_result()` which attaches bell settings and json flag from config.
- **Business logic** (`src/projects.rs:98-108`): `create()` calls `todoist::create_project(config, &name, description, is_favorite, true)` (spinner always true), then `add(config, &project)` to save to config. If json, returns `serde_json::to_string(&project)`; otherwise a human-readable message.
- **API call** (`src/todoist/mod.rs:432-441`): `create_project()` sets URL to `PROJECTS_URL` (`"/api/v1/projects"` constant at line 39). Body is `json!({"name": name, "description": description, "is_favorite": is_favorite})`. Calls `request::post_todoist(config, &url, body, spinner)`.
- **HTTP layer** (`src/todoist/request.rs:33-68`): `post_todoist()` constructs full URL via `get_base_url(config)` — returns `TODOIST_URL` (`"https://api.todoist.com"`) or `mock_url` in tests (`src/todoist/request.rs:168-173`). Sends POST with headers: `Content-Type: application/json`, `Authorization: Bearer {token}`, `X-Request-Id: {uuid}`. Timeout via `get_timeout()` defaults to `DEFAULT_TIMEOUT_SECONDS` (30s). When body is `Value::Null`, uses `client.send()` instead of `client.json(&body)`. On success returns the response text; on 401/403 returns re-login error; on other errors returns diagnostic Error with method/url/body/response.

## Q3: How does `src/commands/mod.rs` register and dispatch command groups (like `ProjectCommands`, `SectionCommands`)? What would need to happen in the `Commands` enum, the `select_command` match block, and the `Cli` struct to add a new top-level command group?

### Findings
- **`Cli` struct** (`src/commands/mod.rs:48-67`): Holds `command: Commands` as a `#[command(subcommand)]`. Also has global flags: `verbose`, `config`, `timeout`, `json`. No changes needed in `Cli` to add a new command group.
- **`Commands` enum** (`src/commands/mod.rs:70-113`): Uses `#[derive(Subcommand, Debug, Clone)]`. Each variant wraps a sub-enum: e.g., `Project(ProjectCommands)`, `Section(SectionCommands)`, etc. Each variant has `#[command(subcommand)]` and `#[clap(alias = "X")]` with a doc comment. Currently 9 variants: Project, Section, Task, List, Reminder, Config, Auth, Shell, Test.
- **Module declarations** (`src/commands/mod.rs:15-27`): Each sub-enum has a corresponding `mod` (e.g., `mod project_commands`, `mod section_commands`) with a `use` import for the type.
- **`select_command()` dispatch** (`src/commands/mod.rs:130-142`): Matches on `cli.command` and dispatches to handler functions: `project_command()`, `section_command()`, `task_command()`, etc. Adding a new group requires a new arm `Commands::Label(command) => label_command(command, &cli, &tx).await` and a corresponding `async fn label_command(...)` that matches the sub-enum variants.
- **Handler pattern** (`src/commands/mod.rs:197-218` for `section_command()`): Each handler matches on sub-enum variants, calls `fetch_config(cli, tx)` (loads config, checks auth, applies CLI context), delegates to the sub-command module (e.g., `section_commands::create()`), wraps result with `build_command_result()`.
- **`fetch_config()`** (`src/commands/mod.rs:460-465`): Always called before business logic. Loads config, applies CLI context (`with_cli_context`), checks version, sets timezone.
- **Sub-enum and arg struct pattern**: Each command group file (e.g., `src/commands/section_commands.rs`) defines: (1) a `#[derive(Subcommand)]` enum with variants, (2) `#[derive(Parser)]` arg structs for each variant, (3) public async fn handlers.

## Q4: How does the `delete_project` flow work end-to-end — specifically, how does `delete_todoist` in `src/todoist/request.rs` handle the DELETE HTTP method, what status codes does it treat as success, and how does the caller (`projects::delete` → `project_commands::delete`) handle the response?

### Findings
- **CLI** (`src/commands/project_commands.rs:222-250`): `delete()` fetches project via `fetch_project()`, then calls `todoist::all_tasks_by_project()` to check if project has tasks. If it has tasks and `!force`, shows a confirmation prompt (CANCEL/DELETE options). If user cancels, returns "Cancelled". Otherwise calls `projects::delete()`.
- **Business logic** (`src/projects.rs:133-137`): `delete()` calls `todoist::delete_project(config, project, true)`, then `config.remove_project(project)`, then `config.save()`. Returns `Ok("✓".into())` on success.
- **API call** (`src/todoist/mod.rs:425-431`): `delete_project()` builds URL `format!("{}/{}", PROJECTS_URL, project.id)` (e.g., `/api/v1/projects/123`), body `json!({})`, calls `request::delete_todoist()`.
- **HTTP layer** (`src/todoist/request.rs:82-109`): `delete_todoist()` constructs full URL, sends DELETE with `Authorization: Bearer`, `Content-Type: application/json`, `X-Request-Id`. Always sends `.json(&body)`. Calls `handle_response()`.
- **`handle_response()`** (`src/todoist/request.rs:149-170`): Treats `status.is_success()` as success (HTTP 200-299 range). On success, returns `response.text().await?` which is an empty string for 204. On 401/403: returns a "run 'tod auth login'" error, unless URL matches a pro-plan URL (`REMINDERS_URL`), then returns pro-plan error. On any other status: returns a diagnostic error with method, URL, body, and response text.
- **Caller handling**: `delete_project` at `src/todoist/mod.rs:431` ignores the response body string, returns `Ok("✓".into())`. `projects::delete` also returns `Ok("✓".into())` after config cleanup. The CLI handler wraps this via `build_command_result()` and sends to `output_result()` in main.

## Q5: What is the existing `all_labels` function's pagination pattern in `src/todoist/mod.rs`, and how does the `LabelResponse::from_json` method deserialize the paginated API response? Are label create/update/delete responses paginated or do they return a single object?

### Findings
- **`all_labels()` pagination** (`src/todoist/mod.rs:359-374`): Uses a `loop` with cursor-based pagination. Default limit is `QUERY_LIMIT` (200, defined at line 44). Initial URL: `format!("{LABELS_URL}?limit={limit}")` where `LABELS_URL = "/api/v1/labels"`. Subsequent: `format!("{LABELS_URL}?limit={limit}&cursor={string}")`. Calls `request::get_todoist()` then `LabelResponse::from_json()`. Extends `labels` vec with `results`, breaks when `next_cursor` is `None`.
- **`LabelResponse`** (`src/labels.rs:17-27`): Derives `Deserialize`. Has two fields: `results: Vec<Label>` and `next_cursor: Option<String>`. `from_json()` calls `serde_json::from_str(json)` and wraps the error in the `Error` type via `From` impl.
- **Pattern consistency**: Identical pagination pattern used by `all_projects`, `all_sections_by_project`, `all_reminders`, `all_comments`, `all_tasks_by_project`, `all_tasks_by_filter`, `all_tasks_by_ids` — all use `Response::from_json()` → extend results → check `next_cursor`.
- **Create/update/delete responses**: The Todoist REST API returns a **single object** for create and update (not paginated). This is the pattern used by `create_project` (`src/todoist/mod.rs:440` calls `Project::from_json(&json)`) and `create_section` (`src/todoist/mod.rs:476` calls `Section::from_json(&json)`). Delete returns 204 No Content (empty body). There is no existing label create/update/delete in the codebase.
- **`Label` already has `from_json`** via serde Deserialize, but it's used on the individual label objects within `LabelResponse.results`. There is no standalone `Label::from_json()` method — but the `Project` and `Section` types do have such methods (`src/projects.rs:92`, `src/sections.rs`).

## Q6: How are HTTP 204 No Content responses handled in `src/todoist/request.rs`'s `handle_response` function? Look at how `archive_project` and `unarchive_project` deal with 204 responses, since `DELETE /labels/{id}` also returns 204.

### Findings
- **`handle_response()`** (`src/todoist/request.rs:149-170`): The check is `status.is_success()`. For HTTP 204, `reqwest::StatusCode::is_success()` returns `true` (204 is in the 200-299 range). The function then calls `response.text().await?` which returns an empty string `""` for 204 responses. No special branching for 204 vs 200.
- **`archive_project()`** (`src/todoist/mod.rs:459-468`): POST to `/api/v1/projects/{id}/archive`. Calls `post_todoist()`, ignores response body, returns `Ok("✓".into())`.
- **`unarchive_project()`** (`src/todoist/mod.rs:471-480`): POST to `/api/v1/projects/{id}/unarchive`. Same pattern: ignores response, returns `Ok("✓".into())`.
- **Test pattern for 204** (`src/todoist/mod.rs` tests, lines ~1080-1097): `test_archive_project_hits_api` and `test_unarchive_project_hits_api` use `.with_status(204)` with **no** `.with_body()` call — mockito returns an empty body by default. The test asserts `Ok("✓".to_string())`.
- **`update_task_priority`** (`src/todoist/mod.rs:315-324`): Also handles 204 responses — POST returns 204 and the function ignores the body, returns `Ok("✓".into())`. Test at `src/todoist/mod.rs` uses `.with_status(204)` with a body provided (ignored by code).
- **Contrast with `delete_section`** (`src/todoist/mod.rs:482-489`): Uses `delete_todoist()` (DELETE method), ignores body, returns `Ok("✓".into())`. Test mocks 200 with `.with_body("null")`, but the actual API returns 204. The code would work identically with either status since both are `is_success()`.
- **Key insight**: The codebase already handles 204 transparently. Any DELETE function can use `delete_todoist()` and ignore the response body (empty string), returning `Ok("✓".into())` — exactly the pattern for `DELETE /labels/{id}`.

## Q7: What test patterns exist for resource creation and deletion — specifically, how do `test_create_section` and `test_delete_section` in `src/todoist/mod.rs` set up mockito mocks, construct test configs, and assert results? What fixtures and response files would a label create/delete test need?

### Findings
- **`test_create_section`** (`src/todoist/mod.rs`, lines ~660-676):
  1. Create mockito server: `let mut server = mockito::Server::new_async().await`
  2. Mock POST: `.mock("POST", "/api/v1/sections")` with `.with_status(200)`, `.with_header("content-type", "application/json")`, `.with_body(ResponseFromFile::Section.read().await)`, `.create_async().await`
  3. Build config: `test::fixtures::config().await.with_mock_url(server.url())`
  4. Get fixture project: `test::fixtures::project()`
  5. Call API: `create_section(&config, "New task", &project, false).await`
  6. Assert: `assert_eq!(result, Ok(test::fixtures::section()))` — exact struct match
  7. Verify: `mock.assert()` — ensures the mock was hit exactly once

- **`test_delete_section`** (`src/todoist/mod.rs`, lines ~702-717):
  1. Mock DELETE: `.mock("DELETE", "/api/v1/sections/1234")` with `.with_status(200)`, `.with_body("null")`
  2. Config via `with_mock_url(server.url())`
  3. Call: `delete_section(&config, "1234", false).await`
  4. Assert: `assert_eq!(result, Ok("✓".into()))` — deletion returns checkmark string
  5. Verify: `mock.assert()`

- **Common test infrastructure**:
  - `test::fixtures::config()` (`src/test/fixtures.rs:92-102`): Creates a Config with token, a project fixture, fixed time provider, and America/Vancouver timezone
  - `.with_mock_url(url)` chains on Config to set the mockito server URL. This is defined on Config (likely `src/config/mod.rs`) and causes `get_base_url()` to return the mock URL.
  - `ResponseFromFile` enum (`src/test/responses.rs:12-37`): Maps variants to `tests/responses/{variant}.json` files. Has `Label` and `Labels` variants already defined (though `Label` is `#[allow(dead_code)]` at line 27-28).
  - `Label.json` (`tests/responses/Label.json`): Single label object, usable for create/update mock responses.
  - `Labels.json` (`tests/responses/Labels.json`): Paginated wrapper `{"results": [...], "next_cursor": null}`, used by `test_all_labels` mock.
  - `test::fixtures::label()` (`src/test/fixtures.rs:16-23`): Returns `Label { id: "123", name: "345", color: "red", order: None, is_favorite: false }` — already exists.

- **What label create/delete tests would need**:
  - **Create**: Mock POST `/api/v1/labels` with status 200 and body from `ResponseFromFile::Label`. Assert result equals `test::fixtures::label()`.
  - **Delete**: Mock DELETE `/api/v1/labels/{id}` — the Todoist API returns 204. Pattern from `archive_project`: `.with_status(204)` without `.with_body()`. Assert result equals `Ok("✓".into())`.
  - **Update**: Would need POST `/api/v1/labels/{id}` with status 200 and body from `ResponseFromFile::Label`. Assert result equals updated `test::fixtures::label()`.
  - All fixtures (`Label.json`, `Labels.json`, `test::fixtures::label()`) already exist.

## Cross-Cutting Observations
- **Pagination pattern**: Every list endpoint uses identical cursor-based pagination: `loop { get → Response::from_json → extend results → match next_cursor }`. The `all_labels` function already implements this at `src/todoist/mod.rs:359-374`.
- **POST for mutations**: The codebase uses POST (not PUT/PATCH) for all update operations (`update_project`, `archive_project`, `update_task_priority`, etc.) as the Todoist REST API uses POST for mutations.
- **204 handling**: The `handle_response()` function has no special-casing for 204; empty body is returned as `""` and callers ignore it. Tests for 204 responses simply omit `.with_body()`.
- **Config wiring**: All command handlers call `fetch_config()` which loads config, checks auth, applies CLI context, checks version, sets timezone. Config carries `args.json` and `mock_url` for test vs production routing.
- **Error type convention**: `Error::new(source, message)` where source is a snake_case function name. The `?` operator works via `From` impls for reqwest, serde_json, io errors. Callers never pre-apply `format::*_string` coloring.
- **JSON mode**: All interactive prompts are guarded with `if config.args.json { return Err(Error::new("json_mode", ...)) }` at the fetch_* call sites in `src/commands/mod.rs`. Commands that require user prompts (delete confirmation, import interactive) return errors in JSON mode unless `--force` or `--auto` flags bypass interaction.
- **Spinner parameter**: API functions take a `spinner: bool` parameter. It's `true` in business-logic callers and sometimes `false` in test_all_endpoints or tests. The spinner is suppressed in test mode via `if cfg!(test)` in `maybe_start_spinner()`.

## Open Areas
- The `Label` variant in `ResponseFromFile` (`src/test/responses.rs:27-28`) is `#[allow(dead_code)]` — currently unused in tests. The enum and file already exist and are ready for use.
- There is no `Label::from_json()` standalone method unlike `Project::from_json()` and `Section::from_json()`. For a label create API function, either a standalone `Label::from_json()` would need to be added or the deserialization handled inline (the struct already derives `Deserialize`).
- The existing `all_labels` function at `src/todoist/mod.rs:359` takes `limit: Option<u8>` as the third parameter while `get_labels` at `src/labels.rs:33` takes only `config` and `spinner` (passes `None` for limit). There is no CLI command that currently invokes either.
