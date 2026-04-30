//! Responses are for generating JSON and mocking API calls

use crate::{VERSION, cargo, comments, id, labels, oauth, projects, sections, tasks, users};
use serde_json::Value;
use std::path::{Path, PathBuf};

const RESPONSE_ROOT: &str = "tests/responses";

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
        let path = format!("{RESPONSE_ROOT}/{self}.json");

        let json = std::fs::read_to_string(&path)
            .unwrap_or_else(|_| panic!("Could not find json file at {path}"));

        self.replace_values(json).await
    }

    /// Loads JSON and replaces INSERTVERSION with a custom version string
    pub async fn read_with_version(&self, version: &str) -> String {
        let path = format!("{RESPONSE_ROOT}/{self}.json");

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

#[derive(Clone, Copy, Debug)]
enum ResponseKind {
    AccessToken,
    Comment,
    Ids,
    Label,
    Project,
    Section,
    Task,
    User,
    Version,
}

#[tokio::test]
async fn every_response_blob_deserializes_with_expected_parser() {
    let mut paths = response_blob_paths(Path::new(RESPONSE_ROOT)).unwrap_or_else(|error| {
        panic!("failed to discover response blobs under {RESPONSE_ROOT}: {error}")
    });

    paths.sort();

    assert!(
        !paths.is_empty(),
        "expected at least one JSON response blob under {RESPONSE_ROOT}"
    );

    let mut failures = Vec::new();
    for path in paths {
        match test_response_blob(&path).await {
            Ok(()) => {}
            Err(error) => failures.push(error),
        }
    }

    assert!(
        failures.is_empty(),
        "response blob parser failures:\n\n{}",
        failures.join("\n\n")
    );
}

async fn test_response_blob(path: &Path) -> Result<(), String> {
    let kind = classify_response_blob(path).ok_or_else(|| {
        format!(
            "{}: no response parser matched this blob. Put it under a typed directory like \
             comments/, tasks/, projects/, sections/, or labels/, or name it after its response type.",
            path.display()
        )
    })?;
    let json = read_response_blob(path).await?;

    parse_response_blob(path, kind, &json)
}

async fn read_response_blob(path: &Path) -> Result<String, String> {
    let mut json = std::fs::read_to_string(path)
        .map_err(|error| format!("{}: failed to read blob: {error}", path.display()))?;

    if json.contains("INSERTDATE") {
        json = json.replace("INSERTDATE", &super::today_date().await);
    }

    if json.contains("INSERTVERSION") {
        json = json.replace("INSERTVERSION", VERSION);
    }

    Ok(json)
}

fn response_blob_paths(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut paths = Vec::new();
    collect_response_blob_paths(root, &mut paths)?;
    Ok(paths)
}

fn collect_response_blob_paths(dir: &Path, paths: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = std::fs::read_dir(dir)
        .map_err(|error| format!("{}: failed to read directory: {error}", dir.display()))?;

    for entry in entries {
        let entry =
            entry.map_err(|error| format!("{}: failed to read entry: {error}", dir.display()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| format!("{}: failed to read file type: {error}", path.display()))?;

        if file_type.is_dir() {
            collect_response_blob_paths(&path, paths)?;
        } else if is_json_file(&path) {
            paths.push(path);
        }
    }

    Ok(())
}

fn is_json_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
}

fn classify_response_blob(path: &Path) -> Option<ResponseKind> {
    classify_by_parent_directory(path).or_else(|| classify_by_file_stem(path))
}

fn classify_by_parent_directory(path: &Path) -> Option<ResponseKind> {
    let mut directory = path.parent();

    while let Some(path) = directory {
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            break;
        };
        let name = normalize(name);

        if name == "responses" {
            break;
        }

        if let Some(kind) = classify_directory_name(&name) {
            return Some(kind);
        }

        directory = path.parent();
    }

    None
}

