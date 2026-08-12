//! Todoist REST API client.
//!
//! Covers tasks (quick-add, CRUD, move, label), projects, sections, comments, labels,
//! reminders, and user/auth. Use `grep 'pub async fn' src/todoist/mod.rs` for the full
//! API surface.

use futures::future;
use serde_json::{Number, Value, json};
use std::collections::HashMap;
use urlencoding::encode;
mod request;

use crate::comments::{Comment, CommentResponse};
use crate::config::Config;
use crate::debug::maybe_print;
use crate::errors::Error;
use crate::labels::{Label, LabelResponse};
use crate::oauth::{AccessToken, CLIENT_ID, CLIENT_SECRET};
use crate::projects::{Project, ProjectResponse};
use crate::reminders::{Reminder, ReminderResponse};
use crate::sections::{Section, SectionResponse};
use crate::shell::execute_command;
use crate::tasks::priority::Priority;
use crate::tasks::{Task, TaskResponse};
use crate::users::User;
use crate::{format, time};
use regex::Regex;

// TODOIST URLS
/// Tasks API base URL.
pub const TASKS_URL: &str = "/api/v1/tasks/";
/// Comments API base URL.
pub const COMMENTS_URL: &str = "/api/v1/comments/";
const SECTIONS_URL: &str = "/api/v1/sections";
const REMINDERS_URL: &str = "/api/v1/reminders";
const USER_URL: &str = "/api/v1/user";
const PROJECTS_URL: &str = "/api/v1/projects";
const LABELS_URL: &str = "/api/v1/labels";
const ACCESS_TOKEN_URL: &str = "/oauth/access_token";
/// OAuth authorization URL.
pub const OAUTH_URL: &str = "/oauth/authorize";

/// Number of items that can be requested from API at once
pub const QUERY_LIMIT: u8 = 200;

/// Used to sanity check all the Todoist API endpoints to make sure that we are able to process the JSON payloads they are sending back.
pub async fn test_all_endpoints(config: &Config) -> Result<String, Error> {
    let name = "TEST".to_string();
    let date = time::date_string_today(config)?;
    let priority = Priority::None;
    let labels: Vec<String> = vec!["one".into(), "two".into()];

    println!("Creating project");
    let project = create_project(config, &name, &name, false, false).await?;

    println!("List projects");
    let _projects = all_projects(config, Some(1)).await?;

    println!("Creating section");
    let section = create_section(config, &name, &project, false).await?;

    println!("Creating task with add_task");
    let task = create_task(
        config,
        &name,
        &project,
        Some(&section),
        priority,
        &name,
        None,
        &[],
        None,
    )
    .await?;

    println!("Getting sections for project");
    let _sections = all_sections_by_project(config, &project, Some(1)).await?;

    println!("Moving task to section");
    let _task = move_task_to_section(config, &task, &section, false).await?;

    println!("Getting task with get_task");
    let task = get_task(config, &task.id).await?;

    println!("Commenting on task twice");
    let _comment = create_comment(config, &task.id, &name, false).await?;

    let _comment = create_comment(config, &task.id, &name, false).await?;

    println!("Getting comments for task");
    let _comments = all_comments(config, &task.id, Some(1)).await?;

    println!("Deleting task");
    delete_task(config, &task.id, false).await?;

    println!("Creating two tasks with quick_add_task");
    let _task = quick_create_task(config, &name, None).await?;
    let task = quick_create_task(config, &name, Some(String::from("tomorrow"))).await?;

    println!("Finding tasks with tasks_for_project");
    let _tasks = all_tasks_by_project(config, &project, Some(1)).await?;

    println!("Finding tasks with tasks_for_filter");
    let _tasks = all_tasks_by_filter(config, "tod", Some(1)).await?;

    println!("Updating task priority");
    let _task = update_task_priority(config, &task.id, &priority, false).await?;

    println!("Updating task content");
    let _task = update_task_content(config, &task.id, &name, false).await?;

    println!("Updating task description");
    let _task = update_task_description(config, &task.id, &name, false).await?;

    println!("Updating task deadline");
    let _task = update_task_deadline(config, &task.id, Some(date), false).await?;

    println!("Updating task labels");
    let _task = update_task_labels(config, &task.id, labels, false).await?;

    println!("Adding task label");
    let _task = add_task_label(config, &task, "three".into(), false).await?;

    println!("Updating task due with natural language");
    let _task =
        update_task_due_natural_language(config, &task, "today".into(), None, false).await?;

    println!("Moving task to project");
    let task = move_task_to_project(config, &task, &project, false).await?;

    println!("Completing task");
    let _task = complete_task(config, &task.id, false).await?;

    println!("Deleting task");
    delete_task(config, &task.id, false).await?;

    println!("Deleting project");
    delete_project(config, &project, false).await?;

    println!("List labels");
    let _labels = all_labels(config, false, Some(1)).await?;

    println!("Get user data");
    let _data = get_user_data(config).await?;

    Ok(format::green_string("Completed successfully"))
}

/// Add a new task to the inbox with natural language support
pub async fn quick_create_task(
    config: &Config,
    content: &str,
    reminder: Option<String>,
) -> Result<Task, Error> {
    let url = format!("{TASKS_URL}quick");
    let body = json!({"text": content, "auto_reminder": true, "reminder": reminder});

    let json = request::post_todoist(config, &url, body, true).await?;
    maybe_run_command(config.task_create_command.as_deref(), config)?;
    Task::from_json(&json)
}

