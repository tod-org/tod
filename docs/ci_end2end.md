# CI: E2E Todoist Integration Tests

Workflow file: `.github/workflows/e2e_todoist.yml`

This workflow runs `tod` against the **live Todoist API** using a dedicated
development account, rather than mocking the API. It exists to catch
regressions that unit tests can't: CLI argument wiring, real API response
shapes, and actual sort/filter behavior against Todoist's own filter query
language.

## Prerequisites

### Secret

`TODOIST_DEV_API_TEST` — a Todoist API token for a **dedicated dev/test
account**, not a personal account. Set as a repository (or org) secret.
Never use a token for an account containing real data; several jobs create,
complete, and delete tasks and projects.

### Todoist account setup

The dev account needs two projects created **once, manually**, before this
workflow can pass:

1. **`Tod_DEV_CI`** — an empty project. Used by the dynamic-task tests as
   scratch space; the workflow creates and cleans up all tasks in it on
   every run. Must be empty when a run starts (a pre-test cleanup step
   handles leftovers from a crashed previous run automatically, but the
   project should not contain any tasks that aren't prefixed `[E2E]`).

2. **`TOD_DEV_CI_STATIC`** — a read-only fixture project. See
   [Static fixture project](#static-fixture-project-tod_dev_ci_static) below
   for exact contents. **Never modify this project** — the workflow only
   reads from it, and several assertions depend on its exact contents.

No other setup is required; `tod project import --auto` pulls both projects
into each job's local config at the start of the relevant jobs.

## Job structure

The workflow is split into five jobs. All four test jobs depend only on
`build`, not on each other, so they run in parallel once the binary exists.
A workflow-level `concurrency` group (`e2e-todoist`) prevents two separate
*workflow runs* from overlapping and corrupting shared Todoist state; running
the jobs in parallel within a single run is safe because they touch
different projects (or no project at all).

```text
build
  |-- cli_only
  |-- static_queries
  |-- dynamic_tasks
  \-- temp_project_lifecycle
```

### `build`

Checks out the repo, installs the Rust toolchain, builds `cargo build
--release`, and uploads the resulting binary as a workflow artifact
(`tod-binary`, 1-day retention). Every other job downloads this artifact
instead of rebuilding, so the Rust build only happens once per run.

### `cli_only` — no config, no live API calls

Pure CLI-wiring checks. Nothing here talks to Todoist, so it has no
dependency on account state and no secret is used.

| Test | What it verifies |
| ---- | ---- |
| `--version` | Output matches `tod X.Y.Z` format |
| `--help` | Lists all top-level subcommands (`project`, `section`, `task`, `list`, `config`, `auth`, `shell`) |
| `config reset` (happy path) | Given an existing config file, `config reset --force` deletes it |
| `config reset` (error path) | Run again with no file present; asserts it fails (nonzero exit), proving the guard against double-reset works |
| `config check-version` | Runs against a config path that doesn't exist, since the command is documented to not require one; asserts exit code is `0` or `1` (both valid outcomes), not a crash |

Uses its own scratch config path (`TOD_SCRATCH_CONFIG`), isolated from every
other job.

### `static_queries` — read-only, against `TOD_DEV_CI_STATIC`

Authenticates, imports projects, then only ever *reads*. Never creates,
completes, or deletes anything. See [Static fixture
project](#static-fixture-project-tod_dev_ci_static) for the exact task data
these assertions are keyed on.

| Test | What it verifies |
| --- | --- |
| Fixture project present | `TOD_DEV_CI_STATIC` shows up in `project list` after import |
| `list view --sort value` | Priority-descending order (ties on priority checked by membership, not position, since tie-break order isn't guaranteed) |
| `list view --sort datetime` | No-date tasks first, then ascending by due date |
| `list view --filter "p1"` | Returns exactly the two raw-priority-4 tasks (Todoist filter syntax: `p1` = urgent/raw priority 4 — inverted from the `--priority` flag on `task create`, where `4` is highest) |
| `list view --filter "@e2estatic"` | Returns exactly the two labeled tasks |
| `list view --filter ".../Static Section"` | Returns exactly the two tasks inside the section |

### `dynamic_tasks` — creates, completes, and deletes in `Tod_DEV_CI`

The original priority-sort test plus new coverage for quick-add and labels.
Every task this job creates is prefixed `[E2E]` so cleanup logic can safely
distinguish "ours" from anything unexpected.

| Test | What it verifies |
| --- | --- |
| Pre-test cleanup | Completes any leftover `[E2E]`-prefixed tasks from a previous failed run; aborts if a non-`[E2E]` task is found (refuses to touch unknown data) |
| Create 4 priority tasks | One task at each priority level (`--priority 1` through `4`), created via `task create` |
| Verify tasks exist | `list view` greps for all four task names |
| Sort checks 1-4 | Each calls `task next` and asserts the returned task matches the expected priority order (High -> Medium -> Low -> None), completing each task between checks. Default `sort_order` is priority descending by raw API value (raw priority `1` = no priority, sorts last) |
| Verify project empty | Confirms all four were completed |
| Quick-add | `task quick-add` with natural-language content (`p1 tomorrow`); verifies the task exists and that a due date was actually parsed (checks for a due-date marker in the output), not just that the task was created |
| Label on create | `task create --label`; verifies via `list view --filter "@e2elabel"` that the label was actually applied, not just that the task exists |
| Verify project empty (again) | Confirms quick-add and labeled tasks were completed |
| Cleanup (`if: always()`) | Runs even on failure; completes any remaining `[E2E]` tasks, stops and warns rather than deleting if it finds anything unexpected |

### `temp_project_lifecycle` — create, section, delete

Runs against a brand-new project named uniquely per run
(`Tod_E2E_Temp_<run_id>_<run_attempt>`), so it can never collide with a
concurrent or leftover run and never touches `Tod_DEV_CI` or
`TOD_DEV_CI_STATIC`.

| Test | What it verifies |
| --- | --- |
| Create temp project | `project create`; verifies it appears in `project list` |
| Create section | `section create` in the temp project; asserts on tod's own success message. This is a smoke test only — `task create` has no `--section` flag, so a task can't be scripted into a section this way. Section-with-tasks behavior (filtering, listing) is covered read-only against the static fixture instead |
| Delete temp project | `project delete`; confirmed to run non-interactively with no confirmation prompt |
| Verify deletion is real | Deletes again and asserts it *fails* — proves the project is actually gone, not just that the first call returned success |
| Cleanup (`if: always()`) | Safety net: attempts delete once more (errors ignored) in case an earlier step failed before the real delete ran, so a crashed job can't leave a stray project in the account |

## Static fixture project: `TOD_DEV_CI_STATIC`

Provisioned manually, once. Contains exactly 6 tasks. Dates are chosen far
in the past or future so "overdue" and "upcoming" stay true indefinitely,
regardless of when CI happens to run — nothing in this project should ever
need to change.

| Task content | Due date | Priority (flag) | Label | Section |
| --- | --- | --- | --- | --- |
| `[E2E-STATIC] Overdue High Priority` | 2020-01-01 | 4 | - | - |
| `[E2E-STATIC] Overdue Medium Priority` | 2020-06-15 | 3 | - | - |
| `[E2E-STATIC] Future Low Priority Labeled` | 2099-12-31 | 2 | `e2estatic` | - |
| `[E2E-STATIC] No Date No Label` | none | 1 | - | - |
| `[E2E-STATIC] Section Task No Date` | none | 1 | - | Static Section |
| `[E2E-STATIC] Section Task Future High Priority Labeled` | 2099-06-15 | 4 | `e2estatic` | Static Section |

**Do not edit, complete, or delete any of these tasks.** The
`static_queries` job's assertions are hardcoded against this exact set;
changing a due date, priority, or label will break those tests without any
corresponding code change to explain why.

## Known gaps / commands not covered

A few `tod` subcommands don't have a scriptable non-interactive path and are
intentionally left out of this workflow:

- **`project rename`** — `--help` shows no flag for the new name, implying
  an interactive prompt. Untested; worth revisiting if a way to pipe the
  answer via stdin is confirmed to work.
- **`list label`, `list process`, `list prioritize`, `list schedule`,
  `list deadline`, `list timebox`** — all iterate through tasks prompting
  per-task; no flag found to supply answers non-interactively.
- **`list remind` / `reminder` commands** — Pro-only Todoist feature; the
  dev account used for this workflow is not on a paid plan.
- **Task creation directly into a section** — `task create` has no
  `--section` flag (only `--no-section`, which skips the prompt entirely).
  Section-scoped behavior is only tested read-only, against tasks already
  placed in a section in the static fixture.

## Filter syntax notes

Todoist's filter query language and `tod`'s own `--priority` flag do not
agree on numbering:

- `tod task create --priority 4` = highest priority (matches the CLI's own
  documented "4 = highest" convention).
- In a `--filter` string, `p1` means *urgent* (raw priority 4), and `p4`
  means *no priority* (raw priority 1) — this is Todoist's own filter
  syntax, inverted from the CLI flag. Confirmed empirically: `--filter "p1"`
  against the static fixture returns the two raw-priority-4 tasks.

Also: `list view` does not accept `--project` and `--filter` together —
project scoping inside a filter query uses `#ProjectName` syntax instead
(e.g. `--filter "#Tod_DEV_CI_STATIC & p1"`).
