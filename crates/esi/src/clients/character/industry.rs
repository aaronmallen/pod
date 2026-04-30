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
