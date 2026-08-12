use clap::{Parser, Subcommand};

use crate::{
    config::Config,
    errors::Error,
    filters, format,
    input::{self, DateTimeInput},
    labels,
    lists::Flag,
    projects, sections,
    tasks::{self, TaskAttribute, priority::Priority},
    todoist,
};

/// Task subcommands (create, edit, complete, etc.).
#[derive(Subcommand, Debug, Clone)]
pub enum TaskCommands {
    #[clap(alias = "q")]
    /// (q) Create a new task using NLP
    QuickAdd(QuickAdd),

    #[clap(alias = "c")]
    /// (c) Create a new task (without NLP)
    Create(Create),

    #[clap(alias = "e")]
    /// (e) Edit an existing task's content
    Edit(Edit),

    #[clap(alias = "n")]
    /// (n) Get the next task by priority
    Next(Next),

    #[clap(alias = "o")]
    /// (o) Complete the last task fetched with the next command
    Complete(Complete),

    #[clap(alias = "r")]
    /// (r) Reopen the last completed task
    Reopen(Reopen),

    #[clap(alias = "m")]
    /// (m) Add a comment to the last task fetched with the next command
    Comment(Comment),

    #[clap(alias = "ec")]
    /// (ec) Edit an existing comment's content
    EditComment(EditComment),

    #[clap(alias = "dc")]
    /// (dc) Delete a comment by ID
    DeleteComment(DeleteComment),
}

#[derive(Parser, Debug, Clone)]
/// Creates a task using Todoist quick-add NLP.
pub struct QuickAdd {
    #[arg(short, long, num_args(1..))]
    /// Content for task. Add a reminder at the end by prefixing the natural language date with `!`.
    /// Example: Get milk on sunday !saturday 4pm
    content: Option<Vec<String>>,
}

#[derive(Parser, Debug, Clone)]
/// Creates a task with structured fields.
pub struct Create {
    #[arg(short, long)]
    /// The project into which the task will be added
    project: Option<String>,

    #[arg(short = 'u', long)]
    /// Due date in format YYYY-MM-DD, YYYY-MM-DD HH:MM, or natural language
    due: Option<String>,

    #[arg(short, long, default_value_t = String::new())]
    /// Description for task
    description: String,

    #[arg(short, long)]
    /// Content for task
    content: Option<String>,

    #[arg(short, long, default_value_t = false)]
    /// Do not prompt for section
    no_section: bool,

    #[arg(short = 'r', long)]
    /// Priority from 1 (without priority) to 4 (highest)
    priority: Option<u8>,

    #[arg(short, long)]
    /// List of labels to choose from, to be applied to each entry. Use flag once per label
    label: Vec<String>,

    #[arg(long)]
    /// Parent task ID to create this task as a subtask of
    parent_id: Option<String>,
}

#[derive(Parser, Debug, Clone)]
/// Edits an existing task's attributes.
pub struct Edit {
    #[arg(short, long)]
    /// The project containing the task
    project: Option<String>,

    #[arg(short, long)]
    /// The filter containing the task
    filter: Option<String>,
}

#[derive(Parser, Debug, Clone)]
/// Fetches the next task by priority.
pub struct Next {
    #[arg(short, long)]
    /// The project containing the task
    project: Option<String>,

    #[arg(short, long)]
    /// The filter containing the task
    filter: Option<String>,
}

#[derive(Parser, Debug, Clone)]
/// Completes the last task fetched with the next command.
pub struct Complete {}

#[derive(Parser, Debug, Clone)]
/// Reopens the last completed task.
pub struct Reopen {}

#[derive(Parser, Debug, Clone)]
/// Adds a comment to the last task fetched with the next command.
pub struct Comment {
    #[arg(short, long)]
    /// Content for comment
    content: Option<String>,
}
/// Creates a task using natural language quick-add.
pub async fn quick_add(config: &Config, args: &QuickAdd, json: bool) -> Result<String, Error> {
    let QuickAdd { content } = args;
    let maybe_string = content.as_ref().map(|c| c.join(" "));
    let content = super::fetch_string(maybe_string.as_deref(), config, input::CONTENT)?;
    let (content, reminder) = if let Some(index) = content.find('!') {
        let (before, after) = content.split_at(index);
        // after starts with '!', so skip it
        (
            before.trim().to_string(),
            Some(after[1..].trim().to_string()),
        )
    } else {
        (content, None)
    };
    let task = todoist::quick_create_task(config, &content, reminder).await?;
    if json {
        Ok(serde_json::to_string(&task)?)
    } else {
        Ok(format::green_string("✓"))
    }
}

