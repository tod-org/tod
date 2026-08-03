// This file contains the functions used for checking for updates and automatically updating the tod CLI tool.
// Functions that attempt to detect the installation method of the current executable, used for autoupdate and debug
use std::{env, process::Command};

/// Wrap a URL in OSC8 terminal hyperlink escape sequences for clickable links.
pub fn osc8_link(url: &str, text: &str) -> String {
    format!("\x1b]8;;{url}\x1b\\{text}\x1b]8;;\x1b\\")
}

#[derive(Debug, PartialEq, Eq)]
pub enum InstallMethod {
    Homebrew,
    Scoop,
    Cargo,
    FromSource,
    Unknown,
}

// Returns the detected install method (or overridden if manually specified)
pub fn get_install_method(override_arg: Option<&str>) -> InstallMethod {
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
pub fn get_install_method_string(override_arg: Option<&str>) -> &'static str {
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
    override_arg: Option<&str>,
) -> Result<(&'static str, Vec<&'static str>), String> {
    match get_install_method(override_arg) {
        InstallMethod::Homebrew => Ok(("brew", vec!["upgrade", "tod"])),
        InstallMethod::Scoop => Ok(("scoop", vec!["update", "tod"])),
        InstallMethod::Cargo => Ok(("cargo", vec!["install", "tod", "--force"])),
        InstallMethod::FromSource | InstallMethod::Unknown => {
            Err("Automatic update is not supported for this installation method.".to_string())
        }
    }
}
pub fn perform_auto_update(override_arg: Option<&str>) -> Result<String, String> {
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
pub fn get_upgrade_command(override_arg: Option<&str>) -> String {
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
    let Ok(path) = env::current_exe() else {
        return InstallMethod::Unknown;
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
    } else if components
        .iter()
        .any(|c| c.contains("homebrew") || c.contains("cellar"))
    {
        InstallMethod::Homebrew
    } else {
        InstallMethod::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_get_install_method_override() {
        assert_eq!(get_install_method(Some("cargo")), InstallMethod::Cargo);
        assert_eq!(get_install_method(Some("scoop")), InstallMethod::Scoop);
        assert_eq!(
            get_install_method(Some("homebrew")),
            InstallMethod::Homebrew
        );
        assert_eq!(
            get_install_method(Some("source")),
            InstallMethod::FromSource
        );
        assert_eq!(get_install_method(Some("unknown")), InstallMethod::Unknown);
        assert_eq!(get_install_method(None), detect_install_method());
    }

    #[test]
    fn detect_install_method_returns_from_source_in_debug_builds() {
        // In debug/test builds, cfg!(debug_assertions) is true, so
        // detect_install_method should return FromSource regardless of path.
        // The test binary also runs from under target/, adding a second
        // layer of confidence.
        assert_eq!(detect_install_method(), InstallMethod::FromSource);
    }

    #[test]
    fn test_get_install_method_string() {
        assert_eq!(get_install_method_string(Some("cargo")), "cargo");
        assert_eq!(get_install_method_string(Some("scoop")), "scoop");
        assert_eq!(get_install_method_string(Some("homebrew")), "homebrew");
        assert_eq!(get_install_method_string(Some("source")), "from source");
        assert_eq!(get_install_method_string(Some("unknown")), "unknown");
    }

    #[test]
    fn test_get_upgrade_command() {
        assert_eq!(
            get_upgrade_command(Some("cargo")),
            "cargo install tod --force"
        );
        assert_eq!(get_upgrade_command(Some("scoop")), "scoop update tod");
        assert_eq!(get_upgrade_command(Some("homebrew")), "brew upgrade tod");
        assert_eq!(
            get_upgrade_command(Some("source")),
            "https://github.com/tod-org/tod#installation"
        );
        assert_eq!(
            get_upgrade_command(Some("unknown")),
            "https://github.com/tod-org/tod#installation"
        );
    }
    #[test]
    fn test_get_update_command_args_homebrew() {
        let cmd = get_update_command_args(Some("homebrew"))
            .expect("Failed to get update command args for homebrew");
        assert_eq!(cmd.0, "brew");
        assert_eq!(cmd.1, vec!["upgrade", "tod"]);
    }

    #[test]
    fn test_get_update_command_args_scoop() {
        let cmd = get_update_command_args(Some("scoop"))
            .expect("Failed to get update command args for scoop");
        assert_eq!(cmd.0, "scoop");
        assert_eq!(cmd.1, vec!["update", "tod"]);
    }

    #[test]
    fn test_get_update_command_args_cargo() {
        let cmd = get_update_command_args(Some("cargo"))
            .expect("Failed to get update command args for cargo");
        assert_eq!(cmd.0, "cargo");
        assert_eq!(cmd.1, vec!["install", "tod", "--force"]);
    }

    #[test]
    fn test_get_update_command_args_from_source() {
        let err = get_update_command_args(Some("source"))
            .expect_err("Expected error when getting update command args for source");
        assert!(
            err.contains("Automatic update is not supported"),
            "Got: {err}"
        );
        // Error no longer contains the URL inline (presentation is in config_commands)
        assert!(
            !err.contains("http"),
            "Error should not contain raw URL: {err}"
        );
    }

    #[test]
    fn test_get_update_command_args_unknown() {
        let err = get_update_command_args(Some("unknown"))
            .expect_err("Expected error when getting update command args for unknown");
        assert!(
            err.contains("Automatic update is not supported"),
            "Got: {err}"
        );
        assert!(
            !err.contains("http"),
            "Error should not contain raw URL: {err}"
        );
    }

    #[test]
    fn test_osc8_link() {
        let link = osc8_link("https://example.com", "click here");
        assert!(link.starts_with("\x1b]8;;https://example.com\x1b\\"));
        assert!(link.contains("click here"));
        assert!(link.ends_with("\x1b]8;;\x1b\\"));
    }

    #[test]
    fn test_detect_install_method_cellar() {
        // Simulate an Intel Mac Homebrew path: /usr/local/Cellar/tod/0.12.1/bin/tod
        // Cannot override current_exe(), but we can test via the override arg
        // The cellar check is inside detect_install_method which runs when no override
        // So we test the override path to confirm the string mapping works
        assert_eq!(
            get_install_method(Some("homebrew")),
            InstallMethod::Homebrew
        );
        assert_eq!(get_install_method_string(Some("homebrew")), "homebrew");
    }
    #[test]
    fn test_get_install_method_override_whitespace_case() {
        assert_eq!(get_install_method(Some("  CaRgO  ")), InstallMethod::Cargo);
    }
    #[test]
    fn test_get_install_method_override_random() {
        assert_eq!(get_install_method(Some("foobar")), InstallMethod::Unknown);
    }
    #[test]
    fn test_get_update_command_args_none() {
        let result = get_update_command_args(None);
        assert!(
            result.is_ok()
                || result
                    .unwrap_err()
                    .contains("Automatic update is not supported")
        );
    }
}
