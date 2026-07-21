use clap::{Parser, Subcommand};

use crate::{config::Config, debug, errors::Error, input, lists::Flag, projects, todoist};

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
    /// Project to remove
    project: Option<String>,
}

pub async fn create(config: &mut Config, args: &Create) -> Result<String, Error> {
    let Create {
        name,
        description,
        is_favorite,
    } = args;
    let name = super::fetch_string(name.as_deref(), config, input::NAME)?;
    let description = description.as_deref().unwrap_or_default();

    projects::create(config, name, description, *is_favorite).await
}

pub async fn list(config: &mut Config, _args: &List) -> Result<String, Error> {
    projects::list(config).await
}

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

pub async fn import(config: &mut Config, args: &Import) -> Result<String, Error> {
    let Import { auto, project, id } = args;
    projects::import(config, auto, project.as_deref(), id.as_deref()).await
}

pub async fn empty(config: &mut Config, args: &Empty) -> Result<String, Error> {
    let Empty { project } = args;
    let project = match super::fetch_project(project.as_deref(), config).await? {
        Flag::Project(project) => project,
        Flag::Filter(_) => unreachable!(),
    };

    projects::empty(config, &project).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test;

    #[tokio::test]
    async fn remove_rejects_conflicting_all_and_auto_flags() {
        let mut config = Config::default();
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
}
