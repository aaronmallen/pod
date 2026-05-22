//! Client for the EVE image server (images.evetech.net).

use crate::http::UrlBuilder;

const BASE_URL: &str = "https://images.evetech.net";

/// Client for fetching images from the EVE image server.
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

  pub async fn alliance_logo(&self, alliance_id: i64, size: u32) -> Result<Vec<u8>, crate::Error> {
    let url = self
      .url_builder()
      .path(format!("alliances/{alliance_id}/logo"))
      .param("size", size.to_string())
      .build();

    self.esi.http().get_bytes(&url).await
  }

  /// Fetches the corporation logo at the requested pixel size.
  ///
  /// Valid sizes: 32, 64, 128, 256.
  pub async fn corporation_logo(&self, corporation_id: i64, size: u32) -> Result<Vec<u8>, crate::Error> {
    let url = self
      .url_builder()
      .path(format!("corporations/{corporation_id}/logo"))
      .param("size", size.to_string())
      .build();

    self.esi.http().get_bytes(&url).await
  }

  /// Fetches the portrait for the given character at the requested pixel size.
  ///
  /// Valid sizes: 32, 64, 128, 256, 512, 1024.
  pub async fn character_portrait(&self, character_id: i64, size: u32) -> Result<Vec<u8>, crate::Error> {
    let url = self
      .url_builder()
      .path(format!("characters/{character_id}/portrait"))
      .param("size", size.to_string())
      .build();

    self.esi.http().get_bytes(&url).await
  }

  pub async fn type_bpc(&self, type_id: i64, size: u32) -> Result<Vec<u8>, crate::Error> {
    let url = self
      .url_builder()
      .path(format!("types/{type_id}/bpc"))
      .param("size", size.to_string())
      .build();

    self.esi.http().get_bytes(&url).await
  }

  pub async fn type_bpo(&self, type_id: i64, size: u32) -> Result<Vec<u8>, crate::Error> {
    let url = self
      .url_builder()
      .path(format!("types/{type_id}/bpo"))
      .param("size", size.to_string())
      .build();

    self.esi.http().get_bytes(&url).await
  }

  pub async fn type_icon(&self, type_id: i64, size: u32) -> Result<Vec<u8>, crate::Error> {
    let url = self
      .url_builder()
      .path(format!("types/{type_id}/icon"))
      .param("size", size.to_string())
      .build();

    self.esi.http().get_bytes(&url).await
  }

  pub async fn type_relic(&self, type_id: i64, size: u32) -> Result<Vec<u8>, crate::Error> {
    let url = self
      .url_builder()
      .path(format!("types/{type_id}/relic"))
      .param("size", size.to_string())
      .build();

    self.esi.http().get_bytes(&url).await
  }

  pub async fn type_render(&self, type_id: i64, size: u32) -> Result<Vec<u8>, crate::Error> {
    let url = self
      .url_builder()
      .path(format!("types/{type_id}/render"))
      .param("size", size.to_string())
      .build();

    self.esi.http().get_bytes(&url).await
  }

  fn url_builder(&self) -> UrlBuilder {
    UrlBuilder::new(BASE_URL)
  }
}

#[cfg(test)]
mod tests {
  mod alliance_logo {
    use pretty_assertions::assert_eq;

    use crate::http::UrlBuilder;

    #[test]
    fn it_builds_the_correct_url() {
      let url = UrlBuilder::new(super::super::BASE_URL)
        .path("alliances/12345/logo")
        .param("size", "64")
        .build();

      assert_eq!(url, "https://images.evetech.net/alliances/12345/logo?size=64");
    }
  }

  mod character_portrait {
    use pretty_assertions::assert_eq;

    use crate::http::UrlBuilder;

    #[test]
    fn it_builds_the_correct_url() {
      let url = UrlBuilder::new(super::super::BASE_URL)
        .path("characters/9876/portrait")
        .param("size", "512")
        .build();

      assert_eq!(url, "https://images.evetech.net/characters/9876/portrait?size=512");
    }
  }

  mod corporation_logo {
    use pretty_assertions::assert_eq;

    use crate::http::UrlBuilder;

    #[test]
    fn it_builds_the_correct_url() {
      let url = UrlBuilder::new(super::super::BASE_URL)
        .path("corporations/54321/logo")
        .param("size", "128")
        .build();

      assert_eq!(url, "https://images.evetech.net/corporations/54321/logo?size=128");
    }
  }

  mod type_icon {
    use pretty_assertions::assert_eq;

    use crate::http::UrlBuilder;

    #[test]
    fn it_builds_the_correct_url() {
      let url = UrlBuilder::new(super::super::BASE_URL)
        .path("types/34/icon")
        .param("size", "32")
        .build();

      assert_eq!(url, "https://images.evetech.net/types/34/icon?size=32");
    }
  }
}
