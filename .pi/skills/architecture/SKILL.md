---
name: architecture
description: Runtime output flow, config plumbing, type serialization status, and key integration points in the tod codebase. Use when planning features that touch output, serialization, or cross-cutting concerns.
---

# Architecture

## Output flow

All commands funnel through a single output gateway:

```
Command handler → Result<String, Error>
  → build_command_result() → CommandResult { result, bell_success, bell_failure, json }
  → output_result() in main.rs:77-91
      json  → output_json(&result) — structured JSON to stdout
      text → Ok  → println!("{text}") to stdout
             Err → eprintln!("{e}") to stderr
```

- `build_command_result()` — wraps `Result<String, Error>` with bell flags and `json` from `Config`
- `build_command_result_without_config()` — variant for commands that don't load config; takes explicit `json: bool`
- `CommandResult` — defined in `src/main.rs:49-55` with `{ result, bell_success, bell_failure, json }`
- `output_json()` / `output_text()` — dispatch branches in `src/main.rs:77-91`

## Config plumbing

```
Cli { verbose, config, timeout } → with_cli_context() → Config.args { verbose, timeout }
```

- `Config.args` is `#[serde(skip)]` — never persisted to disk
- `Config.internal` holds the `mpsc::UnboundedSender<Error>` for async error reporting
- `Config.time_provider` is `#[serde(skip)]` — injected for test determinism

## Type serialization status

### Serialize + Deserialize (both derived)
Task, DateInfo, Deadline, Duration, Unit, TaskResponse, Project, ProjectResponse,
Comment, CommentResponse, Reminder, ReminderResponse, User, AccessToken, Config,
SortKey, SortRule, Priority (serde_repr, serializes as u8)

### Deserialize only (Serialize missing)
Label (`src/labels.rs:8`), Section (`src/sections.rs:10`),
LabelResponse (`src/labels.rs:19`), SectionResponse (`src/sections.rs:30`)

### Neither
Error (`src/errors.rs:23`) — `{ message: String, source: String }` (but has `Serialize` derive — serializes for JSON error output)

## Key integration points

### Spinners
`maybe_start_spinner` in `src/todoist/request.rs:239-249` — suppressed by:
- `cfg!(test)`
- `DISABLE_SPINNER` env var
- `config.spinners = Some(false)`
- `spinner: false` parameter

Most foreground API calls pass `spinner: true`; background spawns pass `false`.

### Colors
`format.rs` `apply_color` uses `cfg!(test)` gate. The `colored` crate (v3.1.1)
respects `NO_COLOR` env var. For runtime disable, use `colored::control::set_override(false)`.

### Hyperlinks
`hyperlinks_disabled` checks `config.disable_links || !supports_hyperlinks::on(Stream::Stdout)`.

### Terminal bell
`terminal_bell()` in `main.rs:105` writes `\x07` to stdout. Controlled by
`bell_success` / `bell_failure` booleans on `CommandResult`.

### println!/eprintln! sites (non-test, production code)

| File | Lines | Context |
|---|---|---|
| `src/main.rs` | 79, 86 | `output_result` — success/error output |
| `src/main.rs` | 70 | Async error channel drain |
| `src/tasks/mod.rs` | 477, 515, 521, 578, 646, 679, 941, 962 | Task formatting/processing loops |
| `src/lists.rs` | 101, 130, 160, 225, 229, 234, 286 | Interactive list commands |
| `src/projects.rs` | 376, 381, 419, 551 | Interactive project commands |
| `src/filters.rs` | 37 | Filter edit task spacing |
| `src/debug.rs` | 32 | Verbose debug output |
| `src/oauth.rs` | 57-58, 72-80 | Auth login prompts |
| `src/update.rs` | 65 | Auto-update command execution |
| `src/commands/config_commands.rs` | 132-133, 142 | Version check output |
| `src/commands/project_commands.rs` | 180 | Project delete spacing |

### Input boundary

`src/input.rs` — inquire-based terminal prompts with mock support
(`config.mock_string`, `config.mock_select`). Guards for JSON/non-interactive
modes belong at the `fetch_*` call sites in `src/commands/mod.rs`, **not** inside
`input.rs`. The fetch helpers are: `fetch_string`, `fetch_project`, `fetch_filter`,
`fetch_priority`, `fetch_project_or_filter`, `maybe_fetch_labels`.

### Mock system for testing

- `config.mock_url` — replaces the Todoist API base URL (used with mockito)
- `config.mock_string` — replaces `inquire::Text` input
- `config.mock_select` — replaces `inquire::Select` index selection
- `config.with_time_provider(TimeProviderEnum::Fixed(...))` — freezes time
- `config.with_mock_url(server.url())` — wires mockito server
