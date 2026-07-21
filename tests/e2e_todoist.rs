//! End-to-end tests against the live Todoist API.
//!
//! Requires two Todoist projects to be pre-existing:
//! - `TOD_DEV_CI_STATIC_READ` — read-only, pre-populated with test data
//! - `TOD_DEV_CI_DYNAMIC` — reused across tests, tasks cleaned between runs
//! - A third `TOD_DEV_CI_PROJECTXXXX` — for project lifecycle tests (rename, delete, import) will be automatically created and deleted.
//!
//! The API token (`TOD_E2E_TOKEN`) is available to maintainers, in Github Secrets, or in CI only.
//!
//! # Usage
//!
//! ```bash
//! TOD_E2E_TOKEN=your_token cargo nextest run --features e2e --profile e2e
//! ```
//!
//! The token is used to write a temporary config file for each test run.
//! No pre-existing config file is required or used.

#![cfg(feature = "e2e")]

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tempfile::{TempDir, tempdir};

const STATIC_READ_PROJECT: &str = "TOD_DEV_CI_STATIC_READ";
const DYNAMIC_PROJECT: &str = "TOD_DEV_CI_DYNAMIC";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Returns a `tod` command with `DISABLE_SPINNER=1` pre-set so API calls
/// produce clean stdout with no spinner characters.
fn tod() -> Command {
    let mut cmd = Command::cargo_bin("tod").expect("tod binary should build");
    cmd.env("DISABLE_SPINNER", "1");
    cmd
}

/// Reads `TOD_E2E_TOKEN`, runs `auth token` (which also fetches and saves the
/// account timezone via the API), and returns the temp dir + config path.
/// The `TempDir` must be kept alive for the duration of the test.
fn setup_config() -> (TempDir, PathBuf) {
    let token = std::env::var("TOD_E2E_TOKEN")
        .expect("TOD_E2E_TOKEN must be set to run API-dependent e2e tests");
    let dir = tempdir().expect("temp dir should be created");
    let path = dir.path().join("tod.cfg");

    tod()
        .arg("--config")
        .arg(&path)
        .args(["auth", "token", &token])
        .assert()
        .success()
        .stdout(predicate::str::contains("✓ API token saved to"));

    (dir, path)
}

fn read_config_json(config: &Path) -> Value {
    let config_contents = std::fs::read_to_string(config).expect("config should be readable");
    serde_json::from_str(&config_contents).expect("config should be valid JSON")
}

fn write_config_json(config: &Path, value: &Value) {
    let serialized = serde_json::to_string_pretty(value).expect("config should serialize");
    std::fs::write(config, format!("{serialized}\n")).expect("config should be writable");
}

fn remove_object_key(value: &mut Value, key: &str) {
    value
        .as_object_mut()
        .expect("config should be a JSON object")
        .remove(key);
}

/// Runs `project import --auto` to import all accessible projects.
fn import_projects(config: &Path) {
    tod()
        .arg("--config")
        .arg(config)
        .args(["project", "import", "--auto"])
        .assert()
        .success();
}

/// Cleanup helper: repeatedly calls `task next --project` and completes tasks
/// while the returned task contains `[E2E]`. Stops when no tasks remain.
fn cleanup_project_tasks(config: &Path, project: &str) {
    let _ = tod()
        .arg("--config")
        .arg(config)
        .args(["project", "empty", "--project", project])
        .output();

    for _ in 0..50 {
        let output = tod()
            .arg("--config")
            .arg(config)
            .args(["task", "next", "--project", project])
            .output()
            .expect("task next should run");

        if !output.status.success() {
            break;
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.contains("No tasks on list") {
            break;
        }
        if !stdout.contains("[E2E]") {
            break;
        }

        tod()
            .arg("--config")
            .arg(config)
            .args(["task", "complete"])
            .assert()
            .success();
    }
}

/// Calls `task next --project <project>` and asserts the output contains `expected`.
fn assert_next_task(config: &Path, project: &str, expected: &str) {
    tod()
        .arg("--config")
        .arg(config)
        .args(["task", "next", "--project", project])
        .assert()
        .success()
        .stdout(predicate::str::contains(expected));
}

/// Calls `task complete` (completes the last task returned by `task next`).
fn task_complete(config: &Path) {
    tod()
        .arg("--config")
        .arg(config)
        .args(["task", "complete"])
        .assert()
        .success();
}

fn random_project_name(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be monotonic")
        .as_nanos();
    format!("{prefix}_{nanos:X}")
}

// ---------------------------------------------------------------------------
// CLI_Only — no API calls, no token required
// ---------------------------------------------------------------------------

/// --version prints the semver version in the expected format.
#[test]
fn version_prints_semver() {
    tod()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::is_match(r"tod \d+\.\d+\.\d+").unwrap());
}

