# Design: Label CRUD (create, update, delete labels)

## Overview

Add `tod label create`, `tod label update`, and `tod label delete` CLI commands, backed by API functions in `src/todoist/mod.rs` and business-logic wrappers in `src/labels.rs`. The existing `all_labels` GET endpoint and `Label` struct already exist — this adds the mutation side.

## Files to change

| File | Type | What |
|------|------|------|
| `src/labels.rs` | Modify | Add `Label::from_json()`, `create()`, `update()`, `delete()` |
| `src/todoist/mod.rs` | Modify | Add `create_label()`, `update_label()`, `delete_label()` |
| `src/commands/label_commands.rs` | **New** | CLI arg structs, handler functions, tests |
| `src/commands/mod.rs` | Modify | Register `LabelCommands`, dispatch, handler |
| `docs/usage.md` | Modify | Add label command examples |

`tests/responses/Label.json` and `src/test/fixtures.rs::label()` already exist — no new fixtures needed.

## 1. `src/labels.rs` — business logic layer

### 1.1 Add `Label::from_json()`

Follow the pattern in `src/projects.rs:92` / `src/sections.rs` (exact line TBD). The struct already derives `Deserialize`, so the method is a one-liner:

```rust
impl Label {
    pub fn from_json(json: &str) -> Result<Label, Error> {
        let label: Label = serde_json::from_str(json)?;
        Ok(label)
    }
}
```

Place after the existing `impl LabelResponse` block and before `impl Display for Label`.

### 1.2 Add business-logic functions

After the existing `get_labels()` function at line 33:

```rust
pub async fn create(
    config: &Config,
    name: &str,
    color: Option<&str>,
    order: Option<u32>,
    is_favorite: bool,
) -> Result<Label, Error> {
    todoist::create_label(config, name, color, order, is_favorite, true).await
}

pub async fn update(
    config: &Config,
    label_id: &str,
    name: Option<&str>,
    color: Option<&str>,
    order: Option<u32>,
    is_favorite: Option<bool>,
) -> Result<Label, Error> {
    todoist::update_label(config, label_id, name, color, order, is_favorite, true).await
}

pub async fn delete(config: &Config, label_id: &str) -> Result<String, Error> {
    todoist::delete_label(config, label_id, true).await
}
```

These are thin wrappers that always pass `spinner: true`, matching the pattern of `get_labels()` and `projects::create()`.

### 1.3 Add unit tests for `from_json`

Add to the existing `mod tests` block:

```rust
#[test]
fn test_label_from_json_valid() {
    let json = r#"{"id":"1","name":"work","color":"red","order":1,"is_favorite":false}"#;
    let label = Label::from_json(json).expect("should parse label");
    assert_eq!(label.id, "1");
    assert_eq!(label.name, "work");
    assert_eq!(label.color, "red");
    assert_eq!(label.order, Some(1));
    assert!(!label.is_favorite);
}

#[test]
fn test_label_from_json_invalid() {
    let result = Label::from_json("not json");
    assert!(result.is_err());
}
```

## 2. `src/todoist/mod.rs` — API layer

### 2.1 `create_label()`

Place after `all_labels()` (after line ~463) and before `move_task_to_project`.

```rust
/// Create a personal label.
pub async fn create_label(
    config: &Config,
    name: &str,
    color: Option<&str>,
    order: Option<u32>,
    is_favorite: bool,
    spinner: bool,
) -> Result<Label, Error> {
    let mut body = json!({"name": name, "is_favorite": is_favorite});
    if let Some(c) = color {
        body["color"] = json!(c);
    }
    if let Some(o) = order {
        body["order"] = json!(o);
    }

    let response = request::post_todoist(config, LABELS_URL, body, spinner).await?;
    Label::from_json(&response)
}
```

**Design decisions:**
- `name` is required (API requires it), `color`, `order`, `is_favorite` are optional
- Only include optional fields in the body when `Some` — avoids sending `null` for unset values
- URL constant: `LABELS_URL` already exists at line 42 (`"/api/v1/labels"`)
- Returns `Label` (single object, not paginated — the Todoist API returns the created label inline)
- Uses `post_todoist` (POST), matching the Todoist REST API

### 2.2 `update_label()`

```rust
/// Update a personal label by ID.
pub async fn update_label(
    config: &Config,
    label_id: &str,
    name: Option<&str>,
    color: Option<&str>,
    order: Option<u32>,
    is_favorite: Option<bool>,
    spinner: bool,
) -> Result<Label, Error> {
    let mut body = json!({});
    if let Some(n) = name {
        body["name"] = json!(n);
    }
    if let Some(c) = color {
        body["color"] = json!(c);
    }
    if let Some(o) = order {
        body["order"] = json!(o);
    }
    if let Some(f) = is_favorite {
        body["is_favorite"] = json!(f);
    }

    let url = format!("{}/{}", LABELS_URL, label_id);
    let response = request::post_todoist(config, &url, body, spinner).await?;
    Label::from_json(&response)
}
```

