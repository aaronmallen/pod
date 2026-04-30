//! Database entity for stargates within EVE Online solar systems.

use pod_model::Stargate;
use sea_orm::{Set, prelude::*};

/// A stargate connecting two solar systems.
#[sea_orm::model]
#[derive(Clone, Debug, DeriveEntityModel, PartialEq)]
#[sea_orm(table_name = "stargates")]
pub struct Model {
  /// ID of the solar system this stargate leads to.
  pub destination_solar_system_id: i32,
  /// The stargate on the other end of this connection (self-referential).
  #[sea_orm(
    self_ref,
    relation_enum = "DestinationStargate",
    from = "destination_stargate_id",
    to = "id"
  )]
  pub destination_stargate: HasOne<Entity>,
  /// Foreign key referencing the paired destination stargate.
  pub destination_stargate_id: i32,
  /// Unique EVE Online stargate ID.
  #[sea_orm(primary_key, auto_increment = false)]
  pub id: i32,
  /// The item type that defines this stargate's visual/physical properties.
  #[sea_orm(belongs_to, from = "item_type_id", to = "id")]
  pub item_type: HasOne<super::item_type::Entity>,
  /// Foreign key referencing the stargate's item type.
  pub item_type_id: i32,
  /// Display name of the stargate.
  pub name: String,
  /// X coordinate of the stargate's position within its solar system (metres).
  pub position_x: f64,
  /// Y coordinate of the stargate's position within its solar system (metres).
  pub position_y: f64,
  /// Z coordinate of the stargate's position within its solar system (metres).
  pub position_z: f64,
  /// The solar system that contains this stargate.
  #[sea_orm(belongs_to, from = "solar_system_id", to = "id")]
  pub solar_system: HasOne<super::solar_system::Entity>,
  /// Foreign key referencing the containing solar system.
  pub solar_system_id: i32,
}

/// Default active-model behaviour with no custom hooks.
impl ActiveModelBehavior for ActiveModel {}

impl From<Model> for Stargate {
  fn from(entity: Model) -> Self {
    let mut model = Stargate::new(entity.id, entity.name);
    model
      .set_destination(entity.destination_stargate_id, entity.destination_solar_system_id)
      .set_item_type_id(entity.item_type_id)
      .set_position(entity.position_x, entity.position_y, entity.position_z)
      .set_solar_system_id(entity.solar_system_id)
      .mark_persisted();
    model
  }
}

impl From<ModelEx> for Stargate {
  fn from(entity: ModelEx) -> Self {
    let mut model = Stargate::new(entity.id, entity.name);
    model
      .set_destination(entity.destination_stargate_id, entity.destination_solar_system_id)
      .set_item_type_id(entity.item_type_id)
      .set_position(entity.position_x, entity.position_y, entity.position_z)
      .set_solar_system_id(entity.solar_system_id)
      .mark_persisted();
    model
  }
}

impl From<Stargate> for ActiveModel {
  fn from(model: Stargate) -> Self {
    Self {
      destination_solar_system_id: Set(*model.destination_solar_system_id()),
      destination_stargate_id: Set(*model.destination_stargate_id()),
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

impl From<Stargate> for ActiveModelEx {
  fn from(model: Stargate) -> Self {
    Self {
      destination_solar_system_id: Set(*model.destination_solar_system_id()),
      destination_stargate: Default::default(),
      destination_stargate_id: Set(*model.destination_stargate_id()),
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
