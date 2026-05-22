//! Character skills, attributes, standings, and notification endpoints.

use crate::{
  Error,
  clients::character::AuthenticatedClient,
  models::character::{
    CharacterAttributes, CharacterFwStats, CharacterMedal, CharacterNotification, CharacterPlanet, CharacterRoles,
    CharacterSkills, CharacterStanding, CharacterTitle, ContactNotification, PlanetColony, SkillQueueEntry,
  },
};

impl AuthenticatedClient<'_> {
  /// Returns the attribute point allocation for this character.
  pub async fn attributes(&self) -> Result<CharacterAttributes, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v1/characters/{}/attributes/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns contact notifications for this character.
  pub async fn contact_notifications(&self) -> Result<Vec<ContactNotification>, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v2/characters/{}/notifications/contacts/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns faction warfare stats for this character.
  pub async fn fw_stats(&self) -> Result<CharacterFwStats, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v2/characters/{}/fw/stats/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns medals awarded to this character.
  pub async fn medals(&self) -> Result<Vec<CharacterMedal>, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v2/characters/{}/medals/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns notifications for this character.
  pub async fn notifications(&self) -> Result<Vec<CharacterNotification>, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v6/characters/{}/notifications/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns the planetary colonies managed by this character.
  pub async fn planets(&self) -> Result<Vec<CharacterPlanet>, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v1/characters/{}/planets/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns the layout of a specific planetary colony.
  pub async fn planet_colony(&self, planet_id: i64) -> Result<PlanetColony, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v3/characters/{}/planets/{planet_id}/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns corporation roles held by this character.
  pub async fn roles(&self) -> Result<CharacterRoles, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v3/characters/{}/roles/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns the skill training queue for this character.
  pub async fn skill_queue(&self) -> Result<Vec<SkillQueueEntry>, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v2/characters/{}/skillqueue/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns skill training data and total SP for this character.
  pub async fn skills(&self) -> Result<CharacterSkills, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v4/characters/{}/skills/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns the standings of this character toward NPCs and players.
  pub async fn standings(&self) -> Result<Vec<CharacterStanding>, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v2/characters/{}/standings/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns the titles held by this character.
  pub async fn titles(&self) -> Result<Vec<CharacterTitle>, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v2/characters/{}/titles/", self.id))
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

  mod skills {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_skills_on_success() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v4/characters/90000001/skills/"))
        .respond_with(
          ResponseTemplate::new(200).set_body_raw(r#"{"skills": [], "total_sp": 5000000}"#, "application/json"),
        )
        .mount(&server)
        .await;
      let esi = crate::Client::builder("test-client")
        .base_url(server.uri())
        .build()
        .unwrap();
      let grant = make_grant();
      let auth = esi.character(&grant);

      let result = auth.skills().await.unwrap();

      assert_eq!(result.total_sp, 5000000);
      assert_eq!(result.skills.len(), 0);
    }

    #[tokio::test]
    async fn it_returns_api_error_on_401() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v4/characters/90000001/skills/"))
        .respond_with(ResponseTemplate::new(401).set_body_raw(r#"{"error": "Unauthorized"}"#, "application/json"))
        .mount(&server)
        .await;
      let esi = crate::Client::builder("test-client")
        .base_url(server.uri())
        .build()
        .unwrap();
      let grant = make_grant();
      let auth = esi.character(&grant);

      let result = auth.skills().await;

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
