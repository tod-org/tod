# Implementation Plan

## Overview

Add `update_project`, `archive_project`, `unarchive_project` to the Todoist API client, bridge functions to `src/projects.rs`, and three CLI subcommands (`Update`, `Archive`, `Unarchive`) under `ProjectCommands`. 5 files changed, no new files, no new dependencies.

---

## Phase 1: Todoist API Layer (`src/todoist/mod.rs`)

### 1.1 `update_project`

**File**: `src/todoist/mod.rs`
**Action**: add function after `create_project` (~line 674)

```rust
/// Updates a project's writable fields. Only provided fields are sent in the request body.
pub async fn update_project(
    config: &Config,
    project_id: &str,
    name: Option<&str>,
    color: Option<&str>,
    is_favorite: Option<bool>,
    view_style: Option<&str>,
    spinner: bool,
) -> Result<Project, Error> {
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
}
```

### 1.2 `archive_project`

**File**: `src/todoist/mod.rs`
**Action**: add function after `update_project`

```rust
/// Archives a project by ID. Returns 204 No Content from the API.
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

**File**: `src/todoist/mod.rs`
**Action**: add function after `archive_project`

```rust
/// Unarchives a project by ID. Returns 204 No Content from the API.
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

### 1.4 API layer tests

**File**: `src/todoist/mod.rs` `#[cfg(test)]` block
**Action**: add tests after existing project tests

```rust
#[tokio::test]
async fn test_update_project() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/api/v1/projects/123")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(ResponseFromFile::Project.read().await)
        .create_async()
        .await;

    let config = test::fixtures::config()
        .await
        .with_mock_url(server.url());

    let result = update_project(&config, "123", Some("NewName"), None, None, None, false).await;
    assert!(result.is_ok());
    mock.assert_async().await;
}

#[tokio::test]
async fn test_archive_project() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/api/v1/projects/123/archive")
        .with_status(204)
        .create_async()
        .await;

    let config = test::fixtures::config()
        .await
        .with_mock_url(server.url());

    let result = archive_project(&config, "123", false).await;
    assert_eq!(result, Ok("✓".to_string()));
    mock.assert_async().await;
}

#[tokio::test]
async fn test_unarchive_project() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/api/v1/projects/123/unarchive")
        .with_status(204)
        .create_async()
        .await;

    let config = test::fixtures::config()
        .await
        .with_mock_url(server.url());

    let result = unarchive_project(&config, "123", false).await;
    assert_eq!(result, Ok("✓".to_string()));
    mock.assert_async().await;
}
```

---

## Phase 2: Business Logic Layer (`src/projects.rs`)

### 2.1 `projects::update`

**File**: `src/projects.rs`
**Action**: add function after `delete` (after line 170, before `rename` at line 173)

```rust
/// Updates a project in Todoist and replaces it in config with the API response.
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

    config.remove_project(project);
    config.add_project(updated.clone());
    config.save().await?;

    if json {
        Ok(serde_json::to_string(&updated)?)
    } else {
        Ok(format!("Updated project {}", updated.name))
    }
}
```

### 2.2 `projects::archive`

**File**: `src/projects.rs`
**Action**: add function after `update`

```rust
/// Archives a project in Todoist and marks it as archived in config.
pub async fn archive(
    config: &mut Config,
    project: &Project,
    json: bool,
) -> Result<String, Error> {
    todoist::archive_project(config, &project.id, true).await?;

    let mut archived = project.clone();
    archived.is_archived = true;
    config.remove_project(project);
    config.add_project(archived);
    config.save().await?;

    if json {
        Ok(serde_json::to_string(&project)?)
    } else {
        Ok(format!("Archived project {}", project.name))
    }
}
```

### 2.3 `projects::unarchive`

**File**: `src/projects.rs`
**Action**: add function after `archive`

```rust
/// Unarchives a project in Todoist and marks it as unarchived in config.
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
    config.save().await?;

    if json {
        Ok(serde_json::to_string(&project)?)
    } else {
        Ok(format!("Unarchived project {}", project.name))
    }
}
```

### 2.4 Business logic tests

**File**: `src/projects.rs` `#[cfg(test)]` block
**Action**: add tests after existing project tests (near `test_project_delete` at line 1179)

