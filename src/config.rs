use crate::cargo::Version;
use crate::errors::Error;
use crate::id::Resource;
use crate::input::page_size;
use crate::projects::{LegacyProject, Project};
use crate::tasks::Task;
use crate::tasks::format::maybe_format_url;
use crate::time::{SystemTimeProvider, TimeProviderEnum};
use crate::{VERSION, cargo, color, debug, input, oauth, time, todoist};
use inquire::Confirm;
use rand::distr::{Alphanumeric, SampleString};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::Path;
use std::path::PathBuf;
use terminal_size::{Height, Width, terminal_size};
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc::UnboundedSender;

#[cfg(test)]
use crate::test_time::FixedTimeProvider;

const MAX_COMMENT_LENGTH: u32 = 500;
pub const DEFAULT_DEADLINE_VALUE: u8 = 30;
pub const DEFAULT_DEADLINE_DAYS: u8 = 5;
pub const OAUTH: &str = "Login with OAuth (recommended)";
pub const DEVELOPER: &str = "Login with developer API token";
pub const TOKEN_METHOD: &str = "Choose your Todoist login method";

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Completed {
    count: u32,
    date: String,
}

/// App configuration, serialized as json in $XDG_CONFIG_HOME/tod.cfg
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// The Todoist Api token
    pub token: Option<String>,
    /// List of Todoist projects and their project numbers
    #[serde(rename = "projectsv1")]
    projects: Option<Vec<Project>>,
    /// These are from the old v9 and SYNC endpoints
    #[serde(rename = "vecprojects")]
    legacy_projects: Option<Vec<LegacyProject>>,
    /// Path to config file
    pub path: PathBuf,
    /// The ID of the next task (NO LONGER IN USE)
    next_id: Option<String>,
    /// The next task, for use with complete
    #[serde(rename = "next_taskv1")]
    next_task: Option<Task>,
    /// Whether to trigger terminal bell on success
    #[serde(default)]
    pub bell_on_success: bool,
    /// Whether to trigger terminal bell on error
    #[serde(default = "bell_on_failure")]
    pub bell_on_failure: bool,
    /// A command to to run on task creation
    pub task_create_command: Option<String>,
    /// A command to run on task completion
    pub task_complete_command: Option<String>,
    /// A command to run on task comment creation
    pub task_comment_command: Option<String>,
    /// Regex to exclude tasks
    #[serde(with = "serde_regex")]
    pub task_exclude_regex: Option<Regex>,
    /// The timezone to use for the config
    timezone: Option<String>,
    pub timeout: Option<u64>,
    /// The last time we checked crates.io for the version
    pub last_version_check: Option<String>,
    pub mock_url: Option<String>,
    pub mock_string: Option<String>,
    pub mock_select: Option<usize>,
    /// Whether spinners are enabled
    pub spinners: Option<bool>,
    #[serde(default)]
    pub disable_links: bool,
    pub completed: Option<Completed>,
    /// Maximum length for printing comments
    pub max_comment_length: Option<u32>,
    /// Regex to exclude specific comments
    #[serde(with = "serde_regex")]
    pub comment_exclude_regex: Option<Regex>,

    pub verbose: Option<bool>,
    /// Don't ask for sections
    pub no_sections: Option<bool>,
    /// Goes straight to natural language input in datetime selection
    pub natural_language_only: Option<bool>,
    pub sort_value: Option<SortValue>,

    /// For storing arguments from the commandline
    #[serde(skip)]
    pub args: Args,

    /// For storing arguments from the commandline
    #[serde(skip)]
    pub internal: Internal,
    /// Optional TimeProvider for testing, defaults to SystemTimeProvider
    #[serde(skip)]
    pub time_provider: TimeProviderEnum,
}

fn bell_on_failure() -> bool {
    true
}
#[derive(Default, Clone, Eq, PartialEq, Debug)]
pub struct Args {
    pub verbose: bool,
    pub timeout: Option<u64>,
}
#[derive(Default, Clone, Debug)]
pub struct Internal {
    pub tx: Option<UnboundedSender<Error>>,
}

// Determining how
#[derive(Clone, Serialize, Deserialize, Eq, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct SortValue {
    /// Task has one of these priorities
    pub priority_none: u8,
    pub priority_low: u8,
    pub priority_medium: u8,
    pub priority_high: u8,
    pub no_due_date: u8,
    pub not_recurring: u8,
    pub today: u8,
    pub overdue: u8,
    /// Happens now plus or minus 15min
    pub now: u8,
    pub deadline_value: Option<u8>,
    pub deadline_days: Option<u8>,
}

impl Default for SortValue {
    fn default() -> Self {
        SortValue {
            priority_none: 2,
            priority_low: 1,
            priority_medium: 3,
            priority_high: 4,
            no_due_date: 80,
            overdue: 150,
            not_recurring: 50,
            today: 100,
            now: 200,
            deadline_value: Some(DEFAULT_DEADLINE_VALUE),
            deadline_days: Some(DEFAULT_DEADLINE_DAYS),
        }
    }
}

impl Config {
    /// Set timezone on Config struct only
    pub fn with_timezone(self: &Config, timezone: &str) -> Config {
        Config {
            timezone: Some(timezone.into()),
            ..self.clone()
        }
    }
    /// Set token on Config struct only
    pub fn with_token(self: &Config, token: &str) -> Config {
        Config {
            token: Some(token.into()),
            ..self.clone()
        }
    }

