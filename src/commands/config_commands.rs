use clap::{Parser, Subcommand};
use std::fmt::Write;

use crate::{
    cargo::{self, Version},
    config::{self, Config},
    errors::Error,
    update,
};
use serde_json::Value;
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;

// Values pulled from Cargo.toml
const NAME: &str = env!("CARGO_PKG_NAME");
const VERSION: &str = env!("CARGO_PKG_VERSION");
// Verbose values set at build time
const BUILD_TARGET: &str = env!("BUILD_TARGET");
const BUILD_PROFILE: &str = env!("BUILD_PROFILE");
const BUILD_TIMESTAMP: &str = env!("BUILD_TIMESTAMP");

#[derive(Subcommand, Debug, Clone)]
pub enum ConfigCommands {
    #[clap(alias = "a")]
    /// (a) Get build information about Tod
    About(About),

    #[clap(alias = "v")]
    /// (v) Check to see if tod is on the latest version, returns exit code 1 if out of date. Does not need a configuration file.
    CheckVersion(CheckVersion),

    /// Validate the configuration file and optionally remove invalid values.
    Check(ConfigCheck),

    /// (r) Deletes the configuration file (if present). Errors if the file does not exist.
    #[clap(alias = "r")]
    Reset(ConfigReset),

    #[clap(alias = "o")]
    /// (o) Open the configuration file in the default editor
    Open(ConfigOpen),

    #[clap(alias = "tz")]
    /// (tz) Change the timezone in the configuration file
    SetTimezone(SetTimezone),
}
#[derive(Parser, Debug, Clone)]
pub struct CheckVersion {
    /// Automatically install the latest version if available
    #[clap(short = 'f', long)]
    pub force: bool,
    /// Manually specify the method to use for installing updates
    #[clap(long, hide = true)]
    pub repo: Option<String>,
}

#[derive(Parser, Debug, Clone)]
pub struct ConfigReset {
    /// Skip confirmation and force deletion
    #[arg(long)]
    pub force: bool,
}

#[derive(Parser, Debug, Clone)]
pub struct ConfigOpen {}

#[derive(Parser, Debug, Clone)]
pub struct ConfigCheck {}

#[derive(Parser, Debug, Clone)]
pub struct About {}

#[derive(Parser, Debug, Clone)]
pub struct SetTimezone {
    #[arg(short, long)]
    /// `TimeZone` to add, i.e. "Canada/Pacific"
    timezone: Option<String>,
}
pub async fn check_version(args: &CheckVersion, mock_url: Option<String>) -> Result<String, Error> {
    let CheckVersion { force, repo } = args;

    match cargo::compare_versions(mock_url).await {
        Ok(Version::Latest) => {
            let msg = format!("Tod is up to date with version: {VERSION}");
            Ok(msg)
        }
        Ok(Version::Dated(latest)) => {
            let msg = format!(
                "Tod is out of date. Installed version: {VERSION}, Latest version: {latest}"
            );
            let method = update::get_install_method_string(repo.as_deref());
            let upgrade_cmd = update::get_upgrade_command(repo.as_deref());
            let method_msg = format!("Detected installation method: {method}");
            if *force {
                // For testability, return the message instead of printing
                let mut result = format!("{msg}\n{method_msg}");
                match update::perform_auto_update(repo.as_deref()) {
                    Ok(_) => {
                        result.push_str("\nUpdate completed successfully.");
                        Ok(result)
                    }
                    Err(e) => {
                        let _ = write!(
                            result,
                            "\nAuto-update failed: {e}. To update manually: '{upgrade_cmd}'"
                        );
                        Ok(result)
                    }
                }
            } else {
                println!("{msg}");
                println!("{method_msg}");

                let should_update = match inquire::Confirm::new("Do you want to update?")
                    .with_default(false)
                    .prompt()
                {
                    Ok(true) => true,
                    Ok(false) => false,
                    Err(e) => {
                        println!("Could not prompt for update: {e}. To update: '{upgrade_cmd}'");
                        false
                    }
                };

                if should_update {
                    match update::perform_auto_update(repo.as_deref()) {
                        Ok(msg) => Ok(msg),
                        Err(e) => Ok(format!(
                            "Auto-update failed: {e}. To update manually: '{upgrade_cmd}'"
                        )),
                    }
                } else {
                    Ok(format!("Update skipped. To update: '{upgrade_cmd}'"))
                }
            }
        }
        Err(e) => {
            let msg = format!("Error checking version: {e}");
            Err(Error::new("config_check_version", &msg))
        }
    }
}

