# Research Findings

## Q1: How does `request::delete_todoist` construct and send DELETE requests, what headers and body does it include, and how does `handle_response` process success vs. error status codes?

### Findings
- **`delete_todoist` signature** — `src/todoist/request.rs:102-107`: accepts `config: &Config`, `url: &str`, `body: serde_json::Value`, `spinner: bool`, returns `Result<String, Error>`.
- **URL construction** — `src/todoist/request.rs:108-110`: resolves base URL via `get_base_url(config)` (returns `config.mock_url` in test mode at line 243-245, otherwise `TODOIST_URL = "https://api.todoist.com"` at line 18). Extracts token via `get_token(config)?` at line 109.
- **Headers** — `src/todoist/request.rs:117-121`: always sends `Content-Type: application/json`, `Authorization: Bearer {token}`, and `X-Request-Id: {uuid}`.
- **Body** — `src/todoist/request.rs:122`: **always** calls `.json(&body)` with no `Value::Null` branch. Unlike `post_todoist` (lines 60-65) which matches `Value::Null => client.send().await?`, `delete_todoist` sends the body as-is even when it's `Value::Null`, resulting in a literal `null` JSON body.
- **Spinner** — `src/todoist/request.rs:113`: `maybe_start_spinner(config, spinner)` at line 253 returns `None` in test mode (`cfg!(test)`) or when `DISABLE_SPINNER` env var is set / `config.spinners` is `Some(false)` / `spinner` param is `false`. Otherwise starts `Spinners::Dots4` with message "Querying API".
- **Timeout** — `src/todoist/request.rs:123`: uses `get_timeout(config)` (lines 228-246) which resolves from `config.timeout`, `config.args.timeout`, or `DEFAULT_TIMEOUT_SECONDS`.
- **`handle_response`** — `src/todoist/request.rs:172-206`:
  - **Success** (line 176): reads response body as text, calls `debug::maybe_print`, returns `Ok(json_string)`.
  - **401/403 with non-pro-plan URL** (lines 179-182): returns `Error::new("reqwest", "Unauthorized or Forbidden response from Todoist\nRun 'tod auth login' to reauthenticate")`.
  - **401/403 with pro-plan URL** (lines 183-185): returns `Error::new("reqwest", REMINDERS_PRO_PLAN_MESSAGE)`.
  - **All other errors** (lines 186-205): reads response body as text, returns `Error::new("reqwest", ...)` with method, url, body, and response text in the message.
- **`requires_login`** — `src/todoist/request.rs:163-165`: returns true for 401 and 403.
- **`is_pro_plan_url`** — `src/todoist/request.rs:168-170`: checks if the URL contains `REMINDERS_URL = "/api/v1/reminders"`.

## Q2: What is the full pattern of existing DELETE operations in `src/todoist/mod.rs` (`delete_task`, `delete_project`), including URL construction, body content, the `spinner` parameter, error propagation, and return type conventions?

### Findings
- **`delete_task`** — `src/todoist/mod.rs:656-662`:
  - URL: `format!("{}/{}", "/api/v1/tasks/", task_id)` (line 658)
  - Body: `json!({})` — empty JSON object (line 657)
  - Returns `Result<String, Error>` — always `Ok("✓".into())` on success (line 661)
  - No deserialization of the response body; discards API return value
  - `spinner: bool` parameter is passed through to `request::delete_todoist`

- **`delete_project`** — `src/todoist/mod.rs:665-677`:
  - URL: `format!("{}/{}", "/api/v1/projects", project.id)` (line 671)
  - Body: `json!({})` — empty JSON object (line 672)
  - Returns `Result<String, Error>` — always `Ok("✓".into())` on success (line 675)
  - Accepts `&Project` (not just an ID string like `delete_task`)
  - No deserialization of the response body; discards API return value

- **Pattern conventions**:
  - Return type: `Result<String, Error>` — returns the checkmark string "✓" on success
  - Body: always `json!({})` (empty object, not `Value::Null`)
  - Error propagation: uses `?` on the `delete_todoist` call; errors bubble up with source "reqwest" or "post_todoist" (from `get_token`)
  - `spinner` parameter: both functions accept and pass through a `spinner: bool`
  - The `delete_project` test at `src/todoist/mod.rs` is not present — DELETE project tests exist only in `src/projects.rs` test module (e.g. `test_project_delete` at line ~560) and `src/commands/project_commands.rs`

