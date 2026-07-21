# E2E Todoist CI Overview

Workflow file: `.github/workflows/e2e_todoist.yml`

This workflow runs Tod end-to-end tests against the live Todoist API using the
Rust `tests/e2e_todoist.rs` suite. It is intentionally serial and uses an
explicit test list so only the approved Todoist E2E scenarios execute.

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

## Groups covered by the CI E2E workflow

1. CLI-only sanity checks
2. Config + auth update invariants
3. Static read/filter/sort queries
4. Dynamic task lifecycle and empty-state checks
5. Disposable project lifecycle (create → rename → delete)

## Execution details

- Command shape:
  - `cargo nextest run --features e2e --profile e2e --no-fail-fast <explicit-test-list>`
- Serial behavior is controlled by `.config/nextest.toml` (`profile.e2e.test-threads = 1`).

## Notes

- Code coverage workflow (`ci_codecov.yml`) explicitly excludes `tests/e2e_todoist.rs`
  because coverage jobs do not provide `TOD_E2E_TOKEN`.
- If fixture data changes in Todoist, static assertion tests must be updated in
  `tests/e2e_todoist.rs`.
