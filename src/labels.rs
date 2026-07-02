use std::fmt::Display;

use crate::{config::Config, errors::Error, todoist};
use serde::Deserialize;

#[derive(Deserialize, Debug, PartialEq, Eq)]
pub struct Label {
    pub id: String,
    pub name: String,
    pub color: String,
    pub order: Option<u32>,
    pub is_favorite: bool,
}

#[allow(dead_code)]
#[derive(Deserialize)]
pub struct LabelResponse {
    pub results: Vec<Label>,
    pub next_cursor: Option<String>,
}
impl LabelResponse {
    pub fn from_json(json: &str) -> Result<LabelResponse, Error> {
        let response: LabelResponse = serde_json::from_str(json)?;
        Ok(response)
    }
}
impl Display for Label {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = self.name.clone();
        write!(f, "{name}")
    }
}
pub async fn get_labels(config: &Config, spinner: bool) -> Result<Vec<Label>, Error> {
    todoist::all_labels(config, spinner, None).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_label_fmt() {
        let label = Label {
            id: "1".to_string(),
            name: "work".to_string(),
            color: "red".to_string(),
            order: Some(1),
            is_favorite: false,
        };
        assert_eq!(label.to_string(), "work");
    }

    #[test]
    fn test_from_json_response_valid() {
        let json = r#"{"results":[{"id":"1","name":"work","color":"red","order":1,"is_favorite":false}],"next_cursor":null}"#;
        let response = LabelResponse::from_json(json).expect("should parse labels response");
        assert_eq!(response.results.len(), 1);
        assert_eq!(response.results[0].name, "work");
        assert_eq!(response.results[0].color, "red");
        assert!(response.next_cursor.is_none());
    }

    #[test]
    fn test_from_json_response_with_cursor() {
        let json = r#"{"results":[],"next_cursor":"abc123"}"#;
        let response =
            LabelResponse::from_json(json).expect("should parse labels response with cursor");
        assert!(response.results.is_empty());
        assert_eq!(response.next_cursor, Some("abc123".to_string()));
    }

    #[test]
    fn test_from_json_response_invalid() {
        let result = LabelResponse::from_json("not json");
        assert!(result.is_err());
    }
}
