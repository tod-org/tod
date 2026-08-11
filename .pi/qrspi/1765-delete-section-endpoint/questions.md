# Research Questions

## Context
The codebase is a Rust CLI (`tod`) that wraps the Todoist REST API. Key areas: `src/todoist/request.rs` (HTTP transport — GET, POST, DELETE), `src/todoist/mod.rs` (API client functions), `src/sections.rs` (Section model), `src/commands/section_commands.rs` (CLI subcommands for sections), `src/commands/mod.rs` (command dispatch), and `src/commands/project_commands.rs` (a parallel command group with create/list/delete patterns).

## Questions
1. How does `request::delete_todoist` construct and send DELETE requests, what headers and body does it include, and how does `handle_response` process success vs. error status codes?
2. What is the full pattern of existing DELETE operations in `src/todoist/mod.rs` (`delete_task`, `delete_project`), including URL construction, body content, the `spinner` parameter, error propagation, and return type conventions?
3. How are CLI subcommands structured in `SectionCommands` (`src/commands/section_commands.rs`), and how does the `section_command` dispatch function in `src/commands/mod.rs` route each variant to its handler — including how JSON mode and config loading are threaded through?
4. What test patterns are used for DELETE API functions in `src/todoist/mod.rs` (mockito server setup, request matching, response fixtures), and what section-related test fixtures exist in `src/test/fixtures.rs` and `src/test/responses.rs`?
5. How does the `project delete` CLI command flow from `ProjectCommands::Delete` through argument parsing, config fetching, user interaction, JSON mode branching, and the API call in `src/commands/project_commands.rs`?
6. What fields does the `Section` struct in `src/sections.rs` contain, how are `Section::from_json` and `SectionResponse::from_json` used, and where are sections fetched, selected, or moved-to across the codebase?
