use std::borrow::Cow;
use supports_hyperlinks::Stream;

use super::{DateTimeInfo, Duration, Task, Unit, priority};
use crate::{
    color, comments::Comment, config::Config, errors::Error, projects::Project, regexes, time,
};

pub fn content(task: &Task, config: &Config) -> String {
    let content = match task.priority {
        priority::Priority::Low => color::blue_string(&task.content),
        priority::Priority::Medium => color::yellow_string(&task.content),
        priority::Priority::High => color::red_string(&task.content),
        priority::Priority::None => color::normal_string(&task.content),
    };

    if hyperlinks_disabled(config) {
        content
    } else {
        create_links(&content)
    }
}

pub async fn project(task: &Task, config: &Config, buffer: &str) -> Result<String, Error> {
    let project_icon = color::purple_string("#");
    let maybe_project = config
        .projects()
        .await?
        .into_iter()
        .filter(|p| p.id == task.project_id)
        .collect::<Vec<Project>>();

    let text = match maybe_project.first() {
        Some(Project { name, .. }) => format!("\n{buffer}{project_icon} {name}"),
        None => {
            let command = color::cyan_string("tod project import --auto");
            format!(
                "\n{buffer}{project_icon} Project not in config\nUse {command} to import missing projects"
            )
        }
    };
    Ok(text)
}

pub fn hyperlinks_disabled(config: &Config) -> bool {
    config.disable_links || !supports_hyperlinks::on(Stream::Stdout)
}

pub fn labels(task: &Task) -> String {
    format!(" {} {}", color::purple_string("@"), task.labels.join(" "))
}

pub fn due(task: &Task, config: &Config, buffer: &str) -> String {
    let due_icon = color::purple_string("!");
    let recurring_icon = color::purple_string("↻");

    match &task.datetimeinfo(config) {
        Ok(DateTimeInfo::Date {
            date,
            is_recurring,
            string,
        }) => {
            let recurring_icon = if *is_recurring {
                format!(" {recurring_icon} {string}")
            } else {
                String::new()
            };
            let date_string = time::date_to_string(date, config).unwrap_or_default();

            format!("\n{buffer}{due_icon} {date_string}{recurring_icon}")
        }
        Ok(DateTimeInfo::DateTime {
            datetime,
            is_recurring,
            string,
        }) => {
            let recurring_icon = if *is_recurring {
                format!(" {recurring_icon} {string}")
            } else {
                String::new()
            };
            let datetime_string = time::datetime_to_string(datetime, config).unwrap_or_default();

            let duration_string = match task.duration {
                None => String::new(),
                Some(Duration {
                    amount: 1,
                    unit: Unit::Day,
                }) => " for 1 day".into(),
                Some(Duration {
                    amount,
                    unit: Unit::Day,
                }) => format!(" for {amount} days"),
                Some(Duration {
                    amount,
                    unit: Unit::Minute,
                }) => format!(" for {amount} min"),
            };

            format!("\n{buffer}{due_icon} {datetime_string}{duration_string}{recurring_icon}")
        }
        Ok(DateTimeInfo::NoDateTime) => String::new(),
        Err(e) => e.to_string(),
    }
}

// Formats a string for all style/formatted links (including markdown) and formats them as a hyperlink
fn create_links(content: &str) -> String {
    let result = regexes::MARKDOWN_LINK.replace_all(content, |caps: &regex::Captures| {
        let text = &caps[1];
        let url = &caps[2];
        Cow::from(format!("\x1b]8;;{url}\x07[{text}]\x1b]8;;\x07"))
    });

    result.into_owned()
}

// Formats a single URL as a hyperlinked URL (with the URL as the Hyperlink), if hyperlinks are enabled in the config - If hyperlinks are disabled, it returns the same URL as a plain string.
pub fn maybe_format_url(url: &str, config: &Config) -> String {
    if hyperlinks_disabled(config) {
        return url.to_string();
    }
    format!("\x1B]8;;{url}\x07[{url}]\x1B]8;;\x07")
}
pub fn number_comments(quantity: usize) -> String {
    let comment_icon = color::purple_string("★");
    if quantity == 1 {
        return format!("\n{comment_icon} 1 comment");
    }

    format!("\n{comment_icon} {quantity} comments")
}
/// Returns a hyperlink-formatted URL formatted as "[link]" for a given task ID if hyperlinks are enabled in the config.
pub fn maybe_format_task_id(task_id: &str, config: &Config) -> String {
    let url = format!("https://app.todoist.com/app/task/{task_id}");
    if hyperlinks_disabled(config) {
        url
    } else {
        format!("\x1B]8;;{url}\x07[link]\x1B]8;;\x07")
    }
}

