use crate::cargo::Version;
mod file;
mod projects;
mod timezone;
use crate::errors::Error;
use crate::format::maybe_format_url;
use crate::input::page_size;
use crate::legacy;
use crate::projects::Project;
use crate::tasks::Task;
use crate::time::{SystemTimeProvider, TimeProviderEnum};
use crate::{VERSION, cargo, format, input, oauth, time};
use regex::Regex;
use serde::de::Error as DeError;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::path::PathBuf;
use terminal_size::{Height, Width, terminal_size};
use tokio::sync::mpsc::UnboundedSender;

const MAX_COMMENT_LENGTH: u32 = 500;
pub const DEFAULT_TIMEOUT_SECONDS: u64 = 30;
pub const OAUTH: &str = "Login with OAuth (recommended)";
pub const DEVELOPER: &str = "Login with developer API token";
pub const TOKEN_METHOD: &str = "Choose your Todoist login method";
const TODOIST_INTEGRATIONS_URL: &str = "https://todoist.com/prefs/integrations";
pub use file::config_open;
pub use file::config_reset;
pub use file::generate_path;
pub use file::get_config;
pub use file::get_or_create;
pub use file::resolve_config_path;
pub use legacy::LegacySortValue;

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Completed {
    #[serde(deserialize_with = "deserialize_nonnegative_u32")]
    count: u32,
    date: String,
}

fn deserialize_nonnegative_u32<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: Deserializer<'de>,
{
    let value = i64::deserialize(deserializer)?;

    if value <= 0 {
        Ok(0)
    } else {
        u32::try_from(value).map_err(D::Error::custom)
    }
}

/// App configuration, serialized as json in `$XDG_CONFIG_HOME/tod.cfg`
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// The Todoist Api token
    pub token: Option<String>,
    /// List of Todoist projects and their project numbers
    #[serde(rename = "projectsv1")]
    projects: Option<Vec<Project>>,
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
    #[serde(default = "bell_on_failure_default")]
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
    /// Ordered list of fields used when sorting by value.
    pub sort_order: Option<Vec<SortRule>>,
    /// Legacy numeric sort configuration. Deserialized for migration only.
    #[serde(skip_serializing)]
    pub sort_value: Option<LegacySortValue>,

    /// For storing arguments from the commandline
    #[serde(skip)]
    pub args: Args,

    /// For storing arguments from the commandline
    #[serde(skip)]
    pub internal: Internal,
    /// Optional `TimeProvider` for testing, defaults to `SystemTimeProvider`
    #[serde(skip)]
    pub time_provider: TimeProviderEnum,
}

fn bell_on_failure_default() -> bool {
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

#[derive(Copy, Clone, Serialize, Deserialize, Eq, PartialEq, Debug)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum SortKey {
    Priority,
    DueDate,
    Overdue,
    Today,
    Now,
    NoDueDate,
    NotRecurring,
    Deadline,
    Order,
}

impl SortKey {
    pub fn default_order() -> Vec<SortKey> {
        vec![
            SortKey::Priority,
            SortKey::DueDate,
            SortKey::Overdue,
            SortKey::Today,
            SortKey::Now,
            SortKey::NoDueDate,
            SortKey::NotRecurring,
            SortKey::Deadline,
            SortKey::Order,
        ]
    }

