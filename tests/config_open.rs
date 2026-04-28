use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

fn noop_editor() -> &'static str {
    if cfg!(windows) {
        "cmd.exe /C rem"
    } else {
        "true"
    }
}

fn tod_command() -> Command {
    let mut command = Command::cargo_bin("tod").expect("tod binary should build");
    command.env("VISUAL", noop_editor());
    command.env("EDITOR", noop_editor());
    command
}

#[test]
fn config_open_opens_existing_config_and_validates() {
    let dir = tempdir().expect("temp dir should be created");
    let path = dir.path().join("tod.cfg");
    fs::write(&path, "{}").expect("config should be written");

    tod_command()
        .arg("--config")
        .arg(&path)
        .args(["config", "open"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "opened and validated successfully",
        ));
}

#[test]
fn config_open_alias_opens_existing_config_and_validates() {
    let dir = tempdir().expect("temp dir should be created");
    let path = dir.path().join("tod.cfg");
    fs::write(&path, "{}").expect("config should be written");

    tod_command()
        .arg("--config")
        .arg(&path)
        .args(["config", "o"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "opened and validated successfully",
        ));
}

#[test]
fn config_open_fails_when_config_is_invalid_after_editor_exits() {
    let dir = tempdir().expect("temp dir should be created");
    let path = dir.path().join("tod.cfg");
    fs::write(&path, "{ invalid").expect("config should be written");

    tod_command()
        .arg("--config")
        .arg(&path)
        .args(["config", "open"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Error loading configuration file"));
}
