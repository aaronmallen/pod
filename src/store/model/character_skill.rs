use getset::CopyGetters;
use sqlx::FromRow;

use crate::clients::esi::models::character::Skill;

#[derive(Clone, Copy, CopyGetters, Debug, Eq, FromRow, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub active_skill_level: i64,
  #[getset(get_copy = "pub")]
  pub character_id: i64,
  #[getset(get_copy = "pub")]
  pub skill_id: i64,
  #[getset(get_copy = "pub")]
  pub skillpoints_in_skill: i64,
  #[getset(get_copy = "pub")]
  pub trained_skill_level: i64,
}

impl From<(i64, Skill)> for Model {
  fn from((character_id, skill): (i64, Skill)) -> Self {
    Self {
      active_skill_level: i64::from(skill.active_skill_level),
      character_id,
      skill_id: i64::from(skill.skill_id),
      skillpoints_in_skill: skill.skillpoints_in_skill,
      trained_skill_level: i64::from(skill.trained_skill_level),
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
    fn it_attaches_character_id_and_widens_levels() {
      let skill = Skill {
        active_skill_level: 4,
        skill_id: 3300,
        skillpoints_in_skill: 90_510,
        trained_skill_level: 5,
      };

      let model = Model::from((42, skill));

      assert_eq!(model.character_id(), 42);
      assert_eq!(model.skill_id(), 3300);
      assert_eq!(model.active_skill_level(), 4);
      assert_eq!(model.trained_skill_level(), 5);
      assert_eq!(model.skillpoints_in_skill(), 90_510);
    }
  }
}
