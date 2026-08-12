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
    tasks::priority::{self, Priority},
    tasks::{self, SortOrder, Task},
    todoist,
};

/// Multi-task subcommands (view, process, schedule, etc.).
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
    /// Deadline date in YYYY-MM-DD format (e.g. "2026-05-01"). Required in JSON mode.
    date: Option<String>,
}

#[derive(Parser, Debug, Clone)]
pub struct Import {
    #[arg(short, long)]
    /// The file or directory to fuzzy find in
    path: Option<String>,
}
/// Views tasks matching a project or filter.
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

/// Applies labels from a predefined list or the API.
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

/// Walks through tasks one at a time for completion.
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

/// Assigns dates, times, and durations to tasks.
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

/// Assigns priorities to unprioritized tasks.
pub async fn prioritize(config: Config, args: &Prioritize, json: bool) -> Result<String, Error> {
    let Prioritize {
        project,
        filter,
        sort,
        priority,
    } = args;

    if !json {
        let flag =
            super::fetch_project_or_filter(project.as_deref(), filter.as_deref(), &config).await?;
        return lists::prioritize(&config, flag, sort).await;
    }

    let Some(p) = priority else {
        return Err(Error::new(
            "json_mode",
            "--priority flag is required in JSON mode. Use 1 (p4), 2 (p3), 3 (p2), or 4 (p1).",
        ));
    };
    let Some(priority) = priority::from_integer(Some(*p))? else {
        return Err(Error::new(
            "json_mode",
            "Invalid priority. Use 1 (p4), 2 (p3), 3 (p2), or 4 (p1).",
        ));
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
            async move { todoist::update_task_priority(&config, &id, &priority, false).await }
        })
        .collect();
    future::join_all(handles).await;

    let json_output = serde_json::json!({"tasks": tasks, "count": count, "priority": priority});
    Ok(json_output.to_string())
}

/// Adds reminders to tasks that lack them.
pub async fn remind(config: Config, args: &Remind, json: bool) -> Result<String, Error> {
    let Remind {
        project,
        filter,
        sort,
        datetime,
    } = args;

    if !json {
        let flag =
            super::fetch_project_or_filter(project.as_deref(), filter.as_deref(), &config).await?;
        return lists::remind(&config, flag, sort).await;
    }

    let Some(datetime) = datetime else {
        return Err(Error::new(
            "json_mode",
            "--datetime flag is required in JSON mode (e.g. \"tomorrow at 3pm\").",
        ));
    };

    let reminder_task_ids = todoist::all_reminders(&config, None)
        .await?
        .into_iter()
        .map(|r| r.item_id)
        .collect::<HashSet<String>>();

    let flag =
        super::fetch_project_or_filter(project.as_deref(), filter.as_deref(), &config).await?;
    let tasks = lists::fetch_tasks_by_flag(
        &config,
        &flag,
        |t| !reminder_task_ids.contains(&t.id),
        |t| !reminder_task_ids.contains(&t.id),
    )
    .await?;
    let tasks = tasks::sort(tasks, &config, *sort);
    let count = tasks.len();

    let handles: Vec<_> = tasks
        .iter()
        .map(|task| {
            let config = config.clone();
            let reminder = datetime.clone();
            let task = task.clone();
            async move { todoist::create_reminder(&config, &task, &reminder, false).await }
        })
        .collect();
    future::join_all(handles).await;

    let json_output = serde_json::json!({"tasks": tasks, "count": count});
    Ok(json_output.to_string())
}
/// Creates tasks from a text file using natural language.
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
        let path = input::select("Select file to process", options, &config.mock_select)?;

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

async fn fetch_schedule_tasks(
    config: &Config,
    flag: &Flag,
    sort: &SortOrder,
    overdue: bool,
    skip_recurring: bool,
) -> Result<Vec<Task>, Error> {
    match flag {
        Flag::Project(project) => {
            let tasks = todoist::all_tasks_by_project(config, project, None).await?;
            let tasks = tasks::sort(tasks, config, *sort);
            let task_filter = if overdue {
                projects::TaskFilter::Overdue
            } else {
                projects::TaskFilter::Unscheduled
            };
            Ok(tasks
                .into_iter()
                .filter(|task| {
                    let matches_filter = task.filter(config, &task_filter);
                    if skip_recurring {
                        matches_filter && !task.filter(config, &projects::TaskFilter::Recurring)
                    } else {
                        matches_filter
                    }
                })
                .collect())
        }
        Flag::Filter(filter) => {
            let tasks = todoist::all_tasks_by_filters(config, filter)
                .await?
                .into_iter()
                .flat_map(|(_, tasks)| tasks)
                .collect::<Vec<Task>>();
            Ok(tasks::sort(tasks, config, *sort))
        }
    }
}

