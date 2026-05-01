//File for shell functions used local to the system, such as command execution, shell completions.
use crate::{Cli, LOWERCASE_NAME, errors::Error};
use clap::CommandFactory;
use std::{io, process::Stdio};
use tokio::{process::Command, sync::mpsc::UnboundedSender};

#[derive(clap::ValueEnum, Debug, Copy, Clone)]
pub enum Shell {
    Bash,
    Zsh,
    Fish,
    #[allow(clippy::enum_variant_names)]
    PowerShell,
    Elvish,
}

/// Starts a local system command in the background and reports failures through tx.
/// Suppresses stdout so command output cannot interfere with terminal rendering.
pub fn execute_command(command: &str, tx: UnboundedSender<Error>) {
    let command = command.to_string();
    tokio::spawn(async move {
        if let Err(error) = execute_command_inner(&command).await {
            let _ = tx.send(error);
        }
    });
}

async fn execute_command_inner(command: &str) -> Result<(), Error> {
    let output = shell_command(command)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| {
            Error::new(
                "shell command",
                &format!("Failed to execute '{command}': {e}"),
            )
        })?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stderr = stderr.trim();
    let message = if stderr.is_empty() {
        format!("Command '{command}' failed with status {}", output.status)
    } else {
        format!("Command '{command}' failed: {stderr}")
    };

    Err(Error::new("shell command", &message))
}

fn shell_command(command: &str) -> Command {
    if cfg!(windows) {
        let mut child = Command::new("cmd");
        child.args(["/C", command]);
        child
    } else {
        let mut child = Command::new("sh");
        child.args(["-c", command]);
        child
    }
}

pub(crate) fn generate_completions(shell: Shell) {
    let mut cli = Cli::command();

    match shell {
        Shell::Bash => {
            let shell = clap_complete::shells::Bash;
            clap_complete::generate(shell, &mut cli, LOWERCASE_NAME, &mut io::stdout());
        }
        Shell::Fish => {
            let shell = clap_complete::shells::Fish;
            clap_complete::generate(shell, &mut cli, LOWERCASE_NAME, &mut io::stdout());
        }
        Shell::Zsh => {
            let shell = clap_complete::shells::Zsh;
            clap_complete::generate(shell, &mut cli, LOWERCASE_NAME, &mut io::stdout());
        }
        Shell::PowerShell => {
            let shell = clap_complete::shells::PowerShell;
            clap_complete::generate(shell, &mut cli, LOWERCASE_NAME, &mut io::stdout());
        }
        Shell::Elvish => {
            let shell = clap_complete::shells::Elvish;
            clap_complete::generate(shell, &mut cli, LOWERCASE_NAME, &mut io::stdout());
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_cmd::Command;
    use predicates::prelude::*;
    // Contains is used to make CMD test cases cross-platform compatible
    use predicates::str::contains;
    use tokio::sync::mpsc::unbounded_channel;
    use tokio::time::{Duration, timeout};

    #[tokio::test]
    async fn test_execute_command_success() {
        // This should succeed and produce no stderr output.
        let (tx, mut rx) = unbounded_channel();
        execute_command("echo 'Hello, world!'", tx);

        assert!(
            timeout(Duration::from_secs(1), rx.recv())
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn test_execute_command_invalid_command_reports_error() {
        let (tx, mut rx) = unbounded_channel();
        execute_command("nonexistent_command_12345", tx);

        let error = timeout(Duration::from_secs(1), rx.recv())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(error.source, "shell command");
        assert!(error.message.contains("nonexistent_command_12345"));
    }

    #[tokio::test]
    async fn test_execute_command_with_stderr_reports_error() {
        let (tx, mut rx) = unbounded_channel();
        execute_command("ls /nonexistent_directory", tx);

        let error = timeout(Duration::from_secs(1), rx.recv())
            .await
            .unwrap()
            .unwrap();

        assert!(error.message.contains("/nonexistent_directory"));
    }

    #[tokio::test]
    async fn test_execute_command_failure_without_stderr_reports_error() {
        let (tx, mut rx) = unbounded_channel();
        execute_command("exit 1", tx);

        let error = timeout(Duration::from_secs(1), rx.recv())
            .await
            .unwrap()
            .unwrap();

        assert!(error.message.contains("failed with status"));
    }

    #[tokio::test]
    async fn test_execute_command_does_not_wait_for_command_completion() {
        let start = std::time::Instant::now();
        let (tx, _rx) = unbounded_channel();
        execute_command("sleep 1", tx);

        assert!(start.elapsed() < std::time::Duration::from_millis(500));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_execute_command_non_utf8_stderr_reports_error() {
        let (tx, mut rx) = unbounded_channel();
        execute_command("printf '\\377' >&2; exit 1", tx);

        let error = timeout(Duration::from_secs(1), rx.recv())
            .await
            .unwrap()
            .unwrap();

        assert!(error.message.contains("Command 'printf"));
    }
    #[tokio::test]
    async fn test_command_echo_output() {
        let mut cmd = if cfg!(windows) {
            let mut c = Command::new("cmd");
            c.args(["/C", "echo Hello"]);
            c
        } else {
            let mut c = Command::new("sh");
            c.args(["-c", "echo Hello"]);
            c
        };

        cmd.assert().success().stdout(contains("Hello"));
    }

    #[tokio::test]
    async fn test_known_command_fails() {
        let mut cmd = if cfg!(windows) {
            let mut c = Command::new("cmd");
            c.args(["/C", "exit 1"]);
            c
        } else {
            let mut c = Command::new("sh");
            c.args(["-c", "exit 1"]);
            c
        };

        cmd.assert().failure();
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn test_command_stderr_output_windows() {
        let mut cmd = Command::new("cmd");
        cmd.args(["/C", "dir C:\\nonexistent_dir"]);

        cmd.assert().failure().stderr(contains("File Not Found"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_command_stderr_output_unix() {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "ls /nonexistent_directory"]);

        cmd.assert()
            .failure()
            .stderr(contains("No such").or(contains("cannot find")));
    }
}
