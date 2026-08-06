use clap::{Parser, Subcommand};
use futures::future;
use std::collections::HashSet;
use std::path::Path;
use walkdir::WalkDir;

use crate::{
    config::Config,
    errors::Error,
    filters, input,
    lists::{self, Flag},
    projects,
    tasks::{self, SortOrder, Task},
    tasks::priority::{self, Priority},
    todoist,
};

#[derive(Subcommand, Debug, Clone)]
pub enum ListCommands {
    #[clap(alias = "v")]
    /// (v) View a list of tasks
    View(View),

    #[clap(alias = "c")]
    /// (c) Complete a list of tasks one by one in priority order
    Process(Process),

    #[clap(alias = "z")]
    /// (z) Give every task a priority
    Prioritize(Prioritize),

    #[clap(alias = "r")]
    /// (r) Assign reminders to tasks that do not already have a reminder
    Remind(Remind),

    #[clap(alias = "t")]
    /// (t) Give every task a date, time, and duration
    Timebox(Timebox),

    #[clap(alias = "l")]
    /// (l) Iterate through tasks and apply labels from defined choices. Use label flag once per label to choose from.
    Label(Label),

    #[clap(alias = "s")]
    /// (s) Assign dates to all tasks individually
    Schedule(Schedule),

    #[clap(alias = "d")]
    /// (d) Assign deadlines to all non-recurring tasks without deadlines individually
    Deadline(Deadline),

    #[clap(alias = "i")]
    /// (i) Create tasks from a text file, one per line using natural language. Skips empty lines.
    Import(Import),
}

#[derive(Parser, Debug, Clone)]
pub struct View {
    #[arg(short, long)]
    /// The project containing the tasks
    project: Option<String>,

    #[arg(short, long)]
    /// The filter containing the tasks. Can add multiple filters separated by commas.
    filter: Option<String>,

    #[arg(
        short = 't',
        long,
        default_value_t = SortOrder::Datetime,
        default_missing_value = "value",
        num_args = 0..=1
    )]
    /// Choose how results should be sorted
    sort: SortOrder,
}

#[derive(Parser, Debug, Clone)]
pub struct Process {
    #[arg(short, long)]
    /// The project containing the tasks
    project: Option<String>,

    #[arg(short, long)]
    /// The filter containing the tasks. Can add multiple filters separated by commas.
    filter: Option<String>,

    #[arg(
        short = 't',
        long,
        default_value_t = SortOrder::Value,
        default_missing_value = "value",
        num_args = 0..=1
    )]
    /// Choose how results should be sorted
    sort: SortOrder,
}

#[derive(Parser, Debug, Clone)]
pub struct Timebox {
    #[arg(short, long)]
    /// The project containing the tasks
    project: Option<String>,

    #[arg(short, long)]
    /// The filter containing the tasks. It does not filter out tasks with durations unless specified in the filter. Can add multiple filters separated by commas.
    filter: Option<String>,

    #[arg(
        short = 't',
        long,
        default_value_t = SortOrder::Value,
        default_missing_value = "value",
        num_args = 0..=1
    )]
    /// Choose how results should be sorted
    sort: SortOrder,
}

#[derive(Parser, Debug, Clone)]
pub struct Prioritize {
    #[arg(short, long)]
    /// The project containing the tasks
    project: Option<String>,

    #[arg(short, long)]
    /// The filter containing the tasks. Can add multiple filters separated by commas.
    filter: Option<String>,

    #[arg(
        short = 't',
        long,
        default_value_t = SortOrder::Value,
        default_missing_value = "value",
        num_args = 0..=1
    )]
    /// Choose how results should be sorted
    sort: SortOrder,

    #[arg(short = 'P', long)]
    /// Priority to assign (1-4). Required in JSON mode.
    priority: Option<u8>,
}

#[derive(Parser, Debug, Clone)]
pub struct Remind {
    #[arg(short, long)]
    /// The project containing the tasks
    project: Option<String>,

    #[arg(short, long)]
    /// The filter containing the tasks. Can add multiple filters separated by commas.
    filter: Option<String>,

    #[arg(
        short = 't',
        long,
        default_value_t = SortOrder::Value,
        default_missing_value = "value",
        num_args = 0..=1
    )]
    /// Choose how results should be sorted
    sort: SortOrder,

    #[arg(short = 'd', long)]
    /// Datetime string in natural language (e.g. "tomorrow at 3pm"). Required in JSON mode.
    datetime: Option<String>,
}

