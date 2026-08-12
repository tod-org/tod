use crate::{config::Config, errors::Error, format, input, labels};
use clap::{Parser, Subcommand};

/// Label subcommands.
#[derive(Subcommand, Debug, Clone)]
pub enum LabelCommands {
    #[clap(alias = "c")]
    /// (c) Create a new personal label
    Create(Create),
}

#[derive(Parser, Debug, Clone)]
pub struct Create {
    #[arg(short, long)]
    /// Label name
    name: Option<String>,

    #[arg(short, long)]
    /// Color for the label (e.g. "red", "blue", "green")
    color: Option<String>,

    #[arg(short, long)]
    /// Display order (1-based)
    order: Option<u32>,

    #[arg(short = 'f', long, default_value_t = false)]
    /// Mark label as a favorite
    is_favorite: bool,
}

/// Creates a personal label.
pub async fn create(config: &Config, args: &Create, json: bool) -> Result<String, Error> {
    let Create {
        name,
        color,
        order,
        is_favorite,
    } = args;
    let name = super::fetch_string(name.as_deref(), config, input::NAME)?;

    let label = labels::create(config, &name, color.as_deref(), *order, *is_favorite).await?;
    if json {
        Ok(serde_json::to_string(&label)?)
    } else {
        Ok(format::green_string(&format!(
            "Label \"{}\" created",
            label.name
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn create_fails_in_json_mode_without_name() {
        let mut config = Config::default_test();
        config.args.json = true;
        let args = Create {
            name: None,
            color: None,
            order: None,
            is_favorite: false,
        };

        let error = create(&config, &args, true)
            .await
            .expect_err("creating a label without name in JSON mode should fail");

        assert_eq!(error.source, "json_mode");
    }
}
