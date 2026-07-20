//! End-to-end tests against the live Todoist API.
//!
//! Requires the internal `TOD_DEV_CI_STATIC` Todoist project to be pre-configured.
//! The API token (`TOD_E2E_TOKEN`) is available to maintainers and in CI only.
//! Running these tests against a personal account will fail on static fixture assertions.
//!
//! # Usage
//!
//! ```bash
//! TOD_E2E_TOKEN=your_token cargo test --features e2e --test e2e_todoist
//! ```
//!
//! The token is used to write a temporary config file for each test run.
//! No pre-existing config file is required or used.

#![cfg(feature = "e2e")]

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::{TempDir, tempdir};

const STATIC_PROJECT: &str = "TOD_DEV_CI_STATIC";

fn tod() -> Command {
    Command::cargo_bin("tod").expect("tod binary should build")
}

/// Reads `TOD_E2E_TOKEN`, runs `auth token` to create a fresh config (which also
/// fetches and saves the account timezone), and returns the temp dir + config path.
/// The `TempDir` must be kept alive for the duration of the test.
fn setup_config() -> (TempDir, std::path::PathBuf) {
    let token = std::env::var("TOD_E2E_TOKEN")
        .expect("TOD_E2E_TOKEN must be set to run API-dependent e2e tests");
    let dir = tempdir().expect("temp dir should be created");
    let path = dir.path().join("tod.cfg");

    tod()
        .arg("--config")
        .arg(&path)
        .args(["auth", "token", &token])
        .assert()
        .success();

    (dir, path)
}

