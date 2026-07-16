//! For config functions that operate on the filesystem

use crate::config::{Args, Internal};
#[cfg(test)]
use crate::config::{SortDirection, SortKey, SortRule};
use crate::{config::Config, errors::Error};
use crate::{debug, format, input};
use inquire::Confirm;
use serde_json::json;
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc::UnboundedSender;

impl Config {
    /// Creates a new config file, will overwrite an existing one
    pub async fn create(self) -> Result<Config, Error> {
        self.touch_file().await?;
        let mut config = self;
        // Save the config to disk
        config.save().await?;
        Ok(config)
    }

    /// Ensures the parent directory exists and touches the config file.
    pub async fn touch_file(&self) -> Result<(), Error> {
        if let Some(parent) = std::path::Path::new(&self.path).parent() {
            fs::create_dir_all(parent).await?;
        }
        fs::File::create(&self.path).await?;
        Ok(())
    }

    /// Writes the config's current contents to disk as JSON.
    pub async fn save(&mut self) -> std::result::Result<String, Error> {
        let config = match Config::load(&self.path).await {
            Ok(Config { verbose, .. }) => Config {
                verbose,
                ..self.clone()
            },
            _ => self.clone(),
        };

        let json = json!(config);
        let string = serde_json::to_string_pretty(&json)?;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .read(true)
            .truncate(true)
            .open(&self.path)
            .await?;
        file.write_all(string.as_bytes()).await?;
        file.flush().await?;
        file.sync_all().await?;

        Ok(format::green_string("✓"))
    }

    pub async fn load(path: &Path) -> Result<Config, Error> {
        let mut json = String::new();
        fs::File::open(path)
            .await?
            .read_to_string(&mut json)
            .await?;

        let config: Config =
            serde_json::from_str(&json).map_err(|e| config_load_error(&e, path))?;
        Ok(config.with_default_sort_order())
    }

    pub async fn reload(&self) -> Result<Self, Error> {
        Config::load(&self.path).await.map(|config| Config {
            internal: self.internal.clone(),
            time_provider: self.time_provider.clone(),
            ..config
        })
    }
}

fn config_load_error(error: &serde_json::Error, path: &Path) -> Error {
    let source = "serde_json";
    let message = format!(
        "\n{}",
        format::red_string(&format!(
            "Error loading configuration file '{}':\n{error}\n\
            \nThe file contains an invalid value.\n\
            Run 'tod config check' to remove invalid values, or run 'tod config reset' to delete (reset) the config.",
            path.display()
        ))
    );

    Error::new(source, &message)
}
/// Fetches config from from disk and creates it if it doesn't exist
/// Prompts for Todoist API token
pub async fn get_or_create(
    config_path: Option<PathBuf>,
    verbose: bool,
    timeout: Option<u64>,
    tx: &UnboundedSender<Error>,
) -> Result<Config, Error> {
    let path = match config_path {
        None => generate_path().await?,
        Some(path) => maybe_expand_home_dir(path)?,
    };

    let config = match fs::File::open(&path).await {
        Ok(_) => Config::load(&path).await,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("Config file not found, creating new config");
            generate_new_configuration(tx, path).await
        }
        Err(err) => Err(Error::new(
            "config.rs",
            &format!("Failed to open config file: {err}"),
        )),
    }?;

    let config = Config {
        args: Args { verbose, timeout },
        internal: Internal {
            tx: Some(tx.clone()),
        },
        ..config
    };

    debug::maybe_print_redacted_config(&config);
    Ok(config)
}
// create the config file and prompt timezone and token
pub async fn generate_new_configuration(
    tx: &UnboundedSender<Error>,
    config_path: PathBuf,
) -> Result<Config, Error> {
    // Create the default in-memory config
    let mut config = Config::new(Some(tx.clone()), config_path).await?;
    // Create the empty file
    config = config.create().await?;

    // Populate the required fields - prompt for token or use existing token logic
    config = config.maybe_set_token().await?;

    // Populate the required timezone
    config = config.maybe_set_timezone().await?;

    // write updated config to disk
    config.save().await?;

    Ok(config)
}
pub async fn generate_path() -> Result<PathBuf, Error> {
    if cfg!(test) {
        let file = tempfile::Builder::new()
            .prefix("tod-")
            .suffix(".testcfg")
            .tempfile()?;
        let path = file.path().to_path_buf();
        drop(file);
        Ok(path)
    } else {
        let config_directory = dirs::config_dir()
            .ok_or_else(|| Error::new("dirs", "Could not find config directory"))?;
        Ok(config_directory.join("tod.cfg"))
    }
}

