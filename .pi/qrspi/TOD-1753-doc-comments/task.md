# Task

Add `///` doc comments to 79 public API items across the tod codebase that were identified as undocumented during an audit. The items span five tiers:

1. **Core data model** — 14 items: `Task`, `TaskResponse`, `TaskAttribute`, `DateInfo`, `Deadline`, `Duration`, `Unit`, `FormatType` structs/enums plus public functions in `src/tasks/mod.rs` and `src/tasks/format.rs`
2. **Todoist API client** — 15 items: undocumented `pub async fn` functions in `src/todoist/mod.rs` and `src/todoist/request.rs`
3. **Command handlers** — 20 items: public dispatch functions in `src/commands/list_commands.rs`, `task_commands.rs`, `project_commands.rs`, `config_commands.rs`, and `section_commands.rs`
4. **Config subsystem** — 18 items: undocumented types and methods in `src/config/mod.rs`, `config/file.rs`, `config/projects.rs`, and `config/timezone.rs`
5. **Other modules** — 12+ items: public functions and types in `src/format.rs`, `labels.rs`, `sections.rs`, `comments.rs`, `reminders.rs`, `users.rs`, `input.rs`, `cargo.rs`, `oauth.rs`, `shell.rs`, `update.rs`, `time.rs`, `lists.rs`, `filters.rs`, and `projects.rs`

Also fixes 3 typos/misleading help texts, 5 `//` comments that should be `///`, and 4 non-code doc issues in `docs/usage.md` and `docs/configuration.md`.

The goal is to make the public API surface understandable to contributors and library consumers, following the existing doc style conventions in the codebase.