    /// Creates the blank config file by touching it and saving defaults
    pub async fn create(self) -> Result<Config, Error> {
        self.touch_file().await?;
        let mut config = self;
        // Save the config to disk
        config.save().await?;
        println!(
            "No config found. New config successfully created at {}",
            config.path.display()
        );
        Ok(config)
    }
    /// Ensures the parent directory exists and touches the config file.
    pub async fn touch_file(&self) -> Result<(), Error> {
        if let Some(parent) = std::path::Path::new(&self.path).parent() {
            fs::create_dir_all(parent).await?;
        }
        fs::File::create(&self.path).await?;
        Ok(())
    }

    /// Writes the config's current contents to disk as JSON.
    pub async fn save(&mut self) -> std::result::Result<String, Error> {
        let config = match Config::load(&self.path).await {
            Ok(Config { verbose, .. }) => Config {
                verbose,
                ..self.clone()
            },
            _ => self.clone(),
        };

        let json = json!(config);
        let string = serde_json::to_string_pretty(&json)?;
        fs::OpenOptions::new()
            .write(true)
            .read(true)
            .truncate(true)
            .open(&self.path)
            .await?
            .write_all(string.as_bytes())
            .await?;

        Ok(color::green_string("✓"))
    }

    /// Converts legacy projects to the new projects if necessary
    pub async fn projects(self: &Config) -> Result<Vec<Project>, Error> {
        let projects = self.projects.clone().unwrap_or_default();
        let legacy_projects = self.legacy_projects.clone().unwrap_or_default();

        if !projects.is_empty() {
            Ok(projects)
        } else if legacy_projects.is_empty() {
            Ok(Vec::new())
        } else {
            let new_projects = todoist::all_projects(self, None).await?;
            let legacy_ids = legacy_projects.into_iter().map(|lp| lp.id).collect();
            let v1_ids = todoist::get_v1_ids(self, Resource::Project, legacy_ids).await?;

            let new_projects: Vec<Project> = new_projects
                .iter()
                .filter(|p| v1_ids.contains(&p.id))
                .map(|p| p.to_owned())
                .collect();

            let mut config = self.clone();
            for project in &new_projects {
                config.add_project(project.clone());
                config.save().await?;
            }
            Ok(new_projects)
        }
    }
    // Returns the maximum comment length if configured, otherwise estimates based on terminal window size (if supported)
    pub fn max_comment_length(&self) -> u32 {
        match self.max_comment_length {
            Some(length) => length,
            None => {
                if let Some((Width(width), Height(height))) = terminal_size() {
                    let menu_height = page_size() as u16;
                    let comment_rows = height.saturating_sub(menu_height);
                    let estimated = comment_rows as u32 * width as u32;
                    estimated.min(MAX_COMMENT_LENGTH)
                } else {
                    MAX_COMMENT_LENGTH
                }
            }
        }
    }

    pub async fn reload_projects(self: &mut Config) -> Result<String, Error> {
        let all_projects = todoist::all_projects(self, None).await?;
        let current_projects = self.projects.clone().unwrap_or_default();
        let current_project_ids: Vec<String> =
            current_projects.iter().map(|p| p.id.to_owned()).collect();

        let updated_projects = all_projects
            .iter()
            .filter(|p| current_project_ids.contains(&p.id))
            .map(|p| p.to_owned())
            .collect::<Vec<Project>>();

        self.projects = Some(updated_projects);

        Ok(color::green_string("✓"))
    }

    /// Fetches a sender for the error channel
    /// Use this to end errors from an async process
    pub fn tx(self) -> UnboundedSender<Error> {
        self.internal.tx.expect("No tx in Config")
    }

    pub async fn check_for_latest_version(self: Config) -> Result<(), Error> {
        let last_version = self.clone().last_version_check;
        let new_config = Config {
            last_version_check: Some(time::date_string_today(&self)?),
            ..self.clone()
        };

        if last_version != Some(time::date_string_today(&self)?) {
            match cargo::compare_versions(None).await {
                Ok(Version::Dated(version)) => {
                    let message = format!(
                        "Your version of Tod is out of date
                        Latest Tod version is {}, you have {} installed.
                        Run {} to update if you installed with Cargo
                        or run {} if you installed with Homebrew",
                        version,
                        VERSION,
                        color::cyan_string("cargo install tod --force"),
                        color::cyan_string("brew update && brew upgrade tod")
                    );
                    self.tx().send(Error {
                        message,
                        source: "Crates.io".into(),
                    })?;
                    new_config.clone().save().await?;
                }
                Ok(Version::Latest) => (),
                Err(err) => self.tx().send(err)?,
            };
        };

        Ok(())
    }

    // Get timezone from config, or API if necessary
    pub fn get_timezone(&self) -> Result<String, Error> {
        self.timezone.clone().ok_or_else(|| Error {
            message: "Must set timezone".to_string(),
            source: "get_timezone".to_string(),
        })
    }

    /// Prompt user for timezone if it does not exist and write to disk
    pub async fn maybe_set_timezone(self) -> Result<Config, Error> {
        if self.timezone.is_none() {
            self.set_timezone().await
        } else {
            Ok(self)
        }
    }

    /// Set timezone and save to disk
    pub async fn set_timezone(self) -> Result<Config, Error> {
        let user = todoist::get_user_data(&self).await?;
        let mut config = self.with_timezone(&user.tz_info.timezone);
        config.save().await?;

        Ok(config)
    }

    pub fn clear_next_task(self) -> Config {
        let next_task: Option<Task> = None;

        Config { next_task, ..self }
    }

