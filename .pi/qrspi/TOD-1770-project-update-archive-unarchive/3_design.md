# Design: Project update, archive, and unarchive

## Overview

Adds four new API client functions (`update_project`, `archive_project`, `unarchive_project`, `all_archived_projects`) to `src/todoist/mod.rs`, three business-logic functions to `src/projects.rs` bridging API + config, and three new CLI subcommands (`Update`, `Archive`, `Unarchive`) wired through `ProjectCommands`.

Follows the existing **API-first, config-second** mutation pattern: call the Todoist API, then update local config only on success.

---

## 1. Todoist API Layer (`src/todoist/mod.rs`)

### 1.1 `update_project`

```
POST /api/v1/projects/{project_id}
```

**Response**: full `Project` JSON (the Todoist REST API returns the updated project).

**Signature**:

```rust
pub async fn update_project(
    config: &Config,
    project_id: &str,
    name: Option<&str>,
    color: Option<&str>,
    is_favorite: Option<bool>,
    view_style: Option<&str>,
    spinner: bool,
) -> Result<Project, Error>
```

**Body construction** — dynamic, only sends provided fields:

```rust
let url = format!("{PROJECTS_URL}/{project_id}");
let mut body = json!({});
if let Some(name) = name {
    body["name"] = json!(name);
}
if let Some(color) = color {
    body["color"] = json!(color);
}
if let Some(is_favorite) = is_favorite {
    body["is_favorite"] = json!(is_favorite);
}
if let Some(view_style) = view_style {
    body["view_style"] = json!(view_style);
}

let json = request::post_todoist(config, &url, body, spinner).await?;
Project::from_json(&json)
```

**Supported writable fields**: `name`, `color`, `is_favorite`, `view_style`. These are the four most commonly used; `description`, `child_order`, and `is_collapsed` can be added later.

**Rationale**: A single function with `Option` params avoids the per-field function proliferation seen in task updates (`update_task_priority`, `update_task_content`, etc.), which exists only because those functions are called from different dispatch paths. The project update endpoint is always the same URL regardless of which fields change.

**Placement**: immediately after `create_project` (~line 674).

### 1.2 `archive_project`

```
POST /api/v1/projects/{project_id}/archive
```

**Response**: 204 No Content (empty body). Follows the `delete_project` pattern — discard the response, return `"✓"`.

```rust
pub async fn archive_project(
    config: &Config,
    project_id: &str,
    spinner: bool,
) -> Result<String, Error> {
    let url = format!("{PROJECTS_URL}/{project_id}/archive");
    let body = json!({});

    request::post_todoist(config, &url, body, spinner).await?;
    Ok("✓".into())
}
```

### 1.3 `unarchive_project`

```
POST /api/v1/projects/{project_id}/unarchive
```

**Response**: 204 No Content (empty body). Identical pattern to `archive_project`.

```rust
pub async fn unarchive_project(
    config: &Config,
    project_id: &str,
    spinner: bool,
) -> Result<String, Error> {
    let url = format!("{PROJECTS_URL}/{project_id}/unarchive");
    let body = json!({});

    request::post_todoist(config, &url, body, spinner).await?;
    Ok("✓".into())
}
```

### 1.4 `all_archived_projects` (optional — implement last)

```
GET /api/v1/projects?filter=archived
```

**Response**: a paginated list of `Project` objects (same shape as `all_projects`).

**Note**: The Todoist REST API uses `GET /projects?filter=archived`, not a separate `/projects/archived` path. Follows the exact same pagination pattern as `all_projects` (`src/todoist/mod.rs`), reusing `ProjectResponse`.

```rust
pub async fn all_archived_projects(
    config: &Config,
    limit: Option<u8>,
) -> Result<Vec<Project>, Error> {
    let limit = limit.unwrap_or(QUERY_LIMIT);
    let mut url = format!("{PROJECTS_URL}?filter=archived&limit={limit}");
    // ... identical pagination loop to all_projects ...
}
```

**Decision**: Defer implementation until someone requests it. The task says "consider exposing" — it's not required for the core update/archive/unarchive feature.

---

## 2. Business Logic Layer (`src/projects.rs`)

### 2.1 `projects::update`

