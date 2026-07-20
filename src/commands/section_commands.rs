use crate::{config::Config, errors::Error, format, input, lists::Flag, todoist};
use clap::{Parser, Subcommand};

#[derive(Subcommand, Debug, Clone)]
pub enum SectionCommands {
    #[clap(alias = "c")]
    /// (c) Create a new section for a project in Todoist
    Create(Create),
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

pub async fn create(config: &Config, args: &Create) -> Result<String, Error> {
    let Create { name, project } = args;
    let name = super::fetch_string(name.as_deref(), config, input::NAME)?;

    let project = match super::fetch_project(project.as_deref(), config).await? {
        Flag::Project(project) => project,
        Flag::Filter(_) => unreachable!(),
    };

    todoist::create_section(config, &name, &project, true).await?;
    Ok(format::green_string("Section created successfully"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn create_fails_when_no_projects_exist_in_config() {
        let config = Config::default();
        let args = Create {
            name: Some("new-section".to_string()),
            project: None,
        };

        let error = create(&config, &args)
            .await
            .expect_err("creating a section should fail without configured projects");

        assert_eq!(error.source, "fetch_project");
        assert!(error.message.contains("No projects in config"));
    }
}
