use chrono::DateTime;
use chrono::NaiveDate;
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashSet;
use std::fmt::Display;
use tokio::task::JoinHandle;

pub mod format;
pub mod priority;
use crate::comments::Comment;
use crate::config::Config;
#[cfg(test)]
use crate::config::SortRule;
use crate::config::{SortDirection, SortKey};
use crate::errors::Error;
use crate::input::CONTENT;
use crate::input::DATE_AND_TIME;
use crate::input::DateTimeInput;
use crate::projects;
use crate::tasks::priority::Priority;
use crate::{input, time, todoist};

/// A task returned by the Todoist API.
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct Task {
    pub id: String,
    pub user_id: String,
    pub project_id: String,
    pub section_id: Option<String>,
    pub parent_id: Option<String>,
    pub added_by_uid: Option<String>,
    pub assigned_by_uid: Option<String>,
    pub responsible_uid: Option<String>,
    pub labels: Vec<String>,
    /// Hard deadline date (YYYY-MM-DD).
    pub deadline: Option<Deadline>,
    /// Duration for timeboxing (amount + unit).
    pub duration: Option<Duration>,
    /// Due date and time information.
    pub due: Option<DateInfo>,
    /// Whether the task has been completed.
    pub checked: bool,
    /// Whether the task has been soft-deleted.
    pub is_deleted: bool,
    /// Whether subtasks are collapsed in Todoist UI.
    pub is_collapsed: bool,
    pub added_at: Option<String>,
    pub completed_at: Option<String>,
    pub updated_at: Option<String>,
    pub priority: Priority,
    pub child_order: i16,
    pub content: String,
    pub description: String,
    /// This doesn't seem to be updated by the API
    pub note_count: u32,
    pub day_order: i16,
}

impl Task {
    /// Converts a JSON string to a single task.
    pub fn from_json(json: &str) -> Result<Task, Error> {
        let task: Task = serde_json::from_str(json)?;
        Ok(task)
    }
}

/// Paginated wrapper for a list of tasks.
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct TaskResponse {
    pub results: Vec<Task>,
    pub next_cursor: Option<String>,
}

impl TaskResponse {
    /// Converts a JSON String to a list of multiple tasks (creates a `TaskResponse` from a JSON string)
    pub fn from_json(json: &str) -> Result<TaskResponse, Error> {
        let response: TaskResponse = serde_json::from_str(json)?;
        Ok(response)
    }
}

/// An editable attribute of a task.
#[derive(Eq, PartialEq)]
pub enum TaskAttribute {
    /// Task title text.
    Content,
    /// Task description.
    Description,
    /// Priority level.
    Priority,
    /// Due date.
    Due,
    /// Labels applied to the task.
    Labels,
    /// Hard deadline.
    Deadline,
}
impl Display for TaskAttribute {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskAttribute::Content => write!(f, "Content"),
            TaskAttribute::Description => write!(f, "Description"),
            TaskAttribute::Priority => write!(f, "Priority"),
            TaskAttribute::Due => write!(f, "Due"),
            TaskAttribute::Labels => write!(f, "Labels"),
            TaskAttribute::Deadline => write!(f, "Deadline"),
        }
    }
}

/// Used for selecting which attribute to set or edit in a task
pub fn edit_task_attributes() -> Vec<TaskAttribute> {
    vec![
        TaskAttribute::Content,
        TaskAttribute::Description,
        TaskAttribute::Priority,
        TaskAttribute::Due,
        TaskAttribute::Labels,
        TaskAttribute::Deadline,
    ]
}

/// Returns task attributes available when creating a task (all except Content).
pub fn create_task_attributes() -> Vec<TaskAttribute> {
    vec![
        TaskAttribute::Description,
        TaskAttribute::Priority,
        TaskAttribute::Due,
        TaskAttribute::Labels,
        TaskAttribute::Deadline,
    ]
}

impl Display for Task {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.content)
    }
}

/// A date and time representation from the Todoist API.
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct DateInfo {
    /// Date string as "YYYY-MM-DD" (date) or "YYYY-MM-DDTHH:MM:SSZ" (datetime).
    pub date: String,
    pub is_recurring: bool,
    /// Formatted date display string, e.g. "2025-04-26 15:00".
    pub string: String,
    /// Language code, e.g. "en".
    pub lang: String,
    /// The IANA timezone for this date, e.g. "America/Vancouver".
    pub timezone: Option<String>,
}

impl Display for DateInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.string)
    }
}

/// A hard deadline for a task.
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct Deadline {
    /// Date in format YYYY-MM-DD.
    pub date: String,
    /// Language code, e.g. "en".
    pub lang: String,
}

/// A duration attached to a task, used for timeboxing.
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct Duration {
    /// Number of units.
    pub amount: u32,
    /// Unit of time.
    pub unit: Unit,
}

/// Unit of time for a task duration.
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub enum Unit {
    /// Duration in minutes.
    #[serde(rename = "minute")]
    Minute,
    /// Duration in days.
    #[serde(rename = "day")]
    Day,
}

/// Controls prefix display in terminal output.
pub enum FormatType {
    /// Indented list format (prefix = "- ").
    List,
    /// No prefix, used for a single task.
    Single,
}

enum DateTimeInfo {
    NoDateTime,
    Date {
        date: NaiveDate,
        is_recurring: bool,
        string: String,
    },
    DateTime {
        datetime: DateTime<Tz>,
        is_recurring: bool,
        string: String,
    },
}

/// Specifies how tasks should be sorted.
#[derive(clap::ValueEnum, Debug, Copy, Clone)]
pub enum SortOrder {
    /// Sort by Tod's configured sort order
    Value,
    /// Sort by datetime only
    Datetime,
    /// Leave Todoist's default sorting in place
    Todoist,
}

impl std::fmt::Display for SortOrder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SortOrder::Value => write!(f, "value"),
            SortOrder::Todoist => write!(f, "todoist"),
            SortOrder::Datetime => write!(f, "datetime"),
        }
    }
}

impl Task {
    pub async fn fmt(
        &self,
        comments: Vec<Comment>,
        config: &Config,
        format: FormatType,
        with_project: bool,
    ) -> Result<String, Error> {
        let content = format::content(self, config);
        let buffer = match format {
            FormatType::List => "  ".into(),
            FormatType::Single => String::new(),
        };

        let description = match &*self.description {
            "" => String::new(),
            _ => format!("\n{buffer}{}", self.description),
        };

        let project = if with_project {
            format::project(self, config, &buffer).await?
        } else {
            String::new()
        };
        // Format_task_id returns the same format if urls are disabled
        let url = format::maybe_format_task_id(&self.id, config);

        let due = format::due(self, config, &buffer);
        let prefix = match format {
            FormatType::List => "- ".into(),
            FormatType::Single => String::new(),
        };

        let labels = if self.labels.is_empty() {
            String::new()
        } else {
            format::labels(self)
        };

        let comment_number = match comments.len() {
            0 => String::new(),
            quantity => format::number_comments(quantity),
        };

        let comments = if comments.is_empty() {
            String::new()
        } else {
            format::render_comments(config, comments).await?
        };

        Ok(format!(
            "{prefix}{content}{description}{due}{labels}{comment_number}{project} {url}{comments}\n\n"
        ))
    }

    /// Return the task due date as a sortable datetime.
    fn datetime(&self, config: &Config) -> Option<DateTime<Tz>> {
        match self.datetimeinfo(config) {
            Ok(DateTimeInfo::DateTime { datetime, .. }) => Some(datetime),
            Ok(DateTimeInfo::Date { date, .. }) => {
                let naive_datetime = date.and_hms_opt(23, 59, 00)?;

                // Mirror datetimeinfo()'s per-task timezone resolution:
                // use due.timezone if present, otherwise the config default.
                let tz_string = config.get_timezone().ok()?;
                let tz = match self.due.as_ref().and_then(|due| due.timezone.as_deref()) {
                    None => time::timezone_from_str(&tz_string).ok()?,
                    Some(other_timezone) => time::timezone_from_str(other_timezone).ok()?,
                };

                naive_datetime.and_local_timezone(tz).single()
            }
            Ok(DateTimeInfo::NoDateTime) | Err(_) => None,
        }
    }

