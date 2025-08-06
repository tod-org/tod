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
    /// (e) Empty a project by putting tasks in other projects"
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
    #[arg(short = 'a', long, default_value_t = false)]
    /// Add all projects to config that are not there already
    auto: bool,
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
    /// Project to remove
    project: Option<String>,
}
#[derive(Parser, Debug, Clone)]
pub struct Empty {
    #[arg(short, long)]
    /// Project to remove
    project: Option<String>,
}

pub async fn create(config: Config, args: &Create) -> Result<String, Error> {
    let Create {
        name,
        description,
        is_favorite,
    } = args;
    let name = super::fetch_string(name.as_deref(), &config, input::NAME)?;
    let description = description.clone().unwrap_or_default();
    let mut config = config;
    projects::create(&mut config, name, description, *is_favorite).await
}

pub async fn list(config: Config, _args: &List) -> Result<String, Error> {
    let mut config = config.clone();
    projects::list(&mut config).await
}

pub async fn remove(config: Config, args: &Remove) -> Result<String, Error> {
    let Remove {
        all,
        auto,
        project,
        repeat,
    } = args;
    let mut config = config.clone();
    match (all, auto) {
        (true, false) => projects::remove_all(&mut config).await,
        (false, true) => projects::remove_auto(&mut config).await,
        (false, false) => loop {
            let project = match super::fetch_project(project.as_deref(), &config).await? {
                Flag::Project(project) => project,
                _ => unreachable!(),
            };
            let value = projects::remove(&mut config, &project).await;

            if !repeat {
                return value;
            }
        },
        (_, _) => Err(Error::new("project_remove", "Incorrect flags provided")),
    }
}

pub async fn delete(config: Config, args: &Delete) -> Result<String, Error> {
    let Delete { project, repeat } = args;
    let mut config = config.clone();
    loop {
        let project = match super::fetch_project(project.as_deref(), &config).await? {
            Flag::Project(project) => project,
            _ => unreachable!(),
        };
        let tasks = todoist::all_tasks_by_project(&config, &project, None).await?;

        if !tasks.is_empty() {
            println!();
            let options = vec![input::CANCEL, input::DELETE];
            let num_tasks = tasks.len();
            let desc = format!("Project has {num_tasks} tasks, confirm deletion");
            let result = input::select(&desc, options, config.mock_select)?;

            if result == input::CANCEL {
                return Ok("Cancelled".into());
            }
        }
        let value = projects::delete(&mut config, &project).await;

        if !repeat {
            return value;
        }
    }
}

pub async fn rename(config: Config, args: &Rename) -> Result<String, Error> {
    let Rename { project } = args;
    let project = match super::fetch_project(project.as_deref(), &config).await? {
        Flag::Project(project) => project,
        _ => unreachable!(),
    };
    debug::maybe_print(
        &config,
        format!("Calling projects::rename with project:\n{project}"),
    );
    projects::rename(config, &project).await
}

pub async fn import(config: Config, args: &Import) -> Result<String, Error> {
    let Import { auto } = args;

    let mut config = config.clone();
    projects::import(&mut config, auto).await
}

pub async fn empty(config: &Config, args: &Empty) -> Result<String, Error> {
    let Empty { project } = args;
    let project = match super::fetch_project(project.as_deref(), config).await? {
        Flag::Project(project) => project,
        _ => unreachable!(),
    };

    let mut config = config.clone();
    projects::empty(&mut config, &project).await
}