/// Fetches a single task by ID.
pub async fn get_task(config: &Config, id: &str) -> Result<Task, Error> {
    let url = format!("{TASKS_URL}{id}");
    let json = request::get_todoist(config, &url, true).await?;
    Task::from_json(&json)
}

/// Exchanges an OAuth code for an access token.
pub async fn get_access_token(config: &Config, code: &str) -> Result<String, Error> {
    let url = ACCESS_TOKEN_URL.to_string();
    let body = json!({"code": code, "client_id": CLIENT_ID, "client_secret": CLIENT_SECRET});

    let json = request::post_todoist_no_token(config, &url, body, true).await?;

    AccessToken::from_json(&json).map(|t| t.access_token)
}

/// Add Task without natural language support but supports additional parameters
#[allow(clippy::too_many_arguments)]
pub async fn create_task(
    config: &Config,
    content: &str,
    project: &Project,
    section: Option<&Section>,
    priority: Priority,
    description: &str,
    due: Option<&str>,
    labels: &[String],
    parent_id: Option<&str>,
) -> Result<Task, Error> {
    let project_id = project.id.clone();
    let url = TASKS_URL;
    let mut body: HashMap<String, Value> = HashMap::new();
    body.insert("content".to_owned(), Value::String(content.to_owned()));
    body.insert(
        "description".to_owned(),
        Value::String(description.to_owned()),
    );
    body.insert("project_id".to_owned(), Value::String(project_id));

    body.insert("auto_reminder".to_owned(), Value::Bool(true));
    body.insert(
        "priority".to_owned(),
        Value::Number(Number::from(priority.to_integer())),
    );
    let labels = labels.iter().map(|l| Value::String(l.to_owned())).collect();
    body.insert("labels".to_owned(), Value::Array(labels));

    if let Some(date) = due {
        if time::is_date(date) || time::is_datetime(date) {
            body.insert("due_date".to_owned(), Value::String(date.to_owned()));
        } else {
            body.insert("due_string".to_owned(), Value::String(date.to_owned()));
        }
    }

    if let Some(section) = section {
        body.insert("section_id".to_owned(), Value::String(section.id.clone()));
    }

    if let Some(parent_id) = parent_id {
        body.insert("parent_id".to_owned(), Value::String(parent_id.to_owned()));
    }

    let body = json!(body);

    let json = request::post_todoist(config, url, body, true).await?;
    maybe_run_command(config.task_create_command.as_deref(), config)?;
    Task::from_json(&json)
}

/// Create a reminder for a task
#[allow(clippy::too_many_arguments)]
pub async fn create_reminder(
    config: &Config,
    task: &Task,
    due_string: &str,
    spinner: bool,
) -> Result<Reminder, Error> {
    let task_id = task.id.clone();
    let url = REMINDERS_URL;
    let body =
        json!({"task_id": task_id, "reminder_type": "absolute", "due": {"string": due_string}});

    let json = request::post_todoist(config, url, body, spinner).await?;
    Reminder::from_json(&json)
}

/// Get a vector of all tasks for a project
pub async fn all_tasks_by_project(
    config: &Config,
    project: &Project,
    limit: Option<u8>,
) -> Result<Vec<Task>, Error> {
    let limit = limit.unwrap_or(QUERY_LIMIT);
    let project_id = project.id.clone();
    let mut tasks = Vec::new();
    let mut url = format!("{TASKS_URL}?project_id={project_id}&limit={limit}");
    let title_regex = config.task_exclude_regex.as_ref();

    loop {
        let json = request::get_todoist(config, &url, true).await?;
        let TaskResponse {
            results,
            next_cursor,
        } = TaskResponse::from_json(&json)?;

        let results = filter_tasks_by_title(results, title_regex, config);
        tasks.extend(results);

        match next_cursor {
            None => break,
            Some(cursor) => {
                url = format!("{TASKS_URL}?project_id={project_id}&limit={limit}&cursor={cursor}");
            }
        }
    }
    Ok(tasks)
}

/// Uses multiple filters (comma-separated) to fetch multiple lists of tasks in parallel. Returns each list of tasks with the filter query that was used to find it.
pub async fn all_tasks_by_filters(
    config: &Config,
    filter: &str,
) -> Result<Vec<(String, Vec<Task>)>, Error> {
    let filters: Vec<_> = filter
        .split(',')
        .map(|f| all_tasks_by_filter(config, f, None))
        .collect();

    future::try_join_all(filters).await
}

/// Fetches a list of tasks by a single filter query.
pub async fn all_tasks_by_filter(
    config: &Config,
    filter: &str,
    limit: Option<u8>,
) -> Result<(String, Vec<Task>), Error> {
    let limit = limit.unwrap_or(QUERY_LIMIT);
    let encoded = encode(filter);
    let mut tasks: Vec<Task> = Vec::new();
    let mut url = format!("{TASKS_URL}filter?query={encoded}&limit={limit}");
    let title_regex = config.task_exclude_regex.as_ref();

    loop {
        let json = request::get_todoist(config, &url, true).await?;
        let TaskResponse {
            results,
            next_cursor,
        } = TaskResponse::from_json(&json)?;

        let results = filter_tasks_by_title(results, title_regex, config);
        tasks.extend(results);

        match next_cursor {
            None => break,
            Some(string) => {
                url = format!("{TASKS_URL}filter?query={encoded}&limit={limit}&cursor={string}");
            }
        }
    }

    Ok((filter.to_string(), tasks))
}