    fn deadline_datetime(&self, config: &Config) -> Option<DateTime<Tz>> {
        let Deadline { date, .. } = self.deadline.as_ref()?;
        let date = time::date_string_to_naive_date(date).ok()?;
        let naive_datetime = date.and_hms_opt(23, 59, 00)?;

        let tz_string = config.get_timezone().ok()?;
        let tz = time::timezone_from_str(&tz_string).ok()?;

        naive_datetime.and_local_timezone(tz).single()
    }

    /// Converts the JSON date representation into Date or Datetime
    fn datetimeinfo(&self, config: &Config) -> Result<DateTimeInfo, Error> {
        let tz_string = config.get_timezone()?;
        let tz = match self.due.as_ref().and_then(|due| due.timezone.as_deref()) {
            None => time::timezone_from_str(&tz_string)?,
            Some(other_timezone) => time::timezone_from_str(other_timezone)?,
        };
        match &self.due {
            None => Ok(DateTimeInfo::NoDateTime),
            Some(DateInfo {
                date,
                is_recurring,
                string,
                ..
            }) if date.len() == 10 => Ok(DateTimeInfo::Date {
                date: time::date_from_str(date, tz)?,
                is_recurring: *is_recurring,
                string: string.clone(),
            }),
            Some(DateInfo {
                date,
                is_recurring,
                string,
                ..
            }) => Ok(DateTimeInfo::DateTime {
                datetime: time::datetime_from_str(date, tz)?,
                is_recurring: *is_recurring,
                string: string.clone(),
            }),
        }
    }

    pub fn filter(&self, config: &Config, filter: &projects::TaskFilter) -> bool {
        match filter {
            projects::TaskFilter::Unscheduled => {
                self.has_no_date() || self.is_overdue(config).unwrap_or_default()
            }
            projects::TaskFilter::Overdue => self.is_overdue(config).unwrap_or_default(),
            projects::TaskFilter::Recurring => self.is_recurring(),
        }
    }

    pub fn has_no_date(&self) -> bool {
        self.due.is_none()
    }

    // Returns true if the datetime is today and there is a time
    pub fn is_today(&self, config: &Config) -> Result<bool, Error> {
        let boolean = match self.datetimeinfo(config) {
            Ok(DateTimeInfo::Date { date, .. }) => date == time::naive_date_today(config)?,
            Ok(DateTimeInfo::DateTime { datetime, .. }) => {
                time::datetime_is_today(datetime, config)?
            }
            Ok(DateTimeInfo::NoDateTime) | Err(_) => false,
        };

        Ok(boolean)
    }

    fn is_now(&self, config: &Config) -> bool {
        let Ok(DateTimeInfo::DateTime { datetime, .. }) = self.datetimeinfo(config) else {
            return false;
        };
        let duration = match time::datetime_now(config) {
            Ok(now) => (datetime - now).num_minutes(),
            _ => return false,
        };
        matches!(duration, -15..=15)
    }

    pub fn is_overdue(&self, config: &Config) -> Result<bool, Error> {
        let boolean = match self.datetimeinfo(config) {
            Ok(DateTimeInfo::Date { date, .. }) => time::is_date_in_past(date, config)?,
            Ok(DateTimeInfo::DateTime { datetime, .. }) => {
                time::is_date_in_past(datetime.date_naive(), config)?
            }
            Ok(DateTimeInfo::NoDateTime) | Err(_) => false,
        };

        Ok(boolean)
    }

    /// Returns true if it is a recurring task
    pub fn is_recurring(&self) -> bool {
        self.due
            .as_ref()
            .is_some_and(|DateInfo { is_recurring, .. }| *is_recurring)
    }
}

/// Filters out tasks whose due date is in the future.
pub fn filter_not_in_future(tasks: Vec<Task>, config: &Config) -> Vec<Task> {
    tasks
        .into_iter()
        .filter(|task| {
            task.is_today(config).unwrap_or_default()
                || task.has_no_date()
                || task.is_overdue(config).unwrap_or_default()
        })
        .collect()
}

/// Sorts tasks using either the config sort order or custom sort order.
pub fn sort(tasks: Vec<Task>, config: &Config, sort: SortOrder) -> Vec<Task> {
    match sort {
        SortOrder::Value => sort_by_value(tasks, config),
        SortOrder::Datetime => sort_by_datetime(tasks, config),
        SortOrder::Todoist => tasks,
    }
}

/// Updates a task attribute interactively, returning a join handle for the API call.
pub async fn update_task(
    config: &Config,
    task: &Task,
    attribute: &TaskAttribute,
) -> Result<Option<JoinHandle<()>>, Error> {
    match attribute {
        TaskAttribute::Content => {
            let value = task.content.as_str();

            let new_value = input::string_with_default("Enter new content:", value)?;

            if *value == new_value {
                Ok(None)
            } else {
                let handle = spawn_update_task_content(config.clone(), task.id.clone(), new_value);
                Ok(Some(handle))
            }
        }
        TaskAttribute::Description => {
            let value = task.description.as_str();

            let new_value = input::string_with_default("Enter a new description:", value)?;

            if *value == new_value {
                Ok(None)
            } else {
                let handle =
                    spawn_update_task_description(config.clone(), task.id.clone(), new_value);
                Ok(Some(handle))
            }
        }
        TaskAttribute::Priority => {
            let value = &task.priority;
            let priorities = priority::all_priorities();

            let new_value = input::select("Select your priority:", priorities, &config.mock_select)?;
            if *value == new_value {
                Ok(None)
            } else {
                let handle = spawn_update_task_priority(config.clone(), task.id.clone(), new_value);
                Ok(Some(handle))
            }
        }
        TaskAttribute::Due => spawn_schedule_task(config.clone(), task.clone()).await,
        TaskAttribute::Deadline => spawn_deadline_task(config.clone(), task.clone()).await,
        TaskAttribute::Labels => {
            let label_string = input::string(
                "Enter labels separated by spaces:",
                config.mock_string.clone(),
            )?;

            let labels = label_string
                .split_whitespace()
                .map(std::borrow::ToOwned::to_owned)
                .collect();

            let handle = spawn_update_task_labels(config.clone(), task.id.clone(), labels);
            Ok(Some(handle))
        }
    }
}

/// Applies labels to a task via interactive menu.
pub async fn label_task(
    config: &Config,
    task: Task,
    labels: &[String],
) -> Result<JoinHandle<()>, Error> {
    let comments = Vec::new();
    let text = task.fmt(comments, config, FormatType::Single, true).await?;
    println!("{text}");
    let mut options = labels.to_vec();
    options.push(input::SKIP.to_string());
    let label = input::select("Select label", options, &config.mock_select)?;

    let config = config.clone();
    Ok(tokio::spawn(async move {
        if label.as_str() == input::SKIP {
        } else if let Err(e) = todoist::add_task_label(&config, &task, label, false).await {
            let _ = config.tx().send(e);
        }
    }))
}

/// Walks through tasks one at a time for completion.
pub async fn process_task(
    comments: Vec<Comment>,
    config: &Config,
    task: Task,
    task_count: &mut i32,
    with_project: bool,
) -> Result<Option<JoinHandle<()>>, Error> {
    let options = [
        input::COMPLETE,
        input::SKIP,
        input::SCHEDULE,
        input::COMMENT,
        input::REMIND,
        input::DELETE,
        input::QUIT,
    ]
    .iter()
    .map(std::string::ToString::to_string)
    .collect();
    let formatted_task = task
        .fmt(comments, config, FormatType::Single, with_project)
        .await?;
    let mut reloaded_config = config.reload().await?.increment_completed()?;
    let tasks_completed = reloaded_config.tasks_completed()?;
    println!("{formatted_task}{tasks_completed} completed today, {task_count} remaining");
    *task_count -= 1;
    let selection = input::select(input::OPTION, options, &config.mock_select)?;
    match selection.as_str() {
        input::COMPLETE => {
            if let Err(e) = reloaded_config.save().await {
                eprintln!("Could not save config: {e}");
            }
            Ok(Some(spawn_complete_task(reloaded_config, task.id)))
        }
        input::DELETE => Ok(Some(spawn_delete_task(config.clone(), task.id))),
        input::COMMENT => {
            let content = input::string(CONTENT, config.mock_string.clone())?;

            Ok(Some(spawn_comment_task(config.clone(), task.id, content)))
        }

        input::REMIND => {
            let content = input::string(DATE_AND_TIME, config.mock_string.clone())?;

            Ok(Some(spawn_create_reminder(config.clone(), task, content)))
        }

        input::SCHEDULE => {
            let date = input::date()?;
            Ok(Some(spawn_update_task_due(
                config.clone(),
                task,
                date,
                None,
            )))
        }
        input::SKIP => {
            // Do nothing
            Ok(Some(tokio::spawn(async move {})))
        }
        input::QUIT => Ok(None),
        _ => {
            unreachable!()
        }
    }
}

