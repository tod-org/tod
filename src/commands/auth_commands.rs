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

pub async fn login(config: Config, _args: &Login) -> Result<String, Error> {
    let mut config = config;
    oauth::login(&mut config, None).await
}
