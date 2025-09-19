use chrono::DateTime;
use chrono::NaiveDate;
use chrono_tz::Tz;
use futures::future;
use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
use std::cmp::max;
use std::fmt::Display;
use tokio::task::JoinHandle;

pub mod format;
pub mod priority;
use crate::comments::Comment;
use crate::config::Config;
use crate::config::SortValue;
use crate::debug;
use crate::errors::Error;
use crate::input::CONTENT;
use crate::input::DateTimeInput;
use crate::projects;
use crate::tasks;
use crate::tasks::priority::Priority;
use crate::{input, time, todoist};

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
    pub deadline: Option<Deadline>,
    pub duration: Option<Duration>,
    pub due: Option<DateInfo>,
    pub checked: bool,
    pub is_deleted: bool,
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

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct TaskResponse {
    pub results: Vec<Task>,
    pub next_cursor: Option<String>,
}

// Update task_attributes fn when adding here
#[derive(Eq, PartialEq)]
pub enum TaskAttribute {
    Content,
    Description,
    Priority,
    Due,
    Labels,
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

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct DateInfo {
    /// "2025-04-26T22:00:00Z"
    pub date: String,
    pub is_recurring: bool,
    /// "2025-04-26 15:00"
    pub string: String,
    /// i.e. "en"
    pub lang: String,
    /// i.e. "America/Vancouver"
    pub timezone: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct Deadline {
    /// In format YYYY-MM-DD
    pub date: String,
    /// i.e. "en"
    pub lang: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct Duration {
    pub amount: u32,
    pub unit: Unit,
}
#[allow(dead_code)] // Body Struct is not currently used/constructed
#[derive(Serialize, Deserialize, Debug)]
struct Body {
    items: Vec<Task>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub enum Unit {
    #[serde(rename(deserialize = "minute"))]
    Minute,
    #[serde(rename(deserialize = "day"))]
    Day,
}

pub enum FormatType {
    List,
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

#[derive(clap::ValueEnum, Debug, Copy, Clone)]
pub enum SortOrder {
    /// Sort by Tod's configurable sort value
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

        let comments = if !comments.is_empty() {
            format::render_comments(config, comments).await?
        } else {
            String::new()
        };

        Ok(format!(
            "{prefix}{content}{description}{due}{labels}{comment_number}{project} {url}{comments}\n\n"
        ))
    }

    /// Determines the numeric value of an task for sorting
    fn value(&self, config: &Config) -> u32 {
        let date_value: u8 = self.date_value(config);
        let priority_value: u8 = self.priority_value(config);
        let deadline_value: u32 = match self.deadline_value(config) {
            Ok(value) => value,
            Err(error) => {
                config
                    .clone()
                    .tx()
                    .send(error)
                    .expect("Failed to send error message in task value calculation");
                0
            }
        };

        let value = date_value as u32 + priority_value as u32 + deadline_value;

        let content = self.content.clone();
        let debug_text = format!("Value: {value}, Content: {content}");
        debug::maybe_print(config, &debug_text);
        value
    }

    /// Return the value of the due field
    fn date_value(&self, config: &Config) -> u8 {
        let SortValue {
            no_due_date,
            today,
            overdue,
            now,
            not_recurring,
            ..
        } = config.sort_value.clone().unwrap_or_default();

        match &self.datetimeinfo(config) {
            Ok(DateTimeInfo::NoDateTime) => no_due_date,
            Ok(DateTimeInfo::Date {
                date, is_recurring, ..
            }) => {
                let today_value = if *date == time::naive_date_today(config).unwrap_or_default() {
                    today
                } else {
                    0
                };
                let overdue_value = if self.is_overdue(config).unwrap_or_default() {
                    overdue
                } else {
                    0
                };
                let recurring_value = if is_recurring.to_owned() {
                    0
                } else {
                    not_recurring
                };
                today_value + overdue_value + recurring_value
            }
            Ok(DateTimeInfo::DateTime {
                datetime,
                is_recurring,
                ..
            }) => {
                let recurring_value = if is_recurring.to_owned() {
                    0
                } else {
                    not_recurring
                };

                let duration = match time::datetime_now(config) {
                    Ok(tz) => (*datetime - tz).num_minutes(),
                    _ => 0,
                };

                match duration {
                    -15..=15 => now + recurring_value,
                    _ => recurring_value,
                }
            }
            Err(_) => not_recurring,
        }
    }

    /// Return the value of the due field
    fn datetime(&self, config: &Config) -> Option<DateTime<Tz>> {
        match self.datetimeinfo(config) {
            Ok(DateTimeInfo::DateTime { datetime, .. }) => Some(datetime),
            Ok(DateTimeInfo::Date { date, .. }) => {
                let naive_datetime = date.and_hms_opt(23, 59, 00)?;

                let now = time::datetime_now(config).ok()?;

                Some(DateTime::from_naive_utc_and_offset(
                    naive_datetime,
                    *now.offset(),
                ))
            }
            Ok(DateTimeInfo::NoDateTime) => None,
            Err(_) => None,
        }
    }

    fn priority_value(&self, config: &Config) -> u8 {
        let SortValue {
            priority_none,
            priority_low,
            priority_medium,
            priority_high,
            ..
        } = config.sort_value.clone().unwrap_or_default();
        match &self.priority {
            Priority::None => priority_none,
            Priority::Low => priority_low,
            Priority::Medium => priority_medium,
            Priority::High => priority_high,
        }
    }

    fn deadline_value(&self, config: &Config) -> Result<u32, Error> {
        match &self.deadline {
            None => Ok(0),
            Some(Deadline { date, .. }) => {
                let naive_date = time::date_string_to_naive_date(date)?;
                let days_from_today = time::naive_date_days_in_future(naive_date, config)?;
                let deadline_days = config.deadline_days();
                let day_multiplier = max(deadline_days as i64 - days_from_today, 0) as u32;
                let day_value = config.deadline_value();
                Ok(day_multiplier * day_value as u32)
            }
        }
    }

    /// Converts the JSON date representation into Date or Datetime
    fn datetimeinfo(&self, config: &Config) -> Result<DateTimeInfo, Error> {
        let tz_string = config.get_timezone()?;
        let tz = match self.clone().due {
            None => time::timezone_from_str(&tz_string)?,
            Some(DateInfo { timezone: None, .. }) => time::timezone_from_str(&tz_string)?,
            Some(DateInfo {
                timezone: Some(other_timezone),
                ..
            }) => time::timezone_from_str(&other_timezone)?,
        };
        match self.clone().due {
            None => Ok(DateTimeInfo::NoDateTime),
            Some(DateInfo {
                date,
                is_recurring,
                string,
                ..
            }) if date.len() == 10 => Ok(DateTimeInfo::Date {
                date: time::date_from_str(&date, tz)?,
                is_recurring,
                string,
            }),
            Some(DateInfo {
                date,
                is_recurring,
                string,
                ..
            }) => Ok(DateTimeInfo::DateTime {
                datetime: time::datetime_from_str(&date, tz)?,
                is_recurring,
                string,
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

    fn has_no_date(&self) -> bool {
        self.due.is_none()
    }

    // Returns true if the datetime is today and there is a time
    fn is_today(&self, config: &Config) -> Result<bool, Error> {
        let boolean = match self.datetimeinfo(config) {
            Ok(DateTimeInfo::NoDateTime) => false,
            Ok(DateTimeInfo::Date { date, .. }) => date == time::naive_date_today(config)?,
            Ok(DateTimeInfo::DateTime { datetime, .. }) => {
                time::datetime_is_today(datetime, config)?
            }
            Err(_) => false,
        };

        Ok(boolean)
    }

    fn is_overdue(&self, config: &Config) -> Result<bool, Error> {
        let boolean = match self.clone().datetimeinfo(config) {
            Ok(DateTimeInfo::NoDateTime) => false,
            Ok(DateTimeInfo::Date { date, .. }) => time::is_date_in_past(date, config)?,
            Ok(DateTimeInfo::DateTime { datetime, .. }) => {
                time::is_date_in_past(datetime.date_naive(), config)?
            }
            Err(_) => false,
        };

        Ok(boolean)
    }

    /// Returns true if it is a recurring task
    pub fn is_recurring(&self) -> bool {
        match self.due {
            None => false,
            Some(DateInfo { is_recurring, .. }) => is_recurring,
        }
    }
}

pub fn sort(tasks: Vec<Task>, config: &Config, sort: &SortOrder) -> Vec<Task> {
    match sort {
        SortOrder::Value => sort_by_value(tasks, config),
        SortOrder::Datetime => sort_by_datetime(tasks, config),
        SortOrder::Todoist => tasks,
    }
}

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
                let handle = spawn_update_task_content(config.clone(), task.clone(), new_value);
                Ok(Some(handle))
            }
        }
        TaskAttribute::Description => {
            let value = task.description.as_str();

            let new_value = input::string_with_default("Enter a new description:", value)?;

            if *value == new_value {
                Ok(None)
            } else {
                let handle = spawn_update_task_description(config.clone(), task.clone(), new_value);
                Ok(Some(handle))
            }
        }
        TaskAttribute::Priority => {
            let value = &task.priority;
            let priorities = priority::all_priorities();

            let new_value = input::select("Select your priority:", priorities, config.mock_select)?;
            if *value == new_value {
                Ok(None)
            } else {
                let handle = spawn_update_task_priority(config.clone(), task.clone(), new_value);
                Ok(Some(handle))
            }
        }
        TaskAttribute::Due => tasks::spawn_schedule_task(config.clone(), task.clone()).await,
        TaskAttribute::Deadline => tasks::spawn_deadline_task(config.clone(), task.clone()).await,
        TaskAttribute::Labels => {
            let label_string = input::string(
                "Enter labels separated by spaces:",
                config.mock_string.clone(),
            )?;

            let labels = label_string
                .split_whitespace()
                .map(|s| s.to_owned())
                .collect();

            let handle = spawn_update_task_labels(config.clone(), task.clone(), labels);
            Ok(Some(handle))
        }
    }
}

pub async fn label_task(
    config: &Config,
    task: Task,
    labels: &Vec<String>,
) -> Result<JoinHandle<()>, Error> {
    let comments = Vec::new();
    let text = task.fmt(comments, config, FormatType::Single, true).await?;
    println!("{text}");
    let mut options = labels.to_owned();
    options.push(input::SKIP.to_string());
    let label = input::select("Select label", options, config.mock_select)?;

    let config = config.clone();
    Ok(tokio::spawn(async move {
        if label.as_str() == input::SKIP {
        } else if let Err(e) = todoist::add_task_label(&config, task, label, false).await {
            config
                .tx()
                .send(e)
                .expect("Failed to send error on task channel");
        }
    }))
}

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
        input::DELETE,
        input::QUIT,
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let formatted_task = task
        .fmt(comments, config, FormatType::Single, with_project)
        .await?;
    let mut reloaded_config = config.reload().await?.increment_completed()?;
    let tasks_completed = reloaded_config.tasks_completed()?;
    println!("{formatted_task}{tasks_completed} completed today, {task_count} remaining");
    *task_count -= 1;
    let selection = input::select(input::OPTION, options, config.mock_select)?;
    match selection.as_str() {
        input::COMPLETE => {
            reloaded_config.save().await.expect("Could not save config");
            Ok(Some(spawn_complete_task(reloaded_config, task)))
        }
        input::DELETE => Ok(Some(spawn_delete_task(config.clone(), task))),
        input::COMMENT => {
            let content = input::string(CONTENT, config.mock_string.clone())?;

            Ok(Some(spawn_comment_task(config.clone(), task, content)))
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
    .map(|s| s.to_string())
    .collect();
    let comments = Vec::new();
    let formatted_task = task
        .fmt(comments, config, FormatType::Single, with_project)
        .await?;
    println!("{formatted_task}{task_count} task(s) remaining");
    *task_count -= 1;
    let selection = input::select("Select an option", options, config.mock_select)?;
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

        input::DELETE => Ok(Some(spawn_delete_task(config.clone(), task))),
        input::COMPLETE => Ok(Some(spawn_complete_task(config.clone(), task))),
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

/// Returns Date, time and duration for a task, uses the date and time on task if available, otherwise prompts. Always prompts for duration.
fn get_timebox(config: &Config, task: &Task) -> Result<(String, u32), Error> {
    let datetime = match task {
        Task {
            due: Some(DateInfo { date, .. }),
            ..
        } => {
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
        }
        _ => {
            let date = input::date()?;
            let time = input::string(input::TIME, config.mock_string.clone())?;
            format!("{date} {time}")
        }
    };

    let duration = input::string(input::DURATION, config.mock_string.clone())?;

    Ok((datetime, duration.parse::<u32>()?))
}

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
        config.mock_select,
        config.mock_string.clone(),
        config.natural_language_only,
        false,
        true,
    )?;
    match datetime_input {
        input::DateTimeInput::Complete => {
            let handle = tasks::spawn_complete_task(config.clone(), task.clone());
            Ok(Some(handle))
        }
        DateTimeInput::Skip => Ok(None),

        input::DateTimeInput::Text(due_string) => {
            let handle =
                tasks::spawn_update_task_due(config.clone(), task.clone(), due_string, None);
            Ok(Some(handle))
        }
        input::DateTimeInput::None => {
            let handle = tasks::spawn_update_task_due(
                config.clone(),
                task.clone(),
                "No date".to_string(),
                None,
            );
            Ok(Some(handle))
        }
    }
}
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
        config.mock_select,
        config.mock_string.clone(),
        config.natural_language_only,
        true,
        true,
    )?;
    match datetime_input {
        input::DateTimeInput::Complete => {
            let handle = tasks::spawn_complete_task(config.clone(), task.clone());
            Ok(Some(handle))
        }
        DateTimeInput::Skip => Ok(None),

        input::DateTimeInput::Text(date) => {
            let handle =
                tasks::spawn_update_task_deadline(config.clone(), task.clone(), Some(date));
            Ok(Some(handle))
        }
        input::DateTimeInput::None => {
            let handle = tasks::spawn_update_task_deadline(config.clone(), task.clone(), None);
            Ok(Some(handle))
        }
    }
}

/// Completes task inside another thread
pub fn spawn_complete_task(config: Config, task: Task) -> JoinHandle<()> {
    tokio::spawn(async move {
        if let Err(e) = todoist::complete_task(&config, &task, false).await {
            config
                .tx()
                .send(e)
                .expect("Failed to send error on task channel");
        }
    })
}

/// Deletes task inside another thread
pub fn spawn_delete_task(config: Config, task: Task) -> JoinHandle<()> {
    tokio::spawn(async move {
        if let Err(e) = todoist::delete_task(&config, &task, false).await {
            config
                .tx()
                .send(e)
                .expect("Failed to send error on task channel");
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
            config
                .tx()
                .send(e)
                .expect("Failed to send error on task channel");
        }
    })
}

/// Updates task inside another thread
pub fn spawn_update_task_deadline(
    config: Config,
    task: Task,
    date: Option<String>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        if let Err(e) = todoist::update_task_deadline(&config, &task, date, false).await {
            config
                .tx()
                .send(e)
                .expect("Failed to send error on task channel");
        }
    })
}

