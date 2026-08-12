# Implementation Plan

## Overview

Add `update_comment` and `delete_comment` to the Todoist API client, then expose them through the interactive `process_task` COMMENT submenu and new CLI subcommands (`edit-comment`, `delete-comment`).

---

## Phase 1: API client — `update_comment` and `delete_comment`

### Changes

#### 1. Add `update_comment` function
**File**: `src/todoist/mod.rs`
**Action**: modify — insert after `create_comment` (after line 536)

```rust
/// Updates the content of a comment by ID.
pub async fn update_comment(
    config: &Config,
    comment_id: &str,
    content: &str,
    spinner: bool,
) -> Result<Comment, Error> {
    let body = json!({"content": content});
    let url = format!("{COMMENTS_URL}{comment_id}");

    let response = request::post_todoist(config, &url, body, spinner).await?;
    Comment::from_json(&response)
}
```

#### 2. Add `delete_comment` function
**File**: `src/todoist/mod.rs`
**Action**: modify — insert after `update_comment`

```rust
/// Deletes a comment by ID. The Todoist API returns 204 No Content.
pub async fn delete_comment(
    config: &Config,
    comment_id: &str,
    spinner: bool,
) -> Result<String, Error> {
    let url = format!("{COMMENTS_URL}{comment_id}");

    request::delete_todoist(config, &url, json!({}), spinner).await?;
    // Does not pass back a comment
    Ok("✓".into())
}
```

#### 3. Add tests for both functions
**File**: `src/todoist/mod.rs`
**Action**: modify — add tests in the `#[cfg(test)] mod tests` block

Location: after the existing `test_create_comment` test (around line 726).

```rust
#[tokio::test]
async fn test_update_comment() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/api/v1/comments/123")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(ResponseFromFile::Comment.read().await)
        .create_async()
        .await;

    let config = test::fixtures::config()
        .await
        .with_mock_url(server.url());

    let comment = update_comment(&config, "123", "updated content", false)
        .await
        .expect("expected value or result, got None or Err");

    assert_eq!(comment.id, "2992679862");
    assert_eq!(comment.content, "Need one bottle of milk");
    mock.assert();
}

#[tokio::test]
async fn test_delete_comment() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("DELETE", "/api/v1/comments/123")
        .with_status(204)
        .create_async()
        .await;

    let config = test::fixtures::config()
        .await
        .with_mock_url(server.url());

    let result = delete_comment(&config, "123", false)
        .await
        .expect("expected value or result, got None or Err");

    assert_eq!(result, "✓");
    mock.assert();
}
```

### Verification
#### Automated
- [x] `cargo test update_comment delete_comment` passes
- [x] `cargo test test_update_comment -- --nocapture` — `update_comment` returns parsed `Comment` matching `Comment.json` fixture
- [x] `cargo test test_delete_comment -- --nocapture` — `delete_comment` returns `"✓"` on 204

#### Manual
- [ ] (After full implementation) `tod task edit-comment <real_id> "test"` against live API to confirm response shape

---

## Phase 2: Interactive comment submenu in `process_task`

### Changes

#### 1. Upgrade `mock_select` to support sequences
**File**: `src/config/mod.rs`
**Action**: modify

Change the `mock_select` field from `Option<usize>` to `RefCell<Vec<usize>>` so tests can drive multiple sequential `input::select` calls. Skip serialization since this is test-only.

**Struct field change** (line 91):
```rust
// Before:
pub mock_select: Option<usize>,
// After:
#[serde(skip)]
pub mock_select: RefCell<Vec<usize>>,
```

Add `use std::cell::RefCell;` to imports at top of file.

**Default value change** (line 456, in `Default` impl):
```rust
// Before:
mock_select: None,
// After:
mock_select: RefCell::new(Vec::new()),
```

**Test Config builder change** (lines 732-736, in test `Config::default_test`):
```rust
// Before:
mock_select: None,
// After:
mock_select: RefCell::new(Vec::new()),
```

