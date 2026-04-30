//! Database entity for EVE Online bloodlines.

use pod_model::Bloodline;
use sea_orm::{Set, prelude::*};

/// A playable bloodline within a race in EVE Online.
#[sea_orm::model]
#[derive(Clone, Debug, DeriveEntityModel, PartialEq)]
#[sea_orm(table_name = "bloodlines")]
pub struct Model {
  /// Base charisma attribute bonus granted to characters of this bloodline.
  pub charisma: i32,
  /// ID of the NPC corporation associated with this bloodline.
  pub corporation_id: i32,
  /// Human-readable description of the bloodline.
  pub description: String,
  /// Unique bloodline identifier.
  #[sea_orm(primary_key, auto_increment = false)]
  pub id: i32,
  /// Base intelligence attribute bonus granted to characters of this bloodline.
  pub intelligence: i32,
  /// Base memory attribute bonus granted to characters of this bloodline.
  pub memory: i32,
  /// Display name of the bloodline.
  pub name: String,
  /// Base perception attribute bonus granted to characters of this bloodline.
  pub perception: i32,
  /// The race this bloodline belongs to.
  #[sea_orm(belongs_to, from = "race_id", to = "id")]
  pub race: HasOne<super::race::Entity>,
  /// Foreign key referencing the parent race.
  pub race_id: i32,
  /// The starter ship item type associated with this bloodline.
  #[sea_orm(belongs_to, from = "ship_item_type_id", to = "id")]
  pub ship_item_type: HasOne<super::item_type::Entity>,
  /// Foreign key referencing the starter ship item type.
  pub ship_item_type_id: i32,
  /// Base willpower attribute bonus granted to characters of this bloodline.
  pub will_power: i32,
}

/// Default active-model behaviour with no custom hooks.
impl ActiveModelBehavior for ActiveModel {}

impl From<Model> for Bloodline {
  fn from(entity: Model) -> Self {
    let mut model = Bloodline::new(entity.id, entity.name);
    model
      .set_charisma(entity.charisma)
      .set_corporation_id(entity.corporation_id)
      .set_description(entity.description)
      .set_intelligence(entity.intelligence)
      .set_memory(entity.memory)
      .set_perception(entity.perception)
      .set_race_id(entity.race_id)
      .set_ship_item_type_id(entity.ship_item_type_id)
      .set_will_power(entity.will_power)
      .mark_persisted();
    model
  }
}

impl From<ModelEx> for Bloodline {
  fn from(entity: ModelEx) -> Self {
    let mut model = Bloodline::new(entity.id, entity.name);
    model
      .set_charisma(entity.charisma)
      .set_corporation_id(entity.corporation_id)
      .set_description(entity.description)
      .set_intelligence(entity.intelligence)
      .set_memory(entity.memory)
      .set_perception(entity.perception)
      .set_race_id(entity.race_id)
      .set_ship_item_type_id(entity.ship_item_type_id)
      .set_will_power(entity.will_power)
      .mark_persisted();
    model
  }
}

impl From<Bloodline> for ActiveModel {
  fn from(model: Bloodline) -> Self {
    Self {
      charisma: Set(*model.charisma()),
      corporation_id: Set(*model.corporation_id()),
      description: Set(model.description().clone()),
      id: Set(*model.id()),
      intelligence: Set(*model.intelligence()),
      memory: Set(*model.memory()),
      name: Set(model.name().clone()),
      perception: Set(*model.perception()),
      race_id: Set(*model.race_id()),
      ship_item_type_id: Set(*model.ship_item_type_id()),
      will_power: Set(*model.will_power()),
    }
  }
}

impl From<Bloodline> for ActiveModelEx {
  fn from(model: Bloodline) -> Self {
    Self {
      charisma: Set(*model.charisma()),
      corporation_id: Set(*model.corporation_id()),
      description: Set(model.description().clone()),
      id: Set(*model.id()),
      intelligence: Set(*model.intelligence()),
      memory: Set(*model.memory()),
      name: Set(model.name().clone()),
      perception: Set(*model.perception()),
      race: Default::default(),
      race_id: Set(*model.race_id()),
      ship_item_type: Default::default(),
      ship_item_type_id: Set(*model.ship_item_type_id()),
      will_power: Set(*model.will_power()),
    }
  }
}
