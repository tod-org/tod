# Design Discussion

## Current State

The codebase has **uneven documentation coverage**. The `src/todoist/` API client has ~61% of its
public functions documented; the `src/tasks/` business-logic layer has ~23%. Command handlers and
config types have near-zero coverage.

**Doc style is consistently minimal.** The dominant convention across `src/todoist/mod.rs` (19 docs),
`src/tasks/mod.rs` (12 fn docs), and `src/projects.rs` (11 docs) is bare `///` one-liners with no
formatting (`src/todoist/mod.rs:47-110`, `src/tasks/mod.rs:384-396`). The single exception is
`src/errors.rs:13-22`, which uses `#` heading sections and four intra-doc links — a pattern not
replicated anywhere else in the codebase.

**Struct field documentation is the largest gap.** `Task` at `src/tasks/mod.rs:26-61` has 1/25
fields documented (only `note_count` with an API caveat). `Project` at `src/projects.rs:19-39` has
0/18. The only reasonably complete field docs are on `DateInfo` (`src/tasks/mod.rs:128-139`, 4/5
fields) and `Deadline` (`src/tasks/mod.rs:147-152`, 2/2), using value-example style (`"2025-04-26
15:00"`, `i.e. "en"`).

**`//` line comments instead of `///` doc comments** appear on two functions —
`max_comment_length()` at `src/config/mod.rs:322` and `config_reset_with_prompt()` at
`src/config/file.rs:130` — making them invisible to `cargo doc`.

**Non-code docs are stale or misleading:**
- `docs/usage.md:20-36` shows manual `-h` output omitting `--json`/`-j` (`src/commands/mod.rs:64`)
  and `--timeout`/`-t` (`src/commands/mod.rs:58-60`).
- `docs/configuration.md:29-57` example JSON misses 5 fields (`task_create_command`,
  `task_comment_command`, `task_complete_command`, `task_exclude_regex`, `comment_exclude_regex`)
  despite each having its own `###` subsection in the Values section.
- `docs/configuration.md:283-289` documents `timeprovider` as user-configurable, but
  `src/config/mod.rs:120-121` marks it `#[serde(skip)]` — it is purely a test concern.
- `docs/development.md:15` says "Debug output should always start with `DEBUG:`" which contradicts
  `AGENTS.md` and the `scripts/test.sh` grep that forbids it.

**Three typos/misleading texts** exist in clap arg docs:
- `src/commands/config_commands.rs:44-46`: `"overriden"` → `"overridden"`
- `src/commands/task_commands.rs:57-59`: `"Date date"` → `"Due date"`
- `src/commands/project_commands.rs:119-122`: `Empty.project` says `"Project to remove"` but the
  command moves tasks out (does not delete); `Delete` is for removal.

**No CI doc enforcement.** There is no `#![warn(missing_docs)]` attribute, no `cargo doc` step in
CI, and no rustdoc lint configuration.

A vestigial `Body` struct at `src/tasks/mod.rs:162-165` is `#[allow(dead_code)]` and unused.

## Desired End State

1. **79 public API items documented** with `///` one-liners in the consistent codebase style.
   Coverage extends across all five tiers: core data model, Todoist client, command handlers,
   config subsystem, and other modules.

2. **Struct fields documented** for non-obvious fields only (bool flags, nested structs,
   fields with derived meaning like `is_today`/`is_overdue`). Obvious fields mapping directly to
   Todoist API names (e.g., `parent_id`, `section_id`) are skipped. Value-example style used where
   helpful, matching `DateInfo` at `src/tasks/mod.rs:128-139`.

3. **5 `//` comments converted to `///`** so they appear in `cargo doc` output.

4. **3 typos/misleading texts fixed** in clap arg docs.

5. **4 non-code doc issues resolved:** usage.md updated with `--json` and `--timeout`;
   configuration.md example JSON filled in and `timeprovider` section corrected or removed;
   development.md `DEBUG:` guidance removed.

6. **`Body` struct removed.**

7. **`#![warn(missing_docs)]` added** to `src/main.rs` and every public item documented to
   silence the warning. This prevents regressions on all future public API additions.

8. **Verification:** `cargo doc` runs without warnings; `scripts/test.sh` passes; `cargo clippy`
   passes; manual review confirms style consistency.

## Patterns to Follow

### Use these patterns (with file:line refs)

- **Bare `///` one-liners** — the dominant convention. `src/todoist/mod.rs:47-110` shows 19
  consistent examples. Use for all function docs, struct type-level docs, and enum variant docs.
  No markdown, no code blocks, no headings.

- **Module-level `//!` docs** — `src/todoist/mod.rs:1-5` shows the pattern: summary line +
  coverage list. Use for modules that need an overview.

- **Value-example field docs** — `src/tasks/mod.rs:128-139` (DateInfo) and
  `src/tasks/mod.rs:147-152` (Deadline). Format: `/// ISO date string, e.g. "2025-04-26 15:00"`.
  Use for fields with non-obvious formats or edge cases.

