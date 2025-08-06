use futures::lock::Mutex;
use serde::Deserialize;
use uuid::Uuid;

use crate::color::green_string;
use crate::errors::Error;
use crate::tasks::format::maybe_format_url;
use crate::todoist::OAUTH_URL;
use crate::{config::Config, todoist};

use axum::{Router, extract::Query, routing::get};
use std::sync::Arc;
use tokio::sync::oneshot::{self, Sender};

pub const CLIENT_ID: &str = "2696d64dc4f745679e21181c56b489fe";
pub const CLIENT_SECRET: &str = "bfde0d10e3d740beb47f95879881634e";
const FAKE_UUID: &str = "42963283-2bab-4b1f-bad2-278ef2b6ba2c";
const TRANSMIT_ERROR: &str = "Could not transmit";
/// Host to bind the OAuth server to in production.
const PROD_LOCALHOST: &str = "127.0.0.1:8080";
const SCOPE: &str = "data:read_write,data:delete,project:delete";

#[derive(Deserialize, Debug)]
struct Params {
    // returns only in the case of an error
    error: Option<String>,
    // authorization code from which we can get an access token
    code: Option<String>,
    // should match the csrf token we passed in
    state: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct AccessToken {
    pub access_token: String,
}

pub async fn login(config: &mut Config, test_tx: Option<Sender<()>>) -> Result<String, Error> {
    // Use the provided config, not a new default every time
    let csrf_token = print_oauth_url(config);
    let listener = tokio::net::TcpListener::bind(PROD_LOCALHOST).await?;
    let code = receive_callback(&csrf_token, test_tx, listener)
        .await?
        .code
        .ok_or_else(|| Error::new("params", "no code provided"))?;
    let access_token = todoist::get_access_token(config, &code).await?;
    let result = config.set_token(access_token).await;

    // Print authentication success message to the terminal
    let check = green_string("Authentication Successful!");
    println!("{check}");
    println!("You can now use the `tod` command to manage your Todoist tasks.");

    result
}

fn print_oauth_url(config: &Config) -> String {
    let csrf_token = new_uuid();

    let url = format!(
        "https://todoist.com{OAUTH_URL}?client_id={CLIENT_ID}&scope={SCOPE}&state={csrf_token}"
    );
    let formatted_url = maybe_format_url(&url, config);
    // Don't open the browser in test mode, just print the URL
    if cfg!(test) {
        println!("Please visit the following url to authenticate with Todoist:");
        println!("{formatted_url}");
    } else {
        match open::that(&url) {
            Ok(_) => {
                println!(
                    "Opening {formatted_url} in the default web browser to authenticate with Todoist."
                );
            }
            Err(_) => {
                println!("Please visit the following url to authenticate with Todoist:");
                println!("{formatted_url}");
            }
        }
    }
    csrf_token
}

async fn receive_callback(
    csrf_token: &str,
    tx: Option<Sender<()>>,
    listener: tokio::net::TcpListener,
) -> Result<Params, Error> {
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let shutdown_signal = Arc::new(Mutex::new(Some(shutdown_tx)));

    let (response_tx, response_rx) = oneshot::channel::<Params>();
    let response = Arc::new(Mutex::new(Some(response_tx)));

    let app = Router::new().route(
        "/",
        get(move |Query(params): Query<Params>| async move {
            if let Some(tx) = shutdown_signal.lock().await.take() {
                let _ = tx.send(());
            }

            if let Some(tx) = response.lock().await.take() {
                if let Some(error_message) = params.error.clone() {
                    tx.send(params).expect(TRANSMIT_ERROR);
                    format!("Error from Todoist: {error_message}")
                } else {
                    tx.send(params).expect(TRANSMIT_ERROR);
                    String::from("Success! You can close this window and return to your terminal.")
                }
            } else {
                String::from("Error: Could not get response tx")
            }
        }),
    );
    if let Some(tx) = tx {
        tx.send(()).expect("failed to notify test");
    };
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            shutdown_rx.await.ok();
        })
        .await?;

    let params = response_rx.await?;

    if let Some(message) = params.error {
        Err(Error::new("oauth get code", &message))
    } else if params.state.clone().unwrap_or_default() == csrf_token {
        Ok(params)
    } else {
        Err(Error::new(
            "oauth get code",
            "state doesn't match csrf token",
        ))
    }
}

pub fn json_to_access_token(json: String) -> Result<AccessToken, Error> {
    let token: AccessToken = serde_json::from_str(&json)?;
    Ok(token)
}

/// Create a new UUID, required for Todoist OAuth
pub fn new_uuid() -> String {
    if cfg!(test) {
        String::from(FAKE_UUID)
    } else {
        Uuid::new_v4().to_string()
    }
}
#[cfg(test)]
mod tests {

