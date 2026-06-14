use serde::Deserialize;

/// A blueprint owned by a character or corporation, as returned by ESI's
/// `/characters/{id}/blueprints/` and `/corporations/{id}/blueprints/` endpoints.
///
/// `runs == -1` denotes an original blueprint (BPO); any other value is the
/// remaining run count of a copy (BPC). The model carries the raw value
/// unchanged — interpretation lives in the sync layer.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct Blueprint {
  pub item_id: i64,
  pub type_id: i32,
  pub location_id: i64,
  pub location_flag: String,
  pub quantity: i32,
  pub material_efficiency: i32,
  pub time_efficiency: i32,
  pub runs: i32,
}

#[cfg(test)]
mod tests {
  use super::*;

  mod blueprint {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_deserializes_a_bpo_and_a_bpc_from_a_representative_esi_payload() {
      let body = r#"[
        {
          "item_id": 1000000000001,
          "type_id": 962,
          "location_id": 60003760,
          "location_flag": "Hangar",
          "quantity": -1,
          "material_efficiency": 10,
          "time_efficiency": 20,
          "runs": -1
        },
        {
          "item_id": 1000000000002,
          "type_id": 963,
          "location_id": 60003760,
          "location_flag": "Hangar",
          "quantity": 1,
          "material_efficiency": 2,
          "time_efficiency": 4,
          "runs": 300
        }
      ]"#;

      let blueprints: Vec<Blueprint> = serde_json::from_str(body).unwrap();

      assert_eq!(blueprints.len(), 2);

      let bpo = &blueprints[0];
      assert_eq!(bpo.item_id, 1000000000001);
      assert_eq!(bpo.type_id, 962);
      assert_eq!(bpo.location_id, 60003760);
      assert_eq!(bpo.location_flag, "Hangar");
      assert_eq!(bpo.quantity, -1);
      assert_eq!(bpo.material_efficiency, 10);
      assert_eq!(bpo.time_efficiency, 20);
      assert_eq!(bpo.runs, -1);

      let bpc = &blueprints[1];
      assert_eq!(bpc.item_id, 1000000000002);
      assert_eq!(bpc.type_id, 963);
      assert_eq!(bpc.quantity, 1);
      assert_eq!(bpc.material_efficiency, 2);
      assert_eq!(bpc.time_efficiency, 4);
      assert_eq!(bpc.runs, 300);
    }
  }
}
