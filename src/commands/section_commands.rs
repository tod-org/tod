use crate::{config::Config, errors::Error, format, input, lists::Flag, todoist};
use clap::{Parser, Subcommand};

/// Section subcommands.
#[derive(Subcommand, Debug, Clone)]
pub enum SectionCommands {
    #[clap(alias = "c")]
    /// (c) Create a new section for a project in Todoist
    Create(Create),
    #[clap(alias = "d")]
    /// (d) Delete a section from a project in Todoist
    Delete(Delete),
}

#[derive(Parser, Debug, Clone)]
pub struct Create {
    #[arg(short, long)]
    /// Section name
    name: Option<String>,

    #[arg(short, long)]
    /// Project to put the section in
    project: Option<String>,
}

/// Creates a section in a Todoist project.
pub async fn create(config: &Config, args: &Create, json: bool) -> Result<String, Error> {
    let Create { name, project } = args;
    let name = super::fetch_string(name.as_deref(), config, input::NAME)?;

    let project = match super::fetch_project(project.as_deref(), config).await? {
        Flag::Project(project) => project,
        Flag::Filter(_) => unreachable!(),
    };

    let section = todoist::create_section(config, &name, &project, true).await?;
    if json {
        Ok(serde_json::to_string(&section)?)
    } else {
        Ok(format::green_string("Section created successfully"))
    }
}

/// Deletes a section from a Todoist project.
pub async fn delete(config: &Config, args: &Delete, json: bool) -> Result<String, Error> {
    let Delete {
        force,
        section,
        project,
        repeat,
    } = args;
    loop {
        let project = match super::fetch_project(project.as_deref(), config).await? {
            Flag::Project(project) => project,
            Flag::Filter(_) => unreachable!(),
        };

        let sections = todoist::all_sections_by_project(config, &project, None).await?;

        if sections.is_empty() {
            return Ok("No sections found for this project".into());
        }

        let section = if let Some(name) = section {
            sections
                .iter()
                .find(|s| s.name == *name)
                .cloned()
                .ok_or_else(|| {
                    Error::new(
                        "delete_section",
                        &format!(
                            "Section \"{name}\" not found in project \"{}\"",
                            project.name
                        ),
                    )
                })?
        } else if json {
            return Err(Error::new("json_mode", super::JSON_INTERACTIVE_ERROR));
        } else {
            let section_names: Vec<String> = sections.iter().map(|s| s.name.clone()).collect();
            let selected = input::select(input::SECTION, section_names, config.mock_select)?;
            sections.into_iter().find(|s| s.name == selected).unwrap()
        };

        if !force {
            if json {
                return Err(Error::new("json_mode", super::JSON_INTERACTIVE_ERROR));
            }
            let options = vec![input::CANCEL, input::DELETE];
            let desc = format!(
                "Delete section \"{}\"? Tasks inside will also be deleted.",
                section.name
            );
            let result = input::select(&desc, options, config.mock_select)?;
            if result == input::CANCEL {
                return Ok("Cancelled".into());
            }
        }

        todoist::delete_section(config, &section.id, true).await?;

        if !repeat {
            if json {
                return Ok(serde_json::to_string(&section)?);
            }
            return Ok(format::green_string(&format!(
                "Section \"{}\" deleted",
                section.name
            )));
        }
    }
}

#[derive(Parser, Debug, Clone)]
pub struct Delete {
    #[arg(short, long, default_value_t = false)]
    /// Skip deletion confirmation
    force: bool,

    #[arg(short = 'r', long, default_value_t = false)]
    /// Keep repeating prompt to delete sections
    repeat: bool,

    #[arg(short, long)]
    /// Section to delete
    section: Option<String>,

    #[arg(short = 'p', long)]
    /// Project the section belongs to
    project: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test;
    use crate::test::responses::ResponseFromFile;

    #[tokio::test]
    async fn create_fails_when_no_projects_exist_in_config() {
        let config = Config::default_test();
        let args = Create {
            name: Some("new-section".to_string()),
            project: None,
        };

        let error = create(&config, &args, false)
            .await
            .expect_err("creating a section should fail without configured projects");

        assert_eq!(error.source, "fetch_project");
        assert!(error.message.contains("No projects in config"));
    }

    #[tokio::test]
    async fn delete_fails_when_section_not_found() {
        let mut server = mockito::Server::new_async().await;

        let sections_mock = server
            .mock("GET", "/api/v1/sections?project_id=123&limit=200")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::Sections.read().await)
            .create_async()
            .await;

        let config = test::fixtures::config().await.with_mock_url(server.url());

        let args = Delete {
            force: false,
            repeat: false,
            section: Some("nonexistent".to_string()),
            project: Some("myproject".to_string()),
        };

        let error = delete(&config, &args, false)
            .await
            .expect_err("deleting a nonexistent section should fail");

        assert_eq!(error.source, "delete_section");
        assert!(error.message.contains("not found"));
        sections_mock.assert_async().await;
    }

    #[tokio::test]
    async fn delete_force_skips_confirmation() {
        let mut server = mockito::Server::new_async().await;

        let sections_mock = server
            .mock("GET", "/api/v1/sections?project_id=123&limit=200")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::Sections.read().await)
            .create_async()
            .await;

        let delete_mock = server
            .mock("DELETE", "/api/v1/sections/1234")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body("null")
            .create_async()
            .await;

        let config = test::fixtures::config().await.with_mock_url(server.url());

        let args = Delete {
            force: true,
            repeat: false,
            section: Some("Bread".to_string()),
            project: Some("myproject".to_string()),
        };

        let result = delete(&config, &args, false)
            .await
            .expect("force delete should succeed");

        assert!(result.contains("deleted"));
        sections_mock.assert_async().await;
        delete_mock.assert_async().await;
    }

    #[tokio::test]
    async fn delete_cancels_when_user_selects_cancel() {
        let mut server = mockito::Server::new_async().await;

        let sections_mock = server
            .mock("GET", "/api/v1/sections?project_id=123&limit=200")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::Sections.read().await)
            .create_async()
            .await;

        let config = test::fixtures::config()
            .await
            .with_mock_url(server.url())
            .mock_select(0);

        let args = Delete {
            force: false,
            repeat: false,
            section: Some("Bread".to_string()),
            project: Some("myproject".to_string()),
        };

        let result = delete(&config, &args, false)
            .await
            .expect("cancel should not error");

        assert_eq!(result, "Cancelled");
        sections_mock.assert_async().await;
    }
}
