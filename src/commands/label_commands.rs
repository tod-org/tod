use crate::{config::Config, errors::Error, format, input, labels};
use clap::{Parser, Subcommand};

/// Label subcommands.
#[derive(Subcommand, Debug, Clone)]
pub enum LabelCommands {
    #[clap(alias = "c")]
    /// (c) Create a new personal label
    Create(Create),
    #[clap(alias = "u")]
    /// (u) Update an existing personal label
    Update(Update),
    #[clap(alias = "d")]
    /// (d) Delete a personal label
    Delete(Delete),
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

#[derive(Parser, Debug, Clone)]
pub struct Update {
    #[arg(short, long)]
    /// Label to update (name or ID)
    label: Option<String>,

    #[arg(short, long)]
    /// New name for the label
    name: Option<String>,

    #[arg(short, long)]
    /// New color for the label
    color: Option<String>,

    #[arg(short, long)]
    /// New display order
    order: Option<u32>,

    #[arg(short = 'f', long)]
    /// Toggle favorite status (true or false)
    favorite: Option<bool>,
}

#[derive(Parser, Debug, Clone)]
pub struct Delete {
    #[arg(short, long)]
    /// Label to delete (name or ID)
    label: Option<String>,

    #[arg(short = 'f', long, default_value_t = false)]
    /// Skip deletion confirmation
    force: bool,
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

/// Updates a personal label.
pub async fn update(config: &Config, args: &Update, json: bool) -> Result<String, Error> {
    let Update {
        label,
        name,
        color,
        order,
        favorite,
    } = args;

    if name.is_none() && color.is_none() && order.is_none() && favorite.is_none() {
        return Err(Error::new(
            "update_label",
            "At least one of --name, --color, --order, or --favorite is required",
        ));
    }

    let labels_list = labels::get_labels(config, true).await?;
    let target = super::fetch_label(label.as_deref(), config, &labels_list)?;

    let updated = labels::update(
        config,
        &target.id,
        name.as_deref(),
        color.as_deref(),
        *order,
        *favorite,
    )
    .await?;
    if json {
        Ok(serde_json::to_string(&updated)?)
    } else {
        Ok(format::green_string(&format!(
            "Label \"{}\" updated",
            updated.name
        )))
    }
}

/// Deletes a personal label.
pub async fn delete(config: &Config, args: &Delete, json: bool) -> Result<String, Error> {
    let Delete { label, force } = args;

    let labels_list = labels::get_labels(config, true).await?;
    if labels_list.is_empty() {
        return Ok("No labels found".into());
    }

    let target = super::fetch_label(label.as_deref(), config, &labels_list)?;

    if !force {
        if json {
            return Err(Error::new("json_mode", super::JSON_INTERACTIVE_ERROR));
        }
        let options = vec![input::CANCEL, input::DELETE];
        let desc = format!("Delete label \"{}\"?", target.name);
        let result = input::select(&desc, options, config.mock_select)?;
        if result == input::CANCEL {
            return Ok("Cancelled".into());
        }
    }

    labels::delete(config, &target.id).await?;
    if json {
        Ok(serde_json::to_string(&target)?)
    } else {
        Ok(format::green_string(&format!(
            "Label \"{}\" deleted",
            target.name
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test::responses::ResponseFromFile;

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

    #[tokio::test]
    async fn update_fails_without_any_fields() {
        let config = Config::default_test();
        let args = Update {
            label: Some("my-label".to_string()),
            name: None,
            color: None,
            order: None,
            favorite: None,
        };

        let error = update(&config, &args, false)
            .await
            .expect_err("update with no fields should fail");

        assert_eq!(error.source, "update_label");
        assert!(error.message.contains("At least one"));
    }

    #[tokio::test]
    async fn delete_fails_when_label_not_found() {
        let mut server = mockito::Server::new_async().await;

        let labels_mock = server
            .mock("GET", "/api/v1/labels?limit=200")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::Labels.read().await)
            .create_async()
            .await;

        let config = crate::test::fixtures::config()
            .await
            .with_mock_url(server.url());

        let args = Delete {
            label: Some("nonexistent".to_string()),
            force: false,
        };

        let error = delete(&config, &args, false)
            .await
            .expect_err("deleting a nonexistent label should fail");

        assert_eq!(error.source, "fetch_label");
        assert!(error.message.contains("not found"));
        labels_mock.assert_async().await;
    }

    #[tokio::test]
    async fn delete_force_skips_confirmation() {
        let mut server = mockito::Server::new_async().await;

        let labels_mock = server
            .mock("GET", "/api/v1/labels?limit=200")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::Labels.read().await)
            .create_async()
            .await;

        let delete_mock = server
            .mock("DELETE", "/api/v1/labels/123")
            .with_status(204)
            .create_async()
            .await;

        let config = crate::test::fixtures::config()
            .await
            .with_mock_url(server.url());

        let args = Delete {
            label: Some("345".to_string()),
            force: true,
        };

        let result = delete(&config, &args, false)
            .await
            .expect("force delete should succeed");

        assert!(result.contains("deleted"));
        labels_mock.assert_async().await;
        delete_mock.assert_async().await;
    }
}
