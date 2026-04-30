//! Character contract endpoints.

use crate::{
  Error,
  clients::character::AuthenticatedClient,
  models::character::{CharacterContract, ContractBid, ContractItem},
};

impl AuthenticatedClient<'_> {
  /// Returns all contracts for this character (paginated).
  pub async fn contracts(&self) -> Result<Vec<CharacterContract>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self
          .esi
          .url_builder()
          .path(format!("v1/characters/{}/contracts/", self.id))
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
          .path(format!("v1/characters/{}/contracts/{contract_id}/bids/", self.id))
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
          .path(format!("v1/characters/{}/contracts/{contract_id}/items/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }
}
