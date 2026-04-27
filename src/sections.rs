use crate::{config::Config, errors::Error, input, projects::Project, todoist};
use futures::future;
use serde::Deserialize;

// Projects are split into sections
#[derive(PartialEq, Deserialize, Clone, Debug)]
pub struct Section {
    pub id: String,
    pub name: String,
    pub user_id: String,
    pub project_id: String,
    pub added_at: String,
    pub updated_at: Option<String>,
    pub archived_at: Option<String>,
    pub section_order: i32,
    pub is_archived: bool,
    pub is_deleted: bool,
    pub is_collapsed: bool,
}

#[derive(PartialEq, Deserialize, Clone, Debug)]
pub struct SectionResponse {
    pub results: Vec<Section>,
    pub next_cursor: Option<String>,
}

// Fetch all sections for all projects
/// Fetches all sections for every project stored in config, in parallel.
pub async fn all_sections(config: &mut Config) -> Result<Vec<Section>, Error> {
    let projects = config.projects().await?;

    let mut handles = Vec::new();
    for project in projects.iter() {
        let handle = todoist::all_sections_by_project(config, project, None);

        handles.push(handle);
    }

    let var = future::join_all(handles).await;

    let sections = var.into_iter().filter_map(Result::ok).flatten().collect();
    Ok(sections)
}

/// Deserializes a JSON string into a single `Section`.
pub fn json_to_section(json: &str) -> Result<Section, Error> {
    let section: Section = serde_json::from_str(json)?;
    Ok(section)
}
/// Deserializes a JSON string into a `SectionResponse` (a paginated list of sections).
pub fn json_to_sections_response(json: &str) -> Result<SectionResponse, Error> {
    let response: SectionResponse = serde_json::from_str(json)?;
    Ok(response)
}

/// Prompts the user to pick a section from those available in the given project, or returns `None` if there are none.
pub async fn select_section(config: &Config, project: &Project) -> Result<Option<Section>, Error> {
    let sections = todoist::all_sections_by_project(config, project, None).await?;
    let mut section_names: Vec<String> = sections.clone().into_iter().map(|x| x.name).collect();
    if section_names.is_empty() {
        Ok(None)
    } else {
        section_names.insert(0, "No section".to_string());
        let section_name = input::select(input::SECTION, section_names, config.mock_select)?;

        let section = sections
            .iter()
            .find(|x| x.name == section_name.as_str())
            .map(|s| s.to_owned());
        Ok(section)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test::{self, responses::ResponseFromFile};
    use pretty_assertions::assert_eq;

    #[test]
    fn should_deserialize_section_with_negative_order() {
        let json = r#"{
            "id": "1234",
            "project_id": "5678",
            "user_id": "910",
            "section_order": -1,
            "name": "Top Section",
            "added_at": "2020-06-11T14:51:08.056500Z",
            "updated_at": null,
            "archived_at": null,
            "is_archived": false,
            "is_deleted": false,
            "is_collapsed": false
        }"#;

        let result = json_to_section(json).expect("should deserialize section with negative order");

        assert_eq!(result.section_order, -1);
        assert_eq!(result.name, "Top Section");
    }

    #[test]
    fn json_to_section_invalid_json_returns_error() {
        let result = json_to_section("not json");
        assert!(result.is_err());
    }

    #[test]
    fn json_to_sections_response_invalid_json_returns_error() {
        let result = json_to_sections_response("not json");
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn should_convert_json_to_sections() {
        let sections = vec![Section {
            id: "1234".to_string(),
            added_at: "2020-06-11T14:51:08.056500Z".to_string(),
            user_id: "910".to_string(),
            project_id: "5678".to_string(),
            section_order: 1,
            name: "Bread".to_string(),
            updated_at: None,
            archived_at: None,
            is_archived: false,
            is_deleted: false,
            is_collapsed: false,
        }];
        let result = json_to_sections_response(&ResponseFromFile::Sections.read().await)
            .expect("expected value or result, got None or Err")
            .results;
        assert_eq!(result, sections);
    }

    #[tokio::test]
    async fn test_select_section() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/api/v1/sections?project_id=123&limit=200")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::Sections.read().await)
            .create_async()
            .await;

        let config = test::fixtures::config()
            .await
            .with_mock_url(server.url())
            .mock_select(1);
        let project = test::fixtures::project();

        let expected = Ok(Some(Section {
            id: "1234".to_string(),
            user_id: "910".to_string(),
            added_at: "2020-06-11T14:51:08.056500Z".to_string(),
            project_id: "5678".to_string(),
            section_order: 1,
            name: "Bread".to_string(),
            updated_at: None,
            archived_at: None,
            is_archived: false,
            is_deleted: false,
            is_collapsed: false,
        }));
        let result = select_section(&config, &project).await;
        assert_eq!(expected, result);
        mock.assert();
    }
}