    fn config_name(self) -> &'static str {
        match self {
            SortKey::Priority => "priority",
            SortKey::DueDate => "due_date",
            SortKey::Overdue => "overdue",
            SortKey::Today => "today",
            SortKey::Now => "now",
            SortKey::NoDueDate => "no_due_date",
            SortKey::NotRecurring => "not_recurring",
            SortKey::Deadline => "deadline",
            SortKey::Order => "order",
        }
    }

    fn from_config_name(value: &str) -> Option<Self> {
        match value {
            "priority" => Some(SortKey::Priority),
            "due_date" => Some(SortKey::DueDate),
            "overdue" => Some(SortKey::Overdue),
            "today" => Some(SortKey::Today),
            "now" => Some(SortKey::Now),
            "no_due_date" => Some(SortKey::NoDueDate),
            "not_recurring" => Some(SortKey::NotRecurring),
            "deadline" => Some(SortKey::Deadline),
            "order" => Some(SortKey::Order),
            _ => None,
        }
    }

    fn default_direction(self) -> SortDirection {
        match self {
            SortKey::Priority
            | SortKey::Overdue
            | SortKey::Today
            | SortKey::Now
            | SortKey::NoDueDate
            | SortKey::NotRecurring => SortDirection::Desc,
            SortKey::DueDate | SortKey::Deadline | SortKey::Order => SortDirection::Asc,
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum SortDirection {
    Asc,
    Desc,
}

impl SortDirection {
    fn config_name(self) -> &'static str {
        match self {
            SortDirection::Asc => "asc",
            SortDirection::Desc => "desc",
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct SortRule {
    pub key: SortKey,
    pub direction: SortDirection,
}

impl SortRule {
    pub fn new(key: SortKey, direction: SortDirection) -> Self {
        Self { key, direction }
    }

    pub(crate) fn with_default_direction(key: SortKey) -> Self {
        Self::new(key, key.default_direction())
    }

    pub fn default_order() -> Vec<Self> {
        SortKey::default_order()
            .into_iter()
            .map(Self::with_default_direction)
            .collect()
    }
}

impl Serialize for SortRule {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!(
            "{}:{}",
            self.key.config_name(),
            self.direction.config_name()
        ))
    }
}

impl<'de> Deserialize<'de> for SortRule {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        let (key, direction) = match value.split_once(':') {
            Some((key, "asc")) => (key, Some(SortDirection::Asc)),
            Some((key, "desc")) => (key, Some(SortDirection::Desc)),
            Some((_, direction)) => {
                return Err(D::Error::custom(format!(
                    "invalid sort direction '{direction}'; expected 'asc' or 'desc'"
                )));
            }
            None => (value.as_str(), None),
        };
        let key = SortKey::from_config_name(key)
            .ok_or_else(|| D::Error::custom(format!("invalid sort key '{key}'")))?;

        Ok(match direction {
            Some(direction) => SortRule::new(key, direction),
            None => SortRule::with_default_direction(key),
        })
    }
}

impl Config {
    fn with_default_sort_order(self) -> Config {
        if self.sort_order.is_some() {
            self
        } else if let Some(sort_order) =
            legacy::detect_and_migrate_sort_value(self.sort_value.as_ref())
        {
            Config {
                sort_order: Some(sort_order),
                ..self
            }
        } else {
            Config {
                sort_order: Some(SortRule::default_order()),
                ..self
            }
        }
    }

    /// Pretty printed message for how to get API token
    pub fn token_message(self: &Config) -> String {
        let url = maybe_format_url(TODOIST_INTEGRATIONS_URL, self);
        format!("Please enter your Todoist API token from {url} ")
    }

    /// Set token on Config struct only
    #[allow(dead_code)]
    pub fn with_token(self: &Config, token: &str) -> Config {
        Config {
            token: Some(token.into()),
            ..self.clone()
        }
    }

    // Returns the maximum comment length if configured, otherwise estimates based on terminal window size (if supported)
    pub fn max_comment_length(&self) -> u32 {
        match self.max_comment_length {
            Some(length) => length,
            None => {
                if let Some((Width(width), Height(height))) = terminal_size() {
                    let menu_height = u16::try_from(page_size()).unwrap_or(height);
                    let comment_rows = height.saturating_sub(menu_height);
                    let estimated = u32::from(comment_rows) * u32::from(width);
                    estimated.min(MAX_COMMENT_LENGTH)
                } else {
                    MAX_COMMENT_LENGTH
                }
            }
        }
    }

    /// Fetches a sender for the error channel
    /// Use this to end errors from an async process
    pub fn tx(self) -> UnboundedSender<Error> {
        self.internal.tx.expect("No tx in Config")
    }

    pub async fn check_for_latest_version(self: Config) -> Result<Config, Error> {
        let last_version = self.last_version_check.clone();
        let today = time::date_string_today(&self)?;

        if last_version != Some(today.clone()) {
            let mut new_config = Config {
                last_version_check: Some(today),
                ..self.clone()
            };

            new_config.save().await?;
            let cloned_config = new_config.clone();
            tokio::spawn(async move {
                match cargo::compare_versions(cloned_config.mock_url).await {
                    Ok(Version::Dated(version)) => {
                        let message = format!(
                            "Your version of Tod is out of date
                        Latest Tod version is {}, you have {} installed.
                        Run {} to update if you installed with Cargo
                        or run {} if you installed with Homebrew",
                            version,
                            VERSION,
                            format::cyan_string("cargo install tod --force"),
                            format::cyan_string("brew update && brew upgrade tod")
                        );
                        let _ = self.tx().send(Error {
                            message,
                            source: "Crates.io".into(),
                        });
                    }
                    Ok(Version::Latest) => (),
                    Err(err) => {
                        let _ = self.tx().send(err);
                    }
                }
            });
            Ok(new_config)
        } else {
            Ok(self)
        }
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

    #[allow(clippy::unused_async)]
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
            sort_order: Some(SortRule::default_order()),
            sort_value: None,
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
            time_provider: TimeProviderEnum::System(SystemTimeProvider),
            task_comment_command: None,
            task_create_command: None,
            task_complete_command: None,
            projects: Some(Vec::new()),
        })
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

    pub async fn set_token(&mut self, access_token: String) -> Result<String, Error> {
        self.token = Some(access_token);
        self.save().await
    }

    pub async fn set_developer_token(mut self, key: &str) -> Result<Config, Error> {
        let trimmed_key = key.trim();
        if trimmed_key.is_empty() {
            return Err(Error::new(
                "auth token",
                "Todoist API token cannot be empty or whitespace",
            ));
        }

        self.set_token(trimmed_key.to_string()).await?;
        self.maybe_set_timezone().await
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
                    let desc = self.token_message();

                    // We can't use mock_string from config here because it can't be set in test.
                    let fake_token = Some("faketoken".into());
                    let token = input::string(&desc, fake_token)?;
                    self.set_developer_token(&token).await?
                }
                _ => unreachable!(),
            };
            config.save().await?;
            Ok(config)
        } else {
            Ok(self)
        }
    }

    pub async fn edit_interactive(self) -> Result<String, Error> {
        let Config {
            bell_on_failure,
            bell_on_success,
            comment_exclude_regex,
            disable_links,
            max_comment_length,
            natural_language_only,
            no_sections,
            spinners,
            task_exclude_regex,
            timeout,
            token,
            verbose,

            // this is not implemented as it will be deprecated
            sort_value: _,
            sort_order: _,

            // We don't want user to set the ones below
            args: _,
            completed: _,
            internal: _,
            last_version_check: _,
            mock_select,
            mock_string: _,
            mock_url: _,
            next_id: _,
            next_task: _,
            path: _,
            projects: _,
            task_comment_command: _,
            task_complete_command: _,
            task_create_command: _,
            time_provider: _,
            timezone: _,
        } = self.clone();

        // --- bell_on_failure
        let desc = "
            bell_on_failure
            Ring terminal bell if a command fails
        ";
        let bell_on_failure = input::bool(desc, bell_on_failure, mock_select)?;

        // --- bell_on_success
        let desc = "
            bell_on_success
            Ring terminal bell if a command succeeds
        ";
        let bell_on_success = input::bool(desc, bell_on_success, mock_select)?;

        // --- spinners
        let desc = "
            spinners
            Display a spinner in terminal while waiting for an API call to complete
        ";
        let default_value = spinners.unwrap_or(true);
        let spinners = Some(input::bool(desc, default_value, mock_select)?);

        // --- verbose
        let desc = "
            verbose
            Output additional information to assist with debugging issues
        ";
        let default_value = verbose.unwrap_or(false);
        let verbose = Some(input::bool(desc, default_value, mock_select)?);

        // --- natural_language_only
        let desc = "
            natural_language_only
            Output additional information to assist with debugging issues
        ";
        let default_value = natural_language_only.unwrap_or(false);
        let natural_language_only = Some(input::bool(desc, default_value, mock_select)?);

        // --- disable_links
        let desc = "
            disable_links
            Output additional information to assist with debugging issues
        ";
        let disable_links = input::bool(desc, disable_links, mock_select)?;

        // --- no_sections
        let desc = "
            no_sections
            Do not prompt a user to select a section when working with projects
        ";
        let default_value = no_sections.unwrap_or(false);
        let no_sections = Some(input::bool(desc, default_value, mock_select)?);

        // --- token
        let desc = format!(
            "
            token
            {}
        ",
            self.token_message()
        );
        let token = input::string_with_default(&desc, &token.unwrap_or_default())?;

        let token = if token.is_empty() { None } else { Some(token) };

        // --- comment_exclude_regex
        let desc = "
            comment_exclude_regex
            Rust regex, comments that match will be excluded
            ";
        let default = match comment_exclude_regex {
            Some(regex) => regex.to_string(),
            None => String::new(),
        };
        let comment_exclude_regex = input::string_with_default(desc, &default)?;

        let comment_exclude_regex = if comment_exclude_regex.is_empty() {
            None
        } else {
            Some(Regex::new(&comment_exclude_regex)?)
        };

        // --- task_exclude_regex
        let desc = "
            task_exclude_regex
            Rust regex, tasks that match will be excluded
            ";
        let default = match task_exclude_regex {
            Some(regex) => regex.to_string(),
            None => String::new(),
        };
        let task_exclude_regex = input::string_with_default(desc, &default)?;

        let task_exclude_regex = if task_exclude_regex.is_empty() {
            None
        } else {
            Some(Regex::new(&task_exclude_regex)?)
        };

        // --- max_comment_length
        let desc = "
            max_comment_length
            Comments exceeding this length will be truncated            
            ";
        let default = match max_comment_length {
            Some(comment_length) => comment_length,
            None => MAX_COMMENT_LENGTH,
        };
        let max_comment_length: Option<u32> =
            Some(input::number_with_default(desc, default.try_into()?)?.try_into()?);

        // --- timeout
        let desc = "
            timeout
            Comments exceeding this length will be truncated            
            ";
        let default = match timeout {
            Some(comment_length) => comment_length,
            None => DEFAULT_TIMEOUT_SECONDS,
        };
        let timeout: Option<u64> =
            Some(input::number_with_default(desc, default.try_into()?)?.try_into()?);

        // ---

        let mut config = Config {
            bell_on_failure,
            max_comment_length,
            bell_on_success,
            timeout,
            comment_exclude_regex,
            task_exclude_regex,
            spinners,
            disable_links,
            no_sections,
            verbose,
            token,
            natural_language_only,
            ..self.clone()
        };

        config.save().await
    }
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
            sort_order: None,
            sort_value: None,
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
            time_provider: TimeProviderEnum::System(SystemTimeProvider),
            projects: Some(Vec::new()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test;
    use crate::test_time::FixedTimeProvider;
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;
    use tempfile::{TempDir, tempdir};

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
                sort_order: Some(SortRule::default_order()),
                sort_value: None,
                projects: Some(vec![]),
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
        /// Set the `TimeProvider` for testing
        pub fn with_time_provider(self: &Config, provider_type: TimeProviderEnum) -> Config {
            let mut config = self.clone();
            config.time_provider = provider_type;
            config
        }
    }

    fn temp_config_path(file_name: &str) -> (TempDir, PathBuf) {
        let dir = tempdir().expect("Could not create temp config directory");
        let path = dir.path().join(file_name);
        (dir, path)
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

        let sort_key = SortKey::Priority;
        let sort_key_debug = format!("{sort_key:?}");
        assert!(sort_key_debug.contains("Priority"));
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

        let sort_key = SortKey::Priority;
        let sort_key_copy = sort_key;
        assert_eq!(sort_key, sort_key_copy);

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

        assert_eq!(SortKey::default_order().first(), Some(&SortKey::Priority));
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

    #[tokio::test]
    async fn check_for_latest_version_should_skip_if_checked_today() {
        let today =
            time::date_string_today(&Config::default_test()).expect("Could not get today's date");
        let config = Config {
            last_version_check: Some(today.clone()),
            ..Config::default_test()
        };

        let result = config
            .clone()
            .check_for_latest_version()
            .await
            .expect("Expected check_for_latest_version to succeed");

        assert_eq!(result.last_version_check, Some(today));
        assert_eq!(result.path, config.path);
    }

    #[tokio::test]
    async fn check_for_latest_version_should_update_when_not_checked_today() {
        let (_temp_dir, path) = temp_config_path("version_check.cfg");
        let mut config = Config::default_test().with_path(path.clone());
        config = config.create().await.expect("Should create config file");
        config.last_version_check = Some("2000-01-01".to_string());

        let today = time::date_string_today(&config).expect("Could not get today's date");
        let result = config
            .check_for_latest_version()
            .await
            .expect("Expected check_for_latest_version to succeed");

        assert_eq!(result.last_version_check, Some(today.clone()));

        let saved = Config::load(&path)
            .await
            .expect("Expected config to be saved to disk");
        assert_eq!(saved.last_version_check, Some(today));
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
    async fn test_edit_interactive() {
        // Create a temporary config for testing
        let (_temp_dir, path) = temp_config_path("test_edit_interactive.cfg");

        // Initialize with some default values
        let mut config = Config::default_test()
            .with_path(path.clone())
            .mock_select(0);
        config = config.create().await.expect("Should create config file");

        // Mock the input to return specific values for testing
        // We'll test by mocking the input functions and verifying the result

        // This tests that edit_interactive can be called without panicking
        let result = config.edit_interactive().await;
        assert!(
            result.is_ok(),
            "edit_interactive should complete successfully"
        );

        // Verify the config file was saved
        assert!(
            tokio::fs::try_exists(&path)
                .await
                .expect("Could not check if file exists"),
            "Config file should exist after edit_interactive"
        );
    }
}