pub async fn check(cli_config_path: Option<PathBuf>) -> Result<String, Error> {
    check_with_prompts(
        cli_config_path,
        |message| confirm(message, false),
        |message| confirm(message, false),
    )
    .await
}

async fn check_with_prompts<R, S>(
    cli_config_path: Option<PathBuf>,
    prompt_remove: R,
    prompt_save: S,
) -> Result<String, Error>
where
    R: FnOnce(&str) -> Result<bool, Error>,
    S: FnOnce(&str) -> Result<bool, Error>,
{
    let path = resolve_config_path(cli_config_path).await?;

    if Config::load(&path).await.is_ok() {
        return Ok(format!("Config file at {} is valid.", path.display()));
    }

    let json = tokio::fs::read_to_string(&path).await?;
    let value: Value = serde_json::from_str(&json).map_err(|e| {
        Error::new(
            "config_check",
            &format!(
                "Config file at {} is invalid and could not be parsed as JSON:\n{e}",
                path.display()
            ),
        )
    })?;

    let repaired = repair_unknown_fields(value).map_err(|e| {
        Error::new(
            "config_check",
            &format!(
                "Config file at {} is invalid and could not be automatically repaired:\n{e}",
                path.display()
            ),
        )
    })?;

    if repaired.removed_fields.is_empty() {
        return Ok(format!("Config file at {} is valid.", path.display()));
    }

    let field_list = repaired.removed_fields.join(", ");
    if !prompt_remove(&format!(
        "Remove invalid config values ({field_list}) from {}?",
        path.display()
    ))? {
        return Ok("Config check aborted. No changes made.".to_string());
    }

    if !prompt_save(&format!("Save updated config file at {}?", path.display()))? {
        return Ok("Config check completed. No changes saved.".to_string());
    }

    let string = serde_json::to_string_pretty(&repaired.value)?;
    tokio::fs::OpenOptions::new()
        .write(true)
        .read(true)
        .truncate(true)
        .open(&path)
        .await?
        .write_all(string.as_bytes())
        .await?;

    Ok(format!("Removed invalid config values: {field_list}"))
}

struct RepairedConfig {
    value: Value,
    removed_fields: Vec<String>,
}

fn repair_unknown_fields(mut value: Value) -> Result<RepairedConfig, serde_json::Error> {
    let mut removed_fields = Vec::new();

    loop {
        match serde_json::from_value::<Config>(value.clone()) {
            Ok(_) => {
                removed_fields.dedup();
                return Ok(RepairedConfig {
                    value,
                    removed_fields,
                });
            }
            Err(error) => {
                let Some(field) = unknown_field(&error.to_string()) else {
                    return Err(error);
                };

                if remove_key_recursive(&mut value, &field) == 0 {
                    return Err(error);
                }

                removed_fields.push(field);
            }
        }
    }
}

fn unknown_field(error: &str) -> Option<String> {
    error
        .split_once("unknown field `")?
        .1
        .split_once('`')
        .map(|(field, _)| field.to_string())
}

fn remove_key_recursive(value: &mut Value, key: &str) -> usize {
    match value {
        Value::Object(object) => {
            let mut removed = usize::from(object.remove(key).is_some());
            for value in object.values_mut() {
                removed += remove_key_recursive(value, key);
            }
            removed
        }
        Value::Array(values) => values
            .iter_mut()
            .map(|value| remove_key_recursive(value, key))
            .sum(),
        _ => 0,
    }
}

async fn resolve_config_path(cli_config_path: Option<PathBuf>) -> Result<PathBuf, Error> {
    match cli_config_path {
        Some(path) => expand_home_dir(path),
        None => config::generate_path().await,
    }
}

fn expand_home_dir(path: PathBuf) -> Result<PathBuf, Error> {
    if let Some(str_path) = path.to_str()
        && str_path.starts_with('~')
    {
        let home =
            homedir::my_home()?.ok_or_else(|| Error::new("homedir", "Could not get homedir"))?;
        let suffix = str_path.trim_start_matches('~').trim_start_matches('/');
        return Ok(home.join(suffix));
    }

    Ok(path)
}

