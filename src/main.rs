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
use std::io::Write;
use tasks::SortOrder;

mod cargo;
mod color;
mod commands;
mod comments;
mod config;
mod debug;
mod errors;
mod filters;
mod id;
mod input;
mod labels;
mod lists;
mod oauth;
mod projects;
mod sections;
mod shell;
mod tasks;
mod test;
mod test_time;
mod time;
mod todoist;
mod update;
mod users;
// Values pulled from Cargo.toml
const NAME: &str = env!("CARGO_PKG_NAME");
const LOWERCASE_NAME: &str = "tod";
const VERSION: &str = env!("CARGO_PKG_VERSION");
// Verbose values set at build time
const BUILD_TARGET: &str = env!("BUILD_TARGET");
const BUILD_PROFILE: &str = env!("BUILD_PROFILE");
const BUILD_TIMESTAMP: &str = env!("BUILD_TIMESTAMP");

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // Channel for sending errors from async processes
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Error>();

    let (bell_success, bell_error, result) = commands::select_command(cli, tx).await;
    while let Some(e) = rx.recv().await {
        eprintln!("Error from async process: {e}");
    }

    match result {
        Ok(text) => {
            if bell_success {
                terminal_bell()
            }
            println!("{text}");
            std::process::exit(0);
        }
        Err(e) => {
            if bell_error {
                terminal_bell()
            }
            eprintln!("\n\n{e}");
            std::process::exit(1);
        }
    }
}

fn terminal_bell() {
    print!("\x07");
    std::io::stdout().flush().unwrap();
}

pub fn long_version() -> String {
    format!("{NAME} ({VERSION}, {BUILD_PROFILE}, {BUILD_TARGET}, {BUILD_TIMESTAMP})")
}

#[test]
fn verify_cmd() {
    use clap::CommandFactory;
    // Mostly checks that it is not going to throw an exception because of conflicting short arguments
    Cli::try_parse().err();
    Cli::command().debug_assert();
}