## Q3: How are CLI subcommands structured in `SectionCommands` (`src/commands/section_commands.rs`), and how does the `section_command` dispatch function in `src/commands/mod.rs` route each variant to its handler — including how JSON mode and config loading are threaded through?

### Findings
- **`SectionCommands` enum** — `src/commands/section_commands.rs:6-10`: currently has a single variant:
  ```rust
  pub enum SectionCommands {
      #[clap(alias = "c")]
      Create(Create),
  }
  ```
- **`Create` struct** — `src/commands/section_commands.rs:12-21`: two optional fields `name: Option<String>` and `project: Option<String>` (both `-s`/`-p` flags).
- **`create` handler** — `src/commands/section_commands.rs:24-40`:
  - Accepts `config: &Config`, `args: &Create`, `json: bool`
  - Resolves name via `super::fetch_string(name.as_deref(), config, input::NAME)` (line 27) — this errors in JSON mode if name is not provided
  - Resolves project via `super::fetch_project(project.as_deref(), config).await?` (line 29) — errors in JSON mode if project is not provided
  - Calls `todoist::create_section(config, &name, &project, true).await?` with `spinner = true` (line 33)
  - JSON mode: serializes the `Section` as JSON (line 35)
  - Non-JSON mode: prints `format::green_string("Section created successfully")` (line 37)
  - Returns `Result<String, Error>`

- **Dispatch** — `src/commands/mod.rs:196-207`:
  ```rust
  async fn section_command(command: &SectionCommands, cli: &Cli, tx: &UnboundedSender<Error>) -> Result<CommandResult, Error> {
      match command {
          SectionCommands::Create(args) => {
              let config = fetch_config(cli, tx).await?;
              let result = section_commands::create(&config, args, cli.json).await;
              Ok(build_command_result(result, &config))
          }
      }
  }
  ```
  - Config fetched via `fetch_config` (line ~429: loads config, sets cli context, ensures auth present, checks version, sets timezone)
  - `cli.json` is passed as the `json: bool` parameter to the handler
  - Result wrapped in `CommandResult` via `build_command_result` (captures bell settings and json flag from config)

- **Contrast with `ProjectCommands` dispatch** — `src/commands/mod.rs:211-253`:
  - `ProjectCommands` has 7 variants (Create, List, Remove, Rename, Import, Empty, Delete)
  - Uses `&mut config` for most handlers (config mutation for add/remove)
  - `SectionCommands` uses `&Config` (immutable reference) — notably, `section_commands::create` does not mutate config
  - `ProjectCommands` dispatch passes `cli.json` differently — via the `build_command_result` which extracts it from `config.args.json` (set by `with_cli_context`)

## Q4: What test patterns are used for DELETE API functions in `src/todoist/mod.rs` (mockito server setup, request matching, response fixtures), and what section-related test fixtures exist in `src/test/fixtures.rs` and `src/test/responses.rs`?

### Findings
- **DELETE task test** — `src/todoist/mod.rs:1019-1033` (`test_delete_task`):
  - Creates mockito async server
  - Mocks `DELETE /api/v1/tasks/6Xqhv4cwxgjwG9w8` with status 200 and `ResponseFromFile::TodayTask` body
  - Confirms mock was called: `mock.assert()`
  - Asserts result is `Ok("✓")`

- **DELETE project test** — No `test_delete_project` exists in `src/todoist/mod.rs`. The delete-project test is in `src/projects.rs` (around line ~560, `test_project_delete`):
  - Mocks `DELETE /api/v1/projects/123` with status 200 and `ResponseFromFile::Project` body
  - Calls `projects::delete(&mut config, &project).await` which internally calls `todoist::delete_project` then `config.remove_project`

- **General test pattern** (observed across all tests in `src/todoist/mod.rs`):
  1. Create `mockito::Server::new_async().await`
  2. Set up `.mock("METHOD", url)` with `.with_status(200)`, `.with_header("content-type", "application/json")`, `.with_body(ResponseFromFile::Variant.read().await)`, `.create_async().await`
  3. Build config via `test::fixtures::config().await.with_mock_url(server.url())`
  4. Call the API function
  5. Assert result and `mock.assert()` (or `mock.assert_async().await`)

