# Research Findings

## Q1: Doc comment style conventions with examples

### Findings

**Module-level `//!` docs** — 6 files use `//!`, but only two use paragraph breaks (blank `//!` lines): `src/todoist/mod.rs:1-5` and `src/main.rs:1-3`. The other four (`src/regexes.rs:1-3`, `src/config/file.rs:1`, `src/test/fixtures.rs:1`, `src/test/responses.rs:1`) are single-line. Only `src/todoist/mod.rs:1-5` uses a summary + coverage list pattern.

**Item-level `///` docs** — 19 of 21 docs in `src/todoist/mod.rs` are one-liners. `src/tasks/mod.rs` has mostly one-liners (12 public fn docs, 10 of which are repetitive `spawn_*` helpers saying `"Updates task inside another thread"`). `src/projects.rs` has 11 consistent one-liners on public fns. **No file uses intra-doc links, code blocks, or markdown headings** outside of `src/errors.rs`.

**Gold standard exception** — `src/errors.rs:13-22`: the `Error` struct doc uses heading-style `#` sections (`# Source naming convention`, `# Display and coloring`) with four intra-doc links: `src/errors.rs:18` ``[`format::red_string`]``, `src/errors.rs:19` ``[`format::yellow_string`]``, `src/errors.rs:20` ``[`format`]``, `src/errors.rs:21` ``[`format::apply_color`]``. **No other file in the codebase uses this syntax.**

**Struct field documentation** — sparse and example-style. `src/tasks/mod.rs:26-50`: `Task` has 1/25 fields documented (`note_count` with an API caveat). `src/tasks/mod.rs:128-139`: `DateInfo` has 4/5 field docs (value examples: `"2025-04-26 15:00"`, `i.e. "en"`). `src/tasks/mod.rs:147-152`: `Deadline` has 2/2 field docs. `src/tasks/mod.rs:154-157`: `Duration` has 0/2. `src/projects.rs:19-39`: `Project` has 0/18 field docs.

**Async vs sync** — no difference in doc style. Both use identical one-liner patterns.

## Q2: Undocumented public items in the core data model

### Findings

**`Task`** (`src/tasks/mod.rs:26-61`) — 24 fields with type `String`, 3 timestamp fields, bool flags, and nested structs (`Deadline`, `Duration`, `DateInfo`, `Priority`). Has a `Display` impl delegating to `content`, `from_json` for deserialization, `fmt` for terminal rendering, datetime extraction methods (`datetimeinfo`, `datetime`, `deadline_datetime`), filtering (`filter`, `has_no_date`, `is_today`, `is_now`, `is_overdue`, `is_recurring`). Primary data carrier: deserialized from API at `src/todoist/mod.rs:130-340`, displayed via `src/tasks/format.rs`, processed in `src/lists.rs`.

**`TaskResponse`** (`src/tasks/mod.rs:63-73`) — paginated wrapper with `results: Vec<Task>` and `next_cursor: Option<String>`. Used only in `src/todoist/mod.rs` for cursor-based pagination loops (`all_tasks_by_project:254`, `all_tasks_by_filter:299`, `all_tasks_by_ids:334`).

**`TaskAttribute`** (`src/tasks/mod.rs:78-86`) — enum of 6 editable task fields. `edit_task_attributes()` (`src/tasks/mod.rs:100-109`) returns all 6; `create_task_attributes()` (`src/tasks/mod.rs:111-119`) returns all except `Content`. Used for interactive editing dispatch in `update_task()` (`src/tasks/mod.rs:408-466`), `projects.rs:442`, `filters.rs:38`, `commands/task_commands.rs:142`.

**`DateInfo`** (`src/tasks/mod.rs:128-142`) — Todoist due-date representation. `date` field holds ISO date or datetime string; `is_recurring` flags repeat; `string` is human-readable; `lang` and `timezone` are metadata. Has partial field docs (4/5 fields). `Display` delegates to `string`. Also used in `src/reminders.rs:23` (`Reminder.due`).

