//! Database entity for EVE Online races.

use pod_model::Race;
use sea_orm::{Set, prelude::*};

/// A playable race in EVE Online, grouping related bloodlines and lore.
#[sea_orm::model]
#[derive(Clone, Debug, DeriveEntityModel, PartialEq)]
#[sea_orm(table_name = "races")]
pub struct Model {
  /// ID of the NPC alliance associated with this race.
  pub alliance_id: i32,
  /// Bloodlines that belong to this race.
  #[sea_orm(has_many)]
  pub bloodlines: HasMany<super::bloodline::Entity>,
  /// Lore description of the race.
  pub description: String,
  /// Unique race identifier from the ESI.
  #[sea_orm(primary_key, auto_increment = false)]
  pub id: i32,
  /// Display name of the race.
  pub name: String,
}

/// Default active-model behaviour with no custom hooks.
impl ActiveModelBehavior for ActiveModel {}

impl From<Model> for Race {
  fn from(entity: Model) -> Self {
    let mut model = Race::new(entity.id, entity.name);
    model
      .set_alliance_id(entity.alliance_id)
      .set_description(entity.description)
      .mark_persisted();
    model
  }
}

impl From<ModelEx> for Race {
  fn from(entity: ModelEx) -> Self {
    let mut model = Race::new(entity.id, entity.name);
    *model.bloodlines_mut() = entity.bloodlines.into_iter().map(Into::into).collect();
    model
      .set_alliance_id(entity.alliance_id)
      .set_description(entity.description)
      .mark_persisted();
    model
  }
}

impl From<Race> for ActiveModel {
  fn from(model: Race) -> Self {
    Self {
      alliance_id: Set(*model.alliance_id()),
      description: Set(model.description().clone()),
      id: Set(*model.id()),
      name: Set(model.name().clone()),
    }
  }
}

impl From<Race> for ActiveModelEx {
  fn from(model: Race) -> Self {
    Self {
      alliance_id: Set(*model.alliance_id()),
      bloodlines: model
        .bloodlines()
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
