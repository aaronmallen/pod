use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Attacker {
  #[allow(dead_code)]
  #[serde(default)]
  pub alliance_id: Option<i64>,
  #[serde(default)]
  pub character_id: Option<i64>,
  #[allow(dead_code)]
  #[serde(default)]
  pub corporation_id: Option<i64>,
  /// ESI marks this required, but `#[serde(default)]` lets partial mock/test payloads deserialize.
  #[allow(dead_code)]
  #[serde(default)]
  pub damage_done: i64,
  #[serde(default)]
  pub final_blow: bool,
  /// Optional because NPC and structure attackers omit alliance/corporation/ship ids.
  #[allow(dead_code)]
  #[serde(default)]
  pub ship_type_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct Item {
  #[allow(dead_code)]
  pub flag: i64,
  /// Nested cargo contents; accepted so the recursive shape deserializes cleanly, but never used for valuation.
  #[allow(dead_code)]
  #[serde(default)]
  pub items: Vec<Item>,
  #[serde(default)]
  pub quantity_destroyed: Option<i64>,
  #[serde(default)]
  pub quantity_dropped: Option<i64>,
  /// ESI keys this field `item_type_id`, not `type_id`; the rename is required or every
  /// fitted-ship killmail silently fails to deserialize.
  #[serde(rename = "item_type_id")]
  pub type_id: i64,
}

#[derive(Debug, Deserialize)]
pub struct Killmail {
  #[serde(default)]
  pub attackers: Vec<Attacker>,
  pub killmail_id: i64,
  pub killmail_time: String,
  pub solar_system_id: i64,
  pub victim: Victim,
}

#[derive(Debug, Deserialize)]
pub struct Victim {
  #[allow(dead_code)]
  #[serde(default)]
  pub alliance_id: Option<i64>,
  #[serde(default)]
  pub character_id: Option<i64>,
  #[serde(default)]
  pub corporation_id: Option<i64>,
  /// ESI marks this required, but `#[serde(default)]` lets partial mock/test payloads deserialize.
  #[allow(dead_code)]
  #[serde(default)]
  pub damage_taken: i64,
  #[serde(default)]
  pub items: Vec<Item>,
  pub ship_type_id: i64,
}

#[cfg(test)]
mod tests {
  use super::*;

  mod killmail {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_deserializes_full_detail_for_npc_and_player_attackers_and_the_victim() {
      let body = r#"{
        "killmail_id": 136076545,
        "killmail_time": "2026-06-14T18:30:00Z",
        "solar_system_id": 30002187,
        "victim": {
          "alliance_id": 99005338,
          "character_id": 2002,
          "corporation_id": 3003,
          "damage_taken": 7821,
          "ship_type_id": 29986
        },
        "attackers": [
          {
            "damage_done": 4500,
            "final_blow": true,
            "security_status": -1.2,
            "alliance_id": 99003581,
            "character_id": 9001,
            "corporation_id": 8001,
            "ship_type_id": 17738
          },
          {
            "damage_done": 3321,
            "final_blow": false,
            "security_status": 0.0
          }
        ]
      }"#;

      let killmail: Killmail = serde_json::from_str(body).unwrap();

      assert_eq!(killmail.victim.damage_taken, 7821);
      assert_eq!(killmail.victim.alliance_id, Some(99005338));
      assert_eq!(killmail.victim.corporation_id, Some(3003));

      let player = &killmail.attackers[0];
      assert_eq!(player.damage_done, 4500);
      assert!(player.final_blow);
      assert_eq!(player.alliance_id, Some(99003581));
      assert_eq!(player.character_id, Some(9001));
      assert_eq!(player.corporation_id, Some(8001));
      assert_eq!(player.ship_type_id, Some(17738));

      let npc = &killmail.attackers[1];
      assert_eq!(npc.damage_done, 3321);
      assert!(!npc.final_blow);
      assert_eq!(npc.alliance_id, None);
      assert_eq!(npc.character_id, None);
      assert_eq!(npc.corporation_id, None);
      assert_eq!(npc.ship_type_id, None);
    }
  }

  mod victim {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_deserializes_a_realistic_esi_item_array_keyed_by_item_type_id() {
      let body = r#"{
        "character_id": 2002,
        "corporation_id": 3003,
        "ship_type_id": 29986,
        "damage_taken": 7821,
        "items": [
          {"flag": 5, "item_type_id": 34, "quantity_destroyed": 3},
          {"flag": 27, "item_type_id": 2488, "quantity_dropped": 1},
          {"flag": 87, "item_type_id": 12058},
          {"flag": 5, "item_type_id": 3467, "quantity_dropped": 1, "items": [
            {"flag": 0, "item_type_id": 34, "quantity_dropped": 100},
            {"flag": 0, "item_type_id": 35, "quantity_destroyed": 50}
          ]}
        ]
      }"#;

      let victim: Victim = serde_json::from_str(body).unwrap();

      assert_eq!(victim.damage_taken, 7821);
      assert_eq!(victim.alliance_id, None);
      assert_eq!(victim.items.len(), 4);
      assert_eq!(victim.items[0].type_id, 34);
      assert_eq!(victim.items[0].quantity_destroyed, Some(3));
      assert_eq!(victim.items[0].quantity_dropped, None);
      assert_eq!(victim.items[1].type_id, 2488);
      assert_eq!(victim.items[1].quantity_destroyed, None);
      assert_eq!(victim.items[1].quantity_dropped, Some(1));
      assert_eq!(victim.items[2].type_id, 12058);
      assert_eq!(victim.items[3].type_id, 3467);
      assert_eq!(victim.items[3].items.len(), 2);
      assert_eq!(victim.items[3].items[0].type_id, 34);
    }

    #[test]
    fn it_defaults_items_to_an_empty_list_when_absent() {
      let body = r#"{"ship_type_id": 35832, "damage_taken": 142}"#;

      let victim: Victim = serde_json::from_str(body).unwrap();

      assert!(victim.items.is_empty());
    }
  }
}
