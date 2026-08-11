# Design: Delete Section Endpoint

## Overview

Add `DELETE /api/v1/sections/{id}` API support and a `tod section delete` CLI subcommand. Fills the gap where sections can be created and listed but not deleted or renamed.

## API Contract

| Attribute | Value |
|---|---|
| **Endpoint** | `DELETE /api/v1/sections/{section_id}` |
| **Success** | `200 OK`, body: `null` |
| **Errors** | `400`, `401`, `403`, `404` |
| **Destructive** | ⚠️ Deleting a section **also deletes all tasks inside it** (per official API v1 docs). |

## Implementation Plan

### Step 1: `src/todoist/mod.rs` — add `delete_section`

Insert after `create_section` (line ~686), following the `delete_task`/`delete_project` pattern:

```rust
/// Deletes a section by ID.
pub async fn delete_section(
    config: &Config,
    section_id: &str,
    spinner: bool,
) -> Result<String, Error> {
    let url = format!("{SECTIONS_URL}/{}", section_id);
    let body = json!({});

    request::delete_todoist(config, &url, body, spinner).await?;
    Ok("✓".into())
}
```

- URL: `format!("{SECTIONS_URL}/{}", section_id)` — `SECTIONS_URL` has no trailing slash (unlike `TASKS_URL`), so we interpolate `/{id}`
- Body: `json!({})` per convention (empty object, not `Value::Null`)
- Return: `Ok("✓")` — response body (`null`) is discarded, matching all other delete functions

### Step 2: `src/commands/section_commands.rs` — add `Delete` variant and handler

**Add to `SectionCommands` enum:**
```rust
#[derive(Subcommand, Debug, Clone)]
pub enum SectionCommands {
    #[clap(alias = "c")]
    Create(Create),
    #[clap(alias = "d")]
    Delete(Delete),
}
```

**Add `Delete` args struct:**
```rust
#[derive(Parser, Debug, Clone)]
pub struct Delete {
    #[arg(short, long, default_value_t = false)]
    /// Skip deletion confirmation
    force: bool,

    #[arg(short = 'r', long, default_value_t = false)]
    /// Keep repeating prompt to delete sections
    repeat: bool,

    #[arg(short, long)]
    /// Section to delete
    section: Option<String>,

    #[arg(short = 'p', long)]
    /// Project the section belongs to
    project: Option<String>,
}
```

