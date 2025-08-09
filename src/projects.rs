use futures::future;
use pad::PadStr;
use std::fmt::Display;
use tokio::task::JoinHandle;

use crate::config::Config;
use crate::errors::Error;
use crate::sections::Section;
use crate::tasks::{FormatType, Task};
use crate::{SortOrder, color, input, sections, tasks, todoist};
use serde::{Deserialize, Serialize};

const PAD_WIDTH: usize = 30;
const PROJECT_URL: &str = "https://app.todoist.com/app/project";

/// Projects are split into sections
// This struct is from the v2 REST API and is deprecated
#[derive(PartialEq, Eq, Serialize, Deserialize, Clone, Debug)]
pub struct LegacyProject {
    pub id: String,
    pub name: String,
    pub color: String,
    pub comment_count: u8,
    pub order: u8,
    pub is_shared: bool,
    pub is_favorite: bool,
    pub is_inbox_project: bool,
    pub is_team_inbox: bool,
    pub view_style: String,
    pub url: String,
    pub parent_id: Option<String>,
}
// Projects are split into sections
#[derive(PartialEq, Eq, Serialize, Deserialize, Clone, Debug)]
pub struct Project {
    pub id: String,
    pub can_assign_tasks: bool,
    pub child_order: u32,
    pub color: String,
    pub created_at: Option<String>,
    pub is_archived: bool,
    pub is_deleted: bool,
    pub is_favorite: bool,
    pub is_frozen: bool,
    pub name: String,
    pub updated_at: Option<String>,
    pub view_style: String,
    pub default_order: u32,
    pub description: String,
    pub parent_id: Option<String>,
    pub inbox_project: Option<bool>,
    pub is_collapsed: bool,
    pub is_shared: bool,
}

#[derive(PartialEq, Eq, Serialize, Deserialize, Clone, Debug)]
pub struct ProjectResponse {
    pub results: Vec<Project>,
    pub next_cursor: Option<String>,
}

pub enum TaskFilter {
    /// Does not have a date or datetime on it
    Unscheduled,
    /// Date or datetime is before today
    Overdue,
    /// Is a repeating task
    Recurring,
}

impl Display for LegacyProject {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}\n{}", self.name, self.url)
    }
}
impl Display for Project {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}\n{}/{}", self.name, PROJECT_URL, self.id)
    }
}
pub fn json_to_project(json: String) -> Result<Project, Error> {
    let project: Project = serde_json::from_str(&json)?;
    Ok(project)
}
pub fn json_to_projects_response(json: String) -> Result<ProjectResponse, Error> {
    let response: ProjectResponse = serde_json::from_str(&json)?;
    Ok(response)
}

pub async fn create(
    config: &mut Config,
    name: String,
    description: String,
    is_favorite: bool,
) -> Result<String, Error> {
    let project =
        todoist::create_project(config, name.clone(), description, is_favorite, true).await?;
    add(config, &project).await?;
    Ok(format!("Created project {name} and added to config"))
}
/// List the projects in config with task counts
pub async fn list(config: &mut Config) -> Result<String, Error> {
    config.reload_projects().await?;

    let mut project_handles = Vec::new();

    for project in config.projects().await? {
        let config = config.clone();
        let handle = tokio::spawn(async move { project_name_with_count(&config, &project).await });

        project_handles.push(handle);
    }

    let mut projects: Vec<String> = future::join_all(project_handles)
        .await
        .into_iter()
        .map(|p| p.unwrap_or_default())
        .collect();
    if projects.is_empty() {
        return Ok("No projects found".into());
    }
    projects.sort();
    let mut buffer = String::new();
    buffer.push_str(&color::green_string("Projects").pad_to_width(PAD_WIDTH + 5));
    buffer.push_str(&color::green_string("# Tasks"));

    for key in projects {
        buffer.push_str("\n - ");
        buffer.push_str(&key);
    }
    Ok(buffer)
}

