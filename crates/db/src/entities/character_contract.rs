//! Database entity for character contracts.

use pod_model::CharacterContract;
use sea_orm::prelude::*;
use validator::Validate;

/// A character contract stored in the `character_contracts` table.
#[sea_orm::model]
#[derive(Clone, Debug, DeriveEntityModel, PartialEq, Validate)]
#[sea_orm(table_name = "character_contracts")]
pub struct Model {
  /// Auto-increment primary key.
  #[sea_orm(primary_key)]
  pub id: i32,
  /// ID of the owning character.
  pub character_id: i64,
  /// ESI contract ID.
  pub contract_id: i64,
  /// Contract type (e.g. "item_exchange", "courier", "auction").
  #[validate(length(min = 1))]
  pub contract_type: String,
  /// Contract status (e.g. "outstanding", "finished", "expired").
  #[validate(length(min = 1))]
  pub status: String,
  /// Contract title (empty string when ESI returns null).
  pub title: String,
  /// ESI ID of the character who issued the contract.
  pub issuer_id: i64,
  /// ESI ID of the character or corp the contract is assigned to.
  pub assignee_id: i64,
  /// ESI ID of the character who accepted the contract.
  pub acceptor_id: i64,
  /// ISK price of the contract.
  pub price: Option<f64>,
  /// ISK collateral required.
  pub collateral: Option<f64>,
  /// ISO 8601 timestamp when the contract was issued.
  #[validate(length(min = 1))]
  pub date_issued: String,
  /// ISO 8601 timestamp when the contract expires.
  #[validate(length(min = 1))]
  pub date_expired: String,
  /// Station or structure ID where items are located.
  pub start_location_id: Option<i64>,
}

/// Default active-model behaviour with no custom hooks.
impl ActiveModelBehavior for ActiveModel {}

impl From<Model> for CharacterContract {
  fn from(e: Model) -> Self {
    Self {
      character_id: e.character_id,
      contract_id: e.contract_id,
      contract_type: e.contract_type,
      status: e.status,
      title: e.title,
      issuer_id: e.issuer_id,
      assignee_id: e.assignee_id,
      acceptor_id: e.acceptor_id,
      price: e.price,
      collateral: e.collateral,
      date_issued: e.date_issued,
      date_expired: e.date_expired,
      start_location_id: e.start_location_id,
    }
  }
}
