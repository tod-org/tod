use assert_cmd::Command;
use predicates::prelude::*;
use predicates::str::contains;
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

fn temp_config_path(label: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("tod-{label}-{}.cfg", Uuid::new_v4()));
    path
}

fn create_empty_config(path: &PathBuf) {
    fs::write(path, "{}").expect("failed to write temp config file");
}

#[test]
fn cli_help_shows_commands() {
    Command::cargo_bin("tod")
        .expect("failed to find tod binary")
        .arg("--help")
        .assert()
        .success()
        .stdout(contains("Commands:"));
}

#[test]
fn cli_version_prints_package_version() {
    let version = env!("CARGO_PKG_VERSION");
    Command::cargo_bin("tod")
        .expect("failed to find tod binary")
        .arg("--version")
        .assert()
        .success()
        .stdout(contains(version));
}

#[test]
fn cli_invalid_command_errors() {
    Command::cargo_bin("tod")
        .expect("failed to find tod binary")
        .arg("not-a-command")
        .assert()
        .failure()
        .stderr(contains("error").and(contains("not-a-command")));
}

#[test]
fn cli_config_about_succeeds() {
    Command::cargo_bin("tod")
        .expect("failed to find tod binary")
        .args(["config", "about"])
        .assert()
        .success()
        .stdout(contains("APP:").and(contains("VERSION:")));
}

#[test]
fn cli_shell_completions_bash_emits_output() {
    Command::cargo_bin("tod")
        .expect("failed to find tod binary")
        .args(["shell", "completions", "bash"])
        .assert()
        .success()
        .stdout(contains("tod").and(contains("complete").or(contains("_tod"))));
}

#[test]
fn cli_config_reset_reports_missing_config() {
    let config_path = temp_config_path("missing");
    if config_path.exists() {
        fs::remove_file(&config_path).expect("failed to remove existing temp config");
    }

    Command::cargo_bin("tod")
        .expect("failed to find tod binary")
        .args([
            "--config",
            config_path
                .to_str()
                .expect("failed to render config path"),
            "config",
            "reset",
            "--force",
        ])
        .assert()
        .success()
        .stdout(
            contains("No config file found")
                .and(contains(config_path.display().to_string())),
        );
}

#[test]
fn cli_config_reset_deletes_existing_config() {
    let config_path = temp_config_path("existing");
    create_empty_config(&config_path);

    Command::cargo_bin("tod")
        .expect("failed to find tod binary")
        .args([
            "--config",
            config_path
                .to_str()
                .expect("failed to render config path"),
            "config",
            "reset",
            "--force",
        ])
        .assert()
        .success()
        .stdout(
            contains("deleted successfully").and(contains(config_path.display().to_string())),
        );

    assert!(
        !config_path.exists(),
        "expected config file to be deleted"
    );
}