- **Section test fixtures** — `src/test/fixtures.rs:126-141`:
  ```rust
  pub fn section() -> Section {
      Section {
          id: "1234".to_string(),
          name: "Bread".to_string(),
          user_id: "910".to_string(),
          project_id: "5678".to_string(),
          added_at: "2020-06-11T14:51:08.056500Z".to_string(),
          updated_at: None,
          archived_at: None,
          section_order: 1,
          is_archived: false,
          is_deleted: false,
          is_collapsed: false,
      }
  }
  ```

- **Section response fixtures** — `src/test/responses.rs`:
  - `Section` variant → reads `tests/responses/Section.json` (single section object)
  - `Sections` variant → reads `tests/responses/Sections.json` (object with `results` array and `next_cursor: null`)
  - `Section` and `Sections` variants have no dynamic value replacement in `replace_values` (line ~80: matches `Self::Section | Self::Sections => Vec::new()`)

## Q5: How does the `project delete` CLI command flow from `ProjectCommands::Delete` through argument parsing, config fetching, user interaction, JSON mode branching, and the API call in `src/commands/project_commands.rs`?

### Findings
- **Argument struct** — `src/commands/project_commands.rs:89-100` (`Delete`):
  - `force: bool` (`-f/--force`, default false): skip deletion confirmation when project has tasks
  - `repeat: bool` (`-r/--repeat`, default false): keep repeating prompt to delete projects
  - `project: Option<String>` (`-s/--project`): project name

- **Dispatch** — `src/commands/mod.rs:278-282`:
  ```rust
  ProjectCommands::Delete(args) => {
      let mut config = fetch_config(cli, tx).await?;
      let result = project_commands::delete(&mut config, args).await;
      Ok(build_command_result(result, &config))
  }
  ```
  - `fetch_config` loads config, sets CLI context (verbose/timeout/json/tx), disables spinners in JSON mode, ensures auth, checks version, sets timezone
  - `json` flag flows through `config.args.json` (set by `with_cli_context`)

- **`delete` handler** — `src/commands/project_commands.rs:148-179`:
  1. Destructures `Delete { force, project, repeat }`
  2. Enters a `loop` (for repeat mode)
  3. Resolves project: `super::fetch_project(project.as_deref(), config).await?` → errors in JSON mode if `project` is `None`
  4. Fetches tasks: `todoist::all_tasks_by_project(config, &project, None).await?` to check if project is non-empty
  5. **Confirmation logic** (lines 159-171):
     - If `!force && !tasks.is_empty()`:
       - JSON mode → returns `Error::new("json_mode", JSON_INTERACTIVE_ERROR)` (line 161)
       - Non-JSON mode → presents `[CANCEL, DELETE]` prompt via `input::select`
       - If user picks `CANCEL` → returns `Ok("Cancelled")`
  6. Calls `projects::delete(config, &project).await` (line 173) which:
     - Calls `todoist::delete_project(config, project, true).await?` (with `spinner = true`)
     - Calls `config.remove_project(project)` to remove from local config
     - Calls `config.save().await`
  7. If `repeat` is false → returns the value; otherwise loops

- **JSON mode behavior**:
  - Project must be provided as a CLI flag (`-p/--project`) — if `None`, `fetch_project` errors with JSON mode error
  - Confirmation prompt errors out in JSON mode if project has tasks and `--force` is not set
  - `--force` flag bypasses confirmation even in JSON mode (as long as project is provided)

- **Tests** — `src/commands/project_commands.rs` tests:
  - `delete_force_skips_confirmation_prompt_for_non_empty_project` (line ~169): mocks tasks GET and project DELETE; asserts force succeeds without "Cancelled"
  - `delete_cancels_when_user_selects_cancel_for_non_empty_project` (line ~193): mock_select(0) picks CANCEL; asserts result is "Cancelled"
  - `delete_confirms_and_removes_project_when_user_selects_delete` (line ~218): mock_select(1) picks DELETE; asserts success
  - `delete_force_flag_parses` (line ~240): clap parsing test

## Q6: What fields does the `Section` struct in `src/sections.rs` contain, how are `Section::from_json` and `SectionResponse::from_json` used, and where are sections fetched, selected, or moved-to across the codebase?

