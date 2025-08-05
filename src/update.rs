// This file contains the functions used for checking for updates and automatically updating the tod CLI tool.
// Functions that attempt to detect the installation method of the current executable, used for autoupdate and debug
use std::{env, process::Command};

#[derive(Debug, PartialEq, Eq)]
pub enum InstallMethod {
    Homebrew,
    Scoop,
    Cargo,
    FromSource,
    Unknown,
}

// Returns the detected install method (or overridden if manually specified)
pub fn get_install_method(override_arg: &Option<String>) -> InstallMethod {
    if let Some(value) = override_arg {
        match value.trim().to_lowercase().as_str() {
            "cargo" => InstallMethod::Cargo,
            "scoop" => InstallMethod::Scoop,
            "homebrew" => InstallMethod::Homebrew,
            "source" | "fromsource" => InstallMethod::FromSource,
            _ => InstallMethod::Unknown,
        }
    } else {
        detect_install_method()
    }
}
// Returns the string name of how software is installed
pub fn get_install_method_string(override_arg: &Option<String>) -> &'static str {
    match get_install_method(override_arg) {
        InstallMethod::Homebrew => "homebrew",
        InstallMethod::Scoop => "scoop",
        InstallMethod::Cargo => "cargo",
        InstallMethod::FromSource => "from source",
        InstallMethod::Unknown => "unknown",
    }
}
// Returns the upgrade instruction (based on installation method)
pub fn get_update_command_args(
    override_arg: &Option<String>,
) -> Result<(&'static str, Vec<&'static str>), String> {
    match get_install_method(override_arg) {
        InstallMethod::Homebrew => Ok(("brew", vec!["upgrade", "tod"])),
        InstallMethod::Scoop => Ok(("scoop", vec!["update", "tod"])),
        InstallMethod::Cargo => Ok(("cargo", vec!["install", "tod", "--force"])),
        InstallMethod::FromSource | InstallMethod::Unknown => {
            let url = "https://github.com/tod-org/tod#installation";
            Err(format!(
                "Automatic update is not supported for this installation method.\nPlease visit: {url}"
            ))
        }
    }
}
pub fn perform_auto_update(override_arg: &Option<String>) -> Result<String, String> {
    let cmd = get_update_command_args(override_arg)?;
    let command_str = format!("{} {}", cmd.0, cmd.1.join(" "));
    println!("Executing command.... {command_str}");

    let status = Command::new(cmd.0)
        .args(&cmd.1)
        .status()
        .map_err(|e| format!("Failed to execute '{}': {}", cmd.0, e))?;

    if status.success() {
        Ok("Upgraded successfully!".into())
    } else {
        let upgrade_cmd = get_upgrade_command(override_arg);
        Err(format!(
            "Automatic update failed. Please run '{upgrade_cmd}' manually."
        ))
    }
}

// Returns the upgrade command as a string for manual use
pub fn get_upgrade_command(override_arg: &Option<String>) -> String {
    match get_install_method(override_arg) {
        InstallMethod::Homebrew => "brew upgrade tod".to_string(),
        InstallMethod::Scoop => "scoop update tod".to_string(),
        InstallMethod::Cargo => "cargo install tod --force".to_string(),
        InstallMethod::FromSource | InstallMethod::Unknown => {
            "https://github.com/tod-org/tod#installation".to_string()
        }
    }
}

fn detect_install_method() -> InstallMethod {
    let path = match env::current_exe() {
        Ok(p) => p,
        Err(_) => return InstallMethod::Unknown,
    };

    let components: Vec<_> = path
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_lowercase())
        .collect();

    if cfg!(debug_assertions) || components.iter().any(|c| c == "target") {
        InstallMethod::FromSource
    } else if components.iter().any(|c| c.contains(".cargo")) {
        InstallMethod::Cargo
    } else if components.iter().any(|c| c.contains("scoop")) {
        InstallMethod::Scoop
    } else if components.iter().any(|c| c.contains("homebrew")) {
        InstallMethod::Homebrew
    } else {
        InstallMethod::Unknown
    }
}