/// Assigns a date, time, and duration to a task via interactive prompts.
pub async fn timebox_task(
    config: &Config,
    task: Task,
    task_count: &mut i32,
    with_project: bool,
) -> Result<Option<JoinHandle<()>>, Error> {
    let options = [
        input::TIMEBOX,
        input::COMPLETE,
        input::SKIP,
        input::DELETE,
        input::QUIT,
    ]
    .iter()
    .map(std::string::ToString::to_string)
    .collect();
    let comments = Vec::new();
    let formatted_task = task
        .fmt(comments, config, FormatType::Single, with_project)
        .await?;
    println!("{formatted_task}{task_count} task(s) remaining");
    *task_count -= 1;
    let selection = input::select("Select an option", options, &config.mock_select)?;
    match selection.as_str() {
        input::TIMEBOX => {
            let (due_string, duration) = get_timebox(config, &task)?;

            Ok(Some(spawn_update_task_due(
                config.clone(),
                task,
                due_string,
                Some(duration),
            )))
        }

        input::DELETE => Ok(Some(spawn_delete_task(config.clone(), task.id))),
        input::COMPLETE => Ok(Some(spawn_complete_task(config.clone(), task.id))),
        input::SKIP => {
            // Do nothing
            Ok(Some(tokio::spawn(async move {})))
        }
        input::QUIT => {
            // The quit clause
            Ok(None)
        }
        _ => {
            unreachable!()
        }
    }
}

/// Returns a (due_string, duration_minutes) pair for timeboxing a task.
/// Uses the task's existing date/time if available, otherwise prompts. Always prompts for the duration.
fn get_timebox(config: &Config, task: &Task) -> Result<(String, u32), Error> {
    let datetime = if let Task {
        due: Some(DateInfo { date, .. }),
        ..
    } = task
    {
        if time::is_date(date) {
            let time = input::string(input::TIME, config.mock_string.clone())?;

            format!("{date} {time}")
        } else {
            let timezone = config.get_timezone()?;
            let tz = time::timezone_from_str(&timezone)?;
            time::datetime_from_str(date, tz)?
                .format(time::FORMAT_DATE_AND_TIME)
                .to_string()
        }
    } else {
        let date = input::date()?;
        let time = input::string(input::TIME, config.mock_string.clone())?;
        format!("{date} {time}")
    };

    let duration = input::string(input::DURATION, config.mock_string.clone())?;

    Ok((datetime, duration.parse::<u32>()?))
}

/// Schedules a task's due date inside a spawned thread.
pub async fn spawn_schedule_task(
    config: Config,
    task: Task,
) -> Result<Option<JoinHandle<()>>, Error> {
    let comments = Vec::new();
    let text = task
        .fmt(comments, &config, FormatType::Single, true)
        .await?;
    println!("{text}");
    let datetime_input = input::datetime(
        &config.mock_select,
        config.mock_string.clone(),
        config.natural_language_only,
        false,
        true,
    )?;
    match datetime_input {
        input::DateTimeInput::Complete => {
            let handle = spawn_complete_task(config, task.id);
            Ok(Some(handle))
        }
        DateTimeInput::Skip => Ok(None),

        input::DateTimeInput::Text(due_string) => {
            let handle = spawn_update_task_due(config, task, due_string, None);
            Ok(Some(handle))
        }
        input::DateTimeInput::None => {
            let handle = spawn_update_task_due(config, task, "No date".to_string(), None);
            Ok(Some(handle))
        }
    }
}
/// Sets a task's deadline inside a spawned thread.
pub async fn spawn_deadline_task(
    config: Config,
    task: Task,
) -> Result<Option<JoinHandle<()>>, Error> {
    let comments = Vec::new();
    let text = task
        .fmt(comments, &config, FormatType::Single, true)
        .await?;
    println!("{text}");
    let datetime_input = input::datetime(
        &config.mock_select,
        config.mock_string.clone(),
        config.natural_language_only,
        true,
        true,
    )?;
    match datetime_input {
        input::DateTimeInput::Complete => {
            let handle = spawn_complete_task(config, task.id);
            Ok(Some(handle))
        }
        DateTimeInput::Skip => Ok(None),

        input::DateTimeInput::Text(date) => {
            let handle = spawn_update_task_deadline(config, task.id, Some(date));
            Ok(Some(handle))
        }
        input::DateTimeInput::None => {
            let handle = spawn_update_task_deadline(config, task.id, None);
            Ok(Some(handle))
        }
    }
}

/// Completes task inside another thread
pub fn spawn_complete_task(config: Config, task_id: String) -> JoinHandle<()> {
    tokio::spawn(async move {
        if let Err(e) = todoist::complete_task(&config, &task_id, false).await {
            let _ = config.tx().send(e);
        }
    })
}

/// Deletes task inside another thread
pub fn spawn_delete_task(config: Config, task_id: String) -> JoinHandle<()> {
    tokio::spawn(async move {
        if let Err(e) = todoist::delete_task(&config, &task_id, false).await {
            let _ = config.tx().send(e);
        }
    })
}

/// Updates task inside another thread
pub fn spawn_update_task_due(
    config: Config,
    task: Task,
    due_string: String,
    duration: Option<u32>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        if let Err(e) =
            todoist::update_task_due_natural_language(&config, &task, due_string, duration, false)
                .await
        {
            let _ = config.tx().send(e);
        }
    })
}

/// creates a reminder inside another thread
pub fn spawn_create_reminder(config: Config, task: Task, due_string: String) -> JoinHandle<()> {
    tokio::spawn(async move {
        if let Err(e) = todoist::create_reminder(&config, &task, &due_string, false).await {
            let _ = config.tx().send(e);
        }
    })
}

/// Updates task inside another thread
pub fn spawn_update_task_deadline(
    config: Config,
    task_id: String,
    date: Option<String>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        if let Err(e) = todoist::update_task_deadline(&config, &task_id, date, false).await {
            let _ = config.tx().send(e);
        }
    })
}

/// Updates task inside another thread
pub fn spawn_comment_task(config: Config, task_id: String, task_comment: String) -> JoinHandle<()> {
    tokio::spawn(async move {
        if let Err(e) = todoist::create_comment(&config, &task_id, &task_comment, false).await {
            let _ = config.tx().send(e);
        }
    })
}

/// Updates task inside another thread
pub fn spawn_update_task_content(
    config: Config,
    task_id: String,
    content: String,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        if let Err(e) = todoist::update_task_content(&config, &task_id, &content, false).await {
            let _ = config.tx().send(e);
        }
    })
}

/// Updates task inside another thread
pub fn spawn_update_task_description(
    config: Config,
    task_id: String,
    description: String,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        if let Err(e) =
            todoist::update_task_description(&config, &task_id, &description, false).await
        {
            let _ = config.tx().send(e);
        }
    })
}

/// Updates task inside another thread
pub fn spawn_update_task_labels(
    config: Config,
    task_id: String,
    labels: Vec<String>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        if let Err(e) = todoist::update_task_labels(&config, &task_id, labels, false).await {
            let _ = config.tx().send(e);
        }
    })
}

/// Updates task inside another thread
pub fn spawn_update_task_priority(
    config: Config,
    task_id: String,
    priority: Priority,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        if let Err(e) = todoist::update_task_priority(&config, &task_id, &priority, false).await {
            let _ = config.tx().send(e);
        }
    })
}

/// Sorts tasks by configured sort key and direction.
pub fn sort_by_value(mut tasks: Vec<Task>, config: &Config) -> Vec<Task> {
    tasks.sort_by(|a, b| compare_by_sort_order(a, b, config));
    tasks
}

fn compare_by_sort_order(a: &Task, b: &Task, config: &Config) -> Ordering {
    for rule in config.sort_order.as_deref().unwrap_or_default() {
        let ordering = compare_by_sort_key(a, b, config, rule.key);
        if ordering != Ordering::Equal {
            return match rule.direction {
                SortDirection::Asc => ordering,
                SortDirection::Desc => ordering.reverse(),
            };
        }
    }

    Ordering::Equal
}

