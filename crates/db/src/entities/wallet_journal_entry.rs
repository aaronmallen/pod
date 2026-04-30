//! Database entity for wallet journal entries.

use pod_model::WalletJournalEntry;
use sea_orm::prelude::*;
use validator::Validate;

/// A wallet journal entry stored in the `wallet_journal_entries` table.
#[sea_orm::model]
#[derive(Clone, Debug, DeriveEntityModel, PartialEq, Validate)]
#[sea_orm(table_name = "wallet_journal_entries")]
pub struct Model {
  /// Auto-increment primary key.
  #[sea_orm(primary_key)]
  pub id: i32,
  /// ID of the owning character.
  pub character_id: i64,
  /// ESI journal entry ID.
  pub entry_id: i64,
  /// Journal ref type (e.g. "market_transaction", "bounty").
  #[validate(length(min = 1))]
  pub ref_type: String,
  /// ISK amount (positive = credit, negative = debit).
  pub amount: Option<f64>,
  /// Running wallet balance after this entry.
  pub balance: Option<f64>,
  /// ISO 8601 timestamp.
  #[validate(length(min = 1))]
  pub date: String,
  /// Human-readable description.
  pub description: String,
  /// ESI ID of the first party (e.g. buyer, payer).
  pub first_party_id: Option<i64>,
  /// ESI ID of the second party (e.g. seller, payee).
  pub second_party_id: Option<i64>,
}

/// Default active-model behaviour with no custom hooks.
impl ActiveModelBehavior for ActiveModel {}

impl From<Model> for WalletJournalEntry {
  fn from(e: Model) -> Self {
    Self {
      character_id: e.character_id,
      entry_id: e.entry_id,
      ref_type: e.ref_type,
      amount: e.amount,
      balance: e.balance,
      date: e.date,
      description: e.description,
      first_party_id: e.first_party_id,
      second_party_id: e.second_party_id,
    }
  }
}
