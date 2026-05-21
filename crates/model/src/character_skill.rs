//! Character skill domain model.

use validator::Validate;

/// A character skill record. `(character_id, skill_id)` is the composite key.
#[derive(Clone, Debug, Validate)]
pub struct Model {
  pub character_id: i64,
  pub skill_id: i32,
  #[validate(range(min = 0, max = 5))]
  pub trained_level: i32,
  #[validate(range(min = 0, max = 5))]
  pub active_level: i32,
  pub skillpoints: i64,
  pub training_end_time: Option<i64>,
  /// SP threshold at the start of the current training level; not persisted to DB.
  pub training_level_end_sp: Option<i64>,
  /// SP threshold at the end of the current training level; not persisted to DB.
  pub training_level_start_sp: Option<i64>,
  pub training_start_time: Option<i64>,
  pub training_start_sp: Option<i64>,
  pub is_active_training: bool,
  /// Display name resolved from ESI at runtime; not persisted to DB.
  pub skill_name: Option<String>,
}

impl Model {
  /// Returns `true` if this skill is currently in the training queue.
  pub fn is_training(&self) -> bool {
    self.is_active_training
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod is_training {
    use super::*;

    #[test]
    fn it_returns_true_when_active() {
      let skill = Model {
        active_level: 4,
        character_id: 1,
        is_active_training: true,
        skill_id: 3300,
        skill_name: None,
        skillpoints: 0,
        trained_level: 4,
        training_end_time: None,
        training_level_end_sp: None,
        training_level_start_sp: None,
        training_start_sp: None,
        training_start_time: None,
      };
      assert!(skill.is_training());
    }

    #[test]
    fn it_returns_false_when_not_active() {
      let skill = Model {
        active_level: 5,
        character_id: 1,
        is_active_training: false,
        skill_id: 3300,
        skill_name: None,
        skillpoints: 135_765,
        trained_level: 5,
        training_end_time: None,
        training_level_end_sp: None,
        training_level_start_sp: None,
        training_start_sp: None,
        training_start_time: None,
      };
      assert!(!skill.is_training());
    }
  }
}