**Test Config builder change** (lines 788-789, in test `Config::default_test` no-config variant):
```rust
// Before:
mock_select: None,
// After:
mock_select: RefCell::new(Vec::new()),
```

**Builder method** (lines 814-817): change to accept `Vec<usize>`:
```rust
pub fn mock_selects(self, selects: Vec<usize>) -> Config {
    Config {
        mock_select: RefCell::new(selects),
        ..self
    }
}
```

Also add a single-value convenience:
```rust
pub fn mock_select(self, index: usize) -> Config {
    Config {
        mock_select: RefCell::new(vec![index]),
        ..self
    }
}
```

**`mock_select` field omission** in struct destructuring patterns throughout config/mod.rs — wherever `mock_select` appears in a destructure (like line 551), replace usage with `&self.mock_select`.

**File**: `src/input.rs`
**Action**: modify

Change `select` and `select_with_cursor_index` to accept `&RefCell<Vec<usize>>` instead of `Option<usize>`. Add `use std::cell::RefCell;` to imports.

```rust
pub fn select<T: Display>(
    desc: &str,
    options: Vec<T>,
    mock_selects: &RefCell<Vec<usize>>,
) -> Result<T, Error> {
    select_with_cursor_index(desc, options, 0, mock_selects)
}

pub fn select_with_cursor_index<T: Display>(
    desc: &str,
    options: Vec<T>,
    cursor_index: usize,
    mock_selects: &RefCell<Vec<usize>>,
) -> Result<T, Error> {
    if cfg!(test) {
        let mut selects = mock_selects.borrow_mut();
        if selects.is_empty() {
            panic!("mock_select ran out of values — need more entries for this test");
        }
        let index = selects.remove(0);
        Ok(options
            .into_iter()
            .nth(index)
            .expect("Must provide a vector of options"))
    } else {
        Select::new(desc, options)
            .with_page_size(page_size() / 2)
            .with_starting_cursor(cursor_index)
            .prompt()
            .map_err(Error::from)
    }
}
```

**File**: `src/input.rs` — `multi_select`
**Action**: modify — same signature change from `Option<usize>` to `&RefCell<Vec<usize>>`, pop one value.

**All callers** of `input::select`, `input::select_with_cursor_index`, `input::multi_select`:
**Action**: modify — change argument from `config.mock_select` to `&config.mock_select`. Affected files:
- `src/tasks/mod.rs` (8 call sites)
- `src/lists.rs` (8 call sites)
- `src/sections.rs` (2 call sites)
- `src/config/mod.rs` (internal `input::bool` calls also use mock_select — these need updating too)

For `input::bool` (in `src/input.rs`): change `mock_select: Option<usize>` param to `mock_selects: &RefCell<Vec<usize>>`, pop one value.

**Test code** using `.mock_select(N)` — these continue to work because `.mock_select(3)` now wraps as `RefCell::new(vec![3])`. For tests needing multiple selects, use `.mock_selects(vec![3, 0, 1])`.

#### 2. Add `format_comment_option` helper
**File**: `src/tasks/mod.rs`
**Action**: modify — add function near `spawn_comment_task`

```rust
/// Formats a comment as a one-line summary for display in select menus.
fn format_comment_option(comment: &Comment, config: &Config) -> String {
    let truncated = if comment.content.len() > 60 {
        format!("{}…", &comment.content[..60])
    } else {
        comment.content.clone()
    };
    // Use a simple date parse (timezone not critical for a selection label)
    match time::parse_and_format_posted_at(&comment.posted_at, config) {
        Ok(formatted) => format!("{truncated} — {formatted}"),
        Err(_) => format!("{truncated} — {}", &comment.posted_at[..10]),
    }
}
```

