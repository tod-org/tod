# Research Questions

## Context
Focus on the Todoist API client layer (`src/todoist/mod.rs`, `src/todoist/request.rs`), the Project struct and business logic (`src/projects.rs`), the CLI command dispatch (`src/commands/project_commands.rs`, `src/commands/mod.rs`), and test infrastructure (`src/test/`, `tests/responses/`). The codebase has an existing pattern for CRUD operations on tasks and projects — trace how create, update, and delete work to understand the conventions.

## Questions
1. How do existing task update functions (`update_task_priority`, `update_task_content`, `update_task_deadline`, `update_task_labels`) structure their POST requests and handle responses, and how does `create_project` differ by deserializing a full `Project` response instead of returning `"✓"`?

2. What writable fields does the Todoist REST API accept for `POST /projects/{id}`, how do those map to the existing `Project` struct fields in `src/projects.rs`, and does the `Project` struct currently derive both `Serialize` and `Deserialize`?

3. How does `handle_response` in `src/todoist/request.rs` handle 204 No Content or empty-body responses, and how do callers like `delete_project`, `update_task_priority`, and `complete_task` handle responses that carry no meaningful JSON body?

4. What is the complete pattern for adding a new subcommand to `ProjectCommands` in `src/commands/project_commands.rs` — including the clap derive struct, the match arm in `project_command()` in `src/commands/mod.rs`, and the handler function signature?

5. How are project API operations tested — what mockito patterns are used for mocking HTTP calls, what project-related JSON fixtures exist in `tests/responses/`, and how do existing tests assert project deserialization and API behavior?

6. How does `projects::create` in `src/projects.rs` bridge between calling the API (`todoist::create_project`) and updating the local config (`config.add_project`), and what does the current `projects::rename` function reveal about the distinction between local-only and API-backed project operations?
