# Implementation Plan

## Overview

Document 79+ public API items with `///` one-liners, fix 3 typos and 5 stale prose docs, remove the dead `Body` struct, add `#![warn(missing_docs)]` to prevent regressions.

---

## Phase 1: Core Data Types

### Changes

#### 1. Task struct — type-level doc + non-obvious field docs
**File**: `src/tasks/mod.rs`
**Action**: modify

Add `/// A task returned by the Todoist API.` above the `Task` struct declaration. Add field docs for non-obvious fields only:

```rust
/// A task returned by the Todoist API.
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct Task {
    pub id: String,
    ...
    pub deadline: Option<Deadline>,
    /// Duration for timeboxing (amount + unit).
    pub duration: Option<Duration>,
    /// Due date and time information.
    pub due: Option<DateInfo>,
    /// Whether the task has been completed.
    pub checked: bool,
    /// Whether the task has been soft-deleted.
    pub is_deleted: bool,
    /// Whether subtasks are collapsed in Todoist UI.
    pub is_collapsed: bool,
    ...
    /// This doesn't seem to be updated by the API
    pub note_count: u32,
    pub day_order: i16,
}
```

Also add field docs on `Task` for the derived boolean fields from the `impl Task` block: `is_today`, `is_overdue`, `is_recurring` — these already have docs (check `is_recurring` at line ~389), `is_today`, `is_overdue` already documented, so only new field docs needed are:

- `deadline` → `/// Hard deadline date (YYYY-MM-DD).`
- `duration` → `/// Duration for timeboxing (amount + unit).`
- `due` → `/// Due date and time information.`
- `checked` → `/// Whether the task has been completed.`
- `is_deleted` → `/// Whether the task has been soft-deleted.`
- `is_collapsed` → `/// Whether subtasks are collapsed in Todoist UI.`

Note: `is_today`, `is_overdue`, `is_recurring` are methods on `Task`, not fields. The 7 bool fields from the struct are actually: `checked`, `is_deleted`, `is_collapsed`. Only 3 bool flags on Task struct. The remaining bool flags from the design are on `Project` not `Task`.

#### 2. DateInfo — type-level doc + remaining field doc
**File**: `src/tasks/mod.rs`
**Action**: modify

```rust
/// A date and time representation from the Todoist API.
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct DateInfo {
    /// Date string as "YYYY-MM-DD" (date) or "YYYY-MM-DDTHH:MM:SSZ" (datetime)
    pub date: String,
    pub is_recurring: bool,
    /// "2025-04-26 15:00"
    pub string: String,
    /// i.e. "en"
    pub lang: String,
    /// i.e. "America/Vancouver"
    pub timezone: Option<String>,
}
```

Add `/// The IANA timezone for this date, e.g. "America/Vancouver".` above `timezone`.

#### 3. Duration + Unit — type-level + field docs
**File**: `src/tasks/mod.rs`
**Action**: modify

```rust
/// A duration attached to a task, used for timeboxing.
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct Duration {
    /// Number of units.
    pub amount: u32,
    /// Unit of time.
    pub unit: Unit,
}
```

```rust
/// Unit of time for a task duration.
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub enum Unit {
    /// Duration in minutes.
    #[serde(rename = "minute")]
    Minute,
    /// Duration in days.
    #[serde(rename = "day")]
    Day,
}
```

#### 4. TaskResponse + TaskAttribute + FormatType — type-level docs
**File**: `src/tasks/mod.rs`
**Action**: modify

```rust
/// Paginated wrapper for a list of tasks.
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct TaskResponse { ... }
```

```rust
/// An editable attribute of a task.
#[derive(Eq, PartialEq)]
pub enum TaskAttribute {
    /// Task title text.
    Content,
    /// Task description.
    Description,
    /// Priority level.
    Priority,
    /// Due date.
    Due,
    /// Labels applied to the task.
    Labels,
    /// Hard deadline.
    Deadline,
}
```

```rust
/// Controls prefix display in terminal output.
pub enum FormatType {
    /// Indented list format (prefix = "- ").
    List,
    /// No prefix, used for a single task.
    Single,
}
```

#### 5. Remove Body struct
**File**: `src/tasks/mod.rs`
**Action**: delete

Remove lines ~159-165:
```rust
#[allow(dead_code)] // Body Struct is not currently used/constructed
#[derive(Serialize, Deserialize, Debug)]
struct Body {
    items: Vec<Task>,
}
```

Verify no references exist: `rg Body src/` — only the definition itself should be found (and removed).

#### 6. Project — type-level doc + non-obvious field docs
**File**: `src/projects.rs`
**Action**: modify

```rust
/// A project from the Todoist API.
#[allow(clippy::struct_excessive_bools)]
#[derive(PartialEq, Eq, Serialize, Deserialize, Clone, Debug)]
pub struct Project {
    pub id: String,
    ...
    /// Whether the project is archived.
    pub is_archived: bool,
    /// Whether the project has been soft-deleted.
    pub is_deleted: bool,
    /// Whether the project is marked as a favorite.
    pub is_favorite: bool,
    pub is_frozen: bool,
    ...
    /// The Todoist view style for this project ("list" or "board").
    pub view_style: String,
    ...
    /// ID of the parent project, if this is a sub-project.
    pub parent_id: Option<String>,
    /// Whether this is the Todoist inbox project.
    #[allow(clippy::struct_field_names)]
    pub inbox_project: Option<bool>,
    /// Whether subtasks are shown collapsed.
    pub is_collapsed: bool,
    /// Whether this project is shared with others.
    pub is_shared: bool,
}
```

