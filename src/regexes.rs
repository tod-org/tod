//! Holds all regular expressions
//! uses once_cell to ensure that they are only evaluated once
//!
use once_cell::sync::Lazy;
use regex::Regex;

/// For finding markdown links, first capture group is the text and second is the url
pub static MARKDOWN_LINK: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\[([^\]]+)\]\(([^)]+)\)").expect("invalid markdown link regex pattern")
});

/// Confirms regex pattern YYYY-MM-DD
pub static DATE_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\d{4}-\d{2}-\d{2}$").expect("invalid DATE_REGEX pattern YYYY-MM-DD")
});

/// Confirms regex pattern YYYY-MM-DD HH:MM
pub static DATETIME_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\d{4}-\d{2}-\d{2} \d{2}:\d{2}$")
        .expect("invalid DATETIME_REGEX pattern YYYY-MM-DD HH:MM")
});
