use crate::errors::Error;
use inquire::{DateSelect, MultiSelect, Select, Text};
use std::fmt::Display;
use terminal_size::{Height, Width, terminal_size};

// These constants are used throughout the app

// Set
pub const CONTENT: &str = "Set content";
pub const DESCRIPTION: &str = "Set description";
pub const NAME: &str = "Set name";
pub const FILTER: &str = "Set filter";
pub const PATH: &str = "Set path";
pub const DATE: &str = "Set a due date";
pub const TIME: &str = "Set time, i.e. 3pm or 1500";
pub const DURATION: &str = "Set duration in minutes";

// Select
pub const ATTRIBUTES: &str = "Select attributes";
pub const PROJECT: &str = "Select a project";
pub const LABELS: &str = "Select labels";
pub const SECTION: &str = "Select section";
pub const PRIORITY: &str = "Select priority";
pub const OPTION: &str = "Select an option";
pub const SELECT_DATE: &str = "Select a date";
pub const TASK: &str = "Select a task";

// Options
pub const NAT_LANG: &str = "Natural Language";
pub const NO_DATE: &str = "No Date";
pub const COMPLETE: &str = "Complete";
pub const TIMEBOX: &str = "Timebox";
pub const COMMENT: &str = "Comment";
pub const SKIP: &str = "Skip";
pub const DELETE: &str = "Delete";
pub const CANCEL: &str = "Cancel";
pub const QUIT: &str = "Quit";
pub const SCHEDULE: &str = "Schedule";

pub enum DateTimeInput {
    Skip,
    None,
    Complete,
    Text(String),
}

/// Get datetime input from user
/// skip_or_delete enables the skip and delete options
/// it is generally true when processing tasks
pub fn datetime(
    mock_select: Option<usize>,
    mock_string: Option<String>,
    natural_language_only: Option<bool>,
    no_natural_language: bool,
    skip_or_complete: bool,
) -> Result<DateTimeInput, Error> {
    let selection = if natural_language_only.unwrap_or_default() {
        NAT_LANG
    } else if no_natural_language && skip_or_complete {
        let options = vec![SELECT_DATE, NO_DATE, SKIP, COMPLETE];
        let description = DATE;
        select(description, options, mock_select)?
    } else if !no_natural_language && skip_or_complete {
        let options = vec![SELECT_DATE, NAT_LANG, NO_DATE, SKIP, COMPLETE];
        let description = DATE;
        select(description, options, mock_select)?
    } else {
        let options = vec![SELECT_DATE, NAT_LANG, NO_DATE];
        let description = DATE;
        select(description, options, mock_select)?
    };

    match selection {
        NAT_LANG => {
            if skip_or_complete {
                let entry = string(
                    "Enter datetime in natural language, or one of:\n[none (n), skip (s), complete (c)]",
                    mock_string,
                )?;

                match entry.as_str() {
                    "none" => Ok(DateTimeInput::None),
                    "n" => Ok(DateTimeInput::None),
                    "complete" => Ok(DateTimeInput::Complete),
                    "c" => Ok(DateTimeInput::Complete),
                    "skip" => Ok(DateTimeInput::Skip),
                    "s" => Ok(DateTimeInput::Skip),
                    _ => Ok(DateTimeInput::Text(entry)),
                }
            } else {
                let entry = string(
                    "Enter datetime in natural language, or none (n)",
                    mock_string,
                )?;

                match entry.as_str() {
                    "none" => Ok(DateTimeInput::None),
                    "n" => Ok(DateTimeInput::None),
                    _ => Ok(DateTimeInput::Text(entry)),
                }
            }
        }
        SELECT_DATE => {
            let string = date()?;
            Ok(DateTimeInput::Text(string))
        }

        NO_DATE => Ok(DateTimeInput::None),
        "Complete" => Ok(DateTimeInput::Complete),
        SKIP => Ok(DateTimeInput::Skip),
        _ => Err(Error {
            message: "Unrecognized input".into(),
            source: "Datetime Input".into(),
        }),
    }
}

/// Displays an interactive calendar and returns the selected date as a `YYYY-MM-DD` string.
pub fn date() -> Result<String, Error> {
    let string = DateSelect::new("Select Date")
        .with_help_message(
            "arrows to move, []{} move months and years, enter to select, esc to cancel",
        )
        .prompt()
        .map_err(Error::from)?
        .to_string();

    Ok(string)
}

/// Get text input from user
pub fn string(desc: &str, mock_string: Option<String>) -> Result<String, Error> {
    if cfg!(test) {
        if let Some(string) = mock_string {
            Ok(string)
        } else {
            panic!("Must set mock_string in config")
        }
    } else {
        Text::new(desc).prompt().map_err(Error::from)
    }
}

/// Get string input with default value
pub fn string_with_default(desc: &str, default_message: &str) -> Result<String, Error> {
    if cfg!(test) {
        return Ok(default_message.into());
    }

    Text::new(desc)
        .with_initial_value(default_message)
        .prompt()
        .map_err(Error::from)
}

/// Select an input from a list
pub fn select<T: Display>(
    desc: &str,
    options: Vec<T>,
    mock_select: Option<usize>,
) -> Result<T, Error> {
    if cfg!(test) {
        if let Some(index) = mock_select {
            Ok(options
                .into_iter()
                .nth(index)
                .expect("Must provide a vector of options"))
        } else {
            panic!("Must set mock_select in config")
        }
    } else {
        Select::new(desc, options)
            .with_page_size(page_size() / 2) //Fixing bug with page size
            .prompt()
            .map_err(Error::from)
    }
}

/// Select an input from a list
pub fn multi_select<T: Display>(
    desc: &str,
    options: Vec<T>,
    mock_select: Option<usize>,
) -> Result<Vec<T>, Error> {
    if cfg!(test) {
        if let Some(index) = mock_select {
            let value = options
                .into_iter()
                .nth(index)
                .expect("Must provide a vector of options");
            Ok(vec![value])
        } else {
            panic!("Must set mock_select in config")
        }
    } else {
        MultiSelect::new(desc, options)
            .with_page_size(page_size() / 2) //Fixing bug with page size
            .prompt()
            .map_err(Error::from)
    }
}

/// Gets the desired number of visible options for select menu and adjusts size
pub fn page_size() -> usize {
    match terminal_size() {
        Some((Width(_), Height(height))) if height >= 6 => (height - 3).into(),
        // We don't want less than 3 options
        Some(_) => 3,
        None => 7,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn can_select() {
        let result = select("type", vec!["there", "are", "words"], Some(0));
        let expected = Ok("there");
        assert_eq!(result, expected);

        let result = select("type", vec!["there", "are", "words"], Some(1));
        let expected = Ok("are");
        assert_eq!(result, expected);
    }
}