Also add: `is_inbox_project` is the `inbox_project` field — document it. Fields skipped (map directly to API): `id`, `can_assign_tasks`, `child_order`, `color`, `created_at`, `name`, `updated_at`, `default_order`, `description`, `is_frozen`.

#### 7. ProjectResponse — type-level doc
**File**: `src/projects.rs`
**Action**: modify

```rust
/// Paginated wrapper for a list of projects.
#[derive(PartialEq, Eq, Serialize, Deserialize, Clone, Debug)]
pub struct ProjectResponse { ... }
```

#### 8. Reminder — type-level doc + field docs
**File**: `src/reminders.rs`
**Action**: modify

```rust
/// A reminder for a task, returned by the Todoist API.
#[allow(clippy::struct_excessive_bools)]
#[derive(PartialEq, Eq, Serialize, Deserialize, Clone, Debug)]
pub struct Reminder {
    /// Unique reminder ID.
    pub id: String,
    /// ID of the task this reminder is attached to.
    pub item_id: String,
    /// User ID that receives the notification.
    pub notify_uid: String,
    /// Reminder type (e.g. "absolute" or "relative").
    pub r#type: String,
    /// Whether the reminder has been soft-deleted.
    pub is_deleted: bool,
    /// Offset in minutes for relative reminders.
    pub minute_offset: Option<u32>,
    /// Whether this reminder is marked as urgent.
    pub is_urgent: bool,
    /// Absolute due date for the reminder.
    pub due: Option<DateInfo>,
}
```

#### 9. ReminderResponse — type-level doc
**File**: `src/reminders.rs`
**Action**: modify

```rust
/// Paginated wrapper for a list of reminders.
#[derive(PartialEq, Eq, Serialize, Deserialize, Clone, Debug)]
pub struct ReminderResponse { ... }
```

### Verification
#### Automated
- [x] `cargo build` passes
- [x] `scripts/test.sh` passes
- [x] `cargo clippy -- -D warnings` passes
- [x] `rg Body src/` returns no results

#### Manual
- [ ] `cargo doc --no-deps --open` → navigate to `tasks` module — `Task`, `DateInfo`, `Duration`, `Unit`, `TaskResponse`, `TaskAttribute`, `FormatType` all have type-level `///` docs
- [ ] `cargo doc --no-deps --open` → navigate to `projects` module — `Project`, `ProjectResponse` have type-level `///` docs
- [ ] `cargo doc --no-deps --open` → navigate to `reminders` module — `Reminder`, `ReminderResponse` have type-level `///` docs; all `Reminder` fields are documented

---

## Phase 2: Business Logic Functions

### Changes

#### 1. New docs on public functions in `src/tasks/mod.rs`
**File**: `src/tasks/mod.rs`
**Action**: modify

Add `///` one-liners above these functions (all currently undocumented):

- `create_task_attributes()` (~L111):
  ```rust
  /// Returns task attributes available when creating a task (all except Content).
  ```
- `filter_not_in_future()` (~L380):
  ```rust
  /// Filters out tasks whose due date is in the future.
  ```
- `sort()` (~L394):
  ```rust
  /// Sorts tasks using either the config sort order or custom sort order.
  ```
- `sort_by_value()` (~L688):
  ```rust
  /// Sorts tasks by configured sort key and direction.
  ```
- `sort_by_datetime()` (~L755):
  ```rust
  /// Sorts tasks by their computed datetime.
  ```
- `update_task()` (~L408):
  ```rust
  /// Updates a task attribute interactively, returning a join handle for the API call.
  ```
- `label_task()` (~L480):
  ```rust
  /// Applies labels to a task via interactive menu.
  ```
- `process_task()` (~L499):
  ```rust
  /// Walks through tasks one at a time for completion.
  ```
- `timebox_task()` (~L546):
  ```rust
  /// Assigns a date, time, and duration to a task via interactive prompts.
  ```
- `set_priority()` (~L821):
  ```rust
  /// Sets task priority via interactive menu.
  ```
- `create_reminder()` (~L838):
  ```rust
  /// Creates a reminder for a task using natural-language date input.
  ```
- `spawn_schedule_task()` (~L619):
  ```rust
  /// Schedules a task's due date inside a spawned thread.
  ```
- `spawn_deadline_task()` (~L640):
  ```rust
  /// Sets a task's deadline inside a spawned thread.
  ```

#### 2. SortOrder — type-level doc
**File**: `src/tasks/mod.rs`
**Action**: modify

Already has variant docs. Just add type-level:

```rust
/// Specifies how tasks should be sorted.
#[derive(clap::ValueEnum, Debug, Copy, Clone)]
pub enum SortOrder { ... }
```

#### 3. New docs on public functions in `src/projects.rs`
**File**: `src/projects.rs`
**Action**: modify

- `projects::create()` (~L70) — currently has no doc:
  ```rust
  /// Creates a project in Todoist and optionally tracks it in the config.
  ```
- `projects::edit_task()` (~L337) — no doc:
  ```rust
  /// Edits a task within a project via interactive attribute selection.
  ```
- `projects::deadline()` (~L411) — no doc:
  ```rust
  /// Sets deadlines for non-recurring tasks in a project.
  ```
- `projects::move_task_to_project()` (~L433) — no doc:
  ```rust
  /// Moves a task from one project to another.
  ```

#### 4. New docs on public functions in `src/lists.rs`
**File**: `src/lists.rs`
**Action**: modify

- `fetch_tasks_by_flag()` (~L57):
  ```rust
  /// Fetches tasks matching a flag (project or filter) with optional filtering.
  ```
- `import()` (~L254):
  ```rust
  /// Imports tasks from a file with natural-language syntax, one per line.
  ```

#### 5. New doc on `filters::edit_task()`
**File**: `src/filters.rs`
**Action**: modify

