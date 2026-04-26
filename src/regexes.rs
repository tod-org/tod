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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_date_regex_matches_valid() {
        assert!(DATE_REGEX.is_match("2024-01-15"));
        assert!(DATE_REGEX.is_match("2000-12-31"));
    }

    #[test]
    fn test_date_regex_rejects_invalid() {
        assert!(!DATE_REGEX.is_match("24-01-15"));
        assert!(!DATE_REGEX.is_match("2024-1-5"));
        assert!(!DATE_REGEX.is_match("2024-01-15T10:00:00"));
        assert!(!DATE_REGEX.is_match("not-a-date"));
        assert!(!DATE_REGEX.is_match(""));
    }

    #[test]
    fn test_datetime_regex_matches_valid() {
        assert!(DATETIME_REGEX.is_match("2024-01-15 10:30"));
        assert!(DATETIME_REGEX.is_match("2000-12-31 23:59"));
    }

    #[test]
    fn test_datetime_regex_rejects_invalid() {
        assert!(!DATETIME_REGEX.is_match("2024-01-15"));
        assert!(!DATETIME_REGEX.is_match("2024-01-15 10:30:00"));
        assert!(!DATETIME_REGEX.is_match("24-01-15 10:30"));
        assert!(!DATETIME_REGEX.is_match("not-a-datetime"));
        assert!(!DATETIME_REGEX.is_match(""));
    }

    #[test]
    fn test_markdown_link_matches_valid() {
        let text = "[Google](https://google.com)";
        let caps = MARKDOWN_LINK
            .captures(text)
            .expect("should match markdown link");
        assert_eq!(&caps[1], "Google");
        assert_eq!(&caps[2], "https://google.com");
    }

    #[test]
    fn test_markdown_link_finds_multiple() {
        let text = "[A](http://a.com) and [B](http://b.com)";
        let matches: Vec<_> = MARKDOWN_LINK.find_iter(text).collect();
        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn test_markdown_link_no_match() {
        assert!(!MARKDOWN_LINK.is_match("plain text"));
        assert!(!MARKDOWN_LINK.is_match("[unclosed](url"));
    }
}
