//! Terminal input prompts (text, select, confirm, datetime) with test mock support.

use crate::errors::Error;
use inquire::{Confirm, CustomType, DateSelect, MultiSelect, Select, Text};
use std::fmt::Display;
use std::sync::Mutex;
use terminal_size::{Height, Width, terminal_size};

// These constants are used throughout the app

// Set

/// Prompt label for task content.
pub const CONTENT: &str = "Set content";
/// Prompt label for task description.
pub const DESCRIPTION: &str = "Set description";
/// Prompt label for project name.
pub const NAME: &str = "Set name";
/// Prompt label for Todoist filter.
pub const FILTER: &str = "Set filter";
/// Prompt label for file path.
pub const PATH: &str = "Set path";
/// Prompt label for due date.
pub const DATE: &str = "Set a due date";
/// Prompt label for time.
pub const TIME: &str = "Set time, i.e. 3pm or 1500";
/// Prompt label for date and time in natural language.
pub const DATE_AND_TIME: &str = "Set a date and time in natural language";
/// Prompt label for duration in minutes.
pub const DURATION: &str = "Set duration in minutes";

// Select

/// Prompt label for attribute selection.
pub const ATTRIBUTES: &str = "Select attributes";
/// Prompt label for project selection.
pub const PROJECT: &str = "Select a project";
/// Prompt label for label selection.
pub const LABELS: &str = "Select labels";
/// Prompt label for single label selection.
pub const LABEL: &str = "Select a label";
/// Prompt label for section selection.
pub const SECTION: &str = "Select section";
/// Prompt label for priority selection.
pub const PRIORITY: &str = "Select priority";
/// Prompt label for option selection.
pub const OPTION: &str = "Select an option";
/// Prompt label for date selection.
pub const SELECT_DATE: &str = "Select a date";
/// Prompt label for task selection.
pub const TASK: &str = "Select a task";

// Options

/// Option: use natural language input.
pub const NAT_LANG: &str = "Natural Language";
/// Option: clear the date.
pub const NO_DATE: &str = "No Date";
/// Option: complete the task.
pub const COMPLETE: &str = "Complete";
/// Option: add a reminder.
pub const REMIND: &str = "Remind";
/// Option: assign a duration.
pub const TIMEBOX: &str = "Timebox";
/// Option: add a comment.
pub const COMMENT: &str = "Comment";
/// Option: skip this task.
pub const SKIP: &str = "Skip";
/// Option: delete the task.
pub const DELETE: &str = "Delete";
/// Option: cancel the operation.
pub const CANCEL: &str = "Cancel";
/// Option: quit processing.
pub const QUIT: &str = "Quit";
/// Option: schedule the task.
pub const SCHEDULE: &str = "Schedule";

/// Natural language date and time input.
#[derive(Debug, PartialEq)]
pub enum DateTimeInput {
    /// Skip this task.
    Skip,
    /// Clear the date.
    None,
    /// Complete the task.
    Complete,
    /// Natural language date string.
    Text(String),
}

