# Structure Outline

## Approach

Document 79+ public API items with bare `///` one-liners in the codebase's dominant style, fix
typos and stale prose docs, add `#![warn(missing_docs)]` to prevent regression. Each phase targets
one reader-facing domain — after every phase, `cargo doc --no-deps` shows a complete, consistent
slice of the public API for that domain.

---

## Phase 1: Core Data Types

Document every struct, enum, and non-obvious field in the data model. Remove the dead `Body`
struct. After this phase, `cargo doc` shows a fully documented type layer — Task, Project, and all
their dependent types.

**Files**: `src/tasks/mod.rs`, `src/projects.rs`, `src/reminders.rs`

**Key changes**:
- `/// A task returned by the Todoist API.` — `Task` type-level doc
- `Task { ... }` — non-obvious field docs: bool flags (`is_today`, `is_overdue`, `is_recurring`,
  `is_collapsed`, `is_checked`, `is_deleted`, `in_history` — 7), nested struct fields
  (`deadline`, `duration`, `due` — 3), API-caveat fields (`note_count` exists, 13 others
  skipped for mapping directly to Todoist API names)
- `/// A date and time representation from the Todoist API.` — `DateInfo` type-level doc
- `DateInfo { timezone: ... }` — remaining undocumented field (5th of 5)
- `/// A duration attached to a task, used for timeboxing.` — `Duration` type-level doc
- `Duration { amount: u32, unit: Unit }` — 2 field docs
- `/// Unit of time for a task duration.` — `Unit` type-level + variant docs (`Minute`, `Day`)
- `/// Paginated wrapper for a list of tasks.` — `TaskResponse` type-level doc
- `/// An editable attribute of a task.` — `TaskAttribute` type-level + variant docs
  (`Content`, `Description`, `Due`, `Deadline`, `Priority`, `Labels`)
- `/// Controls prefix display in terminal output.` — `FormatType` type-level + variant docs
  (`List`, `Single`)
- `/// A project from the Todoist API.` — `Project` type-level doc
- `Project { ... }` — non-obvious field docs: bool flags (`is_favorite`, `is_shared`,
  `is_inbox_project`, `is_archived`, `is_deleted`, `is_collaborator` — 6), nested struct
  fields (`view_style`, `parent_id` via Option — 2). 10 fields mapping directly to API names skipped.
- `/// Paginated wrapper for a list of projects.` — `ProjectResponse` type-level doc
- `/// A reminder for a task, returned by the Todoist API.` — `Reminder` type-level doc
- `Reminder { ... }` — field docs (id, notify_uid, item_id, service, type, due, minute_offset, name, is_deleted, snoozed)
- `/// Paginated wrapper for a list of reminders.` — `ReminderResponse` type-level doc
- Remove `struct Body { items: Vec<Task> }` at `src/tasks/mod.rs:159-165` (dead code, `#[allow(dead_code)]`)

**Verify**: `cargo doc --no-deps 2>&1 | grep -c warning` returns 0 for just these types (other
items may still warn); `cargo build` passes; `scripts/test.sh` passes; `Body` no longer appears
in any `rg Body` results.

---

## Phase 2: Business Logic Functions

Document public functions in the task/project/list domain. Fix the 3 typos in clap arg docs.
After this phase, `cargo doc` shows documented functions alongside the documented types from
Phase 1.

**Files**: `src/tasks/mod.rs`, `src/projects.rs`, `src/lists.rs`, `src/filters.rs`,
`src/commands/task_commands.rs`, `src/commands/project_commands.rs`

**Key changes**:
- `/// Returns task attributes available when creating a task (all except Content).`
  — `create_task_attributes() -> Vec<TaskAttribute>` (new doc)
- `/// Filters out tasks whose due date is in the future.`
  — `filter_not_in_future(tasks: Vec<Task>, config: &Config) -> Vec<Task>` (new doc)
- `/// Sorts tasks using either the config sort order or custom sort order.`
  — `sort(tasks: Vec<Task>, config: &Config, sort: SortOrder) -> Vec<Task>` (new doc)
- `/// Sorts tasks by configured sort key and direction.`
  — `sort_by_value(tasks: Vec<Task>, config: &Config) -> Vec<Task>` (new doc)
- `/// Sorts tasks by their computed datetime.`
  — `sort_by_datetime(tasks: Vec<Task>, config: &Config) -> Vec<Task>` (new doc)
- `/// Updates a task interactively, returning a join handle for the API call.`
  — `update_task(config: &Config, task: Task, project: Option<String>) -> ...` (new doc)