pub async fn render_comments(config: &Config, comments: Vec<Comment>) -> Result<String, Error> {
    let comment_icon = color::purple_string("★");
    let mut comments = comments
        .iter()
        .map(|c| {
            c.fmt(config)
                .unwrap_or_else(|e| format!("Failed to render comment: {e:?}"))
        })
        .collect::<Vec<String>>();
    // Latest comment first
    comments.reverse();
    let comments = comments.join("\n\n");
    let mut formatted_string = format!("\n\n{comment_icon} Comments {comment_icon}\n\n{comments}");
    let max_comment_length: usize = config.max_comment_length().try_into()?;

    if formatted_string.len() > max_comment_length {
        formatted_string.truncate(max_comment_length);
        formatted_string.push_str("...");
    };

    Ok(formatted_string)
}

#[cfg(test)]
mod tests {
    use crate::test;
    use crate::test::responses::ResponseFromFile;

    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_create_links() {
        assert_eq!(create_links("hello"), String::from("hello"));
        assert_eq!(
            create_links("This is text [Google](https://www.google.com/)"),
            String::from("This is text \x1b]8;;https://www.google.com/\x07[Google]\x1b]8;;\x07")
        );
    }

    #[test]
    fn test_task_url_enabled() {
        let config = Config::default();
        // Skip the test if hyperlinks are not supported in this environment (otherwise test fails)
        if !supports_hyperlinks::on(Stream::Stdout) {
            eprintln!("Skipping test: hyperlinks not supported in this environment");
            return;
        }
        assert_eq!(
            maybe_format_task_id("1", &config),
            String::from("\x1B]8;;https://app.todoist.com/app/task/1\x1B\\[link]\x1B]8;;\x1B\\")
        );
    }

    #[test]
    fn test_task_url_disabled() {
        let mut config = Config::default();
        config.disable_links = true;
        assert_eq!(
            maybe_format_task_id("1", &config),
            String::from("https://app.todoist.com/app/task/1")
        );
    }

    #[tokio::test]
    async fn test_comments() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock(
                "GET",
                "/api/v1/comments/?task_id=6Xqhv4cwxgjwG9w8&limit=200",
            )
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::CommentsAllTypes.read().await)
            .create_async()
            .await;

        let config = test::fixtures::config().await.with_mock_url(server.url());

        let comments = vec![test::fixtures::comment()];
        let comments = render_comments(&config, comments)
            .await
            .expect("expected value or result, got None or Err");

        assert_matches!(
            comments.as_str(),
            "\n\n★ Comments ★\n\nPosted 2016-09-22 00:00:00 PDT\nNeed one bottle of milk"
        );
        mock.expect(1);
    }

    #[test]
    fn test_create_links_multiple_and_edge_cases() {
        // Multiple links in one string
        let input = "Links: [Rust](https://www.rust-lang.org/) and [GitHub](https://github.com/)";
        let expected = "Links: \x1b]8;;https://www.rust-lang.org/\x07[Rust]\x1b]8;;\x07 and \x1b]8;;https://github.com/\x07[GitHub]\x1b]8;;\x07";
        assert_eq!(create_links(input), expected);

        // Single link
        let input = "Check this out: [Example](https://example.com)";
        let expected = "Check this out: \x1b]8;;https://example.com\x07[Example]\x1b]8;;\x07";
        assert_eq!(create_links(input), expected);

        // No links present
        assert_eq!(create_links("No links here."), "No links here.");

        // Malformed markdown (should not match)
        assert_eq!(
            create_links("[Broken link](not a url"),
            "[Broken link](not a url"
        );
    }

    #[tokio::test]
    async fn test_format_url_hyperlinks_enabled() {
        let url = "https://www.rust-lang.org/";
        let expected =
            "\x1B]8;;https://www.rust-lang.org/\x1B\\[https://www.rust-lang.org/]\x1B]8;;\x1B\\";
        let config = Config::default();
        // Skip the test if hyperlinks are not supported in the current environment (otherwise test fails)
        if !supports_hyperlinks::on(Stream::Stdout) {
            eprintln!("Skipping test: hyperlinks not supported in this environment");
            return;
        }
        assert_eq!(maybe_format_url(url, &config), expected);
    }
    #[test]
    fn test_format_url_hyperlinks_disabled() {
        let url = "https://www.rust-lang.org/";
        // Create a config with disable_links set to true
        let mut config = Config::default();
        config.disable_links = true;
        assert_eq!(maybe_format_url(url, &config), url);
    }
}
