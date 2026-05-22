//! Client for EVE market ESI endpoints.

use crate::{
  Client as EsiClient, Error,
  models::{
    auth::Grant,
    market::{MarketGroup, MarketHistory, MarketOrder, MarketPrice},
  },
};

/// Client for market ESI endpoints.
pub struct Client<'a> {
  pub(super) esi: &'a EsiClient,
}

impl<'a> Client<'a> {
  /// Creates a new `Client`.
  pub(crate) fn new(esi: &'a EsiClient) -> Self {
    Self {
      esi,
    }
  }

  /// Returns a market group by ID.
  pub async fn group(&self, id: i32) -> Result<MarketGroup, Error> {
    self
      .esi
      .http()
      .get_json(
        &self.esi.url_builder().path(format!("v1/markets/groups/{id}/")).build(),
        None,
      )
      .await
  }

  /// Returns all market group IDs.
  pub async fn group_ids(&self) -> Result<Vec<i32>, Error> {
    self
      .esi
      .http()
      .get_json(&self.esi.url_builder().path("v1/markets/groups/").build(), None)
      .await
  }

  /// Returns current market prices for all types.
  pub async fn prices(&self) -> Result<Vec<MarketPrice>, Error> {
    self
      .esi
      .http()
      .get_json(&self.esi.url_builder().path("v1/markets/prices/").build(), None)
      .await
  }

  /// Returns a client scoped to the given region.
  pub fn region(&self, region_id: i64) -> RegionClient<'_> {
    RegionClient {
      esi: self.esi,
      region_id,
    }
  }

  /// Returns a client scoped to the given structure.
  pub fn structure(&self, structure_id: i64) -> StructureClient<'_> {
    StructureClient {
      esi: self.esi,
      structure_id,
    }
  }
}

/// Market client scoped to a specific region.
pub struct RegionClient<'a> {
  pub(super) esi: &'a EsiClient,
  pub(super) region_id: i64,
}

impl<'a> RegionClient<'a> {
  /// Returns market history for a type in this region.
  pub async fn history(&self, type_id: i32) -> Result<Vec<MarketHistory>, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v1/markets/{}/history/", self.region_id))
          .param("type_id", type_id.to_string())
          .build(),
        None,
      )
      .await
  }

  /// Returns all orders in this region (paginated).
  pub async fn orders(&self) -> Result<Vec<MarketOrder>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self
          .esi
          .url_builder()
          .path(format!("v1/markets/{}/orders/", self.region_id))
          .build(),
        None,
      )
      .await
  }

  /// Returns all type IDs with orders in this region (paginated).
  pub async fn type_ids(&self) -> Result<Vec<i32>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self
          .esi
          .url_builder()
          .path(format!("v1/markets/{}/types/", self.region_id))
          .build(),
        None,
      )
      .await
  }
}

/// Market client scoped to a specific player-owned structure.
pub struct StructureClient<'a> {
  pub(super) esi: &'a EsiClient,
  pub(super) structure_id: i64,
}

impl<'a> StructureClient<'a> {
  /// Returns an authenticated structure market client bound to the given grant.
  pub fn auth<'b>(&'b self, grant: &'b Grant) -> AuthenticatedStructureClient<'b> {
    AuthenticatedStructureClient {
      esi: self.esi,
      grant,
      structure_id: self.structure_id,
    }
  }
}

/// Authenticated market client for a player-owned structure.
pub struct AuthenticatedStructureClient<'a> {
  pub(super) esi: &'a EsiClient,
  pub(super) grant: &'a Grant,
  pub(super) structure_id: i64,
}

impl<'a> AuthenticatedStructureClient<'a> {
  /// Returns all orders in this structure (paginated).
  pub async fn orders(&self) -> Result<Vec<MarketOrder>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self
          .esi
          .url_builder()
          .path(format!("v1/markets/structures/{}/", self.structure_id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }
}

#[cfg(test)]
mod tests {
  use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
  };

  fn make_esi(server_uri: &str) -> crate::Client {
    crate::Client::builder("test-client")
      .base_url(server_uri)
      .build()
      .unwrap()
  }

  mod prices {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_market_prices() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v1/markets/prices/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
          {"type_id": 34, "adjusted_price": 5.5, "average_price": 6.0}
        ])))
        .mount(&server)
        .await;

      let esi = make_esi(&server.uri());
      let result = esi.market().prices().await.unwrap();

      assert_eq!(result.len(), 1);
      assert_eq!(result[0].type_id, 34i32);
      assert_eq!(result[0].adjusted_price, Some(5.5f64));
      assert_eq!(result[0].average_price, Some(6.0f64));
    }

    #[tokio::test]
    async fn it_returns_error_on_500() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v1/markets/prices/"))
        .respond_with(ResponseTemplate::new(500).set_body_json(serde_json::json!({"error": "Internal server error"})))
        .mount(&server)
        .await;

      let esi = make_esi(&server.uri());
      let result = esi.market().prices().await;

      assert!(result.is_err());
    }
  }
}
