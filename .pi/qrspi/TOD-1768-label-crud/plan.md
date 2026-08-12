# Implementation Plan

## Overview

Add `tod label create`, `tod label update`, and `tod label delete` CLI commands by following the existing patterns in `project_commands` / `section_commands`. Each phase delivers a complete, testable vertical slice.

---

## Phase 1: `tod label create` — first vertical slice

Delivers `tod label create` end-to-end: API call, business-logic wrapper, CLI handler, dispatch wiring, and tests at every layer. Establishes the `label_commands` module, `LabelCommands` enum, and dispatch infrastructure.

### Changes

#### 1. Add `Label::from_json()` standalone method
**File**: `src/labels.rs`
**Action**: modify

After the `impl LabelResponse` block (after line 27), before `impl Display for Label` (line 28):

```rust
impl Label {
    pub fn from_json(json: &str) -> Result<Label, Error> {
        let label: Label = serde_json::from_str(json)?;
        Ok(label)
    }
}
```

#### 2. Add `labels::create()` business-logic wrapper
**File**: `src/labels.rs`
**Action**: modify

After the existing `get_labels()` function (after line 33):

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
```

#### 3. Add unit tests for `from_json` and `create`
**File**: `src/labels.rs`
**Action**: modify

Add to the existing `mod tests` block (after line ~108, before the `mod proptests` block):

```rust
    #[test]
    fn test_label_from_json_valid() {
        let json =
            r#"{"id":"1","name":"work","color":"red","order":1,"is_favorite":false}"#;
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

#### 4. Add `todoist::create_label()` API function
**File**: `src/todoist/mod.rs`
**Action**: modify

Place after `all_labels()` (after line ~465 of the `all_labels` function), before `move_task_to_project`:

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

#### 5. Add API test for `create_label`
**File**: `src/todoist/mod.rs`
**Action**: modify

Add to the tests module. Find the section tests (around line 1068) and add after `test_create_section`:

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
```

#### 6. Remove `#[allow(dead_code)]` from `ResponseFromFile::Label`
**File**: `src/test/responses.rs`
**Action**: modify

Change line 27 from:
```rust
    #[allow(dead_code)]
    Label,
```
to:
```rust
    Label,
```

#### 7. Create `src/commands/label_commands.rs`
**File**: `src/commands/label_commands.rs`
**Action**: create

```rust
use crate::{config::Config, errors::Error, format, input, labels};
use clap::{Parser, Subcommand};

/// Label subcommands.
#[derive(Subcommand, Debug, Clone)]
pub enum LabelCommands {
    #[clap(alias = "c")]
    /// (c) Create a new personal label
    Create(Create),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test;

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
}
```

#### 8. Register `label_commands` module and wire dispatch
**File**: `src/commands/mod.rs`
**Action**: modify

**a.** Add module declaration after `mod list_commands;` (line 18):
```rust
mod label_commands;
```

**b.** Add import after `use list_commands::ListCommands;` (line 15):
```rust
use label_commands::LabelCommands;
```

**c.** Add `Label` variant to `Commands` enum after the `Section(SectionCommands)` variant (after line 87):
```rust
    #[command(subcommand)]
    #[clap(alias = "b")]
    /// (b) Commands for managing personal labels
    Label(LabelCommands),
```

**d.** Add match arm in `select_command()` after `Commands::Section(command) => ...` (after line 139):
```rust
        Commands::Label(command) => label_command(command, &cli, &tx).await,
```

**e.** Add `label_command()` handler function after `section_command()` (after line ~218):

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
    }
}
```

Note: `labels::Label` is already available via `use crate::{..., labels};` on line 6 — no new import needed for the label type.

### Verification

#### Automated
- [x] `cargo test` passes all existing tests plus 4 new tests:
  - `labels::tests::test_label_from_json_valid`
  - `labels::tests::test_label_from_json_invalid`
  - `todoist::tests::test_create_label`
  - `label_commands::tests::create_fails_in_json_mode_without_name`
- [x] `cargo build` compiles without warnings
- [x] `#[allow(dead_code)]` is removed from `ResponseFromFile::Label` (verify with `grep dead_code src/test/responses.rs` — should not appear on the `Label` line)

