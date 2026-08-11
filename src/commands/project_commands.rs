use clap::{Parser, Subcommand};

use crate::{config::Config, debug, errors::Error, input, lists::Flag, projects, todoist};

/// Project subcommands (create, delete, import, etc.).
#[derive(Subcommand, Debug, Clone)]
pub enum ProjectCommands {
    #[clap(alias = "c")]
    /// (c) Create a new project in Todoist and add to config
    Create(Create),

    #[clap(alias = "l")]
    /// (l) List all of the projects in config
    List(List),

    #[clap(alias = "r")]
    /// (r) Remove a project from config (not Todoist)
    Remove(Remove),

    #[clap(alias = "d")]
    /// (d) Remove a project from Todoist
    Delete(Delete),

    #[clap(alias = "n")]
    /// (n) Rename a project in config (not in Todoist)
    Rename(Rename),

    #[clap(alias = "i")]
    /// (i) Get projects from Todoist and prompt to add to config
    Import(Import),

    #[clap(alias = "e")]
    /// (e) Empty a project by putting tasks in other projects
    Empty(Empty),

    #[clap(alias = "u")]
    /// (u) Update a project in Todoist
    Update(Update),

    #[clap(alias = "a")]
    /// (a) Archive a project
    Archive(Archive),

    /// Unarchive a project
    Unarchive(Unarchive),
}

#[derive(Parser, Debug, Clone)]
pub struct List {}

#[derive(Parser, Debug, Clone)]
pub struct Create {
    #[arg(short, long)]
    /// Project name
    name: Option<String>,

    #[arg(short, long)]
    /// Project description
    description: Option<String>,

    #[arg(short, long, default_value_t = false)]
    /// Whether the project is marked as favorite
    is_favorite: bool,
}

#[derive(Parser, Debug, Clone)]
pub struct Import {
    #[arg(
        short = 'a',
        long,
        default_value_t = false,
        conflicts_with_all = ["project", "id"]
    )]
    /// Add all projects to config that are not there already
    auto: bool,

    #[arg(short = 'p', long, conflicts_with = "id")]
    /// Import a specific project by name from Todoist
    project: Option<String>,

    #[arg(short = 'i', long, conflicts_with = "project")]
    /// Import a specific project by Todoist project ID
    id: Option<String>,
}

#[derive(Parser, Debug, Clone)]
pub struct Remove {
    #[arg(short = 'a', long, default_value_t = false)]
    /// Remove all projects from config that are not in Todoist
    auto: bool,

    #[arg(short = 'r', long, default_value_t = false)]
    /// Keep repeating prompt to remove projects. Use Ctrl/CMD + c to exit.
    repeat: bool,

    #[arg(short = 'l', long, default_value_t = false)]
    /// Remove all projects from config
    all: bool,

    #[arg(short, long)]
    /// Project to remove
    project: Option<String>,
}

#[derive(Parser, Debug, Clone)]
pub struct Delete {
    #[arg(short, long, default_value_t = false)]
    /// Skip deletion confirmation when the project has tasks
    force: bool,

    #[arg(short = 'r', long, default_value_t = false)]
    /// Keep repeating prompt to delete projects. Use Ctrl/CMD + c to exit.
    repeat: bool,

    #[arg(short, long)]
    /// Project to remove
    project: Option<String>,
}

#[derive(Parser, Debug, Clone)]
pub struct Rename {
    #[arg(short, long)]
    /// Project to rename
    project: Option<String>,

    #[arg(short, long)]
    /// New project name
    name: Option<String>,
}
#[derive(Parser, Debug, Clone)]
pub struct Empty {
    #[arg(short, long)]
    /// Project to empty
    project: Option<String>,
}

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

/// Creates a project in Todoist and adds it to config.
pub async fn create(config: &mut Config, args: &Create, json: bool) -> Result<String, Error> {
    let Create {
        name,
        description,
        is_favorite,
    } = args;
    let name = super::fetch_string(name.as_deref(), config, input::NAME)?;
    let description = description.as_deref().unwrap_or_default();

    projects::create(config, name, description, *is_favorite, json).await
}

