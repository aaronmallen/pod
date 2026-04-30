//! Server status ESI response models.

use serde::{Deserialize, Serialize};

/// Current EVE Tranquility server status.
#[derive(Debug, Deserialize, Serialize)]
pub struct ServerStatus {
  pub players: i32,
  pub server_version: String,
  pub start_time: String,
  pub vip: Option<bool>,
}
