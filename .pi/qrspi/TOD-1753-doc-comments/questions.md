# Research Questions

## Context
This project is a Rust CLI application for managing Todoist tasks. The codebase spans an API client layer (`src/todoist/`), a task business-logic layer (`src/tasks/`), CLI command dispatch (`src/commands/`), a config subsystem (`src/config/`), and various supporting modules for formatting, auth, time handling, and data types. Focus on understanding the existing documentation patterns, mapping what public API items exist and how they relate, and identifying gaps in both code and prose docs.

## Questions

### 1. Doc comment style conventions with examples
What doc comment style conventions exist in well-documented items? The `Error` struct in `src/errors.rs` uses heading-style sections (`# Source naming convention`) with intra-doc links like `` [`format::red_string`] ``. How prevalent are these patterns across `src/todoist/mod.rs`, `src/tasks/mod.rs`, and `src/projects.rs`? Specifically: when are `//!` module docs used vs `///` item docs, how are struct fields documented (if at all), and is there a convention for documenting async functions vs sync functions?

### 2. Undocumented public items in the core data model
The types `Task`, `TaskResponse`, `TaskAttribute`, `DateInfo`, `Deadline`, `Duration`, `Unit`, and `FormatType` in `src/tasks/mod.rs` are all public with little or no documentation. For each, what does the implementation reveal about its purpose? For example, `DateInfo` fields like `date`, `is_recurring`, `string`, `lang`, and `timezone` have partial field-level docs. What information would a consumer of this type need that isn't currently written down?

### 3. Config subsystem documentation gaps
The types `Completed`, `Args`, `Internal`, `SortKey`, `SortDirection`, and `SortRule` in `src/config/mod.rs` are all public and undocumented. Additionally, some functions use `//` comments instead of `///` (e.g., `max_comment_length()`, `config_reset_with_prompt()`). What does each type and method do? Which config fields carry `#[serde(skip)]` but are still discussed in `docs/configuration.md`?

### 4. Command dispatch chain
Trace how `src/commands/mod.rs` dispatches to handlers in `list_commands.rs`, `task_commands.rs`, `project_commands.rs`, `config_commands.rs`, and `section_commands.rs`. For each public `async fn` handler (e.g., `view`, `label`, `process`, `timebox`, `quick_add`, `create`, `edit`, `next`, `complete`), what does it do and what role does it play in the CLI?

### 5. Non-code docs state
What is in `docs/usage.md` and `docs/configuration.md`? Does `usage.md` document `--json`/`-j` mode? Does `configuration.md`'s example JSON block include all config fields or are some missing? Is `timeprovider` discussed there despite being `#[serde(skip)]`? What is the nature of any `check_config_exists` formatting issue?

### 6. Typo and misleading text locations
What are the exact current texts of: (a) the `SetTimezone` variant doc containing `overriden` in `src/commands/config_commands.rs`, (b) the `Create.due` field doc containing `Date date` in `src/commands/task_commands.rs`, and (c) the `Empty.project` arg doc in `src/commands/project_commands.rs` regarding its description of what the operation does?

### 7. Rustdoc linting tooling
What rustdoc and clippy lints exist for enforcing documentation coverage on public API surfaces? Specifically: what items does `#![deny(missing_docs)]` cover (structs, enums, functions, methods, trait impls?), what does `missing_doc_code_examples` add, and how do Rust projects typically phase these lints into CI without breaking existing code?
