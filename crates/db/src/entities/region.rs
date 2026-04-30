//! Database entity for EVE Online regions.

use pod_model::Region;
use sea_orm::{Set, prelude::*};

/// A region — the largest spatial division in the EVE universe.
#[sea_orm::model]
#[derive(Clone, Debug, DeriveEntityModel, PartialEq)]
#[sea_orm(table_name = "regions")]
pub struct Model {
  /// Constellations that belong to this region.
  #[sea_orm(has_many)]
  pub constellations: HasMany<super::constellation::Entity>,
  /// Optional lore description for the region.
  pub description: Option<String>,
  /// Unique region identifier (EVE static data ID).
  #[sea_orm(primary_key, auto_increment = false)]
  pub id: i32,
  /// Display name of the region.
  pub name: String,
}

/// Default active-model behaviour with no custom hooks.
impl ActiveModelBehavior for ActiveModel {}

impl From<Model> for Region {
  fn from(entity: Model) -> Self {
    let mut model = Region::new(entity.id, entity.name);
    model.set_description(entity.description).mark_persisted();
    model
  }
}

impl From<ModelEx> for Region {
  fn from(entity: ModelEx) -> Self {
    let mut model = Region::new(entity.id, entity.name);
    *model.constellations_mut() = entity.constellations.into_iter().map(Into::into).collect();
    model.set_description(entity.description).mark_persisted();
    model
  }
}

impl From<Region> for ActiveModel {
  fn from(model: Region) -> Self {
    Self {
      description: Set(model.description().clone()),
      id: Set(*model.id()),
      name: Set(model.name().clone()),
    }
  }
}

impl From<Region> for ActiveModelEx {
  fn from(model: Region) -> Self {
    Self {
      constellations: model
        .constellations()
        .iter()
        .cloned()
        .map(Into::into)
        .collect::<Vec<_>>()
        .into(),
      description: Set(model.description().clone()),
      id: Set(*model.id()),
      name: Set(model.name().clone()),
    }
  }
}
