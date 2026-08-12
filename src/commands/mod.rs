use crate::config::Config;
use crate::errors::Error;
use crate::lists::Flag;
use crate::tasks::priority::{self, Priority};
use crate::{CommandResult, input, labels};
use auth_commands::AuthCommands;
use clap::{Parser, Subcommand};
use config_commands::ConfigCommands;
use label_commands::LabelCommands;
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
mod label_commands;
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
const JSON_INTERACTIVE_ERROR: &str =
    "Interactive input not available in JSON mode. Provide the required argument via CLI flags.";

/// Parsed command-line arguments.
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

    #[arg(short = 'j', long, global = true, default_value_t = false)]
    /// Output results as JSON for machine-readable consumption
    pub json: bool,

    #[command(subcommand)]
    pub command: Commands,
}

/// Top-level command groups.
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
    #[clap(alias = "b")]
    /// (b) Commands for managing personal labels
    Label(LabelCommands),

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
    /// (a) Commands for logging in with OAuth or managing API tokens
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

/// Routes a parsed command to its handler.
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
        Commands::Label(command) => label_command(command, &cli, &tx).await,
        Commands::Shell(command) => shell_command(command, cli.json).await,
        Commands::Task(command) => task_command(command, &cli, &tx).await,
        Commands::Test(command) => test_command(command, &cli, &tx).await,
    }
}

async fn shell_command(command: &ShellCommands, json: bool) -> Result<CommandResult, Error> {
    match command {
        ShellCommands::Completions(args) => {
            let result = shell_commands::completions(args).await;
            Ok(build_command_result_without_config(result, json))
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
            let result = reminder_commands::list(&mut config, args, cli.json).await;
            Ok(build_command_result(result, &config))
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
            let result = section_commands::create(&config, args, cli.json).await;
            Ok(build_command_result(result, &config))
        }
        SectionCommands::Delete(args) => {
            let config = fetch_config(cli, tx).await?;
            let result = section_commands::delete(&config, args, cli.json).await;
            Ok(build_command_result(result, &config))
        }
    }
}

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

