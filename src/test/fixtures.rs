//! Fixtures are for creating structs in test

use crate::comments::Comment;
use crate::config::{self, Config};
use crate::errors::Error;
use crate::labels::Label;
use crate::projects::Project;
use crate::sections::Section;
use crate::tasks::priority::Priority;
use crate::tasks::{DateInfo, Deadline, Duration, Task, Unit};
use crate::test_time::FixedTimeProvider;
use crate::time::{self, FORMAT_DATE};
use chrono::Duration as ChronoDuration;

pub fn label() -> Label {
    Label {
        id: "123".to_string(),
        name: "345".to_string(),
        color: "red".to_string(),
        order: None,
        is_favorite: false,
    }
}

/// Adjust a date forward in time in days, use negative to go backwards in time
async fn adjusted_date(days: i64) -> String {
    let config = config().await.with_timezone("America/Vancouver");
    let base_date =
        time::naive_date_today(&config).expect("expected value or result, got None or Err");
    let adjusted = base_date + ChronoDuration::days(days);
    adjusted.format(FORMAT_DATE).to_string()
}

pub async fn today_task() -> Task {
    let date = adjusted_date(0).await;
    Task {
        id: "6Xqhv4cwxgjwG9w8".into(),
        section_id: None,
        added_by_uid: Some("633166".into()),
        added_at: Some(format!("{date}T22:29:34.404051Z")),
        child_order: 1,
        day_order: -1,
        responsible_uid: None,
        assigned_by_uid: None,
        updated_at: Some(format!("{date}T22:32:46.415849Z")),
        deadline: Some(Deadline {
            lang: "en".to_string(),
            date: date.clone(),
        }),
        completed_at: None,
        is_collapsed: false,
        user_id: "910".into(),
        content: "TEST".into(),
        checked: false,
        duration: Some(Duration {
            amount: 15,
            unit: Unit::Minute,
        }),
        parent_id: None,
        note_count: 0,
        project_id: "6VRRxv8CM6GVmmgf".into(),
        labels: vec!["computer".into()],
        description: "".into(),
        due: Some(DateInfo {
            date: format!("{date}T12:00:00Z"),
            lang: "en".into(),
            is_recurring: false,
            timezone: Some("America/Vancouver".into()),
            string: format!("{date} 15:00"),
        }),
        priority: Priority::Medium,
        is_deleted: false,
    }
}

pub async fn task(days_in_future: i64) -> Task {
    let date = adjusted_date(days_in_future).await;
    Task {
        id: "6Xqhv4cwxgjwG9w8".into(),
        section_id: None,
        added_by_uid: Some("633166".into()),
        added_at: Some(format!("{date}T22:29:34.404051Z")),
        child_order: 1,
        day_order: -1,
        responsible_uid: None,
        assigned_by_uid: None,
        updated_at: Some(format!("{date}T22:32:46.415849Z")),
        deadline: Some(Deadline {
            lang: "en".into(),
            date: date.clone(),
        }),
        completed_at: None,
        is_collapsed: false,
        user_id: "910".into(),
        content: "TEST".into(),
        checked: false,
        duration: Some(Duration {
            amount: 15,
            unit: Unit::Minute,
        }),
        parent_id: None,
        note_count: 0,
        project_id: "6VRRxv8CM6GVmmgf".into(),
        labels: vec!["computer".into()],
        description: "".into(),
        due: Some(DateInfo {
            date: format!("{date}T12:00:00Z"),
            lang: "en".into(),
            is_recurring: false,
            timezone: Some("America/Vancouver".into()),
            string: format!("{date} 15:00"),
        }),
        priority: Priority::Medium,
        is_deleted: false,
    }
}

pub async fn config() -> Config {
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel::<Error>();

    let path = config::generate_path()
        .await
        .expect("expected value or result, got None or Err");
    Config::new(Some(tx), path)
        .await
        .expect("Could not generate directory")
        .with_token("alreadycreated")
        .with_projects(vec![project()])
        .with_time_provider(time::TimeProviderEnum::Fixed(FixedTimeProvider))
        .with_timezone("America/Vancouver")
}

pub fn project() -> Project {
    Project {
        id: "123".to_string(),
        can_assign_tasks: true,
        child_order: 0,
        color: "blue".to_string(),
        created_at: None,
        is_archived: false,
        is_deleted: false,
        is_favorite: false,
        is_frozen: false,
        name: "myproject".to_string(),
        updated_at: None,
        view_style: "List".to_string(),
        default_order: 0,
        description: "Something".to_string(),
        parent_id: None,
        inbox_project: None,
        is_collapsed: false,
        is_shared: false,
    }
}

pub fn section() -> Section {
    Section {
        id: "1234".to_string(),
        added_at: "2020-06-11T14:51:08.056500Z".to_string(),
        user_id: "910".to_string(),
        project_id: "5678".to_string(),
        section_order: 1,
        name: "Bread".to_string(),
        updated_at: None,
        archived_at: None,
        is_archived: false,
        is_deleted: false,
        is_collapsed: false,
    }
}

pub fn comment() -> Comment {
    Comment {
        id: "2992679862".to_string(),
        posted_uid: None,
        content: "Need one bottle of milk".to_string(),
        uids_to_notify: None,
        posted_at: "2016-09-22T07:00:00.000000Z".to_string(),
        reactions: None,
        item_id: "123".to_string(),
        is_deleted: false,
        file_attachment: None,
    }
}