```rust
/// Edits tasks matching a Todoist filter.
pub async fn edit_task(config: &Config, filter: String) -> Result<String, Error> { ... }
```

#### 6. Typo fixes
**File**: `src/commands/task_commands.rs`
**Action**: modify, line 57-59

Change `/// Date date in format YYYY-MM-DD` to `/// Due date in format YYYY-MM-DD`.

**File**: `src/commands/project_commands.rs`
**Action**: modify, line 119-122

Change `/// Project to remove` on `Empty.project` to `/// Project to empty`.

**File**: `src/commands/config_commands.rs`
**Action**: modify, line 44-46

Change `/// Can be overriden with` to `/// Can be overridden with`.

### Verification
#### Automated
- [ ] `cargo build` passes
- [ ] `scripts/test.sh` passes
- [ ] `cargo clippy -- -D warnings` passes

#### Manual
- [ ] `cargo doc --no-deps --open` → navigate to `tasks` module — all 13 public functions documented with `///` one-liners
- [ ] `cargo doc --no-deps --open` → navigate to `projects` module — `create`, `edit_task`, `deadline`, `move_task_to_project` documented
- [ ] `cargo doc --no-deps --open` → navigate to `lists` module — `fetch_tasks_by_flag`, `import` documented
- [ ] `cargo doc --no-deps --open` → navigate to `filters` module — `edit_task` documented
- [ ] `tod task create -h` shows `Due date` not `Date date`
- [ ] `tod project empty -h` shows `Project to empty` not `Project to remove`
- [ ] `tod config set-timezone -h` shows `overridden` not `overriden`

---

## Phase 3: Config Subsystem

### Changes

#### 1. DEFAULT_TIMEOUT_SECONDS constant
**File**: `src/config/mod.rs`
**Action**: modify

```rust
/// Default HTTP timeout for API requests.
pub const DEFAULT_TIMEOUT_SECONDS: u64 = 30;
```

#### 2. Config struct type — Already has type-level doc (`/// App configuration...`). 
**File**: `src/config/mod.rs`
**Action**: no change needed (already documented)

#### 3. Completed type
**File**: `src/config/mod.rs`
**Action**: modify

```rust
/// Tracks the number of tasks completed today.
#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Completed { ... }
```

#### 4. Args + Internal types
**File**: `src/config/mod.rs`
**Action**: modify

```rust
/// CLI runtime overrides. Fields are `#[serde(skip)]` — not persisted.
#[derive(Default, Clone, Eq, PartialEq, Debug)]
pub struct Args { ... }
```

```rust
/// Internal async error channel. `#[serde(skip)]` — runtime only.
#[derive(Default, Clone, Debug)]
pub struct Internal { ... }
```

#### 5. SortKey, SortDirection, SortRule types
**File**: `src/config/mod.rs`
**Action**: modify

```rust
/// Sort dimension for task ordering.
#[derive(Copy, Clone, Serialize, Deserialize, Eq, PartialEq, Debug)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum SortKey {
    /// Sort by task priority.
    Priority,
    /// Sort by due date.
    DueDate,
    /// Sort overdue tasks first.
    Overdue,
    /// Sort today's tasks first.
    Today,
    /// Sort tasks due within ±15 minutes first.
    Now,
    /// Sort tasks without due dates last.
    NoDueDate,
    /// Sort non-recurring tasks first.
    NotRecurring,
    /// Sort by deadline.
    Deadline,
    /// Sort by Todoist child order.
    Order,
}
```

```rust
/// Sort direction.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum SortDirection {
    /// Ascending order.
    Asc,
    /// Descending order.
    Desc,
}
```

```rust
/// A sort key paired with a direction.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct SortRule { ... }
```

#### 6. Convert `//` to `///` on `max_comment_length()`
**File**: `src/config/mod.rs`, ~L322
**Action**: modify

Replace:
```rust
    // Returns the maximum comment length if configured, otherwise estimates based on terminal window size (if supported)
```
With:
```rust
    /// Returns the configured max comment length or estimates from terminal width.
```

#### 7. Convert `//` to `///` on `config_reset_with_prompt()`
**File**: `src/config/file.rs`, ~L130
**Action**: modify

Replace:
```rust
// Full config reset function, but accepts inputs for CI testing
```
With:
```rust
/// Fully resets the config, accepting inputs for CI testing.
```

#### 8. New one-liner docs on Config methods
**File**: `src/config/file.rs`
**Action**: modify

```rust
    /// Loads the config from disk.
    pub async fn load(path: &Path) -> Result<Config, Error> { ... }
```

```rust
    /// Reloads the config, preserving internal state and time provider.
    pub async fn reload(&self) -> Result<Self, Error> { ... }
```

Add doc on `generate_path()` (~L82):
```rust
/// Generates the config file path (test temp path or `$XDG_CONFIG_HOME/tod.cfg`).
pub async fn generate_path() -> Result<PathBuf, Error> { ... }
```

**File**: `src/config/projects.rs`
**Action**: modify

```rust
    /// Refetches projects from the API and updates the config.
    pub async fn reload_projects(self: &mut Config) -> Result<String, Error> { ... }
```

```rust
    /// Adds a project to the config's project list.
    pub fn add_project(&mut self, project: Project) { ... }
```

```rust
    /// Removes a project from the config by ID.
    pub fn remove_project(&mut self, project: &Project) { ... }
```

**File**: `src/config/mod.rs`
**Action**: modify — add one-liner docs on remaining undocumented Config methods:

```rust
    /// Fetches a sender for the error channel from async processes.
    pub fn tx(self) -> UnboundedSender<Error> { ... }
```

```rust
    /// Checks crates.io for a newer version and saves the check date.
    pub async fn check_for_latest_version(self: Config) -> Result<Config, Error> { ... }
```