/// Lists configured projects with task counts.
pub async fn list(config: &mut Config, _args: &List, json: bool) -> Result<String, Error> {
    if json {
        config.reload_projects().await?;
        let projects = config.projects().await?;
        Ok(serde_json::to_string(&projects)?)
    } else {
        projects::list(config).await
    }
}

/// Removes a project from config (local only).
pub async fn remove(config: &mut Config, args: &Remove) -> Result<String, Error> {
    let Remove {
        all,
        auto,
        project,
        repeat,
    } = args;
    match (all, auto) {
        (true, false) => projects::remove_all(config).await,
        (false, true) => projects::remove_auto(config).await,
        (false, false) => loop {
            let project = match super::fetch_project(project.as_deref(), config).await? {
                Flag::Project(project) => project,
                Flag::Filter(_) => unreachable!(),
            };
            let value = projects::remove(config, &project).await;

            if !repeat {
                return value;
            }
        },
        (_, _) => Err(Error::new("project_remove", "Incorrect flags provided")),
    }
}

/// Deletes a project from Todoist and removes from config.
pub async fn delete(config: &mut Config, args: &Delete) -> Result<String, Error> {
    let Delete {
        force,
        project,
        repeat,
    } = args;
    loop {
        let project = match super::fetch_project(project.as_deref(), config).await? {
            Flag::Project(project) => project,
            Flag::Filter(_) => unreachable!(),
        };
        let tasks = todoist::all_tasks_by_project(config, &project, None).await?;

        if !force && !tasks.is_empty() {
            if config.args.json {
                return Err(Error::new("json_mode", super::JSON_INTERACTIVE_ERROR));
            }
            println!();
            let options = vec![input::CANCEL, input::DELETE];
            let num_tasks = tasks.len();
            let desc = format!("Project has {num_tasks} tasks, confirm deletion");
            let result = input::select(&desc, options, config.mock_select)?;

            if result == input::CANCEL {
                return Ok("Cancelled".into());
            }
        }
        let value = projects::delete(config, &project).await;

        if !repeat {
            return value;
        }
    }
}

/// Renames a project in config (local only).
pub async fn rename(config: &mut Config, args: &Rename) -> Result<String, Error> {
    let Rename { project, name } = args;
    let project = match super::fetch_project(project.as_deref(), config).await? {
        Flag::Project(project) => project,
        Flag::Filter(_) => unreachable!(),
    };
    debug::maybe_print(
        config,
        &format!("Calling projects::rename with project:\n{project}"),
    );
    projects::rename(config, &project, name.as_deref()).await
}

/// Imports projects from Todoist into config.
pub async fn import(config: &mut Config, args: &Import, json: bool) -> Result<String, Error> {
    let Import { auto, project, id } = args;
    if !*auto && json {
        return Err(Error::new("json_mode", super::JSON_INTERACTIVE_ERROR));
    }
    projects::import(config, auto, project.as_deref(), id.as_deref(), json).await
}

