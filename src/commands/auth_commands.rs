use crate::{config::Config, errors::Error, oauth};
use clap::{Parser, Subcommand};

#[derive(Subcommand, Debug, Clone)]
pub enum AuthCommands {
    #[clap(alias = "l")]
    /// (l) Log into Todoist using OAuth
    Login(Login),
}

#[derive(Parser, Debug, Clone)]
pub struct Login {}

pub async fn login(config: &mut Config, _args: &Login) -> Result<String, Error> {
    oauth::login(config, None).await
}