    /// Increase the completed count for today
    pub fn increment_completed(&self) -> Result<Config, Error> {
        let date = time::naive_date_today(self)?.to_string();
        let completed = match &self.completed {
            None => Some(Completed { date, count: 1 }),
            Some(completed) => {
                if completed.date == date {
                    Some(Completed {
                        count: completed.count + 1,
                        ..completed.clone()
                    })
                } else {
                    Some(Completed { date, count: 1 })
                }
            }
        };

        Ok(Config {
            completed,
            ..self.clone()
        })
    }

    pub async fn load(path: &PathBuf) -> Result<Config, Error> {
        let mut json = String::new();
        fs::File::open(path)
            .await?
            .read_to_string(&mut json)
            .await?;

        let config: Config = serde_json::from_str(&json).map_err(|e| config_load_error(e, path))?;
        let config = if config.sort_value.is_none() {
            Config {
                sort_value: Some(SortValue::default()),
                ..config
            }
        } else {
            config
        };

        Ok(config)
    }

    pub async fn new(tx: Option<UnboundedSender<Error>>, path: PathBuf) -> Result<Config, Error> {
        Ok(Config {
            path,
            token: None,
            next_id: None,
            next_task: None,
            last_version_check: None,
            timeout: None,
            bell_on_success: false,
            bell_on_failure: true,
            sort_value: Some(SortValue::default()),
            timezone: None,
            completed: None,
            disable_links: false,
            spinners: Some(true),
            mock_url: None,
            no_sections: None,
            natural_language_only: None,
            mock_string: None,
            mock_select: None,
            max_comment_length: None,
            comment_exclude_regex: None,
            task_exclude_regex: None,
            verbose: None,
            internal: Internal { tx },
            args: Args {
                verbose: false,
                timeout: None,
            },
            legacy_projects: Some(Vec::new()),
            time_provider: TimeProviderEnum::System(SystemTimeProvider),
            task_comment_command: None,
            task_create_command: None,
            task_complete_command: None,
            projects: Some(Vec::new()),
        })
    }

    pub async fn reload(&self) -> Result<Self, Error> {
        Config::load(&self.path).await.map(|config| Config {
            internal: self.internal.clone(),
            time_provider: self.time_provider.clone(),
            ..config
        })
    }

    pub fn add_project(&mut self, project: Project) {
        let option_projects = &mut self.projects;
        match option_projects {
            Some(projects) => {
                projects.push(project);
            }
            None => self.projects = Some(vec![project]),
        }
    }

    pub fn remove_project(&mut self, project: &Project) {
        let projects = self
            .projects
            .clone()
            .unwrap_or_default()
            .iter()
            .filter(|p| p.id != project.id)
            .map(|p| p.to_owned())
            .collect::<Vec<Project>>();

        self.projects = Some(projects);
    }

    pub fn set_next_task(&self, task: Task) -> Config {
        let next_task: Option<Task> = Some(task);

        Config {
            next_task,
            ..self.clone()
        }
    }

    pub fn tasks_completed(&self) -> Result<u32, Error> {
        let date = time::naive_date_today(self)?.to_string();
        match &self.completed {
            None => Ok(0),
            Some(completed) => {
                if completed.date == date {
                    Ok(completed.count)
                } else {
                    Ok(0)
                }
            }
        }
    }

    pub fn next_task(&self) -> Option<Task> {
        self.next_task.clone()
    }

    pub(crate) fn deadline_days(&self) -> u8 {
        self.sort_value
            .clone()
            .unwrap_or_default()
            .deadline_days
            .unwrap_or(DEFAULT_DEADLINE_DAYS)
    }

    pub(crate) fn deadline_value(&self) -> u8 {
        self.sort_value
            .clone()
            .unwrap_or_default()
            .deadline_value
            .unwrap_or(DEFAULT_DEADLINE_VALUE)
    }

    pub async fn set_token(&mut self, access_token: String) -> Result<String, Error> {
        self.token = Some(access_token);
        self.save().await
    }

    async fn maybe_set_token(self) -> Result<Config, Error> {
        if self.token.clone().unwrap_or_default().trim().is_empty() {
            let mock_select = Some(1);
            let options = vec![OAUTH, DEVELOPER];
            let mut config = match input::select(TOKEN_METHOD, options, mock_select)? {
                OAUTH => {
                    let mut config = self.clone();
                    oauth::login(&mut config, None).await?;
                    config
                }
                DEVELOPER => {
                    let url = maybe_format_url("https://todoist.com/prefs/integrations", &self);
                    let desc = format!("Please enter your Todoist API token from {url} ");

                    // We can't use mock_string from config here because it can't be set in test.
                    let fake_token = Some("faketoken".into());
                    let token = input::string(&desc, fake_token)?;
                    self.with_token(&token)
                }
                _ => unreachable!(),
            };
            config.save().await?;
            Ok(config)
        } else {
            Ok(self)
        }
    }
}

fn config_load_error(error: serde_json::Error, path: &Path) -> Error {
    let source = "serde_json";
    let message = format!(
        "\n{}",
        color::red_string(&format!(
            "Error loading configuration file '{}':\n{error}\n\
            \nThe file contains an invalid value.\n\
            Update the value or run 'tod config reset' to delete (reset) the config.",
            path.display()
        ))
    );

    Error::new(source, &message)
}

