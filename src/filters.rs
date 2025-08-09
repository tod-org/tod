use futures::future;

use crate::{
    SortOrder, color,
    config::Config,
    errors::Error,
    input::{self},
    projects::TaskFilter,
    tasks::{self, FormatType, Task},
    todoist,
};

pub async fn edit_task(config: &Config, filter: String) -> Result<String, Error> {
    let tasks = todoist::all_tasks_by_filters(config, &filter)
        .await?
        .into_iter()
        .flat_map(|(_, tasks)| tasks.to_owned())
        .collect::<Vec<Task>>();

    let task = input::select(input::TASK, tasks, config.mock_select)?;

    let options = tasks::edit_task_attributes();

    let selections = input::multi_select(input::ATTRIBUTES, options, config.mock_select)?;

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

/// Get the next task by priority and save its id to config
pub async fn next_task(config: &Config, filter: &str) -> Result<String, Error> {
    match fetch_next_task(config, filter).await {
        Ok(Some((task, remaining))) => {
            let comments = todoist::all_comments(config, &task, None).await?;
            config.set_next_task(task.clone()).save().await?;
            let task_string = task.fmt(comments, config, FormatType::Single, true).await?;
            Ok(format!("{task_string}\n{remaining} task(s) remaining"))
        }
        Ok(None) => Ok(color::green_string("No tasks on list")),
        Err(e) => Err(e),
    }
}

async fn fetch_next_task(config: &Config, filter: &str) -> Result<Option<(Task, usize)>, Error> {
    let tasks = todoist::all_tasks_by_filters(config, filter)
        .await?
        .into_iter()
        .flat_map(|(_, tasks)| tasks.to_owned())
        .collect::<Vec<Task>>();

    let tasks = tasks::sort_by_value(tasks, config);

    Ok(tasks.first().map(|task| (task.to_owned(), tasks.len())))
}

/// Put dates on all tasks without dates
pub async fn schedule(config: &Config, filter: &str, sort: &SortOrder) -> Result<String, Error> {
    let tasks = todoist::all_tasks_by_filters(config, filter)
        .await?
        .into_iter()
        .flat_map(|(_, tasks)| tasks.to_owned())
        .collect::<Vec<Task>>();

    let tasks = tasks::sort(tasks, config, sort);

    if tasks.is_empty() {
        Ok(color::green_string(&format!(
            "No tasks to schedule in '{filter}'"
        )))
    } else {
        let mut handles = Vec::new();
        for task in tasks.iter() {
            if let Some(handle) = tasks::spawn_schedule_task(config.clone(), task.clone()).await? {
                handles.push(handle);
            }
        }

        future::join_all(handles).await;
        Ok(color::green_string(&format!(
            "Successfully scheduled tasks in '{filter}'"
        )))
    }
}
/// Put deadlines on all non-recurring tasks without deadlines
pub async fn deadline(config: &Config, filter: &str, sort: &SortOrder) -> Result<String, Error> {
    let tasks = todoist::all_tasks_by_filters(config, filter)
        .await?
        .into_iter()
        .flat_map(|(_, tasks)| tasks.to_owned())
        .collect::<Vec<Task>>();

    let tasks = tasks::sort(tasks, config, sort);
    let filtered_tasks: Vec<Task> = tasks
        .into_iter()
        .filter(|task| !task.filter(config, &TaskFilter::Recurring))
        .collect::<Vec<Task>>();

    if filtered_tasks.is_empty() {
        Ok(color::green_string(&format!(
            "No tasks to deadline in '{filter}'"
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
            "Successfully deadlined tasks in '{filter}'"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test;
    use crate::test::responses::ResponseFromFile;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn test_rename_task() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/api/v1/tasks/filter?query=today&limit=200")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::TodayTasks.read().await)
            .create_async()
            .await;

        let config = test::fixtures::config()
            .await
            .with_mock_url(server.url())
            .mock_select(0);

        let result = edit_task(&config, "today".into());
        assert_eq!(result.await, Ok("Finished editing task".to_string()));
        mock.assert();
    }
    #[tokio::test]
    async fn test_get_next_task() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/api/v1/tasks/filter?query=today&limit=200")
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
            .with_timezone("US/Pacific")
            .with_path(config_dir.join("test3"));

        config_with_timezone
            .clone()
            .create()
            .await
            .expect("expected value or result, got None or Err");

        let filter = String::from("today");
        let task = next_task(&config_with_timezone, &filter)
            .await
            .expect("expected value or result, got None or Err");

        assert!(task.contains("TEST"));
        assert!(task.contains("for 15 min"));
        mock.assert();
        mock2.assert();
    }

    #[tokio::test]
    async fn test_schedule() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/api/v1/tasks/filter?query=today&limit=200")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::TodayTasks.read().await)
            .create_async()
            .await;

        let mock2 = server
            .mock("POST", "/api/v1/tasks/999999")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::TodayTask.read().await)
            .create_async()
            .await;

        let mock3 = server
            .mock(
                "GET",
                "/api/v1/comments/?task_id=6Xqhv4cwxgjwG9w8&limit=200",
            )
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

        let filter = String::from("today");
        let sort = &SortOrder::Value;
        let result = schedule(&config, &filter, sort);
        assert_eq!(
            result.await,
            Ok("Successfully scheduled tasks in 'today'".to_string())
        );

        let config = config.mock_select(2);

        let filter = String::from("today");
        let result = schedule(&config, &filter, sort);
        assert_eq!(
            result.await,
            Ok("Successfully scheduled tasks in 'today'".to_string())
        );

        mock.expect(2);
        mock2.expect(2);
        mock3.expect(2);
    }

    #[tokio::test]
    async fn test_deadline() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/api/v1/tasks/filter?query=today&limit=200")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::TodayTasks.read().await)
            .create_async()
            .await;

        let mock2 = server
            .mock("POST", "/api/v1/tasks/999999")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::TodayTask.read().await)
            .create_async()
            .await;

        let mock3 = server
            .mock(
                "GET",
                "/api/v1/comments/?task_id=6Xqhv4cwxgjwG9w8&limit=200",
            )
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

        let filter = String::from("today");
        let sort = &SortOrder::Value;
        let result = deadline(&config, &filter, sort);
        assert_eq!(
            result.await,
            Ok("Successfully deadlined tasks in 'today'".to_string())
        );

        let config = config.mock_select(2);

        let filter = String::from("today");
        let result = deadline(&config, &filter, sort);
        assert_eq!(
            result.await,
            Ok("Successfully deadlined tasks in 'today'".to_string())
        );

        mock.expect(2);
        mock2.expect(2);
        mock3.expect(2);
    }
}