/// Empties a project by moving tasks to other projects.
pub async fn empty(config: &mut Config, args: &Empty) -> Result<String, Error> {
    let Empty { project } = args;
    if config.args.json {
        return Err(Error::new("json_mode", super::JSON_INTERACTIVE_ERROR));
    }
    let project = match super::fetch_project(project.as_deref(), config).await? {
        Flag::Project(project) => project,
        Flag::Filter(_) => unreachable!(),
    };

    projects::empty(config, &project).await
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test;
    use crate::test::responses::ResponseFromFile;

    #[tokio::test]
    async fn remove_rejects_conflicting_all_and_auto_flags() {
        let mut config = Config::default_test();
        let args = Remove {
            auto: true,
            repeat: false,
            all: true,
            project: None,
        };

        let error = remove(&mut config, &args)
            .await
            .expect_err("conflicting flags should fail");
        assert_eq!(error.source, "project_remove");
        assert_eq!(error.message, "Incorrect flags provided");
    }

    #[tokio::test]
    async fn delete_force_skips_confirmation_prompt_for_non_empty_project() {
        let mut server = mockito::Server::new_async().await;

        let _tasks_mock = server
            .mock("GET", "/api/v1/tasks/?project_id=123&limit=200")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::TodayTasks.read().await)
            .create_async()
            .await;

        let delete_mock = server
            .mock("DELETE", "/api/v1/projects/123")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::Project.read().await)
            .create_async()
            .await;

        let mut config = test::fixtures::config()
            .await
            .with_mock_url(server.url())
            .mock_select(0)
            .create()
            .await
            .expect("config should be created");

        let args = Delete {
            force: true,
            project: Some("myproject".into()),
            repeat: false,
        };

        let result = delete(&mut config, &args)
            .await
            .expect("force delete should succeed");

        assert!(!result.contains("Cancelled"));
        delete_mock.assert_async().await;
    }

    #[tokio::test]
    async fn delete_cancels_when_user_selects_cancel_for_non_empty_project() {
        let mut server = mockito::Server::new_async().await;

        let tasks_mock = server
            .mock("GET", "/api/v1/tasks/?project_id=123&limit=200")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::TodayTasks.read().await)
            .create_async()
            .await;

        let mut config = test::fixtures::config()
            .await
            .with_mock_url(server.url())
            .mock_select(0) // selects CANCEL (first option) in confirmation prompt
            .create()
            .await
            .expect("config should be created");

        let args = Delete {
            force: false,
            project: Some("myproject".into()),
            repeat: false,
        };

        let result = delete(&mut config, &args)
            .await
            .expect("cancel should not error");

        assert_eq!(result, "Cancelled");
        tasks_mock.assert_async().await;
    }

    #[tokio::test]
    async fn delete_confirms_and_removes_project_when_user_selects_delete() {
        let mut server = mockito::Server::new_async().await;

        let _tasks_mock = server
            .mock("GET", "/api/v1/tasks/?project_id=123&limit=200")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::TodayTasks.read().await)
            .create_async()
            .await;

        let delete_mock = server
            .mock("DELETE", "/api/v1/projects/123")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::Project.read().await)
            .create_async()
            .await;

        let mut config = test::fixtures::config()
            .await
            .with_mock_url(server.url())
            .mock_select(1) // selects DELETE (second option) in confirmation prompt
            .create()
            .await
            .expect("config should be created");

        let args = Delete {
            force: false,
            project: Some("myproject".into()),
            repeat: false,
        };

        let result = delete(&mut config, &args)
            .await
            .expect("delete should succeed");

        assert!(!result.contains("Cancelled"));
        delete_mock.assert_async().await;
    }

    #[test]
    fn delete_force_flag_parses() {
        let args =
            Delete::try_parse_from(["tod", "--force"]).expect("delete arguments should parse");
        assert!(args.force);
    }

    #[test]
    fn rename_name_flag_parses() {
        let args = Rename::try_parse_from(["tod", "-p", "myproject", "-n", "renamed"])
            .expect("rename arguments should parse");
        assert_eq!(args.project.as_deref(), Some("myproject"));
        assert_eq!(args.name.as_deref(), Some("renamed"));
    }

    #[tokio::test]
    async fn rename_uses_name_flag_without_prompt() {
        let mut config = test::fixtures::config()
            .await
            .create()
            .await
            .expect("creating config should succeed");
        let args = Rename {
            project: Some("myproject".to_string()),
            name: Some("renamed-project".to_string()),
        };

        let result = rename(&mut config, &args).await;
        assert_eq!(result, Ok("✓".to_string()));

        let projects = config
            .projects()
            .await
            .expect("loading projects should succeed");
        let project_names = projects
            .iter()
            .map(|project| project.name.as_str())
            .collect::<Vec<&str>>();

        assert!(project_names.contains(&"renamed-project"));
        assert!(!project_names.contains(&"myproject"));
    }

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
}
