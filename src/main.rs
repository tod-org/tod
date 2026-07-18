//! An unofficial Todoist command-line client. Takes simple input and dumps it in your inbox or another project. Takes advantage of natural language processing to assign due dates, tags, etc. Designed for single tasking in a world filled with distraction.
//!
//! Get started with `cargo install tod`
#[cfg(test)]
#[macro_use]
extern crate matches;

extern crate clap;

use clap::Parser;
use commands::Cli;
use errors::Error;
use std::{
    io::{self, Write},
    process::ExitCode,
};
use tasks::SortOrder;
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

mod cargo;
mod commands;
mod comments;
mod config;
mod debug;
mod errors;
mod filters;
mod format;
mod input;
mod labels;
mod legacy;
mod lists;
mod oauth;
mod projects;
mod regexes;
mod reminders;
mod sections;
mod shell;
mod tasks;
mod test;
mod test_time;
mod time;
mod todoist;
mod update;
mod users;

const LOWERCASE_NAME: &str = "tod";
const VERSION: &str = env!("CARGO_PKG_VERSION");

struct CommandResult {
    result: Result<String, Error>,
    bell_success: bool,
    bell_failure: bool,
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();

    // Channel for sending errors from async processes
    let (tx, mut rx) = unbounded_channel::<Error>();

    let result = run_command(cli, tx).await;

    let mut exit_code = output_result(result);

    while let Ok(error) = rx.try_recv() {
        if error.source.as_str() == "shell command" {
            exit_code = 1;
        }
        eprintln!("Error from async process: {error}");
    }

    ExitCode::from(exit_code)
}

fn output_result(result: CommandResult) -> u8 {
    match result.result {
        Ok(text) => {
            println!("{text}");
            if result.bell_success {
                terminal_bell();
            }
            0
        }
        Err(e) => {
            eprintln!("\n\n{e}");
            if result.bell_failure {
                terminal_bell();
            }
            1
        }
    }
}

async fn run_command(cli: Cli, tx: UnboundedSender<Error>) -> CommandResult {
    commands::select_command(cli, tx)
        .await
        .unwrap_or_else(|e| CommandResult {
            result: Err(e),
            bell_success: true,
            bell_failure: true,
        })
}

fn terminal_bell() {
    print!("\x07");
    io::stdout().flush().expect("failed to flush stdout");
}

#[test]
fn verify_cmd() {
    use clap::CommandFactory;
    // Mostly checks that it is not going to throw an exception because of conflicting short arguments
    Cli::try_parse().err();
    Cli::command().debug_assert();
}
