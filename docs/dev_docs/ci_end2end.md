# E2E Todoist CI Overview

Workflow file: `.github/workflows/e2e_todoist.yml`

This workflow runs Tod end-to-end tests against the live Todoist API using the
Rust `crates/tod-e2e/tests/e2e_todoist.rs` suite. It is intentionally serial so
the approved Todoist E2E scenarios execute in a stable order.

## Purpose

- Validate real CLI behavior against Todoist (not mocks)
- Protect config/auth/list/task/project flows end-to-end
- Keep dynamic test state isolated from static fixtures

## Required secret

- `TODOIST_DEV_API_TEST`: API token for the dedicated Todoist test account

The workflow maps this secret to `TOD_E2E_TOKEN` before running tests.

## Test model

### Static fixture project

- `TOD_DEV_CI_STATIC_READ`
- Read-only fixture project
- Never mutated by tests
- Contains persistent fixture tasks (including recurring/non-recurring examples)

### Dynamic mutable project

- `TOD_DEV_CI_DYNAMIC`
- Used for create/complete/comment/empty-state flows
- Must return to empty state after dynamic tests

### Disposable lifecycle project

- Randomly generated per test run
- Used for create/import/rename/delete lifecycle assertions
- Deleted at test end

## Test inventory (current suite)

### CLI-only sanity

- `version_prints_semver`: checks `tod --version` output format.
- `help_includes_expected_commands`: confirms key top-level commands appear in help.
- `check_version_runs_without_config`: verifies version check works without config.
- `config_reset_force_fails_when_file_absent`: verifies reset fails when config file is missing.
- `config_reset_force_reports_deletion`: verifies reset deletes an existing config file.

### Auth/config persistence

- `auth_token_setup_saves_token_and_timezone`: saves token and timezone from API.
- `auth_token_updates_existing_config_without_overwrite`: updates token while preserving other config.
- `set_timezone_updates_existing_config_without_overwrite`: updates timezone while preserving other config.
- `project_import_auto_includes_static_project`: imports projects and confirms static fixture is present.

### Static fixture read/query (`TOD_DEV_CI_STATIC_READ`)

- `list_view_sort_value_orders_by_priority`: validates priority sorting.
- `list_view_sort_datetime_orders_by_date`: validates datetime sorting.
- `filter_by_priority_returns_expected_tasks`: validates priority filter execution.
- `filter_by_label_returns_expected_tasks`: validates label filter execution.
- `filter_by_section_returns_expected_tasks`: validates section filter execution.
- `recurring_filter_returns_recurring_task`: verifies recurring filter shows `[E2E-STATIC] Recurring Task`.

### Dynamic project flow (`TOD_DEV_CI_DYNAMIC`, file-serialized)

- `dynamic_task_lifecycle`: creates priority tasks, iterates `next`, and completes lifecycle.
- `task_comment_create_is_visible_on_next`: adds comment to current task and verifies visibility in `task next`.
- `empty_project_list_and_next_show_nothing_present`: verifies empty-state output for list and next.

### Random project lifecycle (new project per run, file-serialized)

- `dynamic_empty_project_create_query_delete`: creates random project, renames it, creates/checks task in renamed project, empties it, and deletes it.

## Execution details

- Command shape:
  - `cargo build --manifest-path Cargo.toml --bin tod`
  - `TOD_E2E_TOD_BIN=target/debug/tod cargo nextest run --manifest-path crates/tod-e2e/Cargo.toml --no-fail-fast`
- Serial behavior is enforced in code with `serial_test` for the dynamic project group only.

## Notes

- `tod-e2e` is an external sub-crate and is only run via its explicit manifest-path command.
- If fixture data changes in Todoist, static assertion tests must be updated in
  `crates/tod-e2e/tests/e2e_todoist.rs`.