async fn project_command(
    command: &ProjectCommands,
    cli: &Cli,
    tx: &UnboundedSender<Error>,
) -> Result<CommandResult, Error> {
    match command {
        ProjectCommands::Create(args) => {
            let mut config = fetch_config(cli, tx).await?;
            let result = project_commands::create(&mut config, args, cli.json).await;
            Ok(build_command_result(result, &config))
        }
        ProjectCommands::List(args) => {
            let mut config = fetch_config(cli, tx).await?;
            let result = project_commands::list(&mut config, args, cli.json).await;
            Ok(build_command_result(result, &config))
        }
        ProjectCommands::Remove(args) => {
            let mut config = fetch_config(cli, tx).await?;
            let result = project_commands::remove(&mut config, args).await;
            Ok(build_command_result(result, &config))
        }
        ProjectCommands::Rename(args) => {
            let mut config = fetch_config(cli, tx).await?;
            let result = project_commands::rename(&mut config, args).await;
            Ok(build_command_result(result, &config))
        }
        ProjectCommands::Import(args) => {
            let mut config = fetch_config(cli, tx).await?;
            let result = project_commands::import(&mut config, args, cli.json).await;
            Ok(build_command_result(result, &config))
        }
        ProjectCommands::Empty(args) => {
            let mut config = fetch_config(cli, tx).await?;
            let result = project_commands::empty(&mut config, args).await;
            Ok(build_command_result(result, &config))
        }
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

async fn task_command(
    command: &TaskCommands,
    cli: &Cli,
    tx: &UnboundedSender<Error>,
) -> Result<CommandResult, Error> {
    match command {
        TaskCommands::QuickAdd(args) => {
            let config = fetch_config(cli, tx).await?;
            let result = task_commands::quick_add(&config, args, cli.json).await;
            Ok(build_command_result(result, &config))
        }
        TaskCommands::Create(args) => {
            let config = fetch_config(cli, tx).await?;
            let result = task_commands::create(config.clone(), args, cli.json).await;
            Ok(build_command_result(result, &config))
        }
        TaskCommands::Edit(args) => {
            let config = fetch_config(cli, tx).await?;
            let result = task_commands::edit(config.clone(), args, cli.json).await;
            Ok(build_command_result(result, &config))
        }
        TaskCommands::Next(args) => {
            let config = fetch_config(cli, tx).await?;
            let result = task_commands::next(config.clone(), args, cli.json).await;
            Ok(build_command_result(result, &config))
        }
        TaskCommands::Complete(args) => {
            let config = fetch_config(cli, tx).await?;
            let result = task_commands::complete(config.clone(), args, cli.json).await;
            Ok(build_command_result(result, &config))
        }
        TaskCommands::Reopen(args) => {
            let config = fetch_config(cli, tx).await?;
            let result = task_commands::reopen(config.clone(), args, cli.json).await;
            Ok(build_command_result(result, &config))
        }
        TaskCommands::Comment(args) => {
            let config = fetch_config(cli, tx).await?;
            let result = task_commands::comment(config.clone(), args, cli.json).await;
            Ok(build_command_result(result, &config))
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
            let result = list_commands::view(&mut config, args, cli.json).await;
            Ok(build_command_result(result, &config))
        }
        ListCommands::Process(args) => {
            let config = fetch_config(cli, tx).await?;
            let result = list_commands::process(config.clone(), args).await;
            Ok(build_command_result(result, &config))
        }
        ListCommands::Prioritize(args) => {
            let config = fetch_config(cli, tx).await?;
            let result = list_commands::prioritize(config.clone(), args, cli.json).await;
            Ok(build_command_result(result, &config))
        }
        ListCommands::Remind(args) => {
            let config = fetch_config(cli, tx).await?;
            let result = list_commands::remind(config.clone(), args, cli.json).await;
            Ok(build_command_result(result, &config))
        }
        ListCommands::Label(args) => {
            let config = fetch_config(cli, tx).await?;
            let result = list_commands::label(config.clone(), args).await;
            Ok(build_command_result(result, &config))
        }
        ListCommands::Schedule(args) => {
            let config = fetch_config(cli, tx).await?;
            let result = list_commands::schedule(config.clone(), args, cli.json).await;
            Ok(build_command_result(result, &config))
        }
        ListCommands::Deadline(args) => {
            let config = fetch_config(cli, tx).await?;
            let result = list_commands::deadline(config.clone(), args, cli.json).await;
            Ok(build_command_result(result, &config))
        }
        ListCommands::Timebox(args) => {
            let config = fetch_config(cli, tx).await?;
            let result = list_commands::timebox(config.clone(), args).await;
            Ok(build_command_result(result, &config))
        }
        ListCommands::Import(args) => {
            let config = fetch_config(cli, tx).await?;
            let result = list_commands::import(config.clone(), args, cli.json).await;
            Ok(build_command_result(result, &config))
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
            Ok(build_command_result(result, &config))
        }
        ConfigCommands::Edit(args) => {
            let config = fetch_config(cli, tx).await?;
            let result = config_commands::edit(config.clone(), args).await;
            Ok(build_command_result(result, &config))
        }

        ConfigCommands::CheckVersion(args) => {
            let result = config_commands::check_version(args, None, cli.json).await;
            Ok(build_command_result_without_config(result, cli.json))
        }
        ConfigCommands::Check(_args) => {
            let result = config_commands::check(cli.config.clone(), cli.json).await;
            Ok(build_command_result_without_config(result, cli.json))
        }
        ConfigCommands::About(args) => {
            let result = config_commands::about(args, cli.json).await;
            Ok(build_command_result_without_config(result, cli.json))
        }
        ConfigCommands::Reset(args) => {
            let result = crate::config::config_reset(cli.config.clone(), args.force).await;
            Ok(build_command_result_without_config(result, cli.json))
        }
        ConfigCommands::Open(_args) => {
            let result = crate::config::config_open(cli.config.clone()).await;
            Ok(build_command_result_without_config(result, cli.json))
        }
    }
}

async fn auth_command(command: &AuthCommands, cli: &Cli) -> Result<CommandResult, Error> {
    match command {
        AuthCommands::Login(args) => {
            if cli.json {
                return Ok(build_command_result_without_config(
                    Err(Error::new("json_mode", JSON_INTERACTIVE_ERROR)),
                    cli.json,
                ));
            }
            let mut config = auth_commands::load_or_create_config(cli.config.clone()).await?;
            let result = auth_commands::login(&mut config, args).await;
            Ok(build_command_result(result, &config))
        }

        AuthCommands::Token(args) => {
            let result = auth_commands::token(cli.config.clone(), args, cli.json).await;
            Ok(build_command_result_without_config(result, cli.json))
        }

        AuthCommands::View(_args) => {
            let result = auth_commands::view(cli.config.clone(), cli.json).await;
            Ok(build_command_result_without_config(result, cli.json))
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
            Ok(build_command_result(result, &config))
        }
    }
}

fn build_command_result(result: Result<String, Error>, config: &Config) -> CommandResult {
    CommandResult {
        bell_success: config.bell_on_success,
        bell_failure: config.bell_on_failure,
        json: config.args.json,
        result,
    }
}

fn build_command_result_without_config(result: Result<String, Error>, json: bool) -> CommandResult {
    CommandResult {
        bell_success: false,
        bell_failure: true,
        json,
        result,
    }
}

/// Load existing config and ensure auth is present.
async fn fetch_config(cli: &Cli, tx: &UnboundedSender<Error>) -> Result<Config, Error> {
    let config = get_existing_config_exists(cli.config.clone()).await?;
    let config = with_cli_context(config, cli, tx);
    crate::debug::maybe_print_redacted_config(&config);
    ensure_auth_present(&config, "fetch_config")?;
    let config = config.check_for_latest_version().await?;
    config.maybe_set_timezone().await
}

/// Only fetches the config if it exists, otherwise errors.
async fn get_existing_config_exists(config_path: Option<PathBuf>) -> Result<Config, Error> {
    match crate::config::get_config(config_path).await {
        Ok(config) => Ok(config),
        Err(e) => Err(e),
    }
}

fn with_cli_context(mut config: Config, cli: &Cli, tx: &UnboundedSender<Error>) -> Config {
    config.args.verbose = cli.verbose;
    config.args.timeout = cli.timeout;
    config.args.json = cli.json;
    config.internal.tx = Some(tx.clone());
    if cli.json {
        config.spinners = Some(false);
    }
    config
}

fn ensure_auth_present(config: &Config, source: &str) -> Result<(), Error> {
    if config
        .token
        .as_ref()
        .is_none_or(|token| token.trim().is_empty())
    {
        Err(Error::new(
            source,
            "No auth present - run \"tod auth login\"",
        ))
    } else {
        Ok(())
    }
}

/// Resolves task content from an argument or interactive prompt.
fn fetch_string(
    maybe_string: Option<&str>,
    config: &Config,
    prompt: &str,
) -> Result<String, Error> {
    match maybe_string {
        Some(string) => Ok(string.to_owned()),
        None => {
            if config.args.json {
                return Err(Error::new("json_mode", JSON_INTERACTIVE_ERROR));
            }
            input::string(prompt, config.mock_string.clone())
        }
    }
}
/// Resolves a project name from an argument or interactive prompt.
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
        None => {
            if config.args.json {
                return Err(Error::new("json_mode", JSON_INTERACTIVE_ERROR));
            }
            input::select(input::PROJECT, projects, config.mock_select).map(Flag::Project)
        }
    }
}