**`Deadline`** (`src/tasks/mod.rs:147-152`) — pure data struct with `date` (YYYY-MM-DD) and `lang`. Has field-level docs (2/2). No impl blocks. Note: a separate `Deadline` clap args struct exists at `src/commands/list_commands.rs:214`.

**`Duration`** (`src/tasks/mod.rs:155-162`) — `amount: u32` + `unit: Unit`. Used in `src/tasks/format.rs:74-85` to format timebox strings (`"for 15 min"`, `"for 2 days"`).

**`Unit`** (`src/tasks/mod.rs:166-170`) — enum with `Minute` and `Day` variants, serde-renamed to `"minute"`/`"day"`.

**`FormatType`** (`src/tasks/mod.rs:173-175`) — `List`/`Single` enum with no derives (not even `Debug`). Controls prefix and indentation in `Task::fmt()` (`src/tasks/mod.rs:221-223`).

**Public functions** — `create_task_attributes()` is undocumented (differs from `edit_task_attributes` by omitting `Content`). `sort()` dispatches 3 ways including a no-op Todoist pass-through. `sort_by_value()` uses multi-key sort with direction from config. `sort_by_datetime()` sorts by `task.datetime()`. `update_task()` returns `Option<JoinHandle>` (None = no change). `process_task()` and `timebox_task()` are interactive loops with mutable counters. `label_task()` shows a label select menu. `set_priority()` shows priority options. `create_reminder()` uses natural-language-only datetime picker.

## Q3: Config subsystem documentation gaps

### Findings

**Undocumented public types** — 6 types lack type-level `///` but have field-level comments:
- `Completed` (`src/config/mod.rs:32-36`): daily task completion counter with custom `deserialize_nonnegative_u32`
- `Args` (`src/config/mod.rs:129-134`): CLI runtime overrides (`verbose`, `timeout`, `json`), `#[serde(skip)]`
- `Internal` (`src/config/mod.rs:136-139`): async error channel, `#[serde(skip)]`
- `SortKey` (`src/config/mod.rs:142-153`): 9 variants covering sortable task dimensions
- `SortDirection` (`src/config/mod.rs:212-215`): `Asc`/`Desc`
- `SortRule` (`src/config/mod.rs:227-230`): key+direction pair, custom string serialization

**`//` comments that should be `///`:**
- `max_comment_length()` at `src/config/mod.rs:322`: `// Returns the maximum comment length if configured, otherwise estimates based on terminal window size`
- `config_reset_with_prompt()` at `src/config/file.rs:130`: `// Full config reset function, but accepts inputs for CI testing`

**Undocumented public functions (no comment at all):**
- `Config::load()` — `src/config/file.rs:58` — reads JSON from disk
- `Config::reload()` — `src/config/file.rs:67` — re-reads, preserves `internal` and `time_provider`
- `generate_path()` — `src/config/file.rs:82` — test temp path or `$XDG_CONFIG_HOME/tod.cfg`
- `Config::reload_projects()` — `src/config/projects.rs:11` — fetches API projects, filters to tracked
- `Config::add_project()` — `src/config/projects.rs:29` — pushes project into vec
- `Config::remove_project()` — `src/config/projects.rs:39` — filters by ID
- `Config::maybe_set_timezone()` — `src/config/timezone.rs:19` — calls `set_timezone()` only if unset
- `Config::check_for_latest_version()` — `src/config/mod.rs:345` — version check with save
- `Config::clear_next_task()` — `src/config/mod.rs:383` — sets `next_task: None`
- `Config::set_next_task()` — `src/config/mod.rs:437` — wraps Task in Some
- `Config::tasks_completed()` — `src/config/mod.rs:445` — returns today's count
- `Config::next_task()` — `src/config/mod.rs:458` — getter for `next_task`
- `Config::set_token()` — `src/config/mod.rs:462` — sets token + saves
- `Config::set_developer_token()` — `src/config/mod.rs:467` — trims, validates, auto-detects timezone
- `Config::edit_interactive()` — `src/config/mod.rs:483` — interactive config editor for 12 fields

