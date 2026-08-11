# Task: Reopen Task Endpoint

Add a `reopen_task` function to the Todoist API client (`src/todoist/mod.rs`) that calls
`POST /tasks/{id}/reopen` (returns 204 No Content), following the same pattern as the
existing `complete_task` function for `POST /tasks/{id}/close`. Expose it via the CLI
as `tod task reopen` so users can uncomplete a previously completed task.

Part of #1760 (API feature parity).
