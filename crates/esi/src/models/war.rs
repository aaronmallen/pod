//! War ESI response models.

use serde::{Deserialize, Serialize};

/// A war between entities.
#[derive(Debug, Deserialize, Serialize)]
pub struct War {
  pub aggressor: WarSide,
  pub allies: Option<Vec<WarAlly>>,
  pub declared: String,
  pub defender: WarSide,
  pub finished: Option<String>,
  pub id: i32,
  pub mutual: bool,
  pub open_for_allies: bool,
  pub retracted: Option<String>,
  pub started: Option<String>,
}

/// One side of a war.
#[derive(Debug, Deserialize, Serialize)]
pub struct WarSide {
  pub alliance_id: Option<i64>,
  pub corporations_taken: Option<i32>,
  pub isk_destroyed: f64,
  pub ships_killed: i32,
}

/// An ally in a war.
#[derive(Debug, Deserialize, Serialize)]
pub struct WarAlly {
  pub alliance_id: Option<i64>,
  pub corporation_id: Option<i64>,
}

/// A killmail associated with a war.
#[derive(Debug, Deserialize, Serialize)]
pub struct WarKillmail {
  pub killmail_hash: String,
  pub killmail_id: i64,
}