fn maybe_expand_home_dir(path: PathBuf) -> Result<PathBuf, Error> {
    // If the path starts with "~", expand it
    if let Some(str_path) = path.to_str()
        && str_path.starts_with('~')
    {
        let home =
            homedir::my_home()?.ok_or_else(|| Error::new("homedir", "Could not get homedir"))?;

        // Strip the "~" and construct the new path
        let mut expanded = home;
        let suffix = str_path.trim_start_matches('~').trim_start_matches('/');
        expanded.push(suffix);

        return Ok(expanded);
    }

    Ok(path)
}

/// Deletes the config file after resolving its path and confirming with the user.
pub async fn config_reset(cli_config_path: Option<PathBuf>, force: bool) -> Result<String, Error> {
    // Defer to the testable version, but use `inquire::Confirm` for interactive input to pass true/false.
    config_reset_with_prompt(cli_config_path, force, |path| {
        let desc = &format!(
            "Are you sure you want to delete the config at {}?",
            path.display()
        );
        input::confirm(desc).unwrap_or_default()
    })
    .await
}

// Full config reset function, but accepts inputs for CI testing
pub async fn config_reset_with_prompt<F>(
    cli_config_path: Option<PathBuf>,
    force: bool,
    prompt_fn: F,
) -> Result<String, Error>
where
    F: FnOnce(&Path) -> bool,
{
    let path = match cli_config_path {
        Some(p) => maybe_expand_home_dir(p)?,
        None => generate_path().await?,
    };

    if !path.exists() {
        return Ok(format!("No config file found at {}.", path.display()));
    }

    if !force && !prompt_fn(&path) {
        return Ok("Aborted: Config not deleted.".to_string());
    }

    match fs::remove_file(&path).await {
        Ok(()) => Ok(format!(
            "Config file at {} deleted successfully.",
            path.display()
        )),
        Err(e) => Err(Error::new(
            "config_reset",
            &format!("Could not delete config file at {}: {}", path.display(), e),
        )),
    }
}

/// Opens the config file in the user's editor, creating a default config first if requested.
pub async fn config_open(cli_config_path: Option<PathBuf>) -> Result<String, Error> {
    config_open_with_prompt_and_editor(
        cli_config_path,
        |path| {
            Confirm::new(&format!(
                "No config file found at {}. Create it?",
                path.display()
            ))
            .with_default(true)
            .prompt()
            .unwrap_or(false)
        },
        |path| edit::edit_file(path).map_err(Error::from),
    )
    .await
}

async fn config_open_with_prompt_and_editor<P, E>(
    cli_config_path: Option<PathBuf>,
    prompt_fn: P,
    editor_fn: E,
) -> Result<String, Error>
where
    P: FnOnce(&Path) -> bool,
    E: FnOnce(&Path) -> Result<(), Error>,
{
    let path = resolve_config_path(cli_config_path).await?;

    if !path.exists() {
        if !prompt_fn(&path) {
            return Ok("Aborted: Config not created.".to_string());
        }

        let mut config = Config::new(None, path.clone()).await?;
        config.touch_file().await?;
        config.save().await?;
    }

    editor_fn(&path)?;
    Config::load(&path).await?;

    Ok(format!(
        "Config file at {} opened and validated successfully.",
        path.display()
    ))
}