Flag notes:
- `-s`/`--section`: section name (matches `Create`'s `-s`/`--name` usage pattern; short flags are scoped to the subcommand)
- `-p`/`--project`: project name (same as `Create`)
- `-f`/`--force`: skip confirmation (consistent with `project delete`)
- `-r`/`--repeat`: loop mode (consistent with `project delete`)

**Add `delete` handler:**
```rust
/// Deletes a section from a Todoist project.
pub async fn delete(config: &Config, args: &Delete, json: bool) -> Result<String, Error> {
    let Delete { force, section, project, repeat } = args;
    loop {
        let project = match super::fetch_project(project.as_deref(), config).await? {
            Flag::Project(project) => project,
            Flag::Filter(_) => unreachable!(),
        };

        let sections = todoist::all_sections_by_project(config, &project, None).await?;

        if sections.is_empty() {
            return Ok("No sections found for this project".into());
        }

        let section = if let Some(name) = section {
            sections
                .iter()
                .find(|s| s.name == *name)
                .cloned()
                .ok_or_else(|| {
                    Error::new(
                        "delete_section",
                        format!("Section \"{name}\" not found in project \"{}\"", project.name),
                    )
                })?
        } else if json {
            return Err(Error::new("json_mode", super::JSON_INTERACTIVE_ERROR));
        } else {
            let section_names: Vec<String> = sections.iter().map(|s| s.name.clone()).collect();
            let selected = input::select(input::SECTION, section_names, config.mock_select)?;
            sections.into_iter().find(|s| s.name == selected).unwrap()
        };

        if !force {
            if json {
                return Err(Error::new("json_mode", super::JSON_INTERACTIVE_ERROR));
            }
            let options = vec![input::CANCEL, input::DELETE];
            let desc = format!("Delete section \"{}\"? Tasks inside will also be deleted.", section.name);
            let result = input::select(&desc, options, config.mock_select)?;
            if result == input::CANCEL {
                return Ok("Cancelled".into());
            }
        }

        todoist::delete_section(config, &section.id, true).await?;

        if !repeat {
            if json {
                return Ok(serde_json::to_string(&section)?);
            }
            return Ok(format::green_string(&format!(
                "Section \"{}\" deleted",
                section.name
            )));
        }
    }
}
```

**Handler logic:**
1. Resolve project via `fetch_project` (errors in JSON mode if `--project` not provided)
2. Fetch all sections for that project
3. Resolve section: by `--section` name match, or interactive select (errors in JSON mode)
4. Confirmation prompt (skipped with `--force`): warns that tasks inside will also be deleted
5. Call `todoist::delete_section`
6. Output: JSON mode returns serialized `Section`; text mode returns green success message
7. Loop if `--repeat`

### Step 3: `src/commands/mod.rs` — add dispatch arm

In `section_command()` (line ~196), add after the `Create` arm:

```rust
SectionCommands::Delete(args) => {
    let config = fetch_config(cli, tx).await?;
    let result = section_commands::delete(&config, args, cli.json).await;
    Ok(build_command_result(result, &config))
}
```

Follows the existing `Create` pattern exactly: owned config, immutable borrow, `cli.json` passed through.

### Step 4: Tests

#### `src/todoist/mod.rs` — API-level test

Add after existing `test_delete_task`:

```rust
#[tokio::test]
async fn test_delete_section() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("DELETE", "/api/v1/sections/1234")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body("null")
        .create_async()
        .await;

    let config = test::fixtures::config()
        .await
        .with_mock_url(server.url());

    let result = delete_section(&config, "1234", false).await;
    assert_eq!(result, Ok("✓".into()));
    mock.assert();
}
```

#### `src/commands/section_commands.rs` — CLI-level tests

Three tests (patterned after `project_commands` delete tests):

```rust
#[tokio::test]
async fn delete_fails_when_section_not_found() { ... }
// Mock: GET sections for project returns fixture Sections
// Args: --section "nonexistent" --project "MyProject"
// Assert: error with message containing "not found"

#[tokio::test]
async fn delete_force_skips_confirmation() { ... }
// Mock: GET sections for project, DELETE section
// Args: --force --section "Bread" --project "MyProject"
// Assert: Ok with success message, DELETE mock was called

#[tokio::test]
async fn delete_cancels_when_user_selects_cancel() { ... }
// Mock: GET sections, mock_select(0) = CANCEL
// Args: --section "Bread" --project "MyProject" (no --force)
// Assert: Ok("Cancelled")
```

### Step 5: `docs/usage.md` — Add section delete examples

Add under a new `## Section` heading (or alongside existing section examples if they exist):

```markdown
### `tod section delete`

Delete a section from a Todoist project.
⚠️ This also deletes all tasks inside the section.

```bash
# Interactive: pick project, then section, then confirm
tod section delete

# Delete by name (non-interactive, still confirms)
tod section delete -s "Groceries" -p "Shopping"

# Skip confirmation
tod section delete -s "Groceries" -p "Shopping" --force

# Repeat mode — keep deleting sections until Ctrl+C
tod section delete -r
```
```

## Edge Cases & Risks

| Scenario | Behavior |
|---|---|
| Section has tasks | ⚠️ API deletes the tasks too. Confirmation prompt warns about this. |
| Project has no sections | Returns "No sections found for this project" (not an error). |
| Section name not found | Returns `Error` with descriptive message. |
| JSON mode, no `--section` | `fetch_project` or the section-select branch returns `JSON_INTERACTIVE_ERROR`. |
| JSON mode, no `--project` | `fetch_project` returns `JSON_INTERACTIVE_ERROR`. |
| JSON mode, no `--force` | Confirmation branch returns `JSON_INTERACTIVE_ERROR`. |
| Empty body response (`null`) | `handle_response` reads `"null"` as text; caller discards it — no issue. |

## Files Changed

| File | Change |
|---|---|
| `src/todoist/mod.rs` | +10 lines: `delete_section` function + doc comment |
| `src/commands/section_commands.rs` | +~80 lines: `Delete` variant, struct, handler, tests |
| `src/commands/mod.rs` | +4 lines: dispatch arm |
| `docs/usage.md` | +~15 lines: usage examples |

## Dependencies

- No new dependencies. Uses existing `request::delete_todoist`, `input::select`, `input::CANCEL`, `input::DELETE`, `input::SECTION`, `fetch_project`, `all_sections_by_project`, `Flag`, `Error`, `format::green_string`.
