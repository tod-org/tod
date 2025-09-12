use crate::errors::Error;
use serde::Deserialize;

#[derive(Deserialize, Debug, PartialEq, Eq)]
pub struct User {
    pub tz_info: TzInfo,
}
// This file is used to pull the user information (timezone) from the Todoist API
#[derive(Deserialize, Debug, PartialEq, Eq)]
pub struct TzInfo {
    pub timezone: String,
}

pub fn json_to_user(json: &str) -> Result<User, Error> {
    // Deserializes JSON string into a `User` struct using Serde.
    // Returns an error if the JSON string does not match the `User` struct format.
    let user: User = serde_json::from_str(json)?;
    Ok(user)
}
