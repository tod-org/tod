use crate::config::Config;
use crate::errors::Error;
use crate::lists::Flag;
use crate::tasks::priority::{self, Priority};
use crate::{input, labels};
use auth_commands::AuthCommands;
use clap::command;
use clap::{Parser, Subcommand};
use config_commands::ConfigCommands;
use list_commands::ListCommands;
use project_commands::ProjectCommands;
use section_commands::SectionCommands;
use shell_commands::ShellCommands;
use std::fmt::Display;
use std::path::PathBuf;
use task_commands::TaskCommands;
use test_commands::TestCommands;
use tokio::sync::mpsc::UnboundedSender;

mod auth_commands;
mod config_commands;
mod list_commands;
mod project_commands;
mod section_commands;
mod shell_commands;
mod task_commands;
mod test_commands;

const NAME: &str = env!("CARGO_PKG_NAME");
const LONG_VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    " (",
    env!("BUILD_TARGET"),
    "-",
    env!("BUILD_PROFILE"),
    ")"
);
const AUTHOR: &str = env!("CARGO_PKG_AUTHORS");
const ABOUT: &str = env!("CARGO_PKG_DESCRIPTION");
const NO_PROJECTS_ERR: &str = "No projects in config. Add projects with `tod project import`";

#[derive(Parser, Clone)]
#[command(name = NAME)]
#[command(author = AUTHOR)]
#[command(version = LONG_VERSION)]
#[command(about = ABOUT, long_about = None)]
#[command(arg_required_else_help(true))]
pub struct Cli {
    #[arg(short, long, default_value_t = false)]
    /// Display additional debug info while processing
    pub verbose: bool,

    #[arg(short, long)]
    /// Absolute path of configuration. Defaults to $XDG_CONFIG_HOME/tod.cfg
    pub config: Option<PathBuf>,

    #[arg(short, long)]
    /// Time to wait for a response from API in seconds. Defaults to 30.
    pub timeout: Option<u64>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    #[command(subcommand)]
    #[clap(alias = "p")]
    /// (p) Commands that change projects
    Project(ProjectCommands),

    #[command(subcommand)]
    #[clap(alias = "n")]
    /// (n) Commands that change sections
    Section(SectionCommands),

    #[command(subcommand)]
    #[clap(alias = "t")]
    /// (t) Commands for individual tasks
    Task(TaskCommands),

    #[command(subcommand)]
    #[clap(alias = "l")]
    /// (l) Commands for multiple tasks
    List(ListCommands),

    #[command(subcommand)]
    #[clap(alias = "c")]
    /// (c) Commands around configuration and the app
    Config(ConfigCommands),

    #[command(subcommand)]
    #[clap(alias = "a")]
    /// (a) Commands for logging in with OAuth
    Auth(AuthCommands),

    #[command(subcommand)]
    #[clap(alias = "s")]
    /// (s) Commands for generating shell completions
    Shell(ShellCommands),

    #[command(subcommand)]
    #[clap(alias = "e")]
    /// (e) Commands for manually testing Tod against the API
    Test(TestCommands),
}

enum FlagOptions {
    Project,
    Filter,
}

impl Display for FlagOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FlagOptions::Project => write!(f, "Project"),
            FlagOptions::Filter => write!(f, "Filter"),
        }
    }
}