/// Formats a string with project name and the count that is a standard length
async fn project_name_with_count(config: &Config, project: &Project) -> String {
    let count = match count_processable_tasks(config, project).await {
        Ok(num) => format!("{num}"),
        Err(_) => String::new(),
    };

    format!("{}{}", project.name.pad_to_width(PAD_WIDTH), count)
}

/// Gets the number of tasks for a project that are not in the future
async fn count_processable_tasks(config: &Config, project: &Project) -> Result<u8, Error> {
    let all_tasks = todoist::all_tasks_by_project(config, project, None).await?;
    let count = tasks::filter_not_in_future(all_tasks, config)?.len();

    Ok(count as u8)
}

/// Add a project to the projects HashMap in Config
pub async fn add(config: &mut Config, project: &Project) -> Result<String, Error> {
    config.add_project(project.clone());
    config.save().await
}

/// Remove a project from the projects HashMap in Config
pub async fn remove(config: &mut Config, project: &Project) -> Result<String, Error> {
    config.remove_project(project);
    config.save().await
}

/// Remove a project from the projects HashMap in Config
pub async fn delete(config: &mut Config, project: &Project) -> Result<String, Error> {
    todoist::delete_project(config, project, true).await?;
    config.remove_project(project);
    config.save().await
}

/// Rename a project in config
pub async fn rename(config: Config, project: &Project) -> Result<String, Error> {
    let new_name = input::string_with_default(input::NAME, &project.name)?;

    let mut config = config;

    let new_project = Project {
        name: new_name,
        ..project.clone()
    };
    remove(&mut config, project).await?;
    add(&mut config, &new_project).await
}

/// Get the next task by priority and save its id to config
pub async fn next_task(config: Config, project: &Project) -> Result<String, Error> {
    match fetch_next_task(&config, project).await {
        Ok(Some((task, remaining))) => {
            let comments = todoist::all_comments(&config, &task, None).await?;
            config.set_next_task(task.clone()).save().await?;
            let task_string = task
                .fmt(comments, &config, FormatType::Single, false)
                .await?;
            Ok(format!("{task_string}\n{remaining} task(s) remaining"))
        }
        Ok(None) => Ok(color::green_string("No tasks on list")),
        Err(e) => Err(e),
    }
}

async fn fetch_next_task(
    config: &Config,
    project: &Project,
) -> Result<Option<(Task, usize)>, Error> {
    let tasks = todoist::all_tasks_by_project(config, project, None).await?;
    let filtered_tasks = tasks::filter_not_in_future(tasks, config)?;
    let tasks = tasks::sort_by_value(filtered_tasks, config);

    Ok(tasks.first().map(|task| (task.to_owned(), tasks.len())))
}

/// Removes all projects from config that don't exist in Todoist
pub async fn remove_auto(config: &mut Config) -> Result<String, Error> {
    let projects = todoist::all_projects(config, None).await?;
    let missing_projects = filter_missing_projects(config, projects).await?;

    if missing_projects.is_empty() {
        return Ok(color::green_string("No projects to auto remove"));
    }

    for project in &missing_projects {
        config.remove_project(project);
    }
    config.save().await?;
    let project_names = missing_projects
        .iter()
        .map(|p| p.name.clone())
        .collect::<Vec<String>>()
        .join(", ");
    let message = format!("Auto removed: '{project_names}'");
    Ok(color::green_string(&message))
}

/// Removes all projects from config
pub async fn remove_all(config: &mut Config) -> Result<String, Error> {
    let options = vec!["Cancel", "Confirm"];
    let selection = input::select(
        "Confirm removing all projects from config",
        options,
        config.mock_select,
    )?;

    if selection == "Cancel" {
        return Ok("Cancelled".into());
    }

    let projects = config.projects().await?;
    if projects.is_empty() {
        return Ok(color::green_string("No projects to remove"));
    }

    for project in &projects {
        config.remove_project(project);
    }
    config.save().await?;
    let message = "Removed all projects from config";
    Ok(color::green_string(message))
}

