//! Corporation contract endpoints.

use crate::{
  Error,
  clients::corporation::AuthenticatedClient,
  models::corporation::{ContractBid, ContractItem, CorporationContract},
};

impl AuthenticatedClient<'_> {
  /// Returns all contracts for this corporation (paginated).
  pub async fn contracts(&self) -> Result<Vec<CorporationContract>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self
          .esi
          .url_builder()
          .path(format!("v1/corporations/{}/contracts/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns the bids on a contract.
  pub async fn contract_bids(&self, contract_id: i64) -> Result<Vec<ContractBid>, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v1/corporations/{}/contracts/{contract_id}/bids/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns the items in a contract.
  pub async fn contract_items(&self, contract_id: i64) -> Result<Vec<ContractItem>, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v1/corporations/{}/contracts/{contract_id}/items/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }
}
