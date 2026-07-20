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

    #[clap(alias = "m")]
    /// (m) Add a comment to the last task fetched with the next command
    Comment(Comment),
}

#[derive(Parser, Debug, Clone)]
pub struct QuickAdd {
    #[arg(short, long, num_args(1..))]
    /// Content for task. Add a reminder at the end by prefixing the natural language date with `!`.
    /// Example: Get milk on sunday !saturday 4pm
    content: Option<Vec<String>>,
}

#[derive(Parser, Debug, Clone)]
pub struct Create {
    #[arg(short, long)]
    /// The project into which the task will be added
    project: Option<String>,

    #[arg(short = 'u', long)]
    /// Date date in format YYYY-MM-DD, YYYY-MM-DD HH:MM, or natural language
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
}

#[derive(Parser, Debug, Clone)]
pub struct Edit {
    #[arg(short, long)]
    /// The project containing the task
    project: Option<String>,

    #[arg(short, long)]
    /// The filter containing the task
    filter: Option<String>,
}

#[derive(Parser, Debug, Clone)]
pub struct Next {
    #[arg(short, long)]
    /// The project containing the task
    project: Option<String>,

    #[arg(short, long)]
    /// The filter containing the task
    filter: Option<String>,
}

#[derive(Parser, Debug, Clone)]
pub struct Complete {}

#[derive(Parser, Debug, Clone)]
pub struct Comment {
    #[arg(short, long)]
    /// Content for comment
    content: Option<String>,
}
pub async fn quick_add(config: &Config, args: &QuickAdd) -> Result<String, Error> {
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
    todoist::quick_create_task(config, &content, reminder).await?;
    Ok(format::green_string("✓"))
}

/// User does not want to use sections
fn is_no_sections(args: &Create, config: &Config) -> bool {
    args.no_section || config.no_sections.unwrap_or_default()
}

pub async fn create(config: Config, args: &Create) -> Result<String, Error> {
    if no_flags_used(args) {
        let options = tasks::create_task_attributes();
        let selections = input::multi_select(input::ATTRIBUTES, options, config.mock_select)?;

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
                config.mock_select,
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
            input::multi_select(input::LABELS, all_labels, config.mock_select)?
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
        )
        .await?;
    } else {
        let Create {
            project,
            due,
            description,
            content,
            priority,
            label: labels,
            no_section: _no_section,
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
        )
        .await?;
    }
    Ok(format::green_string("✓"))
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
    } = args;

    project.is_none()
        && due.is_none()
        && description.is_empty()
        && content.is_none()
        && priority.is_none()
        && label.is_empty()
}

pub async fn edit(config: Config, args: &Edit) -> Result<String, Error> {
    let Edit { project, filter } = args;
    match super::fetch_project_or_filter(project.as_deref(), filter.as_deref(), &config).await? {
        Flag::Project(project) => projects::edit_task(&config, &project).await,
        Flag::Filter(filter) => filters::edit_task(&config, filter).await,
    }
}
pub async fn next(config: Config, args: &Next) -> Result<String, Error> {
    let Next { project, filter } = args;
    match super::fetch_project_or_filter(project.as_deref(), filter.as_deref(), &config).await? {
        Flag::Project(project) => projects::next_task(config, &project).await,
        Flag::Filter(filter) => filters::next_task(&config, &filter).await,
    }
}

pub async fn complete(config: Config, _args: &Complete) -> Result<String, Error> {
    match config.next_task() {
        Some(task) => {
            todoist::complete_task(&config, &task.id, true).await?;

            Ok(format::green_string("Task completed successfully"))
        }
        None => Err(Error::new(
            "task_complete",
            "There is nothing to complete. A task must first be marked as 'next'.",
        )),
    }
}

pub async fn comment(config: Config, args: &Comment) -> Result<String, Error> {
    let Comment { content } = args;
    match config.next_task() {
        Some(task) => {
            let content = super::fetch_string(content.as_deref(), &config, input::CONTENT)?;
            todoist::create_comment(&config, &task.id, &content, true).await?;
            Ok(format::green_string("Comment created successfully"))
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

    fn create_args() -> Create {
        Create {
            project: None,
            due: None,
            description: String::new(),
            content: None,
            no_section: false,
            priority: None,
            label: Vec::new(),
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
    fn is_no_sections_respects_argument_flag() {
        let mut args = create_args();
        args.no_section = true;

        let config = Config::default();
        assert!(is_no_sections(&args, &config));
    }

    #[test]
    fn is_no_sections_respects_config_setting() {
        let args = create_args();
        let mut config = Config::default();
        config.no_sections = Some(true);

        assert!(is_no_sections(&args, &config));
    }
}