/// Get datetime input from user.
/// `skip_or_complete` enables the skip and complete options;
/// it is generally true when processing tasks.
pub fn datetime(
    mock_selects: &Mutex<Vec<usize>>,
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
        select(description, options, mock_selects)?
    } else if !no_natural_language && skip_or_complete {
        let options = vec![SELECT_DATE, NAT_LANG, NO_DATE, SKIP, COMPLETE];
        let description = DATE;
        select(description, options, mock_selects)?
    } else {
        let options = vec![SELECT_DATE, NAT_LANG, NO_DATE];
        let description = DATE;
        select(description, options, mock_selects)?
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

/// Prompts the user for a date via date picker.
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

/// Prompts the user for a boolean value.
pub fn bool(desc: &str, default_value: bool, mock_selects: &Mutex<Vec<usize>>) -> Result<bool, Error> {
    let options = vec![true, false];
    let cursor_index = usize::from(!default_value);
    select_with_cursor_index(desc, options, cursor_index, mock_selects)
}

/// Select an input from a list
pub fn select<T: Display>(
    desc: &str,
    options: Vec<T>,
    mock_selects: &Mutex<Vec<usize>>,
) -> Result<T, Error> {
    select_with_cursor_index(desc, options, 0, mock_selects)
}

/// Select an input from a list, with a cursor index
pub fn select_with_cursor_index<T: Display>(
    desc: &str,
    options: Vec<T>,
    cursor_index: usize,
    mock_selects: &Mutex<Vec<usize>>,
) -> Result<T, Error> {
    if cfg!(test) {
        let mut selects = mock_selects.lock().unwrap();
        let index = if selects.len() <= 1 {
            // Singleton or empty: peek (backward-compatible, never consumes)
            *selects.first().unwrap_or(&0)
        } else {
            // Multiple values: consume sequentially
            selects.remove(0)
        };
        Ok(options
            .into_iter()
            .nth(index)
            .expect("Must provide a vector of options"))
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
    mock_selects: &Mutex<Vec<usize>>,
) -> Result<Vec<T>, Error> {
    if cfg!(test) {
        let mut selects = mock_selects.lock().unwrap();
        let index = if selects.len() <= 1 {
            *selects.first().unwrap_or(&0)
        } else {
            selects.remove(0)
        };
        let value = options
            .into_iter()
            .nth(index)
            .expect("Must provide a vector of options");
        Ok(vec![value])
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
    use std::sync::Arc;

    #[test]
    fn can_select() {
        let result = select("type", vec!["there", "are", "words"], &Arc::new(Mutex::new(vec![0])));
        let expected = Ok("there");
        assert_eq!(result, expected);

        let result = select("type", vec!["there", "are", "words"], &Arc::new(Mutex::new(vec![1])));
        let expected = Ok("are");
        assert_eq!(result, expected);
    }

    #[test]
    fn datetime_natural_language_only_returns_text() {
        let result = datetime(
            &Arc::new(Mutex::new(Vec::new())),
            Some("tomorrow at 3pm".into()),
            Some(true),
            false,
            false,
        );
        assert_eq!(result, Ok(DateTimeInput::Text("tomorrow at 3pm".into())));
    }

    #[test]
    fn datetime_no_natural_language_skip_complete_select_no_date() {
        let result = datetime(&Arc::new(Mutex::new(vec![1])), None, None, true, true);
        assert_eq!(result, Ok(DateTimeInput::None));
    }

    #[test]
    fn datetime_no_natural_language_skip_complete_select_skip() {
        let result = datetime(&Arc::new(Mutex::new(vec![2])), None, None, true, true);
        assert_eq!(result, Ok(DateTimeInput::Skip));
    }

    #[test]
    fn datetime_no_natural_language_skip_complete_select_complete() {
        let result = datetime(&Arc::new(Mutex::new(vec![3])), None, None, true, true);
        assert_eq!(result, Ok(DateTimeInput::Complete));
    }

    #[test]
    fn datetime_nat_lang_with_skip_complete_enter_none() {
        let result = datetime(&Arc::new(Mutex::new(vec![1])), Some("none".into()), None, false, true);
        assert_eq!(result, Ok(DateTimeInput::None));
    }

    #[test]
    fn datetime_nat_lang_with_skip_complete_enter_skip() {
        let result = datetime(&Arc::new(Mutex::new(vec![1])), Some("skip".into()), None, false, true);
        assert_eq!(result, Ok(DateTimeInput::Skip));
    }

    #[test]
    fn datetime_nat_lang_with_skip_complete_enter_complete() {
        let result = datetime(&Arc::new(Mutex::new(vec![1])), Some("complete".into()), None, false, true);
        assert_eq!(result, Ok(DateTimeInput::Complete));
    }

    #[test]
    fn datetime_nat_lang_with_skip_complete_enter_free_text() {
        let result = datetime(&Arc::new(Mutex::new(vec![1])), Some("next Monday".into()), None, false, true);
        assert_eq!(result, Ok(DateTimeInput::Text("next Monday".into())));
    }

    #[test]
    fn datetime_nat_lang_without_skip_complete_enter_none() {
        let result = datetime(&Arc::new(Mutex::new(vec![1])), Some("none".into()), None, false, false);
        assert_eq!(result, Ok(DateTimeInput::None));
    }

    #[test]
    fn datetime_nat_lang_without_skip_complete_enter_short_n() {
        let result = datetime(&Arc::new(Mutex::new(vec![1])), Some("n".into()), None, false, false);
        assert_eq!(result, Ok(DateTimeInput::None));
    }

    #[test]
    fn datetime_nat_lang_without_skip_complete_enter_free_text() {
        let result = datetime(&Arc::new(Mutex::new(vec![1])), Some("Friday".into()), None, false, false);
        assert_eq!(result, Ok(DateTimeInput::Text("Friday".into())));
    }

    #[test]
    fn datetime_select_no_date_from_default_options() {
        let result = datetime(&Arc::new(Mutex::new(vec![2])), None, None, false, false);
        assert_eq!(result, Ok(DateTimeInput::None));
    }

    #[test]
    fn datetime_without_natural_language_and_without_skip_complete_matches_else_branch() {
        // no_natural_language=false, skip_or_complete=false
        // With the correct code, the else branch fires with options:
        // [SELECT_DATE(0), NAT_LANG(1), NO_DATE(2)]
        // mock_select=2 selects NO_DATE.
        let result = datetime(&Arc::new(Mutex::new(vec![2])), None, None, false, false);
        assert_eq!(result, Ok(DateTimeInput::None));
    }

    #[test]
    fn bool_select_returns_false_when_cursor_starts_on_true() {
        // default_value=true → cursor starts on false (index 1)
        // mock_select=0 overrides and picks the first option (true)
        let result = bool("test", true, &Arc::new(Mutex::new(vec![0])));
        assert_eq!(result, Ok(true));
    }

    #[test]
    fn bool_select_returns_true_when_cursor_starts_on_false() {
        // default_value=false → cursor starts on true (index 0)
        // mock_select=1 picks the second option (false)
        let result = bool("test", false, &Arc::new(Mutex::new(vec![1])));
        assert_eq!(result, Ok(false));
    }

    #[test]
    fn datetime_no_natural_language_without_skip_complete_uses_else_branch() {
        // no_natural_language=true, skip_or_complete=false
        // Must not match no_natural_language && skip_or_complete.
        // Goes to else branch with options: [SELECT_DATE(0), NAT_LANG(1), NO_DATE(2)]
        let result = datetime(&Arc::new(Mutex::new(vec![2])), None, None, true, false);
        assert_eq!(result, Ok(DateTimeInput::None));
    }

    #[test]
    #[should_panic(expected = "Must provide a vector of options")]
    fn datetime_no_nat_lang_no_skip_complete_extra_index_panics() {
        // no_natural_language=false, skip_or_complete=false
        // Original (&&): !false && false = false → else (3 options) → mock_select=3 panics
        // Mutant  (||): !false || false = true  → branch (5 options) → mock_select=3 → SKIP
        let _ = datetime(&Arc::new(Mutex::new(vec![3])), None, None, false, false);
    }

    #[test]
    fn string_with_default_returns_default_message() {
        let result = string_with_default("Test prompt", "default value");
        assert_eq!(result, Ok("default value".to_string()));
    }

    #[test]
    fn number_with_default_returns_default_number() {
        let result = number_with_default("Test prompt", 42);
        assert_eq!(result, Ok(42));
    }
}
