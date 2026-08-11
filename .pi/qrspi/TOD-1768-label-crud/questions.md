# Research Questions

## Context

This codebase is a Todoist CLI client written in Rust. Labels have an existing data model (`src/labels.rs`), a GET endpoint (`all_labels` in `src/todoist/mod.rs`), and a JSON response fixture (`tests/responses/Label.json`). The codebase uses clap for CLI parsing, mockito for HTTP mocking in tests, and has established patterns for CRUD operations on other resources like projects, sections, and tasks. Focus on the API layer (`src/todoist/`), the CLI command dispatch (`src/commands/`), the business logic layer (`src/projects.rs` as a model), and the test infrastructure (`src/test/`).

## Questions

1. What is the full set of fields on the `Label` struct in `src/labels.rs`, and how does the Todoist API label response JSON (from `tests/responses/Label.json`) map to those fields? Which fields are optional vs required in the API?

2. Trace the flow of `create_project` from CLI to API: how does `src/commands/project_commands.rs` parse arguments, dispatch to `src/projects.rs`, and call `src/todoist/mod.rs`? What does the `request::post_todoist` function expect for URL construction, body format, and response handling?

3. How does `src/commands/mod.rs` register and dispatch command groups (like `ProjectCommands`, `SectionCommands`)? What would need to happen in the `Commands` enum, the `select_command` match block, and the `Cli` struct to add a new top-level command group?

4. How does the `delete_project` flow work end-to-end — specifically, how does `delete_todoist` in `src/todoist/request.rs` handle the DELETE HTTP method, what status codes does it treat as success, and how does the caller (`projects::delete` → `project_commands::delete`) handle the response?

5. What is the existing `all_labels` function's pagination pattern in `src/todoist/mod.rs`, and how does the `LabelResponse::from_json` method deserialize the paginated API response? Are label create/update/delete responses paginated or do they return a single object?

6. How are HTTP 204 No Content responses handled in `src/todoist/request.rs`'s `handle_response` function? Look at how `archive_project` and `unarchive_project` deal with 204 responses, since `DELETE /labels/{id}` also returns 204.

7. What test patterns exist for resource creation and deletion — specifically, how do `test_create_section` and `test_delete_section` in `src/todoist/mod.rs` set up mockito mocks, construct test configs, and assert results? What fixtures and response files would a label create/delete test need?
