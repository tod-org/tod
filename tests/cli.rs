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

fn tod_command() -> Command {
    Command::cargo_bin("tod").expect("failed to locate tod test binary")
}

struct TempConfig {
    path: PathBuf,
}

impl TempConfig {
    fn new(label: &str) -> Self {
        Self {
            path: temp_config_path(label),
        }
    }

    fn ensure_missing(&self) {
        if self.path.exists() {
            fs::remove_file(&self.path).expect("failed to remove existing temp config");
        }
    }

    fn create_empty(&self) {
        fs::write(&self.path, "{}").expect("failed to write temp config file");
    }
}

impl Drop for TempConfig {
    fn drop(&mut self) {
        if self.path.exists() {
            let _ = fs::remove_file(&self.path);
        }
    }
}

#[test]
fn cli_help_shows_commands() {
    tod_command()
        .arg("--help")
        .assert()
        .success()
        .stdout(contains("Commands:"));
}

#[test]
fn cli_version_prints_package_version() {
    let version = env!("CARGO_PKG_VERSION");
    tod_command()
        .arg("--version")
        .assert()
        .success()
        .stdout(contains(version));
}

#[test]
fn cli_invalid_command_errors() {
    tod_command()
        .arg("not-a-command")
        .assert()
        .failure()
        .stderr(contains("error").and(contains("not-a-command")));
}

#[test]
fn cli_config_about_succeeds() {
    tod_command()
        .args(["config", "about"])
        .assert()
        .success()
        .stdout(contains("APP:").and(contains("VERSION:")));
}

#[test]
fn cli_shell_completions_bash_emits_output() {
    tod_command()
        .args(["shell", "completions", "bash"])
        .assert()
        .success()
        .stdout(contains("tod").and(contains("complete -F")));
}

#[test]
fn cli_config_reset_reports_missing_config() {
    let config_path = TempConfig::new("missing");
    config_path.ensure_missing();

    tod_command()
        .args([
            "--config",
            config_path
                .path
                .to_str()
                .expect("config path contains invalid UTF-8"),
            "config",
            "reset",
            "--force",
        ])
        .assert()
        .success()
        .stdout(
            contains("No config file found")
                .and(contains(config_path.path.display().to_string())),
        );
}

#[test]
fn cli_config_reset_deletes_existing_config() {
    let config_path = TempConfig::new("existing");
    config_path.create_empty();

    tod_command()
        .args([
            "--config",
            config_path
                .path
                .to_str()
                .expect("config path contains invalid UTF-8"),
            "config",
            "reset",
            "--force",
        ])
        .assert()
        .success()
        .stdout(
            contains("deleted successfully")
                .and(contains(config_path.path.display().to_string())),
        );

    assert!(
        !config_path.path.exists(),
        "expected config file to be deleted"
    );
}
