use clap::{Parser, Subcommand};

use crate::{
    errors::Error,
    shell::{self, Shell},
};

#[derive(Subcommand, Debug, Clone)]
pub enum ShellCommands {
    #[clap(alias = "b")]
    /// (b) Generate shell completions for various shells. Does not need a configuration file
    Completions(Completions),
}

#[derive(Parser, Debug, Clone)]
pub struct Completions {
    shell: Shell,
}

pub async fn completions(args: &Completions) -> Result<String, Error> {
    shell::generate_completions(args.shell);

    Ok(String::new())
}
