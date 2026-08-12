# Research Findings

## Q1: How do existing task CRUD operations construct URLs, build request bodies, and handle API responses? What patterns for 204 No Content?

### Findings

**URL construction** — All operations use a base URL constant with task IDs appended:
- `TASKS_URL: &str = "/api/v1/tasks/"` — `src/todoist/mod.rs:32`
- `COMMENTS_URL: &str = "/api/v1/comments/"` — `src/todoist/mod.rs:34`
- Simple resource: `format!("{TASKS_URL}{task_id}")` — `src/todoist/mod.rs:370`
- Sub-resource actions: `format!("{TASKS_URL}{task_id}/move")` — `src/todoist/mod.rs:312`, `format!("{TASKS_URL}{task_id}/close")` — `src/todoist/mod.rs:401`
- The base URL (`https://api.todoist.com` or mock URL) is prepended inside request functions at `src/todoist/request.rs:253-259`

**HTTP methods** — Four request functions in `src/todoist/request.rs`:
| Function | Lines | Method | Auth |
|---|---|---|---|
| `post_todoist` | 38-74 | POST | Bearer |
| `post_todoist_no_token` | 77-101 | POST | None |
| `delete_todoist` | 108-138 | DELETE | Bearer |
| `get_todoist` | 141-167 | GET | Bearer |

All share `X-Request-Id` header with UUID, `Content-Type: application/json`, and configurable timeout.

**Request body patterns** — Two approaches in `src/todoist/mod.rs`:
- **`json!({"field": value})`** — single-field updates like `update_task_content` (line 368), `update_task_description` (line 387), `update_task_priority` (line 336)
- **`HashMap<String, Value>` built incrementally** — multi-field creation like `create_task` (lines 163-190), where optional fields are conditionally inserted

**Response handling** — Two distinct patterns:
- **Pattern A (returns parsed type)**: Call `request::post_todoist`, then `Type::from_json(&response)` — e.g., `create_comment` at `src/todoist/mod.rs:528-536`, `create_task` at line 198
- **Pattern B (returns `Ok("✓")`)**: Call request function, discard the response string, return `Ok("✓".into())` — e.g., `update_task_content` at `src/todoist/mod.rs:362-370`, `delete_task` at `src/todoist/mod.rs:416-421`, `complete_task` at `src/todoist/mod.rs:396-409`, `archive_project` at `src/todoist/mod.rs:456-461`. The comment `// Does not pass back a task` is consistently used.

**Delete uses `delete_todoist`** — `src/todoist/mod.rs:416-421` is the only operation that calls `delete_todoist`; all updates use `post_todoist`.

**204 No Content handling** — `handle_response` at `src/todoist/request.rs:208-211` treats all `2xx` codes identically via `response.text().await?`. For 204, this returns `Ok("")`. Pattern B callers discard this empty string. The test at `src/todoist/mod.rs:786` (`test_update_task_priority`) mocks status 204 but still provides a body — no test verifies 204 with an empty body specifically.

## Q2: What does the `Comment` struct look like? Would updating return the same shape?

### Findings

**`Comment` struct** — `src/comments.rs:9-18`:
```rust
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct Comment {
    pub id: String,
    pub posted_uid: Option<String>,
    pub content: String,
    pub uids_to_notify: Option<Vec<String>>,
    pub is_deleted: bool,
    pub posted_at: String,
    pub reactions: Option<Reactions>,
    pub item_id: String,
    pub file_attachment: Option<Attachment>,
}
```
All fields public. `from_json` at line 19-22; `fmt` method at line 105+ handles rendering with timezone formatting and OSC8 hyperlinks for attachments.

**What the API returns on create** — Fixture at `tests/responses/Comment.json` returns: `content`, `id`, `posted_at`, `item_id`, `is_deleted`, `attachment` (with `file_name`, `file_type`, `file_url`, `resource_type`). Notably, the API field is `attachment` but the Rust field is `file_attachment`. The `Attachment` enum at `src/comments.rs:43-51` is `#[serde(untagged)]`, and the `FileAttachment` variant at line 54-59 captures `file_name`, `file_type`, `file_url`, `resource_type`.

