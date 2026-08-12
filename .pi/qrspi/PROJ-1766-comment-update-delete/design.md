# Design Discussion

## Current State

The comment system today is create-only. What exists (all refs from `research.md`):

- **Data model**: `Comment` struct in `src/comments.rs:9-18` with `id`, `content`, `posted_uid`, `posted_at`, `item_id`, `is_deleted`, `reactions`, `file_attachment`. `from_json` at line 19 deserializes from API responses.
- **API client**: `create_comment` at `src/todoist/mod.rs:525-536` (POST `/api/v1/comments/`) and `all_comments` at line 549-578 (GET with task_id filter). No update or delete functions exist.
- **Interactive workflow**: `process_task` at `src/tasks/mod.rs:524-583` shows `[COMPLETE, SKIP, SCHEDULE, COMMENT, REMIND, DELETE, QUIT]`. COMMENT (lines 553-557) prompts for content and spawns a create. Comments are already fetched and passed in as `comments: Vec<Comment>`.
- **Spawn pattern**: `spawn_comment_task` at `src/tasks/mod.rs:794-801` fires a `tokio::spawn` that calls `create_comment` with `spinner: false`, forwarding errors via `config.tx()`.
- **CLI dispatch**: `TaskCommands::Comment` at `src/commands/task_commands.rs:14-47` with alias `"m"`. Handler at lines 329-344 requires a "next" task, fetches content, calls `create_comment`, returns JSON or green string.
- **HTTP layer**: `delete_todoist` at `src/todoist/request.rs:108-138` exists and is used only by `delete_task`. `handle_response` at lines 197-231 treats all 2xx identically — 204 returns `Ok("")`.
- **Tests**: `create_comment` test at `src/todoist/mod.rs:713-726` mocks POST with `ResponseFromFile::Comment`. Test fixtures: `tests/responses/Comment.json` (single), `tests/responses/CommentsAllTypes.json` (8 variants). No comment mutation test fixtures.

## Desired End State

Users can **edit** and **delete** comments both from the interactive task processing loop and from the CLI. Verification:

1. **`update_comment`** in `src/todoist/mod.rs` — POST `/api/v1/comments/{id}` with new content, returns parsed `Comment`. Test covers happy path with mock fixture.
2. **`delete_comment`** in `src/todoist/mod.rs` — DELETE `/api/v1/comments/{id}`, returns `Ok("✓")`. Test covers 204 No Content response.
3. **Interactive submenu**: Selecting COMMENT in `process_task` shows existing comments (formatted one-liners) + "New comment" option. Selecting an existing comment shows Edit/Delete/Back.
4. **Spawn functions**: `spawn_update_comment` and `spawn_delete_comment` follow the existing `spawn_*` pattern with error forwarding.
5. **CLI commands**: `TaskCommands::EditComment` (alias TBD) and `TaskCommands::DeleteComment` (alias TBD) with args structs, handlers in `src/commands/task_commands.rs`, and dispatch arms in `src/commands/mod.rs`.
6. **`docs/usage.md`** updated with new command examples.

## Patterns to Follow

### ✅ Good patterns (match these)

| Pattern | Reference | Notes |
|---|---|---|
| Single-field POST body with `json!({...})` | `src/todoist/mod.rs:368` (`update_task_content`) | Use for `update_comment` body: `json!({"content": new_content})` |
| Parse response into type | `src/todoist/mod.rs:528-536` (`create_comment`) | `update_comment` returns `Result<Comment, Error>` |
| Discard response, return `"✓"` | `src/todoist/mod.rs:416-421` (`delete_task`) | `delete_comment` returns `Result<String, Error>` |
| URL: `format!("{COMMENTS_URL}{id}")` | `src/todoist/mod.rs:370` (task pattern) | Simple resource URL, no sub-path |
| `delete_todoist` for DELETE endpoints | `src/todoist/mod.rs:416` | Already wired through `handle_response`, handles 204 |
| `spinner: bool` parameter | All API functions in `src/todoist/mod.rs` | `true` for CLI, `false` for spawned background ops |
| Spawn with error channel | `src/tasks/mod.rs:794-801` (`spawn_comment_task`) | `tokio::spawn` + `config.tx().send(e)` on error |
| `input::select` for picking from list | `src/tasks/mod.rs:548` (options menu) | Use for comment selection and edit/delete submenu |
| `fetch_string` for CLI-or-prompt content | `src/commands/mod.rs:393-403` | `EditComment` handler uses this for new content |
| `#[derive(Parser)]` args struct + handler fn | `src/commands/task_commands.rs:118-122, 329-344` | New `EditComment` and `DeleteComment` structs + handlers |
| Mock with `mockito::Server`, `ResponseFromFile` | `src/todoist/mod.rs:713-726` | Tests for both new API functions |
| `// Does not pass back a task` comment | `src/todoist/mod.rs:370` | Use `// Does not pass back a comment` for delete |

