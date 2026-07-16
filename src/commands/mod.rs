use crate::config::Config;
use crate::errors::Error;
use crate::lists::Flag;
use crate::tasks::priority::{self, Priority};
use crate::{CommandResult, input, labels};
use auth_commands::AuthCommands;
use clap::{Parser, Subcommand};
use config_commands::ConfigCommands;
use list_commands::ListCommands;
use project_commands::ProjectCommands;
use reminder_commands::ReminderCommands;
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
mod reminder_commands;
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
    /// Absolute path to configuration file. Defaults to `$XDG_CONFIG_HOME/tod.cfg`
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
    #[clap(alias = "r")]
    /// (r) Commands for reminders. Only available on Pro Todoist plans
    Reminder(ReminderCommands),

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

pub async fn select_command(cli: Cli, tx: UnboundedSender<Error>) -> Result<CommandResult, Error> {
    if cli.verbose {
        crate::debug::print(LONG_VERSION);
    }

    match &cli.command {
        Commands::Auth(command) => auth_command(command, &cli).await,
        Commands::Config(command) => config_command(command, &cli, &tx).await,
        Commands::List(command) => list_command(command, &cli, &tx).await,
        Commands::Project(command) => project_command(command, &cli, &tx).await,
        Commands::Reminder(command) => reminder_command(command, &cli, &tx).await,
        Commands::Section(command) => section_command(command, &cli, &tx).await,
        Commands::Shell(command) => shell_command(command).await,
        Commands::Task(command) => task_command(command, &cli, &tx).await,
        Commands::Test(command) => test_command(command, &cli, &tx).await,
        // Shell
    }
}

async fn shell_command(command: &ShellCommands) -> Result<CommandResult, Error> {
    match command {
        ShellCommands::Completions(args) => {
            let result = shell_commands::completions(args).await;
            build_command_result_without_config(result)
        }
    }
}

async fn reminder_command(
    command: &ReminderCommands,
    cli: &Cli,
    tx: &UnboundedSender<Error>,
) -> Result<CommandResult, Error> {
    match command {
        ReminderCommands::List(args) => {
            let mut config = fetch_config(cli, tx).await?;
            let result = reminder_commands::list(&mut config, args).await;
            build_command_result(result, config)
        }
    }
}

async fn section_command(
    command: &SectionCommands,
    cli: &Cli,
    tx: &UnboundedSender<Error>,
) -> Result<CommandResult, Error> {
    match command {
        SectionCommands::Create(args) => {
            let config = fetch_config(cli, tx).await?;
            let result = section_commands::create(&config, args).await;
            build_command_result(result, config)
        }
    }
}

async fn project_command(
    command: &ProjectCommands,
    cli: &Cli,
    tx: &UnboundedSender<Error>,
) -> Result<CommandResult, Error> {
    match command {
        ProjectCommands::Create(args) => {
            let mut config = fetch_config(cli, tx).await?;
            let result = project_commands::create(&mut config, args).await;
            build_command_result(result, config)
        }
        ProjectCommands::List(args) => {
            let mut config = fetch_config(cli, tx).await?;
            let result = project_commands::list(&mut config, args).await;
            build_command_result(result, config)
        }
        ProjectCommands::Remove(args) => {
            let mut config = fetch_config(cli, tx).await?;
            let result = project_commands::remove(&mut config, args).await;
            build_command_result(result, config)
        }
        ProjectCommands::Rename(args) => {
            let mut config = fetch_config(cli, tx).await?;
            let result = project_commands::rename(&mut config, args).await;
            build_command_result(result, config)
        }
        ProjectCommands::Import(args) => {
            let mut config = fetch_config(cli, tx).await?;
            let result = project_commands::import(&mut config, args).await;
            build_command_result(result, config)
        }
        ProjectCommands::Empty(args) => {
            let mut config = fetch_config(cli, tx).await?;
            let result = project_commands::empty(&mut config, args).await;
            build_command_result(result, config)
        }
        ProjectCommands::Delete(args) => {
            let mut config = fetch_config(cli, tx).await?;
            let result = project_commands::delete(&mut config, args).await;
            build_command_result(result, config)
        }
    }
}

async fn task_command(
    command: &TaskCommands,
    cli: &Cli,
    tx: &UnboundedSender<Error>,
) -> Result<CommandResult, Error> {
    match command {
        TaskCommands::QuickAdd(args) => {
            let config = fetch_config(cli, tx).await?;
            let result = task_commands::quick_add(&config, args).await;
            build_command_result(result, config)
        }
        TaskCommands::Create(args) => {
            let config = fetch_config(cli, tx).await?;
            let result = task_commands::create(config.clone(), args).await;
            build_command_result(result, config)
        }
        TaskCommands::Edit(args) => {
            let config = fetch_config(cli, tx).await?;
            let result = task_commands::edit(config.clone(), args).await;
            build_command_result(result, config)
        }
        TaskCommands::Next(args) => {
            let config = fetch_config(cli, tx).await?;
            let result = task_commands::next(config.clone(), args).await;
            build_command_result(result, config)
        }
        TaskCommands::Complete(args) => {
            let config = fetch_config(cli, tx).await?;
            let result = task_commands::complete(config.clone(), args).await;
            build_command_result(result, config)
        }
        TaskCommands::Comment(args) => {
            let config = fetch_config(cli, tx).await?;
            let result = task_commands::comment(config.clone(), args).await;
            build_command_result(result, config)
        }
    }
}