fn compare_by_sort_key(a: &Task, b: &Task, config: &Config, key: SortKey) -> Ordering {
    match key {
        SortKey::Priority => a.priority.to_integer().cmp(&b.priority.to_integer()),
        SortKey::DueDate => compare_datetime(a.datetime(config), b.datetime(config)),
        SortKey::Overdue => a
            .is_overdue(config)
            .unwrap_or_default()
            .cmp(&b.is_overdue(config).unwrap_or_default()),
        SortKey::Today => a
            .is_today(config)
            .unwrap_or_default()
            .cmp(&b.is_today(config).unwrap_or_default()),
        SortKey::Now => a.is_now(config).cmp(&b.is_now(config)),
        SortKey::NoDueDate => a.has_no_date().cmp(&b.has_no_date()),
        SortKey::NotRecurring => (!a.is_recurring()).cmp(&(!b.is_recurring())),
        SortKey::Deadline => {
            compare_datetime(a.deadline_datetime(config), b.deadline_datetime(config))
        }
        SortKey::Order => a.child_order.cmp(&b.child_order),
    }
}

fn compare_datetime(a: Option<DateTime<Tz>>, b: Option<DateTime<Tz>>) -> Ordering {
    match (a, b) {
        (Some(a), Some(b)) => a.cmp(&b),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

/// Sorts tasks by their computed datetime.
pub fn sort_by_datetime(mut tasks: Vec<Task>, config: &Config) -> Vec<Task> {
    tasks.sort_by_key(|i| i.datetime(config));
    tasks
}

/// Filters out checked tasks, parent tasks with unchecked children, and children
/// whose parents are in the future.
pub async fn reject_parent_tasks(tasks: Vec<Task>, config: &Config) -> Vec<Task> {
    let parent_ids = tasks
        .iter()
        .filter(|task| task.parent_id.is_some() && !task.checked)
        .filter_map(|task| task.parent_id.clone())
        .collect::<HashSet<String>>();
    let task_ids = tasks
        .iter()
        .map(|task| task.id.clone())
        .collect::<HashSet<String>>();

    // Pre-fetch distinct parent tasks missing from the current set so siblings
    // sharing the same absent parent don't each trigger a redundant API call.
    let missing_parent_ids: HashSet<&String> = parent_ids
        .iter()
        .filter(|id| !task_ids.contains(*id))
        .collect();
    let mut future_parents = std::collections::HashMap::new();
    for parent_id in &missing_parent_ids {
        match todoist::get_task(config, parent_id).await {
            Err(e) => {
                let _ = config.clone().tx().send(e);
            }
            Ok(parent) => {
                let is_future = !(parent.is_overdue(config).unwrap_or_default()
                    || parent.has_no_date()
                    || parent.is_today(config).unwrap_or_default());
                future_parents.insert((*parent_id).clone(), is_future);
            }
        }
    }

    let mut filtered_tasks = Vec::new();
    for task in tasks {
        if !parent_ids.contains(&task.id) && !task.checked {
            let parent_is_future = task
                .parent_id
                .as_ref()
                .and_then(|pid| future_parents.get(pid).copied())
                .unwrap_or(false);
            if !parent_is_future {
                filtered_tasks.push(task);
            }
        }
    }

    filtered_tasks
}

/// Sets task priority via interactive menu.
pub async fn set_priority(
    config: &Config,
    task: Task,
    with_project: bool,
) -> Result<JoinHandle<()>, Error> {
    let comments = Vec::new();
    let text = task
        .fmt(comments, config, FormatType::Single, with_project)
        .await?;
    println!("{text}");

    let options = vec![
        Priority::None,
        Priority::Low,
        Priority::Medium,
        Priority::High,
    ];
    let priority = input::select(input::PRIORITY, options, &config.mock_select)?;

    let config = config.clone();
    Ok(tokio::spawn(async move {
        if let Err(e) = todoist::update_task_priority(&config, &task.id, &priority, false).await {
            let _ = config.tx().send(e);
        }
    }))
}

/// Creates a reminder for a task using natural-language date input.
pub async fn create_reminder(config: &Config, task: Task) -> Result<Option<JoinHandle<()>>, Error> {
    let comments = Vec::new();
    let text = task.fmt(comments, config, FormatType::Single, true).await?;
    println!("{text}");
    let datetime_input = input::datetime(
        &config.mock_select,
        config.mock_string.clone(),
        // We only want to use natural language for this input
        Some(true),
        true,
        true,
    )?;
    let config = config.clone();
    match datetime_input {
        input::DateTimeInput::Complete => {
            let handle = spawn_complete_task(config, task.id);
            Ok(Some(handle))
        }
        DateTimeInput::Skip | input::DateTimeInput::None => Ok(None),

        input::DateTimeInput::Text(date) => {
            let handle = spawn_create_reminder(config, task, date);
            Ok(Some(handle))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test::{self, responses::ResponseFromFile};
    use pretty_assertions::assert_eq;
    use serde_test::{Token, assert_de_tokens};

    #[test]
    fn unit_deserializes_with_serde_tokens() {
        assert_de_tokens(
            &Unit::Minute,
            &[Token::UnitVariant {
                name: "Unit",
                variant: "minute",
            }],
        );
        assert_de_tokens(
            &Unit::Day,
            &[Token::UnitVariant {
                name: "Unit",
                variant: "day",
            }],
        );
    }

    #[tokio::test]
    async fn test_task_from_json_valid() {
        let json = ResponseFromFile::TodayTask.read().await;
        let task = Task::from_json(&json).expect("should parse TodayTask JSON");
        assert_eq!(task.content, "TEST");
        assert_eq!(task.user_id, "910");
    }

    #[test]
    fn test_task_from_json_invalid() {
        let result = Task::from_json("not json");
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_task_response_from_json_valid() {
        let json = ResponseFromFile::TodayTasks.read().await;
        let response = TaskResponse::from_json(&json).expect("should parse TodayTasks JSON");
        assert!(!response.results.is_empty());
    }

    #[test]
    fn test_task_response_from_json_invalid() {
        let result = TaskResponse::from_json("not json");
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_sort_order_display() {
        assert_eq!(SortOrder::Value.to_string(), "value");
        assert_eq!(SortOrder::Datetime.to_string(), "datetime");
        assert_eq!(SortOrder::Todoist.to_string(), "todoist");
    }

    #[tokio::test]
    async fn test_sort_todoist_preserves_order() {
        let config = test::fixtures::config().await;
        let t1 = test::fixtures::today_task().await;
        let t2 = Task {
            id: "other".into(),
            ..test::fixtures::today_task().await
        };
        let tasks = vec![t1.clone(), t2.clone()];
        let sorted = sort(tasks.clone(), &config, SortOrder::Todoist);
        assert_eq!(sorted, tasks);
    }

    #[tokio::test]
    async fn test_filter_not_in_future_keeps_today_and_overdue() {
        let config = test::fixtures::config().await;
        let today = test::fixtures::today_task().await;
        let overdue = Task {
            id: "overdue-id".into(),
            due: Some(DateInfo {
                date: "2020-01-01".into(),
                is_recurring: false,
                lang: "en".into(),
                string: "2020-01-01".into(),
                timezone: None,
            }),
            ..today.clone()
        };
        let future = Task {
            id: "future-id".into(),
            due: Some(DateInfo {
                date: "2099-12-31".into(),
                is_recurring: false,
                lang: "en".into(),
                string: "2099-12-31".into(),
                timezone: None,
            }),
            ..today.clone()
        };
        let tasks = vec![today.clone(), overdue.clone(), future.clone()];
        let result = filter_not_in_future(tasks, &config);
        // today and overdue should remain; future should be filtered
        assert!(result.iter().any(|t| t.id == today.id));
        assert!(result.iter().any(|t| t.id == "overdue-id"));
        assert!(!result.iter().any(|t| t.id == "future-id"));
    }

    #[tokio::test]
    async fn test_filter_not_in_future_keeps_no_date() {
        let config = test::fixtures::config().await;
        let no_date = Task {
            due: None,
            ..test::fixtures::today_task().await
        };
        let result = filter_not_in_future(vec![no_date.clone()], &config);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, no_date.id);
    }

    #[tokio::test]
    async fn test_is_recurring_true() {
        let task = Task {
            due: Some(DateInfo {
                date: "2024-01-01".into(),
                is_recurring: true,
                lang: "en".into(),
                string: "every day".into(),
                timezone: None,
            }),
            ..test::fixtures::today_task().await
        };
        assert!(task.is_recurring());
    }

    #[tokio::test]
    async fn test_is_recurring_false_no_due() {
        let task = Task {
            due: None,
            ..test::fixtures::today_task().await
        };
        assert!(!task.is_recurring());
    }

    #[tokio::test]
    async fn test_is_recurring_false_non_recurring_due() {
        let task = Task {
            due: Some(DateInfo {
                date: "2024-01-01".into(),
                is_recurring: false,
                lang: "en".into(),
                string: "2024-01-01".into(),
                timezone: None,
            }),
            ..test::fixtures::today_task().await
        };
        assert!(!task.is_recurring());
    }

    #[tokio::test]
    async fn test_edit_task_attributes_contains_all() {
        let attrs = edit_task_attributes();
        assert!(attrs.contains(&TaskAttribute::Content));
        assert!(attrs.contains(&TaskAttribute::Description));
        assert!(attrs.contains(&TaskAttribute::Priority));
        assert!(attrs.contains(&TaskAttribute::Due));
        assert!(attrs.contains(&TaskAttribute::Labels));
        assert!(attrs.contains(&TaskAttribute::Deadline));
    }

    #[tokio::test]
    async fn test_create_task_attributes_contains_expected() {
        let attrs = create_task_attributes();
        assert!(attrs.contains(&TaskAttribute::Description));
        assert!(attrs.contains(&TaskAttribute::Priority));
        assert!(attrs.contains(&TaskAttribute::Due));
        assert!(attrs.contains(&TaskAttribute::Labels));
        assert!(attrs.contains(&TaskAttribute::Deadline));
        // Content should not be in create (it's set at creation time)
        assert!(!attrs.contains(&TaskAttribute::Content));
    }

    #[tokio::test]
    async fn test_task_attribute_display() {
        assert_eq!(TaskAttribute::Content.to_string(), "Content");
        assert_eq!(TaskAttribute::Description.to_string(), "Description");
        assert_eq!(TaskAttribute::Priority.to_string(), "Priority");
        assert_eq!(TaskAttribute::Due.to_string(), "Due");
        assert_eq!(TaskAttribute::Labels.to_string(), "Labels");
        assert_eq!(TaskAttribute::Deadline.to_string(), "Deadline");
    }

    #[tokio::test]
    async fn can_format_task_with_a_date() {
        let config = test::fixtures::config().await;
        let task = Task {
            content: "Get gifts for the twins".into(),
            due: Some(DateInfo {
                date: "2021-08-13".into(),
                ..test::fixtures::today_task()
                    .await
                    .due
                    .expect("Failed to unwrap due field in test fixtures for today_task")
            }),
            ..test::fixtures::today_task().await
        };
        let comments = Vec::new();

        let task = task
            .fmt(comments, &config, FormatType::Single, false)
            .await
            .expect("expected value or result, got None or Err");

        assert!(task.contains("Get gifts for the twins"));
        assert!(task.contains("2021-08-13"));
    }

    #[tokio::test]
    async fn can_format_task_with_today() {
        let config = test::fixtures::config().await;
        let task = Task {
            content: "Get gifts for the twins".into(),
            due: Some(DateInfo {
                date: time::date_string_today(&config)
                    .expect("Failed to unwrap date_string_today result in tasks test"),
                ..test::fixtures::today_task()
                    .await
                    .due
                    .expect("Failed to unwrap due field in test fixtures for today_task")
            }),
            ..test::fixtures::today_task().await
        };
        let comments = vec![test::fixtures::comment()];

        let task_text = task
            .fmt(comments, &config, FormatType::Single, true)
            .await
            .expect("expected value or result, got None or Err");

        assert!(task_text.contains("Today @ computer"));
    }

    #[tokio::test]
    async fn datetime_works_with_date() {
        let config = test::fixtures::config().await;
        let task = Task {
            due: Some(DateInfo {
                date: time::date_string_today(&config)
                    .expect("Failed to unwrap date_string_today result in tasks test"),
                ..test::fixtures::today_task()
                    .await
                    .due
                    .expect("Failed to unwrap due field in test fixtures for today_task")
            }),
            ..test::fixtures::today_task().await
        };

        assert!(task.datetime(&config).is_some());
    }

    #[tokio::test]
    async fn datetime_date_only_uses_config_timezone() {
        let config = test::fixtures::config().await.with_timezone("Asia/Tokyo");
        let task = Task {
            due: Some(DateInfo {
                date: "2025-05-10".into(),
                is_recurring: false,
                lang: "en".into(),
                timezone: None,
                string: "2025-05-10".into(),
            }),
            ..test::fixtures::today_task().await
        };

        assert_eq!(
            task.datetime(&config)
                .expect("date-only task should have a sortable datetime")
                .to_rfc3339(),
            "2025-05-10T23:59:00+09:00"
        );
    }

    #[tokio::test]
    async fn has_no_date_works() {
        let config = test::fixtures::config().await;
        let task = Task {
            due: None,
            ..test::fixtures::today_task().await
        };

        assert!(task.has_no_date());

        let task_today = Task {
            due: Some(DateInfo {
                date: time::date_string_today(&config)
                    .expect("Failed to unwrap date_string_today result in tasks test"),
                ..test::fixtures::today_task()
                    .await
                    .due
                    .expect("Failed to unwrap due field in test fixtures for today_task")
            }),
            ..test::fixtures::today_task().await
        };
        assert!(!task_today.has_no_date());
    }

    #[tokio::test]
    async fn is_today_works() {
        let config = test::fixtures::config().await;
        let task = Task {
            due: None,
            ..test::fixtures::today_task().await
        };

        assert!(
            !task
                .is_today(&config)
                .expect("expected value or result, got None or Err")
        );

        let task_today = Task {
            due: Some(DateInfo {
                date: time::date_string_today(&config)
                    .expect("Failed to unwrap date_string_today result in tasks test"),
                lang: "en".into(),
                is_recurring: false,
                string: "Every 2 weeks".into(),
                timezone: None,
            }),
            ..test::fixtures::today_task().await
        };
        assert!(
            task_today
                .is_today(&config)
                .expect("expected value or result, got None or Err")
        );

        let task_in_past = Task {
            due: Some(DateInfo {
                date: "2021-09-06T16:00:00".into(),
                is_recurring: false,
                lang: "en".into(),
                timezone: None,
                string: "Every 2 weeks".into(),
            }),
            ..test::fixtures::today_task().await
        };
        assert!(
            !task_in_past
                .is_today(&config)
                .expect("expected value or result, got None or Err")
        );
    }

    #[tokio::test]
    async fn is_now_handles_15_minute_boundaries() {
        let config = test::fixtures::config().await;
        let base_task = test::fixtures::today_task().await;

        let minus_fifteen = Task {
            due: Some(DateInfo {
                date: "2025-05-10T02:45:00".into(),
                is_recurring: false,
                lang: "en".into(),
                timezone: None,
                string: "2025-05-10 02:45".into(),
            }),
            ..base_task.clone()
        };
        let plus_fifteen = Task {
            due: Some(DateInfo {
                date: "2025-05-10T03:15:00".into(),
                is_recurring: false,
                lang: "en".into(),
                timezone: None,
                string: "2025-05-10 03:15".into(),
            }),
            ..base_task.clone()
        };
        let outside_minus = Task {
            due: Some(DateInfo {
                date: "2025-05-10T02:44:00".into(),
                is_recurring: false,
                lang: "en".into(),
                timezone: None,
                string: "2025-05-10 02:44".into(),
            }),
            ..base_task.clone()
        };
        let outside_plus = Task {
            due: Some(DateInfo {
                date: "2025-05-10T03:16:00".into(),
                is_recurring: false,
                lang: "en".into(),
                timezone: None,
                string: "2025-05-10 03:16".into(),
            }),
            ..base_task
        };

        assert!(minus_fifteen.is_now(&config));
        assert!(plus_fifteen.is_now(&config));
        assert!(!outside_minus.is_now(&config));
        assert!(!outside_plus.is_now(&config));
    }

    #[tokio::test]
    async fn deadline_datetime_orders_correctly_in_positive_offset_timezone() {
        let config = test::fixtures::config().await.with_timezone("Asia/Tokyo");
        let base_task = test::fixtures::today_task().await;

        let earlier_deadline = Task {
            deadline: Some(Deadline {
                date: "2025-05-10".into(),
                lang: "en".into(),
            }),
            ..base_task.clone()
        };
        let later_deadline = Task {
            deadline: Some(Deadline {
                date: "2025-05-11".into(),
                lang: "en".into(),
            }),
            ..base_task
        };

        assert_eq!(
            compare_datetime(
                earlier_deadline.deadline_datetime(&config),
                later_deadline.deadline_datetime(&config),
            ),
            Ordering::Less
        );
    }

    #[tokio::test]
    async fn sort_by_value_works() {
        let config = test::fixtures::config().await;
        let today = Task {
            due: Some(DateInfo {
                date: time::date_string_today(&config)
                    .expect("Failed to unwrap date_string_today result in tasks test"),
                lang: "en".into(),
                is_recurring: false,
                timezone: None,
                string: "Every 2 weeks".into(),
            }),
            ..test::fixtures::today_task().await
        };

        let today_recurring = Task {
            due: Some(DateInfo {
                date: time::date_string_today(&config)
                    .expect("Failed to unwrap date_string_today result in tasks test"),
                is_recurring: false,
                lang: "en".into(),
                string: "Every 2 weeks".into(),
                timezone: None,
            }),
            ..test::fixtures::today_task().await
        };

        let future = Task {
            due: Some(DateInfo {
                date: "2035-12-12".into(),
                is_recurring: false,
                lang: "en".into(),
                string: "Every 2 weeks".into(),
                timezone: None,
            }),
            ..test::fixtures::today_task().await
        };

        let input = vec![future.clone(), today_recurring.clone(), today.clone()];
        let result = vec![today, today_recurring, future];

        assert_eq!(sort_by_value(input, &config), result);
    }

    #[tokio::test]
    async fn sort_by_value_preserves_api_order_after_configured_keys() {
        let mut config = test::fixtures::config().await;
        config.sort_order = Some(vec![SortRule::new(SortKey::Priority, SortDirection::Desc)]);

        let low_early = Task {
            id: "low-early".into(),
            due: Some(DateInfo {
                date: "2030-01-01".into(),
                is_recurring: false,
                lang: "en".into(),
                string: "2030-01-01".into(),
                timezone: None,
            }),
            priority: Priority::Low,
            ..test::fixtures::today_task().await
        };
        let high_late = Task {
            id: "high-late".into(),
            due: Some(DateInfo {
                date: "2030-12-31".into(),
                is_recurring: false,
                lang: "en".into(),
                string: "2030-12-31".into(),
                timezone: None,
            }),
            priority: Priority::High,
            ..test::fixtures::today_task().await
        };
        let high_early = Task {
            id: "high-early".into(),
            due: Some(DateInfo {
                date: "2030-01-01".into(),
                is_recurring: false,
                lang: "en".into(),
                string: "2030-01-01".into(),
                timezone: None,
            }),
            priority: Priority::High,
            ..test::fixtures::today_task().await
        };

        let sorted = sort_by_value(
            vec![low_early.clone(), high_late.clone(), high_early.clone()],
            &config,
        );

        assert_eq!(sorted, vec![high_late, high_early, low_early]);
    }

    #[tokio::test]
    async fn sort_by_value_uses_configured_direction() {
        let mut config = test::fixtures::config().await;
        config.sort_order = Some(vec![SortRule::new(SortKey::Order, SortDirection::Asc)]);

        let second = Task {
            id: "second".into(),
            child_order: 2,
            ..test::fixtures::today_task().await
        };
        let first = Task {
            id: "first".into(),
            child_order: 1,
            ..test::fixtures::today_task().await
        };

        assert_eq!(
            sort_by_value(vec![second.clone(), first.clone()], &config),
            vec![first.clone(), second.clone()]
        );

        config.sort_order = Some(vec![SortRule::new(SortKey::Order, SortDirection::Desc)]);
        assert_eq!(
            sort_by_value(vec![first.clone(), second.clone()], &config),
            vec![second, first]
        );
    }

    #[tokio::test]
    async fn sort_by_datetime_works() {
        let config = test::fixtures::config().await;
        let no_date = Task {
            id: "222".into(),
            section_id: None,
            user_id: "222".into(),
            content: "Get gifts for the twins".into(),
            checked: false,
            child_order: 0,
            day_order: 0,
            updated_at: None,
            deadline: None,
            completed_at: None,
            added_at: None,
            added_by_uid: None,
            responsible_uid: None,
            assigned_by_uid: None,
            note_count: 0,
            is_collapsed: false,
            parent_id: None,
            project_id: "123".into(),
            description: String::new(),
            duration: Some(Duration {
                amount: 123,
                unit: Unit::Minute,
            }),
            due: None,
            labels: vec!["computer".into()],
            priority: Priority::Medium,
            is_deleted: false,
        };

        let date_not_datetime = Task {
            due: Some(DateInfo {
                date: time::date_string_today(&config)
                    .expect("Failed to unwrap date_string_today result in tasks test"),
                is_recurring: false,
                lang: "en".into(),
                string: "Every 2 weeks".into(),
                timezone: None,
            }),
            ..no_date.clone()
        };

        let present = Task {
            due: Some(DateInfo {
                date: "2020-09-06T16:00:00".into(),
                is_recurring: false,
                lang: "en".into(),
                string: "Every 2 weeks".into(),
                timezone: None,
            }),
            ..no_date.clone()
        };

        let future = Task {
            due: Some(DateInfo {
                date: "2035-09-06T16:00:00".into(),
                string: "Every 2 weeks".into(),
                lang: "en".into(),
                is_recurring: false,
                timezone: None,
            }),
            ..no_date.clone()
        };

        let past = Task {
            due: Some(DateInfo {
                date: "2015-09-06T16:00:00".into(),
                lang: "en".into(),
                is_recurring: false,
                string: "Every 2 weeks".into(),
                timezone: None,
            }),
            ..no_date.clone()
        };

        let input = vec![
            future.clone(),
            present.clone(),
            past.clone(),
            no_date.clone(),
            date_not_datetime.clone(),
        ];
        let result = vec![no_date, past, present, date_not_datetime, future];

        assert_eq!(sort_by_datetime(input, &config), result);
    }

    #[tokio::test]
    async fn is_overdue_works() {
        let config = test::fixtures::config().await;
        let task = Task {
            id: "222".into(),
            section_id: None,
            added_by_uid: None,
            responsible_uid: None,
            assigned_by_uid: None,
            added_at: None,
            is_collapsed: false,
            user_id: "222".into(),
            checked: false,
            child_order: 0,
            day_order: 0,
            deadline: None,
            updated_at: None,
            duration: None,
            completed_at: None,
            parent_id: None,
            note_count: 1,
            content: "Get gifts for the twins".into(),
            description: String::new(),
            project_id: "123".into(),
            labels: vec!["computer".into()],
            due: None,
            priority: Priority::Medium,
            is_deleted: false,
        };

        assert!(
            !task
                .is_overdue(&config)
                .expect("expected value or result, got None or Err")
        );

        let task_today = Task {
            due: Some(DateInfo {
                date: time::date_string_today(&config)
                    .expect("Failed to unwrap date_string_today result in tasks test"),
                lang: "en".into(),
                string: "Every 2 weeks".into(),
                is_recurring: false,
                timezone: None,
            }),
            ..task.clone()
        };
        assert!(
            !task_today
                .is_overdue(&config)
                .expect("expected value or result, got None or Err")
        );

        let task_future = Task {
            due: Some(DateInfo {
                date: "2035-12-12".into(),
                lang: "en".into(),
                is_recurring: false,
                string: "Every 2 weeks".into(),
                timezone: None,
            }),
            ..task.clone()
        };
        assert!(
            !task_future
                .is_overdue(&config)
                .expect("expected value or result, got None or Err")
        );

        let task_today = Task {
            due: Some(DateInfo {
                date: "2020-12-20".into(),
                lang: "en".into(),
                is_recurring: false,
                string: "Every 2 weeks".into(),
                timezone: None,
            }),
            ..task
        };
        assert!(
            task_today
                .is_overdue(&config)
                .expect("expected value or result, got None or Err")
        );
    }

    #[test]
    fn test_to_integer() {
        assert_eq!(Priority::None.to_integer(), 1);
        assert_eq!(Priority::Low.to_integer(), 2);
        assert_eq!(Priority::Medium.to_integer(), 3);
        assert_eq!(Priority::High.to_integer(), 4);
    }

    #[tokio::test]
    async fn test_set_priority() {
        let task = test::fixtures::today_task().await;
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/api/v1/tasks/6Xqhv4cwxgjwG9w8")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::TodayTask.read().await)
            .create_async()
            .await;
        let config = test::fixtures::config()
            .await
            .mock_select(1)
            .with_mock_url(server.url());

        let future = set_priority(&config, task, false)
            .await
            .expect("expected value or result, got None or Err");

        tokio::join!(future)
            .0
            .expect("expected value or result, got None or Err");
        mock.assert();
    }

    #[tokio::test]
    async fn test_process_task() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/api/v1/tasks/6Xqhv4cwxgjwG9w8/close")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::TodayTask.read().await)
            .create_async()
            .await;

        let task = test::fixtures::today_task().await;
        let config = test::fixtures::config()
            .await
            .with_mock_url(server.url())
            .mock_select(0)
            .create()
            .await
            .expect("expected value or result, got None or Err");

        let mut task_count = 3;
        let comments = Vec::new();
        process_task(comments, &config, task, &mut task_count, true)
            .await
            .expect("expected value or result, got None or Err")
            .expect("expected value or result, got None or Err")
            .await
            .expect("expected value or result, got None or Err");
        mock.assert();
    }

    #[tokio::test]
    async fn test_process_task_remind() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/api/v1/reminders")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{
                    "id": "abc",
                    "item_id": "6Xqhv4cwxgjwG9w8",
                    "notify_uid": "635166",
                    "type": "relative",
                    "is_deleted": false,
                    "minute_offset": 0,
                    "is_urgent": false,
                    "due": {
                        "date": "2026-01-18T17:00:00",
                        "timezone": null,
                        "string": "2026-01-18 17:00",
                        "lang": "en",
                        "is_recurring": false
                    }
                }"#,
            )
            .create_async()
            .await;

        let task = test::fixtures::today_task().await;
        let config = test::fixtures::config()
            .await
            .with_mock_url(server.url())
            .with_mock_string("tomorrow at 5pm")
            .mock_select(4)
            .create()
            .await
            .expect("expected value or result, got None or Err");

        let mut task_count = 3;
        let comments = Vec::new();
        process_task(comments, &config, task, &mut task_count, true)
            .await
            .expect("expected value or result, got None or Err")
            .expect("expected value or result, got None or Err")
            .await
            .expect("expected value or result, got None or Err");
        mock.assert();
    }

    #[tokio::test]
    async fn test_display_task() {
        let task = test::fixtures::today_task().await;
        let string = String::from("TEST");
        assert_eq!(string, task.to_string());
    }

    // ── reject_parent_tasks ──────────────────────────────────────────

    #[tokio::test]
    async fn test_reject_parent_tasks_siblings_share_single_api_call() {
        // Two children with the same absent parent that is in the future.
        // The memoization in reject_parent_tasks should call get_task
        // exactly once, not once per child.
        let mut server = mockito::Server::new_async().await;
        let future_date = "2099-12-31T12:00:00Z";
        let mock = server
            .mock("GET", "/api/v1/tasks/parent-future")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(format!(
                r#"{{
                    "id": "parent-future",
                    "user_id": "910",
                    "project_id": "p1",
                    "content": "parent",
                    "priority": 1,
                    "child_order": 1,
                    "day_order": -1,
                    "checked": false,
                    "is_deleted": false,
                    "is_collapsed": false,
                    "labels": [],
                    "note_count": 0,
                    "description": "",
                    "due": {{
                        "date": "{future_date}",
                        "lang": "en",
                        "is_recurring": false,
                        "string": "2099-12-31 12:00",
                        "timezone": null
                    }}
                }}"#
            ))
            .expect(1)
            .create_async()
            .await;

        let base = test::fixtures::today_task().await;
        let child1 = Task {
            id: "child-1".into(),
            parent_id: Some("parent-future".into()),
            ..base.clone()
        };
        let child2 = Task {
            id: "child-2".into(),
            parent_id: Some("parent-future".into()),
            ..base
        };
        let tasks = vec![child1, child2];

        let config = test::fixtures::config().await.with_mock_url(server.url());

        let result = reject_parent_tasks(tasks, &config).await;
        assert!(result.is_empty());
        mock.assert();
    }

    #[tokio::test]
    async fn test_reject_parent_tasks_keeps_child_when_parent_has_no_date() {
        // A child whose parent is not in the current set but the parent
        // has no due date should be kept (parent is not in the future).
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/api/v1/tasks/parent-nodate")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::Task.read().await)
            .expect(1)
            .create_async()
            .await;

        let base = test::fixtures::today_task().await;
        let child = Task {
            id: "child".into(),
            parent_id: Some("parent-nodate".into()),
            ..base
        };
        let tasks = vec![child.clone()];

        let config = test::fixtures::config().await.with_mock_url(server.url());

        let result = reject_parent_tasks(tasks, &config).await;
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, child.id);
        mock.assert();
    }

    #[tokio::test]
    async fn test_reject_parent_tasks_keeps_child_when_parent_is_overdue() {
        // A child whose parent is not in the current set but the parent
        // is overdue should be kept.
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/api/v1/tasks/parent-overdue")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{
                    "id": "parent-overdue",
                    "user_id": "910",
                    "project_id": "p1",
                    "content": "overdue parent",
                    "priority": 1,
                    "child_order": 1,
                    "day_order": -1,
                    "checked": false,
                    "is_deleted": false,
                    "is_collapsed": false,
                    "labels": [],
                    "note_count": 0,
                    "description": "",
                    "due": {
                        "date": "2020-01-01T12:00:00Z",
                        "lang": "en",
                        "is_recurring": false,
                        "string": "2020-01-01 12:00",
                        "timezone": null
                    }
                }"#,
            )
            .expect(1)
            .create_async()
            .await;

        let base = test::fixtures::today_task().await;
        let child = Task {
            id: "child".into(),
            parent_id: Some("parent-overdue".into()),
            ..base
        };
        let tasks = vec![child.clone()];

        let config = test::fixtures::config().await.with_mock_url(server.url());

        let result = reject_parent_tasks(tasks, &config).await;
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, child.id);
        mock.assert();
    }

    #[tokio::test]
    async fn test_reject_parent_tasks_keeps_child_when_parent_in_set() {
        // When the parent is present in the current task set, no API call
        // is made and the child is kept.
        let base = test::fixtures::today_task().await;
        let parent = Task {
            id: "parent-present".into(),
            parent_id: None,
            ..base.clone()
        };
        let child = Task {
            id: "child".into(),
            parent_id: Some("parent-present".into()),
            ..base
        };
        let tasks = vec![parent, child.clone()];

        let config = test::fixtures::config().await;

        let result = reject_parent_tasks(tasks, &config).await;
        // The parent is filtered out (it's a parent of another task),
        // but the child is kept because its parent is in the task set.
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, child.id);
    }

    #[tokio::test]
    async fn test_reject_parent_tasks_skips_checked_tasks() {
        // Checked tasks are excluded regardless of parent status.
        let base = test::fixtures::today_task().await;
        let checked_child = Task {
            id: "checked-child".into(),
            parent_id: Some("parent-absent".into()),
            checked: true,
            ..base.clone()
        };
        let unchecked_child = Task {
            id: "unchecked-child".into(),
            parent_id: Some("parent-absent".into()),
            checked: false,
            ..base
        };
        let tasks = vec![checked_child.clone(), unchecked_child.clone()];

        // Only the unchecked child has a parent_id not in the set, so only
        // one API call. The checked child is skipped before any API lookup.
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/api/v1/tasks/parent-absent")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::Task.read().await)
            .expect(1)
            .create_async()
            .await;

        let config = test::fixtures::config().await.with_mock_url(server.url());

        let result = reject_parent_tasks(tasks, &config).await;
        // The checked child is excluded (checked tasks are always filtered
        // out); the unchecked child is kept because the parent (no date) is
        // not in the future.
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, unchecked_child.id);
        mock.assert();
    }

    #[tokio::test]
    async fn test_reject_parent_tasks_multiple_distinct_missing_parents() {
        // Two children with different missing parents: one future, one
        // no-date. Each distinct parent ID is fetched exactly once.
        let mut server = mockito::Server::new_async().await;

        let future_date = "2099-12-31T12:00:00Z";
        let future_mock = server
            .mock("GET", "/api/v1/tasks/parent-future")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(format!(
                r#"{{
                    "id": "parent-future",
                    "user_id": "910",
                    "project_id": "p1",
                    "content": "future",
                    "priority": 1,
                    "child_order": 1,
                    "day_order": -1,
                    "checked": false,
                    "is_deleted": false,
                    "is_collapsed": false,
                    "labels": [],
                    "note_count": 0,
                    "description": "",
                    "due": {{
                        "date": "{future_date}",
                        "lang": "en",
                        "is_recurring": false,
                        "string": "2099-12-31 12:00",
                        "timezone": null
                    }}
                }}"#
            ))
            .expect(1)
            .create_async()
            .await;

        let nodate_mock = server
            .mock("GET", "/api/v1/tasks/parent-nodate")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::Task.read().await)
            .expect(1)
            .create_async()
            .await;

        let base = test::fixtures::today_task().await;
        let future_child = Task {
            id: "future-child".into(),
            parent_id: Some("parent-future".into()),
            ..base.clone()
        };
        let nodate_child = Task {
            id: "nodate-child".into(),
            parent_id: Some("parent-nodate".into()),
            ..base
        };
        let tasks = vec![future_child, nodate_child.clone()];

        let config = test::fixtures::config().await.with_mock_url(server.url());

        let result = reject_parent_tasks(tasks, &config).await;
        // future child is rejected (parent is in the future),
        // nodate child is kept (parent has no date).
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, nodate_child.id);
        future_mock.assert();
        nodate_mock.assert();
    }

    #[tokio::test]
    async fn test_reject_parent_tasks_keeps_tasks_without_parent_id() {
        // Tasks with no parent_id are kept (unless checked).
        // No API calls should be made since there's no parent to look up.
        let base = test::fixtures::today_task().await;
        let orphan = Task {
            id: "orphan".into(),
            parent_id: None,
            checked: false,
            ..base.clone()
        };
        let checked_orphan = Task {
            id: "checked-orphan".into(),
            parent_id: None,
            checked: true,
            ..base
        };
        let tasks = vec![orphan.clone(), checked_orphan];

        let config = test::fixtures::config().await;

        let result = reject_parent_tasks(tasks, &config).await;
        // The unchecked orphan is kept; the checked orphan is filtered out.
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, orphan.id);
    }

    mod proptests {
        use super::*;
        use pretty_assertions::assert_eq;
        use proptest::prelude::*;

        // ── sub-type strategies ────────────────────────────────────

        fn arb_unit() -> impl Strategy<Value = Unit> {
            prop_oneof![Just(Unit::Minute), Just(Unit::Day)]
        }

        fn arb_duration() -> impl Strategy<Value = Duration> {
            (0u32..1000, arb_unit()).prop_map(|(amount, unit)| Duration { amount, unit })
        }

        fn arb_deadline() -> impl Strategy<Value = Deadline> {
            ("[0-9]{4}-[0-9]{2}-[0-9]{2}", "[a-z]{2,5}")
                .prop_map(|(date, lang)| Deadline { date, lang })
        }

        fn arb_date_info() -> impl Strategy<Value = DateInfo> {
            (
                "[0-9T:+-]{10,25}",
                proptest::bool::ANY,
                "\\PC{1,30}",
                "[a-z]{2,5}",
                proptest::option::of("[A-Za-z_/]{3,20}"),
            )
                .prop_map(|(date, is_recurring, string, lang, timezone)| DateInfo {
                    date,
                    is_recurring,
                    string,
                    lang,
                    timezone,
                })
        }

        fn arb_priority() -> impl Strategy<Value = Priority> {
            prop_oneof![
                Just(Priority::None),
                Just(Priority::Low),
                Just(Priority::Medium),
                Just(Priority::High),
            ]
        }

        // ── Task strategy (split across three tuples to stay under 10-element cap) ──

        fn arb_task() -> impl Strategy<Value = Task> {
            let g1 = (
                "[0-9a-f]{5,20}",
                "[0-9]{3,10}",
                "[0-9]{5,15}",
                proptest::option::of("[0-9]{5,15}"),
                proptest::option::of("[0-9]{5,15}"),
                proptest::option::of("[0-9]{3,10}"),
                proptest::option::of("[0-9]{3,10}"),
                proptest::option::of("[0-9]{3,10}"),
                proptest::collection::vec("[a-z_]{2,15}", 0..5),
            );
            let g2 = (
                proptest::option::of(arb_deadline()),
                proptest::option::of(arb_duration()),
                proptest::option::of(arb_date_info()),
                proptest::bool::ANY,
                proptest::bool::ANY,
                proptest::bool::ANY,
                proptest::option::of("[0-9T:+-]{10,25}"),
                proptest::option::of("[0-9T:+-]{10,25}"),
                proptest::option::of("[0-9T:+-]{10,25}"),
            );
            let g3 = (
                arb_priority(),
                -100i16..1000i16,
                "\\PC{0,60}",
                "\\PC{0,100}",
                0u32..100u32,
                -100i16..1000i16,
            );
            (g1, g2, g3).prop_map(
                |(
                    (
                        id,
                        user_id,
                        project_id,
                        section_id,
                        parent_id,
                        added_by_uid,
                        assigned_by_uid,
                        responsible_uid,
                        labels,
                    ),
                    (
                        deadline,
                        duration,
                        due,
                        checked,
                        is_deleted,
                        is_collapsed,
                        added_at,
                        completed_at,
                        updated_at,
                    ),
                    (priority, child_order, content, description, note_count, day_order),
                )| Task {
                    id,
                    user_id,
                    project_id,
                    section_id,
                    parent_id,
                    added_by_uid,
                    assigned_by_uid,
                    responsible_uid,
                    labels,
                    deadline,
                    duration,
                    due,
                    checked,
                    is_deleted,
                    is_collapsed,
                    added_at,
                    completed_at,
                    updated_at,
                    priority,
                    child_order,
                    content,
                    description,
                    note_count,
                    day_order,
                },
            )
        }

        // ── properties ─────────────────────────────────────────────

        proptest! {
            #[test]
            fn priority_serde_roundtrip(priority in arb_priority()) {
                let json = serde_json::to_string(&priority).unwrap();
                let roundtripped: Priority = serde_json::from_str(&json).unwrap();
                assert_eq!(priority, roundtripped);
            }

            #[test]
            fn unit_serde_roundtrip(unit in arb_unit()) {
                let json = serde_json::to_string(&unit).unwrap();
                let roundtripped: Unit = serde_json::from_str(&json).unwrap();
                assert_eq!(unit, roundtripped);
            }

            #[test]
            fn duration_serde_roundtrip(dur in arb_duration()) {
                let json = serde_json::to_string(&dur).unwrap();
                let roundtripped: Duration = serde_json::from_str(&json).unwrap();
                assert_eq!(dur, roundtripped);
            }

            #[test]
            fn deadline_serde_roundtrip(deadline in arb_deadline()) {
                let json = serde_json::to_string(&deadline).unwrap();
                let roundtripped: Deadline = serde_json::from_str(&json).unwrap();
                assert_eq!(deadline, roundtripped);
            }

            #[test]
            fn date_info_serde_roundtrip(di in arb_date_info()) {
                let json = serde_json::to_string(&di).unwrap();
                let roundtripped: DateInfo = serde_json::from_str(&json).unwrap();
                assert_eq!(di, roundtripped);
            }

            #[test]
            fn task_serde_roundtrip(task in arb_task()) {
                let json = serde_json::to_string(&task).unwrap();
                let roundtripped: Task = serde_json::from_str(&json).unwrap();
                assert_eq!(task, roundtripped);
            }

            #[test]
            fn task_response_serde_roundtrip(
                tasks in proptest::collection::vec(arb_task(), 0..10),
                next_cursor in proptest::option::of("[a-zA-Z0-9]{5,20}"),
            ) {
                let response = TaskResponse {
                    results: tasks,
                    next_cursor,
                };
                let json = serde_json::to_string(&response).unwrap();
                let roundtripped: TaskResponse = serde_json::from_str(&json).unwrap();
                assert_eq!(response, roundtripped);
            }
        }
    }
}
