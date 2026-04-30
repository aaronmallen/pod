//! Industry ESI response models.

use serde::{Deserialize, Serialize};

/// An industry facility.
#[derive(Debug, Deserialize, Serialize)]
pub struct IndustryFacility {
  pub facility_id: i64,
  pub owner_id: i64,
  pub region_id: i64,
  pub solar_system_id: i64,
  pub solar_system_security: f64,
  pub tax: Option<f64>,
  pub type_id: i32,
}

/// Industry cost indices for a solar system.
#[derive(Debug, Deserialize, Serialize)]
pub struct IndustrySolarSystem {
  pub cost_indices: Vec<CostIndex>,
  pub solar_system_id: i64,
}

/// A cost index entry.
#[derive(Debug, Deserialize, Serialize)]
pub struct CostIndex {
  pub activity: String,
  pub cost_index: f64,
}