/// Updates task inside another thread
pub fn spawn_comment_task(config: Config, task: Task, task_comment: String) -> JoinHandle<()> {
    tokio::spawn(async move {
        if let Err(e) = todoist::create_comment(&config, &task, &task_comment, false).await {
            config
                .tx()
                .send(e)
                .expect("Failed to send error on task channel");
        }
    })
}

/// Updates task inside another thread
pub fn spawn_update_task_content(config: Config, task: Task, content: String) -> JoinHandle<()> {
    tokio::spawn(async move {
        if let Err(e) = todoist::update_task_content(&config, &task, &content, false).await {
            config
                .tx()
                .send(e)
                .expect("Failed to send error on task channel");
        }
    })
}

/// Updates task inside another thread
pub fn spawn_update_task_description(
    config: Config,
    task: Task,
    description: String,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        if let Err(e) = todoist::update_task_description(&config, &task, &description, false).await
        {
            config
                .tx()
                .send(e)
                .expect("Failed to send error on task channel");
        }
    })
}

/// Updates task inside another thread
pub fn spawn_update_task_labels(config: Config, task: Task, labels: Vec<String>) -> JoinHandle<()> {
    tokio::spawn(async move {
        if let Err(e) = todoist::update_task_labels(&config, &task, labels, false).await {
            config
                .tx()
                .send(e)
                .expect("Failed to send error on task channel");
        }
    })
}