/// Fetches a list of tasks by their ids.
pub async fn all_tasks_by_ids(
    config: &Config,
    task_ids: Vec<String>,
    limit: Option<u8>,
) -> Result<Vec<Task>, Error> {
    let limit = limit.unwrap_or(QUERY_LIMIT);
    let mut tasks: Vec<Task> = Vec::new();
    if task_ids.is_empty() {
        return Ok(tasks);
    }
    let task_ids = task_ids.join(",");
    let mut url = format!("{TASKS_URL}?ids={task_ids}&limit={limit}");

    loop {
        let json = request::get_todoist(config, &url, true).await?;
        let TaskResponse {
            results,
            next_cursor,
        } = TaskResponse::from_json(&json)?;

        tasks.extend(results);

        match next_cursor {
            None => break,
            Some(string) => {
                url = format!("{TASKS_URL}?ids={task_ids}&limit={limit}&cursor={string}");
            }
        }
    }

    Ok(tasks)
}

/// Returns all sections for a project with cursor-based pagination.
pub async fn all_sections_by_project(
    config: &Config,
    project: &Project,
    limit: Option<u8>,
) -> Result<Vec<Section>, Error> {
    let limit = limit.unwrap_or(QUERY_LIMIT);
    let project_id = project.id.clone();
    let mut url = format!("{SECTIONS_URL}?project_id={project_id}&limit={limit}");
    let mut sections: Vec<Section> = Vec::new();

    loop {
        let json = request::get_todoist(config, &url, true).await?;
        let SectionResponse {
            results,
            next_cursor,
        } = SectionResponse::from_json(&json)?;
        sections.extend(results);
        match next_cursor {
            None => break,
            Some(string) => {
                url =
                    format!("{SECTIONS_URL}?project_id={project_id}&limit={limit}&cursor={string}");
            }
        }
    }
    Ok(sections)
}

/// Returns all projects with cursor-based pagination.
pub async fn all_projects(config: &Config, limit: Option<u8>) -> Result<Vec<Project>, Error> {
    let limit = limit.unwrap_or(QUERY_LIMIT);
    let mut url = format!("{PROJECTS_URL}?limit={limit}");
    let mut projects: Vec<Project> = Vec::new();

    loop {
        let json = request::get_todoist(config, &url, true).await?;
        let ProjectResponse {
            results,
            next_cursor,
        } = ProjectResponse::from_json(&json)?;
        projects.extend(results);
        match next_cursor {
            None => break,
            Some(string) => {
                url = format!("{PROJECTS_URL}?limit={limit}&cursor={string}");
            }
        }
    }
    Ok(projects)
}

/// Returns all reminders with cursor-based pagination.
pub async fn all_reminders(config: &Config, limit: Option<u8>) -> Result<Vec<Reminder>, Error> {
    let limit = limit.unwrap_or(QUERY_LIMIT);
    let mut url = format!("{REMINDERS_URL}?limit={limit}");
    let mut reminders: Vec<Reminder> = Vec::new();

    loop {
        let json = request::get_todoist(config, &url, true).await?;
        let ReminderResponse {
            results,
            next_cursor,
        } = ReminderResponse::from_json(&json)?;
        reminders.extend(results);
        match next_cursor {
            None => break,
            Some(string) => {
                url = format!("{REMINDERS_URL}?limit={limit}&cursor={string}");
            }
        }
    }
    Ok(reminders)
}

/// Returns all labels with cursor-based pagination.
pub async fn all_labels(
    config: &Config,
    spinner: bool,
    limit: Option<u8>,
) -> Result<Vec<Label>, Error> {
    let limit = limit.unwrap_or(QUERY_LIMIT);
    let mut url = format!("{LABELS_URL}?limit={limit}");
    let mut labels: Vec<Label> = Vec::new();
    loop {
        let json = request::get_todoist(config, &url, spinner).await?;
        let LabelResponse {
            results,
            next_cursor,
        } = LabelResponse::from_json(&json)?;
        labels.extend(results);
        match next_cursor {
            None => break,
            Some(string) => {
                url = format!("{LABELS_URL}?limit={limit}&cursor={string}");
            }
        }
    }
    Ok(labels)
}

/// Create a personal label.
pub async fn create_label(
    config: &Config,
    name: &str,
    color: Option<&str>,
    order: Option<u32>,
    is_favorite: bool,
    spinner: bool,
) -> Result<Label, Error> {
    let mut body = json!({"name": name, "is_favorite": is_favorite});
    if let Some(c) = color {
        body["color"] = json!(c);
    }
    if let Some(o) = order {
        body["order"] = json!(o);
    }

    let response = request::post_todoist(config, LABELS_URL, body, spinner).await?;
    Label::from_json(&response)
}

/// Move a task to a different project
pub async fn move_task_to_project(
    config: &Config,
    task: &Task,
    project: &Project,
    spinner: bool,
) -> Result<Task, Error> {
    let project_id = project.id.clone();
    let task_id = task.id.clone();
    let body = json!({"project_id": project_id});
    let url = format!("{TASKS_URL}{task_id}/move");

    let response = request::post_todoist(config, &url, body, spinner).await?;
    Task::from_json(&response)
}