/// Runs `project import --auto` against an already-initialised config, pulling all
/// Todoist projects into the config so tests can address them by name.
fn import_projects(config: &std::path::Path) {
    tod()
        .arg("--config")
        .arg(config)
        .args(["project", "import", "--auto"])
        .assert()
        .success();
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

/// config check-version succeeds and reports the current version without a config file.
#[test]
fn check_version_runs_without_config() {
    tod()
        .args(["config", "check-version"])
        .assert()
        .success()
        .stdout(predicate::str::is_match(r"\d+\.\d+\.\d+").unwrap());
}

/// config reset --force deletes an existing config file; the file is gone afterward.
#[test]
fn config_reset_force_deletes_existing_file() {
    let dir = tempdir().expect("temp dir should be created");
    let path = dir.path().join("tod.cfg");
    std::fs::write(&path, "{}").expect("config should be written");
    assert!(path.exists(), "config should exist before reset");

    tod()
        .arg("--config")
        .arg(&path)
        .args(["config", "reset", "--force"])
        .assert()
        .success();

    assert!(!path.exists(), "config should be gone after reset --force");
}

/// config reset --force fails and reports an error when no config file exists.
#[test]
fn config_reset_force_fails_when_file_absent() {
    let dir = tempdir().expect("temp dir should be created");
    let path = dir.path().join("tod.cfg");
    assert!(!path.exists(), "config should not exist before test");

    tod()
        .arg("--config")
        .arg(&path)
        .args(["config", "reset", "--force"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("No config file found at"));
}

// ---------------------------------------------------------------------------
// Static_Queries — read-only queries against the pre-configured CI project
// ---------------------------------------------------------------------------

/// `auth token` writes the config, fetches the account timezone, and reports success.
/// Verifies the config file exists and contains a non-empty timezone field afterward.
#[test]
fn auth_token_setup_saves_token_and_timezone() {
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

    assert!(path.exists(), "config file should be created by auth token");

    let contents = std::fs::read_to_string(&path).expect("config should be readable");
    let json: serde_json::Value =
        serde_json::from_str(&contents).expect("config should be valid JSON");
    assert!(
        json["timezone"].is_string() && !json["timezone"].as_str().unwrap().is_empty(),
        "timezone should be populated after auth token"
    );
}

/// `project import --auto` pulls all Todoist projects into the config; the static
/// fixture project `TOD_DEV_CI_STATIC` must be present in `project list` afterward.
#[test]
fn project_import_auto_includes_static_project() {
    let (_dir, config) = setup_config();
    import_projects(&config);

    tod()
        .arg("--config")
        .arg(&config)
        .args(["project", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains(STATIC_PROJECT));
}

/// `list view --sort value` returns tasks in priority-descending order.
/// The two p4 tasks must be first, then p3, then p2; the two p1 tasks are last
/// (their relative order is not guaranteed).
#[test]
fn list_view_sort_value_orders_by_priority() {
    let (_dir, config) = setup_config();
    import_projects(&config);

    let output = tod()
        .arg("--config")
        .arg(&config)
        .args(["list", "view", "--project", STATIC_PROJECT, "--sort", "value"])
        .output()
        .expect("list view should run");

    assert!(output.status.success(), "list view should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let tasks: Vec<&str> = stdout
        .lines()
        .filter(|l| l.starts_with("- "))
        .collect();

    assert!(tasks.len() >= 6, "expected at least 6 tasks, got {}", tasks.len());
    assert!(tasks[0].contains("Overdue High Priority"), "task 1 should be p4: {:?}", tasks[0]);
    assert!(tasks[1].contains("Section Task Future High Priority Labeled"), "task 2 should be 2nd p4: {:?}", tasks[1]);
    assert!(tasks[2].contains("Overdue Medium Priority"), "task 3 should be p3: {:?}", tasks[2]);
    assert!(tasks[3].contains("Future Low Priority Labeled"), "task 4 should be p2: {:?}", tasks[3]);
    let last_two: Vec<&str> = tasks[tasks.len() - 2..].to_vec();
    assert!(last_two.iter().any(|t| t.contains("No Date No Label")), "last two should include No Date No Label");
    assert!(last_two.iter().any(|t| t.contains("Section Task No Date")), "last two should include Section Task No Date");
}

/// `list view --sort datetime` returns tasks with no due date first, then ascending
/// by date. Positions 3-6 must be the four dated tasks in chronological order.
#[test]
fn list_view_sort_datetime_orders_by_date() {
    let (_dir, config) = setup_config();
    import_projects(&config);

    let output = tod()
        .arg("--config")
        .arg(&config)
        .args(["list", "view", "--project", STATIC_PROJECT, "--sort", "datetime"])
        .output()
        .expect("list view should run");

    assert!(output.status.success(), "list view should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let tasks: Vec<&str> = stdout
        .lines()
        .filter(|l| l.starts_with("- "))
        .collect();

    assert!(tasks.len() >= 6, "expected at least 6 tasks, got {}", tasks.len());
    assert!(tasks[2].contains("Overdue High Priority"), "task 3 should be 2020-01-01: {:?}", tasks[2]);
    assert!(tasks[3].contains("Overdue Medium Priority"), "task 4 should be 2020-06-15: {:?}", tasks[3]);
    assert!(tasks[4].contains("Section Task Future High Priority Labeled"), "task 5 should be 2099-06-15: {:?}", tasks[4]);
    assert!(tasks[5].contains("Future Low Priority Labeled"), "task 6 should be 2099-12-31: {:?}", tasks[5]);
}

/// Filter `#TOD_DEV_CI_STATIC & p1` (Todoist p1 = raw priority 4, the highest)
/// returns exactly the two highest-priority tasks.
#[test]
fn filter_by_priority_returns_expected_tasks() {
    let (_dir, config) = setup_config();
    import_projects(&config);

    let filter = format!("#{STATIC_PROJECT} & p1");
    let output = tod()
        .arg("--config")
        .arg(&config)
        .args(["list", "view", "--filter", &filter])
        .output()
        .expect("list view --filter should run");

    assert!(output.status.success(), "filter by priority should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let tasks: Vec<&str> = stdout.lines().filter(|l| l.starts_with("- ")).collect();

    assert_eq!(tasks.len(), 2, "expected exactly 2 p1 tasks, got {}: {stdout}", tasks.len());
    assert!(stdout.contains("Overdue High Priority"), "missing Overdue High Priority");
    assert!(stdout.contains("Section Task Future High Priority Labeled"), "missing Section Task Future High Priority Labeled");
}

/// Filter `#TOD_DEV_CI_STATIC & @e2estatic` returns exactly the two labeled tasks.
#[test]
fn filter_by_label_returns_expected_tasks() {
    let (_dir, config) = setup_config();
    import_projects(&config);

    let filter = format!("#{STATIC_PROJECT} & @e2estatic");
    let output = tod()
        .arg("--config")
        .arg(&config)
        .args(["list", "view", "--filter", &filter])
        .output()
        .expect("list view --filter should run");

    assert!(output.status.success(), "filter by label should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let tasks: Vec<&str> = stdout.lines().filter(|l| l.starts_with("- ")).collect();

    assert_eq!(tasks.len(), 2, "expected exactly 2 labeled tasks, got {}: {stdout}", tasks.len());
    assert!(stdout.contains("Future Low Priority Labeled"), "missing Future Low Priority Labeled");
    assert!(stdout.contains("Section Task Future High Priority Labeled"), "missing Section Task Future High Priority Labeled");
}

/// Filter `#TOD_DEV_CI_STATIC & /Static Section` returns exactly the two tasks
/// in that section.
#[test]
fn filter_by_section_returns_expected_tasks() {
    let (_dir, config) = setup_config();
    import_projects(&config);

    let filter = format!("#{STATIC_PROJECT} & /Static Section");
    let output = tod()
        .arg("--config")
        .arg(&config)
        .args(["list", "view", "--filter", &filter])
        .output()
        .expect("list view --filter should run");

    assert!(output.status.success(), "filter by section should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let tasks: Vec<&str> = stdout.lines().filter(|l| l.starts_with("- ")).collect();

    assert_eq!(tasks.len(), 2, "expected exactly 2 tasks in Static Section, got {}: {stdout}", tasks.len());
    assert!(stdout.contains("Section Task No Date"), "missing Section Task No Date");
    assert!(stdout.contains("Section Task Future High Priority Labeled"), "missing Section Task Future High Priority Labeled");
}

// ---------------------------------------------------------------------------
// dynamic_tasks — tests that create and mutate tasks
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// temp_project_lifecycle — tests that create and delete temporary projects
// ---------------------------------------------------------------------------
