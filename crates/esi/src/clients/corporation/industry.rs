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