**Update response shape** — No comment update function exists in the codebase, so there's no precedent. The Todoist REST API typically returns the full updated resource object on POST/PUT for comments. A comment update would likely return the same `Comment` shape as creation, but this is not confirmed from the codebase — it depends on the Todoist API contract.

**CommentResponse** — `src/comments.rs:24-32`: paginated wrapper with `results: Vec<Comment>` and `next_cursor: Option<String>`, used by `all_comments` at `src/todoist/mod.rs:549-578`.

**Attachment variants** — `src/comments.rs:43-51`: `File`, `Url`, `ShortUrl`, `Video`, `Image` — `#[serde(untagged)]` enum. Each variant struct is defined at lines 53-102.

## Q3: How does `handle_response` handle HTTP status codes?

### Findings

**Function signature** — `src/todoist/request.rs:197-231`:
```rust
async fn handle_response(config, response, method, url, body) -> Result<String, Error>
```

**Three branches** (lines 204-230):

1. **Success (2xx)**: `response.text().await?` then `Ok(json_string)` — all 2xx codes including 200, 201, 204 treated identically. No distinction made for 204 No Content. **204 returns `Ok("")`** (empty string).
   — `src/todoist/request.rs:208-211`

2. **Auth error on non-pro URL**: Status 401 or 403 + URL is NOT a pro-plan URL → `Error::new("reqwest", "Unauthorized or Forbidden response from Todoist\nRun 'tod auth login' to reauthenticate")`
   — `src/todoist/request.rs:212-216`

3. **Auth error on pro URL**: Status 401 or 403 + URL IS a pro-plan URL → `Error::new("reqwest", REMINDERS_PRO_PLAN_MESSAGE)` ("Reminders are only available on Pro Todoist plans...")
   — `src/todoist/request.rs:217-220`

4. **All other errors**: `response.text().await?`, then `Error::new("reqwest", "{method}\n{url}\n{body}\n{response}")` — dumps raw response for debugging
   — `src/todoist/request.rs:221-230`

**Auth detection**:
- `CODES_REQUIRING_LOGIN: [u16; 2] = [403, 401]` — `src/todoist/request.rs:169`
- `requires_login(status_code)` — `src/todoist/request.rs:171-173`
- `is_pro_plan_url(url)` checks if URL contains any of `PRO_PLAN_URLS: [&str; 1] = [REMINDERS_URL]` — `src/todoist/request.rs:169, 176-178`

**Key detail**: The `source` field for errors from `handle_response` is always `"reqwest"` (lines 214, 219, 224), even though the actual errors come from `response.text().await?` (which wraps `reqwest::Error`).

## Q4: How does `process_task` present options and dispatch to spawned async tasks? What input constants exist?

### Findings

**`process_task`** — `src/tasks/mod.rs:365-414`: Takes `comments, config, task, task_count, with_project`. Options array:
```rust
[COMPLETE, SKIP, SCHEDULE, COMMENT, REMIND, DELETE, QUIT]
```
Prints formatted task with comments, completion count, and remaining count. Calls `input::select(input::OPTION, options, config.mock_select)` then matches:

| Option | Action | Lines |
|---|---|---|
| `COMPLETE` | Saves config, `spawn_complete_task` | 380-384 |
| `DELETE` | `spawn_delete_task` | 385 |
| `COMMENT` | Prompts `input::string(CONTENT, ...)`, `spawn_comment_task` | 386-389 |
| `REMIND` | Prompts `input::string(DATE_AND_TIME, ...)`, `spawn_create_reminder` | 391-394 |
| `SCHEDULE` | `input::date()`, `spawn_update_task_due` | 396-400 |
| `SKIP` | Spawns empty `tokio::spawn(async move {})` | 403 |
| `QUIT` | Returns `None` | 404 |

**Spawn functions** — All at `src/tasks/mod.rs:484-580`, follow identical pattern:
```rust
pub fn spawn_*(config: Config, ...) -> JoinHandle<()> {
    tokio::spawn(async move {
        if let Err(e) = todoist::*(&config, ...).await {
            let _ = config.tx().send(e);
        }
    })
}
```
Error channel (`config.tx()`) sends errors to the main event loop for async display.

