use terminal_size::{Width, terminal_size};

use super::{DateTimeInfo, Duration, Task, Unit, priority};
use crate::{comments::Comment, config::Config, errors::Error, format, projects::Project, time};

pub fn content(task: &Task, config: &Config) -> String {
    let content = match task.priority {
        priority::Priority::Low => format::blue_string(&task.content),
        priority::Priority::Medium => format::yellow_string(&task.content),
        priority::Priority::High => format::red_string(&task.content),
        priority::Priority::None => format::normal_string(&task.content),
    };

    if format::hyperlinks_disabled(config) {
        content
    } else {
        format::create_links(&content)
    }
}

pub async fn project(task: &Task, config: &Config, buffer: &str) -> Result<String, Error> {
    let project_icon = format::purple_string("#");
    let maybe_project = config
        .projects()
        .await?
        .into_iter()
        .filter(|p| p.id == task.project_id)
        .collect::<Vec<Project>>();

    let text = if let Some(Project { name, .. }) = maybe_project.first() {
        format!("\n{buffer}{project_icon} {name}")
    } else {
        let command = format::cyan_string("tod project import --auto");
        format!(
            "\n{buffer}{project_icon} Project not in config\nUse {command} to import missing projects"
        )
    };
    Ok(text)
}

pub fn labels(task: &Task) -> String {
    format!(" {} {}", format::purple_string("@"), task.labels.join(" "))
}

pub fn due(task: &Task, config: &Config, buffer: &str) -> String {
    let due_icon = format::purple_string("!");
    let recurring_icon = format::purple_string("↻");

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
            let date_string = time::date_to_string(*date, config).unwrap_or_default();

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

pub fn number_comments(quantity: usize) -> String {
    let comment_icon = format::purple_string("★");
    if quantity == 1 {
        return format!("\n{comment_icon} 1 comment");
    }

    format!("\n{comment_icon} {quantity} comments")
}
/// Returns a hyperlink-formatted URL formatted as "[link]" for a given task ID if hyperlinks are enabled in the config.
pub fn maybe_format_task_id(task_id: &str, config: &Config) -> String {
    let url = format!("https://app.todoist.com/app/task/{task_id}");
    if format::hyperlinks_disabled(config) {
        url
    } else {
        format::format_osc8_link(&url, "[link]")
    }
}

#[allow(clippy::unused_async)]
pub async fn render_comments(config: &Config, comments: Vec<Comment>) -> Result<String, Error> {
    let comment_icon = format::purple_string("★");
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
        formatted_string = truncate_comment_text(
            &formatted_string,
            max_comment_length,
            current_terminal_width(),
        );
    }

    Ok(formatted_string)
}

fn current_terminal_width() -> Option<usize> {
    terminal_size().map(|(Width(width), _)| usize::from(width))
}

fn truncate_comment_text(text: &str, max_length: usize, terminal_width: Option<usize>) -> String {
    let boundary = comment_truncation_boundary(text, max_length, terminal_width);
    let mut truncated = text[..boundary].trim_end_matches('\n').to_string();
    if boundary < text.len() {
        truncated.push_str("...");
    }
    truncated
}

fn comment_truncation_boundary(
    text: &str,
    max_length: usize,
    terminal_width: Option<usize>,
) -> usize {
    if text.len() <= max_length {
        return text.len();
    }

    let start = char_boundary_at_or_after(text, max_length);
    let newline_boundary = text[start..].find('\n').map(|offset| start + offset);
    let window_boundary = terminal_width
        .filter(|width| *width > 0)
        .and_then(|width| next_window_boundary(text, start, width));

    newline_boundary
        .into_iter()
        .chain(window_boundary)
        .min()
        .unwrap_or(start)
}

fn char_boundary_at_or_after(text: &str, index: usize) -> usize {
    if index >= text.len() {
        return text.len();
    }

    if text.is_char_boundary(index) {
        return index;
    }

    text.char_indices()
        .map(|(index, _)| index)
        .find(|boundary| *boundary > index)
        .unwrap_or(text.len())
}

fn next_window_boundary(text: &str, start: usize, terminal_width: usize) -> Option<usize> {
    let char_count = text[..start].chars().count();
    let remainder = char_count % terminal_width;
    let boundary_chars = if remainder == 0 {
        char_count
    } else {
        char_count + terminal_width - remainder
    };

    byte_index_for_char_count(text, boundary_chars)
}

fn byte_index_for_char_count(text: &str, char_count: usize) -> Option<usize> {
    if char_count == 0 {
        return Some(0);
    }

    text.char_indices()
        .nth(char_count)
        .map(|(index, _)| index)
        .or_else(|| (text.chars().count() <= char_count).then_some(text.len()))
}

