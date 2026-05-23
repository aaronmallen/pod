//! Database entity for EVE Online character clones.

use pod_model::Clone as CloneModel;
use sea_orm::prelude::*;

/// A clone record stored in the `character_clones` table.
#[sea_orm::model]
#[derive(Clone, Debug, DeriveEntityModel, PartialEq)]
#[sea_orm(table_name = "character_clones")]
pub struct Model {
  /// The character that owns this clone.
  #[sea_orm(belongs_to, from = "character_id", to = "id")]
  pub character: HasOne<super::character::Entity>,
  /// ID of the owning character.
  pub character_id: i64,
  /// Primary key: EVE clone identifier (0 for the active implant set).
  #[sea_orm(primary_key, auto_increment = false)]
  pub id: i64,
  /// Implants installed in this clone.
  #[sea_orm(has_many)]
  pub implants: HasMany<super::character_clone_implant::Entity>,
  /// ISO-8601 timestamp when the clone was installed, if known.
  pub installed_at: Option<String>,
  /// Whether this is the character's active clone.
  pub is_active: bool,
  /// Location structure or station ID.
  pub location_id: i64,
  /// Optional user-assigned name for jump clones.
  pub name: Option<String>,
  /// Resolved region name for display.
  pub region_name: String,
  /// Resolved station or structure name for display.
  pub station_name: String,
  /// ISO-8601 timestamp when this record was last synced from ESI.
  pub synced_at: String,
  /// Solar system ID for this clone's location.
  pub system_id: i32,
}

/// Default active-model behaviour with no custom hooks.
impl ActiveModelBehavior for ActiveModel {}

impl From<Model> for CloneModel {
  fn from(entity: Model) -> Self {
    let mut model = CloneModel::new(
      entity.name.unwrap_or_default(),
      entity.station_name,
      entity.system_id as i64,
      entity.region_name,
    );
    model
      .set_implants(vec![])
      .set_installed_at(entity.installed_at)
      .set_is_active(entity.is_active);
    model
  }
}

impl From<ModelEx> for CloneModel {
  fn from(entity: ModelEx) -> Self {
    let mut model = CloneModel::new(
      entity.name.unwrap_or_default(),
      entity.station_name,
      entity.system_id as i64,
      entity.region_name,
    );
    model
      .set_implants(entity.implants.into_iter().map(Into::into).collect())
      .set_installed_at(entity.installed_at)
      .set_is_active(entity.is_active);
    model
  }
}
