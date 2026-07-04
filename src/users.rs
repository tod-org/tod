use crate::errors::Error;
use serde::Deserialize;

#[derive(Deserialize, Debug, PartialEq, Eq)]
pub struct User {
    pub tz_info: TzInfo,
}

impl User {
    pub fn from_json(json: &str) -> Result<User, Error> {
        // Deserializes JSON string into a `User` struct using Serde.
        // Returns an error if the JSON string does not match the `User` struct format.
        let user: User = serde_json::from_str(json)?;
        Ok(user)
    }
}
// This file is used to pull the user information (timezone) from the Todoist API
#[derive(Deserialize, Debug, PartialEq, Eq)]
pub struct TzInfo {
    pub timezone: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_test::{Token, assert_de_tokens};

    #[test]
    fn user_deserializes_with_serde_tokens() {
        let expected = User {
            tz_info: TzInfo {
                timezone: "America/Vancouver".to_string(),
            },
        };

        assert_de_tokens(
            &expected,
            &[
                Token::Struct {
                    name: "User",
                    len: 1,
                },
                Token::Str("tz_info"),
                Token::Struct {
                    name: "TzInfo",
                    len: 1,
                },
                Token::Str("timezone"),
                Token::Str("America/Vancouver"),
                Token::StructEnd,
                Token::StructEnd,
            ],
        );
    }
}