pub async fn select_command(
    cli: Cli,
    tx: UnboundedSender<Error>,
) -> (bool, bool, Result<String, Error>) {
    match &cli.command {
        // Project
        Commands::Project(ProjectCommands::Create(args)) => {
            let config = match fetch_config(&cli, &tx).await {
                Ok(config) => config,
                Err(e) => return (true, true, Err(e)),
            };
            (
                config.bell_on_success,
                config.bell_on_failure,
                project_commands::create(config, args).await,
            )
        }
        Commands::Project(ProjectCommands::List(args)) => {
            let config = match fetch_config(&cli, &tx).await {
                Ok(config) => config,
                Err(e) => return (true, true, Err(e)),
            };
            (
                config.bell_on_success,
                config.bell_on_failure,
                project_commands::list(config, args).await,
            )
        }
        Commands::Project(ProjectCommands::Remove(args)) => {
            let config = match fetch_config(&cli, &tx).await {
                Ok(config) => config,
                Err(e) => return (true, true, Err(e)),
            };
            (
                config.bell_on_success,
                config.bell_on_failure,
                project_commands::remove(config, args).await,
            )
        }
        Commands::Project(ProjectCommands::Rename(args)) => {
            let config = match fetch_config(&cli, &tx).await {
                Ok(config) => config,
                Err(e) => return (true, true, Err(e)),
            };
            (
                config.bell_on_success,
                config.bell_on_failure,
                project_commands::rename(config, args).await,
            )
        }
        Commands::Project(ProjectCommands::Import(args)) => {
            let config = match fetch_config(&cli, &tx).await {
                Ok(config) => config,
                Err(e) => return (true, true, Err(e)),
            };
            (
                config.bell_on_success,
                config.bell_on_failure,
                project_commands::import(config, args).await,
            )
        }
        Commands::Project(ProjectCommands::Empty(args)) => {
            let config = match fetch_config(&cli, &tx).await {
                Ok(config) => config,
                Err(e) => return (true, true, Err(e)),
            };
            (
                config.bell_on_success,
                config.bell_on_failure,
                project_commands::empty(&config, args).await,
            )
        }
        Commands::Project(ProjectCommands::Delete(args)) => {
            let config = match fetch_config(&cli, &tx).await {
                Ok(config) => config,
                Err(e) => return (true, true, Err(e)),
            };
            (
                config.bell_on_success,
                config.bell_on_failure,
                project_commands::delete(config, args).await,
            )
        }

        Commands::Section(SectionCommands::Create(args)) => {
            let config = match fetch_config(&cli, &tx).await {
                Ok(config) => config,
                Err(e) => return (true, true, Err(e)),
            };
            (
                config.bell_on_success,
                config.bell_on_failure,
                section_commands::create(config, args).await,
            )
        }

        // Task
        Commands::Task(TaskCommands::QuickAdd(args)) => {
            let config = match fetch_config(&cli, &tx).await {
                Ok(config) => config,
                Err(e) => return (true, true, Err(e)),
            };
            (
                config.bell_on_success,
                config.bell_on_failure,
                task_commands::quick_add(config, args).await,
            )
        }
        Commands::Task(TaskCommands::Create(args)) => {
            let config = match fetch_config(&cli, &tx).await {
                Ok(config) => config,
                Err(e) => return (true, true, Err(e)),
            };
            (
                config.bell_on_success,
                config.bell_on_failure,
                task_commands::create(config, args).await,
            )
        }
        Commands::Task(TaskCommands::Edit(args)) => {
            let config = match fetch_config(&cli, &tx).await {
                Ok(config) => config,
                Err(e) => return (true, true, Err(e)),
            };
            (
                config.bell_on_success,
                config.bell_on_failure,
                task_commands::edit(config, args).await,
            )
        }
        Commands::Task(TaskCommands::Next(args)) => {
            let config = match fetch_config(&cli, &tx).await {
                Ok(config) => config,
                Err(e) => return (true, true, Err(e)),
            };
            (
                config.bell_on_success,
                config.bell_on_failure,
                task_commands::next(config, args).await,
            )
        }
        Commands::Task(TaskCommands::Complete(args)) => {
            let config = match fetch_config(&cli, &tx).await {
                Ok(config) => config,
                Err(e) => return (true, true, Err(e)),
            };
            (
                config.bell_on_success,
                config.bell_on_failure,
                task_commands::complete(config, args).await,
            )
        }
        Commands::Task(TaskCommands::Comment(args)) => {
            let config = match fetch_config(&cli, &tx).await {
                Ok(config) => config,
                Err(e) => return (true, true, Err(e)),
            };
            (
                config.bell_on_success,
                config.bell_on_failure,
                task_commands::comment(config, args).await,
            )
        }

        // List
        Commands::List(ListCommands::View(args)) => {
            let config = match fetch_config(&cli, &tx).await {
                Ok(config) => config,
                Err(e) => return (true, true, Err(e)),
            };
            (
                config.bell_on_success,
                config.bell_on_failure,
                list_commands::view(config, args).await,
            )
        }
        Commands::List(ListCommands::Process(args)) => {
            let config = match fetch_config(&cli, &tx).await {
                Ok(config) => config,
                Err(e) => return (true, true, Err(e)),
            };
            (
                config.bell_on_success,
                config.bell_on_failure,
                list_commands::process(config, args).await,
            )
        }
        Commands::List(ListCommands::Prioritize(args)) => {
            let config = match fetch_config(&cli, &tx).await {
                Ok(config) => config,
                Err(e) => return (true, true, Err(e)),
            };
            (
                config.bell_on_success,
                config.bell_on_failure,
                list_commands::prioritize(config, args).await,
            )
        }
        Commands::List(ListCommands::Label(args)) => {
            let config = match fetch_config(&cli, &tx).await {
                Ok(config) => config,
                Err(e) => return (true, true, Err(e)),
            };
            (
                config.bell_on_success,
                config.bell_on_failure,
                list_commands::label(config, args).await,
            )
        }
        Commands::List(ListCommands::Schedule(args)) => {
            let config = match fetch_config(&cli, &tx).await {
                Ok(config) => config,
                Err(e) => return (true, true, Err(e)),
            };
            (
                config.bell_on_success,
                config.bell_on_failure,
                list_commands::schedule(config, args).await,
            )
        }
        Commands::List(ListCommands::Deadline(args)) => {
            let config = match fetch_config(&cli, &tx).await {
                Ok(config) => config,
                Err(e) => return (true, true, Err(e)),
            };
            (
                config.bell_on_success,
                config.bell_on_failure,
                list_commands::deadline(config, args).await,
            )
        }
        Commands::List(ListCommands::Timebox(args)) => {
            let config = match fetch_config(&cli, &tx).await {
                Ok(config) => config,
                Err(e) => return (true, true, Err(e)),
            };
            (
                config.bell_on_success,
                config.bell_on_failure,
                list_commands::timebox(config, args).await,
            )
        }
        Commands::List(ListCommands::Import(args)) => {
            let config = match fetch_config(&cli, &tx).await {
                Ok(config) => config,
                Err(e) => return (true, true, Err(e)),
            };
            (
                config.bell_on_success,
                config.bell_on_failure,
                list_commands::import(config, args).await,
            )
        }

        // Config
        Commands::Config(ConfigCommands::CheckVersion(args)) => {
            (true, true, config_commands::check_version(args, None).await)
        }

        Commands::Config(ConfigCommands::About(args)) => {
            (true, true, config_commands::about(args).await)
        }

        Commands::Config(ConfigCommands::Reset(args)) => (
            false,
            false,
            crate::config::config_reset(cli.config.clone(), args.force).await,
        ),

        Commands::Config(ConfigCommands::SetTimezone(args)) => {
            let config = match fetch_config(&cli, &tx).await {
                Ok(config) => config,
                Err(e) => return (true, true, Err(e)),
            };
            (
                config.bell_on_success,
                config.bell_on_failure,
                config_commands::set_timezone(config, args).await,
            )
        }

        Commands::Auth(AuthCommands::Login(args)) => {
            let config = match get_existing_config_exists(cli.config.clone()).await {
                Ok(config) => config,
                Err(_) => match fetch_config(&cli, &tx).await {
                    Ok(config) => config,
                    Err(e) => return (true, true, Err(e)),
                },
            };
            (
                config.bell_on_success,
                config.bell_on_failure,
                auth_commands::login(config, args).await,
            )
        }

        // Shell
        Commands::Shell(ShellCommands::Completions(args)) => {
            (true, true, shell_commands::completions(args).await)
        }

        // Test
        Commands::Test(TestCommands::All(args)) => {
            let config = match fetch_config(&cli, &tx).await {
                Ok(config) => config,
                Err(e) => return (true, true, Err(e)),
            };
            (
                config.bell_on_success,
                config.bell_on_failure,
                test_commands::all(config, args).await,
            )
        }
    }
}
/// Get or create config
async fn fetch_config(cli: &Cli, tx: &UnboundedSender<Error>) -> Result<Config, Error> {
    let Cli {
        verbose,
        config: config_path,
        timeout,
        command: _,
    } = cli;

    let config_path = config_path.to_owned();
    let verbose = verbose.to_owned();
    let timeout = timeout.to_owned();

    let config = crate::config::get_or_create(config_path, verbose, timeout, tx).await?;

    let async_config = config.clone();

    tokio::spawn(async move { async_config.check_for_latest_version().await });

    config.maybe_set_timezone().await
}

