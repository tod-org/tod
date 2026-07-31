use crate::{config::Config, errors::Error, format, projects::Project, todoist};

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

        Ok(format::green_string("✓"))
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

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn add_project_when_projects_is_none_initializes_vec() {
        let mut config = Config::default();
        config.projects = None;
        assert!(config.projects.is_none());

        let project = Project {
            id: "123".to_string(),
            can_assign_tasks: false,
            child_order: 0,
            color: "blue".to_string(),
            created_at: None,
            is_archived: false,
            is_deleted: false,
            is_favorite: false,
            is_frozen: false,
            name: "test".to_string(),
            updated_at: None,
            view_style: "list".to_string(),
            default_order: 0,
            description: String::new(),
            parent_id: None,
            inbox_project: None,
            is_collapsed: false,
            is_shared: false,
        };

        config.add_project(project.clone());

        let projects = config.projects.expect("projects should be initialized");
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].id, "123");
    }

    #[test]
    fn remove_project_when_projects_is_none_does_not_panic() {
        let mut config = Config::default();
        config.projects = None;
        assert!(config.projects.is_none());

        let project = Project {
            id: "nonexistent".to_string(),
            can_assign_tasks: false,
            child_order: 0,
            color: "blue".to_string(),
            created_at: None,
            is_archived: false,
            is_deleted: false,
            is_favorite: false,
            is_frozen: false,
            name: "dummy".to_string(),
            updated_at: None,
            view_style: "list".to_string(),
            default_order: 0,
            description: String::new(),
            parent_id: None,
            inbox_project: None,
            is_collapsed: false,
            is_shared: false,
        };

        config.remove_project(&project);

        let projects = config
            .projects
            .expect("projects should be set to Some even when empty");
        assert!(projects.is_empty());
    }
}
