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
/// Creates the config file at `config_path` (or the platform default) if it does not yet exist,
/// then fetches and saves the account timezone with the provided token.
pub async fn token(config_path: Option<PathBuf>, args: &Token) -> Result<String, Error> {
    let path = config::resolve_config_path(config_path).await?;

    let config = match tokio::fs::metadata(&path).await {
        Err(e) if e.kind() == ErrorKind::NotFound => {
            let config = Config::new(None, path.clone()).await?;
            config.touch_file().await?;
            config
        }
        Err(e) => return Err(e.into()),
        Ok(_) => Config::load(&path).await?,
    };

    config.set_developer_token(&args.key).await?;
    Ok(color::green_string(&format!(
        "✓ API token saved to {}",
        path.display()
    )))
}

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_save_developer_token_updates_existing_config_token() {
        let dir = tempdir().expect("temp dir should be created");
        let path = dir.path().join("tod.cfg");
        let mut config = Config::new(None, path.clone())
            .await
            .expect("config should be created")
            .with_timezone("UTC");
        config
            .touch_file()
            .await
            .expect("config file should be created");
        config.save().await.expect("config should save");

        config
            .set_developer_token("new-token")
            .await
            .expect("token update should succeed");

        let saved = Config::load(&path)
            .await
            .expect("config should be readable");
        assert_eq!(saved.token, Some("new-token".to_string()));
        assert_eq!(saved.get_timezone().expect("timezone should remain"), "UTC");
    }

    #[tokio::test]
    async fn test_token_rejects_empty_or_whitespace_key() {
        let dir = tempdir().expect("temp dir should be created");
        let path = dir.path().join("tod.cfg");
        let config = Config::new(None, path)
            .await
            .expect("config should be created");

        let error = config
            .set_developer_token("   ")
            .await
            .expect_err("empty token should be rejected");
        assert_eq!(error.source, "auth token");
        assert!(
            error.message.contains("cannot be empty or whitespace"),
            "error message should explain empty/whitespace rejection"
        );
    }
}