**`timebox_task`** — `src/tasks/mod.rs:417-452`: Similar pattern with options `[TIMEBOX, COMPLETE, SKIP, DELETE, QUIT]`.

**Input constants** — `src/input.rs`:
- **Text prompts** (lines 10-26): `CONTENT`, `DESCRIPTION`, `NAME`, `FILTER`, `PATH`, `DATE`, `TIME`, `DATE_AND_TIME`, `DURATION`
- **Select prompts** (lines 28-41): `ATTRIBUTES`, `PROJECT`, `LABELS`, `SECTION`, `PRIORITY`, `OPTION`, `SELECT_DATE`, `TASK`
- **Option values** (lines 43-63): `NAT_LANG`, `NO_DATE`, `COMPLETE`, `REMIND`, `TIMEBOX`, `COMMENT`, `SKIP`, `DELETE`, `CANCEL`, `QUIT`, `SCHEDULE`
- **`DateTimeInput` enum** (lines 65-74): `Skip`, `None`, `Complete`, `Text(String)`

## Q5: How are CLI commands dispatched for task operations? How does `TaskCommands::Comment` work?

### Findings

**Dispatch chain** — `src/commands/mod.rs`:
1. `select_command` (line 143) matches `Commands::Task(command)` → calls `task_command(command, &cli, &tx)` (line 157)
2. `task_command` (lines 244-275) matches each `TaskCommands` variant and delegates to handler functions in `task_commands.rs`

**`TaskCommands` enum** — `src/commands/task_commands.rs:14-47`:
```rust
pub enum TaskCommands {
    QuickAdd(QuickAdd),  // alias "q"
    Create(Create),      // alias "c"
    Edit(Edit),          // alias "e"
    Next(Next),          // alias "n"
    Complete(Complete),  // alias "o"
    Comment(Comment),    // alias "m"
}
```
Each variant wraps a `#[derive(Parser)]` struct. Single-letter aliases match the parent command's alias (`tod t m` = task comment).

**`Comment` handler** — `src/commands/task_commands.rs:245-260`:
1. Retrieves task from `config.next_task()` (set by `Next` command)
2. If no next task → `Error::new("task_comment", "There is nothing to comment on...")`
3. Fetches content via `super::fetch_string(content.as_deref(), &config, input::CONTENT)` — accepts CLI arg or prompts interactively, errors in JSON mode if not provided
4. Calls `todoist::create_comment(&config, &task.id, &content, true)` with spinner
5. Returns JSON (via `serde_json::to_string(&comment)`) or green string `"Comment created successfully"`

**Pattern for adding new task subcommands**:
1. Add variant to `TaskCommands` enum with `#[clap(alias = "x")]` and a `#[derive(Parser)]` args struct
2. Add async handler function in `task_commands.rs` following signature `pub async fn handler(config: Config, args: &NewArgs, json: bool) -> Result<String, Error>`
3. Add match arm in `task_command` (mod.rs:244) calling the handler and wrapping with `build_command_result`

**`fetch_string` helper** — `src/commands/mod.rs:393-403`: Returns provided `Option<&str>` or prompts interactively. Errors in JSON mode when no value provided (source: `"json_mode"`).

## Q6: How does the test infrastructure mock Todoist API endpoints? What comment fixtures exist?

### Findings

**Test module structure** — `src/test/`:
- `mod.rs` — module declarations, `today_date()` helper at line 7
- `responses.rs` — `ResponseFromFile` enum for loading JSON fixtures
- `fixtures.rs` — `Config`, `Task`, `Comment`, `Project`, `Section`, `Reminder`, `Label` constructors

**`ResponseFromFile` enum** — `src/test/responses.rs:7-31`: 20 variants mapping to JSON files in `tests/responses/`. Relevant to comments: `Comment`, `CommentsAllTypes`. Other notable variants: `TodayTask`, `TodayTasks`, `Task`, `Project`, `Projects`, `Section`, `Sections`, `Reminder`, `Reminders`, `Label`, `Labels`, `User`, `AccessToken`.