/// Wraps a filter string in `Flag::Filter`, or prompts for one.
fn fetch_filter(filter: Option<&str>, config: &Config) -> Result<Flag, Error> {
    if let Some(string) = filter {
        Ok(Flag::Filter(string.to_owned()))
    } else {
        if config.args.json {
            return Err(Error::new("json_mode", JSON_INTERACTIVE_ERROR));
        }
        let string = input::string(input::FILTER, config.mock_string.clone())?;
        Ok(Flag::Filter(string))
    }
}

/// Resolves a project or filter from arguments, errors if both are set.
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
            if config.args.json {
                return Err(Error::new("json_mode", JSON_INTERACTIVE_ERROR));
            }
            let options = vec![FlagOptions::Project, FlagOptions::Filter];
            match input::select(input::OPTION, options, config.mock_select)? {
                FlagOptions::Project => fetch_project(project, config).await,
                FlagOptions::Filter => fetch_filter(filter, config),
            }
        }
    }
}

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
        Ok(labels
            .iter()
            .find(|l| l.name == selected)
            .expect("selected label must exist in labels slice"))
    }
}

/// Converts a u8 to Priority or prompts the user.
fn fetch_priority(priority: Option<u8>, config: &Config) -> Result<Priority, Error> {
    if let Some(priority) = priority::from_integer(priority)? {
        Ok(priority)
    } else {
        if config.args.json {
            return Err(Error::new("json_mode", JSON_INTERACTIVE_ERROR));
        }
        let options = vec![
            Priority::None,
            Priority::Low,
            Priority::Medium,
            Priority::High,
        ];
        input::select(input::PRIORITY, options, config.mock_select)
    }
}

