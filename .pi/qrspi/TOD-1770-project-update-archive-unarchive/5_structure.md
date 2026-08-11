# Structure Outline

## Approach

Add three API client functions (`update_project`, `archive_project`, `unarchive_project`) to `src/todoist/mod.rs`, three business-logic bridges to `src/projects.rs`, and three CLI subcommands (`Update`, `Archive`, `Unarchive`) under `ProjectCommands`. Follow the existing API-first, config-second mutation pattern. Every phase is independently testable — after each, `cargo test` passes for the layer being built.

---

## Phase 1: Todoist API Layer

Add `update_project`, `archive_project`, `unarchive_project` to `src/todoist/mod.rs` with inline tests. This phase stands alone — the new functions compile and their tests pass without touching any other file.

**File**: `src/todoist/mod.rs`

### 1.1 `update_project` — insert after `create_project` (after line 674)

Current end of `create_project`:
```rust
    let json = request::post_todoist(config, &url, body, spinner).await?;
    Project::from_json(&json)
}
```

Insert after the closing `}`:
```rust
/// Updates a project's writable fields. Only provided `Some(...)` fields are sent
/// in the request body; `None` fields are omitted.
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

### 1.2 `archive_project` — insert after `update_project`

```rust
/// Archives a project by ID. The Todoist API returns 204 No Content.
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

### 1.3 `unarchive_project` — insert after `archive_project`

```rust
/// Unarchives a project by ID. The Todoist API returns 204 No Content.
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

### 1.4 Tests — append to `#[cfg(test)] mod tests` block

Find the existing test block. Add after the last project-related test (likely near `test_create_project` or `test_delete_project`):

```rust
    #[tokio::test]
    async fn test_update_project_hits_api() {
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

        let result = update_project(
            &config, "123", Some("NewName"), None, None, None, false,
        )
        .await;

        assert!(result.is_ok());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_archive_project_hits_api() {
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
    async fn test_unarchive_project_hits_api() {
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

**Verify**: `cargo test --lib todoist::test_update_project todoist::test_archive_project todoist::test_unarchive_project` passes; `cargo clippy` passes on `src/todoist/mod.rs`; `scripts/test.sh` passes.

---

## Phase 2: Business Logic Layer

Add `update`, `archive`, `unarchive` to `src/projects.rs` with integration tests. These bridge the API calls from Phase 1 with local config mutations.

**File**: `src/projects.rs`

### 2.1 `projects::update` — insert after `delete` (after line 170), before `rename` (line 173)

Current boundary:
```rust
/// Delete a project from Todoist and remove it from config.
pub async fn delete(config: &mut Config, project: &Project) -> Result<String, Error> {
    todoist::delete_project(config, project, true).await?;
    config.remove_project(project);
    config.save().await
}

/// Rename a project locally in config (does not sync to Todoist).
pub async fn rename(
```

Insert `update` between `delete` and `rename`:
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

### 2.2 `projects::archive` — insert after `update`

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

### 2.3 `projects::unarchive` — insert after `archive`

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

### 2.4 Tests — append to `#[cfg(test)] mod tests` block

Find the existing `test_project_delete` test (line 1179). Add after it:

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

        let json = update(&mut config, &project, Some("Renamed"), None, None, None, true)
            .await
            .expect("update should succeed");

        let parsed: Project =
            serde_json::from_str(&json).expect("should be valid project JSON");
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

        let projects = config.projects().await.expect("projects should load");
        let archived = projects
            .iter()
            .find(|p| p.id == "123")
            .expect("project should still exist");
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
        let unarchived = projects
            .iter()
            .find(|p| p.id == "123")
            .expect("project should still exist");
        assert!(!unarchived.is_archived);
        mock.assert_async().await;
    }