#[cfg(test)]
mod tests {
    use crate::format;
    use crate::tasks::DateInfo;
    use crate::test;
    use crate::test::responses::ResponseFromFile;

    use super::*;
    use pretty_assertions::assert_eq;
    use supports_hyperlinks::Stream;

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
            String::from("\x1B]8;;https://app.todoist.com/app/task/1\x07[link]\x1B]8;;\x07")
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
    fn test_truncate_comment_text_prefers_next_newline() {
        let text = "0123456789\nnext line";

        assert_eq!(truncate_comment_text(text, 5, Some(80)), "0123456789...");
    }

    #[test]
    fn test_truncate_comment_text_uses_next_window_boundary() {
        let text = "0123456789abcdefghij";

        assert_eq!(truncate_comment_text(text, 6, Some(10)), "0123456789...");
    }

    #[test]
    fn test_truncate_comment_text_is_unicode_safe() {
        let text = "abc✓defgh";

        assert_eq!(truncate_comment_text(text, 4, None), "abc✓...");
    }

    #[test]
    fn test_truncate_comment_text_does_not_mark_untruncated_text() {
        let text = "short comment";

        assert_eq!(truncate_comment_text(text, 80, Some(10)), text);
    }

    #[tokio::test]
    async fn test_content_priority_and_links() {
        let config = test::fixtures::config().await;
        let mut task = test::fixtures::today_task().await;
        task.content = "Test".to_string();

        task.priority = priority::Priority::Low;
        assert_eq!(content(&task, &config), format::blue_string("Test"));

        task.priority = priority::Priority::Medium;
        assert_eq!(content(&task, &config), format::yellow_string("Test"));

        task.priority = priority::Priority::High;
        assert_eq!(content(&task, &config), format::red_string("Test"));

        task.priority = priority::Priority::None;
        assert_eq!(content(&task, &config), format::normal_string("Test"));

        // Hyperlinks disabled
        let mut config_no_links = config.clone();
        config_no_links.disable_links = true;
        assert_eq!(
            content(&task, &config_no_links),
            format::normal_string("Test")
        );
    }

    #[tokio::test]
    async fn test_labels_format() {
        let mut task = test::fixtures::today_task().await;
        task.labels = vec!["foo".to_string(), "bar".to_string()];
        let result = labels(&task);
        assert!(result.contains("@"));
        assert!(result.contains("foo"));
        assert!(result.contains("bar"));
    }

    #[tokio::test]
    async fn test_due_no_date() {
        let config = test::fixtures::config().await;
        let task = Task {
            due: None,
            ..test::fixtures::today_task().await
        };
        assert_eq!(due(&task, &config, ""), "");
    }

    #[test]
    fn test_number_comments() {
        assert!(number_comments(1).contains("1 comment"));
        assert!(number_comments(2).contains("2 comments"));
    }

    #[tokio::test]
    async fn test_project_found_and_not_found() {
        let config = test::fixtures::config().await;
        let fixture_project = test::fixtures::project();

        // Project found
        let task_found = Task {
            project_id: fixture_project.id.clone(),
            ..test::fixtures::today_task().await
        };
        let found = project(&task_found, &config, "  ").await.unwrap();
        assert!(found.contains("myproject"));

        // Project not found
        let task_not_found = Task {
            project_id: "notfound".to_string(),
            ..test::fixtures::today_task().await
        };
        let not_found = project(&task_not_found, &config, "  ").await.unwrap();
        assert!(not_found.contains("Project not in config"));
    }

    #[tokio::test]
    async fn test_due_various_branches() {
        let config = test::fixtures::config().await;
        let base_task = test::fixtures::today_task().await;

        // Date-only due (date string of exactly 10 chars → DateTimeInfo::Date)
        let task_date = Task {
            due: Some(DateInfo {
                date: "2024-01-01".to_string(),
                is_recurring: false,
                string: "every day".to_string(),
                lang: "en".to_string(),
                timezone: None,
            }),
            ..base_task.clone()
        };
        let out = due(&task_date, &config, "");
        assert!(out.contains("!"));

        // Datetime due with duration and recurring flag (→ DateTimeInfo::DateTime)
        let task_datetime = Task {
            due: Some(DateInfo {
                date: "2024-01-01T20:00:00Z".to_string(),
                is_recurring: true,
                string: "every week".to_string(),
                lang: "en".to_string(),
                timezone: None,
            }),
            duration: Some(Duration {
                amount: 2,
                unit: Unit::Day,
            }),
            ..base_task
        };
        let out = due(&task_datetime, &config, "");
        assert!(out.contains("for 2 days"));
        assert!(out.contains("↻"));
    }
}
