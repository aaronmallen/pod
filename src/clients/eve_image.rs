use std::sync::Arc;

use crate::clients::{self, http};

const BASE_URL: &str = "https://images.evetech.net";

pub struct Client {
  base_url: String,
  http: Arc<http::Client>,
}

impl Client {
  pub fn new(http: Arc<http::Client>) -> Self {
    Self {
      base_url: BASE_URL.to_owned(),
      http,
    }
  }

  #[cfg(test)]
  pub fn with_base_url(http: Arc<http::Client>, base_url: impl Into<String>) -> Self {
    Self {
      base_url: base_url.into(),
      http,
    }
  }

  pub fn alliance_logo_url(&self, alliance_id: i64, size: Size) -> String {
    self.url("alliances", alliance_id, "logo", size)
  }

  pub fn character_portrait_url(&self, character_id: i64, size: Size) -> String {
    self.url("characters", character_id, "portrait", size)
  }

  pub fn corporation_logo_url(&self, corporation_id: i64, size: Size) -> String {
    self.url("corporations", corporation_id, "logo", size)
  }

  pub async fn fetch(&self, url: &str) -> Result<Vec<u8>, clients::Error> {
    self.http.get_bytes_uncached(url).await
  }

  pub fn type_icon_url(&self, type_id: i64, size: Size) -> String {
    self.url("types", type_id, "icon", size)
  }

  fn url(&self, category: &str, id: i64, variant: &str, size: Size) -> String {
    let base = self.base_url.trim_end_matches('/');
    format!("{base}/{category}/{id}/{variant}?size={}", size as u16)
  }
}

// Complete catalog of the EVE image-server `size=` values; not every size is wired into production yet
// (S32/S128/S1024 are currently exercised only by tests or reserved), so the catalog stays whole.
#[allow(dead_code)]
// Rule-4 exception: variants stay ordered by ascending pixel size rather than alphabetically because each
// discriminant is the literal EVE image-server `size=` value (used via `size as u16`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Size {
  S32 = 32,
  S64 = 64,
  S128 = 128,
  S256 = 256,
  S512 = 512,
  S1024 = 1024,
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::store;

  async fn make_client() -> Client {
    let db = store::open_test().await.unwrap();
    let http = http::Client::builder(http::Cache::new(db)).build();
    Client::new(http)
  }

  mod fetch {
    use pretty_assertions::assert_eq;
    use wiremock::{
      Mock, MockServer, ResponseTemplate,
      matchers::{method, path},
    };

    use super::*;

    #[tokio::test]
    async fn it_fetches_image_bytes_for_a_built_url() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/characters/42/portrait"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(vec![7u8, 7, 7], "image/png"))
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      let http = http::Client::builder(http::Cache::new(db)).build();
      let client = Client::with_base_url(http, server.uri());
      let url = client.character_portrait_url(42, Size::S64);

      let bytes = client.fetch(&url).await.unwrap();

      assert_eq!(bytes, vec![7u8, 7, 7]);
    }
  }

  mod url {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_builds_a_character_portrait_url() {
      let client = make_client().await;

      assert_eq!(
        client.character_portrait_url(95_465_499, Size::S256),
        "https://images.evetech.net/characters/95465499/portrait?size=256"
      );
    }

    #[tokio::test]
    async fn it_builds_corporation_and_alliance_logo_urls() {
      let client = make_client().await;

      assert_eq!(
        client.corporation_logo_url(98_000_001, Size::S64),
        "https://images.evetech.net/corporations/98000001/logo?size=64"
      );
      assert_eq!(
        client.alliance_logo_url(99_000_001, Size::S128),
        "https://images.evetech.net/alliances/99000001/logo?size=128"
      );
    }

    #[tokio::test]
    async fn it_builds_a_type_icon_url() {
      let client = make_client().await;

      assert_eq!(
        client.type_icon_url(587, Size::S64),
        "https://images.evetech.net/types/587/icon?size=64"
      );
    }
  }
}