```rust
    /// Clears the stored next task.
    pub fn clear_next_task(self) -> Config { ... }
```

```rust
    /// Stores a task as the next task for completion.
    pub fn set_next_task(&self, task: Task) -> Config { ... }
```

```rust
    /// Returns the count of tasks completed today.
    pub fn tasks_completed(&self) -> Result<u32, Error> { ... }
```

```rust
    /// Returns the stored next task, if any.
    pub fn next_task(&self) -> Option<Task> { ... }
```

```rust
    /// Sets the API token and saves the config.
    pub async fn set_token(&mut self, access_token: String) -> Result<String, Error> { ... }
```

```rust
    /// Trims, validates, and saves a developer API token; auto-detects timezone.
    pub async fn set_developer_token(mut self, key: &str) -> Result<Config, Error> { ... }
```

```rust
    /// Interactively edits config fields via prompts.
    #[allow(clippy::too_many_lines)]
    pub async fn edit_interactive(self) -> Result<String, Error> { ... }
```

Also add docs on `maybe_set_timezone` (in `src/config/timezone.rs`):
```rust
    /// Sets the timezone from Todoist user data if not already configured.
    pub async fn maybe_set_timezone(self) -> Result<Config, Error> { ... }
```

#### 9. Fix `check_config_exists` doc typo
**File**: `src/config/file.rs`, ~L239
**Action**: modify

Change:
```rust
/// Checks if the config file exists at the given path OR  default path if None).
```
To:
```rust
/// Checks whether the config file exists at the given path or the default path.
```

#### 10. Docs — remove `timeprovider` section
**File**: `docs/configuration.md`
**Action**: modify

Remove the `### timeprovider` section (~L283-289) and its table of contents entry. Replace with:

```markdown
### timeprovider

*Internal use only. This field is not user-configurable and is ignored when set in the config file.*
```

Also remove the toc entry for `timeprovider` at the top of the file.

### Verification
#### Automated
- [ ] `cargo build` passes
- [ ] `scripts/test.sh` passes
- [ ] `cargo clippy -- -D warnings` passes

#### Manual
- [ ] `cargo doc --no-deps --open` → `config` module — `Completed`, `Args`, `Internal`, `SortKey` (with variants), `SortDirection`, `SortRule`, `DEFAULT_TIMEOUT_SECONDS` all documented
- [ ] `cargo doc --no-deps --open` → `config` module → `Config` — `max_comment_length`, `check_for_latest_version`, `clear_next_task`, `set_next_task`, `tasks_completed`, `next_task`, `set_token`, `set_developer_token`, `edit_interactive`, `tx` documented
- [ ] `cargo doc --no-deps --open` → `config` module → `Config` — `load`, `reload`, `generate_path`, `reload_projects`, `add_project`, `remove_project` documented
- [ ] `check_config_exists` doc reads: "Checks whether the config file exists at the given path or the default path."
- [ ] `docs/configuration.md` no longer implies `timeprovider` is user-configurable

---

## Phase 4: API Client and Utility Modules

### Changes

#### 1. Todoist client — undocumented public functions
**File**: `src/todoist/mod.rs`
**Action**: modify

Add `///` one-liners above these currently-undocumented `pub async fn`s:

```rust
/// Fetches a single task by ID.
pub async fn get_task(config: &Config, id: &str) -> Result<Task, Error> { ... }
```

```rust
/// Exchanges an OAuth code for an access token.
pub async fn get_access_token(config: &Config, code: &str) -> Result<String, Error> { ... }
```

```rust
/// Returns all sections for a project with cursor-based pagination.
pub async fn all_sections_by_project(config: &Config, project: &Project, limit: Option<u8>) -> Result<Vec<Section>, Error> { ... }
```

```rust
/// Returns all projects with cursor-based pagination.
pub async fn all_projects(config: &Config, limit: Option<u8>) -> Result<Vec<Project>, Error> { ... }
```

```rust
/// Returns all reminders with cursor-based pagination.
pub async fn all_reminders(config: &Config, limit: Option<u8>) -> Result<Vec<Reminder>, Error> { ... }
```

```rust
/// Returns all labels with cursor-based pagination.
pub async fn all_labels(config: &Config, spinner: bool, limit: Option<u8>) -> Result<Vec<Label>, Error> { ... }
```

```rust
/// Moves a task to a different section.
pub async fn move_task_to_section(config: &Config, task: &Task, section: &Section, spinner: bool) -> Result<Task, Error> { ... }
```

```rust
/// Deletes a task by ID.
pub async fn delete_task(config: &Config, task_id: &str, spinner: bool) -> Result<String, Error> { ... }
```

```rust
/// Deletes a project by ID.
pub async fn delete_project(config: &Config, project: &Project, spinner: bool) -> Result<String, Error> { ... }
```

```rust
/// Creates a new project in Todoist.
pub async fn create_project(config: &Config, name: &str, description: &str, is_favorite: bool, spinner: bool) -> Result<Project, Error> { ... }
```

```rust
/// Creates a new section in a project.
pub async fn create_section(config: &Config, name: &str, project: &Project, spinner: bool) -> Result<Section, Error> { ... }
```

```rust
/// Creates a comment on a task.
pub async fn create_comment(config: &Config, task_id: &str, content: &str, spinner: bool) -> Result<Comment, Error> { ... }
```

```rust
/// Fetches the authenticated user's data.
pub async fn get_user_data(config: &Config) -> Result<User, Error> { ... }
```

```rust
/// Filters tasks by title regex if configured.
pub fn filter_tasks_by_title(tasks: Vec<Task>, title_regex: Option<&Regex>, config: &Config) -> Vec<Task> { ... }
```