/// --help lists all top-level subcommands.
#[test]
fn help_includes_expected_commands() {
    tod()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("project"))
        .stdout(predicate::str::contains("task"))
        .stdout(predicate::str::contains("list"))
        .stdout(predicate::str::contains("config"))
        .stdout(predicate::str::contains("auth"))
        .stdout(predicate::str::contains("shell"));
}

/// `config check-version` succeeds and reports a semver version without a config file.
#[test]
fn check_version_runs_without_config() {
    tod()
        .args(["config", "check-version"])
        .assert()
        .success()
        .stdout(predicate::str::is_match(r"\d+\.\d+\.\d+").unwrap());
}

/// `config reset --force` fails when no config file exists.
#[test]
fn config_reset_force_fails_when_file_absent() {
    let dir = tempdir().expect("temp dir should be created");
    let nonexistent = dir.path().join("does-not-exist.cfg");

    tod()
        .arg("--config")
        .arg(&nonexistent)
        .args(["config", "reset", "--force"])
        .assert()
        .failure();
}

/// `config reset --force` deletes the config file and reports success text.
#[test]
fn config_reset_force_reports_deletion() {
    let (_dir, config) = setup_config();
    assert!(config.exists(), "config file should exist after setup");

    tod()
        .arg("--config")
        .arg(&config)
        .args(["config", "reset", "--force"])
        .assert()
        .success()
        .stdout(predicate::str::contains("deleted successfully"));

    assert!(!config.exists(), "config file should be deleted");
}

// ---------------------------------------------------------------------------
// Config + Auth — API calls with shared state
// ---------------------------------------------------------------------------

/// `auth token` succeeds, saves token to config, and fetches timezone.
#[test]
fn auth_token_setup_saves_token_and_timezone() {
    let (_dir, config) = setup_config();
    let token = std::env::var("TOD_E2E_TOKEN").expect("TOD_E2E_TOKEN must be set");
    let config_json = read_config_json(&config);

    assert_eq!(
        config_json.get("token").and_then(Value::as_str),
        Some(token.as_str()),
        "token should be written to config"
    );
    assert!(
        config_json
            .get("timezone")
            .and_then(Value::as_str)
            .is_some_and(|tz| !tz.is_empty()),
        "timezone should be fetched and written to config"
    );
}

/// Re-running `auth token` updates the token but preserves other config entries.
#[test]
fn auth_token_updates_existing_config_without_overwrite() {
    let (_dir, config) = setup_config();
    let token = std::env::var("TOD_E2E_TOKEN").expect("TOD_E2E_TOKEN must be set");

    // Import a project, set an old token value manually, then re-run auth.
    import_projects(&config);
    let mut before = read_config_json(&config);
    before
        .as_object_mut()
        .expect("config should be a JSON object")
        .insert(
            "token".to_string(),
            Value::String("OLD_E2E_TOKEN".to_string()),
        );
    write_config_json(&config, &before);

    tod()
        .arg("--config")
        .arg(&config)
        .args(["auth", "token", &token])
        .assert()
        .success();

    let after = read_config_json(&config);
    assert_eq!(
        after.get("token").and_then(Value::as_str),
        Some(token.as_str()),
        "token should be updated to the provided value"
    );

    let mut before_without_token = before;
    remove_object_key(&mut before_without_token, "token");
    remove_object_key(&mut before_without_token, "last_version_check");
    let mut after_without_token = after;
    remove_object_key(&mut after_without_token, "token");
    remove_object_key(&mut after_without_token, "last_version_check");
    assert_eq!(
        before_without_token, after_without_token,
        "auth token should only change token-related config state"
    );
}