- `/// Applies labels to a task via interactive menu.`
  — `label_task(config: &Config, task: Task) -> Result<Task, Error>` (new doc)
- `/// Walks through tasks one at a time for completion.`
  — `process_task(config: Config, tasks: Vec<Task>, filter: ...) -> ...` (new doc)
- `/// Assigns a date, time, and duration to a task via interactive prompts.`
  — `timebox_task(config: Config, task: Task) -> Result<Task, Error>` (new doc)
- `/// Sets task priority via interactive menu.`
  — `set_priority(config: &Config, task: Task) -> Result<Task, Error>` (new doc)
- `/// Creates a reminder for a task using natural-language date input.`
  — `create_reminder(config: &Config, task: Task) -> Result<Option<JoinHandle<()>>, Error>` (new doc)
- `/// Schedules a task's due date inside a spawned thread.`
  — `spawn_schedule_task(config: Config, task: Task, due_string: String) -> JoinHandle<()>` (new doc)
- `/// Sets a task's deadline inside a spawned thread.`
  — `spawn_deadline_task(config: Config, task: Task, deadline_string: String) -> JoinHandle<()>` (new doc)
- `/// Specifies how tasks should be sorted.` — `SortOrder` type-level + variant docs
  (new doc, at `src/tasks/mod.rs:193`)
- `/// Creates a project in Todoist and optionally tracks it in the config.`
  — `projects::create(config: &mut Config, name: &str, ...) -> ...` (new doc)
- `/// Edits a task within a project via interactive attribute selection.`
  — `projects::edit_task(config: &Config, project: &Project) -> Result<String, Error>` (new doc)
- `/// Sets a deadline for a task within a project.`
  — `projects::deadline(config: &Config, project: &Project) -> ...` (new doc)
- `/// Moves a task from one project to another.`
  — `projects::move_task_to_project(config: &Config, task: Task) -> ...` (new doc)
- `/// Fetches tasks matching a flag (project, filter, or next-task selector).`
  — `lists::fetch_tasks_by_flag(flag: &Flag, config: &Config, ...) -> ...` (new doc)
- `/// Imports tasks from a file with NL syntax.`
  — `lists::import(config: &Config, file_path: &str, json: bool) -> Result<String, Error>` (new doc)
- `/// Edits tasks matching a Todoist filter.`
  — `filters::edit_task(config: &Config, filter: String) -> Result<String, Error>` (new doc)
- **Typo fix**: `"Date date"` → `"Due date"` in `src/commands/task_commands.rs:58`
- **Misleading text fix**: `"Project to remove"` → `"Project to empty"` in `src/commands/project_commands.rs:121`
- **Typo fix**: `"overriden"` → `"overridden"` in `src/commands/config_commands.rs:45`

**Verify**: `cargo doc --no-deps` — task/project/list functions are documented; `scripts/test.sh`
passes; `cargo clippy` passes. Manual: `cargo doc --no-deps --open` → navigate to `tasks` and
`projects` modules — every public fn has a one-liner.

---

## Phase 3: Config Subsystem

Document all config types, constants, and public functions. Convert `//` comments to `///` where
needed. Fix the `check_config_exists` doc typo and the stale `timeprovider` prose docs.

**Files**: `src/config/mod.rs`, `src/config/file.rs`, `src/config/projects.rs`,
`docs/configuration.md`

**Key changes**:
- `/// Default HTTP timeout for API requests.`
  — `const DEFAULT_TIMEOUT_SECONDS: u64` (new doc)
- `/// Tracks the number of tasks completed today.` — `Completed` type-level doc
- `/// CLI runtime overrides. Fields are #[serde(skip)] — not persisted.`
  — `Args` type-level doc
- `/// Internal async error channel. #[serde(skip)] — runtime only.`
  — `Internal` type-level doc
- `/// Sort dimension for task ordering.` — `SortKey` type-level + variant docs
  (`Priority`, `DueDate`, `Overdue`, `Today`, `Now`, `Deadline`, `Recurring`, `Value`, `Todoist`)
- `/// Sort direction.` — `SortDirection` type-level + variant docs (`Asc`, `Desc`)
- `/// A sort key paired with a direction.` — `SortRule` type-level doc
- `/// Returns the configured max comment length or estimates from terminal width.`
  — `Config::max_comment_length() -> u32` (convert `//` → `///` at ~L322)
- `/// Fully resets the config, accepting inputs for CI testing.`
  — `config_reset_with_prompt()` (convert `//` → `///` at ~L130)
