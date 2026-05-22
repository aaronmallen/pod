//! Corporation industry and mining endpoints.

use crate::{
  Error,
  clients::corporation::AuthenticatedClient,
  models::corporation::{CorporationIndustryJob, MiningExtraction, MiningObserver, MiningObserverEntry},
};

impl AuthenticatedClient<'_> {
  /// Returns industry jobs for this corporation (paginated).
  pub async fn industry_jobs(&self) -> Result<Vec<CorporationIndustryJob>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self
          .esi
          .url_builder()
          .path(format!("v1/corporations/{}/industry/jobs/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns moon mining extractions for this corporation.
  pub async fn mining_extractions(&self) -> Result<Vec<MiningExtraction>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self
          .esi
          .url_builder()
          .path(format!("v1/corporations/{}/mining/extractions/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns mining observers for this corporation (paginated).
  pub async fn mining_observers(&self) -> Result<Vec<MiningObserver>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self
          .esi
          .url_builder()
          .path(format!("v1/corporations/{}/mining/observers/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns the ledger for a specific mining observer (paginated).
  pub async fn mining_observer_ledger(&self, observer_id: i64) -> Result<Vec<MiningObserverEntry>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self
          .esi
          .url_builder()
          .path(format!("v1/corporations/{}/mining/observers/{observer_id}/", self.id))
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

  mod industry_jobs {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_industry_jobs_on_success() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v1/corporations/109299958/industry/jobs/"))
        .respond_with(
          ResponseTemplate::new(200)
            .insert_header("X-Pages", "1")
            .set_body_json(serde_json::json!([
              {
                "activity_id": 1,
                "blueprint_id": 1_000_000_001i64,
                "blueprint_location_id": 1_000_000_002i64,
                "blueprint_type_id": 820,
                "duration": 3600,
                "end_date": "2025-01-01T12:00:00Z",
                "facility_id": 1_000_000_003i64,
                "installer_id": 123_456_789i64,
                "job_id": 9_000_001i64,
                "location_id": 1_000_000_004i64,
                "output_location_id": 1_000_000_005i64,
                "runs": 10,
                "start_date": "2025-01-01T11:00:00Z",
                "status": "active"
              }
            ])),
        )
        .mount(&server)
        .await;

      let esi = crate::Client::builder("test-client")
        .base_url(server.uri())
        .build()
        .unwrap();
      let corp = esi.corporation(109_299_958i64);
      let grant = make_grant();
      let auth = corp.auth(&grant);

      let result = auth.industry_jobs().await.unwrap();

      assert_eq!(result.len(), 1);
      assert_eq!(result[0].job_id, 9_000_001i64);
      assert_eq!(result[0].status, "active");
    }

    #[tokio::test]
    async fn it_returns_api_error_on_401() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v1/corporations/109299958/industry/jobs/"))
        .respond_with(
          ResponseTemplate::new(401)
            .insert_header("X-Pages", "1")
            .set_body_json(serde_json::json!({"error": "Unauthorized"})),
        )
        .mount(&server)
        .await;

      let esi = crate::Client::builder("test-client")
        .base_url(server.uri())
        .build()
        .unwrap();
      let corp = esi.corporation(109_299_958i64);
      let grant = make_grant();
      let auth = corp.auth(&grant);

      let result = auth.industry_jobs().await;

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