/// Returns the projects that are not already in config
async fn filter_missing_projects(
    config: &mut Config,
    projects: Vec<Project>,
) -> Result<Vec<Project>, Error> {
    let project_ids: Vec<String> = projects.into_iter().map(|v| v.id).collect();
    let config = config
        .projects()
        .await?
        .into_iter()
        .filter(|p| !project_ids.contains(&p.id))
        .collect();

    Ok(config)
}

/// Fetch projects and prompt to add them to config one by one
pub async fn import(config: &mut Config, auto: &bool) -> Result<String, Error> {
    let projects = todoist::all_projects(config, None).await?;
    let new_projects = filter_new_projects(config, projects).await?;
    for project in new_projects {
        maybe_add_project(config, project, auto).await?;
    }
    Ok(color::green_string("No more projects"))
}

/// Returns the projects that are not already in config
async fn filter_new_projects(
    config: &mut Config,
    projects: Vec<Project>,
) -> Result<Vec<Project>, Error> {
    let project_ids: Vec<String> = config
        .projects()
        .await?
        .iter()
        .map(|v| v.id.clone())
        .collect();
    let new_projects: Vec<Project> = projects
        .into_iter()
        .filter(|p| !project_ids.contains(&p.id))
        .collect();

    Ok(new_projects)
}

/// Prompt the user if they want to add project to config and maybe add
async fn maybe_add_project(
    config: &mut Config,
    project: Project,
    auto: &bool,
) -> Result<String, Error> {
    if *auto {
        println!("Adding {project}");
        return add(config, &project).await;
    }

    let options = vec!["add", "skip"];
    println!("{project}");
    match input::select("Select an option", options.clone(), config.mock_select) {
        Ok(string) => {
            if string == "add" {
                add(config, &project).await
            } else if string == "skip" {
                Ok("Skipped".into())
            } else {
                Err(Error::new("add_project", "Invalid option"))
            }
        }
        Err(e) => Err(e)?,
    }
}

pub async fn edit_task(config: &Config, project: &Project) -> Result<String, Error> {
    let project_tasks = todoist::all_tasks_by_project(config, project, None).await?;

    let task = input::select(
        "Choose a task of the project:",
        project_tasks,
        config.mock_select,
    )?;

    let options = tasks::edit_task_attributes();

    let selections = input::multi_select("Choose attributes to edit", options, config.mock_select)?;

    if selections.is_empty() {
        return Err(Error {
            message: "Nothing selected".to_string(),
            source: "edit_task".to_string(),
        });
    }

    let mut handles = Vec::new();
    for attribute in selections {
        // Stops the inputs from rolling over each other in terminal
        println!();
        if let Some(handle) = tasks::update_task(config, &task, &attribute).await? {
            handles.push(handle);
        }
    }

    future::join_all(handles).await;
    Ok("Finished editing task".into())
}

/// Empty a project by sending tasks to other projects one at a time
pub async fn empty(config: &mut Config, project: &Project) -> Result<String, Error> {
    let tasks = todoist::all_tasks_by_project(config, project, None).await?;

    if tasks.is_empty() {
        Ok(color::green_string(&format!(
            "No tasks to empty from '{}'",
            project.name
        )))
    } else {
        let sections = sections::all_sections(config).await?;

        let tasks = tasks
            .into_iter()
            .filter(|task| task.parent_id.is_none())
            .collect::<Vec<Task>>();

        let mut handles = Vec::new();
        for task in tasks.iter() {
            match move_task_to_project(config, task.to_owned(), &sections).await {
                Ok(handle) => handles.push(handle),
                Err(e) => return Err(e),
            };
        }
        future::join_all(handles).await;
        Ok(color::green_string(&format!(
            "Successfully emptied '{}'",
            project.name
        )))
    }
}