async fn list_command(
    command: &ListCommands,
    cli: &Cli,
    tx: &UnboundedSender<Error>,
) -> Result<CommandResult, Error> {
    match command {
        ListCommands::View(args) => {
            let mut config = fetch_config(cli, tx).await?;
            let result = list_commands::view(&mut config, args).await;
            build_command_result(result, config)
        }
        ListCommands::Process(args) => {
            let config = fetch_config(cli, tx).await?;
            let result = list_commands::process(config.clone(), args).await;
            build_command_result(result, config)
        }
        ListCommands::Prioritize(args) => {
            let config = fetch_config(cli, tx).await?;
            let result = list_commands::prioritize(config.clone(), args).await;
            build_command_result(result, config)
        }
        ListCommands::Remind(args) => {
            let config = fetch_config(cli, tx).await?;
            let result = list_commands::remind(config.clone(), args).await;
            build_command_result(result, config)
        }
        ListCommands::Label(args) => {
            let config = fetch_config(cli, tx).await?;
            let result = list_commands::label(config.clone(), args).await;
            build_command_result(result, config)
        }
        ListCommands::Schedule(args) => {
            let config = fetch_config(cli, tx).await?;
            let result = list_commands::schedule(config.clone(), args).await;
            build_command_result(result, config)
        }
        ListCommands::Deadline(args) => {
            let config = fetch_config(cli, tx).await?;
            let result = list_commands::deadline(config.clone(), args).await;
            build_command_result(result, config)
        }
        ListCommands::Timebox(args) => {
            let config = fetch_config(cli, tx).await?;
            let result = list_commands::timebox(config.clone(), args).await;
            build_command_result(result, config)
        }
        ListCommands::Import(args) => {
            let config = fetch_config(cli, tx).await?;
            let result = list_commands::import(config.clone(), args).await;
            build_command_result(result, config)
        }
    }
}

async fn config_command(
    command: &ConfigCommands,
    cli: &Cli,
    tx: &UnboundedSender<Error>,
) -> Result<CommandResult, Error> {
    match command {
        ConfigCommands::SetTimezone(args) => {
            let config = fetch_config(cli, tx).await?;
            let result = config_commands::set_timezone(config.clone(), args).await;
            build_command_result(result, config)
        }
        ConfigCommands::Edit(args) => {
            let config = fetch_config(cli, tx).await?;
            let result = config_commands::edit(config.clone(), args).await;
            build_command_result(result, config)
        }

        ConfigCommands::CheckVersion(args) => {
            let result = config_commands::check_version(args, None).await;
            build_command_result_without_config(result)
        }
        ConfigCommands::Check(_args) => {
            let result = config_commands::check(cli.config.clone()).await;
            build_command_result_without_config(result)
        }
        ConfigCommands::About(args) => {
            let result = config_commands::about(args).await;
            build_command_result_without_config(result)
        }
        ConfigCommands::Reset(args) => {
            let result = crate::config::config_reset(cli.config.clone(), args.force).await;
            build_command_result_without_config(result)
        }
        ConfigCommands::Open(_args) => {
            let result = crate::config::config_open(cli.config.clone()).await;
            build_command_result_without_config(result)
        }
    }
}

async fn auth_command(command: &AuthCommands, cli: &Cli) -> Result<CommandResult, Error> {
    match command {
        AuthCommands::Login(args) => {
            let mut config = auth_commands::load_or_create_config(cli.config.clone()).await?;
            let result = auth_commands::login(&mut config, args).await;
            build_command_result(result, config)
        }

        AuthCommands::Token(args) => {
            let result = auth_commands::token(cli.config.clone(), args).await;
            build_command_result_without_config(result)
        }
    }
}

async fn test_command(
    command: &TestCommands,
    cli: &Cli,
    tx: &UnboundedSender<Error>,
) -> Result<CommandResult, Error> {
    match command {
        TestCommands::All(args) => {
            let config = fetch_config(cli, tx).await?;
            let result = test_commands::all(&config, args).await;
            build_command_result(result, config)
        }
    }
}

fn build_command_result(
    result: Result<String, Error>,
    config: Config,
) -> Result<CommandResult, Error> {
    Ok(CommandResult {
        bell_success: config.bell_on_success,
        bell_failure: config.bell_on_failure,
        result,
    })
}

fn build_command_result_without_config(
    result: Result<String, Error>,
) -> Result<CommandResult, Error> {
    Ok(CommandResult {
        bell_success: false,
        bell_failure: true,
        result,
    })
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

    let config = config.check_for_latest_version().await?;
    config.maybe_set_timezone().await
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
    if let Some(string) = filter {
        Ok(Flag::Filter(string.to_owned()))
    } else {
        let string = input::string(input::FILTER, config.mock_string.clone())?;
        Ok(Flag::Filter(string))
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

fn fetch_priority(priority: Option<u8>, config: &Config) -> Result<Priority, Error> {
    if let Some(priority) = priority::from_integer(priority)? {
        Ok(priority)
    } else {
        let options = vec![
            Priority::None,
            Priority::Low,
            Priority::Medium,
            Priority::High,
        ];
        input::select(input::PRIORITY, options, config.mock_select)
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
