use colored::{ColoredString, Colorize};
use linkify::{LinkFinder, LinkKind};
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

/// Converts Markdown links and bare HTTP URLs to OSC8 hyperlinks.
pub(crate) fn create_links(content: &str) -> String {
    let mut formatted = String::with_capacity(content.len());
    let mut previous_end = 0;

    for captures in regexes::MARKDOWN_LINK.captures_iter(content) {
        let (Some(markdown), Some(text), Some(url)) =
            (captures.get(0), captures.get(1), captures.get(2))
        else {
            continue;
        };

        formatted.push_str(&linkify_urls(&content[previous_end..markdown.start()]));
        formatted.push_str(&format_osc8_link(url.as_str(), text.as_str()));
        previous_end = markdown.end();
    }

    formatted.push_str(&linkify_urls(&content[previous_end..]));
    formatted
}

pub(crate) fn maybe_format_text(content: &str, config: &Config) -> String {
    if hyperlinks_disabled(config) {
        return content.to_string();
    }

    create_links(content)
}

fn linkify_urls(content: &str) -> String {
    let mut finder = LinkFinder::new();
    finder.kinds(&[LinkKind::Url]);
    finder.url_must_have_scheme(true);

    let mut formatted = String::with_capacity(content.len());
    for span in finder.spans(content) {
        let text = span.as_str();
        if span.kind() == Some(&LinkKind::Url) && is_http_url(text) {
            formatted.push_str(&format_osc8_link(text, text));
        } else {
            formatted.push_str(text);
        }
    }
    formatted
}

fn is_http_url(url: &str) -> bool {
    url.get(..7)
        .is_some_and(|scheme| scheme.eq_ignore_ascii_case("http://"))
        || url
            .get(..8)
            .is_some_and(|scheme| scheme.eq_ignore_ascii_case("https://"))
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
    fn test_create_links_linkifies_bare_url() {
        let url = "https://getpocket.com/explore/item/here-are-the-4-simple-introspection-steps-that-will-boost-self-awareness?utm_source=pocket-newtab";
        let title = "Here Are The 4 Simple Introspection Steps That Will Boost Self Awareness - Nir Eyal - Pocket";
        let input = format!("@Review {url} ({title})");
        let expected = format!("@Review {} ({title})", format_osc8_link(url, url));

        assert_eq!(create_links(&input), expected);
    }

    #[test]
    fn test_create_links_supports_brackets_in_markdown_label() {
        let url =
            "https://learndobecome.com/podcast-9-how-to-get-your-projects-past-the-finish-line/";
        let label = "[PODCAST 9]: How to Get Your Projects Past the Finish Line";
        let input = format!("[{label}]({url})");

        assert_eq!(create_links(&input), format_osc8_link(url, label));
    }

    #[test]
    fn test_create_links_does_not_double_link_markdown_url() {
        let markdown_url = "https://example.com/article";
        let bare_url = "https://example.com/source";
        let input = format!("[Article]({markdown_url}) via {bare_url}");
        let expected = format!(
            "{} via {}",
            format_osc8_link(markdown_url, "Article"),
            format_osc8_link(bare_url, bare_url)
        );

        assert_eq!(create_links(&input), expected);
    }

    #[test]
    fn test_create_links_preserves_unsupported_schemes() {
        assert_eq!(
            create_links("Download ftp://example.com/file"),
            "Download ftp://example.com/file"
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