/// User does not want to use sections
fn is_no_sections(args: &Create, config: &Config) -> bool {
    args.no_section || config.no_sections.unwrap_or_default()
}

/// Creates a task with structured fields and optional interactive prompts.
pub async fn create(config: Config, args: &Create, json: bool) -> Result<String, Error> {
    let task = if no_flags_used(args) {
        if config.args.json {
            return Err(Error::new("json_mode", super::JSON_INTERACTIVE_ERROR));
        }
        let options = tasks::create_task_attributes();
        let selections = input::multi_select(input::ATTRIBUTES, options, &config.mock_select)?;

        let content = super::fetch_string(None, &config, input::CONTENT)?;

        let description = if selections.contains(&TaskAttribute::Description) {
            super::fetch_string(None, &config, input::DESCRIPTION)?
        } else {
            String::new()
        };

        let priority = if selections.contains(&TaskAttribute::Priority) {
            super::fetch_priority(None, &config)?
        } else {
            Priority::None
        };
        let due = if selections.contains(&TaskAttribute::Due) {
            let datetime_input = input::datetime(
                &config.mock_select,
                config.mock_string.clone(),
                config.natural_language_only,
                false,
                false,
            )?;

            match datetime_input {
                DateTimeInput::Skip | DateTimeInput::Complete => unreachable!(),
                DateTimeInput::None => None,
                DateTimeInput::Text(datetime) => Some(datetime),
            }
        } else {
            None
        };

        let labels = if selections.contains(&TaskAttribute::Labels) {
            let all_labels = labels::get_labels(&config, false).await?;
            input::multi_select(input::LABELS, all_labels, &config.mock_select)?
        } else {
            Vec::new()
        }
        .into_iter()
        .map(|l| l.name.clone())
        .collect::<Vec<String>>();

        let project = match super::fetch_project(args.project.as_deref(), &config).await? {
            Flag::Project(project) => project,
            Flag::Filter(_) => unreachable!(),
        };

        let section = if is_no_sections(args, &config) {
            None
        } else {
            sections::select_section(&config, &project).await?
        };

        todoist::create_task(
            &config,
            &content,
            &project,
            section.as_ref(),
            priority,
            &description,
            due.as_deref(),
            &labels,
            args.parent_id.as_deref(),
        )
        .await?
    } else {
        let Create {
            project,
            due,
            description,
            content,
            priority,
            label: labels,
            no_section: _no_section,
            parent_id,
        } = args;
        let project = match super::fetch_project(project.as_deref(), &config).await? {
            Flag::Project(project) => project,
            Flag::Filter(_) => unreachable!(),
        };

        let section = if is_no_sections(args, &config) {
            None
        } else {
            sections::select_section(&config, &project).await?
        };
        let content = super::fetch_string(content.as_deref(), &config, input::CONTENT)?;
        let priority = super::fetch_priority(*priority, &config)?;

        todoist::create_task(
            &config,
            &content,
            &project,
            section.as_ref(),
            priority,
            description,
            due.as_deref(),
            labels,
            parent_id.as_deref(),
        )
        .await?
    };
    if json {
        Ok(serde_json::to_string(&task)?)
    } else {
        Ok(format::green_string("✓"))
    }
}

fn no_flags_used(args: &Create) -> bool {
    let Create {
        project,
        due,
        description,
        content,
        no_section: _no_section,
        priority,
        label,
        parent_id,
    } = args;

    project.is_none()
        && due.is_none()
        && description.is_empty()
        && content.is_none()
        && priority.is_none()
        && label.is_empty()
        && parent_id.is_none()
}

/// Edits a task's attributes interactively. Blocks interactive prompts in JSON mode.
pub async fn edit(config: Config, args: &Edit, json: bool) -> Result<String, Error> {
    if json {
        return Err(Error::new("json_mode", super::JSON_INTERACTIVE_ERROR));
    }
    let Edit { project, filter } = args;
    match super::fetch_project_or_filter(project.as_deref(), filter.as_deref(), &config).await? {
        Flag::Project(project) => projects::edit_task(&config, &project).await,
        Flag::Filter(filter) => filters::edit_task(&config, filter).await,
    }
}
/// Fetches the next task by priority and stores it in config.
pub async fn next(config: Config, args: &Next, json: bool) -> Result<String, Error> {
    let Next { project, filter } = args;
    match super::fetch_project_or_filter(project.as_deref(), filter.as_deref(), &config).await? {
        Flag::Project(project) => projects::next_task(config, &project, json).await,
        Flag::Filter(filter) => filters::next_task(&config, &filter, json).await,
    }
}