#### 2. Todoist API client — public constants
**File**: `src/todoist/mod.rs`
**Action**: modify

```rust
/// Tasks API base URL.
pub const TASKS_URL: &str = "/api/v1/tasks/";
/// Comments API base URL.
pub const COMMENTS_URL: &str = "/api/v1/comments/";
/// OAuth authorization URL.
pub const OAUTH_URL: &str = "/oauth/authorize";
/// Number of items that can be requested from API at once.
pub const QUERY_LIMIT: u8 = 200;
```

#### 3. Request layer — undocumented public functions
**File**: `src/todoist/request.rs`
**Action**: modify

```rust
/// Sends a POST request to Todoist without an auth token (used for OAuth token exchange).
pub async fn post_todoist_no_token(...) -> Result<String, Error> { ... }
```

```rust
/// Sends a DELETE request to the Todoist API.
pub async fn delete_todoist(...) -> Result<String, Error> { ... }
```

#### 4. Format module — color functions + hyperlinks_disabled
**File**: `src/format.rs`
**Action**: modify

Add `///` docs on all 8 public color functions:

```rust
/// Wraps text in ANSI green.
pub fn green_string(str: &str) -> String { ... }
/// Wraps text in ANSI red.
pub fn red_string(str: &str) -> String { ... }
/// Wraps text in ANSI bright cyan.
pub fn cyan_string(str: &str) -> String { ... }
/// Wraps text in ANSI purple.
pub fn purple_string(str: &str) -> String { ... }
/// Wraps text in ANSI blue.
pub fn blue_string(str: &str) -> String { ... }
/// Wraps text in ANSI yellow.
pub fn yellow_string(str: &str) -> String { ... }
/// Wraps text in ANSI bright blue on yellow (debug style).
pub fn debug_string(str: &str) -> String { ... }
/// Returns text without color (normal style).
pub fn normal_string(str: &str) -> String { ... }
```

```rust
/// Returns true if terminal hyperlinks are disabled.
pub fn hyperlinks_disabled(config: &Config) -> bool { ... }
```

#### 5. Input module — constants + DateTimeInput + functions
**File**: `src/input.rs`
**Action**: modify

Add `///` docs on all 20+ `pub const` prompt labels, `DateTimeInput` enum, `date()`, `bool()`:

```rust
/// Prompt label for task content.
pub const CONTENT: &str = "Set content";
/// Prompt label for task description.
pub const DESCRIPTION: &str = "Set description";
/// Prompt label for project name.
pub const NAME: &str = "Set name";
/// Prompt label for Todoist filter.
pub const FILTER: &str = "Set filter";
/// Prompt label for file path.
pub const PATH: &str = "Set path";
/// Prompt label for due date.
pub const DATE: &str = "Set a due date";
/// Prompt label for time.
pub const TIME: &str = "Set time, i.e. 3pm or 1500";
/// Prompt label for date and time in natural language.
pub const DATE_AND_TIME: &str = "Set a date and time in natural language";
/// Prompt label for duration in minutes.
pub const DURATION: &str = "Set duration in minutes";
/// Prompt label for attribute selection.
pub const ATTRIBUTES: &str = "Select attributes";
/// Prompt label for project selection.
pub const PROJECT: &str = "Select a project";
/// Prompt label for label selection.
pub const LABELS: &str = "Select labels";
/// Prompt label for section selection.
pub const SECTION: &str = "Select section";
/// Prompt label for priority selection.
pub const PRIORITY: &str = "Select priority";
/// Prompt label for option selection.
pub const OPTION: &str = "Select an option";
/// Prompt label for date selection.
pub const SELECT_DATE: &str = "Select a date";
/// Prompt label for task selection.
pub const TASK: &str = "Select a task";
/// Option: use natural language input.
pub const NAT_LANG: &str = "Natural Language";
/// Option: clear the date.
pub const NO_DATE: &str = "No Date";
/// Option: complete the task.
pub const COMPLETE: &str = "Complete";
/// Option: add a reminder.
pub const REMIND: &str = "Remind";
/// Option: assign a duration.
pub const TIMEBOX: &str = "Timebox";
/// Option: add a comment.
pub const COMMENT: &str = "Comment";
/// Option: skip this task.
pub const SKIP: &str = "Skip";
/// Option: delete the task.
pub const DELETE: &str = "Delete";
/// Option: cancel the operation.
pub const CANCEL: &str = "Cancel";
/// Option: quit processing.
pub const QUIT: &str = "Quit";
/// Option: schedule the task.
pub const SCHEDULE: &str = "Schedule";
```

```rust
/// Natural language date and time input.
#[derive(Debug, PartialEq)]
pub enum DateTimeInput {
    /// Skip this task.
    Skip,
    /// Clear the date.
    None,
    /// Complete the task.
    Complete,
    /// Natural language date string.
    Text(String),
}
```

`date()` already has a doc comment but it's just `/// Get datetime input from user.` on the `datetime()` function at ~L53 — wait, actually `date()` at ~L111 has no doc:

```rust
/// Prompts the user for a date via date picker.
pub fn date() -> Result<String, Error> { ... }
```

`bool()` at ~L175 currently has no doc:
```rust
/// Prompts the user for a boolean value.
pub fn bool(desc: &str, default_value: bool, mock_select: Option<usize>) -> Result<bool, Error> { ... }
```

#### 6. Module-level docs for format.rs and input.rs
**File**: `src/format.rs`
**Action**: modify (add as first line)

```rust
//! Terminal color utilities and hyperlink formatting.
```

**File**: `src/input.rs`
**Action**: modify (add as first line)

```rust
//! Terminal input prompts (text, select, confirm, datetime) with test mock support.
```