/// Updates task inside another thread
pub fn spawn_update_task_priority(
    config: Config,
    task: Task,
    priority: Priority,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        if let Err(e) = todoist::update_task_priority(&config, &task, &priority, false).await {
            config
                .tx()
                .send(e)
                .expect("Failed to send error on task channel");
        }
    })
}

/// Converts a JSON string to a single task (creates a single task from a JSON string)
pub fn json_to_task(json: String) -> Result<Task, Error> {
    let task: Task = serde_json::from_str(&json)?;
    Ok(task)
}
/// Converts a JSON String to a list of multiple tasks (creates a TaskResponse from a JSON string)
pub fn json_to_tasks_response(json: String) -> Result<TaskResponse, Error> {
    let response: TaskResponse = serde_json::from_str(&json)?;
    Ok(response)
}

pub fn sort_by_value(mut tasks: Vec<Task>, config: &Config) -> Vec<Task> {
    tasks.sort_by_key(|b| Reverse(b.value(config)));
    tasks
}

pub fn sort_by_datetime(mut tasks: Vec<Task>, config: &Config) -> Vec<Task> {
    tasks.sort_by_key(|i| i.datetime(config));
    tasks
}

pub fn filter_not_in_future(tasks: Vec<Task>, config: &Config) -> Result<Vec<Task>, Error> {
    let tasks = tasks
        .into_iter()
        .filter(|task| {
            task.is_today(config).unwrap_or_default()
                || task.has_no_date()
                || task.is_overdue(config).unwrap_or_default()
        })
        .collect();

    Ok(tasks)
}