/// Completes the stored next task.
pub async fn complete(config: Config, _args: &Complete, json: bool) -> Result<String, Error> {
    match config.next_task() {
        Some(task) => {
            todoist::complete_task(&config, &task.id, true).await?;

            if json {
                Ok(serde_json::to_string(&task)?)
            } else {
                Ok(format::green_string("Task completed successfully"))
            }
        }
        None => Err(Error::new(
            "task_complete",
            "There is nothing to complete. A task must first be marked as 'next'.",
        )),
    }
}

/// Reopens the last completed task.
pub async fn reopen(config: Config, _args: &Reopen, json: bool) -> Result<String, Error> {
    match config.last_completed_task() {
        Some(task) => {
            todoist::reopen_task(&config, &task.id, true).await?;

            if json {
                Ok(serde_json::to_string(&task)?)
            } else {
                Ok(format::green_string("Task reopened successfully"))
            }
        }
        None => Err(Error::new(
            "task_reopen",
            "There is nothing to reopen. A task must first be completed with 'tod task complete'.",
        )),
    }
}

#[derive(Parser, Debug, Clone)]
/// (ec) Edit an existing comment's content
pub struct EditComment {
    /// ID of the comment to edit
    comment_id: String,

    /// New content for the comment
    content: Option<String>,
}

#[derive(Parser, Debug, Clone)]
/// (dc) Delete a comment by ID
pub struct DeleteComment {
    /// ID of the comment to delete
    comment_id: String,
}

/// Edits the content of an existing comment by ID.
pub async fn edit_comment(config: Config, args: &EditComment, json: bool) -> Result<String, Error> {
    let EditComment {
        comment_id,
        content,
    } = args;

    let content = super::fetch_string(content.as_deref(), &config, input::CONTENT)?;
    let comment = todoist::update_comment(&config, comment_id, &content, true).await?;

    if json {
        Ok(serde_json::to_string(&comment)?)
    } else {
        Ok(format::green_string("Comment updated successfully"))
    }
}

