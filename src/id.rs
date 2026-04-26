use serde::Deserialize;
use std::fmt::Display;

use crate::errors::Error;

#[derive(Clone)]
pub enum Resource {
    Project,
}

#[derive(Deserialize)]
pub struct Id {
    pub new_id: String,
}
pub fn json_to_ids(json: String) -> Result<Vec<Id>, Error> {
    let ids: Vec<Id> = serde_json::from_str(&json)?;
    Ok(ids)
}
impl Display for Resource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Resource::Project => "projects",
        };
        write!(f, "{name}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_resource_fmt_project() {
        assert_eq!(Resource::Project.to_string(), "projects");
    }

    #[test]
    fn test_json_to_ids_valid() {
        let json = r#"[{"new_id": "abc123"},{"new_id": "def456"}]"#.to_string();
        let ids = json_to_ids(json).expect("should parse valid ids JSON");
        assert_eq!(ids.len(), 2);
        assert_eq!(ids[0].new_id, "abc123");
        assert_eq!(ids[1].new_id, "def456");
    }

    #[test]
    fn test_json_to_ids_empty_array() {
        let json = "[]".to_string();
        let ids = json_to_ids(json).expect("should parse empty array");
        assert!(ids.is_empty());
    }

    #[test]
    fn test_json_to_ids_invalid_json() {
        let json = "not json".to_string();
        let result = json_to_ids(json);
        assert!(result.is_err());
    }
}
