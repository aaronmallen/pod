use getset::CopyGetters;
use sqlx::FromRow;

#[derive(Clone, CopyGetters, Debug, FromRow, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub alliance_id: Option<i64>,
  #[getset(get_copy = "pub")]
  pub attacker_character_id: Option<i64>,
  #[getset(get_copy = "pub")]
  pub character_id: i64,
  #[getset(get_copy = "pub")]
  pub corporation_id: Option<i64>,
  #[getset(get_copy = "pub")]
  pub damage_done: i64,
  #[getset(get_copy = "pub")]
  pub final_blow: bool,
  #[getset(get_copy = "pub")]
  pub killmail_id: i64,
  #[getset(get_copy = "pub")]
  pub ordinal: i64,
  #[getset(get_copy = "pub")]
  pub ship_type_id: Option<i64>,
}
