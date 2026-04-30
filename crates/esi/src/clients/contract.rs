//! Client for EVE public contract ESI endpoints.

use crate::{
  Error,
  models::contract::{ContractBid, ContractItem, PublicContract},
};

/// Client for public contract ESI endpoints.
pub struct Client<'a> {
  esi: &'a crate::Client,
}

impl<'a> Client<'a> {
  /// Creates a new `Client`.
  pub(crate) fn new(esi: &'a crate::Client) -> Self {
    Self {
      esi,
    }
  }

  /// Returns bids on an auction contract.
  pub async fn bids(&self, contract_id: i64) -> Result<Vec<ContractBid>, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v1/contracts/public/bids/{contract_id}/"))
          .build(),
        None,
      )
      .await
  }

  /// Returns items in a public contract.
  pub async fn items(&self, contract_id: i64) -> Result<Vec<ContractItem>, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v1/contracts/public/items/{contract_id}/"))
          .build(),
        None,
      )
      .await
  }

  /// Returns public contracts in the given region (paginated).
  pub async fn region(&self, region_id: i64) -> Result<Vec<PublicContract>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self
          .esi
          .url_builder()
          .path(format!("v1/contracts/public/{region_id}/"))
          .build(),
        None,
      )
      .await
  }
}