**Design decisions:**
- All fields optional — user can update just one field
- URL format `{LABELS_URL}/{id}` — same pattern as `delete_project` uses `{PROJECTS_URL}/{id}`
- Uses POST (Todoist REST API uses POST for mutations, not PUT/PATCH)
- Returns `Label` — API returns the updated label object

### 2.3 `delete_label()`

```rust
/// Delete a personal label by ID.
pub async fn delete_label(
    config: &Config,
    label_id: &str,
    spinner: bool,
) -> Result<String, Error> {
    let url = format!("{}/{}", LABELS_URL, label_id);
    request::delete_todoist(config, &url, json!({}), spinner).await?;
    Ok("✓".into())
}
```

**Design decisions:**
- URL format `{LABELS_URL}/{id}`
- Passes `json!({})` as body (empty object) — consistent with `delete_project` and `delete_section` callers
- Ignores response body (204 No Content from API; `handle_response` returns `""`)
- Returns `Ok("✓".into())` — matching `delete_project`, `delete_section`, `archive_project`

### 2.4 API tests

Add to the tests module in `src/todoist/mod.rs` (find the section tests and add after them):

```rust
#[tokio::test]
async fn test_create_label() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/api/v1/labels")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(ResponseFromFile::Label.read().await)
        .create_async()
        .await;

    let config = test::fixtures::config().await.with_mock_url(server.url());

    let result = create_label(&config, "test-label", Some("red"), None, false, false).await;
    assert_eq!(result, Ok(test::fixtures::label()));
    mock.assert();
}

#[tokio::test]
async fn test_update_label() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/api/v1/labels/123")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(ResponseFromFile::Label.read().await)
        .create_async()
        .await;

    let config = test::fixtures::config().await.with_mock_url(server.url());

    let result =
        update_label(&config, "123", Some("new-name"), Some("blue"), None, None, false).await;
    assert_eq!(result, Ok(test::fixtures::label()));
    mock.assert();
}

#[tokio::test]
async fn test_delete_label() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("DELETE", "/api/v1/labels/123")
        .with_status(204)
        .create_async()
        .await;

    let config = test::fixtures::config().await.with_mock_url(server.url());

    let result = delete_label(&config, "123", false).await;
    assert_eq!(result, Ok("✓".into()));
    mock.assert();
}
```

Note: `ResponseFromFile::Label` currently has `#[allow(dead_code)]` at `src/test/responses.rs:27-28`. Remove that attribute when the test file uses it.

## 3. `src/commands/label_commands.rs` — CLI layer (new file)

### 3.1 Subcommand enum and arg structs

```rust
use crate::{config::Config, errors::Error, format, input, labels};
use clap::{Parser, Subcommand};

/// Label subcommands.
#[derive(Subcommand, Debug, Clone)]
pub enum LabelCommands {
    #[clap(alias = "c")]
    /// (c) Create a new personal label
    Create(Create),
    #[clap(alias = "u")]
    /// (u) Update an existing personal label
    Update(Update),
    #[clap(alias = "d")]
    /// (d) Delete a personal label
    Delete(Delete),
}

#[derive(Parser, Debug, Clone)]
pub struct Create {
    #[arg(short, long)]
    /// Label name
    name: Option<String>,

    #[arg(short, long)]
    /// Color for the label (e.g. "red", "blue", "green")
    color: Option<String>,

    #[arg(short, long)]
    /// Display order (1-based)
    order: Option<u32>,

    #[arg(short = 'f', long, default_value_t = false)]
    /// Mark label as a favorite
    is_favorite: bool,
}

#[derive(Parser, Debug, Clone)]
pub struct Update {
    #[arg(short, long)]
    /// Label to update (name or ID)
    label: Option<String>,

    #[arg(short, long)]
    /// New name for the label
    name: Option<String>,

    #[arg(short, long)]
    /// New color for the label
    color: Option<String>,

    #[arg(short, long)]
    /// New display order
    order: Option<u32>,

    #[arg(short = 'f', long)]
    /// Toggle favorite status (true or false)
    favorite: Option<bool>,
}

#[derive(Parser, Debug, Clone)]
pub struct Delete {
    #[arg(short, long)]
    /// Label to delete (name or ID)
    label: Option<String>,

    #[arg(short = 'f', long, default_value_t = false)]
    /// Skip deletion confirmation
    force: bool,
}
```