// We don't want to process parent tasks when child tasks are unchecked, or child tasks when they are checked
// We additionally need to make sure that parent tasks are not in the future

pub async fn reject_parent_tasks(tasks: Vec<Task>, config: &Config) -> Vec<Task> {
    let parent_ids: Vec<String> = tasks
        .clone()
        .into_iter()
        .filter(|task| task.parent_id.is_some() && !task.checked)
        .map(|task| task.parent_id.unwrap_or_default())
        .collect();

    let mut handles = Vec::new();

    for task in tasks.clone() {
        let config = config.clone();
        let parent_ids = parent_ids.clone();
        let tasks = tasks.clone();

        let config = config.clone();
        let handle = tokio::spawn(async move {
            if !parent_ids.contains(&task.id)
                && !task.checked
                && !parent_in_future(task.clone(), tasks, &config).await
            {
                Some(task)
            } else {
                None
            }
        });

        handles.push(handle);
    }

    future::join_all(handles)
        .await
        .into_iter()
        .filter_map(|t| t.ok())
        .flatten()
        .collect::<Vec<Task>>()
}

// Need to make sure that we are not completing a subtask for a parent task that is in the future
async fn parent_in_future(task: Task, tasks: Vec<Task>, config: &Config) -> bool {
    let task_ids: Vec<String> = tasks.clone().into_iter().map(|task| task.id).collect();

    match task {
        Task {
            parent_id: None, ..
        } => false,
        Task {
            parent_id: Some(parent_id),
            ..
        } => {
            if task_ids.contains(&parent_id) {
                false
            } else {
                // look up id and see if it is in the future
                match todoist::get_task(config, &parent_id).await {
                    Err(e) => {
                        config
                            .clone()
                            .tx()
                            .send(e)
                            .expect("expected value or result, got None or Err");
                        false
                    }
                    Ok(task) => {
                        !(task.is_overdue(config).unwrap_or_default()
                            || task.has_no_date()
                            || task.is_today(config).unwrap_or_default())
                    }
                }
            }
        }
    }
}

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
    let priority = input::select(input::PRIORITY, options, config.mock_select)?;

    let config = config.clone();
    Ok(tokio::spawn(async move {
        if let Err(e) = todoist::update_task_priority(&config, &task, &priority, false).await {
            config
                .tx()
                .send(e)
                .expect("Failed to send error on task channel");
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test::{self, responses::ResponseFromFile};
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn date_value_can_handle_date() {
        let config = test::fixtures::config().await;
        // On another day
        assert_eq!(test::fixtures::today_task().await.date_value(&config), 50);

        // Recurring
        let task = Task {
            due: Some(DateInfo {
                is_recurring: true,
                ..test::fixtures::today_task()
                    .await
                    .due
                    .expect("Failed to unwrap due field in test fixtures for today_task")
            }),
            ..test::fixtures::today_task().await
        };
        assert_eq!(task.date_value(&config), 0);

        // Overdue
        let task = Task {
            due: Some(DateInfo {
                date: "2001-11-13".into(),
                is_recurring: true,
                lang: "en".into(),
                timezone: Some("America/Los_Angeles".into()),
                string: "Every 2 weeks".into(),
            }),
            ..test::fixtures::today_task().await
        };
        assert_eq!(task.date_value(&config), 150);

        // No date
        let task = Task { due: None, ..task };
        assert_eq!(task.date_value(&config), 80);
    }

    #[tokio::test]
    async fn date_value_can_handle_datetime() {
        let config = test::fixtures::config().await;
        let task = Task {
            due: Some(DateInfo {
                date: "2021-02-27T19:41:56Z".into(),
                ..test::fixtures::today_task()
                    .await
                    .due
                    .expect("Failed to unwrap due field in test fixtures for today_task")
            }),
            ..test::fixtures::today_task().await
        };

        assert_eq!(task.date_value(&config), 50);
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
    async fn value_can_get_the_value_of_an_task() {
        let config = test::fixtures::config().await;
        let task = Task {
            due: Some(DateInfo {
                date: "2021-09-06T16:00:00".into(),
                ..test::fixtures::today_task()
                    .await
                    .due
                    .expect("Failed to unwrap due field in test fixtures for today_task")
            }),
            ..test::fixtures::today_task().await
        };

        assert_matches!(task.datetime(&config), Some(DateTime { .. }));
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
            description: "".into(),
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
            description: "".into(),
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
    async fn test_display_task() {
        let task = test::fixtures::today_task().await;
        let string = String::from("TEST");
        assert_eq!(string, task.to_string())
    }
    #[tokio::test]
    async fn test_deadline_value_when_today() {
        let config = test::fixtures::config().await;
        let task = test::fixtures::today_task().await;
        let value = task
            .deadline_value(&config)
            .expect("expected value or result, got None or Err");
        assert_eq!(value, 150);
    }

    #[tokio::test]
    async fn test_deadline_value_when_tomorrow() {
        let config = test::fixtures::config().await;
        let task = test::fixtures::task(1).await;

        let value = task
            .deadline_value(&config)
            .expect("expected value or result, got None or Err");
        assert_eq!(value, 120);
    }

    #[tokio::test]
    async fn test_deadline_value_when_in_six_days() {
        let config = test::fixtures::config().await;
        let task = test::fixtures::task(6).await;

        let value = task
            .deadline_value(&config)
            .expect("expected value or result, got None or Err");
        assert_eq!(value, 0);
    }
    #[tokio::test]
    async fn test_deadline_value_when_yesterday() {
        let config = test::fixtures::config().await;
        let task = test::fixtures::task(-1).await;

        let value = task
            .deadline_value(&config)
            .expect("expected value or result, got None or Err");
        assert_eq!(value, 180);
    }
}
