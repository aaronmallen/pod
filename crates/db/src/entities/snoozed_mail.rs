//! Database entity for snoozed mail deadlines.

use sea_orm::prelude::*;

/// A snoozed mail deadline stored in the `snoozed_mails` table.
#[sea_orm::model]
#[derive(Clone, Debug, DeriveEntityModel, PartialEq)]
#[sea_orm(table_name = "snoozed_mails")]
pub struct Model {
  /// Auto-increment primary key.
  #[sea_orm(primary_key)]
  pub id: i32,
  /// ID of the owning character.
  pub character_id: i64,
  /// ESI mail ID.
  pub mail_id: i64,
  /// ISO 8601 UTC timestamp when the snooze expires.
  pub snooze_until: String,
}

/// Default active-model behaviour with no custom hooks.
impl ActiveModelBehavior for ActiveModel {}