#[derive(Parser, Debug, Clone)]
pub struct Label {
    #[arg(short, long)]
    /// The filter containing the tasks. Can add multiple filters separated by commas.
    filter: Option<String>,

    #[arg(short, long)]
    /// The project containing the tasks
    project: Option<String>,

    #[arg(short = 'l', long = "label")]
    /// Labels to select from, if left blank this will be fetched from API
    labels: Vec<String>,

    #[arg(
        short = 't',
        long,
        default_value_t = SortOrder::Value,
        default_missing_value = "value",
        num_args = 0..=1
    )]
    /// Choose how results should be sorted
    sort: SortOrder,
}

#[derive(Parser, Debug, Clone)]
pub struct Schedule {
    #[arg(short, long)]
    /// The project containing the tasks
    project: Option<String>,

    #[arg(short, long)]
    /// The filter containing the tasks. Can add multiple filters separated by commas.
    filter: Option<String>,

    #[arg(short, long, default_value_t = false)]
    /// Don't re-schedule recurring tasks that are overdue
    skip_recurring: bool,

    #[arg(short, long, default_value_t = false)]
    /// Only schedule overdue tasks
    overdue: bool,

    #[arg(short = 'd', long)]
    /// Datetime string in natural language (e.g. "tomorrow at 3pm"). Required in JSON mode.
    datetime: Option<String>,

    #[arg(
        short = 't',
        long,
        default_value_t = SortOrder::Value,
        default_missing_value = "value",
        num_args = 0..=1
    )]
    /// Choose how results should be sorted
    sort: SortOrder,
}

#[derive(Parser, Debug, Clone)]
pub struct Deadline {
    #[arg(short, long)]
    /// The project containing the tasks
    project: Option<String>,

    #[arg(short, long)]
    /// The filter containing the tasks. Can add multiple filters separated by commas.
    filter: Option<String>,

    #[arg(
        short = 't',
        long,
        default_value_t = SortOrder::Value,
        default_missing_value = "value",
        num_args = 0..=1
    )]
    /// Choose how results should be sorted
    sort: SortOrder,

    #[arg(short = 'd', long)]
    /// Date string in natural language (e.g. "next Friday"). Required in JSON mode.
    date: Option<String>,
}

#[derive(Parser, Debug, Clone)]
pub struct Import {
    #[arg(short, long)]
    /// The file or directory to fuzzy find in
    path: Option<String>,
}
pub async fn view(config: &mut Config, args: &View, json: bool) -> Result<String, Error> {
    let View {
        project,
        filter,
        sort,
    } = args;

    let flag =
        super::fetch_project_or_filter(project.as_deref(), filter.as_deref(), config).await?;

    if json {
        let tasks = match &flag {
            Flag::Project(project) => todoist::all_tasks_by_project(config, project, None).await?,
            Flag::Filter(filter) => todoist::all_tasks_by_filters(config, filter)
                .await?
                .into_iter()
                .flat_map(|(_, tasks)| tasks)
                .collect(),
        };
        let count = tasks.len();
        let json = serde_json::json!({"tasks": tasks, "count": count});
        Ok(json.to_string())
    } else {
        lists::view(config, flag, sort).await
    }
}

pub async fn label(config: Config, args: &Label) -> Result<String, Error> {
    let Label {
        filter,
        project,
        labels,
        sort,
    } = args;
    let labels = super::maybe_fetch_labels(&config, labels).await?;
    let flag =
        super::fetch_project_or_filter(project.as_deref(), filter.as_deref(), &config).await?;
    lists::label(&config, flag, &labels, sort).await
}

pub async fn process(config: Config, args: &Process) -> Result<String, Error> {
    let Process {
        project,
        filter,
        sort,
    } = args;
    let flag =
        super::fetch_project_or_filter(project.as_deref(), filter.as_deref(), &config).await?;
    lists::process(&config, flag, sort).await
}

pub async fn timebox(config: Config, args: &Timebox) -> Result<String, Error> {
    let Timebox {
        project,
        filter,
        sort,
    } = args;
    let flag =
        super::fetch_project_or_filter(project.as_deref(), filter.as_deref(), &config).await?;
    lists::timebox(&config, flag, sort).await
}