/// Moves a task to a different section.
pub async fn move_task_to_section(
    config: &Config,
    task: &Task,
    section: &Section,
    spinner: bool,
) -> Result<Task, Error> {
    let section_id = section.id.clone();
    let task_id = task.id.clone();
    let body = json!({"section_id": section_id});
    let url = format!("{TASKS_URL}{task_id}/move");

    let response = request::post_todoist(config, &url, body, spinner).await?;
    Task::from_json(&response)
}

/// Update the priority of a task by ID
pub async fn update_task_priority(
    config: &Config,
    task_id: &str,
    priority: &Priority,
    spinner: bool,
) -> Result<String, Error> {
    let body = json!({ "priority": priority });
    let url = format!("{TASKS_URL}{task_id}");

    request::post_todoist(config, &url, body, spinner).await?;
    // Does not pass back a task
    Ok("✓".into())
}

/// Add a label to task by ID
pub async fn add_task_label(
    config: &Config,
    task: &Task,
    label: String,
    spinner: bool,
) -> Result<String, Error> {
    let mut labels = task.labels.clone();
    labels.push(label);
    let body = json!({ "labels": labels});
    let url = format!("{}{}", TASKS_URL, task.id);

    request::post_todoist(config, &url, body, spinner).await?;
    // Does not pass back a task
    Ok("✓".into())
}

/// Update due date for task using natural language
pub async fn update_task_due_natural_language(
    config: &Config,
    task: &Task,
    due_string: String,
    duration: Option<u32>,
    spinner: bool,
) -> Result<String, Error> {
    let due_string = if let Some(due) = &task.due {
        if task.is_recurring() {
            format!("{} starting {due_string}", due.string)
        } else {
            due_string
        }
    } else {
        due_string
    };

    let body = if let Some(duration) = duration {
        json!({ "due_string": due_string, "duration": duration, "duration_unit": "minute" })
    } else {
        json!({ "due_string": due_string })
    };
    let url = format!("{}{}", TASKS_URL, task.id);

    request::post_todoist(config, &url, body, spinner).await?;
    // Does not pass back a task
    Ok("✓".into())
}

/// Update the content of a task by ID
pub async fn update_task_content(
    config: &Config,
    task_id: &str,
    content: &str,
    spinner: bool,
) -> Result<String, Error> {
    let body = json!({ "content": content});
    let url = format!("{TASKS_URL}{task_id}");

    request::post_todoist(config, &url, body, spinner).await?;
    // Does not pass back a task
    Ok("✓".into())
}

/// Update the deadline of a task by ID. Pass `None` to clear the deadline.
pub async fn update_task_deadline(
    config: &Config,
    task_id: &str,
    date: Option<String>,
    spinner: bool,
) -> Result<String, Error> {
    let body = match date {
        Some(date) => {
            if !time::is_date(&date) {
                return Err(Error {
                    message: format!("Not a valid date in format YYYY-MM-DD, got: {date}"),
                    source: "update_task_deadline".to_string(),
                });
            }
            json!({"deadline_date": date, "deadline_lang": "en"})
        }
        None => json!({"deadline_date": null, "deadline_lang": null}),
    };
    let url = format!("{TASKS_URL}{task_id}");

    request::post_todoist(config, &url, body, spinner).await?;
    // Does not pass back a task
    Ok("✓".into())
}

/// Update the description of a task by ID
pub async fn update_task_description(
    config: &Config,
    task_id: &str,
    description: &str,
    spinner: bool,
) -> Result<String, Error> {
    let body = json!({ "description": description});
    let url = format!("{TASKS_URL}{task_id}");

    request::post_todoist(config, &url, body, spinner).await?;
    // Does not pass back a task
    Ok("✓".into())
}

/// Update the labels of a task by ID
/// Replaces the old labels
pub async fn update_task_labels(
    config: &Config,
    task_id: &str,
    labels: Vec<String>,
    spinner: bool,
) -> Result<String, Error> {
    let body = json!({ "labels": labels});
    let url = format!("{TASKS_URL}{task_id}");

    request::post_todoist(config, &url, body, spinner).await?;
    // Does not pass back a task
    Ok("✓".into())
}

/// Complete a task by its ID. Does not return a new task (the API yields no data).
pub async fn complete_task(config: &Config, task_id: &str, spinner: bool) -> Result<String, Error> {
    let url = format!("{TASKS_URL}{task_id}/close");

    request::post_todoist(config, &url, Value::Null, spinner).await?;

    if !cfg!(test) {
        maybe_run_command(config.task_complete_command.as_deref(), config)?;
        config.reload().await?.clear_next_task().save().await?;
    }
    // Execute the execute_command() complete_task_command if set in config

    // API does not pass back a task
    Ok("✓".into())
}

/// Deletes a task by ID.
pub async fn delete_task(config: &Config, task_id: &str, spinner: bool) -> Result<String, Error> {
    let body = json!({});
    let url = format!("{TASKS_URL}{task_id}");

    request::delete_todoist(config, &url, body, spinner).await?;
    Ok("✓".into())
}