```

**Verify**: `cargo test --lib projects::test_update_project projects::test_update_project_json projects::test_archive_project projects::test_unarchive_project` passes; `cargo clippy` passes on `src/projects.rs`; `scripts/test.sh` passes.

---

## Phase 3: CLI Structs, Enum, and Handlers

Add three structs, three enum variants, and three handler functions to `src/commands/project_commands.rs`. Also add three match arms to `src/commands/mod.rs` in the `project_command()` function.

**Files**: `src/commands/project_commands.rs`, `src/commands/mod.rs`

### 3.1 Struct definitions — insert after `Empty` struct (after line 124)

Current end of `Empty`:
```rust
#[derive(Parser, Debug, Clone)]
pub struct Empty {
    #[arg(short, long)]
    /// Project to empty
    project: Option<String>,
}
```

Insert after the closing `}`:
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

### 3.2 Enum variants — insert into `ProjectCommands` after `Empty` variant (after line 40)

Current end of enum:
```rust
    #[clap(alias = "e")]
    /// (e) Empty a project by putting tasks in other projects
    Empty(Empty),
}
```

Change to:
```rust
    #[clap(alias = "e")]
    /// (e) Empty a project by putting tasks in other projects
    Empty(Empty),

    #[clap(alias = "u")]
    /// (u) Update a project in Todoist
    Update(Update),

    #[clap(alias = "a")]
    /// (a) Archive a project
    Archive(Archive),

    #[clap(subcommand)]
    /// Unarchive a project
    Unarchive(Unarchive),
}
```

### 3.3 Handler functions — insert after `empty` handler (after line 230), before `#[cfg(test)]`

Current end of `empty`:
```rust
    projects::empty(config, &project).await
}

#[cfg(test)]
```

Insert three handlers between the closing `}` of `empty` and `#[cfg(test)]`:
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

### 3.4 Dispatch match arms — `src/commands/mod.rs`

In the `project_command()` function, find the `ProjectCommands::Delete` arm (around line 230 in `mod.rs`). Insert three new arms before the closing `}` of the match block:

Current:
```rust
        ProjectCommands::Delete(args) => {
            let mut config = fetch_config(cli, tx).await?;
            let result = project_commands::delete(&mut config, args).await;
            Ok(build_command_result(result, &config))
        }
    }
}
```

Change to:
```rust
        ProjectCommands::Delete(args) => {
            let mut config = fetch_config(cli, tx).await?;
            let result = project_commands::delete(&mut config, args).await;
            Ok(build_command_result(result, &config))
        }
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
    }
}
```

**Verify**: `cargo build` compiles without errors; `cargo clippy` passes; `scripts/test.sh` passes.

---

## Phase 4: CLI Tests

Add parse and integration tests for the new subcommands.

**File**: `src/commands/project_commands.rs` `#[cfg(test)] mod tests`

### 4.1 Append after existing tests (after the last `}` in the test module)

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
        let args =
            Archive::try_parse_from(["tod", "-p", "myproject"]).expect("archive args should parse");
        assert_eq!(args.project.as_deref(), Some("myproject"));
    }

    #[test]
    fn unarchive_flag_parses() {
        let args = Unarchive::try_parse_from(["tod", "-p", "myproject"])
            .expect("unarchive args should parse");
        assert_eq!(args.project.as_deref(), Some("myproject"));
    }
```

**Verify**: `cargo test --lib project_commands::` passes all 4 new + all existing tests; `cargo clippy` passes; `scripts/test.sh` passes.

---

## Phase 5: Documentation

Add usage examples to `docs/usage.md`.

**File**: `docs/usage.md`

### 5.1 Add project command examples

Find the existing project commands section. Add entries for update, archive, and unarchive:

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

**Verify**: `scripts/test.sh` passes (no grep hits from docs changes).

---

## Testing Checkpoints

After each phase, run:
```
cargo build               # compiles without errors
cargo clippy -- -D warnings  # no warnings
scripts/test.sh           # no grep hits (dbg!, TODO, FIXME, etc.)
```

- **After Phase 1**: `cargo test --lib todoist::test_update_project todoist::test_archive_project todoist::test_unarchive_project` — 3 new tests pass, all existing tests pass
- **After Phase 2**: `cargo test --lib projects::` — 4 new tests pass alongside existing project tests
- **After Phase 3**: `cargo build` succeeds; `tod project update --help`, `tod project archive --help`, `tod project unarchive --help` all output correct help text; `tod project u` and `tod project a` aliases work
- **After Phase 4**: `cargo test --lib project_commands::` — 7 new CLI tests pass alongside existing CLI tests
- **After Phase 5**: `scripts/test.sh` passes; `docs/usage.md` examples are syntactically correct

**Final integration check**: `cargo test` — zero failures across all test suites.
