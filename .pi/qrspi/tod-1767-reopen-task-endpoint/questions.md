# Research Questions

## Context
Focus on the Todoist API client (`src/todoist/`), HTTP request layer (`src/todoist/request.rs`),
CLI command dispatch (`src/commands/mod.rs`, `src/commands/task_commands.rs`), and config state
management (`src/config/mod.rs`). Pay particular attention to the task-closing path and other
204 No Content endpoints as reference patterns.

## Questions
1. Trace the flow of `complete_task` from the CLI entry point (`task_command` dispatch in
   `src/commands/mod.rs`) through to the HTTP request — what function calls, structs, and
   return types are involved at each layer?

2. How does `post_todoist` in `src/todoist/request.rs` handle HTTP 204 No Content responses,
   and what does the caller receive when the response body is empty?

3. How do `archive_project` and `unarchive_project` in `src/todoist/mod.rs` handle the
   204 No Content response pattern, and how does their approach differ from `complete_task`
   in terms of side effects?

4. What side effects does `complete_task` have on config state (next-task clearing, shell
   command hooks, config reload/save), and how are those operations structured?

5. How are task subcommands structured in the `TaskCommands` enum and dispatched in
   `task_command` within `src/commands/mod.rs`? What steps are required to add a new variant?

6. How are mockito tests structured in the `src/todoist/mod.rs` test module for endpoints
   that return no body (e.g., `test_complete_task`, `test_delete_task`)? What mock setup
   and assertion patterns do they follow?