/// Deletes a comment by ID.
pub async fn delete_comment(
    config: Config,
    args: &DeleteComment,
    json: bool,
) -> Result<String, Error> {
    let result = todoist::delete_comment(&config, &args.comment_id, true).await?;

    if json {
        Ok(serde_json::to_string(&result)?)
    } else {
        Ok(format::green_string("Comment deleted successfully"))
    }
}
/// Adds a comment to the stored next task.
pub async fn comment(config: Config, args: &Comment, json: bool) -> Result<String, Error> {
    let Comment { content } = args;
    match config.next_task() {
        Some(task) => {
            let content = super::fetch_string(content.as_deref(), &config, input::CONTENT)?;
            let comment = todoist::create_comment(&config, &task.id, &content, true).await?;
            if json {
                Ok(serde_json::to_string(&comment)?)
            } else {
                Ok(format::green_string("Comment created successfully"))
            }
        }
        None => Err(Error::new(
            "task_comment",
            "There is nothing to comment on. A task must first be marked as 'next'.",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test;
    use crate::test::responses::ResponseFromFile;
    use crate::test_time::FixedTimeProvider;
    use crate::time::TimeProviderEnum;

    fn create_args() -> Create {
        Create {
            project: None,
            due: None,
            description: String::new(),
            content: None,
            no_section: false,
            priority: None,
            label: Vec::new(),
            parent_id: None,
        }
    }

    #[test]
    fn no_flags_used_returns_true_for_default_create_args() {
        let args = create_args();
        assert!(no_flags_used(&args));
    }

    #[test]
    fn no_flags_used_returns_false_when_any_flag_is_set() {
        let mut args = create_args();
        args.priority = Some(3);
        assert!(!no_flags_used(&args));
    }

    #[test]
    fn no_flags_used_returns_false_when_parent_id_is_set() {
        let mut args = create_args();
        args.parent_id = Some("123".to_string());
        assert!(!no_flags_used(&args));
    }

    #[test]
    fn is_no_sections_respects_argument_flag() {
        let mut args = create_args();
        args.no_section = true;

        let config = Config::default_test();
        assert!(is_no_sections(&args, &config));
    }

    #[test]
    fn is_no_sections_respects_config_setting() {
        let args = create_args();
        let mut config = Config::default_test();
        config.no_sections = Some(true);

        assert!(is_no_sections(&args, &config));
    }

    #[test]
    fn is_no_sections_returns_false_when_both_are_disabled() {
        let args = create_args();
        let config = Config::default_test();

        assert!(!is_no_sections(&args, &config));
    }

    #[tokio::test]
    async fn edit_routes_to_filter_and_edits_content() {
        let tasks_body = ResponseFromFile::TodayTasks.read().await;
        let _first_task = crate::tasks::TaskResponse::from_json(&tasks_body)
            .expect("should parse tasks")
            .results
            .into_iter()
            .next()
            .expect("should have at least one task");

        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock(
                "GET",
                mockito::Matcher::Regex(r"^/api/v1/tasks/filter\?query=myfilter.*".to_string()),
            )
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(&tasks_body)
            .create_async()
            .await;

        let config = test::fixtures::config()
            .await
            .with_mock_url(server.url())
            .with_time_provider(TimeProviderEnum::Fixed(FixedTimeProvider))
            .mock_select(0);

        let args = Edit {
            project: None,
            filter: Some("myfilter".to_string()),
        };

        let result = edit(config, &args, false).await;

        assert!(result.is_ok(), "edit should succeed; got: {result:?}");
        assert!(result.unwrap().contains("Finished editing"));
        mock.assert();
    }

    #[tokio::test]
    async fn next_routes_to_filter_and_finds_task() {
        let tasks_body = ResponseFromFile::TodayTasks.read().await;

        let mut server = mockito::Server::new_async().await;
        let tasks_mock = server
            .mock(
                "GET",
                mockito::Matcher::Regex(r"^/api/v1/tasks/filter\?query=myfilter.*".to_string()),
            )
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(&tasks_body)
            .create_async()
            .await;
        let comments_mock = server
            .mock(
                "GET",
                mockito::Matcher::Regex(r"^/api/v1/comments/\?task_id=.*".to_string()),
            )
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body("{\"results\":[],\"next_cursor\":null}")
            .create_async()
            .await;

        let config = test::fixtures::config()
            .await
            .with_mock_url(server.url())
            .with_time_provider(TimeProviderEnum::Fixed(FixedTimeProvider));
        config
            .touch_file()
            .await
            .expect("should create config file");

        let args = Next {
            project: None,
            filter: Some("myfilter".to_string()),
        };

        let result = next(config, &args, false).await;

        assert!(result.is_ok(), "next should succeed; got: {result:?}");
        assert!(result.unwrap().contains("task(s) remaining"));
        tasks_mock.assert();
        comments_mock.assert();
    }

    #[tokio::test]
    async fn edit_comment_happy_path() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/api/v1/comments/123")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::Comment.read().await)
            .create_async()
            .await;

        let config = test::fixtures::config().await.with_mock_url(server.url());

        let args = EditComment {
            comment_id: "123".to_string(),
            content: Some("new content".to_string()),
        };
        let result = edit_comment(config, &args, false).await;
        assert!(
            result.is_ok(),
            "edit_comment should succeed; got: {result:?}"
        );
        assert!(result.unwrap().contains("Comment updated successfully"));
        mock.assert();
    }

    #[tokio::test]
    async fn edit_comment_json_output() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/api/v1/comments/123")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::Comment.read().await)
            .create_async()
            .await;

        let config = test::fixtures::config().await.with_mock_url(server.url());

        let args = EditComment {
            comment_id: "123".to_string(),
            content: Some("new content".to_string()),
        };
        let result = edit_comment(config, &args, true).await;
        assert!(result.is_ok());
        let json = result.unwrap();
        assert!(json.contains("\"id\""));
        assert!(json.contains("2992679862"));
        mock.assert();
    }

    #[tokio::test]
    async fn edit_comment_json_mode_no_content_errors() {
        let mut config = test::fixtures::config().await;
        config.args.json = true;

        let args = EditComment {
            comment_id: "123".to_string(),
            content: None,
        };
        let result = edit_comment(config, &args, true).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().source, "json_mode");
    }

    #[tokio::test]
    async fn delete_comment_happy_path() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("DELETE", "/api/v1/comments/123")
            .with_status(204)
            .create_async()
            .await;

        let config = test::fixtures::config().await.with_mock_url(server.url());

        let args = DeleteComment {
            comment_id: "123".to_string(),
        };
        let result = delete_comment(config, &args, false).await;
        assert!(
            result.is_ok(),
            "delete_comment should succeed; got: {result:?}"
        );
        assert!(result.unwrap().contains("Comment deleted successfully"));
        mock.assert();
    }

    #[tokio::test]
    async fn delete_comment_json_output() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("DELETE", "/api/v1/comments/123")
            .with_status(204)
            .create_async()
            .await;

        let config = test::fixtures::config().await.with_mock_url(server.url());

        let args = DeleteComment {
            comment_id: "123".to_string(),
        };
        let result = delete_comment(config, &args, true).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "\"✓\"");
        mock.assert();
    }
}