async fn fetch_deadline_tasks(
    config: &Config,
    flag: &Flag,
    sort: &SortOrder,
) -> Result<Vec<Task>, Error> {
    match flag {
        Flag::Project(project) => {
            let tasks = todoist::all_tasks_by_project(config, project, None).await?;
            let tasks = tasks::sort(tasks, config, *sort);
            Ok(tasks
                .into_iter()
                .filter(|task| {
                    !task.filter(config, &projects::TaskFilter::Recurring)
                        && task.deadline.is_none()
                })
                .collect())
        }
        Flag::Filter(filter) => {
            let tasks = todoist::all_tasks_by_filters(config, filter)
                .await?
                .into_iter()
                .flat_map(|(_, tasks)| tasks)
                .collect::<Vec<Task>>();
            let tasks = tasks::sort(tasks, config, *sort);
            Ok(tasks
                .into_iter()
                .filter(|task| !task.filter(config, &projects::TaskFilter::Recurring))
                .collect())
        }
    }
}

/// Schedules dates on tasks individually.
pub async fn schedule(config: Config, args: &Schedule, json: bool) -> Result<String, Error> {
    let Schedule {
        project,
        filter,
        skip_recurring,
        overdue,
        sort,
        datetime,
    } = args;

    if !json {
        return match super::fetch_project_or_filter(project.as_deref(), filter.as_deref(), &config)
            .await?
        {
            Flag::Filter(filter) => filters::schedule(&config, &filter, sort).await,
            Flag::Project(project) => {
                let task_filter = if *overdue {
                    projects::TaskFilter::Overdue
                } else {
                    projects::TaskFilter::Unscheduled
                };
                projects::schedule(&config, &project, task_filter, *skip_recurring, sort).await
            }
        };
    }

    let Some(datetime) = datetime else {
        return Err(Error::new(
            "json_mode",
            "--datetime flag is required in JSON mode (e.g. \"tomorrow at 3pm\").",
        ));
    };

    let flag =
        super::fetch_project_or_filter(project.as_deref(), filter.as_deref(), &config).await?;
    let tasks = fetch_schedule_tasks(&config, &flag, sort, *overdue, *skip_recurring).await?;
    let count = tasks.len();

    let handles: Vec<_> = tasks
        .iter()
        .map(|task| {
            let config = config.clone();
            let dt = datetime.clone();
            let task = task.clone();
            async move {
                todoist::update_task_due_natural_language(&config, &task, dt, None, false).await
            }
        })
        .collect();
    future::join_all(handles).await;

    let json_output = serde_json::json!({"tasks": tasks, "count": count});
    Ok(json_output.to_string())
}