Bridges `todoist::update_project` → config update. The API returns the full updated project, so we replace the old project in config with the returned one (single source of truth from API).

```rust
pub async fn update(
    config: &mut Config,
    project: &Project,
    name: Option<&str>,
    color: Option<&str>,
    is_favorite: Option<bool>,
    view_style: Option<&str>,
    json: bool,
) -> Result<String, Error> {
    let updated = todoist::update_project(
        config,
        &project.id,
        name,
        color,
        is_favorite,
        view_style,
        true,
    )
    .await?;

    // Replace old project with API-returned project in config
    config.remove_project(project);
    config.add_project(updated.clone());

    if json {
        Ok(serde_json::to_string(&updated)?)
    } else {
        Ok(format!("Updated project {}", updated.name))
    }
}
```

**Note on double-save**: `remove_project` → `save` then `add_project` → `save` is the same pattern used by `rename` (lines 173–188). The brief intermediate state is acceptable for now; a single-save refactor is out of scope for this feature.

**Placement**: after `delete` (~line 170) and before `rename` (~line 173).

### 2.2 `projects::archive`

Calls `todoist::archive_project`, then updates the local config's `is_archived` field. Since the API returns 204 (no body), we construct the updated project locally.

```rust
pub async fn archive(
    config: &mut Config,
    project: &Project,
    json: bool,
) -> Result<String, Error> {
    todoist::archive_project(config, &project.id, true).await?;

    // Update local config with is_archived = true
    let mut archived = project.clone();
    archived.is_archived = true;
    config.remove_project(project);
    config.add_project(archived);

    if json {
        Ok(serde_json::to_string(&project)?)
    } else {
        Ok(format!("Archived project {}", project.name))
    }
}
```

### 2.3 `projects::unarchive`

Mirror of `archive` — toggles `is_archived` to `false`.

```rust
pub async fn unarchive(
    config: &mut Config,
    project: &Project,
    json: bool,
) -> Result<String, Error> {
    todoist::unarchive_project(config, &project.id, true).await?;

    let mut unarchived = project.clone();
    unarchived.is_archived = false;
    config.remove_project(project);
    config.add_project(unarchived);

    if json {
        Ok(serde_json::to_string(&project)?)
    } else {
        Ok(format!("Unarchived project {}", project.name))
    }
}
```

---

## 3. CLI Layer

### 3.1 Struct definitions (`src/commands/project_commands.rs`)

```rust
#[derive(Parser, Debug, Clone)]
pub struct Update {
    #[arg(short, long)]
    /// Project to update
    project: Option<String>,

    #[arg(short, long)]
    /// New project name
    name: Option<String>,

    #[arg(short, long)]
    /// Project color (e.g. "blue", "red", "charcoal", "berry_red")
    color: Option<String>,

    #[arg(short = 'f', long)]
    /// Toggle favorite status
    is_favorite: Option<bool>,

    #[arg(short = 'v', long)]
    /// View style: "list" or "board"
    view_style: Option<String>,
}

#[derive(Parser, Debug, Clone)]
pub struct Archive {
    #[arg(short, long)]
    /// Project to archive
    project: Option<String>,
}

#[derive(Parser, Debug, Clone)]
pub struct Unarchive {
    #[arg(short, long)]
    /// Project to unarchive
    project: Option<String>,
}
```

**Alias assignments**: `u` for Update, `a` for Archive (already taken by `--auto` on Import? No, aliases are on the enum variant, not on arg flags. `a` is free as a subcommand alias.)

Wait — `a` is not used by any existing `ProjectCommands` variant. But `archive` already starts with `a` and conventional commit-style aliases use the first letter. Let me check existing aliases: `c` (Create), `l` (List), `r` (Remove), `d` (Delete), `n` (Rename), `i` (Import), `e` (Empty). So `a` and `u` are free.

### 3.2 Enum variants

```rust
#[clap(alias = "u")]
/// (u) Update a project in Todoist
Update(Update),

#[clap(alias = "a")]
/// (a) Archive a project
Archive(Archive),

#[clap(subcommand)]
/// Unarchive a project (no short alias to avoid confusion)
Unarchive(Unarchive),
```

