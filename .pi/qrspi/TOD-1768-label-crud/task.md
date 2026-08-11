# Task: Label CRUD (create, update, delete labels)

Add API functions and CLI commands for creating, updating, and deleting personal labels in Todoist. Currently only label listing (`all_labels` / `GET /labels`) is supported. The Todoist REST API also supports `POST /labels` (create), `POST /labels/{id}` (update name, color, order, is_favorite), and `DELETE /labels/{id}`. The goal is to expose these operations both as internal API functions in `src/todoist/mod.rs` and as CLI subcommands (e.g. `tod label create`, `tod label update`, `tod label delete`).

Ref: https://github.com/tod-org/tod/issues/1768
