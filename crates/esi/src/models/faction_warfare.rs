//! Faction warfare ESI response models.

use serde::{Deserialize, Serialize};

/// Faction warfare leaderboard for characters.
#[derive(Debug, Deserialize, Serialize)]
pub struct FwCharacterLeaderboard {
  pub kills: serde_json::Value,
  pub victory_points: serde_json::Value,
}

/// Faction warfare leaderboard for corporations.
#[derive(Debug, Deserialize, Serialize)]
pub struct FwCorporationLeaderboard {
  pub kills: serde_json::Value,
  pub victory_points: serde_json::Value,
}

/// Faction warfare leaderboard for factions.
#[derive(Debug, Deserialize, Serialize)]
pub struct FwLeaderboard {
  pub kills: serde_json::Value,
  pub victory_points: serde_json::Value,
}

/// Faction warfare statistics for a faction.
#[derive(Debug, Deserialize, Serialize)]
pub struct FwStats {
  pub faction_id: i64,
  pub kills: serde_json::Value,
  pub pilots: i32,
  pub systems_controlled: i32,
  pub victory_points: serde_json::Value,
}

/// A faction warfare solar system.
#[derive(Debug, Deserialize, Serialize)]
pub struct FwSystem {
  pub contested: String,
  pub occupier_faction_id: i64,
  pub owner_faction_id: i64,
  pub solar_system_id: i64,
  pub victory_points: i32,
  pub victory_points_threshold: i32,
}

/// An active faction warfare matchup.
#[derive(Debug, Deserialize, Serialize)]
pub struct FwWar {
  pub aggressor_faction_id: i64,
  pub defender_faction_id: i64,
}
