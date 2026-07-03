use clap::{Parser, Subcommand};

use crate::reminders;
use crate::{config::Config, errors::Error};

#[derive(Subcommand, Debug, Clone)]
pub enum ReminderCommands {
    #[clap(alias = "l")]
    /// (l) List all reminders from the API
    List(List),
}

#[derive(Parser, Debug, Clone)]
pub struct List {}

pub async fn list(config: &mut Config, _args: &List) -> Result<String, Error> {
    reminders::list(config).await
}
