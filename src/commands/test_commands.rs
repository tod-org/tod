use clap::{Parser, Subcommand};

use crate::{config::Config, errors::Error, todoist};

#[derive(Subcommand, Debug, Clone)]
pub enum TestCommands {
    #[clap(alias = "a")]
    /// (a) Hit all API endpoints
    All(All),
}

#[derive(Parser, Debug, Clone)]
pub struct All {}

pub async fn all(config: Config, _args: &All) -> Result<String, Error> {
    todoist::test_all_endpoints(config).await
}