**Unarchive alias decision**: No single-letter alias for Unarchive. Common choices (`u` is taken by Update, `n` by Rename). Users can type the full word. This is consistent with the codebase convention — not every subcommand has a single-letter alias (e.g., `shell` commands use full names).

### 3.3 Match arms (`src/commands/mod.rs` — `project_command()`)

Add three arms following the existing pattern. `Update` passes `cli.json` for JSON output support; `Archive` and `Unarchive` do not (they return simple strings).

```rust
ProjectCommands::Update(args) => {
    let mut config = fetch_config(cli, tx).await?;
    let result = project_commands::update(&mut config, args, cli.json).await;
    Ok(build_command_result(result, &config))
}
ProjectCommands::Archive(args) => {
    let mut config = fetch_config(cli, tx).await?;
    let result = project_commands::archive(&mut config, args).await;
    Ok(build_command_result(result, &config))
}
ProjectCommands::Unarchive(args) => {
    let mut config = fetch_config(cli, tx).await?;
    let result = project_commands::unarchive(&mut config, args).await;
    Ok(build_command_result(result, &config))
}
```

**Placement**: after `ProjectCommands::Delete` arm, before the closing `}` of the match block.

### 3.4 Handler functions (`src/commands/project_commands.rs`)

```rust
/// Updates a project in Todoist and syncs changes to config.
pub async fn update(config: &mut Config, args: &Update, json: bool) -> Result<String, Error> {
    let Update {
        project,
        name,
        color,
        is_favorite,
        view_style,
    } = args;

    let project = match super::fetch_project(project.as_deref(), config).await? {
        Flag::Project(project) => project,
        Flag::Filter(_) => unreachable!(),
    };

    projects::update(
        config,
        &project,
        name.as_deref(),
        color.as_deref(),
        *is_favorite,
        view_style.as_deref(),
        json,
    )
    .await
}

/// Archives a project in Todoist and updates config.
pub async fn archive(config: &mut Config, args: &Archive) -> Result<String, Error> {
    let Archive { project } = args;

    if config.args.json {
        return Err(Error::new("json_mode", super::JSON_INTERACTIVE_ERROR));
    }

    let project = match super::fetch_project(project.as_deref(), config).await? {
        Flag::Project(project) => project,
        Flag::Filter(_) => unreachable!(),
    };

    projects::archive(config, &project, false).await
}

/// Unarchives a project in Todoist and updates config.
pub async fn unarchive(config: &mut Config, args: &Unarchive) -> Result<String, Error> {
    let Unarchive { project } = args;

    if config.args.json {
        return Err(Error::new("json_mode", super::JSON_INTERACTIVE_ERROR));
    }

    let project = match super::fetch_project(project.as_deref(), config).await? {
        Flag::Project(project) => project,
        Flag::Filter(_) => unreachable!(),
    };

    projects::unarchive(config, &project, false).await
}
```

**Note on `json_mode` guard**: `Archive` and `Unarchive` guard against JSON mode because `fetch_project` may trigger an interactive prompt. `Update` does not need this guard because it can accept the project via `--project` flag without a prompt (the interactive fallback only triggers when `project` is `None` — and even then, the `update` function in `projects.rs` handles JSON mode for output, not input).

Actually, let me reconsider. Looking at `delete` — it has `json_mode` checks inside its handler (for the confirmation prompt), not at the top. And `remove` has no `json_mode` guard at all. Looking at `empty` — it does guard against JSON mode at the top.

For `archive`/`unarchive`: there's no interactive confirmation prompt (unlike `delete` which asks about non-empty projects). The only interactive call is `fetch_project` when no `--project` flag is provided. So the `json_mode` guard makes sense — if someone runs `tod project archive` in JSON mode without `--project`, there's no way to provide the project non-interactively.

For `update`: same consideration. If `--project` is not provided and we're in JSON mode, `fetch_project` would try to prompt. We should guard against this. But `update` already has the `json` parameter for output formatting. Following the pattern of `import` (which has JSON output but guards against JSON mode when auto is false), let's add the guard:

```rust
// In update handler:
if json && project.is_none() {
    return Err(Error::new("json_mode", super::JSON_INTERACTIVE_ERROR));
}
```