impl Default for Config {
    fn default() -> Self {
        Config {
            token: None,
            path: PathBuf::new(),
            next_id: None,
            next_task: None,
            last_version_check: None,
            timeout: None,
            bell_on_success: false,
            bell_on_failure: true,
            task_create_command: None,
            task_complete_command: None,
            task_comment_command: None,
            task_exclude_regex: None,
            comment_exclude_regex: None,
            sort_value: Some(SortValue::default()),
            timezone: None,
            completed: None,
            disable_links: false,
            spinners: Some(true),
            mock_url: None,
            no_sections: None,
            natural_language_only: None,
            mock_string: None,
            mock_select: None,
            max_comment_length: None,
            verbose: None,
            internal: Internal { tx: None },
            args: Args {
                verbose: false,
                timeout: None,
            },
            legacy_projects: Some(Vec::new()),
            time_provider: TimeProviderEnum::System(SystemTimeProvider),
            projects: Some(Vec::new()),
        }
    }
}
/// Fetches the config from disk; errors out if it doesn't exist
pub async fn get_config(config_path: Option<PathBuf>) -> Result<Config, Error> {
    let path = match config_path {
        None => generate_path().await?,
        Some(path) => maybe_expand_home_dir(path)?,
    };

    let path_for_error = path.clone();
    if !check_config_exists(Some(path)).await? {
        return Err(Error::new(
            "get_config",
            &format!("No config file found at {}.", path_for_error.display()),
        ));
    }

    Config::load(&path_for_error).await
}

/// Checks if the config file exists at the given path OR  default path if None).
pub async fn check_config_exists(config_path: Option<PathBuf>) -> Result<bool, Error> {
    let path = resolve_config_path(config_path).await?;
    Ok(path.exists())
}

/// Resolves the config path, either using the provided path or generating a default one.
async fn resolve_config_path(config_path: Option<PathBuf>) -> Result<PathBuf, Error> {
    match config_path {
        None => generate_path().await,
        Some(path) => maybe_expand_home_dir(path),
    }
}
/// Fetches config from from disk and creates it if it doesn't exist
/// Prompts for Todoist API token
pub async fn get_or_create(
    config_path: Option<PathBuf>,
    verbose: bool,
    timeout: Option<u64>,
    tx: &UnboundedSender<Error>,
) -> Result<Config, Error> {
    let path = match config_path {
        None => generate_path().await?,
        Some(path) => maybe_expand_home_dir(path)?,
    };

    let config = match fs::File::open(&path).await {
        Ok(_) => Config::load(&path).await,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            debug::print("Config file not found, creating new config");
            create_config(tx, path).await
        }
        Err(err) => Err(Error::new(
            "config.rs",
            &format!("Failed to open config file: {err}"),
        )),
    }?;

    let mut config = Config {
        args: Args { timeout, verbose },
        internal: Internal {
            tx: Some(tx.clone()),
        },
        ..config
    };

    debug::maybe_print_redacted_config(&mut config);
    Ok(config)
}
//create the config file with settings
pub async fn create_config(
    tx: &UnboundedSender<Error>,
    config_path: PathBuf,
) -> Result<Config, Error> {
    // Create the default in-memory config
    let mut config = Config::new(Some(tx.clone()), config_path).await?;
    // Create the empty file
    config = config.create().await?;

    // Populate the required fields - prompt for token or use existing token logic
    config = config.maybe_set_token().await?;

    // Populate the required timezone
    config = config.maybe_set_timezone().await?;

    // write updated config to disk
    config.save().await?;

    Ok(config)
}
pub async fn generate_path() -> Result<PathBuf, Error> {
    if cfg!(test) {
        let random_string = Alphanumeric.sample_string(&mut rand::rng(), 100);
        Ok(PathBuf::from(format!("tests/{random_string}.testcfg")))
    } else {
        let config_directory = dirs::config_dir().expect("Could not find config directory");
        Ok(config_directory.join("tod.cfg"))
    }
}

fn maybe_expand_home_dir(path: PathBuf) -> Result<PathBuf, Error> {
    // If the path starts with "~", expand it
    if let Some(str_path) = path.to_str()
        && str_path.starts_with('~')
    {
        let home =
            homedir::my_home()?.ok_or_else(|| Error::new("homedir", "Could not get homedir"))?;

        // Strip the "~" and construct the new path
        let mut expanded = home;
        let suffix = str_path.trim_start_matches('~').trim_start_matches('/');
        expanded.push(suffix);

        return Ok(expanded);
    }

    Ok(path)
}

/// Deletes the config file after resolving its path and confirming with the user.
pub async fn config_reset(cli_config_path: Option<PathBuf>, force: bool) -> Result<String, Error> {
    // Defer to the testable version, but use `inquire::Confirm` for interactive input to pass true/false.
    config_reset_with_prompt(cli_config_path.clone(), force, || {
        let path_display = cli_config_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "<default path>".into());

        Confirm::new(&format!(
            "Are you sure you want to delete the config at {path_display}?"
        ))
        .with_default(false)
        .prompt()
        .unwrap_or(false)
    })
    .await
}
// Full config reset function, but accepts inputs for CI testing

