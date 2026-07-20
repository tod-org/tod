use clap::{Parser, Subcommand};

use crate::reminders;
use crate::{config::Config, errors::Error};

#[derive(Subcommand, Debug, Clone)]
pub enum ReminderCommands {
    #[clap(alias = "l")]
    /// (l) List all reminders from the API
    List(List),
}

#[derive(Parser, Debug, Clone)]
pub struct List {}

pub async fn list(config: &mut Config, _args: &List) -> Result<String, Error> {
    reminders::list(config).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test;
    use crate::test::responses::ResponseFromFile;
    use mockito::Server;

    #[tokio::test]
    async fn list_delegates_to_reminders_module() {
        let mut server = Server::new_async().await;
        let reminders_mock = server
            .mock("GET", "/api/v1/reminders?limit=200")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::Reminders.read().await)
            .create_async()
            .await;

        let tasks_mock = server
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
            .expect("config should be created")
            .with_mock_url(server.url())
            .with_projects(vec![test::fixtures::project()]);
        config.save().await.expect("config should be saved");

        let output = list(&mut config, &List {})
            .await
            .expect("reminder list command should succeed");
        assert!(output.contains("Reminders"));

        reminders_mock.assert();
        tasks_mock.assert();
    }
}