**`#[serde(skip)]` fields:**
- `args: Args` (`src/config/mod.rs:113-114`) — not in docs (correct)
- `internal: Internal` (`src/config/mod.rs:117-118`) — not in docs (correct)
- `time_provider: TimeProviderEnum` (`src/config/mod.rs:120-121`) — documented in `docs/configuration.md:283-289` as if user-configurable **(incorrect — cannot be set via config file)**

## Q4: Command dispatch chain

### Findings

**Entry point** — `select_command()` at `src/commands/mod.rs:117` matches `cli.command` (a `Commands` enum variant) and calls the per-group dispatch function.

**`Commands` enum** (`src/commands/mod.rs:71-115`) — 9 variants, each wrapping a sub-command enum: `Project(ProjectCommands)`, `Section(SectionCommands)`, `Task(TaskCommands)`, `List(ListCommands)`, `Reminder(ReminderCommands)`, `Config(ConfigCommands)`, `Auth(AuthCommands)`, `Shell(ShellCommands)`, `Test(TestCommands)`.

**Dispatch functions** — each is a private `async fn` that pattern-matches and calls public handlers:
- `list_command()` at `src/commands/mod.rs:184` — dispatches 9 sub-variants to handlers in `src/commands/list_commands.rs`
- `task_command()` at `src/commands/mod.rs:294` — dispatches 6 sub-variants to handlers in `src/commands/task_commands.rs`
- `project_command()` at `src/commands/mod.rs:227` — dispatches 7 sub-variants to handlers in `src/commands/project_commands.rs`
- `config_command()` at `src/commands/mod.rs:156` — dispatches to handlers in `src/commands/config_commands.rs`
- `section_command()` at `src/commands/mod.rs:277` — dispatches to `src/commands/section_commands.rs`

**`fetch_*` helpers** (all in `src/commands/mod.rs`):
- `fetch_string()` at `src/commands/mod.rs:392` — resolves content from arg or prompts
- `fetch_project()` at `src/commands/mod.rs:407` — resolves project name from arg or prompts
- `fetch_filter()` at `src/commands/mod.rs:428` — wraps filter string in `Flag::Filter`
- `fetch_project_or_filter()` at `src/commands/mod.rs:439` — resolves project or filter, errors if both set
- `fetch_priority()` at `src/commands/mod.rs:463` — converts u8 to Priority or prompts
- `maybe_fetch_labels()` at `src/commands/mod.rs:478` — returns provided labels or fetches from API

**`list_commands.rs` handlers** — 9 `pub async fn`s:
- `view()` (`src/commands/list_commands.rs:153`) — JSON or interactive task list
- `label()` (`src/commands/list_commands.rs:174`) — apply labels to tasks
- `process()` (`src/commands/list_commands.rs:186`) — walk tasks one-by-one for completion
- `timebox()` (`src/commands/list_commands.rs:195`) — assign dates/times/durations
- `prioritize()` (`src/commands/list_commands.rs:204`) — assign priorities
- `remind()` (`src/commands/list_commands.rs:213`) — add reminders to tasks
- `schedule()` (`src/commands/list_commands.rs:222`) — assign dates, routes to `filters::schedule()` or `projects::schedule()`
- `deadline()` (`src/commands/list_commands.rs:237`) — assign deadlines
- `import()` (`src/commands/list_commands.rs:248`) — create tasks from file

**`task_commands.rs` handlers** — 6 `pub async fn`s:
- `quick_add()` (`src/commands/task_commands.rs:103`) — natural language inbox add
- `create()` (`src/commands/task_commands.rs:125`) — structured task creation with flags or interactive mode
- `edit()` (`src/commands/task_commands.rs:203`) — edit tasks in project/filter
- `next()` (`src/commands/task_commands.rs:213`) — select and store next task
- `complete()` (`src/commands/task_commands.rs:221`) — complete the stored next task
- `comment()` (`src/commands/task_commands.rs:237`) — comment on the stored next task

