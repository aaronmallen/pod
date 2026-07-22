use getset::{CopyGetters, Getters};
use sqlx::FromRow;

#[derive(Clone, CopyGetters, Debug, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub attacker_count: i64,
  #[getset(get_copy = "pub")]
  pub character_id: i64,
  #[getset(get_copy = "pub")]
  pub final_blow: bool,
  #[getset(get_copy = "pub")]
  pub is_kill: bool,
  #[getset(get = "pub")]
  pub kill_hash: String,
  #[getset(get = "pub")]
  pub kill_time: String,
  #[getset(get_copy = "pub")]
  pub killmail_id: i64,
  #[getset(get_copy = "pub")]
  pub ship_type_id: i64,
  #[getset(get = "pub")]
  pub synced_at: String,
  #[getset(get_copy = "pub")]
  pub system_id: i64,
  /// Destroyed-only ISK loss; `value_isk` covers destroyed + dropped (the zKill display total).
  #[getset(get_copy = "pub")]
  pub value_destroyed_isk: f64,
  #[getset(get_copy = "pub")]
  pub value_final: bool,
  #[getset(get_copy = "pub")]
  pub value_isk: f64,
  #[getset(get_copy = "pub")]
  pub value_recheck_count: i64,
  #[getset(get = "pub")]
  pub value_source: String,
  #[getset(get_copy = "pub")]
  pub victim_alliance_id: Option<i64>,
  #[getset(get_copy = "pub")]
  pub victim_corp_id: Option<i64>,
  #[getset(get_copy = "pub")]
  pub victim_damage_taken: i64,
  #[getset(get_copy = "pub")]
  pub victim_id: Option<i64>,
}
