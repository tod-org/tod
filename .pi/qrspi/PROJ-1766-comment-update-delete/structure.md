# Structure Outline

## Approach

Add `update_comment` and `delete_comment` to the Todoist API client, then expose them through both the interactive `process_task` COMMENT submenu and new CLI subcommands (`edit-comment`, `delete-comment`). Each phase crosses all layers needed for its slice — API, UI prompts, dispatch, and tests.

---

## Phase 1: API client — `update_comment` and `delete_comment`

Delivers the two new API functions with mockito tests. No user-visible change yet, but the functions are callable and verified against the Todoist REST API contract.

**Files**: `src/todoist/mod.rs`

**Key changes**:
- `pub async fn update_comment(config: &Config, comment_id: &str, content: &str, spinner: bool) -> Result<Comment, Error>` — POST `/api/v1/comments/{id}` with `json!({"content": content})`, parses response via `Comment::from_json`. Follows `create_comment` (line 525) response pattern.
- `pub async fn delete_comment(config: &Config, comment_id: &str, spinner: bool) -> Result<String, Error>` — DELETE `/api/v1/comments/{id}` via `request::delete_todoist`, expects 204, returns `Ok("✓".into())`. Follows `delete_task` (line 416) pattern with `// Does not pass back a comment`.
- Test: `test_update_comment` — mock `POST /api/v1/comments/123` → 200 with `ResponseFromFile::Comment`, assert returned `Comment` matches fixture.
- Test: `test_delete_comment` — mock `DELETE /api/v1/comments/123` → 204 no body, assert returns `Ok("✓")`.
- New fixture (if needed): `tests/responses/Comment.json` already exists from `create_comment` tests; reuse it.

**Verify**:
```bash
cargo test update_comment delete_comment
```
- `update_comment` returns a parsed `Comment` struct matching the fixture.
- `delete_comment` returns `"✓"` on 204.

---

## Phase 2: Interactive comment submenu in `process_task`

User presses COMMENT → sees existing comments as a select list + "New comment" option. Selecting an existing comment shows Edit / Delete / Back. Selecting "New comment" prompts for content and creates. The full interactive flow is testable by mocking the select input.

**Files**: `src/tasks/mod.rs`, `src/input.rs`

**Key changes**:
- `fn format_comment_option(comment: &Comment, config: &Config) -> String` — helper: `"{truncated_content} — {posted_at_formatted}"` (first ~60 chars of `content`, timezone-adjusted `posted_at`). Used to build the `input::select` options list.
- `pub fn spawn_update_comment(config: Config, comment_id: String, content: String) -> JoinHandle<()>` — calls `todoist::update_comment(…, false)`, forwards errors via `config.tx()`.
- `pub fn spawn_delete_comment(config: Config, comment_id: String) -> JoinHandle<()>` — calls `todoist::delete_comment(…, false)`, forwards errors via `config.tx()`.
- `process_task` (line 524) COMMENT arm rewritten:
  ```
  input::COMMENT matched → build options from comments + "New comment"
    → input::select("Select a comment", options, …)
    → "New comment" → prompt content → spawn_comment_task (existing)
    → existing comment → sub-select ["Edit", "Delete", "Back"]
      → "Edit" → prompt content → spawn_update_comment
      → "Delete" → spawn_delete_comment
      → "Back" → return to comment list (loop)
  ```
- Edge case: empty comments list → skip the select, go straight to content prompt (existing create flow).

**Verify**:
```bash
cargo test process_task comment
```
- With mocked select returning "New comment" → spawns create.
- With mocked select returning an existing comment, then "Edit" → spawns update.
- With mocked select returning an existing comment, then "Delete" → spawns delete.
- With mocked select returning "Back" → returns to comment list.
- Empty comments list → skips select, goes directly to content prompt.

Manual: run `tod list process`, hit COMMENT on a task with comments — see formatted one-liners, select one, edit/delete it.

---

## Phase 3: CLI subcommands — `edit-comment` and `delete-comment`

`tod task edit-comment <id> <content>` and `tod task delete-comment <id>` work from the command line with JSON and human-readable output. Args structs, handlers, and dispatch arms.

**Files**: `src/commands/task_commands.rs`, `src/commands/mod.rs`

**Key changes**:
- `TaskCommands` enum additions:
  ```rust
  #[clap(alias = "ec")]
  /// Edit an existing comment's content
  EditComment(EditComment),

  #[clap(alias = "dc")]
  /// Delete a comment by ID
  DeleteComment(DeleteComment),
  ```
  Aliases `ec` / `dc` avoid collision with existing `e` (Edit task), `c` (Create task), `m` (Comment create), `d` (none).
- Args structs:
  ```rust
  #[derive(Parser, Debug, Clone)]
  pub struct EditComment {
      /// ID of the comment to edit
      comment_id: String,
      /// New content for the comment
      content: Option<String>,
  }

  #[derive(Parser, Debug, Clone)]
  pub struct DeleteComment {
      /// ID of the comment to delete
      comment_id: String,
  }
  ```
- Handler: `pub async fn edit_comment(config: Config, args: &EditComment, json: bool) -> Result<String, Error>` — fetches content via `super::fetch_string(args.content.as_deref(), …)`, calls `todoist::update_comment(…, true)`, returns JSON or green string `"Comment updated successfully"`.
- Handler: `pub async fn delete_comment(config: Config, args: &DeleteComment, json: bool) -> Result<String, Error>` — calls `todoist::delete_comment(…, true)`, returns JSON `"✓"` or green string `"Comment deleted successfully"`.
- Dispatch: two new match arms in `task_command` (`src/commands/mod.rs:244`):
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

**Verify**:
```bash
cargo test edit_comment delete_comment
```
- `tod task edit-comment 123 "new content"` → happy path returns `"Comment updated successfully"` / JSON.
- `tod task edit-comment 123` (no content, interactive) → prompts for content, then updates.
- `tod task delete-comment 123` → returns `"Comment deleted successfully"` / JSON `"✓"`.
- `tod task edit-comment 123 --json` with no content arg → errors with `"json_mode"` source (matching `fetch_string` guard).

Manual: `tod task edit-comment <real_id> "updated text"` against live API, verify in Todoist UI.

---

## Phase 4: Docs and polish

Update usage docs and handle the empty-comments edge case in `process_task` (if deferred from Phase 2).

**Files**: `docs/usage.md`, `src/tasks/mod.rs` (if edge case not handled in Phase 2)

**Key changes**:
- `docs/usage.md`: add `tod task edit-comment` and `tod task delete-comment` examples under the task commands section, with both interactive and JSON-mode variants.
- Empty comments edge case: if `comments.is_empty()` in the COMMENT arm, skip the select and go directly to content prompt (may already be done in Phase 2).

**Verify**:
```bash
# Confirm docs render correctly
cat docs/usage.md | grep -A3 "edit-comment\|delete-comment"
```

---

## Testing Checkpoints

After each phase, these must hold:

| Phase | What must be true |
|---|---|
| **1** | `update_comment` and `delete_comment` compile, pass mockito tests. `cargo test update_comment delete_comment` green. |
| **2** | COMMENT in `process_task` shows comment list + "New comment". Edit/Delete/Back submenu works. Spawn functions forward errors. `cargo test process_task` green (new + existing tests). |
| **3** | `tod t ec <id> <content>` and `tod t dc <id>` work with both text and JSON output. JSON mode rejects missing content arg. `cargo test edit_comment delete_comment` green. `scripts/test.sh` passes (format, clippy, no forbidden strings). |
| **4** | `docs/usage.md` has new command examples. Empty comments list handled gracefully. Full `scripts/test.sh` green. |
