//! Database entity for EVE Online character killmails.

use sea_orm::prelude::*;

/// A killmail record stored in the `character_killmails` table.
#[sea_orm::model]
#[derive(Clone, Debug, DeriveEntityModel, PartialEq)]
#[sea_orm(table_name = "character_killmails")]
pub struct Model {
  /// Number of attackers on the killmail.
  pub attacker_count: i32,
  /// ID of the owning character.
  pub character_id: i64,
  /// Whether the character landed the final blow.
  pub final_blow: bool,
  /// Auto-increment primary key.
  #[sea_orm(primary_key)]
  pub id: i32,
  /// Whether this is a kill (`true`) or loss (`false`) for the character.
  pub is_kill: bool,
  /// zKillboard hash used to construct the killmail URL.
  pub kill_hash: String,
  /// ISO-8601 timestamp when the kill occurred.
  pub kill_time: String,
  /// EVE killmail identifier.
  pub killmail_id: i32,
  /// Resolved name of the ship flown.
  pub ship_name: String,
  /// EVE type ID of the ship flown.
  pub ship_type_id: i32,
  /// ISO-8601 timestamp when this record was last synced.
  pub synced_at: String,
  /// Solar system ID where the kill occurred.
  pub system_id: i32,
  /// Resolved name of the solar system.
  pub system_name: String,
  /// Security status of the solar system.
  pub system_sec: f64,
  /// Estimated ISK value of the kill.
  pub value_isk: f64,
  /// Resolved corporation name of the victim.
  pub victim_corp_name: String,
  /// Resolved name of the victim.
  pub victim_name: String,
}

/// Default active-model behaviour with no custom hooks.
impl ActiveModelBehavior for ActiveModel {}