Hmm, but `create` passes `cli.json` to its handler and has no guard. Let me re-check... `create` calls `super::fetch_string(name.as_deref(), config, input::NAME)` which does its own `json_mode` check. So the guard is in `fetch_*`, not in the command handler.

Looking at `fetch_project` in `src/commands/mod.rs` — does it have a `json_mode` guard? Let me check...

Actually, I don't have that code. But based on the project instructions: "Guards for JSON/non-interactive modes belong at the `fetch_*` call sites in `src/commands/mod.rs`, not inside `input.rs`." And looking at `empty` which does have a guard: it's checking `config.args.json` at the top.

So the pattern is: commands that might trigger interactive prompts should check `config.args.json` at the top and bail. Commands that guarantee non-interactive paths (like `create` with `--name` flag) don't need it.

For `archive` and `unarchive`: the only possible interactive trigger is `fetch_project` when no `--project` flag. So the guard is appropriate.

For `update`: similar — `fetch_project` might prompt. But since `update` also supports JSON output, we need to be more nuanced. If `--project` is provided and JSON is requested, it's fine. If `--project` is not provided and JSON is requested, we need to bail.

Let me handle this inside the `update` handler:

```rust
pub async fn update(config: &mut Config, args: &Update, json: bool) -> Result<String, Error> {
    let Update { project, name, color, is_favorite, view_style } = args;

    if json && project.is_none() {
        return Err(Error::new("json_mode", super::JSON_INTERACTIVE_ERROR));
    }

    let project = match super::fetch_project(project.as_deref(), config).await? {
        Flag::Project(project) => project,
        Flag::Filter(_) => unreachable!(),
    };

    projects::update(config, &project, name.as_deref(), color.as_deref(), *is_favorite, view_style.as_deref(), json).await
}
```

This follows the same pattern as `import` which also takes `json: bool` and guards before the interactive path.

**Placement** of handler functions: after `empty` (~line 230), before the `#[cfg(test)] mod tests` block.

---

## 4. Test Plan

### 4.1 API layer tests (`src/todoist/mod.rs` — `#[cfg(test)]` block)

**`test_update_project`**: Mock `POST /api/v1/projects/123` returning `Project.json` with modified fields. Call `update_project(..., Some("NewName"), None, None, None, false)`. Assert returned `Project.name == "NewName"` (or assert mock was called — the fixture returns "Doomsday", so either use a dedicated fixture or assert the mock hit).

Two options:
1. Use `Project.json` as-is and just assert `mock.assert()` + `result.is_ok()`
2. Create a new fixture `UpdatedProject.json` with a modified name

**Decision**: Option 1 is sufficient for the API layer test. We're testing the HTTP plumbing, not field-by-field mutation logic. Assert `mock.assert_async().await` and `result.is_ok()`.

**`test_archive_project`**: Mock `POST /api/v1/projects/123/archive` with status 204 and empty body. Assert `result == Ok("✓".into())` and `mock.assert_async().await`.

**`test_unarchive_project`**: Mirror of archive test — `POST /api/v1/projects/123/unarchive`, status 204.

**Fixture note for 204 tests**: Existing tests for 204-returning endpoints (e.g., `test_update_task_priority`) stub the mock with a body from a fixture file. For true 204 tests, `.with_body("")` is sufficient. `handle_response` calls `response.text().await?` which returns `""` for empty bodies — the response body is discarded by archive/unarchive callers anyway.

### 4.2 Business logic tests (`src/projects.rs`)

**`test_update_project`** (integration test):
- Mock `POST /api/v1/projects/123` returning `Project.json`
- Create config with fixture project (id "123", name "Doomsday")
- Call `projects::update(config, &project, Some("NewName"), None, None, None, false)`
- Assert result contains "Updated project"
- Reload config, assert project name is "Doomsday" (matches fixture — API returns original fixture data)
- Mock assert

**`test_update_project_json`**:
- Same mock setup
- Call with `json: true`
- Assert result is valid JSON, deserializes to `Project`

**`test_archive_project`**:
- Mock `POST /api/v1/projects/123/archive` with status 204
- Call `projects::archive(config, &project, false)`
- Assert `project.is_archived == true` in config after operation
- Assert result `"Archived project Doomsday"`

