//! Database entity for EVE Online constellations.

use pod_model::Constellation;
use sea_orm::{Set, prelude::*};

/// A constellation — a named grouping of solar systems within a region.
#[sea_orm::model]
#[derive(Clone, Debug, DeriveEntityModel, PartialEq)]
#[sea_orm(table_name = "constellations")]
pub struct Model {
  /// Unique EVE constellation ID.
  #[sea_orm(primary_key, auto_increment = false)]
  pub id: i32,
  /// Human-readable constellation name.
  pub name: String,
  /// X coordinate of the constellation's reference position (metres).
  pub position_x: f64,
  /// Y coordinate of the constellation's reference position (metres).
  pub position_y: f64,
  /// Z coordinate of the constellation's reference position (metres).
  pub position_z: f64,
  /// The region this constellation belongs to.
  #[sea_orm(belongs_to, from = "region_id", to = "id")]
  pub region: HasOne<super::region::Entity>,
  /// Foreign key referencing the parent region.
  pub region_id: i32,
  /// Solar systems contained within this constellation.
  #[sea_orm(has_many)]
  pub solar_systems: HasMany<super::solar_system::Entity>,
}

/// Default active-model behaviour with no custom hooks.
impl ActiveModelBehavior for ActiveModel {}

impl From<Model> for Constellation {
  fn from(entity: Model) -> Self {
    let mut model = Constellation::new(entity.id, entity.name);
    model
      .set_position(entity.position_x, entity.position_y, entity.position_z)
      .set_region_id(entity.region_id)
      .mark_persisted();
    model
  }
}

impl From<ModelEx> for Constellation {
  fn from(entity: ModelEx) -> Self {
    let mut model = Constellation::new(entity.id, entity.name);
    *model.solar_systems_mut() = entity.solar_systems.into_iter().map(Into::into).collect();
    model
      .set_position(entity.position_x, entity.position_y, entity.position_z)
      .set_region_id(entity.region_id)
      .mark_persisted();
    model
  }
}

impl From<Constellation> for ActiveModel {
  fn from(model: Constellation) -> Self {
    Self {
      id: Set(*model.id()),
      name: Set(model.name().clone()),
      position_x: Set(*model.position_x()),
      position_y: Set(*model.position_y()),
      position_z: Set(*model.position_z()),
      region_id: Set(*model.region_id()),
    }
  }
}

impl From<Constellation> for ActiveModelEx {
  fn from(model: Constellation) -> Self {
    Self {
      id: Set(*model.id()),
      name: Set(model.name().clone()),
      position_x: Set(*model.position_x()),
      position_y: Set(*model.position_y()),
      position_z: Set(*model.position_z()),
      region: Default::default(),
      region_id: Set(*model.region_id()),
      solar_systems: model
        .solar_systems()
        .iter()
        .cloned()
        .map(Into::into)
        .collect::<Vec<_>>()
        .into(),
    }
  }
}
