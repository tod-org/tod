use crate::{color, config::{self, Config}, errors::Error, oauth};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Subcommand, Debug, Clone)]
pub enum AuthCommands {
    #[clap(alias = "l")]
    /// (l) Log into Todoist using OAuth
    Login(Login),

    #[clap(alias = "a")]
    /// (a) Save a Todoist developer API token directly to the config (non-interactive)
    Api(Api),
}

#[derive(Parser, Debug, Clone)]
pub struct Login {}

#[derive(Parser, Debug, Clone)]
pub struct Api {
    #[arg(short, long)]
    /// Todoist developer API token from https://todoist.com/prefs/integrations
    token: String,
}

pub async fn login(config: &mut Config, _args: &Login) -> Result<String, Error> {
    oauth::login(config, None).await
}

/// Saves the given Todoist API token to the config without any interactive prompts.
///
/// Creates the config file at `config_path` (or the platform default) if it does not yet exist.
/// Timezone and other settings are left at their defaults and will be filled in automatically
/// on the next command that contacts the Todoist API.
pub async fn api(config_path: Option<PathBuf>, args: &Api) -> Result<String, Error> {
    let Api { token } = args;
    let path = config::resolve_path_or_default(config_path).await?;

    let mut config = match Config::load(&path).await {
        Ok(existing) => existing,
        Err(_) => {
            // Config doesn't exist yet — create a blank file with default settings.
            // No interactive prompts are triggered here.
            Config::new(None, path.clone()).await?.create().await?
        }
    };

    config.set_token(token.clone()).await?;
    Ok(color::green_string(&format!(
        "✓ API token saved to {}",
        path.display()
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_api_creates_config_and_saves_token() {
        let dir = tempdir().expect("temp dir should be created");
        let path = dir.path().join("tod.cfg");

        let args = Api {
            token: "test-api-token-123".to_string(),
        };

        let result = api(Some(path.clone()), &args)
            .await
            .expect("api command should succeed");

        assert!(result.contains("✓"), "should return success checkmark");

        let config = Config::load(&path)
            .await
            .expect("config should be readable after api command");
        assert_eq!(
            config.token,
            Some("test-api-token-123".to_string()),
            "token should be saved in config"
        );
    }

    #[tokio::test]
    async fn test_api_updates_existing_config_token() {
        let dir = tempdir().expect("temp dir should be created");
        let path = dir.path().join("tod.cfg");

        // Create a config with an initial token
        let initial = Api {
            token: "old-token".to_string(),
        };
        api(Some(path.clone()), &initial)
            .await
            .expect("first api call should succeed");

        // Update with a new token
        let updated = Api {
            token: "new-token".to_string(),
        };
        api(Some(path.clone()), &updated)
            .await
            .expect("second api call should succeed");

        let config = Config::load(&path)
            .await
            .expect("config should be readable");
        assert_eq!(
            config.token,
            Some("new-token".to_string()),
            "token should be updated in config"
        );
    }
}
