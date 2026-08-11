# Plan: Delete Section Endpoint

## Overview
Add `DELETE /api/v1/sections/{id}` API support and a `tod section delete` CLI subcommand.

## Phases

### Phase 1: API function — `src/todoist/mod.rs`
Add `delete_section` function + API-level test.

- [x] Add `delete_section` function after `create_section`
- [x] Add `test_delete_section` test

**Verification:**
- [x] `cargo test delete_section` passes

### Phase 2: CLI handler — `src/commands/section_commands.rs`
Add `Delete` variant, args struct, handler + CLI-level tests.

- [x] Add `Delete` variant to `SectionCommands` enum
- [x] Add `Delete` args struct
- [x] Add `delete` handler function
- [x] Add tests: `delete_fails_when_section_not_found`, `delete_force_skips_confirmation`, `delete_cancels_when_user_selects_cancel`

**Verification:**
- [x] `cargo test section_commands` passes
- [x] `cargo test` (full suite) passes

### Phase 3: Dispatch — `src/commands/mod.rs`
Add dispatch arm for `SectionCommands::Delete`.

- [x] Add `Delete` arm in `section_command()`

**Verification:**
- [x] `cargo build` succeeds (dispatch is pattern-matched exhaustively)
- [x] `scripts/test.sh` passes

### Phase 4: Docs — `docs/usage.md`
Add section delete usage examples.

- [x] Add `tod section create` and `tod section delete` examples

**Verification:**
- [ ] Manual review of docs