pub async fn prioritize(config: Config, args: &Prioritize, json: bool) -> Result<String, Error> {
    let Prioritize {
        project,
        filter,
        sort,
        priority,
    } = args;

    if json {
        let priority = match priority {
            Some(p) => match priority::from_integer(Some(*p))? {
                Some(p) => p,
                None => {
                    return Err(Error::new(
                        "json_mode",
                        "Invalid priority. Use 1 (p4), 2 (p3), 3 (p2), or 4 (p1).",
                    ));
                }
            },
            None => {
                return Err(Error::new(
                    "json_mode",
                    "--priority flag is required in JSON mode. Use 1 (p4), 2 (p3), 3 (p2), or 4 (p1).",
                ));
            }
        };

        let flag =
            super::fetch_project_or_filter(project.as_deref(), filter.as_deref(), &config).await?;
        let tasks =
            lists::fetch_tasks_by_flag(&config, &flag, |t| t.priority == Priority::None, |_| true)
                .await?;
        let count = tasks.len();

        let handles: Vec<_> = tasks
            .iter()
            .map(|task| {
                let config = config.clone();
                let id = task.id.clone();
                async move {
                    todoist::update_task_priority(&config, &id, &priority, false).await
                }
            })
            .collect();
        future::join_all(handles).await;

        let json = serde_json::json!({"tasks": tasks, "count": count, "priority": priority});
        Ok(json.to_string())
    } else {
        let flag =
            super::fetch_project_or_filter(project.as_deref(), filter.as_deref(), &config).await?;
        lists::prioritize(&config, flag, sort).await
    }
}

pub async fn remind(config: Config, args: &Remind, json: bool) -> Result<String, Error> {
    let Remind {
        project,
        filter,
        sort,
        datetime,
    } = args;

    if json {
        let datetime = match datetime {
            Some(d) => d.clone(),
            None => {
                return Err(Error::new(
                    "json_mode",
                    "--datetime flag is required in JSON mode (e.g. \"tomorrow at 3pm\").",
                ));
            }
        };

        let reminder_task_ids = todoist::all_reminders(&config, None)
            .await?
            .into_iter()
            .map(|r| r.item_id)
            .collect::<HashSet<String>>();

        let flag =
            super::fetch_project_or_filter(project.as_deref(), filter.as_deref(), &config).await?;
        let tasks =
            lists::fetch_tasks_by_flag(&config, &flag, |t| !reminder_task_ids.contains(&t.id), |t| !reminder_task_ids.contains(&t.id))
                .await?;
        let tasks = tasks::sort(tasks, &config, *sort);
        let count = tasks.len();

        let handles: Vec<_> = tasks
            .iter()
            .map(|task| {
                let config = config.clone();
                let reminder = datetime.clone();
                async move {
                    let task = task.clone();
                    todoist::create_reminder(&config, &task, &reminder, false).await
                }
            })
            .collect();
        future::join_all(handles).await;

        let json = serde_json::json!({"tasks": tasks, "count": count});
        Ok(json.to_string())
    } else {
        let flag =
            super::fetch_project_or_filter(project.as_deref(), filter.as_deref(), &config).await?;
        lists::remind(&config, flag, sort).await
    }
}
pub async fn import(config: Config, args: &Import, json: bool) -> Result<String, Error> {
    let Import { path } = args;
    let path = super::fetch_string(path.as_deref(), &config, input::PATH)?;
    let file_path = select_file(path, &config)?;
    lists::import(&config, &file_path, json).await
}

fn select_file(path_or_file: String, config: &Config) -> Result<String, Error> {
    let path = Path::new(&path_or_file);
    if Path::is_dir(path) {
        let mut options = WalkDir::new(path_or_file)
            .into_iter()
            .filter_map(std::result::Result::ok)
            .filter(is_md_file)
            .map(|e| {
                e.path()
                    .to_str()
                    .expect("Could not make str out of DirEntry")
                    .to_string()
            })
            .collect::<Vec<String>>();
        options.sort();
        options.dedup();
        let path = input::select("Select file to process", options, config.mock_select)?;

        Ok(path)
    } else if Path::is_file(path) {
        Ok(path_or_file)
    } else {
        Err(Error {
            source: "select_file".to_string(),
            message: format!("{path_or_file} is neither a file nor a directory"),
        })
    }
}

fn is_md_file(entry: &walkdir::DirEntry) -> bool {
    std::path::Path::new(entry.file_name().to_str().unwrap_or_default())
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
}

