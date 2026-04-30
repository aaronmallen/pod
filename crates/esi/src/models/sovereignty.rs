//! Sovereignty ESI response models.

use serde::{Deserialize, Serialize};

/// An active sovereignty campaign.
#[derive(Debug, Deserialize, Serialize)]
pub struct SovereigntyCampaign {
  pub attackers_score: Option<f64>,
  pub campaign_id: i64,
  pub constellation_id: i64,
  pub defender_id: Option<i64>,
  pub defender_score: Option<f64>,
  pub event_type: String,
  pub participants: Option<Vec<serde_json::Value>>,
  pub solar_system_id: i64,
  pub start_time: String,
  pub structure_id: i64,
}

/// Sovereignty data for a solar system.
#[derive(Debug, Deserialize, Serialize)]
pub struct SovereigntyMap {
  pub alliance_id: Option<i64>,
  pub corporation_id: Option<i64>,
  pub faction_id: Option<i64>,
  pub system_id: i64,
}

/// A sovereignty structure.
#[derive(Debug, Deserialize, Serialize)]
pub struct SovereigntyStructure {
  pub alliance_id: i64,
  pub solar_system_id: i64,
  pub structure_id: i64,
  pub structure_type_id: i32,
  pub vulnerability_occupancy_level: Option<f64>,
  pub vulnerable_end_time: Option<String>,
  pub vulnerable_start_time: Option<String>,
}