- `/// Checks whether the config file exists at the given path or the default path.`
  — `check_config_exists(config_path: Option<PathBuf>) -> Result<bool, Error>`
  (fix: `"OR  default path if None)"` → `"or the default path"`)
- `/// Loads the config from disk.` — `Config::load()` (new doc, `src/config/file.rs:58`)
- `/// Reloads the config, preserving internal state and time provider.`
  — `Config::reload()` (new doc, `src/config/file.rs:67`)
- `/// Generates the config file path.` — `generate_path() -> Result<PathBuf, Error>` (new doc)
- Docs on `reload_projects`, `add_project`, `remove_project`, `maybe_set_timezone`,
  `check_for_latest_version`, `clear_next_task`, `set_next_task`, `tasks_completed`,
  `next_task`, `set_token`, `set_developer_token`, `edit_interactive` — all new one-liners
  on existing signatures
- **Docs fix**: Remove `### timeprovider` section from `docs/configuration.md` (~L283-289)
  and any references to it as user-configurable. Replace with a note that it is an
  internal/test-only concern.

**Verify**: `cargo doc --no-deps` — config types and functions all documented; `scripts/test.sh`
passes; `cargo clippy` passes. Manual: `docs/configuration.md` no longer implies `timeprovider`
is configurable; `check_config_exists` doc reads correctly.

---

## Phase 4: API Client and Utility Modules

Document the remaining Todoist API client public items, plus `src/format.rs`, `src/input.rs`,
and `src/reminders.rs` (types already done in Phase 1, this phase covers constants + functions).
Add module-level `//!` docs where useful.

**Files**: `src/todoist/mod.rs`, `src/todoist/request.rs`, `src/format.rs`, `src/input.rs`,
`src/regexes.rs`, `src/reminders.rs`

**Key changes**:
- Todoist client: new one-liner docs on `get_task`, `get_access_token`, `all_sections_by_project`,
  `all_projects`, `all_reminders`, `all_labels`, `move_task_to_section`, `delete_task`,
  `delete_project`, `create_project`, `create_section`, `create_comment`, `get_user_data`,
  `filter_tasks_by_title`, and the 4 `pub const` URLs + `QUERY_LIMIT`
- `/// Sends a POST request to Todoist without an auth token (used for OAuth token exchange).`
  — `post_todoist_no_token(...)` (new doc)
- `/// Sends a DELETE request to the Todoist API.`
  — `delete_todoist(...)` (new doc)
- `src/format.rs`: `/// Wraps text in ANSI green.` style one-liners on all 8 color functions +
  `/// Returns true if terminal hyperlinks are disabled.` on `hyperlinks_disabled`
- `src/input.rs`: `/// Prompt label for task content.` style one-liners on all 20+
  `pub const` prompt labels + `/// Natural language date and time input.`
  on `DateTimeInput` enum + `/// Prompts the user for a date.` on `date()` +
  `/// Prompts the user for a boolean value.` on `bool()`
- `//!` module-level doc for `src/todoist/mod.rs` (exists, verify adequate) and
  add short `//!` mod docs for `src/format.rs` and `src/input.rs` describing their role

**Verify**: `cargo doc --no-deps` — todoist, format, input modules fully documented;
`scripts/test.sh` passes; `cargo clippy` passes.

---

## Phase 5: Command Handlers and Dispatch Chain

Document all command handler functions, their clap arg structs, the dispatch chain, and
`fetch_*` helpers. After this phase, every `pub` item in `src/commands/` has a doc.

**Files**: `src/commands/mod.rs`, `src/commands/task_commands.rs`,
`src/commands/list_commands.rs`, `src/commands/project_commands.rs`,
`src/commands/config_commands.rs`, `src/commands/section_commands.rs`,
`src/commands/reminder_commands.rs`, `src/commands/auth_commands.rs`,
`src/commands/shell_commands.rs`

**Key changes**:
- `/// Parsed command-line arguments.` — `Cli` type-level doc
- `/// Top-level command groups.` — `Commands` type-level + variant docs
  (9 variants: `Project`, `Task`, `List`, `Reminder`, `Config`, `Auth`, `Section`, `Shell`, `Test`)
- `/// Routes a parsed command to its handler.`
  — `select_command(cli: Cli, tx: UnboundedSender<Error>) -> Result<CommandResult, Error>` (new doc)
- `/// Resolves task content from an argument or interactive prompt.`
  — `fetch_string(args: &Create, ...) -> Result<String, Error>` (new doc)
