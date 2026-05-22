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

#[cfg(test)]
mod tests {
  mod character_clone {
    use pretty_assertions::assert_eq;

    use super::super::*;

    #[test]
    fn it_round_trips_through_json() {
      let clone = CharacterClone {
        clone_id: 0,
        implants: vec![CharacterImplant {
          name: "Memory Augmentation - Basic".into(),
          slot: 4,
          type_id: 9942,
        }],
        is_active: true,
        jump_ready_at: None,
        name: None,
        station_name: "Jita IV - Moon 4".into(),
      };

      let json = serde_json::to_string(&clone).unwrap();
      let decoded: CharacterClone = serde_json::from_str(&json).unwrap();

      assert_eq!(decoded.clone_id, 0);
      assert_eq!(decoded.is_active, true);
      assert_eq!(decoded.implants.len(), 1);
      assert_eq!(decoded.implants[0].slot, 4);
    }
  }

  mod character_contact {
    use pretty_assertions::assert_eq;

    use super::super::*;

    #[test]
    fn it_round_trips_through_json() {
      let contact = CharacterContact {
        contact_id: 90_000_001,
        contact_type: "character".into(),
        is_blocked: false,
        is_watched: true,
        label_names: vec!["Corp Mate".into()],
        name: "Test Pilot".into(),
        standing: 5.0,
      };

      let json = serde_json::to_string(&contact).unwrap();
      let decoded: CharacterContact = serde_json::from_str(&json).unwrap();

      assert_eq!(decoded.contact_id, 90_000_001);
      assert_eq!(decoded.standing, 5.0);
      assert_eq!(decoded.label_names, vec!["Corp Mate"]);
    }
  }

  mod character_contact_label {
    use pretty_assertions::assert_eq;

    use super::super::*;

    #[test]
    fn it_round_trips_through_json() {
      let label = CharacterContactLabel {
        label_id: 1,
        name: "Corp Mate".into(),
      };

      let json = serde_json::to_string(&label).unwrap();
      let decoded: CharacterContactLabel = serde_json::from_str(&json).unwrap();

      assert_eq!(decoded.label_id, 1);
      assert_eq!(decoded.name, "Corp Mate");
    }
  }

  mod character_kill_entry {
    use pretty_assertions::assert_eq;

    use super::super::*;

    #[test]
    fn it_round_trips_through_json() {
      let entry = CharacterKillEntry {
        attacker_count: 3,
        final_blow: true,
        is_kill: true,
        killmail_id: 100_000_001,
        ship_name: "Rifter".into(),
        ship_type_id: 587,
        solar_system_security: 0.5,
        total_value: 6_000_000.0,
        solar_system_name: "Jita".into(),
        timestamp: "2024-06-01T12:00:00Z".into(),
        victim_corp: "Test Corp".into(),
        victim_name: "Target Pilot".into(),
      };

      let json = serde_json::to_string(&entry).unwrap();
      let decoded: CharacterKillEntry = serde_json::from_str(&json).unwrap();

      assert_eq!(decoded.killmail_id, 100_000_001);
      assert_eq!(decoded.is_kill, true);
      assert_eq!(decoded.total_value, 6_000_000.0);
    }
  }

  mod character_notification {
    use pretty_assertions::assert_eq;
    use serde_json::json;

    use super::super::*;

    #[test]
    fn it_round_trips_through_json() {
      let notif = CharacterNotification {
        category: "war".into(),
        is_read: false,
        notification_id: 1_000_001,
        sender_id: 500_001,
        sender_type: "corporation".into(),
        text: Some("War declared.".into()),
        timestamp: "2024-06-01T00:00:00Z".into(),
        type_: "WarDeclared".into(),
      };

      let json = serde_json::to_string(&notif).unwrap();
      let decoded: CharacterNotification = serde_json::from_str(&json).unwrap();

      assert_eq!(decoded.notification_id, 1_000_001);
      assert_eq!(decoded.category, "war");
      assert_eq!(decoded.text, Some("War declared.".into()));
    }

    #[test]
    fn it_deserializes_from_json_fixture() {
      let fixture = json!({
        "category": "structure",
        "is_read": true,
        "notification_id": 999,
        "sender_id": 1000,
        "sender_type": "character",
        "text": null,
        "timestamp": "2024-01-01T00:00:00Z",
        "type_": "StructureUnderAttack"
      });

      let notif: CharacterNotification = serde_json::from_value(fixture).unwrap();

      assert_eq!(notif.category, "structure");
      assert!(notif.is_read);
      assert!(notif.text.is_none());
    }
  }

  mod character_standing {
    use pretty_assertions::assert_eq;

    use super::super::*;

    #[test]
    fn it_round_trips_through_json() {
      let standing = CharacterStanding {
        from_id: 500_001,
        from_name: "Caldari State".into(),
        from_type: "faction".into(),
        standing: 2.5,
      };

      let json = serde_json::to_string(&standing).unwrap();
      let decoded: CharacterStanding = serde_json::from_str(&json).unwrap();

      assert_eq!(decoded.from_id, 500_001);
      assert_eq!(decoded.standing, 2.5);
      assert_eq!(decoded.from_type, "faction");
    }
  }
}