/// Put dates on all tasks without dates
pub async fn schedule(
    config: &Config,
    project: &Project,
    filter: TaskFilter,
    skip_recurring: bool,
    sort: &SortOrder,
) -> Result<String, Error> {
    let tasks = todoist::all_tasks_by_project(config, project, None).await?;
    let tasks = tasks::sort(tasks, config, sort);

    let filtered_tasks: Vec<Task> = if skip_recurring {
        tasks
            .into_iter()
            .filter(|task| {
                task.filter(config, &filter) && !task.filter(config, &TaskFilter::Recurring)
            })
            .collect::<Vec<Task>>()
    } else {
        tasks
            .into_iter()
            .filter(|task| task.filter(config, &filter))
            .collect::<Vec<Task>>()
    };

    if filtered_tasks.is_empty() {
        Ok(color::green_string(&format!(
            "No tasks to schedule in '{}'",
            project.name
        )))
    } else {
        let mut handles = Vec::new();
        for task in filtered_tasks.iter() {
            if let Some(handle) = tasks::spawn_schedule_task(config.clone(), task.clone()).await? {
                handles.push(handle);
            }
        }

        future::join_all(handles).await;
        Ok(color::green_string(&format!(
            "Successfully scheduled tasks in '{}'",
            project.name
        )))
    }
}
pub async fn deadline(
    config: &Config,
    project: &Project,
    sort: &SortOrder,
) -> Result<String, Error> {
    let tasks = todoist::all_tasks_by_project(config, project, None).await?;
    let tasks = tasks::sort(tasks, config, sort);

    let filtered_tasks: Vec<Task> = tasks
        .into_iter()
        .filter(|task| !task.filter(config, &TaskFilter::Recurring) && task.deadline.is_none())
        .collect::<Vec<Task>>();

    if filtered_tasks.is_empty() {
        Ok(color::green_string(&format!(
            "No tasks to deadline in '{}'",
            project.name
        )))
    } else {
        let mut handles = Vec::new();
        for task in filtered_tasks.iter() {
            if let Some(handle) = tasks::spawn_deadline_task(config.clone(), task.clone()).await? {
                handles.push(handle);
            }
        }

        future::join_all(handles).await;
        Ok(color::green_string(&format!(
            "Successfully deadlined tasks in '{}'",
            project.name
        )))
    }
}