/// Deletes a project by ID.
pub async fn delete_project(
    config: &Config,
    project: &Project,
    spinner: bool,
) -> Result<String, Error> {
    let url = format!("{}/{}", PROJECTS_URL, project.id);
    let body = json!({});

    request::delete_todoist(config, &url, body, spinner).await?;
    Ok("✓".into())
}
/// Creates a new project in Todoist.
pub async fn create_project(
    config: &Config,
    name: &str,
    description: &str,
    is_favorite: bool,
    spinner: bool,
) -> Result<Project, Error> {
    let url = PROJECTS_URL.to_string();
    let body = json!({"name": name, "description": description, "is_favorite": is_favorite});

    let json = request::post_todoist(config, &url, body, spinner).await?;
    Project::from_json(&json)
}

/// Updates a project's writable fields. Only provided `Some(...)` fields are sent
/// in the request body; `None` fields are omitted.
pub async fn update_project(
    config: &Config,
    project_id: &str,
    name: Option<&str>,
    color: Option<&str>,
    is_favorite: Option<bool>,
    view_style: Option<&str>,
    spinner: bool,
) -> Result<Project, Error> {
    let url = format!("{PROJECTS_URL}/{project_id}");
    let mut body = json!({});

    if let Some(name) = name {
        body["name"] = json!(name);
    }
    if let Some(color) = color {
        body["color"] = json!(color);
    }
    if let Some(is_favorite) = is_favorite {
        body["is_favorite"] = json!(is_favorite);
    }
    if let Some(view_style) = view_style {
        body["view_style"] = json!(view_style);
    }

    let json = request::post_todoist(config, &url, body, spinner).await?;
    Project::from_json(&json)
}

/// Archives a project by ID. The Todoist API returns 204 No Content.
pub async fn archive_project(
    config: &Config,
    project_id: &str,
    spinner: bool,
) -> Result<String, Error> {
    let url = format!("{PROJECTS_URL}/{project_id}/archive");
    let body = json!({});

    request::post_todoist(config, &url, body, spinner).await?;
    Ok("✓".into())
}

/// Unarchives a project by ID. The Todoist API returns 204 No Content.
pub async fn unarchive_project(
    config: &Config,
    project_id: &str,
    spinner: bool,
) -> Result<String, Error> {
    let url = format!("{PROJECTS_URL}/{project_id}/unarchive");
    let body = json!({});

    request::post_todoist(config, &url, body, spinner).await?;
    Ok("✓".into())
}

/// Creates a new section in a project.
pub async fn create_section(
    config: &Config,
    name: &str,
    project: &Project,
    spinner: bool,
) -> Result<Section, Error> {
    let url = SECTIONS_URL.to_string();
    let body = json!({"name": name, "project_id": project.id});

    let json = request::post_todoist(config, &url, body, spinner).await?;
    Section::from_json(&json)
}

/// Deletes a section by ID.
pub async fn delete_section(
    config: &Config,
    section_id: &str,
    spinner: bool,
) -> Result<String, Error> {
    let url = format!("{SECTIONS_URL}/{}", section_id);
    let body = json!({});

    request::delete_todoist(config, &url, body, spinner).await?;
    Ok("✓".into())
}

/// Creates a comment on a task.
pub async fn create_comment(
    config: &Config,
    task_id: &str,
    content: &str,
    spinner: bool,
) -> Result<Comment, Error> {
    let body = json!({"task_id": task_id, "content": content});
    let url = COMMENTS_URL.to_string();

    let response = request::post_todoist(config, &url, body, spinner).await?;
    maybe_run_command(config.task_comment_command.as_deref(), config)?;
    Comment::from_json(&response)
}

/// Fetches the authenticated user's data.
pub async fn get_user_data(config: &Config) -> Result<User, Error> {
    let url = USER_URL.to_string();
    let json = request::get_todoist(config, &url, true).await?;
    User::from_json(&json)
}

/// Returns all of the comments for a task from the Todoist JSON API
/// Paginates through the results until all comments are retrieved.
/// Then will filter out deleted and excluded comments based on the Regex Config.
pub async fn all_comments(
    config: &Config,
    task_id: &str,
    limit: Option<u8>,
) -> Result<Vec<Comment>, Error> {
    let limit = limit.unwrap_or(QUERY_LIMIT);
    let mut url = format!("{COMMENTS_URL}?task_id={task_id}&limit={limit}");
    let mut comments: Vec<Comment> = Vec::new();

    let exclude_regex = config.comment_exclude_regex.as_ref();

    loop {
        let json = request::get_todoist(config, &url, true).await?;
        let CommentResponse {
            results,
            next_cursor,
        } = CommentResponse::from_json(&json)?;

        comments.extend(results.into_iter().filter(|c| {
            !c.is_deleted
                && match exclude_regex {
                    Some(regex) => !regex.is_match(&c.content),
                    None => true,
                }
        }));

        match next_cursor {
            None => break,
            Some(cursor) => {
                url =
                    format!("{COMMENTS_URL}?task_id={task_id}&limit={QUERY_LIMIT}&cursor={cursor}");
            }
        }
    }

    Ok(comments)
}

// Executes a CLI command (if set in the configuration).
fn maybe_run_command(command: Option<&str>, config: &Config) -> Result<(), Error> {
    if let Some(command) = command {
        let tx = config.internal.tx.clone().ok_or_else(|| {
            Error::new(
                "shell command",
                "Unable to report shell command errors because no async error channel is configured",
            )
        })?;
        execute_command(command, tx);
    }

    Ok(())
}