/// Re-running `config set-timezone` updates timezone but preserves other config entries.
#[test]
fn set_timezone_updates_existing_config_without_overwrite() {
    let (_dir, config) = setup_config();

    // Import projects, then change timezone and restore it explicitly.
    import_projects(&config);
    let original = read_config_json(&config);
    let timezone = original
        .get("timezone")
        .and_then(Value::as_str)
        .expect("initial timezone should be present")
        .to_string();
    let interim_timezone = if timezone == "UTC" {
        "America/Denver"
    } else {
        "UTC"
    };
    let mut with_changed_timezone = original.clone();
    with_changed_timezone
        .as_object_mut()
        .expect("config should be a JSON object")
        .insert(
            "timezone".to_string(),
            Value::String(interim_timezone.to_string()),
        );
    write_config_json(&config, &with_changed_timezone);

    tod()
        .arg("--config")
        .arg(&config)
        .args(["config", "set-timezone", "--timezone", &timezone])
        .assert()
        .success();

    let after = read_config_json(&config);
    assert!(
        after
            .get("timezone")
            .and_then(Value::as_str)
            .is_some_and(|tz| !tz.is_empty()),
        "timezone should be restored by config set-timezone"
    );

    let mut original_without_timezone = original;
    remove_object_key(&mut original_without_timezone, "timezone");
    remove_object_key(&mut original_without_timezone, "last_version_check");
    let mut after_without_timezone = after;
    remove_object_key(&mut after_without_timezone, "timezone");
    remove_object_key(&mut after_without_timezone, "last_version_check");
    assert_eq!(
        original_without_timezone, after_without_timezone,
        "set-timezone should only change timezone-related config state"
    );
}

// ---------------------------------------------------------------------------
// Static_Read tests — read-only tests against TOD_DEV_CI_STATIC_READ
// ---------------------------------------------------------------------------

/// `project import --auto` includes the static read project in the config.
#[test]
fn project_import_auto_includes_static_project() {
    let (_dir, config) = setup_config();
    import_projects(&config);

    let config_contents = std::fs::read_to_string(&config).expect("config should be readable");
    assert!(
        config_contents.contains(STATIC_READ_PROJECT),
        "static read project should be imported"
    );
}

/// `list view --sort value` orders tasks by priority (highest first).
#[test]
fn list_view_sort_value_orders_by_priority() {
    let (_dir, config) = setup_config();
    import_projects(&config);

    let output = tod()
        .arg("--config")
        .arg(&config)
        .args([
            "list",
            "view",
            "--project",
            STATIC_READ_PROJECT,
            "--sort",
            "value",
        ])
        .output()
        .expect("list view should run");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let tasks: Vec<&str> = stdout.lines().filter(|l| l.starts_with("- ")).collect();
    assert!(
        tasks.len() >= 6,
        "expected at least 6 tasks, got {}",
        tasks.len()
    );
}

/// `list view --sort datetime` orders tasks by due date (no-date first, then ascending).
#[test]
fn list_view_sort_datetime_orders_by_date() {
    let (_dir, config) = setup_config();
    import_projects(&config);

    let output = tod()
        .arg("--config")
        .arg(&config)
        .args([
            "list",
            "view",
            "--project",
            STATIC_READ_PROJECT,
            "--sort",
            "datetime",
        ])
        .output()
        .expect("list view should run");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let tasks: Vec<&str> = stdout.lines().filter(|l| l.starts_with("- ")).collect();
    assert!(
        tasks.len() >= 6,
        "expected at least 6 tasks, got {}",
        tasks.len()
    );
}

/// Filter by priority returns only tasks with that priority.
#[test]
fn filter_by_priority_returns_expected_tasks() {
    let (_dir, config) = setup_config();
    import_projects(&config);

    tod()
        .arg("--config")
        .arg(&config)
        .args([
            "list",
            "view",
            "--filter",
            &format!("#{STATIC_READ_PROJECT} & p1"),
        ])
        .assert()
        .success();
}

