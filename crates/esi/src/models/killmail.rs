//! Killmail ESI response models.

use serde::{Deserialize, Serialize};

/// A killmail record.
#[derive(Debug, Deserialize, Serialize)]
pub struct Killmail {
  pub attackers: Vec<serde_json::Value>,
  pub killmail_id: i64,
  pub killmail_time: String,
  pub moon_id: Option<i64>,
  pub solar_system_id: i64,
  pub victim: serde_json::Value,
  pub war_id: Option<i64>,
}