```rust
#[tokio::test]
async fn test_update_project() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/api/v1/projects/123")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(ResponseFromFile::Project.read().await)
        .create_async()
        .await;

    let mut config = test::fixtures::config()
        .await
        .with_mock_url(server.url())
        .create()
        .await
        .expect("config should be created");

    let project = config
        .projects()
        .await
        .expect("projects should load")
        .into_iter()
        .find(|p| p.name == "myproject")
        .expect("fixture project should exist");

    let result = update(&mut config, &project, Some("Renamed"), None, None, None, false).await;
    assert!(result.is_ok());
    mock.assert_async().await;
}

#[tokio::test]
async fn test_update_project_json() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/api/v1/projects/123")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(ResponseFromFile::Project.read().await)
        .create_async()
        .await;

    let mut config = test::fixtures::config()
        .await
        .with_mock_url(server.url())
        .create()
        .await
        .expect("config should be created");

    let project = config
        .projects()
        .await
        .expect("projects should load")
        .into_iter()
        .find(|p| p.name == "myproject")
        .expect("fixture project should exist");

    let result = update(&mut config, &project, Some("Renamed"), None, None, None, true).await;
    assert!(result.is_ok());
    // JSON output should deserialize back to a Project
    let json = result.unwrap();
    let parsed: Project = serde_json::from_str(&json).expect("should be valid project JSON");
    assert_eq!(parsed.name, "Doomsday"); // fixture returns "Doomsday"
    mock.assert_async().await;
}

#[tokio::test]
async fn test_archive_project() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/api/v1/projects/123/archive")
        .with_status(204)
        .create_async()
        .await;

    let mut config = test::fixtures::config()
        .await
        .with_mock_url(server.url())
        .create()
        .await
        .expect("config should be created");

    let project = config
        .projects()
        .await
        .expect("projects should load")
        .into_iter()
        .find(|p| p.name == "myproject")
        .expect("fixture project should exist");

    let result = archive(&mut config, &project, false).await;
    assert!(result.is_ok());
    assert!(result.unwrap().contains("Archived project"));

    // Verify config was updated
    let projects = config.projects().await.expect("projects should load");
    let archived = projects.iter().find(|p| p.id == "123").expect("project should still exist");
    assert!(archived.is_archived);
    mock.assert_async().await;
}

#[tokio::test]
async fn test_unarchive_project() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/api/v1/projects/123/unarchive")
        .with_status(204)
        .create_async()
        .await;

    let mut config = test::fixtures::config()
        .await
        .with_mock_url(server.url())
        .create()
        .await
        .expect("config should be created");

    // Start with project marked as archived in config
    let project = {
        let projects = config.projects().await.expect("projects should load");
        let mut p = projects
            .into_iter()
            .find(|p| p.name == "myproject")
            .expect("fixture project should exist");
        p.is_archived = true;
        p
    };
    // Replace in config: remove original, add modified
    let original = config
        .projects()
        .await
        .expect("projects should load")
        .into_iter()
        .find(|p| p.id == "123")
        .expect("fixture project should exist");
    config.remove_project(&original);
    config.add_project(project.clone());
    config.save().await.expect("save should succeed");

    let result = unarchive(&mut config, &project, false).await;
    assert!(result.is_ok());
    assert!(result.unwrap().contains("Unarchived project"));

    let projects = config.projects().await.expect("projects should load");
    let unarchived = projects.iter().find(|p| p.id == "123").expect("project should still exist");
    assert!(!unarchived.is_archived);
    mock.assert_async().await;
}
```

---

## Phase 3: CLI Structs & Enum (`src/commands/project_commands.rs`)

### 3.1 Add struct definitions

**File**: `src/commands/project_commands.rs`
**Action**: add after `Empty` struct (~line 124)

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

### 3.2 Add enum variants

**File**: `src/commands/project_commands.rs`
**Action**: add to `ProjectCommands` enum (after `Empty` variant ~line 40)

```rust
    #[clap(alias = "u")]
    /// (u) Update a project in Todoist
    Update(Update),

    #[clap(alias = "a")]
    /// (a) Archive a project
    Archive(Archive),

    #[clap(subcommand)]
    /// Unarchive a project
    Unarchive(Unarchive),
```

### 3.3 Add handler functions

**File**: `src/commands/project_commands.rs`
**Action**: add after `empty` handler (~line 230, before `#[cfg(test)]`)

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

    if json && project.is_none() {
        return Err(Error::new("json_mode", super::JSON_INTERACTIVE_ERROR));
    }

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

/// Archives a project in Todoist and marks it archived in config.
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

/// Unarchives a project in Todoist and marks it unarchived in config.
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

---

## Phase 4: CLI Dispatch (`src/commands/mod.rs`)

### 4.1 Add match arms

