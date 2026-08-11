# Research Questions

## Context
Focus on the Todoist API client layer (`src/todoist/mod.rs`, `src/todoist/request.rs`), the Project data model (`src/projects.rs`), the CLI command dispatch layer (`src/commands/project_commands.rs`, `src/commands/mod.rs`), and the test infrastructure (`src/test/`, `tests/responses/`). The codebase is a Rust CLI for the Todoist REST API using `reqwest` for HTTP, `clap` for argument parsing, `serde_json` for serialization, and `mockito` for HTTP mocking in tests.

## Questions
1. Trace the full call chain for an existing project mutation like `create_project` or `delete_project`: from the clap subcommand definition in `src/commands/project_commands.rs`, through the command dispatch in `src/commands/mod.rs`, the business logic in `src/projects.rs`, the API client function in `src/todoist/mod.rs`, down to the HTTP request builder in `src/todoist/request.rs`. What are the exact signatures, argument patterns, and return types at each layer?

2. How does the existing task-update pattern work for `update_task_priority`, `update_task_content`, `update_task_description`, `update_task_labels`, and `update_task_deadline` in `src/todoist/mod.rs`? Specifically: what HTTP method and URL pattern do they use, how do they construct the JSON body, what does `request::post_todoist` return, and how do they handle the response (some return a parsed Task, others return `Ok("✓".into())`)?

3. How does `request::handle_response` in `src/todoist/request.rs` handle HTTP 204 (No Content) responses? A 204 is a success status with an empty body — trace what happens when `response.text().await` is called on an empty body. Is there any existing code that handles the 204 case explicitly?

4. How is the `Project` struct defined in `src/projects.rs`, what fields does it have, and how are `from_json` and `ProjectResponse::from_json` implemented? Specifically, how are `color`, `is_favorite`, `view_style`, and `is_archived` already present and deserialized? What does the `create_project` API function in `src/todoist/mod.rs` already do with these fields?

5. What is the exact pattern for adding a new subcommand to `ProjectCommands` in `src/commands/project_commands.rs`? Trace how `create`, `list`, `delete`, `rename`, `import`, `remove`, and `empty` are each defined (their struct, their clap attributes, their dispatch arm in `project_command()` in `src/commands/mod.rs`, and their handler function). How does the `--json` output flag propagate through the system?

6. How are mockito-based tests structured for project API operations? Look at `test_all_endpoints` and the test modules in `src/todoist/mod.rs` and `src/commands/project_commands.rs`. How are mock servers set up, how are test response JSON files in `tests/responses/` loaded via `ResponseFromFile`, and how does `test::fixtures::project()` provide a test Project? What is the pattern for testing a POST that returns 204 (empty body)?

7. How is the `Projects` URL constant (`PROJECTS_URL`) used throughout `src/todoist/mod.rs`? What endpoint paths are currently constructed from it (`all_projects`, `create_project`, `delete_project`), and what is the URL format pattern (base URL + path + optional query params)?
