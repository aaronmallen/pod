use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Attacker {
  #[serde(default)]
  pub character_id: Option<i64>,
  #[serde(default)]
  pub final_blow: bool,
}

#[derive(Debug, Deserialize)]
pub struct Item {
  #[allow(dead_code)]
  pub flag: i64,
  #[serde(default)]
  pub quantity_destroyed: Option<i64>,
  #[serde(default)]
  pub quantity_dropped: Option<i64>,
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
  #[serde(default)]
  pub character_id: Option<i64>,
  #[serde(default)]
  pub corporation_id: Option<i64>,
  #[serde(default)]
  pub items: Vec<Item>,
  pub ship_type_id: i64,
}

#[cfg(test)]
mod tests {
  use super::*;

  mod victim {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_deserializes_items_with_destroyed_and_dropped_quantities() {
      let body = r#"{
        "character_id": 2002,
        "corporation_id": 3003,
        "ship_type_id": 587,
        "items": [
          {"flag": 5, "item_type_id": 0, "quantity_destroyed": 3, "type_id": 34},
          {"flag": 27, "quantity_dropped": 1, "type_id": 2488},
          {"flag": 87, "type_id": 12058}
        ]
      }"#;

      let victim: Victim = serde_json::from_str(body).unwrap();

      assert_eq!(victim.items.len(), 3);
      assert_eq!(victim.items[0].flag, 5);
      assert_eq!(victim.items[0].type_id, 34);
      assert_eq!(victim.items[0].quantity_destroyed, Some(3));
      assert_eq!(victim.items[0].quantity_dropped, None);
      assert_eq!(victim.items[1].quantity_destroyed, None);
      assert_eq!(victim.items[1].quantity_dropped, Some(1));
      assert_eq!(victim.items[2].quantity_destroyed, None);
      assert_eq!(victim.items[2].quantity_dropped, None);
    }

    #[test]
    fn it_defaults_items_to_an_empty_list_when_absent() {
      let body = r#"{"ship_type_id": 35832}"#;

      let victim: Victim = serde_json::from_str(body).unwrap();

      assert!(victim.items.is_empty());
    }
  }
}