**`read()` method** — `src/test/responses.rs:34-41`: Reads `tests/responses/{self}.json`, then calls `replace_values()` for dynamic substitution. `TodayTask`/`TodayTasks`/`UnscheduledTasks`/`TodayTasksWithoutDuration` replace `INSERTDATE` with today's date. `Versions` replaces `INSERTVERSION`.

**Mock setup pattern** (from `src/todoist/mod.rs` tests):
```rust
let mut server = mockito::Server::new_async().await;
let mock = server
    .mock("POST", "/api/v1/tasks/quick")
    .with_status(200)
    .with_header("content-type", "application/json")
    .with_body(ResponseFromFile::TodayTask.read().await)
    .create_async()
    .await;
let config = test::fixtures::config().await.with_mock_url(server.url());
// ... call function under test ...
mock.assert();  // or mock.assert_async().await;
```

**Body matching** (from `src/todoist/mod.rs` tests):
- Regex: `.match_body(mockito::Matcher::Regex(r#""parent_id":"999""#.to_string()))` — line 607
- AllOf: `.match_body(mockito::Matcher::AllOf(vec![...]))` — line 642

**Config fixture** — `src/test/fixtures.rs:96-108`: Creates `Config` with mock channel (`tokio::sync::mpsc::unbounded_channel`), temp config file, token `"alreadycreated"`, one project (`project()` fixture), `FixedTimeProvider`, timezone `"America/Vancouver"`. Chained with `.with_mock_url(server.url())` in tests.

**Comment fixtures**:
- `tests/responses/Comment.json` — single comment with `content`, `id`, `posted_at`, `item_id`, `is_deleted`, `attachment` (file type)
- `tests/responses/CommentsAllTypes.json` — 8 comments: file, video, image, URL, short URL, rich embed, plain (no attachment), and one deleted (`is_deleted: true`). Used by `test_all_comments_filters_deleted` at `src/todoist/mod.rs:876`
- `src/test/fixtures.rs:comment()` (lines 147-159) — returns `Comment` struct with hardcoded values (id `"2992679862"`, content `"Need one bottle of milk"`, no attachment)

**Test for create_comment** — `src/todoist/mod.rs:713-726`: Mocks `POST /api/v1/comments/` with status 200 and `ResponseFromFile::Comment`, asserts returned `Comment` matches fixture.

**Tests for 204 status** — `src/todoist/mod.rs:786-800` (`test_update_task_priority`): Mocks status 204 but still provides a body. `src/todoist/mod.rs:992-1002` (`test_archive_project_hits_api`): Mocks 204 with no body at all — this is the only test that exercises a true 204 empty-body scenario.

## Cross-Cutting Observations

- **Consistent error source convention**: `handle_response` always uses `"reqwest"` as error source. Upstream callers use descriptive sources like `"fetch_project"`, `"json_mode"`, `"task_comment"`.
- **Spinner control**: The `spinner: bool` parameter flows through every API function to `post_todoist`/`delete_todoist`/`get_todoist`. `true` for interactive operations, `false` for batch/background operations.
- **No comment mutation functions exist**: Only `create_comment` (`src/todoist/mod.rs:525-536`) and `all_comments` (`src/todoist/mod.rs:549-578`) exist. No `update_comment` or `delete_comment`.
- **Spawn pattern**: All async operations in `src/tasks/mod.rs` use `tokio::spawn` with error forwarding via `config.tx().send(e)`. The caller gets `Option<JoinHandle<()>>` — `None` signals quit/skip.
- **JSON mode guard**: `fetch_string` and similar functions in `src/commands/mod.rs` error with `"json_mode"` source when no CLI arg is provided and JSON mode is active. Interactive prompts are blocked.

## Open Areas

- **Comment update API response shape**: Cannot confirm from codebase whether the Todoist API returns the full `Comment` object on update. Would need to consult Todoist REST API docs or test against live API.
- **Comment delete API response**: Similarly unknown — some APIs return 204 No Content, others return the deleted object.
- **`attachment` vs `file_attachment` field name**: The JSON fixture uses `"attachment"` but the Rust struct uses `file_attachment`. The `#[serde(untagged)]` on `Attachment` may handle this via serde's `#[serde(rename)]` or there may be undocumented behavior. A live API test would confirm deserialization works.
