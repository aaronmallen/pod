//! Database entity for EVE Online character contacts.

use sea_orm::prelude::*;

/// A contact record stored in the `character_contacts` table.
#[sea_orm::model]
#[derive(Clone, Debug, DeriveEntityModel, PartialEq)]
#[sea_orm(table_name = "character_contacts")]
pub struct Model {
  /// ID of the owning character.
  pub character_id: i64,
  /// EVE entity ID for this contact.
  pub contact_id: i32,
  /// Resolved display name of the contact.
  pub contact_name: String,
  /// Entity type: character, corp, alliance, or faction.
  pub contact_type: String,
  /// Auto-increment primary key.
  #[sea_orm(primary_key)]
  pub id: i32,
  /// Whether the contact appears on the watch list.
  pub is_watchlist: bool,
  /// JSON-encoded array of label IDs applied to this contact.
  pub label_ids: String,
  /// Standing value toward this contact.
  pub standing: f64,
  /// ISO-8601 timestamp when this record was last synced from ESI.
  pub synced_at: String,
}

/// Default active-model behaviour with no custom hooks.
impl ActiveModelBehavior for ActiveModel {}