/// Sets deadlines on non-recurring tasks without deadlines.
pub async fn deadline(config: Config, args: &Deadline, json: bool) -> Result<String, Error> {
    let Deadline {
        project,
        filter,
        sort,
        date,
    } = args;

    if !json {
        return match super::fetch_project_or_filter(project.as_deref(), filter.as_deref(), &config)
            .await?
        {
            Flag::Filter(filter) => filters::deadline(&config, &filter, sort).await,
            Flag::Project(project) => projects::deadline(&config, &project, sort).await,
        };
    }

    let Some(date) = date else {
        return Err(Error::new(
            "json_mode",
            "--date flag is required in JSON mode (YYYY-MM-DD format, e.g. \"2026-05-01\").",
        ));
    };

    let flag =
        super::fetch_project_or_filter(project.as_deref(), filter.as_deref(), &config).await?;
    let tasks = fetch_deadline_tasks(&config, &flag, sort).await?;
    let count = tasks.len();

    let handles: Vec<_> = tasks
        .iter()
        .map(|task| {
            let config = config.clone();
            let d = date.clone();
            let id = task.id.clone();
            async move { todoist::update_task_deadline(&config, &id, Some(d), false).await }
        })
        .collect();
    future::join_all(handles).await;

    let json_output = serde_json::json!({"tasks": tasks, "count": count});
    Ok(json_output.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test;
    use pretty_assertions::assert_eq;

    /// Minimal task JSON with overrideable priority, labels, due, deadline, and duration.
    fn task_json(id: &str, priority: u8) -> String {
        format!(
            r#"{{"id":"{id}","user_id":"910","project_id":"123","section_id":null,"parent_id":null,"added_by_uid":null,"assigned_by_uid":null,"responsible_uid":null,"labels":[],"deadline":null,"duration":null,"due":null,"checked":false,"is_deleted":false,"is_collapsed":false,"added_at":"2026-01-01T00:00:00Z","completed_at":null,"updated_at":"2026-01-01T00:00:00Z","priority":{priority},"child_order":1,"content":"Task {id}","description":"","note_count":0,"day_order":1}}"#
        )
    }

    fn tasks_response_json(tasks: &[&str]) -> String {
        let results = tasks.join(",");
        format!(r#"{{"results":[{results}],"next_cursor":null}}"#)
    }

    fn empty_tasks_response() -> String {
        r#"{"results":[],"next_cursor":null}"#.to_string()
    }

    fn empty_reminders_response() -> String {
        r#"{"results":[],"next_cursor":null}"#.to_string()
    }

    fn recurring_task_json(id: &str) -> String {
        format!(
            r#"{{"id":"{id}","user_id":"910","project_id":"123","section_id":null,"parent_id":null,"added_by_uid":null,"assigned_by_uid":null,"responsible_uid":null,"labels":[],"deadline":null,"duration":null,"due":{{"date":"2020-01-01","is_recurring":true,"string":"every day","lang":"en","timezone":null}},"checked":false,"is_deleted":false,"is_collapsed":false,"added_at":"2026-01-01T00:00:00Z","completed_at":null,"updated_at":"2026-01-01T00:00:00Z","priority":1,"child_order":1,"content":"Recurring {id}","description":"","note_count":0,"day_order":1}}"#
        )
    }

    async fn config_with_project() -> Config {
        test::fixtures::config().await
    }

    fn assert_json_tasks_count(json: &str, expected_count: usize) {
        let v: serde_json::Value = serde_json::from_str(json).expect("output should be valid JSON");
        assert_eq!(
            v["count"].as_u64().expect("count should be a number") as usize,
            expected_count
        );
        assert!(v["tasks"].is_array(), "tasks should be an array");
    }

    // ─── prioritize (JSON) ────────────────────────────────────────

    #[tokio::test]
    async fn prioritize_json_with_filter_sets_priority_and_returns_json() {
        let mut server = mockito::Server::new_async().await;
        let task_a = task_json("t1", 1); // Priority::None
        let task_b = task_json("t2", 1); // Priority::None

        let tasks_mock = server
            .mock("GET", "/api/v1/tasks/filter?query=today&limit=200")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(tasks_response_json(&[&task_a, &task_b]))
            .create_async()
            .await;
        let update_mock = server
            .mock("POST", "/api/v1/tasks/t1")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(task_json("t1", 4))
            .expect(1)
            .create_async()
            .await;
        let update_mock2 = server
            .mock("POST", "/api/v1/tasks/t2")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(task_json("t2", 4))
            .expect(1)
            .create_async()
            .await;

        let config = config_with_project().await.with_mock_url(server.url());
        let args = Prioritize {
            project: None,
            filter: Some("today".into()),
            sort: SortOrder::Value,
            priority: Some(4),
        };

        let result = prioritize(config, &args, true)
            .await
            .expect("should succeed");
        assert_json_tasks_count(&result, 2);
        let v: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(v["priority"].as_u64().unwrap(), 4);

        tasks_mock.assert();
        update_mock.assert();
        update_mock2.assert();
    }

    #[tokio::test]
    async fn prioritize_json_only_updates_tasks_matching_filter() {
        let mut server = mockito::Server::new_async().await;
        let task_a = task_json("t1", 1); // Priority::None
        let task_b = task_json("t2", 4); // Already Priority::High

        let tasks_mock = server
            .mock("GET", "/api/v1/tasks/?project_id=123&limit=200")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(tasks_response_json(&[&task_a, &task_b]))
            .create_async()
            .await;
        // Only t1 (Priority::None) should be updated since project_filter filters by priority
        let update_mock = server
            .mock("POST", "/api/v1/tasks/t1")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(task_json("t1", 3))
            .expect(1)
            .create_async()
            .await;

        let config = config_with_project().await.with_mock_url(server.url());
        let args = Prioritize {
            project: Some("myproject".into()),
            filter: None,
            sort: SortOrder::Value,
            priority: Some(3),
        };

        let result = prioritize(config, &args, true)
            .await
            .expect("should succeed");
        assert_json_tasks_count(&result, 1);

        tasks_mock.assert();
        update_mock.assert();
    }

    #[tokio::test]
    async fn prioritize_json_missing_priority_flag_errors() {
        let config = config_with_project().await;
        let args = Prioritize {
            project: Some("myproject".into()),
            filter: None,
            sort: SortOrder::Value,
            priority: None,
        };

        let result = prioritize(config, &args, true).await;
        let err = result.expect_err("should error without --priority flag");
        assert_eq!(err.source, "json_mode");
        assert!(err.message.contains("--priority"));
    }

    #[tokio::test]
    async fn prioritize_non_json_uses_interactive_path() {
        let mut server = mockito::Server::new_async().await;
        let tasks_mock = server
            .mock("GET", "/api/v1/tasks/filter?query=today&limit=200")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(empty_tasks_response())
            .create_async()
            .await;

        let config = config_with_project()
            .await
            .with_mock_url(server.url())
            .mock_select(1);
        let args = Prioritize {
            project: None,
            filter: Some("today".into()),
            sort: SortOrder::Value,
            priority: None,
        };

        let result = prioritize(config, &args, false)
            .await
            .expect("should succeed");
        assert!(result.contains("No tasks"));
        tasks_mock.assert();
    }

    // ─── remind (JSON) ────────────────────────────────────────────

    #[tokio::test]
    async fn remind_json_with_filter_creates_reminders_and_returns_json() {
        let mut server = mockito::Server::new_async().await;
        let task = task_json("t1", 1);

        let reminders_mock = server
            .mock("GET", "/api/v1/reminders?limit=200")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(empty_reminders_response())
            .create_async()
            .await;
        let tasks_mock = server
            .mock("GET", "/api/v1/tasks/filter?query=today&limit=200")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(tasks_response_json(&[&task]))
            .create_async()
            .await;
        let create_mock = server
            .mock("POST", "/api/v1/reminders")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id":"r1","item_id":"t1","notify_uid":"910","type":"absolute","is_deleted":false,"minute_offset":0,"is_urgent":false,"due":{"date":"2026-01-18T17:00:00","timezone":null,"string":"2026-01-18 17:00","lang":"en","is_recurring":false}}"#)
            .expect(1)
            .create_async()
            .await;

        let config = config_with_project().await.with_mock_url(server.url());
        let args = Remind {
            project: None,
            filter: Some("today".into()),
            sort: SortOrder::Value,
            datetime: Some("tomorrow at 3pm".into()),
        };

        let result = remind(config, &args, true).await.expect("should succeed");
        assert_json_tasks_count(&result, 1);

        reminders_mock.assert();
        tasks_mock.assert();
        create_mock.assert();
    }

    #[tokio::test]
    async fn remind_json_skips_tasks_with_existing_reminders() {
        let mut server = mockito::Server::new_async().await;
        let task_a = task_json("t1", 1);
        let task_b = task_json("t2", 1);

        let reminders_mock = server
            .mock("GET", "/api/v1/reminders?limit=200")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"results":[{"id":"r1","item_id":"t1","notify_uid":"910","type":"absolute","is_deleted":false,"minute_offset":0,"is_urgent":false,"due":{"date":"2026-01-18T17:00:00","timezone":null,"string":"2026-01-18 17:00","lang":"en","is_recurring":false}}],"next_cursor":null}"#)
            .create_async()
            .await;
        let tasks_mock = server
            .mock("GET", "/api/v1/tasks/filter?query=today&limit=200")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(tasks_response_json(&[&task_a, &task_b]))
            .create_async()
            .await;
        let create_mock = server
            .mock("POST", "/api/v1/reminders")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id":"r2","item_id":"t2","notify_uid":"910","type":"absolute","is_deleted":false,"minute_offset":0,"is_urgent":false,"due":{"date":"2026-01-18T17:00:00","timezone":null,"string":"2026-01-18 17:00","lang":"en","is_recurring":false}}"#)
            .expect(1)
            .create_async()
            .await;

        let config = config_with_project().await.with_mock_url(server.url());
        let args = Remind {
            project: None,
            filter: Some("today".into()),
            sort: SortOrder::Value,
            datetime: Some("tomorrow at 3pm".into()),
        };

        let result = remind(config, &args, true).await.expect("should succeed");
        assert_json_tasks_count(&result, 1);

        reminders_mock.assert();
        tasks_mock.assert();
        create_mock.assert();
    }

    #[tokio::test]
    async fn remind_json_missing_datetime_flag_errors() {
        let config = config_with_project().await;
        let args = Remind {
            project: Some("myproject".into()),
            filter: None,
            sort: SortOrder::Value,
            datetime: None,
        };

        let result = remind(config, &args, true).await;
        let err = result.expect_err("should error without --datetime flag");
        assert_eq!(err.source, "json_mode");
        assert!(err.message.contains("--datetime"));
    }

    #[tokio::test]
    async fn remind_non_json_uses_interactive_path() {
        let mut server = mockito::Server::new_async().await;
        let reminders_mock = server
            .mock("GET", "/api/v1/reminders?limit=200")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(empty_reminders_response())
            .create_async()
            .await;
        let tasks_mock = server
            .mock("GET", "/api/v1/tasks/filter?query=today&limit=200")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(empty_tasks_response())
            .create_async()
            .await;

        let config = config_with_project()
            .await
            .with_mock_url(server.url())
            .mock_select(1);
        let args = Remind {
            project: None,
            filter: Some("today".into()),
            sort: SortOrder::Value,
            datetime: None,
        };

        let result = remind(config, &args, false).await.expect("should succeed");
        assert!(result.contains("No tasks"));
        reminders_mock.assert();
        tasks_mock.assert();
    }

    #[tokio::test]
    async fn remind_json_with_project_skips_tasks_with_existing_reminders() {
        let mut server = mockito::Server::new_async().await;
        let task_a = task_json("t1", 1);
        let task_b = task_json("t2", 1);

        let reminders_mock = server
            .mock("GET", "/api/v1/reminders?limit=200")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{"results":[{"id":"r1","item_id":"t1","notify_uid":"910","type":"absolute","is_deleted":false,"minute_offset":0,"is_urgent":false,"due":{"date":"2026-01-18T17:00:00","timezone":null,"string":"2026-01-18 17:00","lang":"en","is_recurring":false}}],"next_cursor":null}"#,
            )
            .create_async()
            .await;
        let tasks_mock = server
            .mock("GET", "/api/v1/tasks/?project_id=123&limit=200")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(tasks_response_json(&[&task_a, &task_b]))
            .create_async()
            .await;
        let create_mock = server
            .mock("POST", "/api/v1/reminders")
            .match_body(mockito::Matcher::Regex(r#""task_id":"t2""#.into()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{"id":"r2","item_id":"t2","notify_uid":"910","type":"absolute","is_deleted":false,"minute_offset":0,"is_urgent":false,"due":{"date":"2026-01-18T17:00:00","timezone":null,"string":"2026-01-18 17:00","lang":"en","is_recurring":false}}"#,
            )
            .expect(1)
            .create_async()
            .await;

        let config = config_with_project().await.with_mock_url(server.url());
        let args = Remind {
            project: Some("myproject".into()),
            filter: None,
            sort: SortOrder::Value,
            datetime: Some("tomorrow at 3pm".into()),
        };

        let result = remind(config, &args, true).await.expect("should succeed");
        assert_json_tasks_count(&result, 1);

        reminders_mock.assert();
        tasks_mock.assert();
        create_mock.assert();
    }

    // ─── schedule (JSON) ──────────────────────────────────────────

    #[tokio::test]
    async fn schedule_json_with_filter_schedules_tasks_and_returns_json() {
        let mut server = mockito::Server::new_async().await;
        let task = task_json("t1", 1);

        let tasks_mock = server
            .mock("GET", "/api/v1/tasks/filter?query=today&limit=200")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(tasks_response_json(&[&task]))
            .create_async()
            .await;
        let update_mock = server
            .mock("POST", "/api/v1/tasks/t1")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(task_json("t1", 1))
            .expect(1)
            .create_async()
            .await;

        let config = config_with_project().await.with_mock_url(server.url());
        let args = Schedule {
            project: None,
            filter: Some("today".into()),
            skip_recurring: false,
            overdue: false,
            sort: SortOrder::Value,
            datetime: Some("tomorrow at 3pm".into()),
        };

        let result = schedule(config, &args, true).await.expect("should succeed");
        assert_json_tasks_count(&result, 1);

        tasks_mock.assert();
        update_mock.assert();
    }

    #[tokio::test]
    async fn schedule_json_with_project_respects_overdue_filter() {
        let mut server = mockito::Server::new_async().await;
        // Task has no due date (not overdue), so it should be filtered out when overdue=true
        let task = task_json("t1", 1);

        let tasks_mock = server
            .mock("GET", "/api/v1/tasks/?project_id=123&limit=200")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(tasks_response_json(&[&task]))
            .create_async()
            .await;

        let config = config_with_project().await.with_mock_url(server.url());
        let args = Schedule {
            project: Some("myproject".into()),
            filter: None,
            skip_recurring: false,
            overdue: true,
            sort: SortOrder::Value,
            datetime: Some("tomorrow at 3pm".into()),
        };

        let result = schedule(config, &args, true).await.expect("should succeed");
        assert_json_tasks_count(&result, 0);

        tasks_mock.assert();
    }

    #[tokio::test]
    async fn schedule_json_missing_datetime_flag_errors() {
        let config = config_with_project().await;
        let args = Schedule {
            project: Some("myproject".into()),
            filter: None,
            skip_recurring: false,
            overdue: false,
            sort: SortOrder::Value,
            datetime: None,
        };

        let result = schedule(config, &args, true).await;
        let err = result.expect_err("should error without --datetime flag");
        assert_eq!(err.source, "json_mode");
        assert!(err.message.contains("--datetime"));
    }

    #[tokio::test]
    async fn schedule_non_json_uses_interactive_path() {
        let mut server = mockito::Server::new_async().await;
        let tasks_mock = server
            .mock("GET", "/api/v1/tasks/filter?query=today&limit=200")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(empty_tasks_response())
            .create_async()
            .await;

        let config = config_with_project()
            .await
            .with_mock_url(server.url())
            .mock_select(1);
        let args = Schedule {
            project: None,
            filter: Some("today".into()),
            skip_recurring: false,
            overdue: false,
            sort: SortOrder::Value,
            datetime: None,
        };

        let result = schedule(config, &args, false)
            .await
            .expect("should succeed");
        assert!(result.contains("No tasks"));
        tasks_mock.assert();
    }

    // ─── deadline (JSON) ──────────────────────────────────────────

    #[tokio::test]
    async fn deadline_json_with_project_sets_deadlines_and_returns_json() {
        let mut server = mockito::Server::new_async().await;
        let task = task_json("t1", 1);

        let tasks_mock = server
            .mock("GET", "/api/v1/tasks/?project_id=123&limit=200")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(tasks_response_json(&[&task]))
            .create_async()
            .await;
        let update_mock = server
            .mock("POST", "/api/v1/tasks/t1")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(task_json("t1", 1))
            .expect(1)
            .create_async()
            .await;

        let config = config_with_project().await.with_mock_url(server.url());
        let args = Deadline {
            project: Some("myproject".into()),
            filter: None,
            sort: SortOrder::Value,
            date: Some("2026-05-01".into()),
        };

        let result = deadline(config, &args, true).await.expect("should succeed");
        assert_json_tasks_count(&result, 1);

        tasks_mock.assert();
        update_mock.assert();
    }

    #[tokio::test]
    async fn deadline_non_json_uses_interactive_path() {
        let mut server = mockito::Server::new_async().await;
        let tasks_mock = server
            .mock("GET", "/api/v1/tasks/filter?query=today&limit=200")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(empty_tasks_response())
            .create_async()
            .await;

        let config = config_with_project()
            .await
            .with_mock_url(server.url())
            .mock_select(1);
        let args = Deadline {
            project: None,
            filter: Some("today".into()),
            sort: SortOrder::Value,
            date: None,
        };

        let result = deadline(config, &args, false)
            .await
            .expect("should succeed");
        assert!(result.contains("No tasks"));
        tasks_mock.assert();
    }

    #[tokio::test]
    async fn deadline_json_missing_date_flag_errors() {
        let config = config_with_project().await;
        let args = Deadline {
            project: Some("myproject".into()),
            filter: None,
            sort: SortOrder::Value,
            date: None,
        };

        let result = deadline(config, &args, true).await;
        let err = result.expect_err("should error without --date flag");
        assert_eq!(err.source, "json_mode");
        assert!(err.message.contains("--date"));
    }

    #[tokio::test]
    async fn fetch_deadline_tasks_with_filter_passes_non_recurring_tasks() {
        let mut server = mockito::Server::new_async().await;
        let task = task_json("t1", 1);
        let tasks_mock = server
            .mock("GET", "/api/v1/tasks/filter?query=today&limit=200")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(tasks_response_json(&[&task]))
            .create_async()
            .await;

        let config = config_with_project().await.with_mock_url(server.url());
        let result =
            fetch_deadline_tasks(&config, &Flag::Filter("today".into()), &SortOrder::Value)
                .await
                .expect("should succeed");

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "t1");
        tasks_mock.assert();
    }

    // ─── helpers ──────────────────────────────────────────────────

    #[tokio::test]
    async fn fetch_schedule_tasks_with_project_filters_unscheduled() {
        let mut server = mockito::Server::new_async().await;
        let task = task_json("t1", 1);
        let tasks_mock = server
            .mock("GET", "/api/v1/tasks/?project_id=123&limit=200")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(tasks_response_json(&[&task]))
            .create_async()
            .await;

        let config = config_with_project().await.with_mock_url(server.url());
        let project = config
            .projects()
            .await
            .expect("should fetch projects")
            .into_iter()
            .next()
            .expect("should have at least one project");

        let result = fetch_schedule_tasks(
            &config,
            &Flag::Project(project),
            &SortOrder::Value,
            false, // overdue
            false, // skip_recurring
        )
        .await
        .expect("should succeed");

        assert_eq!(result.len(), 1);
        tasks_mock.assert();
    }

    #[tokio::test]
    async fn fetch_schedule_tasks_skip_recurring_filters_out_recurring() {
        let mut server = mockito::Server::new_async().await;
        let recurring = recurring_task_json("t1");
        let normal = task_json("t2", 1);
        let tasks_mock = server
            .mock("GET", "/api/v1/tasks/?project_id=123&limit=200")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(tasks_response_json(&[&recurring, &normal]))
            .create_async()
            .await;

        let config = config_with_project().await.with_mock_url(server.url());
        let project = config
            .projects()
            .await
            .expect("should fetch projects")
            .into_iter()
            .next()
            .expect("should have at least one project");

        let result = fetch_schedule_tasks(
            &config,
            &Flag::Project(project),
            &SortOrder::Value,
            false, // overdue
            true,  // skip_recurring
        )
        .await
        .expect("should succeed");

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "t2");
        tasks_mock.assert();
    }

    #[tokio::test]
    async fn fetch_deadline_tasks_filters_recurring_and_existing_deadlines() {
        let mut server = mockito::Server::new_async().await;
        // Task with a deadline already set — should be filtered out
        let with_deadline = r#"{"id":"t1","user_id":"910","project_id":"123","section_id":null,"parent_id":null,"added_by_uid":null,"assigned_by_uid":null,"responsible_uid":null,"labels":[],"deadline":{"date":"2026-05-01","lang":"en"},"duration":null,"due":null,"checked":false,"is_deleted":false,"is_collapsed":false,"added_at":"2026-01-01T00:00:00Z","completed_at":null,"updated_at":"2026-01-01T00:00:00Z","priority":1,"child_order":1,"content":"Has deadline","description":"","note_count":0,"day_order":1}"#;
        let without_deadline = task_json("t2", 1);

        let tasks_mock = server
            .mock("GET", "/api/v1/tasks/?project_id=123&limit=200")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(tasks_response_json(&[with_deadline, &without_deadline]))
            .create_async()
            .await;

        let config = config_with_project().await.with_mock_url(server.url());
        let project = config
            .projects()
            .await
            .expect("should fetch projects")
            .into_iter()
            .next()
            .expect("should have at least one project");

        let result = fetch_deadline_tasks(&config, &Flag::Project(project), &SortOrder::Value)
            .await
            .expect("should succeed");

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "t2");
        tasks_mock.assert();
    }

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
