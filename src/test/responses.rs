//! Responses are for generating JSON and mocking API calls

use crate::VERSION;

/// File name is the same as the enum name
/// So you can find the `Task` variant in tests/responses/Task.json
#[derive(strum_macros::Display)]
pub enum ResponseFromFile {
    AccessToken,
    /// List of all kinds of comments
    CommentsAllTypes,
    /// An unscheduled task
    Task,
    TodayTasksWithoutDuration,
    /// Today with no due and no deadline
    UnscheduledTasks,
    /// A task where all dates are set to today
    TodayTask,
    TodayTasks,
    Comment,
    #[allow(dead_code)]
    Label,
    Labels,
    Project,
    Projects,
    // Has a new ID
    NewProjects,
    Section,
    Sections,
    /// Data about the logged in user
    User,
    /// Response from crates.io API
    Versions,
}

impl ResponseFromFile {
    /// Loads JSON responses from file for testing
    pub async fn read(&self) -> String {
        let path = format!("tests/responses/{self}.json");

        let json = std::fs::read_to_string(&path)
            .unwrap_or_else(|_| panic!("Could not find json file at {path}"));

        self.replace_values(json).await
    }

    /// Loads JSON and replaces INSERTVERSION with a custom version string
    pub async fn read_with_version(&self, version: &str) -> String {
        let path = format!("tests/responses/{self}.json");

        let json = std::fs::read_to_string(&path)
            .unwrap_or_else(|_| panic!("Could not find json file at {path}"));

        match self {
            Self::Versions => json.replace("INSERTVERSION", version),
            _ => self.replace_values(json).await,
        }
    }

    /// Allows us to replace static values in JSON with dynamic data
    async fn replace_values(&self, json_string: String) -> String {
        let replace_with: Vec<(&str, String)> = match self {
            Self::AccessToken => Vec::new(),
            Self::CommentsAllTypes => Vec::new(),
            Self::Comment => Vec::new(),
            Self::Task => Vec::new(),
            Self::Section => Vec::new(),
            Self::Sections => Vec::new(),
            Self::Label => Vec::new(),
            Self::Labels => Vec::new(),
            Self::Project => Vec::new(),
            Self::Projects => Vec::new(),
            Self::NewProjects => Vec::new(),
            Self::User => Vec::new(),
            Self::TodayTask => vec![("INSERTDATE", super::today_date().await)],
            Self::UnscheduledTasks => vec![("INSERTDATE", super::today_date().await)],
            Self::TodayTasksWithoutDuration => vec![("INSERTDATE", super::today_date().await)],
            Self::TodayTasks => vec![("INSERTDATE", super::today_date().await)],
            Self::Versions => vec![("INSERTVERSION", VERSION.to_string())],
        };

        let mut result = json_string;

        for (from, to) in replace_with {
            result = result.replace(from, &to);
        }
        result
    }
}