### Verification
#### Automated
- [ ] `cargo build` passes
- [ ] `scripts/test.sh` passes
- [ ] `cargo clippy -- -D warnings` passes

#### Manual
- [ ] `cargo doc --no-deps --open` → `todoist` module — `get_task`, `get_access_token`, `all_sections_by_project`, `all_projects`, `all_reminders`, `all_labels`, `move_task_to_section`, `delete_task`, `delete_project`, `create_project`, `create_section`, `create_comment`, `get_user_data`, `filter_tasks_by_title`, `TASKS_URL`, `COMMENTS_URL`, `OAUTH_URL`, `QUERY_LIMIT` documented
- [ ] `cargo doc --no-deps --open` → `todoist::request` — `post_todoist_no_token`, `delete_todoist` documented
- [ ] `cargo doc --no-deps --open` → `format` module — module-level doc present; all 9 public functions documented
- [ ] `cargo doc --no-deps --open` → `input` module — module-level doc present; all `pub const` labels, `DateTimeInput`, `date()`, `bool()` documented

---

## Phase 5: Command Handlers and Dispatch Chain

### Changes

#### 1. Dispatch chain — `src/commands/mod.rs`
**File**: `src/commands/mod.rs`
**Action**: modify

```rust
/// Parsed command-line arguments.
#[derive(Parser, Clone)]
#[command(name = NAME)]
...
pub struct Cli { ... }
```

```rust
/// Top-level command groups.
#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    /// (p) Commands that change projects
    #[command(subcommand)]
    #[clap(alias = "p")]
    Project(ProjectCommands),
    /// (n) Commands that change sections
    #[command(subcommand)]
    #[clap(alias = "n")]
    Section(SectionCommands),
    /// (t) Commands for individual tasks
    #[command(subcommand)]
    #[clap(alias = "t")]
    Task(TaskCommands),
    /// (l) Commands for multiple tasks
    #[command(subcommand)]
    #[clap(alias = "l")]
    List(ListCommands),
    /// (r) Commands for reminders. Only available on Pro Todoist plans
    #[command(subcommand)]
    #[clap(alias = "r")]
    Reminder(ReminderCommands),
    /// (c) Commands around configuration and the app
    #[command(subcommand)]
    #[clap(alias = "c")]
    Config(ConfigCommands),
    /// (a) Commands for logging in with OAuth
    #[command(subcommand)]
    #[clap(alias = "a")]
    Auth(AuthCommands),
    /// (s) Commands for generating shell completions
    #[command(subcommand)]
    #[clap(alias = "s")]
    Shell(ShellCommands),
    /// (e) Commands for manually testing Tod against the API
    #[command(subcommand)]
    #[clap(alias = "e")]
    Test(TestCommands),
}
```

```rust
/// Routes a parsed command to its handler.
pub async fn select_command(cli: Cli, tx: UnboundedSender<Error>) -> Result<CommandResult, Error> { ... }
```

```rust
/// Resolves task content from an argument or interactive prompt.
fn fetch_string(maybe_string: Option<&str>, config: &Config, prompt: &str) -> Result<String, Error> { ... }
```

```rust
/// Resolves a project name from an argument or interactive prompt.
async fn fetch_project(project_name: Option<&str>, config: &Config) -> Result<Flag, Error> { ... }
```

```rust
/// Wraps a filter string in `Flag::Filter`, or prompts for one.
fn fetch_filter(filter: Option<&str>, config: &Config) -> Result<Flag, Error> { ... }
```

```rust
/// Resolves a project or filter from arguments, errors if both are set.
async fn fetch_project_or_filter(project: Option<&str>, filter: Option<&str>, config: &Config) -> Result<Flag, Error> { ... }
```

```rust
/// Converts a u8 to Priority or prompts the user.
fn fetch_priority(priority: Option<u8>, config: &Config) -> Result<Priority, Error> { ... }
```

```rust
/// Returns the provided labels or fetches them from the API.
async fn maybe_fetch_labels(config: &Config, labels: &[String]) -> Result<Vec<String>, Error> { ... }
```

#### 2. Task commands
**File**: `src/commands/task_commands.rs`
**Action**: modify

```rust
/// Task subcommands (create, edit, complete, etc.).
#[derive(Subcommand, Debug, Clone)]
pub enum TaskCommands { ... }
```

Add `///` docs on each variant's struct:

```rust
/// Creates a task using Todoist quick-add NLP.
pub struct QuickAdd { ... }
/// Creates a task with structured fields.
pub struct Create { ... }
/// Edits an existing task's attributes.
pub struct Edit { ... }
/// Fetches the next task by priority.
pub struct Next { ... }
/// Completes the last task fetched with the next command.
pub struct Complete { ... }
/// Adds a comment to the last task fetched with the next command.
pub struct Comment { ... }
```

Add docs on each handler function:

```rust
/// Creates a task using natural language quick-add.
pub async fn quick_add(config: &Config, args: &QuickAdd, json: bool) -> Result<String, Error> { ... }
/// Creates a task with structured fields and optional interactive prompts.
pub async fn create(config: Config, args: &Create, json: bool) -> Result<String, Error> { ... }
/// Edits a task's attributes interactively. Blocks interactive prompts in JSON mode.
pub async fn edit(config: Config, args: &Edit, json: bool) -> Result<String, Error> { ... }
/// Fetches the next task by priority and stores it in config.
pub async fn next(config: Config, args: &Next, json: bool) -> Result<String, Error> { ... }
/// Completes the stored next task.
pub async fn complete(config: Config, args: &Complete, json: bool) -> Result<String, Error> { ... }
/// Adds a comment to the stored next task.
pub async fn comment(config: Config, args: &Comment, json: bool) -> Result<String, Error> { ... }
```

