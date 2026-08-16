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
  /// Where the capsuleer is right now, which is where the active clone is. `None` when location
  /// tracking is off or the telemetry job has not run, in which case Pod says so rather than
  /// falling back to the home station (which is a different place).
  pub current_location: Option<CloneLocation>,
  pub jump_clones: Vec<CloneWithImplants<CharacterJumpClone>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CloneLocation {
  pub id: i64,
  pub name: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CloneWithImplants<C> {
  pub clone: C,
  pub implants: Vec<CharacterCloneImplant>,
}
