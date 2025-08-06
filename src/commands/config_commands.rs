use clap::{Parser, Subcommand};

use crate::{
    cargo::{self, Version},
    config::Config,
    errors::Error,
    update,
};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Subcommand, Debug, Clone)]
pub enum ConfigCommands {
    #[clap(alias = "v")]
    /// (v) Check to see if tod is on the latest version, returns exit code 1 if out of date. Does not need a configuration file.
    CheckVersion(CheckVersion),
    /// (r) Deletes the configuration file (if present). Errors if the file does not exist.
    #[clap(alias = "r")]
    Reset(ConfigReset),

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
pub struct SetTimezone {
    #[arg(short, long)]
    /// TimeZone to add, i.e. "Canada/Pacific"
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
            let method = update::get_install_method_string(repo);
            let upgrade_cmd = update::get_upgrade_command(repo);
            let method_msg = format!("Detected installation method: {method}");
            if *force {
                // For testability, return the message instead of printing
                let mut result = format!("{msg}\n{method_msg}");
                match update::perform_auto_update(repo) {
                    Ok(_) => {
                        result.push_str("\nUpdate completed successfully.");
                        Ok(result)
                    }
                    Err(e) => {
                        result.push_str(&format!(
                            "\nAuto-update failed: {e}. To update manually: '{upgrade_cmd}'"
                        ));
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
                    match update::perform_auto_update(repo) {
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

#[cfg(test)]
mod tests {

    use super::*;
    use crate::test::responses::ResponseFromFile;
    use mockito::Server;

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
