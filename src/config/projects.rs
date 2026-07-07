use crate::{color, config::Config, errors::Error, projects::Project, todoist};

impl Config {
    /// Returns projects from the config.
    #[allow(clippy::unused_async)]
    pub async fn projects(self: &Config) -> Result<Vec<Project>, Error> {
        Ok(self.projects.clone().unwrap_or_default())
    }

    pub async fn reload_projects(self: &mut Config) -> Result<String, Error> {
        let all_projects = todoist::all_projects(self, None).await?;
        let current_projects = self.projects.clone().unwrap_or_default();
        let current_project_ids: Vec<String> =
            current_projects.iter().map(|p| p.id.clone()).collect();

        let updated_projects = all_projects
            .iter()
            .filter(|p| current_project_ids.contains(&p.id))
            .map(std::borrow::ToOwned::to_owned)
            .collect::<Vec<Project>>();

        self.projects = Some(updated_projects);

        Ok(color::green_string("✓"))
    }

    pub fn add_project(&mut self, project: Project) {
        let option_projects = &mut self.projects;
        match option_projects {
            Some(projects) => {
                projects.push(project);
            }
            None => self.projects = Some(vec![project]),
        }
    }

    pub fn remove_project(&mut self, project: &Project) {
        let projects = self
            .projects
            .clone()
            .unwrap_or_default()
            .iter()
            .filter(|p| p.id != project.id)
            .map(std::borrow::ToOwned::to_owned)
            .collect::<Vec<Project>>();

        self.projects = Some(projects);
    }
}
