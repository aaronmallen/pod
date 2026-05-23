//! Database entity for EVE Online solar systems.

use pod_model::SolarSystem;
use sea_orm::{Set, prelude::*};

/// A solar system within the EVE universe.
#[sea_orm::model]
#[derive(Clone, Debug, DeriveEntityModel, PartialEq)]
#[sea_orm(table_name = "solar_systems")]
pub struct Model {
  /// The constellation this solar system belongs to.
  #[sea_orm(belongs_to, from = "constellation_id", to = "id")]
  pub constellation: HasOne<super::constellation::Entity>,
  /// Foreign key referencing the parent constellation.
  pub constellation_id: i32,
  /// Unique EVE solar system ID.
  #[sea_orm(primary_key, auto_increment = false)]
  pub id: i32,
  /// Display name of the solar system.
  pub name: String,
  /// Planets located within this solar system.
  #[sea_orm(has_many)]
  pub planets: HasMany<super::planet::Entity>,
  /// X coordinate of the solar system's position in space (metres).
  pub position_x: f64,
  /// Y coordinate of the solar system's position in space (metres).
  pub position_y: f64,
  /// Z coordinate of the solar system's position in space (metres).
  pub position_z: f64,
  /// Security classification label (e.g. "A", "B"), if assigned.
  pub security_class: Option<String>,
  /// Numeric security status of the solar system (typically -1.0 to 1.0).
  pub security_status: f64,
  /// The star at the center of this solar system, if present.
  #[sea_orm(belongs_to, from = "star_id", to = "id")]
  pub star: HasOne<super::star::Entity>,
  /// Foreign key referencing the solar system's star, if one exists.
  pub star_id: Option<i32>,
  /// Stargates located within this solar system.
  #[sea_orm(has_many)]
  pub stargates: HasMany<super::stargate::Entity>,
  /// NPC and player-owned stations within this solar system.
  #[sea_orm(has_many)]
  pub stations: HasMany<super::station::Entity>,
}

/// Default active-model behaviour with no custom hooks.
impl ActiveModelBehavior for ActiveModel {}

impl From<Model> for SolarSystem {
  fn from(entity: Model) -> Self {
    let mut model = SolarSystem::new(entity.id, entity.name);
    model
      .set_constellation_id(entity.constellation_id)
      .set_position(entity.position_x, entity.position_y, entity.position_z)
      .set_security_class(entity.security_class)
      .set_security_status(entity.security_status)
      .set_star_id(entity.star_id)
      .mark_persisted();
    model
  }
}

impl From<ModelEx> for SolarSystem {
  fn from(entity: ModelEx) -> Self {
    let mut model = SolarSystem::new(entity.id, entity.name);
    *model.planets_mut() = entity.planets.into_iter().map(Into::into).collect();
    *model.stargates_mut() = entity.stargates.into_iter().map(Into::into).collect();
    *model.stations_mut() = entity.stations.into_iter().map(Into::into).collect();
    model
      .set_constellation(entity.constellation.into_option().map(Into::into))
      .set_constellation_id(entity.constellation_id)
      .set_position(entity.position_x, entity.position_y, entity.position_z)
      .set_security_class(entity.security_class)
      .set_security_status(entity.security_status)
      .set_star_id(entity.star_id)
      .mark_persisted();
    model
  }
}

impl From<SolarSystem> for ActiveModel {
  fn from(model: SolarSystem) -> Self {
    Self {
      constellation_id: Set(*model.constellation_id()),
      id: Set(*model.id()),
      name: Set(model.name().clone()),
      position_x: Set(*model.position_x()),
      position_y: Set(*model.position_y()),
      position_z: Set(*model.position_z()),
      security_class: Set(model.security_class().clone()),
      security_status: Set(*model.security_status()),
      star_id: Set(*model.star_id()),
    }
  }
}

impl From<SolarSystem> for ActiveModelEx {
  fn from(model: SolarSystem) -> Self {
    Self {
      constellation: Default::default(),
      constellation_id: Set(*model.constellation_id()),
      id: Set(*model.id()),
      name: Set(model.name().clone()),
      planets: model
        .planets()
        .iter()
        .cloned()
        .map(Into::into)
        .collect::<Vec<_>>()
        .into(),
      position_x: Set(*model.position_x()),
      position_y: Set(*model.position_y()),
      position_z: Set(*model.position_z()),
      security_class: Set(model.security_class().clone()),
      security_status: Set(*model.security_status()),
      star: Default::default(),
      star_id: Set(*model.star_id()),
      stargates: model
        .stargates()
        .iter()
        .cloned()
        .map(Into::into)
        .collect::<Vec<_>>()
        .into(),
      stations: model
        .stations()
        .iter()
        .cloned()
        .map(Into::into)
        .collect::<Vec<_>>()
        .into(),
    }
  }
}
