//! Fleet ESI response models.

use serde::{Deserialize, Serialize};

/// Fleet information.
#[derive(Debug, Deserialize, Serialize)]
pub struct Fleet {
  pub is_free_move: bool,
  pub is_registered: bool,
  pub is_voice_enabled: bool,
  pub motd: String,
}

/// A fleet member.
#[derive(Debug, Deserialize, Serialize)]
pub struct FleetMember {
  pub character_id: i64,
  pub join_time: String,
  pub role: String,
  pub role_name: String,
  pub ship_type_id: i32,
  pub solar_system_id: i64,
  pub squad_id: i64,
  pub station_id: Option<i64>,
  pub takes_fleet_warp: bool,
  pub wing_id: i64,
}

/// Response when a new squad is created.
#[derive(Debug, Deserialize, Serialize)]
pub struct FleetSquadCreated {
  pub squad_id: i64,
}

/// A fleet wing.
#[derive(Debug, Deserialize, Serialize)]
pub struct FleetWing {
  pub id: i64,
  pub name: String,
  pub squads: Vec<FleetSquad>,
}

/// A fleet squad within a wing.
#[derive(Debug, Deserialize, Serialize)]
pub struct FleetSquad {
  pub id: i64,
  pub name: String,
}

/// Response when a new wing is created.
#[derive(Debug, Deserialize, Serialize)]
pub struct FleetWingCreated {
  pub wing_id: i64,
}
