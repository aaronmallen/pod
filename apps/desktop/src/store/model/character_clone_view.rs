use sqlx::FromRow;

use crate::store::model::{CharacterClone, CharacterCloneImplant, CharacterJumpClone};

#[derive(FromRow)]
pub struct ActiveCloneRow {
  pub character_id: i64,
  pub home_location_id: i64,
  pub home_location_name: Option<String>,
  pub home_location_type: String,
  pub implant_icon: Option<String>,
  pub implant_name: Option<String>,
  pub implant_type_id: Option<i64>,
  pub last_clone_jump_date: Option<String>,
  pub last_station_change_date: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CharacterClones {
  pub active: CloneWithImplants<CharacterClone>,
  pub jump_clones: Vec<CloneWithImplants<CharacterJumpClone>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CloneWithImplants<C> {
  pub clone: C,
  pub implants: Vec<CharacterCloneImplant>,
}