### ❌ Patterns to avoid

| Pattern | Reference | Why avoid |
|---|---|---|
| `HashMap<String, Value>` for single-field updates | `src/todoist/mod.rs:163-190` (`create_task`) | Over-engineered for single-field updates; use `json!({...})` |
| Pre-applying `format::*_string` in error messages | `src/errors.rs` Display impl | The Display impl owns coloring; callers must not pre-color |

## Design Decisions

1. **COMMENT submenu UX**: When COMMENT is selected in `process_task`, show a submenu: existing comments (formatted as one-line summaries via `input::select`) + a "New comment" entry. If "New comment" → prompt for content → spawn create. If existing comment selected → sub-submenu with Edit / Delete / Back.

2. **`update_comment` returns `Comment`**: Parse the API response into the `Comment` struct (Pattern A), matching `create_comment` at `src/todoist/mod.rs:528-536`. This enables JSON output in CLI mode. **Risk**: the Todoist API response shape for comment update is unconfirmed from the codebase — flag for manual live testing after implementation.

3. **`delete_comment` uses `delete_todoist`, expects 204**: Follow the `delete_task` pattern at `src/todoist/mod.rs:416-421`. Call `delete_todoist`, discard the empty response, return `Ok("✓")`. `handle_response` at `src/todoist/request.rs:208-211` already handles 204 gracefully.

4. **New `TaskCommands` variants**: Add `EditComment` (with `comment_id` and `content` args) and `DeleteComment` (with `comment_id` arg) to the `TaskCommands` enum at `src/commands/task_commands.rs:14-47`. Each gets a handler function and a dispatch arm in `task_command`. Single-letter aliases TBD that don't collide with existing: `c` (Create), `e` (Edit), `n` (Next), `o` (Complete), `m` (Comment).

5. **Comment selection via `input::select`**: Format each comment as `"{content_truncated} — {posted_at}"` (first ~60 chars of content) and use the existing `input::select` pattern (`src/input.rs:37`). Consistent with how tasks, projects, and labels are selected throughout the interactive workflow. No ownership filtering.

6. **No confirmation for delete**: Deleting a task in `process_task` (`src/tasks/mod.rs:552`) is immediate with no confirmation prompt. Delete comment follows the same pattern for consistency.

7. **Content prompt for edit reuses `input::CONTENT`**: When editing a comment, prompt for new content using the existing `input::string(input::CONTENT, ...)` constant (`src/input.rs:11`), same as comment creation.

8. **Spawn for interactive, direct for CLI**: In the interactive workflow, comment update/delete use `tokio::spawn` with error forwarding (matching `spawn_comment_task`). In CLI handlers, calls are awaited directly with `spinner: true` (matching the existing `comment` handler at `src/commands/task_commands.rs:334`).

## What We're NOT Doing

- **No comment reactions management** — the `reactions` field on `Comment` is read-only through this feature
- **No file attachment editing** — attachments are immutable after creation
- **No batch comment operations** — edit/delete one comment at a time
- **No comment search or filtering** — selection is limited to the comments already fetched for the current task
- **No ownership-based filtering** — the backend enforces authorization; we don't pre-filter by `posted_uid`
- **No comment edit/delete in `timebox_task`** — `timebox_task` at `src/tasks/mod.rs:417-452` has its own menu; out of scope
- **No comment edit via `update_task`** — the attribute editing loop at `src/tasks/mod.rs:445-492` doesn't handle comments

## Open Risks

- **`update_comment` response shape unconfirmed**: The codebase has no existing comment-update call to learn from. The Todoist REST API may return the full `Comment` object (assumed), a subset, or 204 with no body. Must be verified with a live API call after implementation. If the shape differs, the `from_json` deserialization will fail at runtime.

- **Comment ownership enforcement**: The Todoist API may reject (403/404) attempts to edit or delete comments posted by other users. `handle_response` at `src/todoist/request.rs:212-216` treats 401/403 as auth errors and prompts re-login — if the API returns 403 for ownership failures, the error message will be misleading ("Run 'tod auth login' to reauthenticate").

- **CLI alias collisions**: `TaskCommands` aliases are single letters. Existing: `q` (QuickAdd), `c` (Create), `e` (Edit), `n` (Next), `o` (Complete), `m` (Comment). Available single letters are limited. If no clean alias fits, skip the alias for the new commands.

- **Empty comments list edge case**: If the task has no comments, the COMMENT submenu should skip straight to "New comment" prompt rather than showing an empty select list. This avoids an extra keystroke for the common case.
