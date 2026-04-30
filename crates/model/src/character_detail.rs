//! Domain models for the character detail view.

use serde::{Deserialize, Serialize};

/// A single implant installed in a clone.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CharacterImplant {
  /// Resolved display name.
  pub name: String,
  /// Implant slot number (1–10).
  pub slot: usize,
  /// EVE type ID used for icon lookups.
  pub type_id: i32,
}

/// A character clone (either the active implant set or a jump clone).
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CharacterClone {
  /// The clone identifier (0 for the active clone).
  pub clone_id: i64,
  /// Installed implants with slot, name, and cached icon.
  pub implants: Vec<CharacterImplant>,
  /// Whether this is the character's active clone.
  pub is_active: bool,
  /// ISO-8601 timestamp of the last clone jump (active clone only).
  pub jump_ready_at: Option<String>,
  /// Optional user-assigned name for jump clones.
  pub name: Option<String>,
  /// Resolved station name.
  pub station_name: String,
}

/// A single entry in a character's contact list.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CharacterContact {
  /// Unique EVE entity ID for this contact.
  pub contact_id: i64,
  /// EVE entity type (character, corporation, alliance, faction).
  pub contact_type: String,
  /// Whether the contact is blocked.
  pub is_blocked: bool,
  /// Whether the contact is watched.
  pub is_watched: bool,
  /// Names of labels applied to this contact.
  pub label_names: Vec<String>,
  /// Display name of the contact.
  pub name: String,
  /// Standing value toward this contact.
  pub standing: f64,
}

/// A label used to categorize contacts.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CharacterContactLabel {
  /// Unique label identifier.
  pub label_id: i64,
  /// Display name of the label.
  pub name: String,
}

/// A kill or loss record for a character.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CharacterKillEntry {
  /// Number of attackers involved.
  pub attacker_count: u32,
  /// `true` if the character landed the final blow.
  pub final_blow: bool,
  /// `true` if the character landed the killing blow (kill), `false` if they were the victim (loss).
  pub is_kill: bool,
  /// EVE killmail identifier.
  pub killmail_id: i64,
  /// Resolved name of the ship flown.
  pub ship_name: String,
  /// EVE type ID of the ship (used for icon lookups).
  pub ship_type_id: i32,
  /// Security status of the solar system where the kill occurred.
  pub solar_system_security: f64,
  /// Total ISK value of the killmail (from zKillboard).
  pub total_value: f64,
  /// Resolved name of the solar system where the kill occurred.
  pub solar_system_name: String,
  /// ISO-8601 timestamp of the kill.
  pub timestamp: String,
  /// Resolved name of the victim's corporation.
  pub victim_corp: String,
  /// Resolved name of the victim character.
  pub victim_name: String,
}

/// A notification received by a character.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CharacterNotification {
  /// High-level category derived from the notification type (e.g., "war", "structure").
  pub category: String,
  /// Whether the character has read this notification.
  pub is_read: bool,
  /// Unique notification identifier.
  pub notification_id: i64,
  /// Sender entity ID.
  pub sender_id: i64,
  /// Sender entity type.
  pub sender_type: String,
  /// Optional notification body text.
  pub text: Option<String>,
  /// ISO-8601 timestamp when the notification was sent.
  pub timestamp: String,
  /// Raw EVE notification type string.
  pub type_: String,
}

/// A standing entry for a character toward an NPC or player entity.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CharacterStanding {
  /// Unique identifier of the entity toward which the standing applies.
  pub from_id: i64,
  /// Resolved display name of the entity.
  pub from_name: String,
  /// EVE entity type (agent, npc_corp, faction, character, corporation, alliance).
  pub from_type: String,
  /// Standing value in the range [-10.0, 10.0].
  pub standing: f64,
}
