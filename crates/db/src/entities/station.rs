//! Database entity for dockable stations within EVE Online solar systems.

use pod_model::Station;
use sea_orm::{Set, prelude::*};

use crate::entities::service;

/// A dockable station within a solar system.
#[sea_orm::model]
#[derive(Clone, Debug, DeriveEntityModel, PartialEq)]
#[sea_orm(table_name = "stations")]
pub struct Model {
  /// Unique station identifier.
  #[sea_orm(primary_key, auto_increment = false)]
  pub id: i32,
  /// The station's item type.
  #[sea_orm(belongs_to, from = "item_type_id", to = "id")]
  pub item_type: HasOne<super::item_type::Entity>,
  /// Foreign key referencing the station's item type.
  pub item_type_id: i32,
  /// Maximum ship volume (m³) that may dock at this station.
  pub max_dockable_ship_volume: f64,
  /// Display name of the station.
  pub name: String,
  /// ISK cost to rent an office slot at this station.
  pub office_rental_cost: f64,
  /// Corporation or faction ID that owns this station, if any.
  pub owner_id: Option<i32>,
  /// X coordinate of the station's position in the solar system (metres).
  pub position_x: f64,
  /// Y coordinate of the station's position in the solar system (metres).
  pub position_y: f64,
  /// Z coordinate of the station's position in the solar system (metres).
  pub position_z: f64,
  /// The race associated with this station.
  #[sea_orm(belongs_to, from = "race_id", to = "id")]
  pub race: HasOne<super::race::Entity>,
  /// Foreign key referencing the station's associated race, if any.
  pub race_id: Option<i32>,
  /// Fraction of ore value retained when reprocessing at this station (0–1).
  pub reprocessing_efficiency: f64,
  /// Fraction of reprocessed materials the station takes as a fee (0–1).
  pub reprocessing_stations_take: f64,
  /// Services available at this station.
  pub services: service::List,
  /// The solar system that contains this station.
  #[sea_orm(belongs_to, from = "solar_system_id", to = "id")]
  pub solar_system: HasOne<super::solar_system::Entity>,
  /// Foreign key referencing the solar system that contains this station.
  pub solar_system_id: i32,
}

/// Default active-model behaviour with no custom hooks.
impl ActiveModelBehavior for ActiveModel {}

impl From<Model> for Station {
  fn from(entity: Model) -> Self {
    let mut model = Station::new(entity.id, entity.name);
    *model.services_mut() = entity.services.0;
    model
      .set_item_type_id(entity.item_type_id)
      .set_max_dockable_ship_volume(entity.max_dockable_ship_volume)
      .set_office_rental_cost(entity.office_rental_cost)
      .set_owner_id(entity.owner_id)
      .set_position(entity.position_x, entity.position_y, entity.position_z)
      .set_race_id(entity.race_id)
      .set_reprocessing_efficiency(entity.reprocessing_efficiency)
      .set_reprocessing_stations_take(entity.reprocessing_stations_take)
      .set_solar_system_id(entity.solar_system_id)
      .mark_persisted();
    model
  }
}

impl From<ModelEx> for Station {
  fn from(entity: ModelEx) -> Self {
    let mut model = Station::new(entity.id, entity.name);
    *model.services_mut() = entity.services.0;
    model
      .set_item_type_id(entity.item_type_id)
      .set_max_dockable_ship_volume(entity.max_dockable_ship_volume)
      .set_office_rental_cost(entity.office_rental_cost)
      .set_owner_id(entity.owner_id)
      .set_position(entity.position_x, entity.position_y, entity.position_z)
      .set_race_id(entity.race_id)
      .set_reprocessing_efficiency(entity.reprocessing_efficiency)
      .set_reprocessing_stations_take(entity.reprocessing_stations_take)
      .set_solar_system_id(entity.solar_system_id)
      .mark_persisted();
    model
  }
}

impl From<Station> for ActiveModel {
  fn from(model: Station) -> Self {
    Self {
      id: Set(*model.id()),
      item_type_id: Set(*model.item_type_id()),
      max_dockable_ship_volume: Set(*model.max_dockable_ship_volume()),
      name: Set(model.name().clone()),
      office_rental_cost: Set(*model.office_rental_cost()),
      owner_id: Set(*model.owner_id()),
      position_x: Set(*model.position_x()),
      position_y: Set(*model.position_y()),
      position_z: Set(*model.position_z()),
      race_id: Set(*model.race_id()),
      reprocessing_efficiency: Set(*model.reprocessing_efficiency()),
      reprocessing_stations_take: Set(*model.reprocessing_stations_take()),
      services: Set(service::List(model.services().clone())),
      solar_system_id: Set(*model.solar_system_id()),
    }
  }
}

impl From<Station> for ActiveModelEx {
  fn from(model: Station) -> Self {
    Self {
      id: Set(*model.id()),
      item_type: Default::default(),
      item_type_id: Set(*model.item_type_id()),
      max_dockable_ship_volume: Set(*model.max_dockable_ship_volume()),
      name: Set(model.name().clone()),
      office_rental_cost: Set(*model.office_rental_cost()),
      owner_id: Set(*model.owner_id()),
      position_x: Set(*model.position_x()),
      position_y: Set(*model.position_y()),
      position_z: Set(*model.position_z()),
      race: Default::default(),
      race_id: Set(*model.race_id()),
      reprocessing_efficiency: Set(*model.reprocessing_efficiency()),
      reprocessing_stations_take: Set(*model.reprocessing_stations_take()),
      services: Set(service::List(model.services().clone())),
      solar_system: Default::default(),
      solar_system_id: Set(*model.solar_system_id()),
    }
  }
}