pub async fn schedule(config: Config, args: &Schedule, json: bool) -> Result<String, Error> {
    let Schedule {
        project,
        filter,
        skip_recurring,
        overdue,
        sort,
        datetime,
    } = args;

    if json {
        let datetime = match datetime {
            Some(d) => d.clone(),
            None => {
                return Err(Error::new(
                    "json_mode",
                    "--datetime flag is required in JSON mode (e.g. \"tomorrow at 3pm\").",
                ));
            }
        };

        let flag =
            super::fetch_project_or_filter(project.as_deref(), filter.as_deref(), &config).await?;
        let tasks = match &flag {
            Flag::Project(project) => {
                let tasks = todoist::all_tasks_by_project(&config, project, None).await?;
                let tasks = tasks::sort(tasks, &config, *sort);
                let task_filter = if *overdue {
                    projects::TaskFilter::Overdue
                } else {
                    projects::TaskFilter::Unscheduled
                };
                let tasks: Vec<Task> = if *skip_recurring {
                    tasks
                        .into_iter()
                        .filter(|task| {
                            task.filter(&config, &task_filter)
                                && !task.filter(&config, &projects::TaskFilter::Recurring)
                        })
                        .collect()
                } else {
                    tasks
                        .into_iter()
                        .filter(|task| task.filter(&config, &task_filter))
                        .collect()
                };
                tasks
            }
            Flag::Filter(filter) => {
                let tasks = todoist::all_tasks_by_filters(&config, filter)
                    .await?
                    .into_iter()
                    .flat_map(|(_, tasks)| tasks)
                    .collect::<Vec<Task>>();
                tasks::sort(tasks, &config, *sort)
            }
        };
        let count = tasks.len();

        let handles: Vec<_> = tasks
            .iter()
            .map(|task| {
                let config = config.clone();
                let dt = datetime.clone();
                let task = task.clone();
                async move {
                    todoist::update_task_due_natural_language(
                        &config, &task, dt, None, false,
                    )
                    .await
                }
            })
            .collect();
        future::join_all(handles).await;

        let json = serde_json::json!({"tasks": tasks, "count": count});
        Ok(json.to_string())
    } else {
        match super::fetch_project_or_filter(project.as_deref(), filter.as_deref(), &config).await? {
            Flag::Filter(filter) => filters::schedule(&config, &filter, sort).await,
            Flag::Project(project) => {
                let task_filter = if *overdue {
                    projects::TaskFilter::Overdue
                } else {
                    projects::TaskFilter::Unscheduled
                };

                projects::schedule(&config, &project, task_filter, *skip_recurring, sort).await
            }
        }
    }
}

