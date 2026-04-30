//! Corporation wallet and market order endpoints.

use crate::{
  Error,
  clients::corporation::AuthenticatedClient,
  models::corporation::{
    CorporationOrder, CorporationWallet, CorporationWalletJournalEntry, CorporationWalletTransaction,
  },
};

impl AuthenticatedClient<'_> {
  /// Returns active market orders for this corporation (paginated).
  pub async fn orders(&self) -> Result<Vec<CorporationOrder>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self
          .esi
          .url_builder()
          .path(format!("v3/corporations/{}/orders/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns historical market orders for this corporation (paginated).
  pub async fn order_history(&self) -> Result<Vec<CorporationOrder>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self
          .esi
          .url_builder()
          .path(format!("v2/corporations/{}/orders/history/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns the wallet divisions and balances for this corporation.
  pub async fn wallets(&self) -> Result<Vec<CorporationWallet>, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v1/corporations/{}/wallets/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns the wallet journal for a division of this corporation (paginated).
  pub async fn wallet_journal(&self, division: i32) -> Result<Vec<CorporationWalletJournalEntry>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self
          .esi
          .url_builder()
          .path(format!("v4/corporations/{}/wallets/{division}/journal/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns the wallet transactions for a division of this corporation (paginated).
  pub async fn wallet_transactions(&self, division: i32) -> Result<Vec<CorporationWalletTransaction>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self
          .esi
          .url_builder()
          .path(format!("v1/corporations/{}/wallets/{division}/transactions/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }
}
