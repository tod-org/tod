use crate::errors::Error;
use inquire::{Confirm, CustomType, DateSelect, MultiSelect, Select, Text};
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
pub const DATE_AND_TIME: &str = "Set a date and time in natural language";
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
pub const REMIND: &str = "Remind";
pub const TIMEBOX: &str = "Timebox";
pub const COMMENT: &str = "Comment";
pub const SKIP: &str = "Skip";
pub const DELETE: &str = "Delete";
pub const CANCEL: &str = "Cancel";
pub const QUIT: &str = "Quit";
pub const SCHEDULE: &str = "Schedule";

#[derive(Debug, PartialEq)]
pub enum DateTimeInput {
    Skip,
    None,
    Complete,
    Text(String),
}

/// Get datetime input from user
/// `skip_or_delete` enables the skip and delete options
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
                    "none" | "n" => Ok(DateTimeInput::None),
                    "complete" | "c" => Ok(DateTimeInput::Complete),
                    "skip" | "s" => Ok(DateTimeInput::Skip),
                    _ => Ok(DateTimeInput::Text(entry)),
                }
            } else {
                let entry = string(
                    "Enter datetime in natural language, or none (n)",
                    mock_string,
                )?;

                match entry.as_str() {
                    "none" | "n" => Ok(DateTimeInput::None),
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

/// Get confirmation from user
pub fn confirm(desc: &str) -> Result<bool, Error> {
    Confirm::new(desc)
        .with_default(false)
        .prompt()
        .map_err(Into::into)
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

/// Get number input with default value
pub fn number_with_default(desc: &str, default_message: usize) -> Result<usize, Error> {
    if cfg!(test) {
        return Ok(default_message);
    }
    CustomType::<usize>::new(desc)
        .with_error_message("Please type a valid number")
        .with_starting_input(&default_message.to_string())
        .prompt()
        .map_err(Error::from)
}

pub fn bool(desc: &str, default_value: bool, mock_select: Option<usize>) -> Result<bool, Error> {
    let options = vec![true, false];
    let cursor_index = usize::from(!default_value);
    select_with_cursor_index(desc, options, cursor_index, mock_select)
}

/// Select an input from a list
pub fn select<T: Display>(
    desc: &str,
    options: Vec<T>,
    mock_select: Option<usize>,
) -> Result<T, Error> {
    select_with_cursor_index(desc, options, 0, mock_select)
}

/// Select an input from a list, with a cursor index
pub fn select_with_cursor_index<T: Display>(
    desc: &str,
    options: Vec<T>,
    cursor_index: usize,
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
            .with_starting_cursor(cursor_index)
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

    #[test]
    fn datetime_natural_language_only_returns_text() {
        let result = datetime(
            None,
            Some("tomorrow at 3pm".into()),
            Some(true),
            false,
            false,
        );
        assert_eq!(result, Ok(DateTimeInput::Text("tomorrow at 3pm".into())));
    }

    #[test]
    fn datetime_no_natural_language_skip_complete_select_no_date() {
        let result = datetime(Some(1), None, None, true, true);
        assert_eq!(result, Ok(DateTimeInput::None));
    }

    #[test]
    fn datetime_no_natural_language_skip_complete_select_skip() {
        let result = datetime(Some(2), None, None, true, true);
        assert_eq!(result, Ok(DateTimeInput::Skip));
    }

    #[test]
    fn datetime_no_natural_language_skip_complete_select_complete() {
        let result = datetime(Some(3), None, None, true, true);
        assert_eq!(result, Ok(DateTimeInput::Complete));
    }

    #[test]
    fn datetime_nat_lang_with_skip_complete_enter_none() {
        let result = datetime(Some(1), Some("none".into()), None, false, true);
        assert_eq!(result, Ok(DateTimeInput::None));
    }

    #[test]
    fn datetime_nat_lang_with_skip_complete_enter_skip() {
        let result = datetime(Some(1), Some("skip".into()), None, false, true);
        assert_eq!(result, Ok(DateTimeInput::Skip));
    }

    #[test]
    fn datetime_nat_lang_with_skip_complete_enter_complete() {
        let result = datetime(Some(1), Some("complete".into()), None, false, true);
        assert_eq!(result, Ok(DateTimeInput::Complete));
    }

    #[test]
    fn datetime_nat_lang_with_skip_complete_enter_free_text() {
        let result = datetime(Some(1), Some("next Monday".into()), None, false, true);
        assert_eq!(result, Ok(DateTimeInput::Text("next Monday".into())));
    }

    #[test]
    fn datetime_nat_lang_without_skip_complete_enter_none() {
        let result = datetime(Some(1), Some("none".into()), None, false, false);
        assert_eq!(result, Ok(DateTimeInput::None));
    }

    #[test]
    fn datetime_nat_lang_without_skip_complete_enter_short_n() {
        let result = datetime(Some(1), Some("n".into()), None, false, false);
        assert_eq!(result, Ok(DateTimeInput::None));
    }

    #[test]
    fn datetime_nat_lang_without_skip_complete_enter_free_text() {
        let result = datetime(Some(1), Some("Friday".into()), None, false, false);
        assert_eq!(result, Ok(DateTimeInput::Text("Friday".into())));
    }

    #[test]
    fn datetime_select_no_date_from_default_options() {
        let result = datetime(Some(2), None, None, false, false);
        assert_eq!(result, Ok(DateTimeInput::None));
    }

    #[test]
    fn datetime_without_natural_language_and_without_skip_complete_matches_else_branch() {
        // no_natural_language=false, skip_or_complete=false
        // With the correct code, the else branch fires with options:
        // [SELECT_DATE(0), NAT_LANG(1), NO_DATE(2)]
        // mock_select=2 selects NO_DATE.
        let result = datetime(Some(2), None, None, false, false);
        assert_eq!(result, Ok(DateTimeInput::None));
    }

    #[test]
    fn bool_select_returns_false_when_cursor_starts_on_true() {
        // default_value=true → cursor starts on false (index 1)
        // mock_select=0 overrides and picks the first option (true)
        let result = bool("test", true, Some(0));
        assert_eq!(result, Ok(true));
    }

    #[test]
    fn bool_select_returns_true_when_cursor_starts_on_false() {
        // default_value=false → cursor starts on true (index 0)
        // mock_select=1 picks the second option (false)
        let result = bool("test", false, Some(1));
        assert_eq!(result, Ok(false));
    }
}
