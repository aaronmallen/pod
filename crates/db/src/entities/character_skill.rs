//! Database entity for EVE Online character skills.

use pod_model::CharacterSkill;
use sea_orm::prelude::*;

/// A skill record stored in the `character_skills` table.
#[sea_orm::model]
#[derive(Clone, Debug, DeriveEntityModel, PartialEq)]
#[sea_orm(table_name = "character_skills")]
pub struct Model {
  /// The currently active (trained) skill level.
  pub active_level: i32,
  /// ID of the owning character (composite primary key).
  #[sea_orm(primary_key, auto_increment = false)]
  pub character_id: i64,
  /// Whether this skill is currently in the training queue.
  pub is_active_training: bool,
  /// EVE skill type ID (composite primary key).
  #[sea_orm(primary_key, auto_increment = false)]
  pub skill_id: i32,
  /// Total skillpoints accumulated in this skill.
  pub skillpoints: i64,
  /// Unix timestamp when the current training run finishes.
  pub training_end_time: Option<i64>,
  /// Skillpoints accumulated at the start of the current run.
  pub training_start_sp: Option<i64>,
  /// Unix timestamp when the current training run started.
  pub training_start_time: Option<i64>,
  /// The highest level this skill has been trained to.
  pub trained_level: i32,
}

/// Default active-model behaviour with no custom hooks.
impl ActiveModelBehavior for ActiveModel {}

impl From<Model> for CharacterSkill {
  fn from(m: Model) -> Self {
    Self {
      character_id: m.character_id,
      skill_id: m.skill_id,
      trained_level: m.trained_level,
      active_level: m.active_level,
      skillpoints: m.skillpoints,
      training_end_time: m.training_end_time,
      training_level_end_sp: None,
      training_level_start_sp: None,
      training_start_time: m.training_start_time,
      training_start_sp: m.training_start_sp,
      is_active_training: m.is_active_training,
      skill_name: None,
    }
  }
}
