use getset::{CopyGetters, Getters};
use sqlx::FromRow;

use crate::clients::esi::models::character::SkillQueueEntry;

#[derive(Clone, CopyGetters, Debug, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub character_id: i64,
  #[getset(get = "pub")]
  pub finish_date: Option<String>,
  #[getset(get_copy = "pub")]
  pub finished_level: i64,
  #[getset(get_copy = "pub")]
  pub level_end_sp: Option<i64>,
  #[getset(get_copy = "pub")]
  pub level_start_sp: Option<i64>,
  #[getset(get_copy = "pub")]
  pub queue_position: i64,
  #[getset(get_copy = "pub")]
  pub skill_id: i64,
  #[getset(get = "pub")]
  pub start_date: Option<String>,
  #[getset(get_copy = "pub")]
  pub training_start_sp: Option<i64>,
}

impl From<(i64, SkillQueueEntry)> for Model {
  fn from((character_id, entry): (i64, SkillQueueEntry)) -> Self {
    Self {
      character_id,
      finish_date: entry.finish_date,
      finished_level: i64::from(entry.finished_level),
      level_end_sp: entry.level_end_sp,
      level_start_sp: entry.level_start_sp,
      queue_position: i64::from(entry.queue_position),
      skill_id: i64::from(entry.skill_id),
      start_date: entry.start_date,
      training_start_sp: entry.training_start_sp,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod from {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_carries_nulls_for_a_not_yet_started_entry() {
      let entry = SkillQueueEntry {
        finish_date: None,
        finished_level: 3,
        level_end_sp: None,
        level_start_sp: None,
        queue_position: 2,
        skill_id: 3301,
        start_date: None,
        training_start_sp: None,
      };

      let model = Model::from((42, entry));

      assert_eq!(model.finish_date().as_deref(), None);
      assert_eq!(model.start_date().as_deref(), None);
      assert_eq!(model.level_end_sp(), None);
      assert_eq!(model.training_start_sp(), None);
    }

    #[test]
    fn it_widens_ids_and_carries_optional_fields() {
      let entry = SkillQueueEntry {
        finish_date: Some("2026-06-01T00:00:00Z".to_owned()),
        finished_level: 5,
        level_end_sp: Some(256_000),
        level_start_sp: Some(45_255),
        queue_position: 0,
        skill_id: 3300,
        start_date: Some("2026-05-01T00:00:00Z".to_owned()),
        training_start_sp: Some(45_255),
      };

      let model = Model::from((42, entry));

      assert_eq!(model.character_id(), 42);
      assert_eq!(model.queue_position(), 0);
      assert_eq!(model.skill_id(), 3300);
      assert_eq!(model.finished_level(), 5);
      assert_eq!(model.finish_date().as_deref(), Some("2026-06-01T00:00:00Z"));
      assert_eq!(model.level_end_sp(), Some(256_000));
    }
  }
}