fn classify_by_file_stem(path: &Path) -> Option<ResponseKind> {
    let stem = path.file_stem()?.to_str()?;
    let stem = normalize(stem);

    match stem.as_str() {
        "accesstoken" => Some(ResponseKind::AccessToken),
        "ids" => Some(ResponseKind::Ids),
        "user" => Some(ResponseKind::User),
        "versions" => Some(ResponseKind::Version),
        _ if stem.contains("comment") => Some(ResponseKind::Comment),
        _ if stem.contains("label") => Some(ResponseKind::Label),
        _ if stem.contains("project") => Some(ResponseKind::Project),
        _ if stem.contains("section") => Some(ResponseKind::Section),
        _ if stem.contains("task") => Some(ResponseKind::Task),
        _ => None,
    }
}

fn classify_directory_name(name: &str) -> Option<ResponseKind> {
    match name {
        "comments" | "comment" => Some(ResponseKind::Comment),
        "labels" | "label" => Some(ResponseKind::Label),
        "projects" | "project" => Some(ResponseKind::Project),
        "sections" | "section" => Some(ResponseKind::Section),
        "tasks" | "task" => Some(ResponseKind::Task),
        "users" | "user" => Some(ResponseKind::User),
        "versions" | "version" => Some(ResponseKind::Version),
        "ids" => Some(ResponseKind::Ids),
        "accesstokens" | "accesstoken" => Some(ResponseKind::AccessToken),
        _ => None,
    }
}

fn normalize(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn parse_response_blob(path: &Path, kind: ResponseKind, json: &str) -> Result<(), String> {
    match kind {
        ResponseKind::AccessToken => oauth::json_to_access_token(json.to_string())
            .map(|_| ())
            .map_err(|error| parse_error(path, "access token", error)),
        ResponseKind::Comment => parse_single_or_response(
            path,
            json,
            "comment",
            |json| comments::json_to_comment(json.to_string()).map(|_| ()),
            |json| comments::json_to_comment_response(json.to_string()).map(|_| ()),
        ),
        ResponseKind::Ids => id::json_to_ids(json.to_string())
            .map(|_| ())
            .map_err(|error| parse_error(path, "id list", error)),
        ResponseKind::Label => parse_single_or_response(
            path,
            json,
            "label",
            |json| labels::json_to_label(json).map(|_| ()),
            |json| labels::json_to_labels_response(json).map(|_| ()),
        ),
        ResponseKind::Project => parse_single_or_response(
            path,
            json,
            "project",
            |json| projects::json_to_project(json.to_string()).map(|_| ()),
            |json| projects::json_to_projects_response(json.to_string()).map(|_| ()),
        ),
        ResponseKind::Section => parse_single_or_response(
            path,
            json,
            "section",
            |json| sections::json_to_section(json).map(|_| ()),
            |json| sections::json_to_sections_response(json).map(|_| ()),
        ),
        ResponseKind::Task => parse_single_or_response(
            path,
            json,
            "task",
            |json| tasks::json_to_task(json.to_string()).map(|_| ()),
            |json| tasks::json_to_tasks_response(json.to_string()).map(|_| ()),
        ),
        ResponseKind::User => users::json_to_user(json)
            .map(|_| ())
            .map_err(|error| parse_error(path, "user", error)),
        ResponseKind::Version => cargo::json_to_latest_version(json)
            .map(|_| ())
            .map_err(|error| parse_error(path, "version response", error)),
    }
}

fn parse_single_or_response<Single, Response>(
    path: &Path,
    json: &str,
    type_name: &str,
    parse_single: Single,
    parse_response: Response,
) -> Result<(), String>
where
    Single: FnOnce(&str) -> Result<(), crate::errors::Error>,
    Response: FnOnce(&str) -> Result<(), crate::errors::Error>,
{
    if has_results(json).map_err(|error| parse_error(path, "JSON object", error))? {
        parse_response(json)
            .map_err(|error| parse_error(path, &format!("{type_name} response"), error))
    } else {
        parse_single(json).map_err(|error| parse_error(path, type_name, error))
    }
}

fn has_results(json: &str) -> Result<bool, serde_json::Error> {
    let value: Value = serde_json::from_str(json)?;
    Ok(value.get("results").is_some())
}

fn parse_error<E>(path: &Path, type_name: &str, error: E) -> String
where
    E: std::fmt::Display,
{
    format!(
        "{}: failed to parse as {type_name}: {error}",
        path.display()
    )
}