**`project_commands.rs` handlers** — 7 `pub async fn`s: `create`, `list`, `remove` (3 modes: all/auto/repeat), `delete` (with force), `rename`, `import`, `empty`.

**`config_commands.rs` handlers** — `check_version()` (`src/commands/config_commands.rs:100`), `check()` (`src/commands/config_commands.rs:160`), `set_timezone()` (`src/commands/config_commands.rs:360`), `edit()` (`src/commands/config_commands.rs:377`), `about()` (`src/commands/config_commands.rs:385`).

**`section_commands.rs` handler** — single handler: `create()` (`src/commands/section_commands.rs:22`).

## Q5: Non-code docs state

### Findings

**`docs/usage.md`** — 174 lines. The "Discovering the commands" section (`docs/usage.md:20-36`) shows `tod -h` output listing only `-v, --verbose`, `-c, --config`, `-h, --help`, `-V, --version`. The global `--json`/`-j` flag (`src/commands/mod.rs:64`) and `--timeout`/`-t` flag (`src/commands/mod.rs:58-60`) are **not documented**. JSON mode is a major feature wired through the entire dispatch chain (`src/commands/mod.rs:440-444` blocks spinners and interactive prompts in JSON mode; `src/main.rs:79-84` routes to `output_json` vs `output_text`).

**`docs/configuration.md`** — 361 lines. The example JSON block (`docs/configuration.md:29-57`) lists 22 fields but misses 5: `task_create_command`, `task_comment_command`, `task_complete_command`, `task_exclude_regex`, `comment_exclude_regex` — even though each has a documented `###` subsection in the Values section below.

**`timeprovider` in docs** — `docs/configuration.md:283-289` has a dedicated section describing it as a configurable field with `type: string`. But `src/config/mod.rs:120-121` marks it `#[serde(skip)]`, meaning it can never be serialized or deserialized from the config file. It is purely a runtime/internal test concern. The docs incorrectly imply it is user-configurable.

**`check_config_exists` formatting** — `src/config/file.rs:239`: `/// Checks if the config file exists at the given path OR  default path if None).` has a **double space** (`OR  default`) and an **unbalanced parenthesis** (closing `)` with no opening `(`).

**`docs/development.md` stale guidance** — `docs/development.md:15` says "Debug output should always start with `DEBUG:`." This contradicts the project's `AGENTS.md` which forbids `DEBUG:` in `.rs` files (verified by `scripts/test.sh` grep).

## Q6: Typo and misleading text locations

### Findings

**(a) `overriden` in `SetTimezone`** — `src/commands/config_commands.rs:44-46`:
```rust
#[clap(alias = "tz")]
/// (tz) Automatically set the timezone to your Todoist timezone. Can be overriden with the --timezone flag.
SetTimezone(SetTimezone),
```
Should be `"overridden"` (double-d).

**(b) `Date date` in `Create.due`** — `src/commands/task_commands.rs:57-59`:
```rust
#[arg(short = 'u', long)]
/// Date date in format YYYY-MM-DD, YYYY-MM-DD HH:MM, or natural language
due: Option<String>,
```
Should be `"Due date"` not `"Date date"`.

**(c) `Empty.project` help text** — `src/commands/project_commands.rs:119-122`:
```rust
#[derive(Parser, Debug, Clone)]
pub struct Empty {
    #[arg(short, long)]
    /// Project to remove
    project: Option<String>,
```
The `Empty` command moves all tasks out of a project to other projects (does not remove/delete the project). `"Project to remove"` is misleading; the `Delete.command` is for removal. The doc should describe emptying/moving tasks.

## Q7: Rustdoc linting tooling

### Findings