/// Filters tasks by title regex if configured.
pub fn filter_tasks_by_title(
    tasks: Vec<Task>,
    regex: Option<&Regex>,
    config: &Config,
) -> Vec<Task> {
    match regex {
        Some(re) => tasks
            .into_iter()
            .filter(|task| {
                let exclude = re.is_match(&task.content);
                if exclude {
                    maybe_print(
                        config,
                        &format!("Task '{}' excluded by regex", task.content),
                    );
                }
                !exclude // exclude matching tasks
            })
            .collect(),
        None => tasks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tasks::priority::{self, Priority};
    use crate::test;
    use crate::test::responses::ResponseFromFile;
    use crate::test_time::FixedTimeProvider;
    use crate::time::TimeProviderEnum;
    use crate::users::TzInfo;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn test_get_user_data() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/api/v1/user")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::User.read().await)
            .create_async()
            .await;

        let config = test::fixtures::config().await.with_mock_url(server.url());

        assert_eq!(
            get_user_data(&config).await,
            Ok(User {
                tz_info: TzInfo {
                    timezone: "America/Vancouver".to_string()
                }
            })
        );
        mock.assert();
    }

    #[tokio::test]
    async fn test_quick_create_task() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/api/v1/tasks/quick")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::TodayTask.read().await)
            .create_async()
            .await;

        let config = test::fixtures::config()
            .await
            .with_mock_url(server.url())
            .with_time_provider(TimeProviderEnum::Fixed(FixedTimeProvider));

        assert_eq!(
            quick_create_task(&config, "testy test", None).await,
            Ok(test::fixtures::today_task().await)
        );
        mock.assert();
    }

    #[tokio::test]
    async fn test_all_labels() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/api/v1/labels?limit=200")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::Labels.read().await)
            .create_async()
            .await;

        let config = test::fixtures::config().await.with_mock_url(server.url());

        assert_eq!(
            all_labels(&config, false, None).await,
            Ok(vec![test::fixtures::label()])
        );
        mock.assert();
    }

    #[tokio::test]
    async fn test_create_task() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/api/v1/tasks/")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::TodayTask.read().await)
            .create_async()
            .await;

        let config = test::fixtures::config()
            .await
            .with_mock_url(server.url())
            .with_time_provider(TimeProviderEnum::Fixed(FixedTimeProvider));

        let project = test::fixtures::project();

        let priority = priority::Priority::None;
        let section = test::fixtures::section();
        assert_eq!(
            create_task(
                &config,
                "New task",
                &project,
                Some(&section),
                priority,
                "",
                None,
                &[],
                None,
            )
            .await,
            Ok(test::fixtures::today_task().await)
        );
        mock.assert();
    }

    #[tokio::test]
    async fn test_create_task_with_parent_id() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/api/v1/tasks/")
            .match_body(mockito::Matcher::Regex(r#""parent_id":"999""#.to_string()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::TodayTask.read().await)
            .create_async()
            .await;

        let config = test::fixtures::config()
            .await
            .with_mock_url(server.url())
            .with_time_provider(TimeProviderEnum::Fixed(FixedTimeProvider));

        let project = test::fixtures::project();
        let priority = priority::Priority::None;

        let result = create_task(
            &config,
            "Subtasks",
            &project,
            None,
            priority,
            "",
            None,
            &[],
            Some("999"),
        )
        .await;

        assert!(
            result.is_ok(),
            "create_task with parent_id should succeed; got: {result:?}"
        );
        mock.assert();
    }

    #[tokio::test]
    async fn test_create_task_without_parent_id_omits_field() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/api/v1/tasks/")
            .match_body(mockito::Matcher::AllOf(vec![
                mockito::Matcher::Regex(r#""content":"No parent task""#.to_string()),
                mockito::Matcher::Regex(r#""project_id":"123""#.to_string()),
            ]))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::TodayTask.read().await)
            .create_async()
            .await;

        let config = test::fixtures::config()
            .await
            .with_mock_url(server.url())
            .with_time_provider(TimeProviderEnum::Fixed(FixedTimeProvider));

        let project = test::fixtures::project();
        let priority = priority::Priority::None;

        let result = create_task(
            &config,
            "No parent task",
            &project,
            None,
            priority,
            "",
            None,
            &[],
            None,
        )
        .await;

        assert!(
            result.is_ok(),
            "create_task without parent_id should succeed; got: {result:?}"
        );
        mock.assert();
    }

    #[tokio::test]
    async fn test_create_section() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/api/v1/sections")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::Section.read().await)
            .create_async()
            .await;

        let config = test::fixtures::config().await.with_mock_url(server.url());

        let project = test::fixtures::project();

        assert_eq!(
            create_section(&config, "New task", &project, false).await,
            Ok(test::fixtures::section())
        );
        mock.assert();
    }

    #[tokio::test]
    async fn test_create_label() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/api/v1/labels")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::Label.read().await)
            .create_async()
            .await;

        let config = test::fixtures::config().await.with_mock_url(server.url());

        let result = create_label(&config, "test-label", Some("red"), None, false, false).await;
        assert_eq!(result, Ok(test::fixtures::label()));
        mock.assert();
    }

    #[tokio::test]
    async fn test_create_comment() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/api/v1/comments/")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::Comment.read().await)
            .create_async()
            .await;

        let config = test::fixtures::config().await.with_mock_url(server.url());
        let task = test::fixtures::today_task().await;
        let comment = test::fixtures::comment();
        assert_eq!(
            create_comment(&config, &task.id, "New comment", true).await,
            Ok(comment)
        );
        mock.assert();
    }

    #[tokio::test]
    async fn test_all_tasks_by_project() {
        let mut server = mockito::Server::new_async().await;

        let mock = server
            .mock("GET", "/api/v1/tasks/?project_id=123&limit=200")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::TodayTasks.read().await)
            .create_async()
            .await;

        let config = test::fixtures::config().await.with_mock_url(server.url());
        let config_with_timezone = config.with_timezone("US/Pacific");
        let binding = config_with_timezone
            .projects()
            .await
            .expect("Failed to fetch projects asynchronously");
        let project = binding
            .first()
            .expect("Expected at least one project in binding");

        assert_eq!(
            all_tasks_by_project(&config_with_timezone, project, None).await,
            Ok(vec![test::fixtures::today_task().await])
        );

        mock.assert();
    }

    #[tokio::test]
    async fn test_complete_task() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/api/v1/tasks/6Xqhv4cwxgjwG9w8/close")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::TodayTask.read().await)
            .create_async()
            .await;

        let config = test::fixtures::config().await.with_mock_url(server.url());

        let task = test::fixtures::today_task().await;
        let response = complete_task(&config, &task.id, false)
            .await
            .expect("Did not complete task");
        mock.assert();
        assert_eq!(response, String::from("✓"));
    }

    #[tokio::test]
    async fn test_move_task_to_project() {
        let mut server = mockito::Server::new_async().await;

        let mock = server
            .mock("POST", "/api/v1/tasks/6Xqhv4cwxgjwG9w8/move")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::TodayTask.read().await)
            .create_async()
            .await;

        let task = test::fixtures::today_task().await;
        let config = test::fixtures::config()
            .await
            .with_mock_url(server.url())
            .with_time_provider(TimeProviderEnum::Fixed(FixedTimeProvider));

        let binding = config
            .projects()
            .await
            .expect("Failed to fetch projects asynchronously");
        let project = binding
            .first()
            .expect("Expected at least one project in binding");
        let response = move_task_to_project(&config, &task, project, false)
            .await
            .expect("Could not move task to project");

        assert_eq!(response, task);
        mock.assert();
    }
    #[tokio::test]
    async fn test_move_task_to_section() {
        let mut server = mockito::Server::new_async().await;

        let mock = server
            .mock("POST", "/api/v1/tasks/6Xqhv4cwxgjwG9w8/move")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::TodayTask.read().await)
            .create_async()
            .await;

        let task = test::fixtures::today_task().await;
        let config = test::fixtures::config()
            .await
            .with_mock_url(server.url())
            .with_time_provider(TimeProviderEnum::Fixed(FixedTimeProvider));

        let section = test::fixtures::section();
        let response = move_task_to_section(&config, &task, &section, false)
            .await
            .expect("Could not move task to section");

        assert_eq!(response, task);
        mock.assert();
    }

    #[tokio::test]
    async fn test_delete_task() {
        let mut server = mockito::Server::new_async().await;

        let mock = server
            .mock("DELETE", "/api/v1/tasks/6Xqhv4cwxgjwG9w8")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::TodayTask.read().await)
            .create_async()
            .await;

        let task = test::fixtures::today_task().await;
        let config = test::fixtures::config().await.with_mock_url(server.url());

        let response = delete_task(&config, &task.id, false).await;
        mock.assert();

        assert_eq!(response, Ok(String::from("✓")));
    }

    #[tokio::test]
    async fn test_delete_section() {
        let mut server = mockito::Server::new_async().await;

        let mock = server
            .mock("DELETE", "/api/v1/sections/1234")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body("null")
            .create_async()
            .await;

        let config = test::fixtures::config().await.with_mock_url(server.url());

        let result = delete_section(&config, "1234", false).await;
        assert_eq!(result, Ok("✓".into()));
        mock.assert();
    }

    #[tokio::test]
    async fn test_get_task() {
        let mut server = mockito::Server::new_async().await;

        let mock = server
            .mock("GET", "/api/v1/tasks/5149481867")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::TodayTask.read().await)
            .create_async()
            .await;

        let config = test::fixtures::config().await.with_mock_url(server.url());

        let response = get_task(&config, "5149481867")
            .await
            .expect("could not get task");
        mock.assert();

        assert_eq!(response.id, String::from("6Xqhv4cwxgjwG9w8"));
        assert_eq!(response.project_id, String::from("6VRRxv8CM6GVmmgf"));
    }

    #[tokio::test]
    async fn test_create_reminder() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/api/v1/reminders")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::Reminder.read().await)
            .create_async()
            .await;

        let config = test::fixtures::config().await.with_mock_url(server.url());
        let task = test::fixtures::today_task().await;

        let response = create_reminder(&config, &task, "2026-01-18 17:00", false).await;
        mock.assert();

        assert_eq!(response, Ok(test::fixtures::reminder()));
    }

    #[tokio::test]
    async fn test_all_reminders() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/api/v1/reminders?limit=200")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::Reminders.read().await)
            .create_async()
            .await;

        let config = test::fixtures::config().await.with_mock_url(server.url());

        let response = all_reminders(&config, None).await;
        mock.assert();

        assert_eq!(response, Ok(vec![test::fixtures::reminder()]));
    }

    #[tokio::test]
    async fn test_reminders_forbidden_shows_pro_plan_message() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/api/v1/reminders?limit=200")
            .with_status(403)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::Reminders.read().await)
            .create_async()
            .await;

        let config = test::fixtures::config().await.with_mock_url(server.url());

        let error = all_reminders(&config, None).await.unwrap_err();
        mock.assert();
        assert_eq!(error.source, String::from("reqwest"));
        assert_eq!(
            error.message,
            String::from(
                "Reminders are only available on Pro Todoist plans. Upgrade to Todoist Pro to access reminder features."
            )
        );
    }

    #[tokio::test]
    async fn test_forbidden() {
        let mut server = mockito::Server::new_async().await;

        let mock = server
            .mock("GET", "/api/v1/tasks/5149481867")
            .with_status(403)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::TodayTask.read().await)
            .create_async()
            .await;

        let config = test::fixtures::config().await.with_mock_url(server.url());

        let error = get_task(&config, "5149481867").await.unwrap_err();
        mock.assert();
        assert_eq!(error.source, String::from("reqwest"));
        assert_eq!(
            error.message,
            String::from(
                "Unauthorized or Forbidden response from Todoist\nRun 'tod auth login' to reauthenticate"
            )
        );
    }

    #[tokio::test]
    async fn test_update_task_priority() {
        let task = test::fixtures::today_task().await;
        let url: &str = &format!("{}{}", "/api/v1/tasks/", task.id);
        let mut server = mockito::Server::new_async().await;

        let mock = server
            .mock("POST", url)
            .with_status(204)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::TodayTask.read().await)
            .create_async()
            .await;

        let config = test::fixtures::config().await.with_mock_url(server.url());

        let response = update_task_priority(&config, &task.id, &Priority::High, true).await;
        mock.assert();
        assert_eq!(response, Ok(String::from("✓")));
    }

    #[tokio::test]
    async fn test_update_task_due_natural_language() {
        let task = test::fixtures::today_task().await;
        let url: &str = &format!("{}{}", "/api/v1/tasks/", task.id);
        let mut server = mockito::Server::new_async().await;

        let mock = server
            .mock("POST", url)
            .with_status(204)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::TodayTasks.read().await)
            .create_async()
            .await;

        let config = test::fixtures::config().await.with_mock_url(server.url());

        let response =
            update_task_due_natural_language(&config, &task, "today".to_string(), None, true).await;
        mock.assert();
        assert_eq!(response, Ok(String::from("✓")));
    }

    #[tokio::test]
    async fn test_all_comments_filters_deleted() {
        let mut server = mockito::Server::new_async().await;

        let mock = server
            .mock(
                "GET",
                "/api/v1/comments/?task_id=6Xqhv4cwxgjwG9w8&limit=200",
            )
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::CommentsAllTypes.read().await)
            .create_async()
            .await;

        let config = test::fixtures::config().await.with_mock_url(server.url());

        let task = test::fixtures::today_task().await;

        let comments = all_comments(&config, &task.id, None)
            .await
            .expect("Could not get all comments");
        mock.assert();

        assert_eq!(comments.len(), 7); // One comment in the JSON is_deleted = true
        assert!(comments.iter().all(|c| !c.is_deleted));
    }
    #[tokio::test]
    async fn test_task_is_filtered_out_by_regex() {
        let mut task = test::fixtures::today_task().await;
        task.content = "Brush Teeth".to_string();

        let mut config = test::fixtures::config().await;
        config.task_exclude_regex =
            Some(regex::Regex::new(r"^Brush").expect("Could not create regex"));

        let result = filter_tasks_by_title(vec![task], config.task_exclude_regex.as_ref(), &config);
        assert!(result.is_empty(), "Expected task to be excluded by regex");
    }

    #[tokio::test]
    async fn test_task_is_retained_if_not_matching_regex() {
        let mut task = test::fixtures::today_task().await;
        task.content = "Eat Breakfast".to_string();

        let mut config = test::fixtures::config().await;
        config.task_exclude_regex =
            Some(regex::Regex::new(r"^Brush").expect("Could not create regex"));

        let result = filter_tasks_by_title(
            vec![task.clone()],
            config.task_exclude_regex.as_ref(),
            &config,
        );
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content, task.content);
    }

    #[tokio::test]
    async fn test_update_project_hits_api() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/api/v1/projects/123")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::Project.read().await)
            .create_async()
            .await;

        let config = test::fixtures::config().await.with_mock_url(server.url());

        let result = update_project(&config, "123", Some("NewName"), None, None, None, false).await;

        assert!(result.is_ok());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_archive_project_hits_api() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/api/v1/projects/123/archive")
            .with_status(204)
            .create_async()
            .await;

        let config = test::fixtures::config().await.with_mock_url(server.url());

        let result = archive_project(&config, "123", false).await;

        assert_eq!(result, Ok("✓".to_string()));
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_unarchive_project_hits_api() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/api/v1/projects/123/unarchive")
            .with_status(204)
            .create_async()
            .await;

        let config = test::fixtures::config().await.with_mock_url(server.url());

        let result = unarchive_project(&config, "123", false).await;

        assert_eq!(result, Ok("✓".to_string()));
        mock.assert_async().await;
    }
}
