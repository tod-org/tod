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
//! TOD_E2E_TOKEN=your_token cargo nextest run --manifest-path crates/tod-e2e/Cargo.toml
//! ```
//!
//! The token is used to write a temporary config file for each test run.
//! No pre-existing config file is required or used.

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;
use std::sync::OnceLock;
use std::thread::sleep;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tempfile::{TempDir, tempdir};

const STATIC_READ_PROJECT: &str = "TOD_DEV_CI_STATIC_READ";
const DYNAMIC_PROJECT: &str = "TOD_DEV_CI_DYNAMIC";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Returns a `tod` command with `DISABLE_SPINNER=1` pre-set so API calls
/// produce clean stdout with no spinner characters.
fn tod() -> Command {
    let mut cmd = Command::new(tod_binary_path());
    cmd.env("DISABLE_SPINNER", "1");
    cmd
}

fn tod_binary_path() -> PathBuf {
    static TOD_BINARY_PATH: OnceLock<PathBuf> = OnceLock::new();
    TOD_BINARY_PATH
        .get_or_init(resolve_tod_binary_path)
        .to_path_buf()
}

fn resolve_tod_binary_path() -> PathBuf {
    if let Some(path) = std::env::var_os("TOD_E2E_TOD_BIN") {
        return PathBuf::from(path);
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root should exist")
        .to_path_buf();

    let binary_name = if cfg!(windows) { "tod.exe" } else { "tod" };
    let target_dir = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace_root.join("target"));
    let binary_path = target_dir.join("debug").join(binary_name);
    if binary_path.exists() {
        return binary_path;
    }

    let status = StdCommand::new("cargo")
        .arg("build")
        .arg("--manifest-path")
        .arg(workspace_root.join("Cargo.toml"))
        .arg("--bin")
        .arg("tod")
        .status()
        .expect("cargo build should run");
    assert!(status.success(), "cargo build --bin tod should succeed");

    assert!(
        binary_path.exists(),
        "tod binary should exist at {}",
        binary_path.display()
    );
    binary_path
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

fn ensure_project_exists(config: &Path, project: &str) {
    let config_json = read_config_json(config);
    let projects = config_json
        .get("projectsv1")
        .and_then(Value::as_array)
        .expect("projectsv1 should exist in config");

    if projects
        .iter()
        .filter_map(|project_json| project_json.get("name").and_then(Value::as_str))
        .any(|name| name == project)
    {
        return;
    }

    tod()
        .arg("--config")
        .arg(config)
        .args(["project", "create", "--name", project])
        .assert()
        .success();
    pause_for_api_sync();
}

/// Cleanup helper: repeatedly calls `task next --project` and completes tasks
/// while the returned task contains `[E2E]`. Stops when no tasks remain.
fn cleanup_project_tasks(config: &Path, project: &str) {
    let _ = tod()
        .arg("--config")
        .arg(config)
        .args(["project", "empty", "--project", project])
        .output();

    let mut consecutive_empty_checks = 0_u8;
    for _ in 0..80 {
        let output = tod()
            .arg("--config")
            .arg(config)
            .args(["task", "next", "--project", project])
            .output()
            .expect("task next should run");

        if !output.status.success() {
            sleep(Duration::from_millis(350));
            continue;
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.contains("No tasks on list") {
            consecutive_empty_checks += 1;
            if consecutive_empty_checks >= 3 {
                break;
            }
            sleep(Duration::from_millis(350));
            continue;
        }
        consecutive_empty_checks = 0;
        if !stdout.contains("[E2E]") {
            break;
        }

        tod()
            .arg("--config")
            .arg(config)
            .args(["task", "complete"])
            .assert()
            .success();
        sleep(Duration::from_millis(350));
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

fn pause_for_api_sync() {
    sleep(Duration::from_millis(750));
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

fn list_task_names(stdout: &str) -> Vec<&str> {
    stdout
        .lines()
        .filter_map(|line| line.trim().strip_prefix("- "))
        .map(str::trim)
        .collect()
}

fn project_id_for(config: &Path, project_name: &str) -> String {
    let config_json = read_config_json(config);
    let projects = config_json
        .get("projectsv1")
        .and_then(Value::as_array)
        .expect("projectsv1 should exist in config");

    projects
        .iter()
        .find_map(|project| {
            let name = project.get("name").and_then(Value::as_str)?;
            let id = project.get("id").and_then(Value::as_str)?;
            (name == project_name).then_some(id.to_string())
        })
        .unwrap_or_else(|| panic!("project {project_name} should be present in config"))
}

fn task_priorities_by_title(config: &Path, project_name: &str) -> HashMap<String, u8> {
    let token = std::env::var("TOD_E2E_TOKEN").expect("TOD_E2E_TOKEN must be set");
    let project_id = project_id_for(config, project_name);
    let client = reqwest::blocking::Client::new();
    let response = client
        .get(format!(
            "https://api.todoist.com/api/v2/tasks?project_id={project_id}"
        ))
        .header(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {token}"),
        )
        .send()
        .expect("Todoist tasks should be retrievable");

    let status = response.status();
    let body = response.text().expect("Todoist tasks response should be readable");
    assert!(status.is_success(), "Todoist tasks request should succeed, got {status}: {body}");

    let payload: Value = serde_json::from_str(&body).expect("Todoist tasks response should be valid JSON");
    let tasks = payload
        .get("results")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("Todoist tasks payload should contain a results array: {body}"));

    tasks
        .iter()
        .filter_map(|task| {
            let content = task.get("content")?.as_str()?.to_string();
            let priority = task.get("priority")?.as_u64()? as u8;
            Some((content, priority))
        })
        .collect::<HashMap<_, _>>()
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
    let tasks = list_task_names(&stdout);
    let priorities = task_priorities_by_title(&config, STATIC_READ_PROJECT);
    let actual_priorities = tasks
        .iter()
        .map(|title| priorities.get(*title).copied().expect("task priority should exist"))
        .collect::<Vec<_>>();

    assert_eq!(
        tasks,
        vec![
            "[E2E-STATIC] Overdue High Priority",
            "[E2E-STATIC] Section Task Future High Priority Labeled",
            "[E2E-STATIC] Overdue Medium Priority",
            "[E2E-STATIC] Future Low Priority Labeled",
            "[E2E-STATIC] Oneoff Task",
            "[E2E-STATIC] Recurring Task",
            "[E2E-STATIC] Section Task No Date",
            "[E2E-STATIC] No Date No Label",
        ]
    );
    assert_eq!(actual_priorities, vec![4, 4, 3, 2, 1, 1, 1, 1]);
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
    let tasks = list_task_names(&stdout);
    let priorities = task_priorities_by_title(&config, STATIC_READ_PROJECT);
    let actual_priorities = tasks
        .iter()
        .map(|title| priorities.get(*title).copied().expect("task priority should exist"))
        .collect::<Vec<_>>();

    assert_eq!(
        tasks,
        vec![
            "[E2E-STATIC] Section Task No Date",
            "[E2E-STATIC] No Date No Label",
            "[E2E-STATIC] Overdue High Priority",
            "[E2E-STATIC] Overdue Medium Priority",
            "[E2E-STATIC] Oneoff Task",
            "[E2E-STATIC] Recurring Task",
            "[E2E-STATIC] Section Task Future High Priority Labeled",
            "[E2E-STATIC] Future Low Priority Labeled",
        ]
    );
    assert_eq!(actual_priorities, vec![1, 1, 4, 3, 1, 1, 4, 2]);
}

/// Filter by priority returns only tasks with that priority and preserves datetime ordering.
#[test]
fn filter_by_priority_returns_expected_tasks() {
    let (_dir, config) = setup_config();
    import_projects(&config);

    let output = tod()
        .arg("--config")
        .arg(&config)
        .args([
            "list",
            "view",
            "--filter",
            &format!("#{STATIC_READ_PROJECT} & p1"),
        ])
        .output()
        .expect("list view should run");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let tasks = list_task_names(&stdout);
    let priorities = task_priorities_by_title(&config, STATIC_READ_PROJECT);
    let actual_priorities = tasks
        .iter()
        .map(|title| priorities.get(*title).copied().expect("task priority should exist"))
        .collect::<Vec<_>>();

    assert_eq!(
        tasks,
        vec![
            "[E2E-STATIC] Overdue High Priority",
            "[E2E-STATIC] Section Task Future High Priority Labeled",
        ]
    );
    assert_eq!(actual_priorities, vec![4, 4]);
}

/// Filter by label returns only tasks with that label and preserves datetime ordering.
#[test]
fn filter_by_label_returns_expected_tasks() {
    let (_dir, config) = setup_config();
    import_projects(&config);

    let output = tod()
        .arg("--config")
        .arg(&config)
        .args([
            "list",
            "view",
            "--filter",
            &format!("#{STATIC_READ_PROJECT} & @e2estatic"),
        ])
        .output()
        .expect("list view should run");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let tasks = list_task_names(&stdout);
    let priorities = task_priorities_by_title(&config, STATIC_READ_PROJECT);
    let actual_priorities = tasks
        .iter()
        .map(|title| priorities.get(*title).copied().expect("task priority should exist"))
        .collect::<Vec<_>>();

    assert_eq!(
        tasks,
        vec![
            "[E2E-STATIC] Section Task Future High Priority Labeled",
            "[E2E-STATIC] Future Low Priority Labeled",
        ]
    );
    assert_eq!(actual_priorities, vec![4, 2]);
}

/// Filter by section returns only tasks in that section and preserves datetime ordering.
#[test]
fn filter_by_section_returns_expected_tasks() {
    let (_dir, config) = setup_config();
    import_projects(&config);

    let output = tod()
        .arg("--config")
        .arg(&config)
        .args([
            "list",
            "view",
            "--filter",
            &format!("#{STATIC_READ_PROJECT} & /Static Section"),
        ])
        .output()
        .expect("list view should run");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let tasks = list_task_names(&stdout);
    let priorities = task_priorities_by_title(&config, STATIC_READ_PROJECT);
    let actual_priorities = tasks
        .iter()
        .map(|title| priorities.get(*title).copied().expect("task priority should exist"))
        .collect::<Vec<_>>();

    assert_eq!(
        tasks,
        vec![
            "[E2E-STATIC] Section Task No Date",
            "[E2E-STATIC] Section Task Future High Priority Labeled",
        ]
    );
    assert_eq!(actual_priorities, vec![1, 4]);
}

// ---------------------------------------------------------------------------
// Dynamic tests — reuse TOD_DEV_CI_DYNAMIC, cleanup between tests
// (and run serially via serial_test lock)
// ---------------------------------------------------------------------------

/// Create 2 tasks, verify list and next, then complete them.
#[test]
fn dynamic_task_lifecycle() {
    let (_dir, config) = setup_config();
    import_projects(&config);
    ensure_project_exists(&config, DYNAMIC_PROJECT);

    cleanup_project_tasks(&config, DYNAMIC_PROJECT);
    pause_for_api_sync();

    for priority in [4, 2].iter() {
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
    pause_for_api_sync();

    tod()
        .arg("--config")
        .arg(&config)
        .args(["list", "view", "--project", DYNAMIC_PROJECT])
        .assert()
        .success()
        .stdout(predicate::str::contains("[E2E] Task Priority 4"))
        .stdout(predicate::str::contains("[E2E] Task Priority 2"));

    for _ in 0..2 {
        assert_next_task(&config, DYNAMIC_PROJECT, "[E2E] Task Priority");
        task_complete(&config);
        pause_for_api_sync();
    }

    cleanup_project_tasks(&config, DYNAMIC_PROJECT);
    pause_for_api_sync();
}

/// Add a comment to the recurring static fixture task and verify it appears in the next output.
#[test]
fn task_comment_create_is_visible_on_next() {
    let (_dir, config) = setup_config();
    import_projects(&config);

    let filter = format!("#{STATIC_READ_PROJECT} & recurring");
    let comment_content = "e2e test comment";

    tod()
        .arg("--config")
        .arg(&config)
        .args(["task", "next", "--filter", &filter])
        .assert()
        .success();

    tod()
        .arg("--config")
        .arg(&config)
        .args(["task", "comment", "--content", comment_content])
        .assert()
        .success();

    tod()
        .arg("--config")
        .arg(&config)
        .args(["task", "next", "--filter", &filter])
        .assert()
        .success()
        .stdout(predicate::str::contains(comment_content));
}

/// Static recurring fixture is returned by the `recurring` filter.
#[test]
fn recurring_filter_returns_recurring_task() {
    let (_dir, config) = setup_config();
    import_projects(&config);

    let output = tod()
        .arg("--config")
        .arg(&config)
        .args([
            "list",
            "view",
            "--filter",
            &format!("#{STATIC_READ_PROJECT} & recurring"),
        ])
        .output()
        .expect("list view should run");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let tasks = list_task_names(&stdout);
    assert_eq!(tasks.len(), 1, "expected 1 recurring task, got {tasks:?}");
    assert_eq!(
        tasks[0],
        "[E2E-STATIC] Recurring Task",
        "expected the recurring task to be returned, got {tasks:?}"
    );
}

/// Empty project shows no tasks in list view and task next.
#[test]
fn empty_project_list_and_next_show_nothing_present() {
    let (_dir, config) = setup_config();
    import_projects(&config);

    let project = random_project_name("TOD_CI_EMPTY");
    tod()
        .arg("--config")
        .arg(&config)
        .args(["project", "create", "--name", &project])
        .assert()
        .success();
    pause_for_api_sync();

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
        .args(["project", "delete", "--project", &project])
        .assert()
        .success();
    pause_for_api_sync();
}

/// Create a task in the existing dynamic project and clean the project up afterward.
#[test]
fn quick_project_create_and_task_create() {
    let (_dir, config) = setup_config();
    import_projects(&config);
    ensure_project_exists(&config, DYNAMIC_PROJECT);

    tod()
        .arg("--config")
        .arg(&config)
        .args([
            "task",
            "create",
            "--content",
            "[E2E] Quick Task",
            "--project",
            DYNAMIC_PROJECT,
            "--priority",
            "1",
            "--no-section",
        ])
        .assert()
        .success();

    assert_next_task(&config, DYNAMIC_PROJECT, "[E2E] Quick Task");
    task_complete(&config);
    pause_for_api_sync();

    cleanup_project_tasks(&config, DYNAMIC_PROJECT);
    pause_for_api_sync();
}

/// Create a random project, rename it, then delete it.
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

    tod()
        .arg("--config")
        .arg(&config)
        .args(["project", "delete", "--project", &renamed_project])
        .assert()
        .success()
        .stdout(predicate::str::contains("✓"));
}