### 3.2 Handler functions

```rust
/// Creates a personal label.
pub async fn create(config: &Config, args: &Create, json: bool) -> Result<String, Error> {
    let Create {
        name,
        color,
        order,
        is_favorite,
    } = args;
    let name = super::fetch_string(name.as_deref(), config, input::NAME)?;

    let label = labels::create(config, &name, color.as_deref(), *order, *is_favorite).await?;
    if json {
        Ok(serde_json::to_string(&label)?)
    } else {
        Ok(format::green_string(&format!(
            "Label \"{}\" created",
            label.name
        )))
    }
}

/// Updates a personal label.
pub async fn update(config: &Config, args: &Update, json: bool) -> Result<String, Error> {
    let Update {
        label,
        name,
        color,
        order,
        favorite,
    } = args;

    // Fetch all labels and resolve the target by name or ID
    let labels_list = labels::get_labels(config, true).await?;
    let target = super::fetch_label(label.as_deref(), config, &labels_list)?;

    // Require at least one field to update
    if name.is_none() && color.is_none() && order.is_none() && favorite.is_none() {
        return Err(Error::new(
            "update_label",
            "At least one of --name, --color, --order, or --favorite is required",
        ));
    }

    let updated =
        labels::update(config, &target.id, name.as_deref(), color.as_deref(), *order, *favorite)
            .await?;
    if json {
        Ok(serde_json::to_string(&updated)?)
    } else {
        Ok(format::green_string(&format!(
            "Label \"{}\" updated",
            updated.name
        )))
    }
}

/// Deletes a personal label.
pub async fn delete(config: &Config, args: &Delete, json: bool) -> Result<String, Error> {
    let Delete { label, force } = args;

    let labels_list = labels::get_labels(config, true).await?;
    if labels_list.is_empty() {
        return Ok("No labels found".into());
    }

    let target = super::fetch_label(label.as_deref(), config, &labels_list)?;

    if !force {
        if json {
            return Err(Error::new("json_mode", super::JSON_INTERACTIVE_ERROR));
        }
        let options = vec![input::CANCEL, input::DELETE];
        let desc = format!("Delete label \"{}\"?", target.name);
        let result = input::select(&desc, options, config.mock_select)?;
        if result == input::CANCEL {
            return Ok("Cancelled".into());
        }
    }

    labels::delete(config, &target.id).await?;
    if json {
        Ok(serde_json::to_string(&target)?)
    } else {
        Ok(format::green_string(&format!(
            "Label \"{}\" deleted",
            target.name
        )))
    }
}
```

Note: `fetch_label()` is a new helper function (see §4.2 below) placed in `src/commands/mod.rs`. It resolves a label by ID or name from the labels list, with JSON-mode guard for interactive selection.