**`#![deny(missing_docs)]` coverage** — applies to: structs, enums, enum variants, public struct fields, unions, free functions, traits, trait methods, trait associated functions/types/consts, type aliases, constants/statics, `macro_rules!` macros, foreign items in `extern` blocks, and **inherent impl methods/associated items**. Does NOT cover: `impl` blocks themselves, items inside trait implementations (docs inherited from trait), `use`/re-export declarations, `extern crate`, `#[doc(hidden)]` items, `#[test]` functions. Allow-by-default; enable with `#![warn(missing_docs)]` or `#![deny(missing_docs)]`. Works with both `rustc` and `rustdoc`.

**Other rustdoc lints:**
- `rustdoc::broken_intra_doc_links` — warn by default, detects unresolvable intra-doc links
- `rustdoc::missing_crate_level_docs` — allow by default, detects no `//!` at crate root
- `rustdoc::missing_doc_code_examples` — allow by default, **nightly-only**, detects public items without code examples (skips impl blocks, variants, fields, type aliases, statics, modules)
- `rustdoc::private_intra_doc_links`, `rustdoc::private_doc_tests`, `rustdoc::invalid_codeblock_attributes`, `rustdoc::invalid_html_tags`, `rustdoc::invalid_rust_codeblocks`, `rustdoc::bare_urls`

**Clippy doc lints:**
- `clippy::missing_errors_doc` — pedantic (allow by default), requires `# Errors` section on fns returning `Result`
- `clippy::missing_panics_doc` — pedantic (allow by default), requires `# Panics` section on fns that may panic
- `clippy::missing_safety_doc` — style (warn by default), requires `# Safety` section on `unsafe fn`
- `clippy::missing_docs_in_private_items` — restriction (allow by default), like `missing_docs` but for non-public items

**Phased rollout pattern:**
- **Phase 0**: Add `#![warn(missing_docs)]`, capture warning list as backlog
- **Phase 1**: Place `#[allow(missing_docs)]` on undocumented modules, keep global `warn`
- **Phase 2**: Graduate module-by-module: remove `allow`, document everything, add regression guard
- **Phase 3**: Once all modules graduated, flip to `#![deny(missing_docs)]` or rely on `-D warnings` in CI

**CI setup** — requires both `RUSTFLAGS="-D warnings"` (for `cargo check`/`clippy`) and `RUSTDOCFLAGS="-D warnings"` (for `cargo doc`) since they are independent flag sets. Projects like [rust-lightning](https://github.com/lightningdevkit/rust-lightning) use `#![deny(missing_docs)]` + CI doc checks.

## Cross-Cutting Observations

- **Doc style is consistently minimal** across the entire codebase. The `src/errors.rs` heading-section + intra-doc-link approach is aspirational but not replicated anywhere. The dominant convention is bare `///` one-liners with no formatting.
- **Struct field documentation is the biggest gap** — `Task` (25 fields, 1 documented), `Project` (18 fields, 0 documented). The only near-complete field docs are on `DateInfo` and `Deadline`, using value-example style.
- **`//` instead of `///` is a recurring pattern** — `max_comment_length()` and `config_reset_with_prompt()` use line comments instead of doc comments, making them invisible to `cargo doc`.
- **The `timeprovider` config doc is misleading** — it appears user-configurable but has `#[serde(skip)]`.
- **`docs/usage.md` is stale** — the `-h` output was pasted manually and hasn't been updated to include `--json`/`-j` or `--timeout`.
- **Documentation is unevenly distributed** — the `todoist` API client has ~61% doc coverage; the `tasks` business logic has ~23%; command handlers have 0% on dispatch functions.

## Open Areas

- **`missing_doc_code_examples` is nightly-only** — cannot be used on stable Rust. No equivalent stable lint exists for requiring code examples.
- **No current CI doc enforcement** — the project has no `#![warn(missing_docs)]` attribute or CI doc-check step.
- **The `Body` struct** at `src/tasks/mod.rs:162-165` is `#[allow(dead_code)]` and unused — may be vestigial.
