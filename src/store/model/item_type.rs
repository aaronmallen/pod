use getset::{CopyGetters, Getters};
use sqlx::FromRow;

use crate::clients::esi::models::universe::ItemType;

#[derive(Clone, CopyGetters, Debug, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub capacity: Option<f64>,
  #[getset(get = "pub")]
  pub description: Option<String>,
  #[getset(get = "pub")]
  pub dogma_attributes: String,
  #[getset(get_copy = "pub")]
  pub group_id: i64,
  #[getset(get_copy = "pub")]
  pub icon_id: Option<i64>,
  #[getset(get_copy = "pub")]
  pub id: i64,
  #[getset(get_copy = "pub")]
  pub market_group_id: Option<i64>,
  #[getset(get = "pub")]
  pub name: String,
  #[getset(get_copy = "pub")]
  pub packaged_volume: Option<f64>,
  #[getset(get_copy = "pub")]
  pub portion_size: Option<i32>,
  #[getset(get_copy = "pub")]
  pub published: bool,
  #[getset(get_copy = "pub")]
  pub radius: Option<f64>,
  #[getset(get_copy = "pub")]
  pub volume: Option<f64>,
}

impl From<ItemType> for Model {
  fn from(item_type: ItemType) -> Self {
    Self {
      capacity: item_type.capacity,
      description: Some(item_type.description),
      dogma_attributes: serde_json::to_string(&item_type.dogma_attributes).unwrap_or_else(|_| "[]".to_owned()),
      group_id: i64::from(item_type.group_id),
      icon_id: item_type.icon_id.map(i64::from),
      id: i64::from(item_type.type_id),
      market_group_id: item_type.market_group_id.map(i64::from),
      name: item_type.name,
      packaged_volume: item_type.packaged_volume,
      portion_size: item_type.portion_size,
      published: item_type.published,
      radius: item_type.radius,
      volume: item_type.volume,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod from {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::clients::esi::models::universe::DogmaAttribute;

    fn make_item_type() -> ItemType {
      ItemType {
        capacity: Some(0.0),
        description: "Tritanium is the basic building block.".to_owned(),
        dogma_attributes: vec![],
        graphic_id: None,
        group_id: 18,
        icon_id: None,
        market_group_id: None,
        mass: Some(0.0),
        name: "Tritanium".to_owned(),
        packaged_volume: None,
        portion_size: Some(1),
        published: true,
        radius: Some(1.0),
        type_id: 34,
        volume: Some(0.01),
      }
    }

    #[test]
    fn it_serializes_dogma_attributes_to_a_json_blob() {
      let mut item_type = make_item_type();
      item_type.dogma_attributes = vec![
        DogmaAttribute {
          attribute_id: 275,
          value: 1.0,
        },
        DogmaAttribute {
          attribute_id: 180,
          value: 167.0,
        },
      ];

      let model = Model::from(item_type);

      let parsed: serde_json::Value = serde_json::from_str(model.dogma_attributes()).unwrap();
      assert_eq!(parsed[0]["attribute_id"], 275);
      assert_eq!(parsed[0]["value"], 1.0);
      assert_eq!(parsed[1]["attribute_id"], 180);
    }

    #[test]
    fn it_widens_optional_market_group_id_when_present() {
      let mut item_type = make_item_type();
      item_type.market_group_id = Some(1857);
      item_type.icon_id = Some(22);

      let model = Model::from(item_type);

      assert_eq!(model.market_group_id(), Some(1857));
      assert_eq!(model.icon_id(), Some(22));
    }

    #[test]
    fn it_wraps_description_and_widens_ids() {
      let model = Model::from(make_item_type());

      assert_eq!(model.id(), 34);
      assert_eq!(model.group_id(), 18);
      assert_eq!(
        model.description().as_deref(),
        Some("Tritanium is the basic building block.")
      );
      assert_eq!(model.dogma_attributes(), "[]");
    }
  }
}
