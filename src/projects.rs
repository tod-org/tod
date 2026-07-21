use futures::{StreamExt, TryStreamExt, future, stream};
use pad::PadStr;
use std::collections::HashSet;
use std::fmt::Display;
use tokio::task::JoinHandle;

use crate::config::Config;
use crate::errors::Error;
use crate::sections::Section;
use crate::tasks::{FormatType, Task};
use crate::{SortOrder, format, input, sections, tasks, todoist};
use serde::{Deserialize, Serialize};

const PAD_WIDTH: usize = 30;
const PROJECT_URL: &str = "https://app.todoist.com/app/project";

// Projects are split into sections
#[allow(clippy::struct_excessive_bools)]
#[derive(PartialEq, Eq, Serialize, Deserialize, Clone, Debug)]
pub struct Project {
    pub id: String,
    pub can_assign_tasks: bool,
    pub child_order: i32,
    pub color: String,
    pub created_at: Option<String>,
    pub is_archived: bool,
    pub is_deleted: bool,
    pub is_favorite: bool,
    pub is_frozen: bool,
    pub name: String,
    pub updated_at: Option<String>,
    pub view_style: String,
    pub default_order: i32,
    pub description: String,
    pub parent_id: Option<String>,
    #[allow(clippy::struct_field_names)]
    pub inbox_project: Option<bool>,
    pub is_collapsed: bool,
    pub is_shared: bool,
}

#[derive(PartialEq, Eq, Serialize, Deserialize, Clone, Debug)]
pub struct ProjectResponse {
    pub results: Vec<Project>,
    pub next_cursor: Option<String>,
}

impl ProjectResponse {
    pub fn from_json(json: &str) -> Result<ProjectResponse, Error> {
        let response: ProjectResponse = serde_json::from_str(json)?;
        Ok(response)
    }
}

pub enum TaskFilter {
    /// Does not have a date or datetime on it
    Unscheduled,
    /// Date or datetime is before today
    Overdue,
    /// Is a repeating task
    Recurring,
}

impl Display for Project {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}\n{}/{}", self.name, PROJECT_URL, self.id)
    }
}

impl Project {
    pub fn from_json(json: &str) -> Result<Project, Error> {
        let project: Project = serde_json::from_str(json)?;
        Ok(project)
    }
}
pub async fn create(
    config: &mut Config,
    name: String,
    description: &str,
    is_favorite: bool,
) -> Result<String, Error> {
    let project = todoist::create_project(config, &name, description, is_favorite, true).await?;
    add(config, &project).await?;
    Ok(format!("Created project {name} and added to config"))
}
/// List the projects in config with task counts
pub async fn list(config: &mut Config) -> Result<String, Error> {
    config.reload_projects().await?;

    let project_handles = config
        .projects()
        .await?
        .into_iter()
        .map(|project| {
            let config = config.clone();
            tokio::spawn(async move { project_name_with_count(&config, &project).await })
        })
        .collect::<Vec<_>>();

    let mut projects: Vec<String> = future::join_all(project_handles)
        .await
        .into_iter()
        .map(std::result::Result::unwrap_or_default)
        .collect();
    if projects.is_empty() {
        return Ok("No projects found".into());
    }
    projects.sort();
    let mut buffer = String::new();
    buffer.push_str(&format::green_string("Projects").pad_to_width(PAD_WIDTH + 5));
    buffer.push_str(&format::green_string("# Tasks"));

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
    let count = tasks::filter_not_in_future(all_tasks, config).len();

    Ok(u8::try_from(count)?)
}

/// Add a project to the projects `HashMap` in Config
pub async fn add(config: &mut Config, project: &Project) -> Result<String, Error> {
    config.add_project(project.clone());
    config.save().await
}

/// Remove a project from the projects `HashMap` in Config
pub async fn remove(config: &mut Config, project: &Project) -> Result<String, Error> {
    config.remove_project(project);
    config.save().await
}

/// Remove a project from the projects `HashMap` in Config
pub async fn delete(config: &mut Config, project: &Project) -> Result<String, Error> {
    todoist::delete_project(config, project, true).await?;
    config.remove_project(project);
    config.save().await
}

/// Rename a project in config
pub async fn rename(
    config: &mut Config,
    project: &Project,
    new_name: Option<&str>,
) -> Result<String, Error> {
    let new_name = match new_name {
        Some(name) => name.to_string(),
        None => input::string_with_default(input::NAME, &project.name)?,
    };

    let new_project = Project {
        name: new_name,
        ..project.clone()
    };
    remove(config, project).await?;
    add(config, &new_project).await
}

/// Get the next task by priority and save its id to config
pub async fn next_task(config: Config, project: &Project) -> Result<String, Error> {
    match fetch_next_task(&config, project).await {
        Ok(Some((task, remaining))) => {
            let comments = todoist::all_comments(&config, &task.id, None).await?;
            let task_string = task
                .fmt(comments, &config, FormatType::Single, false)
                .await?;
            config.set_next_task(task).save().await?;
            Ok(format!("{task_string}\n{remaining} task(s) remaining"))
        }
        Ok(None) => Ok(format::green_string("No tasks on list")),
        Err(e) => Err(e),
    }
}

