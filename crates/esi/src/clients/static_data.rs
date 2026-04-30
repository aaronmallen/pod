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