### Findings
- **`Section` struct** — `src/sections.rs:9-21`:
  ```rust
  pub struct Section {
      pub id: String,
      pub name: String,
      pub user_id: String,
      pub project_id: String,
      pub added_at: String,
      pub updated_at: Option<String>,
      pub archived_at: Option<String>,
      pub section_order: i32,
      pub is_archived: bool,
      pub is_deleted: bool,
      pub is_collapsed: bool,
  }
  ```
  Derives: `PartialEq, Deserialize, Serialize, Clone, Debug`

- **`Section::from_json`** — `src/sections.rs:23-26`: single-entity deserialization via `serde_json::from_str(json)`, used when Todoist API returns a single section (e.g. after `create_section`)

- **`SectionResponse`** — `src/sections.rs:29-32`: paginated wrapper with `results: Vec<Section>` and `next_cursor: Option<String>`. `SectionResponse::from_json` (line 35) deserializes cursor-paginated list responses; used everywhere with cursor-based pagination loops.

- **`all_sections_by_project`** — `src/todoist/mod.rs:358-380`: cursor-based paginated GET at `SECTIONS_URL?project_id={id}&limit={limit}`. Uses `SectionResponse::from_json`.

- **`all_sections`** — `src/sections.rs:43-54`: fetches sections for all configured projects in parallel via `future::join_all`, flattens results. The only cross-project sections aggregator.

- **`select_section`** — `src/sections.rs:57-73`: fetches sections for a project via `all_sections_by_project`, prepends "No section" option, prompts user to pick one. Returns `Result<Option<Section>, Error>`. Used by `task_commands.rs` lines 205 and 237 when creating/editing tasks.

- **`create_section`** — `src/todoist/mod.rs:677-686`: POSTs to `SECTIONS_URL` with `{"name": ..., "project_id": ...}`, deserializes response with `Section::from_json`.

- **`move_task_to_section`** — `src/todoist/mod.rs:475-487`: POSTs to `{TASKS_URL}{task_id}/move` with `{"section_id": ...}`, deserializes response with `Task::from_json`. Used in `project::empty` flow (`src/projects.rs:648`).

- **No DELETE section exists** anywhere in the codebase. Sections are created, fetched, selected, and used for task-placement, but never deleted.

## Cross-Cutting Observations
- **DELETE return convention**: All DELETE API functions return `Result<String, Error>` with `Ok("✓")`; response body is discarded. Create functions return the domain type (e.g. `Result<Task, Error>`, `Result<Section, Error>`).
- **Config mutation**: `ProjectCommands` handlers use `&mut config`; `SectionCommands` uses `&Config` (immutable). This matters because `delete_project` in `projects.rs` mutates config (removes project after API deletion). A hypothetical `delete_section` would likely not need config mutation since sections are not stored in config.
- **Spinner convention**: API functions expose `spinner: bool` parameter. CLI command handlers pass `true` (visible spinner). Tests pass `false` (mockito servers with `cfg!(test)` already bypass spinners, but explicit `false` is still used in tests like `create_section`).
- **JSON mode**: Guarded at the `fetch_*` level in `src/commands/mod.rs` — interactive prompts produce `Error::new("json_mode", JSON_INTERACTIVE_ERROR)` when `config.args.json` is true and the required argument isn't provided via CLI flag.
- **Mockito test pattern**: Tests use `mockito::Server::new_async().await`, `.mock("METHOD", url)`, `.with_status(200)`, `.with_header("content-type", "application/json")`, `.with_body(ResponseFromFile::Variant.read().await)`. The pattern is consistent across all API tests.
- **`delete_todoist` body quirk**: Unlike `post_todoist` which branches on `Value::Null` to send without a body, `delete_todoist` always calls `.json(&body)`. Both callers (`delete_task`, `delete_project`) pass `json!({})` so this is not exercised differently in practice.
- **Section delete gap**: Sections are created via `POST /api/v1/sections` but there is no `DELETE /api/v1/sections/{id}` endpoint client in the codebase. Section-related operations are: create, list (by project), and move-tasks-to.

## Open Areas
- The Todoist REST API v2 documentation for the `DELETE /rest/v2/sections/{section_id}` endpoint would need to be consulted for the exact URL path and response format. The current codebase uses `/api/v1/sections` for GET-list and POST-create but there is no `SECTIONS_URL` with a trailing slash variant like `TASKS_URL = "/api/v1/tasks/"` uses.
- No existing `SECTIONS_URL` constant ends with a trailing `/`, unlike `TASKS_URL`. A new constant or format string would be needed for a delete-by-ID URL.