**File**: `src/commands/mod.rs`
**Action**: add three arms to `project_command()` after `ProjectCommands::Delete` match arm

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

---

## Phase 5: CLI Tests (`src/commands/project_commands.rs`)

### 5.1 Add CLI parse + integration tests

**File**: `src/commands/project_commands.rs` `#[cfg(test)] mod tests`
**Action**: add after existing tests

```rust
    #[tokio::test]
    async fn update_name_flag_hits_api() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/api/v1/projects/123")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::Project.read().await)
            .create_async()
            .await;

        let mut config = test::fixtures::config()
            .await
            .with_mock_url(server.url())
            .create()
            .await
            .expect("config should be created");

        let args = Update {
            project: Some("myproject".into()),
            name: Some("NewName".into()),
            color: None,
            is_favorite: None,
            view_style: None,
        };

        let result = update(&mut config, &args, false).await;
        assert!(result.is_ok());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn archive_hits_api() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/api/v1/projects/123/archive")
            .with_status(204)
            .create_async()
            .await;

        let mut config = test::fixtures::config()
            .await
            .with_mock_url(server.url())
            .create()
            .await
            .expect("config should be created");

        let args = Archive {
            project: Some("myproject".into()),
        };

        let result = archive(&mut config, &args).await;
        assert!(result.is_ok());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn unarchive_hits_api() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/api/v1/projects/123/unarchive")
            .with_status(204)
            .create_async()
            .await;

        let mut config = test::fixtures::config()
            .await
            .with_mock_url(server.url())
            .create()
            .await
            .expect("config should be created");

        let args = Unarchive {
            project: Some("myproject".into()),
        };

        let result = unarchive(&mut config, &args).await;
        assert!(result.is_ok());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn update_json_mode_without_project_fails() {
        let mut config = Config::default_test();
        config.args.json = true;

        let args = Update {
            project: None,
            name: Some("N".into()),
            color: None,
            is_favorite: None,
            view_style: None,
        };

        let error = update(&mut config, &args, true)
            .await
            .expect_err("should fail without project in json mode");
        assert_eq!(error.source, "json_mode");
    }

    #[test]
    fn update_flags_parse() {
        let args = Update::try_parse_from([
            "tod", "-p", "myproject", "-n", "new-name", "-c", "red", "-f", "true",
        ])
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

    #[test]
    fn unarchive_flag_parses() {
        let args = Unarchive::try_parse_from(["tod", "-p", "myproject"])
            .expect("unarchive args should parse");
        assert_eq!(args.project.as_deref(), Some("myproject"));
    }
```

---

## Phase 6: Documentation (`docs/usage.md`)

### 6.1 Add usage examples

**File**: `docs/usage.md`
**Action**: add entries in the project commands section

```markdown
### Update a project
```bash
tod project update -p "My Project" -n "Renamed Project" -c "red"
tod project update --project "Work" --is-favorite true
tod project update -p "Side Project" -v "board"
```

### Archive / unarchive a project
```bash
tod project archive -p "Old Project"
tod project unarchive -p "Old Project"
```
```

---

## Execution Order

| Step | Phase | File | Action |
|---|---|---|---|
| 1 | Phase 1 | `src/todoist/mod.rs` | Add `update_project`, `archive_project`, `unarchive_project` |
| 2 | Phase 1 | `src/todoist/mod.rs` | Add API layer tests |
| 3 | Phase 2 | `src/projects.rs` | Add `update`, `archive`, `unarchive` |
| 4 | Phase 2 | `src/projects.rs` | Add business logic tests |
| 5 | Phase 3 | `src/commands/project_commands.rs` | Add structs, enum variants, handlers |
| 6 | Phase 4 | `src/commands/mod.rs` | Add match arms in `project_command()` |
| 7 | Phase 5 | `src/commands/project_commands.rs` | Add CLI tests |
| 8 | Phase 6 | `docs/usage.md` | Add usage examples |
| 9 | — | — | Run `scripts/test.sh`, fix any issues |

---

## Verification Checklist

- [x] `cargo build` compiles without errors
- [x] `cargo clippy` passes with no warnings
- [x] `cargo test` — all new + existing tests pass
- [x] `scripts/test.sh` — no `dbg!`, `TODO`, `FIXME`, `DEBUG:`, or `FIXTURE:` strings
- [x] `tod project update --help` shows all flags
- [x] `tod project archive --help` shows `-p` flag
- [x] `tod project unarchive --help` shows `-p` flag
- [x] `tod project u` works as alias for update
- [x] `tod project a` works as alias for archive
