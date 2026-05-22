//! Corporation member endpoints.

use crate::{
  Error,
  clients::corporation::AuthenticatedClient,
  models::corporation::{MemberRoles, MemberTitleEntry, MemberTracking, RoleHistoryEntry},
};

impl AuthenticatedClient<'_> {
  /// Returns the list of member character IDs for this corporation.
  pub async fn members(&self) -> Result<Vec<i64>, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v4/corporations/{}/members/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns the member limit for this corporation.
  pub async fn member_limit(&self) -> Result<i32, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v2/corporations/{}/members/limit/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns the roles assigned to each member.
  pub async fn member_roles(&self) -> Result<Vec<MemberRoles>, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v2/corporations/{}/roles/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns the role history for members of this corporation (paginated).
  pub async fn member_role_history(&self) -> Result<Vec<RoleHistoryEntry>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self
          .esi
          .url_builder()
          .path(format!("v2/corporations/{}/roles/history/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns the titles held by each member.
  pub async fn member_titles(&self) -> Result<Vec<MemberTitleEntry>, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v2/corporations/{}/members/titles/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns tracking data for all members (paginated).
  pub async fn member_tracking(&self) -> Result<Vec<MemberTracking>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self
          .esi
          .url_builder()
          .path(format!("v2/corporations/{}/membertracking/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns the shareholders of this corporation (paginated).
  pub async fn shareholders(&self) -> Result<Vec<crate::models::corporation::Shareholder>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self
          .esi
          .url_builder()
          .path(format!("v1/corporations/{}/shareholders/", self.id))
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

  mod members {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_member_ids_on_success() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v4/corporations/109299958/members/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([90_000_001i64, 90_000_002i64])))
        .mount(&server)
        .await;

      let esi = crate::Client::builder("test-client")
        .base_url(server.uri())
        .build()
        .unwrap();
      let corp = esi.corporation(109_299_958i64);
      let grant = make_grant();
      let auth = corp.auth(&grant);

      let result = auth.members().await.unwrap();

      assert_eq!(result.len(), 2);
      assert_eq!(result[0], 90_000_001i64);
      assert_eq!(result[1], 90_000_002i64);
    }

    #[tokio::test]
    async fn it_returns_api_error_on_401() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v4/corporations/109299958/members/"))
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

      let result = auth.members().await;

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
