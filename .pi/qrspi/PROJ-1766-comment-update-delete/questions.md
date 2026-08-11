# Research Questions

## Context
Focus on the Todoist API client layer (`src/todoist/mod.rs` and `src/todoist/request.rs`), the comment data model (`src/comments.rs`), the interactive task processing workflow (`src/tasks/mod.rs`, `src/lists.rs`), CLI command dispatch (`src/commands/mod.rs`), and the test infrastructure (`src/test/`).

## Questions
1. How do existing task CRUD operations (e.g., `update_task_content`, `update_task_description`, `delete_task`) construct their URLs, build request bodies, and handle API responses in `src/todoist/mod.rs`? What patterns do they follow for status codes that don't return JSON bodies (e.g., 204 No Content)?

2. What does the `Comment` struct in `src/comments.rs` look like, specifically which fields does a freshly-created comment receive from the API? Would updating a comment return the same shape or a subset?

3. How does `handle_response` in `src/todoist/request.rs` handle HTTP status codes — which codes trigger auth errors, which trigger pro-plan messages, and what happens for non-200 success codes like 204?

4. How does the interactive `process_task` function in `src/tasks/mod.rs` present options (Complete, Skip, Comment, etc.) to the user and dispatch to spawned async tasks? What input constants are defined in `src/input.rs`?

5. How are CLI commands for task operations dispatched in `src/commands/mod.rs`, and how does the existing `TaskCommands::Comment` handler work? Is there a pattern for adding new task subcommands?

6. How does the test infrastructure in `src/test/` mock Todoist API endpoints (mockito patterns, `ResponseFromFile` enum), construct test fixtures, and verify API calls? What response fixture files exist for comments?