/// Only fetches the config if it exists, otherwise errors.
async fn get_existing_config_exists(config_path: Option<PathBuf>) -> Result<Config, Error> {
    match crate::config::get_config(config_path).await {
        Ok(config) => Ok(config),
        Err(e) => Err(e),
    }
}

fn fetch_string(
    maybe_string: Option<&str>,
    config: &Config,
    prompt: &str,
) -> Result<String, Error> {
    match maybe_string {
        Some(string) => Ok(string.to_owned()),
        None => input::string(prompt, config.mock_string.clone()),
    }
}
async fn fetch_project(project_name: Option<&str>, config: &Config) -> Result<Flag, Error> {
    let projects = config.projects().await?;
    if projects.is_empty() {
        return Err(Error::new("fetch_project", NO_PROJECTS_ERR));
    }

    if projects.len() == 1 {
        return Ok(Flag::Project(
            projects.first().expect("No projects found").clone(),
        ));
    }

    match project_name {
        Some(project_name) => projects
            .iter()
            .find(|p| p.name == project_name)
            .map_or_else(
                || {
                    Err(Error::new(
                        "fetch_project",
                        "Could not find project in config",
                    ))
                },
                |p| Ok(Flag::Project(p.to_owned())),
            ),
        None => input::select(input::PROJECT, projects, config.mock_select).map(Flag::Project),
    }
}

