use std::fmt::Display;

use crate::{config::Config, errors::Error, todoist};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, PartialEq, Eq)]
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
impl Label {
    pub fn from_json(json: &str) -> Result<Label, Error> {
        let label: Label = serde_json::from_str(json)?;
        Ok(label)
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

pub async fn create(
    config: &Config,
    name: &str,
    color: Option<&str>,
    order: Option<u32>,
    is_favorite: bool,
) -> Result<Label, Error> {
    todoist::create_label(config, name, color, order, is_favorite, true).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_label_from_json_valid() {
        let json =
            r#"{"id":"1","name":"work","color":"red","order":1,"is_favorite":false}"#;
        let label = Label::from_json(json).expect("should parse label");
        assert_eq!(label.id, "1");
        assert_eq!(label.name, "work");
        assert_eq!(label.color, "red");
        assert_eq!(label.order, Some(1));
        assert!(!label.is_favorite);
    }

    #[test]
    fn test_label_from_json_invalid() {
        let result = Label::from_json("not json");
        assert!(result.is_err());
    }

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

    mod proptests {
        use super::*;
        use pretty_assertions::assert_eq;
        use proptest::prelude::*;

        fn arb_label() -> impl Strategy<Value = Label> {
            (
                "[0-9a-f]{5,20}",
                "[A-Za-z0-9 _-]{1,30}",
                "[a-z]{3,10}",
                proptest::option::of(0u32..100),
                proptest::bool::ANY,
            )
                .prop_map(|(id, name, color, order, is_favorite)| Label {
                    id,
                    name,
                    color,
                    order,
                    is_favorite,
                })
        }

        proptest! {
            #[test]
            fn label_serde_roundtrip(label in arb_label()) {
                let json = serde_json::to_string(&label).unwrap();
                let roundtripped: Label = serde_json::from_str(&json).unwrap();
                assert_eq!(label, roundtripped);
            }

            #[test]
            fn label_response_deserialize_from_labels(
                labels in proptest::collection::vec(arb_label(), 0..10),
                next_cursor in proptest::option::of("[a-zA-Z0-9]{5,20}"),
            ) {
                let results_json = serde_json::to_string(&labels).unwrap();
                let cursor_json = match &next_cursor {
                    Some(c) => format!("\"{}\"", c),
                    None => "null".to_string(),
                };
                let json = format!(
                    "{{\"results\":{},\"next_cursor\":{}}}",
                    results_json, cursor_json
                );
                let response = LabelResponse::from_json(&json).unwrap();
                assert_eq!(response.results, labels);
                assert_eq!(response.next_cursor, next_cursor);
            }
        }
    }
}