**`test_unarchive_project`**:
- Same pattern, assert `is_archived == false`
- Start with `is_archived: true` on the project in config

**`test_unarchive_project_json`**:
- Mock setup, call with `json: true`
- Assert result is a JSON string of the project

### 4.3 CLI command tests (`src/commands/project_commands.rs`)

**`test_update_name_flag`**:
- Mock `POST /api/v1/projects/123` returning `Project.json`
- Create args: `Update { project: Some("myproject".into()), name: Some("New".into()), color: None, is_favorite: None, view_style: None }`
- Call `update(&mut config, &args, false)`
- Assert result is `Ok(...)`, mock hit

**`test_archive_basic`**:
- Mock `POST /api/v1/projects/123/archive` status 204
- Args: `Archive { project: Some("myproject".into()) }`
- Assert `Ok(...)`, mock hit

**`test_unarchive_basic`**: Mirror.

**`test_update_json_mode_without_project`**:
- `config.args.json = true`
- Args: `Update { project: None, name: Some("N".into()), color: None, is_favorite: None, view__style: None }`
- Assert `Err` with source `"json_mode"`

**CLI parse tests** (no HTTP needed):
```rust
#[test]
fn update_flags_parse() {
    let args = Update::try_parse_from(["tod", "-p", "myproject", "-n", "new-name", "-c", "red", "-f", "true"])
        .expect("update args should parse");
    assert_eq!(args.project.as_deref(), Some("myproject"));
    assert_eq!(args.name.as_deref(), Some("new-name"));
    assert_eq!(args.color.as_deref(), Some("red"));
    assert_eq!(args.is_favorite, Some(true));
}

#[test]
fn archive_flag_parses() {
    let args = Archive::try_parse_from(["tod", "-p", "myproject"])
        .expect("archive args should parse");
    assert_eq!(args.project.as_deref(), Some("myproject"));
}
```

---

## 5. Files Changed

| File | Change |
|---|---|
| `src/todoist/mod.rs` | Add `update_project`, `archive_project`, `unarchive_project` functions |
| `src/projects.rs` | Add `update`, `archive`, `unarchive` functions |
| `src/commands/project_commands.rs` | Add `Update`, `Archive`, `Unarchive` structs; variants to enum; handler functions; tests |
| `src/commands/mod.rs` | Add three match arms to `project_command()` |
| `docs/usage.md` | Add examples for `update`, `archive`, `unarchive` subcommands |

No new files. No new dependencies. No fixture changes (existing `Project.json` is sufficient).

---

## 6. Decisions & Tradeoffs

### 6.1 Update field set

Only `name`, `color`, `is_favorite`, `view_style` are exposed. `description`, `child_order`, and `is_collapsed` are supported by the API but deferred. Adding them later is a non-breaking change (just add `Option` params to `update_project` and new CLI flags).

### 6.2 Archive/unarchive config update pattern

Uses the `remove_project` + `add_project` double-save pattern (same as `rename`). A single-save approach would require adding a `config.update_project` method — out of scope.

### 6.3 Unarchive alias

No single-letter alias. `u` is taken by Update. The full word is clear and unambiguous. Users typing `tod project unarchive` with tab completion won't feel the lack of an alias.

### 6.4 `all_archived_projects`

Deferred. The task marks it as "consider" rather than required. The pagination pattern is identical to `all_projects` and can be copied later.

### 6.5 JSON mode for archive/unarchive

`Archive` and `Unarchive` don't accept `json: bool` in their handler signatures. Their `projects::*` functions do accept it for future use (the handlers hardcode `false`). If JSON output for these commands is needed later, it's a one-line change.

---

## 7. Implementation Order

1. **`src/todoist/mod.rs`**: Add `update_project`, `archive_project`, `unarchive_project` + unit tests
2. **`src/projects.rs`**: Add `update`, `archive`, `unarchive` + integration tests
3. **`src/commands/project_commands.rs`**: Add structs, enum variants, handlers
4. **`src/commands/mod.rs`**: Add match arms in `project_command()`
5. **`src/commands/project_commands.rs`**: Add CLI-level tests
6. **`docs/usage.md`**: Add usage examples
7. Run `scripts/test.sh` and `cargo clippy` to verify