fn fetch_filter(filter: Option<&str>, config: &Config) -> Result<Flag, Error> {
    match filter {
        Some(string) => Ok(Flag::Filter(string.to_owned())),
        None => {
            let string = input::string(input::FILTER, config.mock_string.clone())?;
            Ok(Flag::Filter(string))
        }
    }
}

async fn fetch_project_or_filter(
    project: Option<&str>,
    filter: Option<&str>,
    config: &Config,
) -> Result<Flag, Error> {
    match (project, filter) {
        (Some(_), None) => fetch_project(project, config).await,
        (None, Some(_)) => fetch_filter(filter, config),
        (Some(_), Some(_)) => Err(Error::new(
            "project_or_filter",
            "Must select project OR filter",
        )),
        (None, None) => {
            let options = vec![FlagOptions::Project, FlagOptions::Filter];
            match input::select(input::OPTION, options, config.mock_select)? {
                FlagOptions::Project => fetch_project(project, config).await,
                FlagOptions::Filter => fetch_filter(filter, config),
            }
        }
    }
}

fn fetch_priority(priority: &Option<u8>, config: &Config) -> Result<Priority, Error> {
    match priority::from_integer(priority) {
        Some(priority) => Ok(priority),
        None => {
            let options = vec![
                Priority::None,
                Priority::Low,
                Priority::Medium,
                Priority::High,
            ];
            input::select(input::PRIORITY, options, config.mock_select)
        }
    }
}

async fn maybe_fetch_labels(config: &Config, labels: &[String]) -> Result<Vec<String>, Error> {
    if labels.is_empty() {
        let labels = labels::get_labels(config, false)
            .await?
            .into_iter()
            .map(|l| l.name)
            .collect();
        Ok(labels)
    } else {
        Ok(labels.to_vec())
    }
}