- **Intra-doc links for cross-references** — `src/errors.rs:18-21` uses `` [`format::red_string`] ``
  style. Use sparingly when one item's behavior depends on another module's function.

- **`--json` mode guard pattern** — `src/commands/mod.rs:440-444` blocks interactive prompts and
  spinners when JSON mode is active. New handler docs should note this behavior where relevant.

### Do NOT follow these patterns

- **`//` line comments on public items** — `src/config/mod.rs:322` and `src/config/file.rs:130`.
  Always use `///` for public API docs. `//` is for internal implementation notes only.

- **Heading-section doc style** — `src/errors.rs:13-22`. This is aspirational but inconsistent
  with the rest of the codebase. Do not replicate it for new docs.

- **Manual `-h` output in prose docs** — `docs/usage.md:20-36` shows pasted CLI output that
  drifts out of date. Reference flag names in prose instead of reproducing terminal output.

- **Documenting `#[serde(skip)]` fields as user-configurable** — `docs/configuration.md:283-289`.
  Internal/test-only fields must not appear in user-facing configuration docs.

## Design Decisions

1. **Doc style: bare `///` one-liners (Option A)** — The dominant codebase convention wins. The
   `src/errors.rs` heading-section style, while richer, has zero adoption outside that one file and
   would create inconsistency alongside 79 new docs. Intra-doc links may be used sparingly for
   genuine cross-module dependencies (e.g., handler docs referencing `fetch_*` helpers).

2. **Field docs: non-obvious only (Option B)** — Full field documentation for `Task` (25 fields)
   and `Project` (18 fields) would add ~50 field docs, many of which mirror the Todoist REST API
   schema and are self-documenting (`parent_id`, `section_id`, `project_id`). Focus effort on fields
   with derived meaning (bool flags like `is_today`, `is_overdue`, `is_recurring`), nested struct
   fields (`deadline`, `duration`, `due`), and fields with API caveats (like `note_count`).

3. **Lint enforcement: add `#![warn(missing_docs)]` now (Option A)** — Adding the lint as part of
   this work ensures the 79 documented items stay documented and prevents future public API additions
   from regressing. Any undocumented public items not in the audit scope must also be documented
   to silence the warning. Place at top of `src/main.rs` (there is no `lib.rs` — this is a binary
   crate). The lint covers structs, enums, enum variants, public fields, free functions, trait
   methods, type aliases, constants, macros, and inherent impl methods.

4. **Non-code docs: fix all 4 issues (Option A)** — `docs/development.md:15` contradicts
   `AGENTS.md` and `scripts/test.sh`, making it a correctness issue, not just staleness. Fixing it
   alongside the usage.md and configuration.md issues is low-cost and prevents contributor confusion.

5. **Body struct: remove it (Option A)** — `Body` at `src/tasks/mod.rs:162-165` is private,
   `#[allow(dead_code)]`, and unused. Removing it reduces the undocumented surface and cleans up dead
   code. Since it is private, `#![warn(missing_docs)]` would not flag it either way.

## What We're NOT Doing

- **Not touching `src/errors.rs`** — it is already well-documented and uses a different style.
  Leave it as-is.
- **Not adding `missing_doc_code_examples`** — it is nightly-only and cannot be used on stable
  Rust.
- **Not adding clippy pedantic lints** (`missing_errors_doc`, `missing_panics_doc`,
  `missing_safety_doc`) — these require `# Errors`, `# Panics`, and `# Safety` sections that are
  inconsistent with the one-liner style. Auditing every function for error variants and panic
  conditions is out of scope.
- **Not rewriting existing docs** — only undocumented items, the 5 `//`→`///` conversions, and
  the 3 typos are in scope. Existing one-liner docs are left unchanged even if they could be more
  detailed.
- **Not adding a CI `cargo doc` step** — this design adds the lint attribute; adding a CI check is
  a follow-up concern.
- **Not documenting private items** — `#![warn(missing_docs)]` only covers public API surface.
  Private functions, modules, and types are out of scope.

## Open Risks

- **Undocumented public items beyond the audit**: `#![warn(missing_docs)]` will flag every public
  item, not just the 79 in the audit. Discovery during implementation may reveal additional
  undocumented items that must be documented to silence the warning. This is the right thing to do
  but may increase implementation scope beyond the known list.
- **`missing_docs` on inherent impl methods**: The lint covers `pub fn` methods inside `impl`
  blocks — e.g., `Config::set_token()`, `Config::next_task()`. Some of these are simple getters
  (`src/config/mod.rs:458`); deciding the doc text for trivial accessors requires judgment.
- **`usage.md` rewrite scope**: Updating the `-h` output section may balloon if we decide to
  reproduce current help text. The design calls for prose references instead — verify this is
  acceptable during implementation.
- **`timeprovider` removal from docs**: Removing the section from `docs/configuration.md` is
  straightforward, but the field is still referenced in the Values sections. If any other prose
  references `timeprovider`, those must be found and updated.
- **Downstream breakage from `Body` removal**: `Body` is `#[allow(dead_code)]` and unused, but
  other code may refer to it transitively (e.g., test fixtures). Verify no references exist before
  removal.