    use super::*;
    use crate::test::{self, responses::ResponseFromFile};
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn login_test() {
        let mut server = mockito::Server::new_async().await;

        let mock = server
            .mock("POST", "/oauth/access_token")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::AccessToken.read().await)
            .create_async()
            .await;

        let mut config = test::fixtures::config().await.with_mock_url(server.url());

        config
            .clone()
            .create()
            .await
            .expect("Failed to create config asynchronously in oauth test");

        assert_eq!(config.token, Some(String::from("alreadycreated")));
        let (test_tx, test_rx) = oneshot::channel::<()>();
        let login_handle = tokio::spawn(async move {
            login(&mut config, Some(test_tx))
                .await
                .expect("Login async operation failed")
        });

        test_rx
            .await
            .expect("Failed to await test receiver completion");

        let params = [("code", "state"), ("state", FAKE_UUID)];
        let client = reqwest::Client::new();
        let resp = client
            .get("http://127.0.0.1:8080/")
            .query(&params)
            .send()
            .await
            .expect("Failed to send callback");

        assert!(resp.status().is_success());
        let body = resp
            .text()
            .await
            .expect("Failed to get text from response asynchronously");
        assert!(body.contains("Success"));

        let result = login_handle
            .await
            .expect("Failed to await login handle completion");
        assert_eq!(result, String::from("✓"));
        mock.assert()
    }

    #[tokio::test]
    async fn receive_callback_with_error_param() {
        let (test_tx, test_rx) = oneshot::channel::<()>();
        let csrf_token = new_uuid();

        // Spawn the server on a random port in test mode
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("Failed to bind TCP listener asynchronously");
        let port = listener
            .local_addr()
            .expect("Failed to get local address")
            .port();
        // Move a clone into the server task, keep the original for the request
        let server_handle = tokio::spawn({
            let csrf_token = csrf_token.clone();
            async move { receive_callback(&csrf_token, Some(test_tx), listener).await }
        });

        test_rx
            .await
            .expect("Failed to await test receiver completion");

        // Simulate callback with error
        let params = [("error", "access_denied"), ("state", &csrf_token)];
        let client = reqwest::Client::new();
        let resp = client
            .get(format!("http://127.0.0.1:{port}/"))
            .query(&params)
            .send()
            .await
            .expect("Failed to send callback");

        assert!(resp.status().is_success());
        let body = resp
            .text()
            .await
            .expect("Failed to get text from response asynchronously");
        assert!(body.contains("Error"));

        let result = server_handle
            .await
            .expect("Failed to await server handle completion");
        assert!(result.is_err());
        let err = result.expect_err("Expected error result but got success");
        assert!(err.to_string().contains("access_denied"));
    }

    #[tokio::test]
    async fn receive_callback_with_invalid_csrf() {
        let (test_tx, test_rx) = oneshot::channel::<()>();
        let csrf_token = new_uuid();

        // Bind to a random port for the callback server
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("Failed to bind TCP listener asynchronously");
        let port = listener
            .local_addr()
            .expect("Failed to get local address")
            .port();
        let server_handle =
            tokio::spawn(
                async move { receive_callback(&csrf_token, Some(test_tx), listener).await },
            );

        test_rx
            .await
            .expect("Failed to await test receiver completion");

        // Simulate callback with mismatched csrf_token
        let params = [("code", "somecode"), ("state", "not-the-csrf-token")];
        let client = reqwest::Client::new();
        let resp = client
            .get(format!("http://127.0.0.1:{port}/"))
            .query(&params)
            .send()
            .await
            .expect("Failed to send callback");

        assert!(resp.status().is_success());

        let result = server_handle
            .await
            .expect("Failed to await server handle completion");
        assert!(result.is_err());
        let err = result.expect_err("Expected error result but got success");
        assert!(
            err.to_string().contains("state doesn't match csrf token"),
            "Unexpected error: {err}"
        );
    }

    #[test]
    fn test_print_oauth_url_returns_csrf_token() {
        // In test mode, new_uuid() returns FAKE_UUID
        let csrf_token = print_oauth_url(&Config::default());
        assert_eq!(csrf_token, FAKE_UUID);

        // Optionally, check that the formatted URL contains the CSRF token
        let expected_url_part = format!("state={FAKE_UUID}");
        let url = format!(
            "https://todoist.com{OAUTH_URL}?client_id={CLIENT_ID}&scope={SCOPE}&state={FAKE_UUID}"
        );
        let formatted_url = maybe_format_url(&url, &Config::default());
        assert!(formatted_url.contains(&expected_url_part));
    }
}