fn confirm(message: &str, default: bool) -> Result<bool, Error> {
    inquire::Confirm::new(message)
        .with_default(default)
        .prompt()
        .map_err(Error::from)
}

pub async fn set_timezone(config: Config, _args: &SetTimezone) -> Result<String, Error> {
    match config.set_timezone().await {
        Ok(updated_config) => {
            let tz = updated_config.get_timezone()?;
            Ok(format!("Timezone set successfully to: {tz}"))
        }
        Err(e) => Err(Error::new(
            "tz_reset",
            &format!("Could not reset timezone in config. {e}"),
        )),
    }
}

#[allow(clippy::unused_async)]
pub async fn about(_args: &About) -> Result<String, Error> {
    Ok(format!(
        "APP:             {NAME}\nVERSION:         {VERSION}\nBUILD_PROFILE:   {BUILD_PROFILE}\nBUILD_TARGET:    {BUILD_TARGET}\nBUILD_TIMESTAMP: {BUILD_TIMESTAMP}"
    ))
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::test::responses::ResponseFromFile;
    use mockito::Server;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_config_check_removes_unknown_key_when_confirmed() {
        let dir = tempdir().expect("temp dir should be created");
        let path = dir.path().join("tod.cfg");
        let contents = serde_json::json!({
            "path": path,
            "timezone": "UTC",
            "unknown_key": []
        })
        .to_string();
        tokio::fs::write(&path, contents)
            .await
            .expect("config should be written");

        let response = check_with_prompts(Some(path.clone()), |_| Ok(true), |_| Ok(true))
            .await
            .expect("config check should repair unknown config key");

        assert!(response.contains("Removed invalid config values: unknown_key"));
        let updated = tokio::fs::read_to_string(&path)
            .await
            .expect("updated config should be readable");
        assert!(!updated.contains("unknown_key"));
        Config::load(&path)
            .await
            .expect("updated config should validate");
    }

    #[tokio::test]
    async fn test_config_check_does_not_save_when_declined() {
        let dir = tempdir().expect("temp dir should be created");
        let path = dir.path().join("tod.cfg");
        let contents = serde_json::json!({
            "path": path,
            "timezone": "UTC",
            "unknown_key": []
        })
        .to_string();
        tokio::fs::write(&path, &contents)
            .await
            .expect("config should be written");

        let response = check_with_prompts(Some(path.clone()), |_| Ok(true), |_| Ok(false))
            .await
            .expect("config check should complete without saving");

        assert_eq!(response, "Config check completed. No changes saved.");
        let unchanged = tokio::fs::read_to_string(&path)
            .await
            .expect("config should be readable");
        assert_eq!(unchanged, contents);
    }

    #[tokio::test]
    async fn test_config_check_version_outdated() {
        // Start mock server
        let mut server = Server::new_async().await;

        // Mock the crates.io versions endpoint
        let mock = server
            .mock("GET", "/v1/crates/tod/versions")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                ResponseFromFile::Versions
                    .read_with_version("999.99.999")
                    .await,
            )
            .create_async()
            .await;

        let args = CheckVersion {
            force: true,
            repo: None,
        };

        // Run the version check
        let response = check_version(&args, Some(server.url()))
            .await
            .expect("Expected version check to succeed");

        // Print full output for debugging if test fails
        println!("Version check output:\n{response}");

        // Assertions — robust against changing installed version
        assert!(
            response.contains("Tod is out of date"),
            "Missing 'Tod is out of date' message"
        );
        assert!(
            response.contains("Installed version:"),
            "Missing installed version line"
        );
        assert!(
            response.contains("Latest version: 999.99.999"),
            "Missing latest version string"
        );
        assert!(
            response.contains("Detected installation method:"),
            "Missing installation method detection"
        );
        assert!(
            response.contains("Auto-update failed:"),
            "Missing auto-update failure notice"
        );
        assert!(
            response.contains("https://github.com/tod-org/tod#installation"),
            "Missing manual update link"
        );

        // Ensure the mock was actually called
        mock.assert();
    }
}