### 3.3 Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::test;
    use crate::test::responses::ResponseFromFile;

    #[tokio::test]
    async fn create_fails_in_json_mode_without_name() {
        let mut config = Config::default_test();
        config.args.json = true;
        let args = Create {
            name: None,
            color: None,
            order: None,
            is_favorite: false,
        };

        let error = create(&config, &args, true)
            .await
            .expect_err("creating a label without name in JSON mode should fail");

        assert_eq!(error.source, "json_mode");
    }

    #[tokio::test]
    async fn delete_fails_when_label_not_found() {
        let mut server = mockito::Server::new_async().await;

        let labels_mock = server
            .mock("GET", "/api/v1/labels?limit=200")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::Labels.read().await)
            .create_async()
            .await;

        let config = test::fixtures::config().await.with_mock_url(server.url());

        let args = Delete {
            label: Some("nonexistent".to_string()),
            force: false,
        };

        let error = delete(&config, &args, false)
            .await
            .expect_err("deleting a nonexistent label should fail");

        assert_eq!(error.source, "fetch_label");
        assert!(error.message.contains("not found"));
        labels_mock.assert_async().await;
    }

    #[tokio::test]
    async fn delete_force_skips_confirmation() {
        let mut server = mockito::Server::new_async().await;

        let labels_mock = server
            .mock("GET", "/api/v1/labels?limit=200")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::Labels.read().await)
            .create_async()
            .await;

        let delete_mock = server
            .mock("DELETE", "/api/v1/labels/123")
            .with_status(204)
            .create_async()
            .await;

        let config = test::fixtures::config().await.with_mock_url(server.url());

        let args = Delete {
            label: Some("345".to_string()), // matches name in Label.json fixture
            force: true,
        };

        let result = delete(&config, &args, false)
            .await
            .expect("force delete should succeed");

        assert!(result.contains("deleted"));
        labels_mock.assert_async().await;
        delete_mock.assert_async().await;
    }
}
```

## 4. `src/commands/mod.rs` — dispatch wiring

### 4.1 Add module and import

Add `mod label_commands;` to the module declarations (after `mod list_commands;` at line 20):

```rust
mod label_commands;
```

Add `use label_commands::LabelCommands;` to the imports (after `use list_commands::ListCommands;`):

```rust
use label_commands::LabelCommands;
```

### 4.2 Add `fetch_label()` helper

After the existing `fetch_string()` / `fetch_project()` functions, add a new helper. Labels differ from projects because they don't live in config — they're fetched from the API. The helper resolves by ID first, then name:

```rust
pub fn fetch_label<'a>(
    arg: Option<&str>,
    config: &Config,
    labels: &'a [labels::Label],
) -> Result<&'a labels::Label, Error> {
    if let Some(input) = arg {
        // Try exact ID match first, then name match
        labels
            .iter()
            .find(|l| l.id == input || l.name == input)
            .ok_or_else(|| {
                Error::new(
                    "fetch_label",
                    &format!("Label \"{input}\" not found"),
                )
            })
    } else if config.args.json {
        Err(Error::new("json_mode", JSON_INTERACTIVE_ERROR))
    } else {
        let label_names: Vec<String> = labels.iter().map(|l| l.name.clone()).collect();
        let selected = input::select(input::LABEL, label_names, config.mock_select)?;
        Ok(labels.iter().find(|l| l.name == selected).unwrap())
    }
}
```

Note: This requires `input::LABEL` to exist. Check `src/input.rs` for the prompt constants — if it doesn't exist, add `pub const LABEL: &str = "Label:";` alongside the existing `SECTION`, `NAME`, etc. constants.

### 4.3 Add `Label` variant to `Commands` enum

After the `Section(SectionCommands)` variant (after line 90), add:

```rust
    #[command(subcommand)]
    #[clap(alias = "b")]
    /// (b) Commands for managing personal labels
    Label(LabelCommands),
```

Aliases: `p`, `n`, `t`, `l`, `r`, `c`, `a`, `s`, `e` are taken. Use `b` for la**b**el, or `L` if uppercase aliases work (check clap behavior).

### 4.4 Add match arm in `select_command()`

After `Commands::Section(command) => ...` (after line 143):

```rust
        Commands::Label(command) => label_command(command, &cli, &tx).await,
```

### 4.5 Add handler function

After `section_command()` (around line 220), add:

```rust
async fn label_command(
    command: &LabelCommands,
    cli: &Cli,
    tx: &UnboundedSender<Error>,
) -> Result<CommandResult, Error> {
    match command {
        LabelCommands::Create(args) => {
            let config = fetch_config(cli, tx).await?;
            let result = label_commands::create(&config, args, cli.json).await;
            Ok(build_command_result(result, &config))
        }
        LabelCommands::Update(args) => {
            let config = fetch_config(cli, tx).await?;
            let result = label_commands::update(&config, args, cli.json).await;
            Ok(build_command_result(result, &config))
        }
        LabelCommands::Delete(args) => {
            let config = fetch_config(cli, tx).await?;
            let result = label_commands::delete(&config, args, cli.json).await;
            Ok(build_command_result(result, &config))
        }
    }
}
```

## 5. `src/input.rs` — add prompt constant

If `LABEL` doesn't already exist in `src/input.rs`, add it alongside the other prompt constants:

```rust
pub const LABEL: &str = "Label:";
```

## 6. `src/test/responses.rs` — remove dead_code attribute

Change line 27-28 from:

```rust
    #[allow(dead_code)]
    Label,
```

to:

```rust
    Label,
```

## 7. `docs/usage.md` — add label examples

Add a new "Labels" section after the existing "Sections" examples.

## Implementation order

1. **`src/labels.rs`** — Add `from_json()`, business-logic wrappers, unit tests
2. **`src/todoist/mod.rs`** — Add `create_label()`, `update_label()`, `delete_label()`, API tests
3. **`src/input.rs`** — Add `LABEL` prompt constant (if missing)
4. **`src/commands/label_commands.rs`** — Create file with CLI arg structs, handlers, tests
5. **`src/commands/mod.rs`** — Wire dispatch: module, import, enum variant, match arm, handler, `fetch_label()` helper
6. **`src/test/responses.rs`** — Remove `#[allow(dead_code)]` on `Label` variant
7. **`docs/usage.md`** — Add label command examples
8. Run `scripts/test.sh` — verify formatting, compilation, clippy, tests, forbidden strings
