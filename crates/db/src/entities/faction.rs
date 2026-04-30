//! Database entity for EVE Online factions.

use pod_model::Faction;
use sea_orm::{Set, prelude::*};

/// A faction as stored in the `factions` table.
#[sea_orm::model]
#[derive(Clone, Debug, DeriveEntityModel, PartialEq)]
#[sea_orm(table_name = "factions")]
pub struct Model {
  /// Human-readable description of the faction.
  pub description: String,
  /// Unique EVE faction ID.
  #[sea_orm(primary_key, auto_increment = false)]
  pub id: i32,
  /// Whether this faction is unique within the game world.
  pub is_unique: bool,
  /// Display name of the faction.
  pub name: String,
  /// Relative size factor used for game-balance calculations.
  pub size_factor: f64,
  /// The faction's home solar system.
  #[sea_orm(belongs_to, from = "solar_system_id", to = "id")]
  pub solar_system: HasOne<super::solar_system::Entity>,
  /// Foreign key referencing the faction's home solar system, if set.
  pub solar_system_id: Option<i32>,
}

/// Default active-model behaviour with no custom hooks.
impl ActiveModelBehavior for ActiveModel {}

impl From<Model> for Faction {
  fn from(entity: Model) -> Self {
    let mut model = Faction::new(entity.id, entity.name);
    model
      .set_description(entity.description)
      .set_is_unique(entity.is_unique)
      .set_size_factor(entity.size_factor)
      .set_solar_system_id(entity.solar_system_id)
      .mark_persisted();
    model
  }
}

impl From<ModelEx> for Faction {
  fn from(entity: ModelEx) -> Self {
    let mut model = Faction::new(entity.id, entity.name);
    model
      .set_description(entity.description)
      .set_is_unique(entity.is_unique)
      .set_size_factor(entity.size_factor)
      .set_solar_system_id(entity.solar_system_id)
      .mark_persisted();
    model
  }
}

impl From<Faction> for ActiveModel {
  fn from(model: Faction) -> Self {
    Self {
      description: Set(model.description().clone()),
      id: Set(*model.id()),
      is_unique: Set(*model.is_unique()),
      name: Set(model.name().clone()),
      size_factor: Set(*model.size_factor()),
      solar_system_id: Set(*model.solar_system_id()),
    }
  }
}

impl From<Faction> for ActiveModelEx {
  fn from(model: Faction) -> Self {
    Self {
      description: Set(model.description().clone()),
      id: Set(*model.id()),
      is_unique: Set(*model.is_unique()),
      name: Set(model.name().clone()),
      size_factor: Set(*model.size_factor()),
      solar_system: Default::default(),
      solar_system_id: Set(*model.solar_system_id()),
    }
  }
}
