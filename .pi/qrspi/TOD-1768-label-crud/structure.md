# Structure Outline

## Approach

Add `tod label create`, `tod label update`, and `tod label delete` CLI commands by following the existing patterns in `project_commands`/`section_commands`. Each phase delivers a complete, testable vertical slice — from API call to CLI output — rather than building all database/API/service layers before touching the UI.

---

## Phase 1: `tod label create` — first vertical slice

Delivers the `tod label create` command end-to-end: API call, business-logic wrapper, CLI handler, dispatch wiring, and tests at every layer. Establishes the `label_commands` module, `LabelCommands` enum, and dispatch infrastructure that Phases 2–3 build on.

**Files**: `src/labels.rs`, `src/todoist/mod.rs`, `src/commands/label_commands.rs` (new), `src/commands/mod.rs`, `src/test/responses.rs`

**Key changes**:

- `Label::from_json(json: &str) -> Result<Label, Error>` — new standalone method on `Label`
- `labels::create(config: &Config, name: &str, color: Option<&str>, order: Option<u32>, is_favorite: bool) -> Result<Label, Error>` — new business-logic wrapper (thin pass-through to `todoist::create_label`, spinner always `true`)
- `todoist::create_label(config: &Config, name: &str, color: Option<&str>, order: Option<u32>, is_favorite: bool, spinner: bool) -> Result<Label, Error>` — new API function, POST to `/api/v1/labels`, returns `Label` from single-object response
- `LabelCommands { Create(Create) }` — new subcommand enum in `label_commands.rs` (only `Create` variant in this phase)
- `Create { name: Option<String>, color: Option<String>, order: Option<u32>, is_favorite: bool }` — new arg struct (clap `Parser`)
- `label_commands::create(config: &Config, args: &Create, json: bool) -> Result<String, Error>` — new handler, resolves `name` via `fetch_string`, calls `labels::create`, formats output
- `Commands::Label(LabelCommands)` — new variant with `#[clap(alias = "b")]`
- `select_command()` match arm — `Commands::Label(command) => label_command(command, &cli, &tx).await`
- `label_command()` — new dispatch function matching `LabelCommands::Create`
- `mod label_commands;` + `use label_commands::LabelCommands;` — module registration
- Remove `#[allow(dead_code)]` from `ResponseFromFile::Label` (line 27 of `src/test/responses.rs`)

**Tests**:

| Layer | Test | What it verifies |
|-------|------|------------------|
| `src/labels.rs` | `test_label_from_json_valid` | Parses single label JSON |
| `src/labels.rs` | `test_label_from_json_invalid` | Returns error on bad JSON |
| `src/todoist/mod.rs` | `test_create_label` | POST hits `/api/v1/labels`, returns `Label` |
| `src/commands/label_commands.rs` | `create_fails_in_json_mode_without_name` | JSON mode + no `--name` → error |

**Verify**: `cargo test` passes all four new tests. Manually: `tod label create --name "test-label" --color red` creates a real label (visible in Todoist UI), `tod label create --name "test-label" --json` returns JSON with `id`/`name`/`color`/`order`/`is_favorite`.

---

## Phase 2: `tod label update` and `tod label delete`

Adds the remaining two commands on top of Phase 1's infrastructure. Introduces `fetch_label()` for label resolution by name/ID and the `LABEL` prompt constant. The `LabelCommands` enum grows from one variant to three.

**Files**: `src/todoist/mod.rs`, `src/labels.rs`, `src/commands/label_commands.rs`, `src/commands/mod.rs`, `src/input.rs`

**Key changes**:

- `todoist::update_label(config: &Config, label_id: &str, name: Option<&str>, color: Option<&str>, order: Option<u32>, is_favorite: Option<bool>, spinner: bool) -> Result<Label, Error>` — POST to `/api/v1/labels/{id}`, all fields optional, returns updated `Label`
- `todoist::delete_label(config: &Config, label_id: &str, spinner: bool) -> Result<String, Error>` — DELETE `/api/v1/labels/{id}`, returns `Ok("✓".into())`
- `labels::update(config: &Config, label_id: &str, name: Option<&str>, color: Option<&str>, order: Option<u32>, is_favorite: Option<bool>) -> Result<Label, Error>` — thin wrapper, spinner always `true`
- `labels::delete(config: &Config, label_id: &str) -> Result<String, Error>` — thin wrapper, spinner always `true`
- `LabelCommands` gains `Update(Update)` and `Delete(Delete)` variants
- `Update { label: Option<String>, name: Option<String>, color: Option<String>, order: Option<u32>, favorite: Option<bool> }` — new arg struct, at least one update field required
- `Delete { label: Option<String>, force: bool }` — new arg struct, `--force` skips confirmation
- `label_commands::update(config, args, json)` — fetches labels, resolves target via `fetch_label`, validates at least one update field, calls `labels::update`
- `label_commands::delete(config, args, json)` — fetches labels, resolves target, confirmation prompt (canceled in `--force` or `--json` mode), calls `labels::delete`
- `commands::fetch_label<'a>(arg: &str, config, labels: &[Label]) -> Result<&Label, Error>` — resolves by ID first, then name; falls back to interactive select (with JSON-mode guard)
- `input::LABEL: &str = "Label:"` — new prompt constant in `src/input.rs`
- `label_command()` gains `Update` and `Delete` match arms

**Tests**:

| Layer | Test | What it verifies |
|-------|------|------------------|
| `src/todoist/mod.rs` | `test_update_label` | POST hits `/api/v1/labels/123`, returns `Label` |
| `src/todoist/mod.rs` | `test_delete_label` | DELETE hits `/api/v1/labels/123`, returns `"✓"` |
| `src/commands/label_commands.rs` | `delete_fails_when_label_not_found` | Unknown label → `fetch_label` error |
| `src/commands/label_commands.rs` | `delete_force_skips_confirmation` | `--force` bypasses prompt, label deleted |

**Verify**: `cargo test` passes all Phase 1 + Phase 2 tests. Manually: `tod label update --label "test-label" --name "renamed"` renames a label. `tod label delete --label "renamed" --force` deletes it without confirmation. `tod label delete --label "renamed"` (interactive) shows a confirmation prompt.

---

## Phase 3: Docs and final verification

Updates `docs/usage.md` with label command examples, runs the full pre-commit checklist, and ensures all dead-code annotations are cleaned up.

**Files**: `docs/usage.md`

**Key changes**:
- New "Labels" section in `docs/usage.md`, placed after the existing "Sections" examples, following the same format with `tod label create`, `tod label update`, and `tod label delete` examples

**Verify**: `scripts/test.sh` passes (fmt, build, clippy, tests, forbidden strings). `tod -h` shows `label`/`b` in command list. `tod label -h` shows all three subcommands with correct aliases.

---

## Testing Checkpoints

After each phase, the following should be true:

| Phase | Checkpoint |
|-------|-----------|
| 1 | `tod label create --name "test" --json` succeeds; `cargo test` passes with 4 new tests; `#[allow(dead_code)]` gone from `ResponseFromFile::Label` |
| 2 | All three label subcommands work end-to-end; `cargo test` passes with 8 total new tests; `LABEL` constant exists in `input.rs` |
| 3 | `scripts/test.sh` passes clean; `docs/usage.md` has label examples; `tod -h` lists the `b` alias |
