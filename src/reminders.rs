use std::fmt::Display;

use serde::{Deserialize, Serialize};

use crate::{
    config::Config,
    errors::Error,
    format,
    tasks::{DateInfo, Task},
    todoist,
};

#[allow(clippy::struct_excessive_bools)]
#[derive(PartialEq, Eq, Serialize, Deserialize, Clone, Debug)]
pub struct Reminder {
    pub id: String,
    pub item_id: String,
    pub notify_uid: String,
    pub r#type: String,
    pub is_deleted: bool,
    pub minute_offset: Option<u32>,
    pub is_urgent: bool,
    pub due: Option<DateInfo>,
}

impl Reminder {
    pub fn from_json(json: &str) -> Result<Reminder, Error> {
        let reminder: Reminder = serde_json::from_str(json)?;
        Ok(reminder)
    }
}

impl Display for Reminder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(dateinfo) = &self.due {
            write!(f, "{}", dateinfo)
        } else if let Some(minute_offset) = self.minute_offset {
            write!(f, "{}", minute_offset)
        } else {
            write!(f, "{:?}", self)
        }
    }
}

#[derive(PartialEq, Eq, Serialize, Deserialize, Clone, Debug)]
pub struct ReminderResponse {
    pub results: Vec<Reminder>,
    pub next_cursor: Option<String>,
}

impl ReminderResponse {
    pub fn from_json(json: &str) -> Result<ReminderResponse, Error> {
        let response: ReminderResponse = serde_json::from_str(json)?;
        Ok(response)
    }
}

/// List all the reminders with their tasks
pub async fn list(config: &mut Config) -> Result<String, Error> {
    let reminders = todoist::all_reminders(config, None).await?;

    let task_ids = reminders
        .clone()
        .into_iter()
        .map(|reminder| reminder.item_id)
        .collect::<Vec<_>>();

    let tasks = todoist::all_tasks_by_ids(config, task_ids, None).await?;

    let reminders_with_tasks = reminders
        .into_iter()
        .map(|reminder| (find_task(&tasks, &reminder), reminder))
        .collect::<Vec<(Option<Task>, Reminder)>>();

    let mut buffer = String::new();
    buffer.push_str(&format::green_string("Reminders"));

    for (maybe_task, reminder) in reminders_with_tasks {
        buffer.push_str("\n - ");
        buffer.push_str(&reminder.to_string());
        if let Some(task) = maybe_task {
            buffer.push_str("\n   ");
            buffer.push_str(&task.to_string());
        }
    }
    Ok(buffer)
}

fn find_task(tasks: &[Task], reminder: &Reminder) -> Option<Task> {
    tasks.iter().find(|t| t.id == reminder.item_id).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test;
    use crate::test::responses::ResponseFromFile;
    #[tokio::test]
    async fn test_list() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/api/v1/reminders?limit=200")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::Reminders.read().await)
            .create_async()
            .await;

        let mock2 = server
            .mock("GET", "/api/v1/tasks/?ids=6Xqhv4cwxgjwG9w8&limit=200")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::TodayTasks.read().await)
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

        let str = "Reminders\n - 2026-01-18 17:00\n   TEST";

        assert_eq!(list(&mut config).await, Ok(String::from(str)));
        mock.expect(1);
        mock2.expect(1);
    }
    use pretty_assertions::assert_eq;
}