- One-liner docs on all 5 remaining `fetch_*` helpers: `fetch_project`, `fetch_filter`,
  `fetch_project_or_filter`, `fetch_priority`, `maybe_fetch_labels`
- `task_commands.rs`: new docs on `TaskCommands` enum + 6 variant structs (`QuickAdd`,
  `Create`, `Edit`, `Next`, `Complete`, `Comment`) + handler fns (`create`, `edit`, `next`,
  `complete`, `comment`). Note the `--json` mode guard behavior where applicable.
- `list_commands.rs`: new docs on `ListCommands` enum + 9 variant structs + handler fns
  (`view`, `process`, `timebox`, `prioritize`, `remind`, `label`, `schedule`, `deadline`,
  `import`)
- `project_commands.rs`: new docs on `ProjectCommands` enum + 7 variant structs + handler fns
  (`create`, `list`, `remove`, `delete`, `rename`, `import`, `empty`)
- `config_commands.rs`: new docs on `ConfigCommands` enum + 7 variant structs + handler fns
  (`check_version`, `check`, `set_timezone`, `edit`, `about`)
- `section_commands.rs`: new docs on `SectionCommands` enum + `Create` struct + handler
- `reminder_commands.rs`: new docs on `ReminderCommands` enum + `List` struct + handler
- `auth_commands.rs`: new docs on `AuthCommands` enum + `Login`/`Token` structs + handler
- `shell_commands.rs`: new docs on `ShellCommands` enum + `Completions` struct + handler

**Verify**: `cargo doc --no-deps` — every `pub` item in `src/commands/` has a `///` one-liner
and no warnings from the commands module; `scripts/test.sh` passes; `cargo clippy` passes.

---

## Phase 6: Non-Code Docs and Lint Enforcement

Fix stale prose docs. Add `#![warn(missing_docs)]` to `src/main.rs` and fix any remaining
undocumented public items flagged by the lint. This is the final gate.

**Files**: `docs/usage.md`, `docs/configuration.md`, `docs/development.md`, `src/main.rs`,
plus any file with newly caught undocumented items

**Key changes**:
- `docs/usage.md`: replace the pasted `tod -h` output (~L20-36) with prose references to
  global flags, explicitly naming `--json`/`-j` and `--timeout`/`-t`
- `docs/configuration.md`: add the 5 missing fields (`task_create_command`,
  `task_comment_command`, `task_complete_command`, `task_exclude_regex`, `comment_exclude_regex`)
  to the example JSON block (~L29-57) with their default values
- `docs/development.md`: remove `"Debug output should always start with DEBUG:"` (~L15) —
  it contradicts `AGENTS.md` and the `scripts/test.sh` grep
- Add `#![warn(missing_docs)]` to `src/main.rs` (before any module declarations)
- Run `cargo doc --no-deps 2>&1 | grep warning` — fix any undocumented public items not caught
  in Phases 1-5 (e.g., inherent impl methods on `Config`, module declarations, any overlooked
  types)
- Add `#[allow(missing_docs)]` on any intentional exceptions with brief inline comments
  explaining why

**Verify**: `cargo doc --no-deps` runs with **zero warnings**; `scripts/test.sh` passes;
`cargo clippy` passes; `cargo build` passes with zero warnings. Manual review: all 5 prose-doc
issues resolved, no `//` comments remain on public items.

---

## Testing Checkpoints

After each phase, a clean-state CI run should pass:
```
cargo doc --no-deps 2>&1    # fewer warnings each phase, zero after Phase 6
scripts/test.sh             # always passes (no grep hits)
cargo clippy -- -D warnings # always passes
```

- **After Phase 1**: All core types (`Task`, `Project`, `DateInfo`, `Deadline`, `Duration`,
  `Unit`, `TaskResponse`, `TaskAttribute`, `Reminder`, `ReminderResponse`, `FormatType`)
  are documented. `Body` is gone.
- **After Phase 2**: All business-logic public fns in `tasks`, `projects`, `lists`, `filters`
  are documented. 3 typos fixed.
- **After Phase 3**: All config types and fns documented. `//`→`///` conversions done.
  `check_config_exists` doc fixed. `timeprovider` removed from prose docs.
- **After Phase 4**: Todoist client, format, input, reminders fully documented with module
  docs.
- **After Phase 5**: Every command handler and the dispatch chain documented.
- **After Phase 6**: `cargo doc --no-deps` has zero warnings; all 4 non-code doc issues fixed.
  `#![warn(missing_docs)]` guards against future regressions.