pub async fn move_task_to_project(
    config: &mut Config,
    task: Task,
    sections: &[Section],
) -> Result<JoinHandle<()>, Error> {
    let comments = Vec::new();
    let text = task
        .fmt(comments, config, FormatType::Single, false)
        .await?;
    println!("{text}");

    let options = ["Pick project", "Complete", "Skip", "Delete"]
        .iter()
        .map(|o| o.to_string())
        .collect::<Vec<String>>();
    let selection = input::select("Choose", options, config.mock_select)?;

    match selection.as_str() {
        "Complete" => Ok(tasks::spawn_complete_task(config.clone(), task)),

        "Delete" => Ok(tasks::spawn_delete_task(config.clone(), task)),
        "Skip" => Ok(tokio::spawn(async move {})),
        _ => {
            let projects = config.projects().await?;
            let project = input::select("Select project", projects, config.mock_select)?;

            let sections: Vec<Section> = sections
                .iter()
                .filter(|s| s.project_id == project.id)
                .cloned()
                .collect();

            let section_names: Vec<String> = sections.clone().into_iter().map(|x| x.name).collect();
            if section_names.is_empty() || config.no_sections.unwrap_or_default() {
                let config = config.clone();
                Ok(tokio::spawn(async move {
                    if let Err(e) =
                        todoist::move_task_to_project(&config, &task, &project, false).await
                    {
                        config
                            .tx()
                            .send(e)
                            .expect("expected value or result, got None or Err");
                    }
                }))
            } else {
                let section_name =
                    input::select("Select section", section_names, config.mock_select)?;
                let section = sections
                    .iter()
                    .find(|x| x.name == section_name.as_str())
                    .expect("Section does not exist")
                    .clone();
                let config = config.clone();
                Ok(tokio::spawn(async move {
                    if let Err(e) =
                        todoist::move_task_to_section(&config, &task, &section, false).await
                    {
                        config
                            .tx()
                            .send(e)
                            .expect("expected value or result, got None or Err");
                    }
                }))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test;
    use crate::test::responses::ResponseFromFile;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn should_add_and_remove_projects() {
        let config = test::fixtures::config()
            .await
            .create()
            .await
            .expect("expected value or result, got None or Err");

        let mut config = config;
        let binding = config
            .projects()
            .await
            .expect("expected value or result, got None or Err");
        let project = binding
            .first()
            .expect("expected value or result, got None or Err");

        let result = remove(&mut config, project).await;
        assert_eq!(Ok("✓".to_string()), result);
        let result = add(&mut config, project).await;
        assert_eq!(Ok("✓".to_string()), result);
    }
    #[tokio::test]
    async fn test_list() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/api/v1/projects?limit=200")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::Projects.read().await)
            .create_async()
            .await;

        let mut config = test::fixtures::config()
            .await
            .create()
            .await
            .expect("expected value or result, got None or Err")
            .with_mock_url(server.url())
            .with_projects(vec![test::fixtures::project()]);

        config
            .save()
            .await
            .expect("expected value or result, got None or Err");

        let str = "Projects                           # Tasks\n - Doomsday                      ";

        assert_eq!(list(&mut config).await, Ok(String::from(str)));
        mock.expect(3);
    }

    #[tokio::test]
    async fn test_get_next_task() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/api/v1/tasks/?project_id=123&limit=200")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::TodayTasks.read().await)
            .create_async()
            .await;

        let mock2 = server
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

        let config_dir = dirs::config_dir().expect("Could not find config directory");

        let config_with_timezone = config
            .with_timezone("America/Vancouver")
            .with_path(config_dir.join("test3"))
            .with_mock_url(server.url());
        let binding = config_with_timezone
            .projects()
            .await
            .expect("expected value or result, got None or Err");
        let project = binding
            .first()
            .expect("expected value or result, got None or Err");

        config_with_timezone
            .clone()
            .create()
            .await
            .expect("expected value or result, got None or Err");

        let response = next_task(config_with_timezone, project)
            .await
            .expect("expected value or result, got None or Err");

        assert!(response.contains("TEST"));
        assert!(response.contains("1 task(s) remaining"));
        mock.assert();
        mock2.assert();
    }

    #[tokio::test]
    async fn test_import() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/api/v1/projects?limit=200")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::NewProjects.read().await)
            .create_async()
            .await;

        let mut config = test::fixtures::config()
            .await
            .with_mock_url(server.url())
            .mock_select(0)
            .create()
            .await
            .expect("expected value or result, got None or Err");

        assert_eq!(
            import(&mut config, &false).await,
            Ok("No more projects".to_string())
        );
        mock.assert_async().await;

        let config = config
            .reload()
            .await
            .expect("expected value or result, got None or Err");
        let config_keys: Vec<String> = config
            .projects()
            .await
            .expect("expected value or result, got None or Err")
            .iter()
            .map(|p| p.name.to_owned())
            .collect();
        assert!(config_keys.contains(&"Doomsday".to_string()))
    }

    #[tokio::test]
    async fn test_remove_auto() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/api/v1/projects?limit=200")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::NewProjects.read().await)
            .create_async()
            .await;

        let mut config = test::fixtures::config()
            .await
            .with_mock_url(server.url())
            .create()
            .await
            .expect("expected value or result, got None or Err");

        let result = remove_auto(&mut config);
        let expected: Result<String, Error> = Ok(String::from("Auto removed: 'myproject'"));
        assert_eq!(result.await, expected);
        mock.assert_async().await;
        let projects = config
            .projects()
            .await
            .expect("expected value or result, got None or Err");
        assert_eq!(projects.is_empty(), true);
    }

    #[tokio::test]
    async fn test_remove_all() {
        let mut config = test::fixtures::config()
            .await
            .mock_select(1)
            .create()
            .await
            .expect("expected value or result, got None or Err");

        let result = remove_all(&mut config).await;
        let expected: Result<String, Error> = Ok(String::from("Removed all projects from config"));
        assert_eq!(result, expected);

        let projects = config
            .projects()
            .await
            .expect("expected value or result, got None or Err");
        assert_eq!(projects.is_empty(), true);
    }

    #[tokio::test]
    async fn test_empty() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/api/v1/tasks/?project_id=123&limit=200")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::TodayTasks.read().await)
            .create_async()
            .await;

        let mock2 = server
            .mock("POST", "/api/v1/tasks/6Xqhv4cwxgjwG9w8/move")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::Task.read().await)
            .create_async()
            .await;

        let mock3 = server
            .mock("GET", "/api/v1/sections?project_id=123&limit=200")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::Sections.read().await)
            .create_async()
            .await;

        let mock4 = server
            .mock("GET", "/api/v1/id_mappings/projects/123")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::Ids.read().await)
            .create_async()
            .await;
        let mock5 = server
            .mock(
                "GET",
                "/api/v1/comments/?task_id=6Xqhv4cwxgjwG9w8&limit=200",
            )
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::CommentsAllTypes.read().await)
            .create_async()
            .await;

        let mut config = test::fixtures::config()
            .await
            .with_mock_url(server.url())
            .with_mock_string("newtext")
            .mock_select(0);

        let binding = config
            .projects()
            .await
            .expect("expected value or result, got None or Err");
        let project = binding
            .first()
            .expect("expected value or result, got None or Err");
        let result = empty(&mut config, project).await;
        assert_eq!(result, Ok(String::from("Successfully emptied 'myproject'")));
        mock.expect(2);
        mock2.assert();
        mock3.assert();
        mock4.expect(2);
        mock5.expect(2);
    }

    #[tokio::test]
    async fn test_move_task_to_project() {
        let mut config = test::fixtures::config().await.mock_select(2);
        let task = test::fixtures::today_task().await;
        let sections: Vec<Section> = Vec::new();

        move_task_to_project(&mut config, task, &sections)
            .await
            .expect("expected value or result, got None or Err")
            .await
            .expect("expected value or result, got None or Err");
    }

    #[tokio::test]
    async fn test_rename_task() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/api/v1/tasks/?project_id=123&limit=200")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::TodayTasks.read().await)
            .create_async()
            .await;

        let config = test::fixtures::config()
            .await
            .with_mock_url(server.url())
            .mock_select(0);
        let binding = config
            .projects()
            .await
            .expect("expected value or result, got None or Err");
        let project = binding
            .first()
            .expect("expected value or result, got None or Err");

        let result = edit_task(&config, project);
        assert_eq!(result.await, Ok("Finished editing task".to_string()));
        mock.assert();
    }
    #[tokio::test]
    async fn test_project_delete() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("DELETE", "/api/v1/projects/123")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::Project.read().await)
            .create_async()
            .await;

        let mut config = test::fixtures::config()
            .await
            .with_mock_url(server.url())
            .mock_select(0)
            .create()
            .await
            .expect("expected value or result, got None or Err");
        let binding = config
            .projects()
            .await
            .expect("expected value or result, got None or Err");
        let project = binding
            .first()
            .expect("expected value or result, got None or Err");

        let result = delete(&mut config, project).await;
        assert_eq!(result, Ok("✓".to_string()));
        mock.assert_async().await;
    }
    #[tokio::test]
    async fn test_schedule() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/api/v1/tasks/?project_id=123&limit=200")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::UnscheduledTasks.read().await)
            .create_async()
            .await;

        let mock2 = server
            .mock("POST", "/rest/v2/tasks/999999")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::TodayTask.read().await)
            .create_async()
            .await;

        let mock3 = server
            .mock("GET", "/api/v1/id_mappings/projects/123")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::Ids.read().await)
            .create_async()
            .await;
        let mock4 = server
            .mock("GET", "/api/v1/comments/?task_id=999999&limit=200")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::CommentsAllTypes.read().await)
            .create_async()
            .await;
        let config = test::fixtures::config()
            .await
            .with_mock_url(server.url())
            .mock_select(1)
            .with_mock_string("tod");

        let binding = config
            .projects()
            .await
            .expect("expected value or result, got None or Err");
        let project = binding
            .first()
            .expect("expected value or result, got None or Err");
        let sort = &SortOrder::Value;
        let result = schedule(&config, project, TaskFilter::Unscheduled, false, sort);
        assert_eq!(
            result.await,
            Ok("Successfully scheduled tasks in 'myproject'".to_string())
        );

        let config = config.mock_select(2);

        let binding = config
            .projects()
            .await
            .expect("expected value or result, got None or Err");
        let project = binding
            .first()
            .expect("expected value or result, got None or Err");
        let result = schedule(&config, project, TaskFilter::Overdue, false, sort);
        assert_eq!(
            result.await,
            Ok("No tasks to schedule in 'myproject'".to_string())
        );

        let config = config.mock_select(3);

        let binding = config
            .projects()
            .await
            .expect("expected value or result, got None or Err");
        let project = binding
            .first()
            .expect("expected value or result, got None or Err");
        let result = schedule(&config, project, TaskFilter::Unscheduled, false, sort);
        assert_eq!(
            result.await,
            Ok("Successfully scheduled tasks in 'myproject'".to_string())
        );

        let result = schedule(&config, project, TaskFilter::Unscheduled, true, sort);
        assert_eq!(
            result.await,
            Ok("Successfully scheduled tasks in 'myproject'".to_string())
        );
        mock.expect(2);
        mock2.expect(2);
        mock3.expect(4);
        mock4.expect(4);
    }

    #[tokio::test]
    async fn test_deadline() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/api/v1/tasks/?project_id=123&limit=200")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::UnscheduledTasks.read().await)
            .create_async()
            .await;

        let mock2 = server
            .mock("POST", "/rest/v2/tasks/999999")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::TodayTask.read().await)
            .create_async()
            .await;

        let mock3 = server
            .mock("GET", "/api/v1/id_mappings/projects/123")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::Ids.read().await)
            .create_async()
            .await;
        let config = test::fixtures::config()
            .await
            .with_mock_url(server.url())
            .mock_select(1)
            .with_mock_string("tod");

        let mock4 = server
            .mock("GET", "/api/v1/comments/?task_id=999999&limit=200")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::CommentsAllTypes.read().await)
            .create_async()
            .await;
        let binding = config
            .projects()
            .await
            .expect("expected value or result, got None or Err");
        let project = binding
            .first()
            .expect("expected value or result, got None or Err");
        let sort = &SortOrder::Value;
        let result = deadline(&config, project, sort);
        assert_eq!(
            result.await,
            Ok("Successfully deadlined tasks in 'myproject'".to_string())
        );

        let config = config.mock_select(3);

        let binding = config
            .projects()
            .await
            .expect("expected value or result, got None or Err");
        let project = binding
            .first()
            .expect("expected value or result, got None or Err");
        let result = deadline(&config, project, sort);
        assert_eq!(
            result.await,
            Ok("Successfully deadlined tasks in 'myproject'".to_string())
        );

        let result = deadline(&config, project, sort);
        assert_eq!(
            result.await,
            Ok("Successfully deadlined tasks in 'myproject'".to_string())
        );
        mock.expect(2);
        mock2.expect(2);
        mock3.expect(4);
        mock4.expect(4);
    }
}