async fn fetch_next_task(
    config: &Config,
    project: &Project,
) -> Result<Option<(Task, usize)>, Error> {
    let tasks = todoist::all_tasks_by_project(config, project, None).await?;
    let filtered_tasks = tasks::filter_not_in_future(tasks, config);
    let tasks = tasks::sort_by_value(filtered_tasks, config);

    Ok(tasks.first().map(|task| (task.to_owned(), tasks.len())))
}

/// Removes all projects from config that don't exist in Todoist
pub async fn remove_auto(config: &mut Config) -> Result<String, Error> {
    let projects = todoist::all_projects(config, None).await?;
    let missing_projects = filter_missing_projects(config, projects).await?;

    if missing_projects.is_empty() {
        return Ok(format::green_string("No projects to auto remove"));
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
    Ok(format::green_string(&message))
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
        return Ok(format::green_string("No projects to remove"));
    }

    for project in &projects {
        config.remove_project(project);
    }
    config.save().await?;
    let message = "Removed all projects from config";
    Ok(format::green_string(message))
}

/// Returns the projects that are not already in config
async fn filter_missing_projects(
    config: &mut Config,
    projects: Vec<Project>,
) -> Result<Vec<Project>, Error> {
    let configured_projects = config.projects().await?;
    let project_ids = projects
        .iter()
        .map(|v| v.id.as_str())
        .collect::<HashSet<_>>();
    let config = configured_projects
        .into_iter()
        .filter(|p| !project_ids.contains(p.id.as_str()))
        .collect();

    Ok(config)
}

/// Fetch projects and prompt to add them to config one by one
pub async fn import(
    config: &mut Config,
    auto: &bool,
    project: Option<&str>,
    id: Option<&str>,
) -> Result<String, Error> {
    if project.is_some() && id.is_some() {
        return Err(Error::new(
            "project_import",
            "Cannot provide both project and id",
        ));
    }

    let projects = todoist::all_projects(config, None).await?;

    if let Some(project_name) = project {
        let target = projects
            .into_iter()
            .find(|p| p.name == project_name)
            .ok_or_else(|| {
                Error::new(
                    "project_import",
                    &format!("Could not find Todoist project named '{project_name}'"),
                )
            })?;
        let new_projects = filter_new_projects(config, vec![target]).await?;

        return if let Some(project) = new_projects.into_iter().next() {
            add_imported_project(config, &project).await
        } else {
            Ok(format::green_string("Project already in config"))
        };
    }

    if let Some(project_id) = id {
        let target = projects
            .into_iter()
            .find(|p| p.id == project_id)
            .ok_or_else(|| {
                Error::new(
                    "project_import",
                    &format!("Could not find Todoist project with id '{project_id}'"),
                )
            })?;
        let new_projects = filter_new_projects(config, vec![target]).await?;

        return if let Some(project) = new_projects.into_iter().next() {
            add_imported_project(config, &project).await
        } else {
            Ok(format::green_string("Project already in config"))
        };
    }

    let new_projects = filter_new_projects(config, projects).await?;
    for project in new_projects {
        maybe_add_project(config, project, auto).await?;
    }
    Ok(format::green_string("No more projects"))
}

/// Returns the projects that are not already in config
async fn filter_new_projects(
    config: &mut Config,
    projects: Vec<Project>,
) -> Result<Vec<Project>, Error> {
    let configured_projects = config.projects().await?;
    let project_ids = configured_projects
        .iter()
        .map(|v| v.id.as_str())
        .collect::<HashSet<_>>();
    let new_projects: Vec<Project> = projects
        .into_iter()
        .filter(|p| !project_ids.contains(p.id.as_str()))
        .collect();

    Ok(new_projects)
}

