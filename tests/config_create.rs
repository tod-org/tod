use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::json;
use std::fs;
use tempfile::tempdir;

fn tod_command() -> Command {
    Command::cargo_bin("tod").expect("tod binary should build")
}

fn write_config_with_timezone(path: &std::path::Path, token: Option<&str>) {
    let mut config = json!({
        "path": path.to_string_lossy(),
        "timezone": "UTC",
    });

    if let Some(token) = token {
        config["token"] = json!(token);
    }

    fs::write(path, config.to_string()).expect("config should be written");
}

// --- Config creation via `auth token` (the non-interactive creation path) ---

/// When the config file already has a timezone, `auth token` updates the token and reports success.
#[test]
fn auth_token_updates_config_with_existing_timezone() {
    let dir = tempdir().expect("temp dir should be created");
    let path = dir.path().join("tod.cfg");
    write_config_with_timezone(&path, None);

    tod_command()
        .arg("--config")
        .arg(&path)
        .args(["auth", "token", "test-token-abc123"])
        .assert()
        .success()
        .stdout(predicate::str::contains("✓ API token saved to"));

    assert!(
        path.exists(),
        "config file should still exist after auth token"
    );
}

/// The success output includes the path where the config was written.
#[test]
fn auth_token_output_includes_config_path() {
    let dir = tempdir().expect("temp dir should be created");
    let path = dir.path().join("tod.cfg");
    write_config_with_timezone(&path, None);

    tod_command()
        .arg("--config")
        .arg(&path)
        .args(["auth", "token", "test-token-abc123"])
        .assert()
        .success()
        .stdout(predicate::str::contains(path.to_string_lossy().as_ref()));
}

/// When the config file already exists (created by a prior `auth token`), the command
/// updates the token and reports success.
#[test]
fn auth_token_succeeds_with_existing_config() {
    let dir = tempdir().expect("temp dir should be created");
    let path = dir.path().join("tod.cfg");

    write_config_with_timezone(&path, Some("initial-token"));

    tod_command()
        .arg("--config")
        .arg(&path)
        .args(["auth", "token", "updated-token-xyz"])
        .assert()
        .success()
        .stdout(predicate::str::contains("✓ API token saved to"));
}

/// Running `auth token` twice (create then update) both succeed and the second
/// call retains the config file.
#[test]
fn auth_token_can_update_after_creating() {
    let dir = tempdir().expect("temp dir should be created");
    let path = dir.path().join("tod.cfg");

    write_config_with_timezone(&path, Some("first-token"));

    assert!(
        path.exists(),
        "config should exist before auth token update"
    );

    tod_command()
        .arg("--config")
        .arg(&path)
        .args(["auth", "token", "second-token"])
        .assert()
        .success()
        .stdout(predicate::str::contains("✓ API token saved to"));

    assert!(path.exists(), "config should still exist after update");
}

// --- Config check validates an existing config ---

/// `config check` reports that a valid, existing config is valid.
#[test]
fn config_check_reports_valid_for_existing_config() {
    let dir = tempdir().expect("temp dir should be created");
    let path = dir.path().join("tod.cfg");
    fs::write(&path, "{}").expect("config should be written");

    tod_command()
        .arg("--config")
        .arg(&path)
        .args(["config", "check"])
        .assert()
        .success()
        .stdout(predicate::str::contains("is valid"));
}

/// `config check` alias `c check` also works with a valid config.
#[test]
fn config_check_alias_reports_valid_for_existing_config() {
    let dir = tempdir().expect("temp dir should be created");
    let path = dir.path().join("tod.cfg");
    fs::write(&path, "{}").expect("config should be written");

    tod_command()
        .arg("--config")
        .arg(&path)
        .args(["c", "check"])
        .assert()
        .success()
        .stdout(predicate::str::contains("is valid"));
}

// --- Commands that work without any config file ---

/// `config about` succeeds and prints build information without needing a config file.
#[test]
fn config_about_works_without_config() {
    tod_command()
        .args(["config", "about"])
        .assert()
        .success()
        .stdout(predicate::str::contains("APP:"))
        .stdout(predicate::str::contains("VERSION:"));
}

/// `config about` alias `config a` also succeeds without a config file.
#[test]
fn config_about_alias_works_without_config() {
    tod_command()
        .args(["config", "a"])
        .assert()
        .success()
        .stdout(predicate::str::contains("APP:"));
}

// --- Config reset (deletion) ---

/// `config reset --force` deletes an existing config file and reports success.
#[test]
fn config_reset_force_deletes_existing_config() {
    let dir = tempdir().expect("temp dir should be created");
    let path = dir.path().join("tod.cfg");
    fs::write(&path, "{}").expect("config should be written");

    assert!(path.exists(), "config should exist before reset");

    tod_command()
        .arg("--config")
        .arg(&path)
        .args(["config", "reset", "--force"])
        .assert()
        .success()
        .stdout(predicate::str::contains("deleted successfully"));

    assert!(!path.exists(), "config should be gone after reset --force");
}

/// `config reset --force` when no config file exists reports the path and fails.
#[test]
fn config_reset_force_when_config_absent_reports_not_found() {
    let dir = tempdir().expect("temp dir should be created");
    let path = dir.path().join("tod.cfg");

    assert!(!path.exists(), "config should not exist before test");

    tod_command()
        .arg("--config")
        .arg(&path)
        .args(["config", "reset", "--force"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("No config file found at"));
}

/// `-c` passes an explicit config path through reset and reports that exact path on deletion.
#[test]
fn config_reset_force_with_short_config_flag_deletes_manual_path() {
    let dir = tempdir().expect("temp dir should be created");
    let path = dir.path().join("manual-tod.cfg");
    fs::write(&path, "{}").expect("config should be written");
    let expected = format!("Config file at {} deleted successfully.", path.display());

    tod_command()
        .arg("-c")
        .arg(&path)
        .args(["config", "reset", "--force"])
        .assert()
        .success()
        .stdout(predicate::str::contains(expected));

    assert!(!path.exists(), "manual config path should be deleted");
}

/// `-c` reports the explicit path when reset is requested for a missing config.
#[test]
fn config_reset_force_with_short_config_flag_reports_missing_manual_path() {
    let dir = tempdir().expect("temp dir should be created");
    let path = dir.path().join("missing-manual-tod.cfg");
    let expected = format!("No config file found at {}.", path.display());

    tod_command()
        .arg("-c")
        .arg(&path)
        .args(["config", "reset", "--force"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(expected));
}
