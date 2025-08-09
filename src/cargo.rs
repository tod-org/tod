use reqwest::Client;
use reqwest::header::USER_AGENT;
use serde::Deserialize;

use crate::VERSION;
use crate::errors::Error;

// CRATES.IO URLS
const VERSIONS_URL: &str = "/v1/crates/tod/versions";

#[derive(Deserialize)]
struct CargoResponse {
    versions: Vec<CargoVersion>,
}

#[derive(Deserialize)]
struct CargoVersion {
    num: String,
}

#[derive(PartialEq, Eq, Debug)]
pub enum Version {
    Latest,
    Dated(String),
}
pub async fn compare_versions(_mock_url: Option<String>) -> Result<Version, Error> {
    match get_latest_version(_mock_url).await {
        Ok(version) if version.as_str() != VERSION => Ok(Version::Dated(version)),
        Ok(_) => Ok(Version::Latest),
        Err(err) => Err(err),
    }
}
/// Get latest version number from Cargo.io
pub async fn get_latest_version(mock_url: Option<String>) -> Result<String, Error> {
    let cargo_url = if cfg!(test) {
        mock_url.expect("Mock URL not set")
    } else {
        "https://crates.io/api".to_string()
    };

    let request_url = format!("{cargo_url}{VERSIONS_URL}");

    let response = Client::new()
        .get(request_url)
        .header(USER_AGENT, format!("Tod/{VERSION}"))
        .send()
        .await?;

    if response.status().is_success() {
        let cr: CargoResponse = serde_json::from_str(&response.text().await?)?;
        Ok(cr
            .versions
            .first()
            .expect("crates.io response contained no versions")
            .num
            .clone())
    } else {
        let message = format!("Error: {:#?}", response.text().await);
        let source = "get_latest_version response failure".to_string();
        Err(Error { message, source })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{VERSION, test::responses::ResponseFromFile};
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn test_get_latest_version() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/v1/crates/tod/versions")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::Versions.read().await)
            .create_async()
            .await;

        let response = get_latest_version(Some(server.url())).await;
        mock.assert();

        assert_eq!(response, Ok(String::from(VERSION)));
    }

    #[tokio::test]
    async fn test_compare_versions() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/v1/crates/tod/versions")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ResponseFromFile::Versions.read().await)
            .create_async()
            .await;

        let response = compare_versions(Some(server.url())).await;
        mock.assert();

        assert_eq!(response, Ok(Version::Latest));
    }
    // Manually specify the current version is 999.99.99 to test outdated version detection
    #[tokio::test]
    async fn test_compare_versions_outdated() {
        let mut server = mockito::Server::new_async().await;
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

        let response = compare_versions(Some(server.url())).await;
        mock.assert();

        assert_eq!(response, Ok(Version::Dated("999.99.999".into())));
    }
}