/// Returns the provided labels or fetches them from the API.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tasks::priority::Priority;
    use crate::test;
    use crate::test::responses::ResponseFromFile;
    use crate::test_time::FixedTimeProvider;
    use crate::time::TimeProviderEnum;
    use tokio::sync::mpsc;

    #[test]
    fn build_command_result_uses_config_bell_settings() {
        let mut config = Config::default_test();
        config.bell_on_success = true;
        config.bell_on_failure = false;
        config.args.json = true;

        let result = build_command_result(Ok("ok".to_string()), &config);
        assert!(result.bell_success);
        assert!(!result.bell_failure);
        assert!(result.json);
        assert!(matches!(result.result, Ok(text) if text == "ok"));
    }

    #[test]
    fn build_command_result_without_config_uses_defaults() {
        let result = build_command_result_without_config(Ok("ok".to_string()), false);
        assert!(!result.bell_success);
        assert!(result.bell_failure);
        assert!(!result.json);
        assert!(matches!(result.result, Ok(text) if text == "ok"));
    }

    #[test]
    fn ensure_auth_present_errors_when_token_missing() {
        let mut config = Config::default_test();
        config.token = None;

        let result = ensure_auth_present(&config, "test-source");
        let error = result.expect_err("missing token should fail auth check");
        assert!(error.message.contains("tod auth login"));
    }

    #[test]
    fn ensure_auth_present_errors_when_token_whitespace() {
        let mut config = Config::default_test();
        config.token = Some("   ".to_string());

        let result = ensure_auth_present(&config, "test-source");
        let error = result.expect_err("whitespace token should fail auth check");
        assert!(error.message.contains("tod auth login"));
    }

    #[test]
    fn ensure_auth_present_succeeds_with_token() {
        let mut config = Config::default_test();
        config.token = Some("token".to_string());

        let result = ensure_auth_present(&config, "test-source");
        assert!(result.is_ok());
    }

    #[test]
    fn with_cli_context_sets_verbose_timeout_and_tx() {
        let (tx, _rx) = mpsc::unbounded_channel::<Error>();
        let cli = Cli {
            verbose: true,
            config: None,
            timeout: Some(42),
            json: false,
            command: Commands::Test(TestCommands::All(test_commands::All {})),
        };
        let config = Config::default_test();

        let result = with_cli_context(config, &cli, &tx);

        assert!(result.args.verbose);
        assert_eq!(result.args.timeout, Some(42));
        assert!(result.internal.tx.is_some());
    }

    #[test]
    fn fetch_filter_returns_provided_value() {
        let config = Config::default_test();

        let result = fetch_filter(Some("myfilter"), &config);

        let flag = result.expect("filter should be returned");
        assert!(matches!(flag, Flag::Filter(f) if f == "myfilter"));
    }

    #[test]
    fn fetch_filter_uses_mock_string_when_none() {
        let mut config = Config::default_test();
        config.mock_string = Some("prompted-filter".to_string());

        let result = fetch_filter(None, &config);

        let flag = result.expect("filter should be prompted");
        assert!(matches!(flag, Flag::Filter(f) if f == "prompted-filter"));
    }

    #[test]
    fn fetch_priority_returns_from_valid_integer() {
        let config = Config::default_test();

        let result = fetch_priority(Some(4), &config);

        let priority = result.expect("priority 4 should be High");
        assert_eq!(priority, Priority::High);
    }

    #[test]
    fn fetch_priority_uses_mock_select_when_none() {
        let mut config = Config::default_test();
        config.mock_select = Some(1);

        let result = fetch_priority(None, &config);

        let priority = result.expect("should select priority from list");
        assert_eq!(priority, Priority::Low);
    }

    #[test]
    fn fetch_string_returns_provided_value() {
        let config = Config::default_test();

        let result = fetch_string(Some("hello"), &config, "test prompt");

        assert_eq!(result.expect("should return provided string"), "hello");
    }

    #[test]
    fn fetch_string_uses_mock_string_when_none() {
        let mut config = Config::default_test();
        config.mock_string = Some("mocked input".to_string());

        let result = fetch_string(None, &config, "test prompt");

        assert_eq!(result.expect("should return mocked string"), "mocked input");
    }

    #[tokio::test]
    async fn maybe_fetch_labels_returns_provided_labels() {
        let config = Config::default_test();
        let labels = vec!["label1".to_string(), "label2".to_string()];

        let result = maybe_fetch_labels(&config, &labels).await;

        assert_eq!(
            result.expect("should return provided labels"),
            vec!["label1", "label2"]
        );
    }

    #[tokio::test]
    async fn maybe_fetch_labels_fetches_from_api_when_empty() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/api/v1/labels?limit=200")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::Labels.read().await)
            .create_async()
            .await;

        let config = test::fixtures::config()
            .await
            .with_mock_url(server.url())
            .with_time_provider(TimeProviderEnum::Fixed(FixedTimeProvider));

        let result = maybe_fetch_labels(&config, &[]).await;

        assert!(
            result.is_ok(),
            "should fetch labels from API when none provided"
        );
        assert!(!result.unwrap().is_empty());
        mock.assert();
    }

    #[test]
    fn fetch_string_json_mode_errors_when_no_input() {
        let mut config = Config::default_test();
        config.args.json = true;

        let result = fetch_string(None, &config, "test prompt");
        let error = result.expect_err("should error in JSON mode");
        assert_eq!(error.source, "json_mode");
        assert!(error.message.contains("JSON mode"));
    }

    #[test]
    fn fetch_string_json_mode_ok_when_provided() {
        let mut config = Config::default_test();
        config.args.json = true;

        let result = fetch_string(Some("hello"), &config, "test prompt");
        assert_eq!(result.expect("should succeed"), "hello");
    }

    #[test]
    fn fetch_filter_json_mode_errors_when_no_input() {
        let mut config = Config::default_test();
        config.args.json = true;

        let result = fetch_filter(None, &config);
        let error = result.expect_err("should error in JSON mode");
        assert_eq!(error.source, "json_mode");
        assert!(error.message.contains("JSON mode"));
    }

    #[test]
    fn fetch_filter_json_mode_ok_when_provided() {
        let mut config = Config::default_test();
        config.args.json = true;

        let result = fetch_filter(Some("myfilter"), &config);
        let flag = result.expect("should succeed");
        assert!(matches!(flag, Flag::Filter(f) if f == "myfilter"));
    }

    #[test]
    fn fetch_priority_json_mode_errors_when_no_input() {
        let mut config = Config::default_test();
        config.args.json = true;

        let result = fetch_priority(None, &config);
        let error = result.expect_err("should error in JSON mode");
        assert_eq!(error.source, "json_mode");
        assert!(error.message.contains("JSON mode"));
    }

    #[test]
    fn fetch_priority_json_mode_ok_when_provided() {
        let mut config = Config::default_test();
        config.args.json = true;

        let result = fetch_priority(Some(4), &config);
        assert_eq!(result.expect("should succeed"), Priority::High);
    }

    fn test_project() -> crate::projects::Project {
        crate::projects::Project {
            id: "123".to_string(),
            name: "test-project".to_string(),
            can_assign_tasks: false,
            child_order: 0,
            color: "red".to_string(),
            created_at: None,
            is_archived: false,
            is_deleted: false,
            is_favorite: false,
            is_frozen: false,
            updated_at: None,
            view_style: "list".to_string(),
            default_order: 0,
            description: String::new(),
            parent_id: None,
            inbox_project: None,
            is_collapsed: false,
            is_shared: false,
        }
    }

    #[tokio::test]
    async fn fetch_project_json_mode_errors_when_no_project_name() {
        let mut config = Config::default_test();
        config.add_project(test_project());
        config.args.json = true;

        let result = fetch_project(None, &config).await;
        let error = result.expect_err("should error in JSON mode");
        assert_eq!(error.source, "json_mode");
        assert!(error.message.contains("JSON mode"));
    }

    #[tokio::test]
    async fn fetch_project_json_mode_ok_when_provided() {
        let mut config = Config::default_test();
        config.add_project(test_project());
        config.args.json = true;

        let result = fetch_project(Some("test-project"), &config).await;
        let flag = result.expect("should succeed");
        assert!(matches!(flag, Flag::Project(p) if p.name == "test-project"));
    }

    #[tokio::test]
    async fn fetch_project_or_filter_json_mode_errors_when_both_none() {
        let mut config = Config::default_test();
        config.add_project(test_project());
        config.args.json = true;

        let result = fetch_project_or_filter(None, None, &config).await;
        let error = result.expect_err("should error in JSON mode");
        assert_eq!(error.source, "json_mode");
        assert!(error.message.contains("JSON mode"));
    }

    #[tokio::test]
    async fn fetch_project_or_filter_json_mode_ok_with_project() {
        let mut config = Config::default_test();
        config.add_project(test_project());
        config.args.json = true;

        let result = fetch_project_or_filter(Some("test-project"), None, &config).await;
        let flag = result.expect("should succeed");
        assert!(matches!(flag, Flag::Project(p) if p.name == "test-project"));
    }

    #[tokio::test]
    async fn fetch_project_or_filter_json_mode_ok_with_filter() {
        let mut config = Config::default_test();
        config.args.json = true;

        let result = fetch_project_or_filter(None, Some("myfilter"), &config).await;
        let flag = result.expect("should succeed");
        assert!(matches!(flag, Flag::Filter(f) if f == "myfilter"));
    }
}
