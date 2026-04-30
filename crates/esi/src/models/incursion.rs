//! Incursion ESI response models.

use serde::{Deserialize, Serialize};

/// An active EVE incursion.
#[derive(Debug, Deserialize, Serialize)]
pub struct Incursion {
  pub constellation_id: i64,
  pub faction_id: i64,
  pub has_boss: bool,
  pub infested_solar_systems: Vec<i64>,
  pub influence: f64,
  pub staging_solar_system_id: i64,
  pub state: String,
  pub r#type: String,
}