#### 3. List commands
**File**: `src/commands/list_commands.rs`
**Action**: modify

```rust
/// Multi-task subcommands (view, process, schedule, etc.).
#[derive(Subcommand, Debug, Clone)]
pub enum ListCommands { ... }
```

Add `///` docs on each handler:

```rust
/// Views tasks matching a project or filter.
pub async fn view(config: &mut Config, args: &View, json: bool) -> Result<String, Error> { ... }
/// Walks through tasks one at a time for completion.
pub async fn process(config: Config, args: &Process) -> Result<String, Error> { ... }
/// Assigns priorities to unprioritized tasks.
pub async fn prioritize(config: Config, args: &Prioritize) -> Result<String, Error> { ... }
/// Adds reminders to tasks that lack them.
pub async fn remind(config: Config, args: &Remind) -> Result<String, Error> { ... }
/// Applies labels from a predefined list or the API.
pub async fn label(config: Config, args: &Label) -> Result<String, Error> { ... }
/// Schedules dates on tasks individually.
pub async fn schedule(config: Config, args: &Schedule) -> Result<String, Error> { ... }
/// Sets deadlines on non-recurring tasks without deadlines.
pub async fn deadline(config: Config, args: &Deadline) -> Result<String, Error> { ... }
/// Assigns dates, times, and durations to tasks.
pub async fn timebox(config: Config, args: &Timebox) -> Result<String, Error> { ... }
/// Creates tasks from a text file using natural language.
pub async fn import(config: Config, args: &Import, json: bool) -> Result<String, Error> { ... }
```

#### 4. Project commands
**File**: `src/commands/project_commands.rs`
**Action**: modify

```rust
/// Project subcommands (create, delete, import, etc.).
#[derive(Subcommand, Debug, Clone)]
pub enum ProjectCommands { ... }
```

Handler docs:
```rust
/// Creates a project in Todoist and adds it to config.
pub async fn create(config: &mut Config, args: &Create, json: bool) -> Result<String, Error> { ... }
/// Lists configured projects with task counts.
pub async fn list(config: &mut Config, _args: &List, json: bool) -> Result<String, Error> { ... }
/// Removes a project from config (local only).
pub async fn remove(config: &mut Config, args: &Remove) -> Result<String, Error> { ... }
/// Deletes a project from Todoist and removes from config.
pub async fn delete(config: &mut Config, args: &Delete) -> Result<String, Error> { ... }
/// Renames a project in config (local only).
pub async fn rename(config: &mut Config, args: &Rename) -> Result<String, Error> { ... }
/// Imports projects from Todoist into config.
pub async fn import(config: &mut Config, args: &Import, json: bool) -> Result<String, Error> { ... }
/// Empties a project by moving tasks to other projects.
pub async fn empty(config: &mut Config, args: &Empty) -> Result<String, Error> { ... }
```

#### 5. Config commands
**File**: `src/commands/config_commands.rs`
**Action**: modify

```rust
/// Configuration subcommands (check, edit, timezone, etc.).
#[derive(Subcommand, Debug, Clone)]
pub enum ConfigCommands { ... }
```

Handler docs:
```rust
/// Prints build and version information.
pub async fn about(_args: &About, json: bool) -> Result<String, Error> { ... }
/// Checks crates.io for a newer version.
pub async fn check_version(args: &CheckVersion, config: Option<Config>, json: bool) -> Result<String, Error> { ... }
/// Validates the config file and optionally removes invalid values.
pub async fn check(config_path: Option<PathBuf>, json: bool) -> Result<String, Error> { ... }
/// Sets the timezone from Todoist user data or a flag.
pub async fn set_timezone(config: Config, args: &SetTimezone) -> Result<String, Error> { ... }
/// Interactively edits config fields via prompts.
pub async fn edit(config: Config, _args: &Edit) -> Result<String, Error> { ... }
```

Note: `open` and `reset` are handled directly via `crate::config::config_open`/`config_reset` in the dispatch — those functions already have docs from Phase 3.

#### 6. Section commands
**File**: `src/commands/section_commands.rs`
**Action**: modify

```rust
/// Section subcommands.
#[derive(Subcommand, Debug, Clone)]
pub enum SectionCommands { ... }
```

```rust
/// Creates a section in a Todoist project.
pub async fn create(config: &Config, args: &Create, json: bool) -> Result<String, Error> { ... }
```

#### 7. Reminder commands
**File**: `src/commands/reminder_commands.rs`
**Action**: modify

```rust
/// Reminder subcommands.
#[derive(Subcommand, Debug, Clone)]
pub enum ReminderCommands { ... }
```

```rust
/// Lists all reminders with their associated tasks.
pub async fn list(config: &mut Config, _args: &List, json: bool) -> Result<String, Error> { ... }
```

#### 8. Auth commands
**File**: `src/commands/auth_commands.rs`
**Action**: modify

```rust
/// Authentication subcommands (OAuth login, developer token).
#[derive(Subcommand, Debug, Clone)]
pub enum AuthCommands { ... }
```

```rust
/// Logs in via OAuth browser flow.
pub async fn login(config: &mut Config, _args: &Login) -> Result<String, Error> { ... }
```

#### 9. Shell commands
**File**: `src/commands/shell_commands.rs`
**Action**: modify

```rust
/// Shell completion subcommands.
#[derive(Subcommand, Debug, Clone)]
pub enum ShellCommands { ... }
```

```rust
/// Generates shell completions for the specified shell.
pub async fn completions(args: &Completions) -> Result<String, Error> { ... }
```

### Verification
#### Automated
- [ ] `cargo build` passes
- [ ] `scripts/test.sh` passes
- [ ] `cargo clippy -- -D warnings` passes