async fn add_imported_project(config: &mut Config, project: &Project) -> Result<String, Error> {
    add(config, project).await?;
    let added_project_name = config
        .projects()
        .await?
        .into_iter()
        .find(|p| p.id == project.id)
        .map(|p| p.name)
        .ok_or_else(|| {
            Error::new(
                "project_import",
                "Added project was not found in config after saving",
            )
        })?;

    Ok(format!(
        "{} Added project {added_project_name}",
        format::green_string("✓")
    ))
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
    match input::select("Select an option", options, config.mock_select) {
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
        Ok(format::green_string(&format!(
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
        for task in tasks {
            handles.push(move_task_to_project(config, task, &sections).await?);
        }
        future::join_all(handles).await;
        Ok(format::green_string(&format!(
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
    let tasks = tasks::sort(tasks, config, *sort);

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
        Ok(format::green_string(&format!(
            "No tasks to schedule in '{}'",
            project.name
        )))
    } else {
        let handles = stream::iter(filtered_tasks)
            .then(|task| tasks::spawn_schedule_task(config.clone(), task))
            .try_collect::<Vec<_>>()
            .await?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();

        future::join_all(handles).await;
        Ok(format::green_string(&format!(
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
    let tasks = tasks::sort(tasks, config, *sort);

    let filtered_tasks: Vec<Task> = tasks
        .into_iter()
        .filter(|task| !task.filter(config, &TaskFilter::Recurring) && task.deadline.is_none())
        .collect::<Vec<Task>>();

    if filtered_tasks.is_empty() {
        Ok(format::green_string(&format!(
            "No tasks to deadline in '{}'",
            project.name
        )))
    } else {
        let handles = stream::iter(filtered_tasks)
            .then(|task| tasks::spawn_deadline_task(config.clone(), task))
            .try_collect::<Vec<_>>()
            .await?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();

        future::join_all(handles).await;
        Ok(format::green_string(&format!(
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
        .map(std::string::ToString::to_string)
        .collect::<Vec<String>>();
    let selection = input::select("Choose", options, config.mock_select)?;

    match selection.as_str() {
        "Complete" => Ok(tasks::spawn_complete_task(config.clone(), task.id)),

        "Delete" => Ok(tasks::spawn_delete_task(config.clone(), task.id)),
        "Skip" => Ok(tokio::spawn(async move {})),
        _ => {
            let projects = config.projects().await?;
            let project = input::select("Select project", projects, config.mock_select)?;

            let sections: Vec<Section> = sections
                .iter()
                .filter(|s| s.project_id == project.id)
                .cloned()
                .collect();

            let section_names: Vec<String> = sections.iter().map(|x| x.name.clone()).collect();
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
        let mut config = test::fixtures::config()
            .await
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
            import(&mut config, &false, None, None).await,
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
            .map(|p| p.name.clone())
            .collect();
        assert!(config_keys.contains(&"Doomsday".to_string()));
    }

    #[tokio::test]
    async fn test_import_by_project_name() {
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

        assert_eq!(
            import(&mut config, &false, Some("Doomsday"), None).await,
            Ok("✓ Added project Doomsday".to_string())
        );
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_import_by_project_id() {
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

        assert_eq!(
            import(&mut config, &false, None, Some("890")).await,
            Ok("✓ Added project Doomsday".to_string())
        );
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_import_by_project_name_not_found() {
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

        let result = import(&mut config, &false, Some("does-not-exist"), None).await;
        assert_eq!(
            result,
            Err(Error::new(
                "project_import",
                "Could not find Todoist project named 'does-not-exist'"
            ))
        );
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_import_by_project_id_not_found() {
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

        let result = import(&mut config, &false, None, Some("999999")).await;
        assert_eq!(
            result,
            Err(Error::new(
                "project_import",
                "Could not find Todoist project with id '999999'"
            ))
        );
        mock.assert_async().await;
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
        mock4.expect(4);
    }

    #[tokio::test]
    async fn test_project_from_json_valid() {
        let json = ResponseFromFile::Project.read().await;
        let project = Project::from_json(&json).expect("should parse project JSON");
        assert_eq!(project.name, "Doomsday");
    }

    #[test]
    fn test_project_from_json_invalid() {
        let result = Project::from_json("not json");
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_project_response_from_json_valid() {
        let json = ResponseFromFile::Projects.read().await;
        let response = ProjectResponse::from_json(&json).expect("should parse projects response");
        assert!(!response.results.is_empty());
    }

    #[test]
    fn test_project_response_from_json_invalid() {
        let result = ProjectResponse::from_json("not json");
        assert!(result.is_err());
    }

    #[test]
    fn should_deserialize_project_with_negative_order() {
        let json = r#"{
            "id": "123",
            "can_assign_tasks": true,
            "child_order": -1,
            "color": "blue",
            "created_at": null,
            "is_archived": false,
            "is_deleted": false,
            "is_favorite": false,
            "is_frozen": false,
            "name": "Inbox",
            "updated_at": null,
            "view_style": "list",
            "default_order": -1,
            "description": "",
            "parent_id": null,
            "inbox_project": true,
            "is_collapsed": false,
            "is_shared": false
        }"#;

        let project =
            Project::from_json(json).expect("should deserialize project with negative order");

        assert_eq!(project.child_order, -1);
        assert_eq!(project.default_order, -1);
    }

    #[test]
    fn should_deserialize_project_with_negative_child_order() {
        let json = r#"{
            "id": "123",
            "can_assign_tasks": true,
            "child_order": -1,
            "color": "blue",
            "created_at": null,
            "is_archived": false,
            "is_deleted": false,
            "is_favorite": false,
            "is_frozen": false,
            "name": "Inbox",
            "updated_at": null,
            "view_style": "list",
            "default_order": 1,
            "description": "",
            "parent_id": null,
            "inbox_project": true,
            "is_collapsed": false,
            "is_shared": false
        }"#;

        let project =
            Project::from_json(json).expect("should deserialize project with negative child order");

        assert_eq!(project.child_order, -1);
        assert_eq!(project.default_order, 1);
    }

    #[test]
    fn test_project_display() {
        let project = test::fixtures::project();
        let displayed = project.to_string();
        assert!(displayed.contains("myproject"));
        assert!(displayed.contains("https://app.todoist.com/app/project"));
        assert!(displayed.contains(&project.id));
    }
}