Note: check what `time::` functions are available for formatting `posted_at`. The `Comment::fmt` method uses `time::datetime_from_str` + `time::datetime_to_string`. Use `time::naive_datetime_from_str` (or equivalent) for a simpler parse. If no simple function exists, inline: parse the date portion `&comment.posted_at[..10]` and use that directly.

**Adjustment**: look at how `Comment::fmt` in `src/comments.rs:105+` formats the date. Use the same pattern:
```rust
fn format_comment_option(comment: &Comment, config: &Config) -> String {
    let truncated = if comment.content.len() > 60 {
        format!("{}…", &comment.content[..60])
    } else {
        comment.content.clone()
    };
    let formatted_date = time::datetime_from_str(&comment.posted_at, time::timezone_from_str("UTC").unwrap())
        .map(|dt| time::datetime_to_string(&dt, config).unwrap_or_default())
        .unwrap_or_else(|_| comment.posted_at[..10].to_string());
    format!("{truncated} — {formatted_date}")
}
```

#### 3. Add `spawn_update_comment` and `spawn_delete_comment`
**File**: `src/tasks/mod.rs`
**Action**: modify — add after `spawn_comment_task` (line 801)

```rust
/// Updates a comment inside another thread
pub fn spawn_update_comment(config: Config, comment_id: String, content: String) -> JoinHandle<()> {
    tokio::spawn(async move {
        if let Err(e) = todoist::update_comment(&config, &comment_id, &content, false).await {
            let _ = config.tx().send(e);
        }
    })
}

/// Deletes a comment inside another thread
pub fn spawn_delete_comment(config: Config, comment_id: String) -> JoinHandle<()> {
    tokio::spawn(async move {
        if let Err(e) = todoist::delete_comment(&config, &comment_id, false).await {
            let _ = config.tx().send(e);
        }
    })
}
```

#### 4. Rewrite COMMENT arm in `process_task`
**File**: `src/tasks/mod.rs`
**Action**: modify — replace lines 553-557 (the current COMMENT match arm) with the full submenu logic:

```rust
input::COMMENT => {
    if comments.is_empty() {
        // No existing comments — go straight to new comment prompt
        let content = input::string(CONTENT, config.mock_string.clone())?;
        return Ok(Some(spawn_comment_task(config.clone(), task.id, content)));
    }

    // Build options: each existing comment + "New comment"
    let mut comment_options: Vec<String> = comments
        .iter()
        .map(|c| format_comment_option(c, config))
        .collect();
    comment_options.push("New comment".to_string());

    let selected = input::select(
        "Select a comment",
        comment_options,
        &config.mock_select,
    )?;

    if selected == "New comment" {
        let content = input::string(CONTENT, config.mock_string.clone())?;
        return Ok(Some(spawn_comment_task(config.clone(), task.id, content)));
    }

    // User picked an existing comment — find it by matching the formatted string
    let comment_index = comment_options
        .iter()
        .position(|opt| *opt == selected)
        .expect("selected option must exist");
    let comment = &comments[comment_index];

    // Sub-submenu: Edit / Delete / Back
    let action_options = vec!["Edit", "Delete", "Back"];
    let action = input::select(
        "Choose an action",
        action_options,
        &config.mock_select,
    )?;

    match action {
        "Edit" => {
            let content = input::string(CONTENT, config.mock_string.clone())?;
            Ok(Some(spawn_update_comment(
                config.clone(),
                comment.id.clone(),
                content,
            )))
        }
        "Delete" => Ok(Some(spawn_delete_comment(
            config.clone(),
            comment.id.clone(),
        ))),
        "Back" => {
            // Recurse to show comment list again (loop)
            // Use a simple approach: return a skip handle to avoid nesting
            Ok(Some(tokio::spawn(async move {})))
        }
        _ => unreachable!(),
    }
}
```

Note: The "Back" option returns a no-op spawn handle rather than recursively calling `process_task` to keep the control flow simple. The caller (`process` in `src/lists.rs`) continues iterating, and the user can select COMMENT again on the next iteration.