#### Manual
- [ ] `cargo doc --no-deps --open` → `commands` module — `Cli`, `Commands`, `select_command`, all 6 `fetch_*` helpers documented
- [ ] `cargo doc --no-deps --open` → `commands::task_commands` — `TaskCommands` enum, all 6 variant structs, all 6 handler functions documented
- [ ] `cargo doc --no-deps --open` → `commands::list_commands` — `ListCommands` enum, all 9 handler functions documented
- [ ] `cargo doc --no-deps --open` → `commands::project_commands` — `ProjectCommands` enum, all 7 handler functions documented
- [ ] `cargo doc --no-deps --open` → `commands::config_commands` — `ConfigCommands` enum, handler functions documented
- [ ] `cargo doc --no-deps --open` → `commands::section_commands` — `SectionCommands` enum + `create` documented
- [ ] `cargo doc --no-deps --open` → `commands::reminder_commands` — `ReminderCommands` enum + `list` documented
- [ ] `cargo doc --no-deps --open` → `commands::auth_commands` — `AuthCommands` enum + `login` documented
- [ ] `cargo doc --no-deps --open` → `commands::shell_commands` — `ShellCommands` enum + `completions` documented

---

## Phase 6: Non-Code Docs and Lint Enforcement

### Changes

#### 1. Update `docs/usage.md` — replace stale `-h` output
**File**: `docs/usage.md`
**Action**: modify

Replace the code block at ~L20-36 (from ` ```bash` to the closing ` ``` `) with prose. The replacement text for the code block:

```markdown
Run `tod -h` to see all available commands and global flags. Key global flags:

- `--json` / `-j` — Output results as JSON for machine-readable consumption. Suppresses interactive prompts and spinners.
- `--timeout` / `-t` — Time to wait for API responses in seconds (default: 30).
- `--verbose` / `-v` — Display additional debug info while processing.
- `--config` / `-c` — Absolute path to configuration file (default: `$XDG_CONFIG_HOME/tod.cfg`).

Use `tod <command> -h` to see subcommands and flags for each command group.
```

#### 2. Update `docs/configuration.md` — add missing fields to example JSON
**File**: `docs/configuration.md`
**Action**: modify

In the JSON example block (~L29-57), add these 5 missing fields with their default values:

```json
  "comment_exclude_regex": null,
  "task_comment_command": null,
  "task_complete_command": null,
  "task_create_command": null,
  "task_exclude_regex": null,
```

Insert them alphabetically among the existing fields in the JSON block — `comment_exclude_regex` after `bell_on_success`, `task_comment_command`/`task_complete_command`/`task_create_command` before `timeout`, `task_exclude_regex` before `timeout`.

#### 3. Fix `docs/development.md` — remove `DEBUG:` guidance
**File**: `docs/development.md`
**Action**: modify

Remove the line:
```markdown
- Debug output should always start with `DEBUG:`.
```

(~L15 — line reads: `- Debug output should always start with `DEBUG:`.")

#### 4. Add `#![warn(missing_docs)]` to `src/main.rs`
**File**: `src/main.rs`
**Action**: modify

Add after `#![cfg(test)]` / `#[macro_use]` but before `extern crate clap`:

```rust
#![warn(missing_docs)]
```

The exact location: after the `//!` module doc comment, before `#[cfg(test)]`:

```rust
//! An unofficial Todoist command-line client...
//! Get started with `cargo install tod`
#![warn(missing_docs)]
#[cfg(test)]
```

#### 5. Fix any remaining warnings from `#![warn(missing_docs)]`
**File**: various
**Action**: modify (as discovered)

Run `cargo doc --no-deps 2>&1 | grep warning` and address each warning. Likely candidates based on the audit:

- `src/main.rs`: `CommandResult` struct, `output_result`, `output_json`, `output_text`, `run_command`, `terminal_bell` — these are private (not `pub`), confirm they're omitted. Actually `CommandResult` is not `pub` — it's `struct CommandResult` without `pub`. So it won't be flagged. Same for the functions.

- Verify `VERSION` and `LOWERCASE_NAME` constants are not `pub`.
- Check `src/regexes.rs` for `MARKDOWN_LINK` and other pub statics — already documented.
- Check `src/errors.rs` — already fully documented.

For any truly public items caught by the lint that are trivial getters or intentional omissions, add `#[allow(missing_docs)]` with a brief `//` comment:

```rust
#[allow(missing_docs)] // simple field accessor
pub fn next_task(&self) -> Option<Task> { ... }
```

Only use `#[allow(missing_docs)]` as a last resort for items where a doc would be genuinely useless (e.g., `pub use` re-exports for backward compatibility).

### Verification
#### Automated
- [ ] `cargo doc --no-deps 2>&1 | grep warning` returns **zero** matches
- [ ] `cargo build` passes with **zero warnings**
- [ ] `scripts/test.sh` passes
- [ ] `cargo clippy -- -D warnings` passes

#### Manual
- [ ] `docs/usage.md` — no pasted `-h` output; `--json`/`-j` and `--timeout`/`-t` are documented
- [ ] `docs/configuration.md` — example JSON has all 5 missing fields with correct defaults
- [ ] `docs/configuration.md` — `timeprovider` is marked as internal/non-user-configurable
- [ ] `docs/development.md` — no mention of `DEBUG:` prefix requirement
- [ ] `cargo doc --no-deps --open` → all modules — no undocumented public items visible

---

## Final Checkpoint

After all 6 phases complete:

```bash
cargo doc --no-deps 2>&1     # zero warnings
scripts/test.sh              # passes (no grep hits)
cargo clippy -- -D warnings  # passes
cargo build                  # passes with zero warnings
```
