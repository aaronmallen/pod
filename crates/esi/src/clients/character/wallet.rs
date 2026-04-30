//! Character wallet and market order endpoints.

use crate::{
  Error,
  clients::character::AuthenticatedClient,
  models::character::{CharacterOrder, CharacterWalletBalance, WalletJournalEntry, WalletTransaction},
};

impl AuthenticatedClient<'_> {
  /// Returns all active market orders for this character.
  pub async fn orders(&self) -> Result<Vec<CharacterOrder>, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v2/characters/{}/orders/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns historical market orders for this character (paginated).
  pub async fn order_history(&self) -> Result<Vec<CharacterOrder>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self
          .esi
          .url_builder()
          .path(format!("v1/characters/{}/orders/history/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns the ISK balance of this character's wallet.
  pub async fn wallet_balance(&self) -> Result<CharacterWalletBalance, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v1/characters/{}/wallet/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns the wallet journal for this character (paginated).
  pub async fn wallet_journal(&self) -> Result<Vec<WalletJournalEntry>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self
          .esi
          .url_builder()
          .path(format!("v6/characters/{}/wallet/journal/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns the wallet transactions for this character (paginated).
  pub async fn wallet_transactions(&self) -> Result<Vec<WalletTransaction>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self
          .esi
          .url_builder()
          .path(format!("v1/characters/{}/wallet/transactions/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }
}
