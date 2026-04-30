//! Alliance ESI response models.

use serde::{Deserialize, Serialize};

/// Contact in an alliance contact list.
#[derive(Debug, Deserialize, Serialize)]
pub struct AllianceContact {
  pub contact_id: i64,
  pub contact_type: String,
  pub standing: f64,
}

/// Label for categorizing alliance contacts.
#[derive(Debug, Deserialize, Serialize)]
pub struct AllianceContactLabel {
  pub label_id: i64,
  pub label_name: String,
}

/// Public information about an alliance.
#[derive(Debug, Deserialize, Serialize)]
pub struct AllianceDetail {
  pub creator_corporation_id: i64,
  pub creator_id: i64,
  pub date_founded: String,
  pub executor_corporation_id: Option<i64>,
  pub name: String,
  pub ticker: String,
}

/// Icon URLs for an alliance.
#[derive(Debug, Deserialize, Serialize)]
pub struct AllianceIcons {
  pub px128x128: Option<String>,
  pub px64x64: Option<String>,
}
