//! Database entity for stars within EVE Online solar systems.

use pod_model::Star;
use sea_orm::{Set, prelude::*};

/// A star at the center of a solar system.
#[sea_orm::model]
#[derive(Clone, Debug, DeriveEntityModel, PartialEq)]
#[sea_orm(table_name = "stars")]
pub struct Model {
  /// Age of the star in years.
  pub age: i64,
  /// Unique EVE star ID (primary key).
  #[sea_orm(primary_key, auto_increment = false)]
  pub id: i32,
  /// The item type associated with this star.
  #[sea_orm(belongs_to, from = "item_type_id", to = "id")]
  pub item_type: HasOne<super::item_type::Entity>,
  /// Foreign key referencing the star's item type.
  pub item_type_id: i32,
  /// Luminosity of the star relative to the Sun.
  pub luminosity: f64,
  /// Display name of the star.
  pub name: String,
  /// Radius of the star in metres.
  pub radius: i64,
  /// The solar system this star belongs to.
  #[sea_orm(belongs_to, from = "solar_system_id", to = "id")]
  pub solar_system: HasOne<super::solar_system::Entity>,
  /// Foreign key referencing the star's solar system.
  pub solar_system_id: i32,
  /// Stellar classification (e.g. `"G2 V"`).
  pub spectral_class: String,
  /// Surface temperature of the star in Kelvin.
  pub temperature: i32,
}

/// Default active-model behaviour with no custom hooks.
impl ActiveModelBehavior for ActiveModel {}

impl From<Model> for Star {
  fn from(entity: Model) -> Self {
    let mut model = Star::new(entity.id, entity.name);
    model
      .set_age(entity.age)
      .set_item_type_id(entity.item_type_id)
      .set_luminosity(entity.luminosity)
      .set_radius(entity.radius)
      .set_solar_system_id(entity.solar_system_id)
      .set_spectral_class(entity.spectral_class)
      .set_temperature(entity.temperature)
      .mark_persisted();
    model
  }
}

impl From<ModelEx> for Star {
  fn from(entity: ModelEx) -> Self {
    let mut model = Star::new(entity.id, entity.name);
    model
      .set_age(entity.age)
      .set_item_type_id(entity.item_type_id)
      .set_luminosity(entity.luminosity)
      .set_radius(entity.radius)
      .set_solar_system_id(entity.solar_system_id)
      .set_spectral_class(entity.spectral_class)
      .set_temperature(entity.temperature)
      .mark_persisted();
    model
  }
}

impl From<Star> for ActiveModel {
  fn from(model: Star) -> Self {
    Self {
      age: Set(*model.age()),
      id: Set(*model.id()),
      item_type_id: Set(*model.item_type_id()),
      luminosity: Set(*model.luminosity()),
      name: Set(model.name().clone()),
      radius: Set(*model.radius()),
      solar_system_id: Set(*model.solar_system_id()),
      spectral_class: Set(model.spectral_class().clone()),
      temperature: Set(*model.temperature()),
    }
  }
}

impl From<Star> for ActiveModelEx {
  fn from(model: Star) -> Self {
    Self {
      age: Set(*model.age()),
      id: Set(*model.id()),
      item_type: Default::default(),
      item_type_id: Set(*model.item_type_id()),
      luminosity: Set(*model.luminosity()),
      name: Set(model.name().clone()),
      radius: Set(*model.radius()),
      solar_system: Default::default(),
      solar_system_id: Set(*model.solar_system_id()),
      spectral_class: Set(model.spectral_class().clone()),
      temperature: Set(*model.temperature()),
    }
  }
}
