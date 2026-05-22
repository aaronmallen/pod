//! Miscellaneous corporation endpoints.

use crate::{
  Error,
  clients::corporation::AuthenticatedClient,
  models::corporation::{
    ContainerLog, CorporationDivisions, CorporationFwStats, CorporationMedal, CorporationStanding,
    CorporationStructure, CorporationTitle, CustomsOffice, Facility, IssuedMedal, Starbase, StarbaseDetail,
  },
};

impl AuthenticatedClient<'_> {
  /// Returns container access logs for this corporation (paginated).
  pub async fn container_logs(&self) -> Result<Vec<ContainerLog>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self
          .esi
          .url_builder()
          .path(format!("v2/corporations/{}/containers/logs/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns the customs offices owned by this corporation (paginated).
  pub async fn customs_offices(&self) -> Result<Vec<CustomsOffice>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self
          .esi
          .url_builder()
          .path(format!("v1/corporations/{}/customsoffices/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns the division names for this corporation.
  pub async fn divisions(&self) -> Result<CorporationDivisions, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v2/corporations/{}/divisions/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns the industry facilities used by this corporation.
  pub async fn facilities(&self) -> Result<Vec<Facility>, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v2/corporations/{}/facilities/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns faction warfare stats for this corporation.
  pub async fn fw_stats(&self) -> Result<CorporationFwStats, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v2/corporations/{}/fw/stats/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns medals created by this corporation (paginated).
  pub async fn medals(&self) -> Result<Vec<CorporationMedal>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self
          .esi
          .url_builder()
          .path(format!("v2/corporations/{}/medals/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns medals issued to members of this corporation (paginated).
  pub async fn issued_medals(&self) -> Result<Vec<IssuedMedal>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self
          .esi
          .url_builder()
          .path(format!("v2/corporations/{}/medals/issued/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns the standings of this corporation toward NPCs and players.
  pub async fn standings(&self) -> Result<Vec<CorporationStanding>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self
          .esi
          .url_builder()
          .path(format!("v2/corporations/{}/standings/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns the list of starbases (POSes) owned by this corporation (paginated).
  pub async fn starbases(&self) -> Result<Vec<Starbase>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self
          .esi
          .url_builder()
          .path(format!("v2/corporations/{}/starbases/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns details for a specific starbase.
  pub async fn starbase_detail(&self, starbase_id: i64, system_id: i64) -> Result<StarbaseDetail, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v2/corporations/{}/starbases/{starbase_id}/", self.id))
          .param("system_id", system_id.to_string())
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns the structures owned by this corporation (paginated).
  pub async fn structures(&self) -> Result<Vec<CorporationStructure>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self
          .esi
          .url_builder()
          .path(format!("v3/corporations/{}/structures/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns the titles defined in this corporation.
  pub async fn titles(&self) -> Result<Vec<CorporationTitle>, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v2/corporations/{}/titles/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }
}

#[cfg(test)]
mod tests {
  use std::time::{Duration, SystemTime};

  use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
  };

  fn make_grant() -> crate::models::auth::Grant {
    crate::models::auth::Grant::new(
      "test-token",
      123_456_789i64,
      "Test Member",
      SystemTime::now() + Duration::from_secs(3600),
      "refresh",
      vec![],
    )
  }

  mod divisions {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_divisions_on_success() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v2/corporations/109299958/divisions/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
          "hangar": [{"division": 1, "name": "Alpha Hangar"}],
          "wallet": [{"division": 1, "name": "Master Wallet"}]
        })))
        .mount(&server)
        .await;

      let esi = crate::Client::builder("test-client")
        .base_url(server.uri())
        .build()
        .unwrap();
      let corp = esi.corporation(109_299_958i64);
      let grant = make_grant();
      let auth = corp.auth(&grant);

      let result = auth.divisions().await.unwrap();

      let hangars = result.hangar.unwrap();
      assert_eq!(hangars.len(), 1);
      assert_eq!(hangars[0].division, 1);
      assert_eq!(hangars[0].name.as_deref(), Some("Alpha Hangar"));

      let wallets = result.wallet.unwrap();
      assert_eq!(wallets.len(), 1);
      assert_eq!(wallets[0].name.as_deref(), Some("Master Wallet"));
    }

    #[tokio::test]
    async fn it_returns_api_error_on_401() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v2/corporations/109299958/divisions/"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({"error": "Unauthorized"})))
        .mount(&server)
        .await;

      let esi = crate::Client::builder("test-client")
        .base_url(server.uri())
        .build()
        .unwrap();
      let corp = esi.corporation(109_299_958i64);
      let grant = make_grant();
      let auth = corp.auth(&grant);

      let result = auth.divisions().await;

      assert!(matches!(
        result,
        Err(crate::Error::Api {
          status: 401,
          ..
        })
      ));
    }
  }
}
