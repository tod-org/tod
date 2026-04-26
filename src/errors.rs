use std::{
    fmt::Display,
    num::{ParseIntError, TryFromIntError},
};

use crate::color;
use homedir::GetHomeError;
use serde::Deserialize;
use tokio::{sync::oneshot::error::RecvError, task::JoinError};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Error {
    pub message: String,
    pub source: String,
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Error { source, message } = self;
        write!(
            f,
            "Error from {}:\n{}",
            color::yellow_string(source),
            color::red_string(message)
        )
    }
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self {
            source: "io".into(),
            message: format!("{value}"),
        }
    }
}

impl From<RecvError> for Error {
    fn from(_value: RecvError) -> Self {
        Self {
            source: String::from("RecvError"),
            message: "Sender dropped without sending".to_string(),
        }
    }
}

impl From<TryFromIntError> for Error {
    fn from(value: TryFromIntError) -> Self {
        Self {
            source: "TryFromIntError".into(),
            message: format!("{value}"),
        }
    }
}

impl From<JoinError> for Error {
    fn from(value: JoinError) -> Self {
        Self {
            source: "Join on future".into(),
            message: format!("{value}"),
        }
    }
}

impl From<chrono::LocalResult<chrono::DateTime<chrono_tz::Tz>>> for Error {
    fn from(value: chrono::LocalResult<chrono::DateTime<chrono_tz::Tz>>) -> Self {
        Self {
            source: "chrono".into(),
            message: format!("{value:?}"),
        }
    }
}

impl From<tokio::sync::mpsc::error::SendError<Error>> for Error {
    fn from(value: tokio::sync::mpsc::error::SendError<Error>) -> Self {
        Self {
            source: "tokio mpsc".into(),
            message: format!("{value}"),
        }
    }
}

impl From<chrono_tz::ParseError> for Error {
    fn from(value: chrono_tz::ParseError) -> Self {
        Self {
            source: "chrono_tz".into(),
            message: format!("{value}"),
        }
    }
}

impl From<ParseIntError> for Error {
    fn from(value: ParseIntError) -> Self {
        Self {
            source: "ParseIntError".into(),
            message: format!("{value}"),
        }
    }
}

impl From<chrono::ParseError> for Error {
    fn from(value: chrono::ParseError) -> Self {
        Self {
            source: "chrono".into(),
            message: format!("{value}"),
        }
    }
}

impl From<GetHomeError> for Error {
    fn from(value: GetHomeError) -> Self {
        Self {
            source: "homedir".into(),
            message: format!("{value}"),
        }
    }
}

impl From<serde_json::Error> for Error {
    fn from(value: serde_json::Error) -> Self {
        Self {
            source: "serde_json".into(),
            message: format!("{value}"),
        }
    }
}

impl From<reqwest::Error> for Error {
    fn from(value: reqwest::Error) -> Self {
        Self {
            source: "reqwest".into(),
            message: format!("{value}"),
        }
    }
}

impl From<inquire::InquireError> for Error {
    fn from(value: inquire::InquireError) -> Self {
        Self {
            source: "inquire".into(),
            message: format!("{value}"),
        }
    }
}

impl Error {
    pub fn new(source: &str, message: &str) -> Error {
        Error {
            source: source.into(),
            message: message.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn can_format() {
        let error = Error {
            message: "there".to_string(),
            source: "hello".to_string(),
        };
        assert_eq!(error.to_string(), String::from("Error from hello:\nthere"))
    }

    #[test]
    fn test_error_new() {
        let e = Error::new("my_source", "my_message");
        assert_eq!(e.source, "my_source");
        assert_eq!(e.message, "my_message");
    }

    #[test]
    fn test_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let e: Error = io_err.into();
        assert_eq!(e.source, "io");
        assert!(e.message.contains("file not found"));
    }

    #[test]
    fn test_from_parse_int_error() {
        let parse_err = "abc".parse::<i32>().unwrap_err();
        let e: Error = parse_err.into();
        assert_eq!(e.source, "ParseIntError");
        assert!(!e.message.is_empty());
    }

    #[test]
    fn test_from_serde_json_error() {
        let serde_err = serde_json::from_str::<serde_json::Value>("not json").unwrap_err();
        let e: Error = serde_err.into();
        assert_eq!(e.source, "serde_json");
        assert!(!e.message.is_empty());
    }

    #[test]
    fn test_from_chrono_parse_error() {
        let chrono_err = "not-a-date".parse::<chrono::NaiveDate>().unwrap_err();
        let e: Error = chrono_err.into();
        assert_eq!(e.source, "chrono");
        assert!(!e.message.is_empty());
    }

    #[test]
    fn test_error_clone_and_eq() {
        let e = Error::new("src", "msg");
        let cloned = e.clone();
        assert_eq!(e, cloned);
    }
}