#### Manual
- [x] `tod label -h` shows `Create` subcommand with alias `c`
- [x] `tod label create --name "test-label" --color red` creates a real label (visible in Todoist UI)
- [x] `tod label create --name "test-label" --json` returns JSON with `id`, `name`, `color`, `order`, `is_favorite` fields
- [x] `tod -h` shows `label` command with alias `b` in the command list

---

## Phase 2: `tod label update` and `tod label delete`

Adds the remaining two commands on top of Phase 1's infrastructure. The `LabelCommands` enum grows from one variant to three.

### Changes

#### 1. Add `todoist::update_label()` API function
**File**: `src/todoist/mod.rs`
**Action**: modify

Place after `create_label()` (added in Phase 1):

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

#### 2. Add `todoist::delete_label()` API function
**File**: `src/todoist/mod.rs`
**Action**: modify

Place after `update_label()`:

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

#### 3. Add API tests for update and delete
**File**: `src/todoist/mod.rs`
**Action**: modify

Add after `test_create_label` (added in Phase 1):

```rust
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

#### 4. Add `labels::update()` and `labels::delete()` business-logic wrappers
**File**: `src/labels.rs`
**Action**: modify

Add after `create()` (added in Phase 1):

```rust
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

#### 5. Add `LABEL` prompt constant
**File**: `src/input.rs`
**Action**: modify

Add after `pub const LABELS: &str = "Select labels";` (line 33):

```rust
/// Prompt label for single label selection.
pub const LABEL: &str = "Label:";
```

#### 6. Add `fetch_label()` helper to `commands/mod.rs`
**File**: `src/commands/mod.rs`
**Action**: modify

Add after `fetch_project_or_filter()` (after line ~460). This helper resolves a label by ID or name from a pre-fetched list:

```rust
/// Resolves a label by ID or name from a list, or prompts interactively.
pub fn fetch_label<'a>(
    arg: Option<&str>,
    config: &Config,
    labels: &'a [labels::Label],
) -> Result<&'a labels::Label, Error> {
    if let Some(input) = arg {
        labels
            .iter()
            .find(|l| l.id == input || l.name == input)
            .ok_or_else(|| Error::new("fetch_label", &format!("Label \"{input}\" not found")))
    } else if config.args.json {
        Err(Error::new("json_mode", JSON_INTERACTIVE_ERROR))
    } else {
        let label_names: Vec<String> = labels.iter().map(|l| l.name.clone()).collect();
        let selected = input::select(input::LABEL, label_names, config.mock_select)?;
        Ok(labels.iter().find(|l| l.name == selected).unwrap())
    }
}
```

Note: `labels::Label` is already in scope via `use crate::{..., labels};` on line 6.

#### 7. Add `Update` and `Delete` to `LabelCommands` + handlers + tests
**File**: `src/commands/label_commands.rs`
**Action**: modify

**a.** Add `Update` and `Delete` variants to the `LabelCommands` enum:

```rust
    #[clap(alias = "u")]
    /// (u) Update an existing personal label
    Update(Update),
    #[clap(alias = "d")]
    /// (d) Delete a personal label
    Delete(Delete),
```

**b.** Add new arg structs after `Create`:

