use std::env;
use std::time::Duration;

use reqwest::Client;
use reqwest::Response;
use reqwest::header::AUTHORIZATION;
use reqwest::header::CONTENT_TYPE;
use serde_json::Value;
use serde_json::json;
use spinners::Spinner;
use spinners::Spinners;
use uuid::Uuid;

use crate::color;
use crate::config::Args;
use crate::config::Config;
use crate::debug;
use crate::errors::Error;

const FAKE_UUID: &str = "42963283-2bab-4b1f-bad2-278ef2b6ba2c";
const TODOIST_URL: &str = "https://api.todoist.com";

const SPINNER: Spinners = Spinners::Dots4;
const MESSAGE: &str = "Querying API";
const HTTP_UNAUTHORIZED: u16 = 401;
const HTTP_FORBIDDEN: u16 = 403;

/// Post to Todoist via REST api
/// We use this when we want more options and don't need natural language processing
/// Pass in a Value::Null for the body if there is no payload
pub async fn post_todoist(
    config: &Config,
    url: &str,
    body: serde_json::Value,
    spinner: bool,
) -> Result<String, Error> {
    let base_url = get_base_url(config);
    let token = get_token(config)?;

    let request_url = format!("{base_url}{url}");
    let authorization = format!("Bearer {token}");
    let spinner = maybe_start_spinner(config, spinner);

    debug::maybe_print(config, &format!("POST {request_url}\nbody: {body}"));

    let client = Client::new()
        .post(request_url.clone())
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, authorization)
        .header("X-Request-Id", new_uuid())
        .timeout(get_timeout(config));

    let response = match &body {
        Value::Null => client.send().await?,

        body => client.json(&body).send().await?,
    };
    maybe_stop_spinner(spinner);
    handle_response(config, response, "POST", url, body).await
}

pub async fn post_todoist_no_token(
    config: &Config,
    url: &str,
    body: serde_json::Value,
    spinner: bool,
) -> Result<String, Error> {
    let base_url = get_base_url(config);
    let request_url = format!("{base_url}{url}");
    let spinner = maybe_start_spinner(config, spinner);

    debug::maybe_print(config, &format!("POST {request_url}\nbody: {body}"));

    let client = Client::new()
        .post(request_url.clone())
        .header(CONTENT_TYPE, "application/json")
        .header("X-Request-Id", new_uuid())
        .timeout(get_timeout(config));

    let response = match &body {
        Value::Null => client.send().await?,

        body => client.json(&body).send().await?,
    };
    maybe_stop_spinner(spinner);
    handle_response(config, response, "POST", url, body).await
}

fn get_token(config: &Config) -> Result<String, Error> {
    config
        .token
        .clone()
        .ok_or_else(|| Error::new("post_todoist", "No token, use auth login to set your token"))
}

pub async fn delete_todoist(
    config: &Config,
    url: &str,
    body: serde_json::Value,
    spinner: bool,
) -> Result<String, Error> {
    let base_url = get_base_url(config);
    let token = get_token(config)?;

    let request_url = format!("{base_url}{url}");
    let authorization = format!("Bearer {token}");
    let spinner = maybe_start_spinner(config, spinner);

    debug::maybe_print(config, &format!("DELETE {request_url}\nbody: {body}"));

    let response = Client::new()
        .delete(request_url.clone())
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, authorization)
        .header("X-Request-Id", new_uuid())
        .json(&body)
        .timeout(get_timeout(config))
        .send()
        .await?;

    maybe_stop_spinner(spinner);
    handle_response(config, response, "DELETE", url, body).await
}

// Combine get and post into one function
/// Get Todoist via REST api
pub async fn get_todoist(config: &Config, url: &str, spinner: bool) -> Result<String, Error> {
    let base_url = get_base_url(config);
    let token = get_token(config)?;

    let request_url = format!("{base_url}{url}");
    let authorization = format!("Bearer {token}");
    let spinner = maybe_start_spinner(config, spinner);
    if config.verbose.unwrap_or_default() {
        println!("GET {request_url}")
    }
    debug::maybe_print(config, &format!("GET {request_url}"));
    let response = Client::new()
        .get(request_url.clone())
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, authorization)
        .timeout(get_timeout(config))
        .send()
        .await?;

    maybe_stop_spinner(spinner);
    handle_response(config, response, "GET", url, json!({})).await
}

async fn handle_response(
    config: &Config,
    response: Response,
    method: &str,
    url: &str,
    body: serde_json::Value,
) -> Result<String, Error> {
    let status = response.status();
    let status_code = status.as_u16();
    if status.is_success() {
        let json_string = response.text().await?;
        debug::maybe_print(config, &format!("{method} {url}\nresponse: {json_string}"));
        Ok(json_string)
    } else if status_code == HTTP_UNAUTHORIZED || status_code == HTTP_FORBIDDEN {
        let command = color::blue_string("tod auth login");
        Err(Error::new(
            "reqwest",
            &format!(
                "Unauthorized or Forbidden response from Todoist\nRun {command} to reauthenticate"
            ),
        ))
    } else {
        let json_string = response.text().await?;
        Err(Error::new(
            "reqwest",
            &format!(
                "
            method: {method}
            url: {url}
            body: {body}
            response: {json_string}",
            ),
        ))
    }
}

fn get_timeout(config: &Config) -> Duration {
    match config {
        Config {
            timeout: Some(timeout),
            args: Args { timeout: None, .. },
            ..
        } => Duration::from_secs(timeout.to_owned()),
        Config {
            timeout: Some(_),
            args: Args {
                timeout: Some(timeout),
                ..
            },
            ..
        } => Duration::from_secs(timeout.to_owned()),
        Config {
            timeout: None,
            args: Args { timeout: None, .. },
            ..
        } => Duration::from_secs(30),

        Config {
            timeout: None,
            args: Args {
                timeout: Some(timeout),
                ..
            },
            ..
        } => Duration::from_secs(timeout.to_owned()),
    }
}

fn get_base_url(config: &Config) -> String {
    if cfg!(test) {
        config.mock_url.clone().expect("Mock URL not set")
    } else {
        TODOIST_URL.to_string()
    }
}

fn maybe_start_spinner(config: &Config, spinner: bool) -> Option<Spinner> {
    if cfg!(test) {
        return None;
    }

    match (env::var("DISABLE_SPINNER"), config.spinners, spinner) {
        (Ok(_), _, _) => None,
        (_, Some(false), _) => None,
        (_, _, false) => None,
        _ => {
            let sp = Spinner::new(SPINNER, MESSAGE.into());
            Some(sp)
        }
    }
}
fn maybe_stop_spinner(spinner: Option<Spinner>) {
    if let Some(mut sp) = spinner {
        sp.stop();
        print!("\x1b[2K\r");
    };
}

/// Create a new UUID, required for Todoist API
pub fn new_uuid() -> String {
    if cfg!(test) {
        FAKE_UUID.into()
    } else {
        Uuid::new_v4().to_string()
    }
}
