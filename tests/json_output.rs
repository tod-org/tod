use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::json;
use std::fs;
use tempfile::tempdir;

fn tod_command() -> Command {
    Command::cargo_bin("tod").expect("tod binary should build")
}

fn write_config_with_token(path: &std::path::Path, token: &str) {
    let config = json!({
        "path": path.to_string_lossy(),
        "timezone": "UTC",
        "token": token,
    });
    fs::write(path, config.to_string()).expect("config should be written");
}

// ── Test 1: `tod -j config about` — valid JSON, contains data.version, data.target ──

#[test]
fn json_config_about_returns_valid_json_with_version_and_target() {
    let output = tod_command()
        .arg("-j")
        .args(["config", "about"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).expect("stdout should be valid UTF-8");

    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("output should be valid JSON");

    assert!(
        parsed["data"]["version"].is_string(),
        "data.version should be a string, got: {stdout}"
    );
    assert!(
        parsed["data"]["target"].is_string(),
        "data.target should be a string, got: {stdout}"
    );
}

// ── Test 2: `tod config about` (no -j) — human-readable text, not JSON ──

#[test]
fn config_about_without_json_flag_outputs_text_not_json() {
    let output = tod_command()
        .args(["config", "about"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).expect("stdout should be valid UTF-8");

    assert!(
        stdout.contains("APP:"),
        "text output should contain APP:, got: {stdout}"
    );
    assert!(
        stdout.contains("VERSION:"),
        "text output should contain VERSION:, got: {stdout}"
    );

    // Should not parse as JSON
    assert!(
        serde_json::from_str::<serde_json::Value>(&stdout).is_err(),
        "text output should not be valid JSON, got: {stdout}"
    );
}

// ── Test 3: `tod -j task quick-add "json test"` — valid JSON with task fields (requires API) ──

#[test]
#[ignore = "requires live Todoist API or mockito server"]
fn json_task_quick_add_returns_valid_task_json() {
    let dir = tempdir().expect("temp dir should be created");
    let path = dir.path().join("tod.cfg");
    write_config_with_token(&path, "test-token");

    let output = tod_command()
        .arg("--config")
        .arg(&path)
        .arg("-j")
        .args(["task", "quick-add", "json test"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).expect("stdout should be valid UTF-8");
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("output should be valid JSON");

    let data = &parsed["data"];
    assert!(data["id"].is_string(), "task should have an id");
    assert!(data["content"].is_string(), "task should have content");
}

// ── Test 4: `tod -j project list` — valid JSON array (requires API) ──

#[test]
#[ignore = "requires live Todoist API or mockito server"]
fn json_project_list_returns_valid_json_array() {
    let dir = tempdir().expect("temp dir should be created");
    let path = dir.path().join("tod.cfg");
    write_config_with_token(&path, "test-token");

    let output = tod_command()
        .arg("--config")
        .arg(&path)
        .arg("-j")
        .args(["project", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).expect("stdout should be valid UTF-8");
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("output should be valid JSON");

    assert!(
        parsed["data"].is_array(),
        "data should be an array, got: {stdout}"
    );
}

// ── Test 5: `tod -j list view --project inbox` — valid JSON with tasks array (requires API) ──

#[test]
#[ignore = "requires live Todoist API or mockito server"]
fn json_list_view_returns_tasks_array_and_count() {
    let dir = tempdir().expect("temp dir should be created");
    let path = dir.path().join("tod.cfg");
    write_config_with_token(&path, "test-token");

    let output = tod_command()
        .arg("--config")
        .arg(&path)
        .arg("-j")
        .args(["list", "view", "--project", "inbox"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).expect("stdout should be valid UTF-8");
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("output should be valid JSON");

    assert!(
        parsed["data"]["tasks"].is_array(),
        "data.tasks should be an array, got: {stdout}"
    );
    assert!(
        parsed["data"]["count"].is_number(),
        "data.count should be a number, got: {stdout}"
    );
}

// ── Test 6: `tod -j task create` — error about missing interactive input ──

#[test]
fn json_task_create_with_no_args_errors_about_interactive_input() {
    let dir = tempdir().expect("temp dir should be created");
    let path = dir.path().join("tod.cfg");
    write_config_with_token(&path, "test-token");

    let output = tod_command()
        .arg("--config")
        .arg(&path)
        .arg("-j")
        .args(["task", "create"])
        .assert()
        .failure()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).expect("stdout should be valid UTF-8");
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("error output should be valid JSON");

    assert_eq!(parsed["error"]["source"], "json_mode");
    assert!(
        parsed["error"]["message"]
            .as_str()
            .is_some_and(|m| m.contains("JSON mode")),
        "error message should mention JSON mode, got: {stdout}"
    );
}

// ── Test 7: `tod -j nonexistent-command` — error from clap ──

#[test]
fn json_nonexistent_command_errors_from_clap() {
    tod_command()
        .arg("-j")
        .arg("nonexistent-command")
        .assert()
        .failure()
        .stderr(predicate::str::contains("error"));
}

// ── Test 8: `tod -j --help` — help text (not JSON, handled by clap before our code) ──

#[test]
fn json_help_outputs_help_text_not_json() {
    let output = tod_command()
        .arg("-j")
        .arg("--help")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).expect("stdout should be valid UTF-8");

    assert!(
        stdout.contains("Usage:"),
        "help output should contain Usage:, got: {stdout}"
    );
    assert!(
        stdout.contains("--json"),
        "help output should document --json flag, got: {stdout}"
    );

    // Help text is not JSON
    assert!(
        serde_json::from_str::<serde_json::Value>(&stdout).is_err(),
        "help output should not parse as JSON, got: {stdout}"
    );
}

// ── Test 9: `tod -j config about | jq .data.version` — exits 0 ──

#[test]
fn json_config_about_piped_to_jq_extracts_version() {
    let mut cmd = tod_command();
    let assert = cmd.arg("-j").args(["config", "about"]).assert().success();

    let stdout_bytes = assert.get_output().stdout.clone();
    let stdout = String::from_utf8(stdout_bytes).expect("stdout should be valid UTF-8");

    // Simulate what `jq .data.version` would do: parse JSON, extract data.version
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("output should be valid JSON");
    let version = parsed["data"]["version"]
        .as_str()
        .expect("data.version should be a string");

    assert!(!version.is_empty(), "version should not be empty");
}

// ── Test 10: `tod -j config about 2>/dev/null` — no stderr output for success ──

#[test]
fn json_config_about_produces_no_stderr_on_success() {
    let output = tod_command()
        .arg("-j")
        .args(["config", "about"])
        .assert()
        .success()
        .get_output()
        .stderr
        .clone();

    assert!(
        output.is_empty(),
        "stderr should be empty on success, got: {}",
        String::from_utf8_lossy(&output)
    );
}

// ── Additional: JSON mode with -j short flag (not just --json) ──

#[test]
fn json_short_flag_j_config_about_returns_valid_json() {
    let output = tod_command()
        .arg("-j")
        .args(["config", "about"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).expect("stdout should be valid UTF-8");

    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("output should be valid JSON");

    assert!(parsed["data"]["version"].is_string());
}

// ── Additional: JSON error format contract ──

#[test]
fn json_task_create_error_has_message_and_source_fields() {
    let dir = tempdir().expect("temp dir should be created");
    let path = dir.path().join("tod.cfg");
    write_config_with_token(&path, "test-token");

    let output = tod_command()
        .arg("--config")
        .arg(&path)
        .arg("-j")
        .args(["task", "create"])
        .assert()
        .failure()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).expect("stdout should be valid UTF-8");
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("error output should be valid JSON");

    let error = &parsed["error"];
    assert!(
        error["message"].is_string(),
        "error should have a message string field"
    );
    assert!(
        error["source"].is_string(),
        "error should have a source string field"
    );
    assert!(
        error["message"].as_str().is_some_and(|m| !m.is_empty()),
        "error message should not be empty"
    );
    assert!(
        error["source"].as_str().is_some_and(|s| !s.is_empty()),
        "error source should not be empty"
    );
}

// ── Additional: JSON mode does not produce colored output ──

#[test]
fn json_config_about_output_has_no_ansi_escape_codes() {
    let output = tod_command()
        .arg("-j")
        .args(["config", "about"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).expect("stdout should be valid UTF-8");

    assert!(
        !stdout.contains('\x1b'),
        "JSON output should not contain ANSI escape codes, got: {stdout:?}"
    );
}