```rust
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

**c.** Add handler functions after the `create()` handler:

```rust
/// Updates a personal label.
pub async fn update(config: &Config, args: &Update, json: bool) -> Result<String, Error> {
    let Update {
        label,
        name,
        color,
        order,
        favorite,
    } = args;

    let labels_list = labels::get_labels(config, true).await?;
    let target = super::fetch_label(label.as_deref(), config, &labels_list)?;

    if name.is_none() && color.is_none() && order.is_none() && favorite.is_none() {
        return Err(Error::new(
            "update_label",
            "At least one of --name, --color, --order, or --favorite is required",
        ));
    }

    let updated = labels::update(
        config,
        &target.id,
        name.as_deref(),
        color.as_deref(),
        *order,
        *favorite,
    )
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

**d.** Add command-level tests to the existing `mod tests` block:

```rust
    #[tokio::test]
    async fn update_fails_without_any_fields() {
        let config = Config::default_test();
        let args = Update {
            label: Some("my-label".to_string()),
            name: None,
            color: None,
            order: None,
            favorite: None,
        };

        let error = update(&config, &args, false)
            .await
            .expect_err("update with no fields should fail");

        assert_eq!(error.source, "update_label");
        assert!(error.message.contains("At least one"));
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
            label: Some("345".to_string()),
            force: true,
        };

        let result = delete(&config, &args, false)
            .await
            .expect("force delete should succeed");

        assert!(result.contains("deleted"));
        labels_mock.assert_async().await;
        delete_mock.assert_async().await;
    }
```

Note: The test module needs these additional imports added at the top of the `mod tests` block:
```rust
    use crate::test::responses::ResponseFromFile;
```
(already present from Phase 1 if `create_fails_in_json_mode_without_name` uses it — it doesn't, so add it now for the delete tests that need it.)

#### 8. Add `Update` and `Delete` arms to `label_command()` dispatch
**File**: `src/commands/mod.rs`
**Action**: modify

Expand the `label_command()` function (added in Phase 1) to handle the new variants:

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

Replace the Phase 1 version of `label_command()` (which only had the `Create` arm) with this expanded one.

### Verification

#### Automated
- [x] `cargo test` passes all Phase 1 + Phase 2 tests (8 total new tests across both phases):
  - `labels::tests::test_label_from_json_valid`
  - `labels::tests::test_label_from_json_invalid`
  - `todoist::tests::test_create_label`
  - `todoist::tests::test_update_label`
  - `todoist::tests::test_delete_label`
  - `label_commands::tests::create_fails_in_json_mode_without_name`
  - `label_commands::tests::update_fails_without_any_fields`
  - `label_commands::tests::delete_fails_when_label_not_found`
  - `label_commands::tests::delete_force_skips_confirmation`
- [x] `cargo build` compiles without warnings

#### Manual
- [x] `tod label update --label "test-label" --name "renamed"` renames a label
- [x] `tod label delete --label "renamed" --force` deletes without confirmation
- [x] `tod label delete --label "renamed"` (interactive, no `--force`) shows confirmation prompt with Cancel/Delete options
- [x] `tod label -h` shows all three subcommands with aliases: `c` (create), `u` (update), `d` (delete)

---

## Phase 3: Docs and final verification

Updates `docs/usage.md` with label command examples and runs the full pre-commit checklist.

### Changes

#### 1. Add label command examples to docs
**File**: `docs/usage.md`
**Action**: modify

Add a new "Labels" section after the existing "Sections" examples. The format should match existing sections — a `### Labels` heading with code-fenced bash examples:

```markdown
### Labels

Create a label:
```bash
tod label create --name "Urgent" --color red
tod label create -n "Personal" --color blue --favorite
tod label create -n "Weekend" -c green -o 2
```

Update a label:
```bash
tod label update --label "Urgent" --name "Critical"
tod label update -l "Personal" --color purple
```

Delete a label:
```bash
tod label delete --label "Weekend" --force
tod label delete -l "Critical"
```

View all labels:
```bash
tod label -h
```
```

Note: Read the existing `docs/usage.md` to find the exact "Sections" section location and follow its heading level and code fence conventions exactly.

### Verification

#### Automated
- [ ] `scripts/test.sh` passes clean (fmt, build, clippy, tests, forbidden strings)
- [ ] No `dbg!`, `TODO`, `FIXME`, `DEBUG:`, or `FIXTURE:` strings in any `.rs` file

#### Manual
- [ ] `tod -h` shows `label` / `b` in the command list
- [ ] `tod label -h` shows all three subcommands with correct aliases:
  - `create` / `c`
  - `update` / `u`
  - `delete` / `d`
- [ ] `docs/usage.md` renders correctly (check in editor)

---

## Summary of all files touched

| File | Phase | Action |
|------|-------|--------|
| `src/labels.rs` | 1, 2 | Add `Label::from_json()`, `create()`, `update()`, `delete()`, 2 unit tests |
| `src/todoist/mod.rs` | 1, 2 | Add `create_label()`, `update_label()`, `delete_label()`, 3 API tests |
| `src/input.rs` | 1 | Add `pub const LABEL: &str = "Label:";` |
| `src/commands/label_commands.rs` | 1, 2 | **New file**: `LabelCommands` enum, `Create`/`Update`/`Delete` arg structs, 3 handlers, 4 tests |
| `src/commands/mod.rs` | 1, 2 | Module + import, `Commands::Label` variant, `select_command` arm, `label_command()` handler, `fetch_label()` helper |
| `src/test/responses.rs` | 1 | Remove `#[allow(dead_code)]` from `Label` variant |
| `docs/usage.md` | 3 | Add "Labels" section with examples |