pub async fn config_reset_with_prompt<F>(
    cli_config_path: Option<PathBuf>,
    force: bool,
    prompt_fn: F,
) -> Result<String, Error>
where
    F: FnOnce() -> bool,
{
    let path = match cli_config_path {
        Some(p) => maybe_expand_home_dir(p)?,
        None => generate_path().await?,
    };

    if !path.exists() {
        return Ok(format!("No config file found at {}.", path.display()));
    }

    if !force && !prompt_fn() {
        return Ok("Aborted: Config not deleted.".to_string());
    }

    match fs::remove_file(&path).await {
        Ok(()) => Ok(format!(
            "Config file at {} deleted successfully.",
            path.display()
        )),
        Err(e) => Err(Error::new(
            "config_reset",
            &format!("Could not delete config file at {}: {}", path.display(), e),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test;
    use pretty_assertions::assert_eq;
    use std::env::temp_dir;
    use std::fs::File;
    use std::path::Path;
    use std::path::PathBuf;

    impl Config {
        pub fn default_test() -> Self {
            Config {
                token: Some("default-token".to_string()),
                path: PathBuf::from("/tmp/test.cfg"),
                time_provider: TimeProviderEnum::Fixed(FixedTimeProvider),
                args: Args {
                    verbose: false,
                    timeout: None,
                },
                internal: Internal { tx: None },
                sort_value: Some(SortValue::default()),
                projects: Some(vec![]),
                legacy_projects: Some(vec![]),
                next_id: None,
                next_task: None,
                bell_on_success: false,
                bell_on_failure: true,
                task_create_command: None,
                task_complete_command: None,
                task_comment_command: None,
                task_exclude_regex: None,
                comment_exclude_regex: None,
                timezone: Some("UTC".to_string()),
                timeout: None,
                last_version_check: None,
                mock_url: None,
                mock_string: None,
                mock_select: None,
                spinners: None,
                disable_links: false,
                completed: None,
                max_comment_length: None,
                verbose: None,
                no_sections: None,
                natural_language_only: None,
            }
        }
        // Mock the url used for fetching projects and tasks
        pub fn with_mock_url(self, url: String) -> Config {
            Config {
                mock_url: Some(url),
                ..self
            }
        }
        // Mock the string returned by the mock url
        pub fn with_mock_string(self, string: &str) -> Config {
            Config {
                mock_string: Some(string.to_string()),
                ..self
            }
        }

        pub fn mock_select(self, index: usize) -> Config {
            Config {
                mock_select: Some(index),
                ..self
            }
        }

        pub fn with_path(self: &Config, path: PathBuf) -> Config {
            Config {
                path,
                ..self.clone()
            }
        }

        pub fn with_projects(self: &Config, projects: Vec<Project>) -> Config {
            Config {
                projects: Some(projects),
                ..self.clone()
            }
        }
        /// Set the TimeProvider for testing
        pub fn with_time_provider(self: &Config, provider_type: TimeProviderEnum) -> Config {
            let mut config = self.clone();
            config.time_provider = provider_type;
            config
        }
    }

    async fn config_with_mock(mock_url: impl Into<String>) -> Config {
        test::fixtures::config()
            .await
            .with_mock_url(mock_url.into())
    }

    async fn config_with_mock_and_token(
        mock_url: impl Into<String>,
        token: impl Into<String>,
    ) -> Config {
        test::fixtures::config()
            .await
            .with_mock_url(mock_url.into())
            .with_token(&token.into())
    }

    fn tx() -> UnboundedSender<Error> {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        tx
    }

    #[tokio::test]
    async fn config_tests() {
        let server = mockito::Server::new_async().await;
        let mock_url = server.url();

        let config_create = config_with_mock_and_token(&mock_url, "created").await;
        let path_created = config_create.path.clone();
        config_create
            .create()
            .await
            .expect("Failed to create config in async call");

        let loaded = Config::load(&path_created)
            .await
            .expect("Failed to load config from path asynchronously");
        assert_eq!(loaded.token, Some("created".into()));
        delete_config(&path_created).await;

        let config_create = config_with_mock(&mock_url).await;
        let path_create = config_create.path.clone();
        config_create
            .create()
            .await
            .expect("Failed to create config in async call");

        let created = get_or_create(Some(path_create.clone()), false, None, &tx())
            .await
            .expect("get_or_create (create) failed");
        assert!(created.token.is_some());
        delete_config(&created.path).await;

        let config_load = config_with_mock_and_token(&mock_url, "loaded").await;
        let path_load = config_load.path.clone();
        config_load
            .create()
            .await
            .expect("Failed to create config load asynchronously");

        let loaded = get_or_create(Some(path_load.clone()), false, None, &tx())
            .await
            .expect("get_or_create (load) failed");
        assert_eq!(loaded.token, Some("loaded".into()));
        assert!(loaded.internal.tx.is_some());

        let fetched = get_or_create(Some(path_load.clone()), false, None, &tx()).await;
        assert_matches!(fetched, Ok(Config { .. }));
        delete_config(&path_load).await;
    }

    async fn delete_config(path: &PathBuf) {
        assert_matches!(fs::remove_file(path).await, Ok(_));
    }

    #[tokio::test]
    async fn new_should_generate_config() {
        let config = Config::new(
            None,
            generate_path().await.expect("Could not create config"),
        )
        .await
        .expect("Could not create config");
        assert_eq!(config.token, None);
    }

    #[tokio::test]
    async fn reload_config_should_work() {
        let config = test::fixtures::config().await;
        let mut config = config.create().await.expect("Failed to create test config");
        let project = test::fixtures::project();
        config.add_project(project);
        let projects = config
            .projects()
            .await
            .expect("Failed to fetch projects asynchronously");
        assert!(!&projects.is_empty());

        config.reload().await.expect("Failed to reload config");
    }

    #[tokio::test]
    async fn set_and_clear_next_task_should_work() {
        let config = test::fixtures::config().await;
        assert_eq!(config.next_task, None);
        let task = test::fixtures::today_task().await;
        let config = config.set_next_task(task.clone());
        assert_eq!(config.next_task, Some(task));
        let config = config.clear_next_task();
        assert_eq!(config.next_task, None);
    }

    #[tokio::test]
    async fn add_project_should_work() {
        let mut config = test::fixtures::config().await;
        let projects_count = config
            .projects()
            .await
            .expect("Failed to fetch projects asynchronously")
            .len();
        assert_eq!(projects_count, 1);
        config.add_project(test::fixtures::project());
        let projects_count = config
            .projects()
            .await
            .expect("Failed to fetch projects asynchronously")
            .len();
        assert_eq!(projects_count, 2);
    }

    #[tokio::test]
    async fn remove_project_should_work() {
        let mut config = test::fixtures::config().await;
        let projects = config
            .projects()
            .await
            .expect("Failed to fetch projects asynchronously");
        let project = projects
            .first()
            .expect("Expected at least one project in projects list");
        let projects_count = config
            .projects()
            .await
            .expect("Failed to fetch projects asynchronously")
            .len();
        assert_eq!(projects_count, 1);
        config.remove_project(project);
        let projects_count = config
            .projects()
            .await
            .expect("Failed to fetch projects asynchronously")
            .len();
        assert_eq!(projects_count, 0);
    }

    #[test]
    fn test_maybe_expand_home_dir() {
        // No tilde, so path should remain unchanged
        let input = PathBuf::from("/Users/tod.cfg");
        let result = maybe_expand_home_dir(input.clone()).expect("Could not create PathBuf");

        assert_eq!(result, input);
    }

    #[tokio::test]
    async fn load_should_fail_on_invalid_u8_value() {
        use tokio::fs::write;

        let bad_config_path = "tests/bad_config_invalid_u8.cfg";
        let contents = r#"{
        "token": "abc123",
        "path": "tests/bad_config_invalid_u8.cfg",
        "sort_value": {
            "priority_none": 500
        }
    }"#;

        write(bad_config_path, contents)
            .await
            .expect("Could not write to file");

        let bad_config_path_buf = std::path::PathBuf::from(bad_config_path);
        let result = Config::load(&bad_config_path_buf).await;
        assert!(result.is_err(), "Expected error from invalid u8");

        fs::remove_file(bad_config_path)
            .await
            .expect("Could not remove file");
    }

    #[tokio::test]
    async fn debug_impl_for_config_should_work() {
        let config = test::fixtures::config().await;
        let debug_output = format!("{config:?}");
        // Assert that the debug output contains the struct name and some fields
        assert!(debug_output.contains("Config"));
        assert!(debug_output.contains("token"));
        assert!(debug_output.contains(&config.token.expect("No token found in config")));
    }

    #[test]
    fn debug_impls_for_config_components_should_work() {
        use tokio::sync::mpsc::unbounded_channel;

        let args = Args {
            verbose: true,
            timeout: Some(42),
        };
        let args_debug = format!("{args:?}");
        assert!(args_debug.contains("Args"));
        assert!(args_debug.contains("verbose"));
        assert!(args_debug.contains("timeout"));

        let (tx, _rx) = unbounded_channel::<Error>();
        let internal = Internal { tx: Some(tx) };
        let internal_debug = format!("{internal:?}");
        assert!(internal_debug.contains("Internal"));

        let sort_value = SortValue::default();
        let sort_value_debug = format!("{sort_value:?}");
        assert!(sort_value_debug.contains("SortValue"));
        assert!(sort_value_debug.contains("priority_none"));
        assert!(sort_value_debug.contains("deadline_value"));
    }

    #[test]
    fn trait_impls_for_config_components_should_work() {
        let args = Args {
            verbose: true,
            timeout: Some(10),
        };
        let args_clone = args.clone();
        assert_eq!(args, args_clone);

        let internal = Internal { tx: None };
        let internal_clone = internal.clone();
        assert_eq!(internal.tx.is_none(), internal_clone.tx.is_none());

        let sort_value = SortValue::default();
        let sort_value_clone = sort_value.clone();
        assert_eq!(sort_value, sort_value_clone);

        assert_eq!(
            args,
            Args {
                verbose: true,
                timeout: Some(10)
            }
        );
        assert_ne!(
            args,
            Args {
                verbose: false,
                timeout: Some(5)
            }
        );

        let default_args = Args::default();
        assert_eq!(default_args.verbose, false);
        assert_eq!(default_args.timeout, None);

        let default_internal = Internal::default();
        assert!(default_internal.tx.is_none());

        let default_sort = SortValue::default();
        assert_eq!(default_sort.priority_none, 2);
        assert_eq!(default_sort.deadline_value, Some(DEFAULT_DEADLINE_VALUE));
    }

    #[tokio::test]
    async fn test_config_with_methods() {
        let path = generate_path().await.expect("Could not generate path");
        let base_config = Config::new(None, path)
            .await
            .expect("Failed to create base config");

        let tz_config = base_config.with_timezone("America/New_York");
        assert_eq!(tz_config.timezone, Some("America/New_York".to_string()));

        let mock_url = "http://localhost:1234";
        let mock_config = base_config.clone().with_mock_url(mock_url.to_string());
        assert_eq!(mock_config.mock_url, Some(mock_url.to_string()));

        let mock_str_config = base_config.clone().with_mock_string("mock response");
        assert_eq!(
            mock_str_config.mock_string,
            Some("mock response".to_string())
        );

        let select_config = base_config.clone().mock_select(2);
        assert_eq!(select_config.mock_select, Some(2));

        let path_config = base_config.with_path(PathBuf::from("some/test/path.cfg"));
        assert_eq!(path_config.path, PathBuf::from("some/test/path.cfg"));

        let projects = vec![Project {
            id: "test123".to_string(),
            can_assign_tasks: true,
            child_order: 0,
            color: "blue".to_string(),
            created_at: None,
            is_archived: false,
            is_deleted: false,
            is_favorite: false,
            is_frozen: false,
            name: "Test Project".to_string(),
            updated_at: None,
            view_style: "list".to_string(),
            default_order: 0,
            description: "desc".to_string(),
            parent_id: None,
            inbox_project: None,
            is_collapsed: false,
            is_shared: false,
        }];
        let project_config = Config {
            projects: Some(projects.clone()),
            ..base_config.clone()
        };
        assert!(project_config.projects.is_some());
    }

    #[test]
    fn test_config_debug_with_time_provider() {
        let config = Config::default_test()
            .with_time_provider(TimeProviderEnum::Fixed(FixedTimeProvider))
            .with_path(PathBuf::from("/tmp/test.cfg"));

        let debug_output = format!("{config:?}");
        assert!(debug_output.contains("Config"));
        assert!(debug_output.contains("/tmp/test.cfg"));
    }
    // Test function for max_comment_length
    #[test]
    fn max_comment_length_should_return_configured_value() {
        let config = Config {
            max_comment_length: Some(1234),
            ..Config::default_test()
        };

        assert_eq!(config.max_comment_length(), 1234);
    }

    #[test]
    fn max_comment_length_should_fallback_when_not_set() {
        let config = Config {
            max_comment_length: None,
            ..Config::default_test()
        };

        let result = config.max_comment_length();

        // In CI or test environments terminal_size might return None
        // so just ensure it's a positive, nonzero value
        assert!(result > 0);
        assert!(result <= MAX_COMMENT_LENGTH);
    }
    #[test]
    fn test_unknown_field_rejected() {
        let json = r#"
    {
        "token": "abc123",
        "Bobby": {
            "bobby_enabled": true
        }
    }
    "#;

        let result: Result<Config, _> = serde_json::from_str(json);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("unknown field `Bobby`"));
    }
    #[tokio::test]
    async fn test_create_config_with_custom_path() {
        let path = PathBuf::from("/tmp/custom_path");
        let mut config = Config {
            path,
            ..Config::default_test()
        };
        config = config.create().await.expect("Should create file");
        config.save().await.expect("Should save file");

        // Check that required fields are populated
        assert!(config.token.is_some(), "Token should be set");
        assert!(config.timezone.is_some(), "Timezone should be set");

        // Check that the file exists
        assert!(
            tokio::fs::try_exists(&config.path)
                .await
                .expect("Could not determine if file exists"),
            "Config file should exist at {}",
            config.path.display()
        );
    }

    #[tokio::test]
    async fn test_create_config_saves_file() {
        let mut config = Config::default_test();
        config = config.create().await.expect("Should create file");
        config.save().await.expect("Should save file");

        // Check that required fields are populated
        assert!(config.token.is_some(), "Token should be set");
        assert!(config.timezone.is_some(), "Timezone should be set");

        // Check that the file exists
        assert!(
            tokio::fs::try_exists(&config.path)
                .await
                .expect("Could not determine if file exists"),
            "Config file should exist at {}",
            config.path.display()
        );
    }

    #[tokio::test]
    async fn test_generate_path_in_test_mode() {
        let path = generate_path().await.expect("Should return a test path");

        // Check that the parent is "tests"
        assert!(
            path.parent().map(|p| p.ends_with("tests")).unwrap_or(false),
            "Expected path to be in the 'tests/' directory, got {}",
            path.display()
        );

        // Check that the file extension is ".testcfg"
        assert!(
            path.extension()
                .map(|ext| ext == "testcfg")
                .unwrap_or(false),
            "Expected file extension to be .testcfg, got {}",
            path.display()
        );
    }
    #[tokio::test]
    async fn test_load_config_rejects_invalid_regex() {
        // Use test fixture to get temp config path
        let config = test::fixtures::config().await;
        let path = &config.path;

        // Write the invalid regex string "[a-z" to the config file which should cause serde_json to fail
        let invalid_json = r#"
    {
        "token": "abc123",
        "timezone": "UTC",
        "task_exclude_regex": "[a-z"
    }
    "#;

        tokio::fs::write(path, invalid_json)
            .await
            .expect("Failed to write invalid config");

        let result = Config::load(path).await;

        assert!(
            result.is_err(),
            "Expected load to fail due to invalid regex"
        );
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("Error loading configuration file"),
            "Expected 'Error loading configuration file' in error message:\n{msg}"
        );

        assert!(
            msg.contains("regex parse error"),
            "Expected 'regex parse error' in error message:\n{msg}"
        );
    }

    #[tokio::test]
    async fn test_create_config_populates_token_and_timezone() {
        // Manually set token and timezone and ensure they're saved
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let path = generate_path().await.expect("Could not generate path");
        let mut config = Config::new(Some(tx.clone()), path)
            .await
            .expect("Init default config");

        config.token = Some("test-token-123".into());
        config.timezone = Some("UTC".into());
        config = config.create().await.expect("Should create file");
        config.save().await.expect("Should save config");

        // Reload from disk and validate contents
        let contents = tokio::fs::read_to_string(&config.path)
            .await
            .expect("File should exist");
        assert!(
            contents.contains("test-token-123"),
            "Saved config should contain token"
        );
        assert!(
            contents.contains("UTC"),
            "Saved config should contain timezone"
        );
    }

    #[tokio::test]
    async fn test_config_reset_force_deletes_temp_file() {
        let mut temp_path: PathBuf = temp_dir();
        temp_path.push("temp_test_config.cfg");

        File::create(&temp_path).expect("Failed to create temp config file");
        assert!(temp_path.exists(), "Temp config should exist before reset");

        let result = crate::config::config_reset(Some(temp_path.clone()), true).await;
        assert!(result.is_ok(), "Expected Ok, got {result:?}");

        assert!(!Path::new(&temp_path).exists(), "File should be deleted");
    }

    #[test]
    fn test_maybe_expand_home_dir_expands_tilde() {
        let input = PathBuf::from("~/myfolder/mysubfile.txt");
        let expanded = maybe_expand_home_dir(input).expect("Could not expand home dir");

        let expected_prefix = homedir::my_home()
            .expect("Could not find home path")
            .expect("No home path found");
        assert!(expanded.starts_with(&expected_prefix));
        assert!(expanded.ends_with("myfolder/mysubfile.txt"));
    }
    #[test]
    fn test_maybe_expand_home_dir_non_tilde_unchanged() {
        let input = PathBuf::from("/usr/bin/env");
        let result = maybe_expand_home_dir(input.clone()).expect("Could not expand home dir");
        assert_eq!(result, input);
    }

    #[tokio::test]
    async fn test_get_config_with_existing_path() {
        // Create a temp config file
        let dir = temp_dir();
        let temp_path: PathBuf = dir.join("test_get_config_exists.cfg");
        let mut config = Config {
            path: temp_path.clone(),
            token: Some("abc".to_string()),
            timezone: Some("UTC".to_string()),
            ..Config::default()
        };
        // Ensure parent directory exists and file is created
        config = config.create().await.expect("Should create config file");
        config.save().await.expect("Should save config");

        // Should load successfully
        let loaded = get_config(Some(temp_path.clone())).await;
        assert!(loaded.is_ok(), "Expected Ok for existing config");
        let loaded = loaded.expect("No config found");
        assert_eq!(loaded.token, Some("abc".to_string()));

        // Cleanup
        tokio::fs::remove_file(&temp_path).await.ok();
    }

    #[tokio::test]
    async fn test_get_config_with_nonexistent_path() {
        let dir = temp_dir();
        let temp_path: PathBuf = dir.join("test_get_config_nonexistent.cfg");
        // Ensure file does not exist
        tokio::fs::remove_file(&temp_path).await.ok();

        let loaded = get_config(Some(temp_path.clone())).await;
        assert!(loaded.is_err(), "Expected Err for nonexistent config");
        let err = loaded.unwrap_err().to_string();
        assert!(
            err.contains("No config file found"),
            "Expected missing config error, got: {err}"
        );
    }

    #[tokio::test]
    async fn test_check_config_exists_true_and_false() {
        let dir = temp_dir();
        let temp_path: PathBuf = dir.join("test_check_config_exists.cfg");
        // Should not exist yet
        tokio::fs::remove_file(&temp_path).await.ok();

        let exists = check_config_exists(Some(temp_path.clone()))
            .await
            .expect("Could not check if config exists");
        assert!(!exists, "Should be false for nonexistent config");

        tokio::fs::File::create(&temp_path)
            .await
            .expect("Could not create file");
        let exists = check_config_exists(Some(temp_path.clone()))
            .await
            .expect("Could not check if config exists");
        assert!(exists, "Should be true for existing config");

        tokio::fs::remove_file(&temp_path).await.ok();
    }
    #[tokio::test]
    async fn test_config_reset_with_prompt_yes_deletes_file() {
        let mut temp_path: PathBuf = temp_dir();
        temp_path.push("temp_test_config_prompt_yes.cfg");

        File::create(&temp_path).expect("Failed to create temp config file");
        assert!(temp_path.exists(), "Temp config should exist before reset");

        // Simulate user saying "yes"
        let result = config_reset_with_prompt(Some(temp_path.clone()), false, || true).await;

        assert!(result.is_ok(), "Expected Ok, got {result:?}");
        let msg = result.expect("Could not get msg");
        assert!(
            msg.contains("deleted successfully"),
            "Expected deletion message, got: {msg}"
        );
        assert!(
            !Path::new(&temp_path).exists(),
            "File should be deleted after reset"
        );
    }

    #[tokio::test]
    async fn test_config_reset_with_prompt_no_aborts() {
        let mut temp_path: PathBuf = temp_dir();
        temp_path.push("temp_test_config_prompt_no.cfg");

        File::create(&temp_path).expect("Failed to create temp config file");
        assert!(temp_path.exists(), "Temp config should exist before reset");

        // Simulate user saying "no"
        let result = config_reset_with_prompt(Some(temp_path.clone()), false, || false).await;

        assert!(result.is_ok(), "Expected Ok, got {result:?}");
        let msg = result.expect("Could not get reset config response");
        assert_eq!(msg, "Aborted: Config not deleted.");
        assert!(
            Path::new(&temp_path).exists(),
            "File should not be deleted after abort"
        );

        // Cleanup
        tokio::fs::remove_file(&temp_path).await.ok();
    }
}
