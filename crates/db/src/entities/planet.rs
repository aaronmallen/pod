//! Database entity for planets within EVE Online solar systems.

use pod_model::Planet;
use sea_orm::{Set, prelude::*};

/// A planet orbiting within a solar system.
#[sea_orm::model]
#[derive(Clone, Debug, DeriveEntityModel, PartialEq)]
#[sea_orm(table_name = "planets")]
pub struct Model {
  /// Unique planet identifier (EVE planet ID).
  #[sea_orm(primary_key, auto_increment = false)]
  pub id: i32,
  /// Associated item type record.
  #[sea_orm(belongs_to, from = "item_type_id", to = "id")]
  pub item_type: HasOne<super::item_type::Entity>,
  /// Foreign key referencing the planet's item type.
  pub item_type_id: i32,
  /// Display name of the planet.
  pub name: String,
  /// X coordinate of the planet's position in the solar system (metres).
  pub position_x: f64,
  /// Y coordinate of the planet's position in the solar system (metres).
  pub position_y: f64,
  /// Z coordinate of the planet's position in the solar system (metres).
  pub position_z: f64,
  /// Solar system that contains this planet.
  #[sea_orm(belongs_to, from = "solar_system_id", to = "id")]
  pub solar_system: HasOne<super::solar_system::Entity>,
  /// Foreign key referencing the containing solar system.
  pub solar_system_id: i32,
}

/// Default active-model behaviour with no custom hooks.
impl ActiveModelBehavior for ActiveModel {}

impl From<Model> for Planet {
  fn from(entity: Model) -> Self {
    let mut model = Planet::new(entity.id, entity.name);
    model
      .set_item_type_id(entity.item_type_id)
      .set_position(entity.position_x, entity.position_y, entity.position_z)
      .set_solar_system_id(entity.solar_system_id)
      .mark_persisted();
    model
  }
}

impl From<ModelEx> for Planet {
  fn from(entity: ModelEx) -> Self {
    let mut model = Planet::new(entity.id, entity.name);
    model
      .set_item_type_id(entity.item_type_id)
      .set_position(entity.position_x, entity.position_y, entity.position_z)
      .set_solar_system_id(entity.solar_system_id)
      .mark_persisted();
    model
  }
}

impl From<Planet> for ActiveModel {
  fn from(model: Planet) -> Self {
    Self {
      id: Set(*model.id()),
      item_type_id: Set(*model.item_type_id()),
      name: Set(model.name().clone()),
      position_x: Set(*model.position_x()),
      position_y: Set(*model.position_y()),
      position_z: Set(*model.position_z()),
      solar_system_id: Set(*model.solar_system_id()),
    }
  }
}

impl From<Planet> for ActiveModelEx {
  fn from(model: Planet) -> Self {
    Self {
      id: Set(*model.id()),
      item_type: Default::default(),
      item_type_id: Set(*model.item_type_id()),
      name: Set(model.name().clone()),
      position_x: Set(*model.position_x()),
      position_y: Set(*model.position_y()),
      position_z: Set(*model.position_z()),
      solar_system: Default::default(),
      solar_system_id: Set(*model.solar_system_id()),
    }
  }
}