#### 5. Add comment option constants
**File**: `src/input.rs`
**Action**: modify — add comment submenu constants (optional; strings can be inline since they're only used once):

If adding constants, insert near line 43-63 alongside existing option constants:
```rust
/// Comment submenu option: edit.
pub const EDIT_COMMENT: &str = "Edit";
/// Comment submenu option: delete.
pub const DELETE_COMMENT: &str = "Delete";
/// Comment submenu option: go back.
pub const BACK: &str = "Back";
/// Comment submenu option: create new comment.
pub const NEW_COMMENT: &str = "New comment";
```

Then use these constants in the COMMENT arm instead of string literals.

### Verification
#### Automated
- [x] `cargo test process_task` passes — existing tests still green with upgraded mock_select infrastructure
- [x] `cargo test spawn_update_comment spawn_delete_comment` passes (if unit-testing spawn functions — note: they fire-and-forget so it's hard to assert; rely on integration-level process_task tests)
- [x] New tests for COMMENT submenu flows:
  - [x] Empty comments: `mock_selects(vec![3])` + `mock_string("test")` → spawns create (COMMENT at index 3 in process menu)
  - [x] One comment + "New comment" selected: `mock_selects(vec![3, 1])` + `mock_string("new")` → spawns create (index 1 = "New comment" in comment list)
  - [x] One comment → Edit: `mock_selects(vec![3, 0, 0])` + `mock_string("updated")` → spawns update (index 0 = existing comment, then index 0 = Edit)
  - [x] One comment → Delete: `mock_selects(vec![3, 0, 1])` → spawns delete (index 0 = existing comment, then index 1 = Delete)
  - [x] One comment → Back: `mock_selects(vec![3, 0, 2])` → returns no-op spawn (index 0 = existing comment, then index 2 = Back)

#### Manual
- [ ] `tod list process` → hit COMMENT on a task with comments → see formatted one-liners, select one, edit/delete it
- [ ] Task with no comments → COMMENT → goes straight to content prompt
- [ ] Select "Back" → returns to task process loop

---

## Phase 3: CLI subcommands — `edit-comment` and `delete-comment`

### Changes

#### 1. Add `EditComment` and `DeleteComment` args structs
**File**: `src/commands/task_commands.rs`
**Action**: modify — add after existing `Comment` struct (line 47)

```rust
#[derive(Parser, Debug, Clone)]
/// (ec) Edit an existing comment's content
pub struct EditComment {
    /// ID of the comment to edit
    comment_id: String,

    /// New content for the comment
    content: Option<String>,
}

#[derive(Parser, Debug, Clone)]
/// (dc) Delete a comment by ID
pub struct DeleteComment {
    /// ID of the comment to delete
    comment_id: String,
}
```

#### 2. Add variants to `TaskCommands` enum
**File**: `src/commands/task_commands.rs`
**Action**: modify — add before the closing `}` of `TaskCommands` enum (line 47):

```rust
#[clap(alias = "ec")]
/// (ec) Edit an existing comment's content
EditComment(EditComment),

#[clap(alias = "dc")]
/// (dc) Delete a comment by ID
DeleteComment(DeleteComment),
```

#### 3. Add handler functions
**File**: `src/commands/task_commands.rs`
**Action**: modify — add after the `comment` handler function (line 344):

```rust
/// Edits the content of an existing comment by ID.
pub async fn edit_comment(
    config: Config,
    args: &EditComment,
    json: bool,
) -> Result<String, Error> {
    let EditComment {
        comment_id,
        content,
    } = args;

    let content = super::fetch_string(content.as_deref(), &config, input::CONTENT)?;
    let comment = todoist::update_comment(&config, comment_id, &content, true).await?;

    if json {
        Ok(serde_json::to_string(&comment)?)
    } else {
        Ok(format::green_string("Comment updated successfully"))
    }
}

/// Deletes a comment by ID.
pub async fn delete_comment(
    config: Config,
    args: &DeleteComment,
    json: bool,
) -> Result<String, Error> {
    let result = todoist::delete_comment(&config, &args.comment_id, true).await?;

    if json {
        Ok(serde_json::to_string(&result)?)
    } else {
        Ok(format::green_string("Comment deleted successfully"))
    }
}
```

#### 4. Add dispatch arms in `task_command`
**File**: `src/commands/mod.rs`
**Action**: modify — add after the `TaskCommands::Comment` arm (lines 287-291):

```rust
TaskCommands::EditComment(args) => {
    let config = fetch_config(cli, tx).await?;
    let result = task_commands::edit_comment(config.clone(), args, cli.json).await;
    Ok(build_command_result(result, &config))
}
TaskCommands::DeleteComment(args) => {
    let config = fetch_config(cli, tx).await?;
    let result = task_commands::delete_comment(config.clone(), args, cli.json).await;
    Ok(build_command_result(result, &config))
}
```

#### 5. Add tests
**File**: `src/commands/task_commands.rs`
**Action**: modify — add tests in the `#[cfg(test)] mod tests` block, after existing tests:

```rust
#[tokio::test]
async fn edit_comment_happy_path() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/api/v1/comments/123")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(ResponseFromFile::Comment.read().await)
        .create_async()
        .await;

    let config = test::fixtures::config()
        .await
        .with_mock_url(server.url());

    let args = EditComment {
        comment_id: "123".to_string(),
        content: Some("new content".to_string()),
    };
    let result = edit_comment(config, &args, false).await;
    assert!(result.is_ok(), "edit_comment should succeed; got: {result:?}");
    assert!(result.unwrap().contains("Comment updated successfully"));
    mock.assert();
}

#[tokio::test]
async fn edit_comment_json_output() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/api/v1/comments/123")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(ResponseFromFile::Comment.read().await)
        .create_async()
        .await;

    let config = test::fixtures::config()
        .await
        .with_mock_url(server.url());

    let args = EditComment {
        comment_id: "123".to_string(),
        content: Some("new content".to_string()),
    };
    let result = edit_comment(config, &args, true).await;
    assert!(result.is_ok());
    let json = result.unwrap();
    assert!(json.contains("\"id\""));
    assert!(json.contains("2992679862"));
    mock.assert();
}

#[tokio::test]
async fn edit_comment_json_mode_no_content_errors() {
    let mut config = test::fixtures::config().await;
    config.args.json = true;

    let args = EditComment {
        comment_id: "123".to_string(),
        content: None,
    };
    let result = edit_comment(config, &args, true).await;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().source, "json_mode");
}

#[tokio::test]
async fn delete_comment_happy_path() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("DELETE", "/api/v1/comments/123")
        .with_status(204)
        .create_async()
        .await;

    let config = test::fixtures::config()
        .await
        .with_mock_url(server.url());

    let args = DeleteComment {
        comment_id: "123".to_string(),
    };
    let result = delete_comment(config, &args, false).await;
    assert!(result.is_ok(), "delete_comment should succeed; got: {result:?}");
    assert!(result.unwrap().contains("Comment deleted successfully"));
    mock.assert();
}

#[tokio::test]
async fn delete_comment_json_output() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("DELETE", "/api/v1/comments/123")
        .with_status(204)
        .create_async()
        .await;

    let config = test::fixtures::config()
        .await
        .with_mock_url(server.url());

    let args = DeleteComment {
        comment_id: "123".to_string(),
    };
    let result = delete_comment(config, &args, true).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "\"✓\"");
    mock.assert();
}
```

### Verification
#### Automated
- [ ] `cargo test edit_comment delete_comment` passes — all 5 new tests green
- [ ] `cargo test test_update_comment test_delete_comment` — Phase 1 tests still green
- [ ] `cargo test process_task` — Phase 2 tests still green
- [ ] `scripts/test.sh` passes (format, clippy, no forbidden strings)

#### Manual
- [ ] `tod task edit-comment <real_comment_id> "updated text"` → success message
- [ ] `tod task delete-comment <real_comment_id>` → success message
- [ ] `tod task edit-comment <real_comment_id> --json "updated text"` → JSON output with comment object
- [ ] `tod task delete-comment <real_comment_id> --json` → `"✓"`
- [ ] `tod task edit-comment <real_comment_id> --json` (no content) → errors with `json_mode` source
- [ ] `tod t ec <id> "text"` and `tod t dc <id>` — aliases work
- [ ] Verify in Todoist UI that edited content appears / comment is deleted

---

## Phase 4: Docs and polish

### Changes

#### 1. Update usage docs
**File**: `docs/usage.md`
**Action**: modify — add examples under the task commands section (after `tod task comment` example around line 144):

```markdown
# Edit an existing comment by ID
tod task edit-comment 1234567890 "Updated comment text"

# Edit a comment with JSON output
tod task edit-comment 1234567890 "Updated comment text" --json

# Edit a comment (interactive prompt for new content)
tod task edit-comment 1234567890

# Delete a comment by ID
tod task delete-comment 1234567890

# Delete a comment with JSON output
tod task delete-comment 1234567890 --json
```

Also update the `tod task -h` help output block earlier in the doc (around line 46) to include `edit-comment` and `delete-comment`:
```markdown
  edit-comment  (ec) Edit an existing comment's content
  delete-comment (dc) Delete a comment by ID
```

#### 2. Verify empty-comments edge case
**File**: `src/tasks/mod.rs`
**Action**: verify — the empty-comments edge case (skip select, go straight to content prompt) is handled in Phase 2's COMMENT arm rewrite. Confirm it's working correctly.

### Verification
#### Automated
- [ ] `scripts/test.sh` passes — full pipeline (format, build, clippy, tests, forbidden strings)
- [ ] `grep -A3 "edit-comment\|delete-comment" docs/usage.md` — confirms docs mention new commands

#### Manual
- [ ] Review `docs/usage.md` for accuracy of examples

---

## Implementation Order

Phases must be implemented in order: 1 → 2 → 3 → 4. Each phase builds on the previous.

Within Phase 2, the mock_select infrastructure upgrade (item 1) must be done before the COMMENT arm rewrite (item 4) since the rewrite depends on it for tests.

---

## Testing Checkpoints

| Phase | What must be true |
|---|---|
| **1** | `update_comment` and `delete_comment` compile, pass mockito tests. `cargo test update_comment delete_comment` green. |
| **2** | COMMENT submenu works. Spawn functions forward errors. `cargo test process_task` green (existing + new tests). |
| **3** | `tod t ec <id> <content>` and `tod t dc <id>` work with text and JSON output. JSON mode rejects missing content arg. `cargo test edit_comment delete_comment` green. `scripts/test.sh` passes. |
| **4** | `docs/usage.md` has new command examples. Empty comments list handled gracefully. Full `scripts/test.sh` green. |

## Risks and Mitigations

| Risk | Mitigation |
|---|---|
| **`update_comment` API response shape differs from `Comment` struct** | The `test_update_comment` test verifies parsing against the existing `Comment.json` fixture. If live API returns a different shape, `from_json` will fail at runtime. Manual live test after Phase 3 confirms. |
| **Comment ownership — 403 from API** | `handle_response` maps 401/403 to re-auth error. If ownership failures return 403, the error message is misleading. Out of scope for this feature; the Todoist API may return 404 for unauthorized comment access. |
| **`mock_select` infrastructure change breaks existing tests** | All callers updated mechanically. Run `cargo test` after Phase 2 item 1 to catch any missed call sites. |
| **"Back" in comment submenu doesn't truly loop** | The current implementation returns a no-op spawn. The user re-selects COMMENT on the next process iteration. Acceptable for initial release. |