pub async fn deadline(config: Config, args: &Deadline, json: bool) -> Result<String, Error> {
    let Deadline {
        project,
        filter,
        sort,
        date,
    } = args;

    if json {
        let date = match date {
            Some(d) => d.clone(),
            None => {
                return Err(Error::new(
                    "json_mode",
                    "--date flag is required in JSON mode (e.g. \"next Friday\").",
                ));
            }
        };

        let flag =
            super::fetch_project_or_filter(project.as_deref(), filter.as_deref(), &config).await?;
        let tasks = match &flag {
            Flag::Project(project) => {
                let tasks = todoist::all_tasks_by_project(&config, project, None).await?;
                let tasks = tasks::sort(tasks, &config, *sort);
                tasks
                    .into_iter()
                    .filter(|task| {
                        !task.filter(&config, &projects::TaskFilter::Recurring)
                            && task.deadline.is_none()
                    })
                    .collect::<Vec<Task>>()
            }
            Flag::Filter(filter) => {
                let tasks = todoist::all_tasks_by_filters(&config, filter)
                    .await?
                    .into_iter()
                    .flat_map(|(_, tasks)| tasks)
                    .collect::<Vec<Task>>();
                let tasks = tasks::sort(tasks, &config, *sort);
                tasks
                    .into_iter()
                    .filter(|task| {
                        !task.filter(&config, &projects::TaskFilter::Recurring)
                    })
                    .collect::<Vec<Task>>()
            }
        };
        let count = tasks.len();

        let handles: Vec<_> = tasks
            .iter()
            .map(|task| {
                let config = config.clone();
                let d = date.clone();
                let id = task.id.clone();
                async move {
                    todoist::update_task_deadline(&config, &id, Some(d), false).await
                }
            })
            .collect();
        future::join_all(handles).await;

        let json = serde_json::json!({"tasks": tasks, "count": count});
        Ok(json.to_string())
    } else {
        match super::fetch_project_or_filter(project.as_deref(), filter.as_deref(), &config).await? {
            Flag::Filter(filter) => filters::deadline(&config, &filter, sort).await,
            Flag::Project(project) => projects::deadline(&config, &project, sort).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn view_sort_without_value_uses_configured_sort() {
        let args = View::try_parse_from(["tod", "--sort"]).expect("--sort should be valid");
        assert_eq!(args.sort.to_string(), "value");
    }

    #[test]
    fn view_without_sort_keeps_datetime_default() {
        let args = View::try_parse_from(["tod"]).expect("view arguments should be valid");
        assert_eq!(args.sort.to_string(), "datetime");
    }

    #[test]
    fn is_md_file_returns_true_for_markdown() {
        let dir = tempfile::tempdir().expect("temp dir should be created");
        std::fs::write(dir.path().join("readme.md"), "# Hello").expect("file should be created");
        let entry = walkdir::WalkDir::new(dir.path())
            .into_iter()
            .filter_map(std::result::Result::ok)
            .find(|e| e.file_name() == "readme.md")
            .expect("readme.md should be found");
        assert!(is_md_file(&entry));
    }

    #[test]
    fn is_md_file_case_insensitive() {
        let dir = tempfile::tempdir().expect("temp dir should be created");
        std::fs::write(dir.path().join("NOTES.MD"), "# Notes").expect("file should be created");
        let entry = walkdir::WalkDir::new(dir.path())
            .into_iter()
            .filter_map(std::result::Result::ok)
            .find(|e| e.file_name() == "NOTES.MD")
            .expect("NOTES.MD should be found");
        assert!(is_md_file(&entry));
    }

    #[test]
    fn is_md_file_returns_false_for_non_markdown() {
        let dir = tempfile::tempdir().expect("temp dir should be created");
        std::fs::write(dir.path().join("data.txt"), "hello").expect("file should be created");
        let entry = walkdir::WalkDir::new(dir.path())
            .into_iter()
            .filter_map(std::result::Result::ok)
            .find(|e| e.file_name() == "data.txt")
            .expect("data.txt should be found");
        assert!(!is_md_file(&entry));
    }

    #[test]
    fn is_md_file_returns_false_for_no_extension() {
        let dir = tempfile::tempdir().expect("temp dir should be created");
        std::fs::write(dir.path().join("Makefile"), "all:").expect("file should be created");
        let entry = walkdir::WalkDir::new(dir.path())
            .into_iter()
            .filter_map(std::result::Result::ok)
            .find(|e| e.file_name() == "Makefile")
            .expect("Makefile should be found");
        assert!(!is_md_file(&entry));
    }

    #[tokio::test]
    async fn select_file_returns_path_when_file_exists() {
        let dir = tempfile::tempdir().expect("temp dir should be created");
        let file_path = dir.path().join("tasks.md");
        std::fs::write(&file_path, "# Tasks").expect("file should be created");
        let config = crate::config::Config::new(None, dir.path().join("tod.cfg"))
            .await
            .expect("config should be created");
        let result = select_file(
            file_path
                .to_str()
                .expect("path should be valid")
                .to_string(),
            &config,
        );
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            file_path.to_str().expect("path should be valid")
        );
    }

    #[tokio::test]
    async fn select_file_returns_error_for_nonexistent_path() {
        let dir = tempfile::tempdir().expect("temp dir should be created");
        let config = crate::config::Config::new(None, dir.path().join("tod.cfg"))
            .await
            .expect("config should be created");
        let result = select_file(
            dir.path()
                .join("nonexistent")
                .to_str()
                .expect("path should be valid")
                .to_string(),
            &config,
        );
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .message
                .contains("neither a file nor a directory")
        );
    }

    #[tokio::test]
    async fn select_file_picks_md_from_directory() {
        let dir = tempfile::tempdir().expect("temp dir should be created");
        std::fs::write(dir.path().join("a.md"), "A").expect("file should be created");
        std::fs::write(dir.path().join("b.md"), "B").expect("file should be created");
        std::fs::write(dir.path().join("c.txt"), "C").expect("file should be created");
        let config = crate::config::Config::new(None, dir.path().join("tod.cfg"))
            .await
            .expect("config should be created")
            .mock_select(0);
        let result = select_file(
            dir.path()
                .to_str()
                .expect("path should be valid")
                .to_string(),
            &config,
        );
        assert!(
            result.is_ok(),
            "selecting first .md file from dir should succeed"
        );
        assert!(result.unwrap().ends_with("a.md"));
    }
}