#[cfg(test)]
mod tests {
    use crate::{ConfigCheckVersion, config_check_version, test::responses::ResponseFromFile};

    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_get_install_method_override() {
        assert_eq!(
            get_install_method(&Some("cargo".into())),
            InstallMethod::Cargo
        );
        assert_eq!(
            get_install_method(&Some("scoop".into())),
            InstallMethod::Scoop
        );
        assert_eq!(
            get_install_method(&Some("homebrew".into())),
            InstallMethod::Homebrew
        );
        assert_eq!(
            get_install_method(&Some("source".into())),
            InstallMethod::FromSource
        );
        assert_eq!(
            get_install_method(&Some("unknown".into())),
            InstallMethod::Unknown
        );
        assert_eq!(get_install_method(&None), detect_install_method());
    }

    #[test]
    fn test_get_install_method_string() {
        assert_eq!(get_install_method_string(&Some("cargo".into())), "cargo");
        assert_eq!(get_install_method_string(&Some("scoop".into())), "scoop");
        assert_eq!(
            get_install_method_string(&Some("homebrew".into())),
            "homebrew"
        );
        assert_eq!(
            get_install_method_string(&Some("source".into())),
            "from source"
        );
        assert_eq!(
            get_install_method_string(&Some("unknown".into())),
            "unknown"
        );
    }

    #[test]
    fn test_get_upgrade_command() {
        assert_eq!(
            get_upgrade_command(&Some("cargo".into())),
            "cargo install tod --force"
        );
        assert_eq!(
            get_upgrade_command(&Some("scoop".into())),
            "scoop update tod"
        );
        assert_eq!(
            get_upgrade_command(&Some("homebrew".into())),
            "brew upgrade tod"
        );
        assert_eq!(
            get_upgrade_command(&Some("source".into())),
            "https://github.com/tod-org/tod#installation"
        );
        assert_eq!(
            get_upgrade_command(&Some("unknown".into())),
            "https://github.com/tod-org/tod#installation"
        );
    }

    #[tokio::test]
    async fn test_config_check_version_outdated() {
        use mockito::Server;

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

        let args = ConfigCheckVersion {
            force: true,
            repo: None,
        };

        // Run the version check
        let response = config_check_version(&args, Some(server.url()))
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

    #[test]
    fn test_get_update_command_args_homebrew() {
        let cmd = get_update_command_args(&Some("homebrew".into())).unwrap();
        assert_eq!(cmd.0, "brew");
        assert_eq!(cmd.1, vec!["upgrade", "tod"]);
    }

    #[test]
    fn test_get_update_command_args_scoop() {
        let cmd = get_update_command_args(&Some("scoop".into())).unwrap();
        assert_eq!(cmd.0, "scoop");
        assert_eq!(cmd.1, vec!["update", "tod"]);
    }

    #[test]
    fn test_get_update_command_args_cargo() {
        let cmd = get_update_command_args(&Some("cargo".into())).unwrap();
        assert_eq!(cmd.0, "cargo");
        assert_eq!(cmd.1, vec!["install", "tod", "--force"]);
    }

    #[test]
    fn test_get_update_command_args_from_source() {
        let err = get_update_command_args(&Some("source".into())).unwrap_err();
        assert!(err.contains("Automatic update is not supported"));
    }

    #[test]
    fn test_get_update_command_args_unknown() {
        let err = get_update_command_args(&Some("unknown".into())).unwrap_err();
        assert!(err.contains("Automatic update is not supported"));
    }
    #[test]
    fn test_get_install_method_override_whitespace_case() {
        assert_eq!(
            get_install_method(&Some("  CaRgO  ".into())),
            InstallMethod::Cargo
        );
    }
    #[test]
    fn test_get_install_method_override_random() {
        assert_eq!(
            get_install_method(&Some("foobar".into())),
            InstallMethod::Unknown
        );
    }
    #[test]
    fn test_get_update_command_args_none() {
        let result = get_update_command_args(&None);
        assert!(
            result.is_ok()
                || result
                    .unwrap_err()
                    .contains("Automatic update is not supported")
        );
    }
}