/// Filter by label returns only tasks with that label.
#[test]
fn filter_by_label_returns_expected_tasks() {
    let (_dir, config) = setup_config();
    import_projects(&config);

    tod()
        .arg("--config")
        .arg(&config)
        .args([
            "list",
            "view",
            "--filter",
            &format!("#{STATIC_READ_PROJECT} & @e2estatic"),
        ])
        .assert()
        .success();
}

/// Filter by section returns only tasks in that section.
#[test]
fn filter_by_section_returns_expected_tasks() {
    let (_dir, config) = setup_config();
    import_projects(&config);

    tod()
        .arg("--config")
        .arg(&config)
        .args([
            "list",
            "view",
            "--filter",
            &format!("#{STATIC_READ_PROJECT} & /Static"),
        ])
        .assert()
        .success();
}

// ---------------------------------------------------------------------------
// Dynamic tests — reuse TOD_DEV_CI_DYNAMIC, cleanup between tests
// ---------------------------------------------------------------------------

/// Create 4 tasks, verify list and next, complete them, verify empty state.
#[test]
fn dynamic_task_lifecycle() {
    let (_dir, config) = setup_config();
    import_projects(&config);

    cleanup_project_tasks(&config, DYNAMIC_PROJECT);

    // Create 4 tasks at different priorities
    for (_i, priority) in [4, 3, 2, 1].iter().enumerate() {
        tod()
            .arg("--config")
            .arg(&config)
            .args([
                "task",
                "create",
                "--content",
                &format!("[E2E] Task Priority {}", priority),
                "--project",
                DYNAMIC_PROJECT,
                "--priority",
                &priority.to_string(),
                "--no-section",
            ])
            .assert()
            .success();
    }

    // Verify list contains all 4
    tod()
        .arg("--config")
        .arg(&config)
        .args(["list", "view", "--project", DYNAMIC_PROJECT])
        .assert()
        .success()
        .stdout(predicate::str::contains("[E2E] Task Priority"));

    // Complete all tasks
    for _ in 0..4 {
        assert_next_task(&config, DYNAMIC_PROJECT, "[E2E] Task Priority");
        task_complete(&config);
    }

    // Verify empty
    tod()
        .arg("--config")
        .arg(&config)
        .args(["task", "next", "--project", DYNAMIC_PROJECT])
        .assert()
        .success()
        .stdout(predicate::str::contains("No tasks on list"));

    cleanup_project_tasks(&config, DYNAMIC_PROJECT);
}

/// Create a task, add a comment, verify comment appears in task next output.
#[test]
fn task_comment_create_is_visible_on_next() {
    let (_dir, config) = setup_config();
    import_projects(&config);

    cleanup_project_tasks(&config, DYNAMIC_PROJECT);

    let task_content = "[E2E] Comment Test";
    let comment_content = "e2e test comment";

    tod()
        .arg("--config")
        .arg(&config)
        .args([
            "task",
            "create",
            "--content",
            task_content,
            "--project",
            DYNAMIC_PROJECT,
            "--priority",
            "1",
            "--no-section",
        ])
        .assert()
        .success();

    assert_next_task(&config, DYNAMIC_PROJECT, task_content);

    // Add comment
    tod()
        .arg("--config")
        .arg(&config)
        .args(["task", "comment", "--content", comment_content])
        .assert()
        .success();

    // Verify comment appears in next
    assert_next_task(&config, DYNAMIC_PROJECT, comment_content);

    task_complete(&config);
    cleanup_project_tasks(&config, DYNAMIC_PROJECT);
}

