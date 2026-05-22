//! Character industry and mining endpoints.

use crate::{
  Error,
  clients::character::AuthenticatedClient,
  models::character::{AgentResearch, IndustryJob, JumpFatigue, MiningEntry},
};

impl AuthenticatedClient<'_> {
  /// Returns agent research data for this character.
  pub async fn agent_research(&self) -> Result<Vec<AgentResearch>, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v2/characters/{}/agents_research/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns industry jobs for this character (paginated).
  pub async fn industry_jobs(&self) -> Result<Vec<IndustryJob>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self
          .esi
          .url_builder()
          .path(format!("v1/characters/{}/industry/jobs/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns jump fatigue data for this character.
  pub async fn jump_fatigue(&self) -> Result<JumpFatigue, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v1/characters/{}/fatigue/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns mining ledger entries for this character (paginated).
  pub async fn mining_ledger(&self) -> Result<Vec<MiningEntry>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self
          .esi
          .url_builder()
          .path(format!("v1/characters/{}/mining/", self.id))
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
      90_000_001i64,
      "Test Char",
      SystemTime::now() + Duration::from_secs(3600),
      "refresh",
      vec![],
    )
  }

  mod industry_jobs {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_industry_jobs_for_character() {
      let server = MockServer::start().await;

      Mock::given(method("GET"))
        .and(path("/v1/characters/90000001/industry/jobs/"))
        .respond_with(
          ResponseTemplate::new(200)
            .insert_header("X-Pages", "1")
            .set_body_json(serde_json::json!([
              {
                "activity_id": 1,
                "blueprint_id": 1000000000001i64,
                "blueprint_location_id": 60006382i64,
                "blueprint_type_id": 948,
                "duration": 548,
                "end_date": "2025-01-01T12:00:00Z",
                "facility_id": 60006382i64,
                "installer_id": 90000001,
                "job_id": 123456789,
                "output_location_id": 60006382i64,
                "runs": 1,
                "start_date": "2025-01-01T00:00:00Z",
                "station_id": 60006382i64,
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
      let grant = make_grant();
      let auth = esi.character(&grant);

      let result = auth.industry_jobs().await.unwrap();

      assert_eq!(result.len(), 1);
      assert_eq!(result[0].job_id, 123_456_789i64);
      assert_eq!(result[0].status, "active");
      assert_eq!(result[0].activity_id, 1i32);
    }

    #[tokio::test]
    async fn it_returns_error_on_401() {
      let server = MockServer::start().await;

      Mock::given(method("GET"))
        .and(path("/v1/characters/90000001/industry/jobs/"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({"error": "Unauthorized"})))
        .mount(&server)
        .await;

      let esi = crate::Client::builder("test-client")
        .base_url(server.uri())
        .build()
        .unwrap();
      let grant = make_grant();
      let auth = esi.character(&grant);

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
