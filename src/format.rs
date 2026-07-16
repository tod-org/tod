use std::borrow::Cow;

use colored::{ColoredString, Colorize};
use supports_hyperlinks::Stream;

use crate::{config::Config, regexes};

fn apply_color(str: &str, color: fn(String) -> ColoredString) -> String {
    if cfg!(test) {
        return str.to_string();
    }

    color(str.to_string()).to_string()
}

pub fn green_string(str: &str) -> String {
    apply_color(str, |s| s.green())
}

pub fn red_string(str: &str) -> String {
    apply_color(str, |s| s.red())
}

pub fn cyan_string(str: &str) -> String {
    apply_color(str, |s| s.bright_cyan())
}

pub fn purple_string(str: &str) -> String {
    apply_color(str, |s| s.purple())
}

pub fn blue_string(str: &str) -> String {
    apply_color(str, |s| s.blue())
}

pub fn yellow_string(str: &str) -> String {
    apply_color(str, |s| s.yellow())
}

pub fn debug_string(str: &str) -> String {
    apply_color(str, |s| s.bright_blue().on_yellow())
}

pub fn normal_string(str: &str) -> String {
    String::from(str).normal().to_string()
}

pub fn hyperlinks_disabled(config: &Config) -> bool {
    config.disable_links || !supports_hyperlinks::on(Stream::Stdout)
}

/// Formats a URL and display text as an OSC8 hyperlink sequence.
pub(crate) fn format_osc8_link(url: &str, text: &str) -> String {
    format!("\x1B]8;;{url}\x07{text}\x1B]8;;\x07")
}

/// Converts Markdown links to OSC8 hyperlinks showing only the link text.
pub(crate) fn create_links(content: &str) -> String {
    regexes::MARKDOWN_LINK
        .replace_all(content, |caps: &regex::Captures| {
            let text = &caps[1];
            let url = &caps[2];
            Cow::from(format_osc8_link(url, text))
        })
        .into_owned()
}

/// Formats a URL as an OSC8 hyperlink when hyperlinks are enabled.
pub fn maybe_format_url(url: &str, config: &Config) -> String {
    if hyperlinks_disabled(config) {
        return url.to_string();
    }
    format_osc8_link(url, url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blue_string() {
        assert_eq!(blue_string("TEST"), "TEST");
    }

    #[test]
    fn test_purple_string() {
        assert_eq!(purple_string("TEST"), "TEST");
    }

    #[test]
    fn test_green_string() {
        assert_eq!(green_string("OK"), "OK");
    }

    #[test]
    fn test_red_string() {
        assert_eq!(red_string("ERR"), "ERR");
    }

    #[test]
    fn test_cyan_string() {
        assert_eq!(cyan_string("INFO"), "INFO");
    }

    #[test]
    fn test_yellow_string() {
        assert_eq!(yellow_string("WARN"), "WARN");
    }

    #[test]
    fn test_debug_string() {
        assert_eq!(debug_string("DBG"), "DBG");
    }

    #[test]
    fn test_normal_string() {
        assert!(normal_string("plain").contains("plain"));
    }

    #[test]
    fn test_format_osc8_link() {
        assert_eq!(
            format_osc8_link("https://example.com", "https://example.com"),
            "\x1B]8;;https://example.com\x07https://example.com\x1B]8;;\x07"
        );
        assert_eq!(
            format_osc8_link("https://example.com", "[link]"),
            "\x1B]8;;https://example.com\x07[link]\x1B]8;;\x07"
        );
    }

    #[test]
    fn test_create_links() {
        assert_eq!(create_links("hello"), "hello");
        assert_eq!(
            create_links("This is text [Google](https://www.google.com/)"),
            "This is text \x1b]8;;https://www.google.com/\x07Google\x1b]8;;\x07"
        );
    }

    #[test]
    fn test_create_links_multiple_and_edge_cases() {
        let input = "Links: [Rust](https://www.rust-lang.org/) and [GitHub](https://github.com/)";
        let expected = "Links: \x1b]8;;https://www.rust-lang.org/\x07Rust\x1b]8;;\x07 and \x1b]8;;https://github.com/\x07GitHub\x1b]8;;\x07";
        assert_eq!(create_links(input), expected);

        let input = "Check this out: [Example](https://example.com)";
        let expected = "Check this out: \x1b]8;;https://example.com\x07Example\x1b]8;;\x07";
        assert_eq!(create_links(input), expected);
        assert_eq!(create_links("No links here."), "No links here.");
        assert_eq!(
            create_links("[Broken link](not a url"),
            "[Broken link](not a url"
        );
    }

    #[test]
    fn test_format_url_hyperlinks_enabled() {
        if !supports_hyperlinks::on(Stream::Stdout) {
            eprintln!("Skipping test: hyperlinks not supported in this environment");
            return;
        }

        assert_eq!(
            maybe_format_url("https://www.rust-lang.org/", &Config::default()),
            "\x1B]8;;https://www.rust-lang.org/\x07https://www.rust-lang.org/\x1B]8;;\x07"
        );
    }

    #[test]
    fn test_format_url_hyperlinks_disabled() {
        let mut config = Config::default();
        config.disable_links = true;
        let url = "https://www.rust-lang.org/";

        assert_eq!(maybe_format_url(url, &config), url);
    }
}