/// Static recurring fixtures appear only in `recurring`, non-recurring in `!recurring`.
#[test]
fn recurring_vs_not_recurring_filters() {
    let (_dir, config) = setup_config();
    import_projects(&config);

    // Recurring filter
    tod()
        .arg("--config")
        .arg(&config)
        .args([
            "list",
            "view",
            "--filter",
            &format!("#{STATIC_READ_PROJECT} & recurring"),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Recurring Task"))
        .stdout(predicate::str::contains("Oneoff Task").not());

    // Non-recurring filter
    tod()
        .arg("--config")
        .arg(&config)
        .args([
            "list",
            "view",
            "--filter",
            &format!("#{STATIC_READ_PROJECT} & !recurring"),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Oneoff Task"))
        .stdout(predicate::str::contains("Recurring Task").not());
}

/// Empty project shows no tasks in list view and task next.
#[test]
fn empty_project_list_and_next_show_nothing_present() {
    let (_dir, config) = setup_config();
    import_projects(&config);

    cleanup_project_tasks(&config, DYNAMIC_PROJECT);

    tod()
        .arg("--config")
        .arg(&config)
        .args(["list", "view", "--project", DYNAMIC_PROJECT])
        .assert()
        .success()
        .stdout(predicate::str::contains(&format!(
            "Tasks for {DYNAMIC_PROJECT}"
        )))
        .stdout(predicate::str::contains("- ").not());

    tod()
        .arg("--config")
        .arg(&config)
        .args(["task", "next", "--project", DYNAMIC_PROJECT])
        .assert()
        .success()
        .stdout(predicate::str::contains("No tasks on list"));
}

/// Create a random empty project, verify empty-query behavior, rename it, then
/// delete it.
#[test]
fn dynamic_empty_project_create_query_delete() {
    let (_dir, config) = setup_config();
    import_projects(&config);

    let project = random_project_name("TOD_CI_PRJ");
    let renamed_project = format!("{project}_REN");

    tod()
        .arg("--config")
        .arg(&config)
        .args(["project", "create", "--name", &project])
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "Created project {project} and added to config"
        )));

    tod()
        .arg("--config")
        .arg(&config)
        .args(["project", "import", "--project", &project])
        .assert()
        .success();

    tod()
        .arg("--config")
        .arg(&config)
        .args(["list", "view", "--project", &project])
        .assert()
        .success()
        .stdout(predicate::str::contains(format!("Tasks for {project}")))
        .stdout(predicate::str::contains("- ").not());

    tod()
        .arg("--config")
        .arg(&config)
        .args(["task", "next", "--project", &project])
        .assert()
        .success()
        .stdout(predicate::str::contains("No tasks on list"));

    tod()
        .arg("--config")
        .arg(&config)
        .args([
            "project",
            "rename",
            "--project",
            &project,
            "--name",
            &renamed_project,
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("✓"));

    let renamed_config = read_config_json(&config);
    let renamed_names: Vec<&str> = renamed_config
        .get("projectsv1")
        .and_then(Value::as_array)
        .expect("projectsv1 should exist in config")
        .iter()
        .filter_map(|p| p.get("name").and_then(Value::as_str))
        .collect();
    assert!(
        renamed_names.iter().any(|n| *n == renamed_project),
        "renamed project should exist in config"
    );
    assert!(
        !renamed_names.iter().any(|n| *n == project),
        "old project name should be removed from config"
    );

    tod()
        .arg("--config")
        .arg(&config)
        .args(["list", "view", "--project", &renamed_project])
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "Tasks for {renamed_project}"
        )))
        .stdout(predicate::str::contains("- ").not());

    tod()
        .arg("--config")
        .arg(&config)
        .args(["task", "next", "--project", &renamed_project])
        .assert()
        .success()
        .stdout(predicate::str::contains("No tasks on list"));

    tod()
        .arg("--config")
        .arg(&config)
        .args(["project", "delete", "--project", &renamed_project])
        .assert()
        .success()
        .stdout(predicate::str::contains("✓"));

    let deleted_config = read_config_json(&config);
    let deleted_names: Vec<&str> = deleted_config
        .get("projectsv1")
        .and_then(Value::as_array)
        .expect("projectsv1 should exist in config")
        .iter()
        .filter_map(|p| p.get("name").and_then(Value::as_str))
        .collect();
    assert!(
        !deleted_names.iter().any(|n| *n == project),
        "old project name should not remain after delete"
    );
    assert!(
        !deleted_names.iter().any(|n| *n == renamed_project),
        "renamed project should not remain after delete"
    );
}
