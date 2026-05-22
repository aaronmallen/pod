//! Client for downloading EVE static data exports.

use crate::Error;

const SDE_YAML_URL: &str = "https://developers.eveonline.com/static-data/eve-online-static-data-latest-yaml.zip";
const SDE_JSONL_URL: &str = "https://developers.eveonline.com/static-data/eve-online-static-data-latest-jsonl.zip";

/// Client for downloading static EVE data (SDE) archives.
pub struct Client<'a> {
  esi: &'a crate::Client,
}

impl<'a> Client<'a> {
  /// Creates a new `Client`.
  pub(crate) fn new(esi: &'a crate::Client) -> Self {
    Self {
      esi,
    }
  }

  /// Streams the latest JSONL SDE zip to `dest`, with a 10-minute timeout.
  pub async fn download_jsonl(&self, dest: &std::path::Path) -> Result<(), Error> {
    self.esi.http().download_to_file(SDE_JSONL_URL, dest, 600).await
  }

  /// Streams the latest YAML SDE zip to `dest`, with a 10-minute timeout.
  pub async fn download_yaml(&self, dest: &std::path::Path) -> Result<(), Error> {
    self.esi.http().download_to_file(SDE_YAML_URL, dest, 600).await
  }
}

#[cfg(test)]
mod tests {
  mod download_jsonl {
    use wiremock::{
      Mock, MockServer, ResponseTemplate,
      matchers::{method, path},
    };

    #[tokio::test]
    async fn it_streams_archive_to_file_on_success() {
      let server = MockServer::start().await;
      let dest = std::env::temp_dir().join("pod_esi_sde_jsonl_test.zip");
      let url = format!("{}/static-data/eve-online-static-data-latest-jsonl.zip", server.uri());
      Mock::given(method("GET"))
        .and(path("/static-data/eve-online-static-data-latest-jsonl.zip"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(b"PK\x03\x04".to_vec(), "application/zip"))
        .mount(&server)
        .await;

      let esi = crate::Client::builder("test-client")
        .base_url(&server.uri())
        .build()
        .unwrap();
      let result = esi.http().download_to_file(&url, &dest, 30).await;

      let _ = std::fs::remove_file(&dest);
      assert!(result.is_ok());
    }

    #[tokio::test]
    async fn it_returns_error_on_api_failure() {
      let server = MockServer::start().await;
      let dest = std::env::temp_dir().join("pod_esi_sde_jsonl_err_test.zip");
      let url = format!("{}/static-data/eve-online-static-data-latest-jsonl.zip", server.uri());
      Mock::given(method("GET"))
        .and(path("/static-data/eve-online-static-data-latest-jsonl.zip"))
        .respond_with(ResponseTemplate::new(503).set_body_raw(r#"{"error":"Service Unavailable"}"#, "application/json"))
        .mount(&server)
        .await;

      let esi = crate::Client::builder("test-client")
        .base_url(&server.uri())
        .build()
        .unwrap();
      let result = esi.http().download_to_file(&url, &dest, 30).await;

      let _ = std::fs::remove_file(&dest);
      assert!(result.is_err());
    }
  }
}