/// Resolves the config path, either using the provided path or generating a default one.
pub async fn resolve_config_path(config_path: Option<PathBuf>) -> Result<PathBuf, Error> {
    match config_path {
        None => generate_path().await,
        Some(path) => maybe_expand_home_dir(path),
    }
}

/// Fetches the config from disk; errors out if it doesn't exist
pub async fn get_config(config_path: Option<PathBuf>) -> Result<Config, Error> {
    let path = match config_path {
        None => generate_path().await?,
        Some(path) => maybe_expand_home_dir(path)?,
    };

    let path_for_error = path.clone();
    if !check_config_exists(Some(path)).await? {
        return Err(Error::new(
            "get_config",
            &format!("No config file found at {}.", path_for_error.display()),
        ));
    }

    Config::load(&path_for_error).await
}

/// Checks if the config file exists at the given path OR  default path if None).
pub async fn check_config_exists(config_path: Option<PathBuf>) -> Result<bool, Error> {
    let path = resolve_config_path(config_path).await?;
    Ok(path.exists())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test;
    use pretty_assertions::assert_eq;
    use std::io::Write;
    use std::path::PathBuf;
    use tempfile::{TempDir, tempdir};

    async fn config_with_mock(mock_url: impl Into<String>) -> Config {
        test::fixtures::config()
            .await
            .with_mock_url(mock_url.into())
    }

    async fn config_with_mock_and_token(
        mock_url: impl Into<String>,
        token: impl Into<String>,
    ) -> Config {
        test::fixtures::config()
            .await
            .with_mock_url(mock_url.into())
            .with_token(&token.into())
    }

    fn tx() -> UnboundedSender<Error> {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        tx
    }

    fn temp_config_path(file_name: &str) -> (TempDir, PathBuf) {
        let dir = tempdir().expect("Could not create temp config directory");
        let path = dir.path().join(file_name);
        (dir, path)
    }

    #[tokio::test]
    async fn config_tests() {
        let server = mockito::Server::new_async().await;
        let mock_url = server.url();
        let temp_dir = tempdir().expect("Could not create temp config directory");

        let config_create = config_with_mock_and_token(&mock_url, "created")
            .await
            .with_path(temp_dir.path().join("created.cfg"));
        let path_created = config_create.path.clone();
        config_create
            .create()
            .await
            .expect("Failed to create config in async call");

        let loaded = Config::load(&path_created)
            .await
            .expect("Failed to load config from path asynchronously");
        assert_eq!(loaded.token, Some("created".into()));

        let config_create = config_with_mock(&mock_url)
            .await
            .with_path(temp_dir.path().join("create.cfg"));
        let path_create = config_create.path.clone();
        config_create
            .create()
            .await
            .expect("Failed to create config in async call");

        let created = get_or_create(Some(path_create.clone()), false, None, &tx())
            .await
            .expect("get_or_create (create) failed");
        assert!(created.token.is_some());

        let config_load = config_with_mock_and_token(&mock_url, "loaded")
            .await
            .with_path(temp_dir.path().join("loaded.cfg"));
        let path_load = config_load.path.clone();
        config_load
            .create()
            .await
            .expect("Failed to create config load asynchronously");

        let loaded = get_or_create(Some(path_load.clone()), false, None, &tx())
            .await
            .expect("get_or_create (load) failed");
        assert_eq!(loaded.token, Some("loaded".into()));
        assert!(loaded.internal.tx.is_some());

        let fetched = get_or_create(Some(path_load.clone()), false, None, &tx()).await;
        assert_matches!(fetched, Ok(Config { .. }));
    }

    #[tokio::test]
    async fn new_should_generate_config() {
        let config = Config::new(
            None,
            generate_path().await.expect("Could not create config"),
        )
        .await
        .expect("Could not create config");
        assert_eq!(config.token, None);
    }

    #[tokio::test]
    async fn reload_config_should_work() {
        let (_temp_dir, temp_path) = temp_config_path("reload.cfg");
        let config = test::fixtures::config().await.with_path(temp_path);
        let mut config = config.create().await.expect("Failed to create test config");
        let project = test::fixtures::project();
        config.add_project(project);
        let projects = config
            .projects()
            .await
            .expect("Failed to fetch projects asynchronously");
        assert!(!&projects.is_empty());

        config.reload().await.expect("Failed to reload config");
    }
    #[test]
    fn test_maybe_expand_home_dir() {
        // No tilde, so path should remain unchanged
        let input = PathBuf::from("/Users/tod.cfg");
        let result = maybe_expand_home_dir(input.clone()).expect("Could not create PathBuf");

        assert_eq!(result, input);
    }

    #[tokio::test]
    async fn load_should_fail_on_invalid_sort_order_value() {
        let (_temp_dir, bad_config_path) = temp_config_path("bad_config_invalid_sort_order.cfg");
        let contents = serde_json::json!({
            "token": "abc123",
            "path": bad_config_path,
            "sort_order": ["not_a_sort_key"]
        })
        .to_string();

        tokio::fs::write(&bad_config_path, contents)
            .await
            .expect("Could not write to file");

        let result = Config::load(&bad_config_path).await;
        assert!(result.is_err(), "Expected error from invalid sort order");
    }

    #[tokio::test]
    async fn load_should_accept_order_sort_key() {
        let (_temp_dir, path) = temp_config_path("order_sort_key.cfg");
        let contents = serde_json::json!({
            "path": path,
            "sort_order": ["order"]
        })
        .to_string();

        tokio::fs::write(&path, contents)
            .await
            .expect("Could not write to file");

        let config = Config::load(&path)
            .await
            .expect("order should be a valid sort key");

        assert_eq!(
            config.sort_order,
            Some(vec![SortRule::new(SortKey::Order, SortDirection::Asc)])
        );
    }

    #[tokio::test]
    async fn load_should_accept_explicit_sort_direction() {
        let (_temp_dir, path) = temp_config_path("explicit_sort_direction.cfg");
        let contents = serde_json::json!({
            "path": path,
            "sort_order": ["priority:asc", "order:desc"]
        })
        .to_string();

        tokio::fs::write(&path, contents)
            .await
            .expect("Could not write to file");

        let config = Config::load(&path)
            .await
            .expect("explicit sort directions should load");

        assert_eq!(
            config.sort_order,
            Some(vec![
                SortRule::new(SortKey::Priority, SortDirection::Asc),
                SortRule::new(SortKey::Order, SortDirection::Desc),
            ])
        );
    }

    #[tokio::test]
    async fn load_should_reject_invalid_sort_direction() {
        let (_temp_dir, path) = temp_config_path("invalid_sort_direction.cfg");
        let contents = serde_json::json!({
            "path": path,
            "sort_order": ["priority:sideways"]
        })
        .to_string();

        tokio::fs::write(&path, contents)
            .await
            .expect("Could not write to file");

        assert!(Config::load(&path).await.is_err());
    }

    #[tokio::test]
    async fn load_should_migrate_legacy_sort_value_to_sort_order() {
        let (_temp_dir, path) = temp_config_path("legacy_sort_value.cfg");
        let contents = serde_json::json!({
            "token": "abc123",
            "path": path,
            "sort_value": {
                "priority_none": 2,
                "priority_low": 1,
                "priority_medium": 3,
                "priority_high": 4,
                "no_due_date": 80,
                "not_recurring": 50,
                "today": 100,
                "overdue": 150,
                "now": 200,
                "deadline_value": 30,
                "deadline_days": 5
            }
        })
        .to_string();

        tokio::fs::write(&path, contents)
            .await
            .expect("Could not write to file");

        let config = Config::load(&path)
            .await
            .expect("legacy sort_value should migrate");

        assert_eq!(
            config.sort_order.expect("sort_order should be populated"),
            vec![
                SortRule::new(SortKey::Now, SortDirection::Desc),
                SortRule::new(SortKey::Overdue, SortDirection::Desc),
                SortRule::new(SortKey::Today, SortDirection::Desc),
                SortRule::new(SortKey::NoDueDate, SortDirection::Desc),
                SortRule::new(SortKey::NotRecurring, SortDirection::Desc),
                SortRule::new(SortKey::Deadline, SortDirection::Asc),
                SortRule::new(SortKey::Priority, SortDirection::Desc),
                SortRule::new(SortKey::DueDate, SortDirection::Asc),
                SortRule::new(SortKey::Order, SortDirection::Asc),
            ]
        );
    }

    #[tokio::test]
    async fn load_should_clamp_negative_completed_count() {
        let (_temp_dir, path) = temp_config_path("negative_completed_count.cfg");
        let contents = serde_json::json!({
            "completed": {
                "count": -1,
                "date": "2026-04-30"
            },
            "path": path,
            "timezone": "UTC"
        })
        .to_string();

        tokio::fs::write(&path, contents)
            .await
            .expect("Could not write to file");

        let config = Config::load(&path)
            .await
            .expect("negative completed count should load");

        assert_eq!(config.completed.expect("completed should be set").count, 0);
    }
    #[test]
    fn test_maybe_expand_home_dir_expands_tilde() {
        let input = PathBuf::from("~/myfolder/mysubfile.txt");
        let expanded = maybe_expand_home_dir(input).expect("Could not expand home dir");

        let expected_prefix = homedir::my_home()
            .expect("Could not find home path")
            .expect("No home path found");
        assert!(expanded.starts_with(&expected_prefix));
        assert!(expanded.ends_with("myfolder/mysubfile.txt"));
    }
    #[test]
    fn test_maybe_expand_home_dir_non_tilde_unchanged() {
        let input = PathBuf::from("/usr/bin/env");
        let result = maybe_expand_home_dir(input.clone()).expect("Could not expand home dir");
        assert_eq!(result, input);
    }

    #[tokio::test]
    async fn test_get_config_with_existing_path() {
        let (_temp_dir, temp_path) = temp_config_path("test_get_config_exists.cfg");
        let mut config = Config {
            path: temp_path.clone(),
            token: Some("abc".to_string()),
            timezone: Some("UTC".to_string()),
            ..Config::default()
        };
        // Ensure parent directory exists and file is created
        config = config.create().await.expect("Should create config file");
        config.save().await.expect("Should save config");
        assert!(temp_path.exists(), "Config file should exist after create");

        // Should load successfully
        let loaded = get_config(Some(temp_path.clone())).await;
        assert!(
            loaded.is_ok(),
            "Expected Ok for existing config, got {loaded:?}"
        );
        let loaded = loaded.expect("No config found");
        assert_eq!(loaded.token, Some("abc".to_string()));
    }
    #[tokio::test]
    async fn test_check_config_exists_true_and_false() {
        let (_temp_dir, temp_path) = temp_config_path("test_check_config_exists.cfg");

        let exists = check_config_exists(Some(temp_path.clone()))
            .await
            .expect("Could not check if config exists");
        assert!(!exists, "Should be false for nonexistent config");

        tokio::fs::File::create(&temp_path)
            .await
            .expect("Could not create file");
        let exists = check_config_exists(Some(temp_path.clone()))
            .await
            .expect("Could not check if config exists");
        assert!(exists, "Should be true for existing config");
    }

    #[tokio::test]
    async fn test_config_reset_with_prompt_yes_deletes_file() {
        let (_temp_dir, temp_path) = temp_config_path("temp_test_config_prompt_yes.cfg");

        tokio::fs::File::create(&temp_path)
            .await
            .expect("Failed to create temp config file");
        assert!(temp_path.exists(), "Temp config should exist before reset");

        // Simulate user saying "yes"
        let result = config_reset_with_prompt(Some(temp_path.clone()), false, |_| true).await;

        assert!(result.is_ok(), "Expected Ok, got {result:?}");
        let msg = result.expect("Could not get msg");
        assert!(
            msg.contains("deleted successfully"),
            "Expected deletion message, got: {msg}"
        );
        assert!(!temp_path.exists(), "File should be deleted after reset");
    }

    #[tokio::test]
    async fn test_config_reset_with_prompt_no_aborts() {
        let (_temp_dir, temp_path) = temp_config_path("temp_test_config_prompt_no.cfg");

        tokio::fs::File::create(&temp_path)
            .await
            .expect("Failed to create temp config file");
        assert!(temp_path.exists(), "Temp config should exist before reset");

        // Simulate user saying "no"
        let result = config_reset_with_prompt(Some(temp_path.clone()), false, |_| false).await;

        assert!(result.is_ok(), "Expected Ok, got {result:?}");
        let msg = result.expect("Could not get reset config response");
        assert_eq!(msg, "Aborted: Config not deleted.");
        assert!(temp_path.exists(), "File should not be deleted after abort");
    }

    #[tokio::test]
    async fn test_config_reset_prompt_receives_resolved_path() {
        let (_temp_dir, temp_path) = temp_config_path("temp_test_config_prompt_path.cfg");
        tokio::fs::File::create(&temp_path)
            .await
            .expect("Failed to create temp config file");
        let mut prompted_path = None;

        let result = config_reset_with_prompt(Some(temp_path.clone()), false, |path| {
            prompted_path = Some(path.to_path_buf());
            false
        })
        .await;

        assert_eq!(result, Ok("Aborted: Config not deleted.".to_string()));
        assert_eq!(prompted_path, Some(temp_path));
    }

    #[tokio::test]
    async fn test_config_open_missing_prompt_no_aborts() {
        let (_temp_dir, temp_path) = temp_config_path("missing_open_no.cfg");

        let result = config_open_with_prompt_and_editor(
            Some(temp_path.clone()),
            |_| false,
            |_| -> Result<(), Error> { panic!("editor should not be called") },
        )
        .await;

        assert_eq!(result, Ok("Aborted: Config not created.".to_string()));
        assert!(
            !temp_path.exists(),
            "Config file should not be created after abort"
        );
    }

    #[tokio::test]
    async fn test_config_open_missing_prompt_yes_creates_and_validates() {
        let (_temp_dir, temp_path) = temp_config_path("missing_open_yes.cfg");

        let result =
            config_open_with_prompt_and_editor(Some(temp_path.clone()), |_| true, |_| Ok(())).await;

        assert!(result.is_ok(), "Expected Ok, got {result:?}");
        assert!(temp_path.exists(), "Config file should be created");

        let loaded = Config::load(&temp_path)
            .await
            .expect("Created config should load");
        assert_eq!(loaded.path, temp_path);
    }

    #[tokio::test]
    async fn test_config_open_invalid_after_editor_errors() {
        let (_temp_dir, temp_path) = temp_config_path("invalid_after_open.cfg");
        Config::default_test()
            .with_path(temp_path.clone())
            .create()
            .await
            .expect("Failed to create temp config");

        let result = config_open_with_prompt_and_editor(
            Some(temp_path.clone()),
            |_| -> bool { panic!("prompt should not be called") },
            |path| {
                let mut file = std::fs::File::create(path).expect("Failed to open invalid config");
                file.write_all(b"{ invalid")
                    .expect("Failed to write invalid config");
                file.sync_all().expect("Failed to sync invalid config");
                Ok(())
            },
        )
        .await;

        let err = result.expect_err("Invalid config should fail validation");
        assert!(
            err.message.contains("Error loading configuration file"),
            "Expected config load error, got: {err}"
        );
    }
    #[tokio::test]
    async fn test_create_config_with_custom_path() {
        let (_temp_dir, path) = temp_config_path("custom_path.cfg");
        let mut config = Config {
            path,
            ..Config::default_test()
        };
        config = config.create().await.expect("Should create file");
        config.save().await.expect("Should save file");

        // Check that required fields are populated
        assert!(config.token.is_some(), "Token should be set");
        assert!(config.timezone.is_some(), "Timezone should be set");

        // Check that the file exists
        assert!(
            tokio::fs::try_exists(&config.path)
                .await
                .expect("Could not determine if file exists"),
            "Config file should exist at {}",
            config.path.display()
        );
    }

    #[tokio::test]
    async fn test_create_config_saves_file() {
        let (_temp_dir, path) = temp_config_path("default_test.cfg");
        let mut config = Config::default_test().with_path(path);
        config = config.create().await.expect("Should create file");
        config.save().await.expect("Should save file");

        // Check that required fields are populated
        assert!(config.token.is_some(), "Token should be set");
        assert!(config.timezone.is_some(), "Timezone should be set");

        // Check that the file exists
        assert!(
            tokio::fs::try_exists(&config.path)
                .await
                .expect("Could not determine if file exists"),
            "Config file should exist at {}",
            config.path.display()
        );
    }

    #[tokio::test]
    async fn test_generate_path_in_test_mode() {
        let path = generate_path().await.expect("Should return a test path");
        let temp_dir = std::env::temp_dir();

        // Check that the generated path is isolated under the system temp directory.
        assert!(
            path.starts_with(&temp_dir),
            "Expected path to be under '{}', got {}",
            temp_dir.display(),
            path.display()
        );

        // Check that the file extension is ".testcfg"
        assert!(
            path.extension().is_some_and(|ext| ext == "testcfg"),
            "Expected file extension to be .testcfg, got {}",
            path.display()
        );
    }
    #[tokio::test]
    async fn test_load_config_rejects_invalid_regex() {
        let (_temp_dir, path) = temp_config_path("invalid_regex.cfg");

        // Write the invalid regex string "[a-z" to the config file which should cause serde_json to fail
        let invalid_json = r#"
    {
        "token": "abc123",
        "timezone": "UTC",
        "task_exclude_regex": "[a-z"
    }
    "#;

        tokio::fs::write(&path, invalid_json)
            .await
            .expect("Failed to write invalid config");

        let result = Config::load(&path).await;

        assert!(
            result.is_err(),
            "Expected load to fail due to invalid regex"
        );
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("Error loading configuration file"),
            "Expected 'Error loading configuration file' in error message:\n{msg}"
        );

        assert!(
            msg.contains("regex parse error"),
            "Expected 'regex parse error' in error message:\n{msg}"
        );
    }

    #[tokio::test]
    async fn test_create_config_populates_token_and_timezone() {
        // Manually set token and timezone and ensure they're saved
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let (_temp_dir, path) = temp_config_path("populated_config.cfg");
        let mut config = Config::new(Some(tx.clone()), path)
            .await
            .expect("Init default config");

        config.token = Some("test-token-123".into());
        config.timezone = Some("UTC".into());
        config = config.create().await.expect("Should create file");
        config.save().await.expect("Should save config");

        // Reload from disk and validate contents
        let contents = tokio::fs::read_to_string(&config.path)
            .await
            .expect("File should exist");
        assert!(
            contents.contains("test-token-123"),
            "Saved config should contain token"
        );
        assert!(
            contents.contains("UTC"),
            "Saved config should contain timezone"
        );
    }

    #[tokio::test]
    async fn test_config_reset_force_deletes_temp_file() {
        let (_temp_dir, temp_path) = temp_config_path("temp_test_config.cfg");

        tokio::fs::File::create(&temp_path)
            .await
            .expect("Failed to create temp config file");
        assert!(temp_path.exists(), "Temp config should exist before reset");

        let result = crate::config::config_reset(Some(temp_path.clone()), true).await;
        assert!(result.is_ok(), "Expected Ok, got {result:?}");

        assert!(!temp_path.exists(), "File should be deleted");
    }

    #[tokio::test]
    async fn test_get_config_with_nonexistent_path() {
        let (_temp_dir, temp_path) = temp_config_path("test_get_config_nonexistent.cfg");

        let loaded = get_config(Some(temp_path.clone())).await;
        assert!(loaded.is_err(), "Expected Err for nonexistent config");
        let err = loaded.unwrap_err().to_string();
        assert!(
            err.contains("No config file found"),
            "Expected missing config error, got: {err}"
        );
    }
}
