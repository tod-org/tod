use crate::{
    color,
    config::{self, Config},
    errors::Error,
    oauth,
};
use clap::{Parser, Subcommand};
use std::{io::ErrorKind, path::PathBuf};

#[derive(Subcommand, Debug, Clone)]
pub enum AuthCommands {
    #[clap(alias = "l")]
    /// (l) Log into Todoist using OAuth
    Login(Login),

    #[clap(alias = "t")]
    /// (t) Save a Todoist developer API token directly to the config (non-interactive)
    Token(Token),
}

#[derive(Parser, Debug, Clone)]
pub struct Login {}

#[derive(Parser, Debug, Clone)]
pub struct Token {
    /// Todoist developer API token from https://todoist.com/prefs/integrations
    key: String,
}

pub async fn login(config: &mut Config, _args: &Login) -> Result<String, Error> {
    oauth::login(config, None).await
}

/// Saves the given Todoist API token to the config without any interactive prompts.
///
/// Creates the config file at `config_path` (or the platform default) if it does not yet exist.
/// Timezone and other settings are left at their defaults and will be filled in automatically
/// on the next command that contacts the Todoist API.
pub async fn token(config_path: Option<PathBuf>, args: &Token) -> Result<String, Error> {
    let Token { key } = args;
    let trimmed_key = key.trim();
    if trimmed_key.is_empty() {
        return Err(Error::new(
            "auth token",
            "Todoist API token cannot be empty or whitespace",
        ));
    }
    let path = config::resolve_path_or_default(config_path).await?;

    let mut config = match tokio::fs::metadata(&path).await {
        Err(e) if e.kind() == ErrorKind::NotFound => {
            // Config doesn't exist yet — create a blank file with default settings.
            // No interactive prompts are triggered here.
            Config::new(None, path.clone()).await?.create().await?
        }
        Err(e) => return Err(e.into()),
        Ok(_) => Config::load(&path).await?,
    };

    config.set_token(trimmed_key.to_string()).await?;
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
    async fn test_token_creates_config_and_saves_token() {
        let dir = tempdir().expect("temp dir should be created");
        let path = dir.path().join("tod.cfg");

        let args = Token {
            key: "test-api-token-123".to_string(),
        };

        let result = token(Some(path.clone()), &args)
            .await
            .expect("token command should succeed");

        assert!(result.contains("✓"), "should return success checkmark");

        let config = Config::load(&path)
            .await
            .expect("config should be readable after token command");
        assert_eq!(
            config.token,
            Some("test-api-token-123".to_string()),
            "token should be saved in config"
        );
    }

    #[tokio::test]
    async fn test_token_updates_existing_config_token() {
        let dir = tempdir().expect("temp dir should be created");
        let path = dir.path().join("tod.cfg");

        // Create a config with an initial token
        let initial = Token {
            key: "old-token".to_string(),
        };
        token(Some(path.clone()), &initial)
            .await
            .expect("first token call should succeed");

        // Update with a new token
        let updated = Token {
            key: "new-token".to_string(),
        };
        token(Some(path.clone()), &updated)
            .await
            .expect("second token call should succeed");

        let config = Config::load(&path)
            .await
            .expect("config should be readable");
        assert_eq!(
            config.token,
            Some("new-token".to_string()),
            "token should be updated in config"
        );
    }

    #[tokio::test]
    async fn test_token_rejects_empty_or_whitespace_key() {
        let dir = tempdir().expect("temp dir should be created");
        let path = dir.path().join("tod.cfg");

        let empty = Token {
            key: "   ".to_string(),
        };

        let error = token(Some(path), &empty)
            .await
            .expect_err("empty token should be rejected");
        assert_eq!(error.source, "auth token");
        assert!(
            error.message.contains("cannot be empty or whitespace"),
            "error message should explain empty/whitespace rejection"
        );
    }
}
