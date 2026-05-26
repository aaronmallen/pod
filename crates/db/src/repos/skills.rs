//! Repository for character skills.

use std::collections::HashMap;

use pod_model::CharacterSkill;
use sea_orm::{ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, sea_query::OnConflict};

use crate::{
  Error,
  entities::{
    character_skill::{ActiveModel, Column, Entity},
    item_type::{Column as TypeColumn, Entity as TypeEntity},
  },
};

/// Repository for character skill read and write operations.
pub struct Repo<'a> {
  db: &'a DatabaseConnection,
}

impl<'a> Repo<'a> {
  /// Creates a new `Repo` bound to the given database connection.
  pub fn new(db: &'a DatabaseConnection) -> Self {
    Self {
      db,
    }
  }

  /// Returns all skill rows for the given character, with skill names resolved from `item_types`.
  #[tracing::instrument(level = "trace", skip(self), fields(character_id = character_id))]
  pub async fn skills_for_character(&self, character_id: i64) -> Result<Vec<CharacterSkill>, Error> {
    let rows = Entity::find()
      .filter(Column::CharacterId.eq(character_id))
      .all(self.db)
      .await?;
    let skill_ids: Vec<i32> = rows.iter().map(|r| r.skill_id).collect();
    let name_map: HashMap<i32, String> = TypeEntity::find()
      .filter(TypeColumn::Id.is_in(skill_ids))
      .all(self.db)
      .await
      .unwrap_or_default()
      .into_iter()
      .map(|t| (t.id, t.name))
      .collect();
    Ok(
      rows
        .into_iter()
        .map(|r| {
          let mut skill = CharacterSkill::from(r);
          skill.skill_name = name_map.get(&skill.skill_id).cloned();
          skill
        })
        .collect(),
    )
  }

  /// Upserts all skill rows for the given character.
  #[tracing::instrument(level = "trace", skip(self), fields(character_id = character_id))]
  pub async fn upsert_character_skills(&self, character_id: i64, skills: &[CharacterSkill]) -> Result<(), Error> {
    for skill in skills {
      let active = ActiveModel {
        active_level: ActiveValue::Set(skill.active_level),
        character_id: ActiveValue::Set(character_id),
        is_active_training: ActiveValue::Set(skill.is_active_training),
        skill_id: ActiveValue::Set(skill.skill_id),
        skillpoints: ActiveValue::Set(skill.skillpoints),
        trained_level: ActiveValue::Set(skill.trained_level),
        training_end_time: ActiveValue::Set(skill.training_end_time),
        training_start_sp: ActiveValue::Set(skill.training_start_sp),
        training_start_time: ActiveValue::Set(skill.training_start_time),
      };
      Entity::insert(active)
        .on_conflict(
          OnConflict::columns([Column::CharacterId, Column::SkillId])
            .update_columns([
              Column::ActiveLevel,
              Column::IsActiveTraining,
              Column::Skillpoints,
              Column::TrainedLevel,
              Column::TrainingEndTime,
              Column::TrainingStartSp,
              Column::TrainingStartTime,
            ])
            .to_owned(),
        )
        .exec(self.db)
        .await?;
    }
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use sea_orm::{Database, DatabaseConnection};

  use super::*;

  async fn setup_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    crate::migrations::run(&db).await.unwrap();
    db
  }

  fn make_skill(character_id: i64, skill_id: i32) -> CharacterSkill {
    CharacterSkill {
      active_level: 3,
      character_id,
      is_active_training: false,
      skill_id,
      skill_name: None,
      skillpoints: 24_000,
      trained_level: 3,
      training_end_time: None,
      training_level_end_sp: None,
      training_level_start_sp: None,
      training_start_sp: None,
      training_start_time: None,
    }
  }

  mod skills_for_character {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_empty_when_no_skills_exist() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      let result = repo.skills_for_character(1).await.unwrap();

      assert_eq!(result.len(), 0);
    }

    #[tokio::test]
    async fn it_returns_skills_for_the_given_character() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      repo
        .upsert_character_skills(1, &[make_skill(1, 3300), make_skill(1, 3301)])
        .await
        .unwrap();
      repo.upsert_character_skills(2, &[make_skill(2, 3300)]).await.unwrap();

      let result = repo.skills_for_character(1).await.unwrap();

      assert_eq!(result.len(), 2);
    }

    #[tokio::test]
    async fn it_does_not_return_skills_for_other_characters() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      repo.upsert_character_skills(1, &[make_skill(1, 3300)]).await.unwrap();

      let result = repo.skills_for_character(2).await.unwrap();

      assert_eq!(result.len(), 0);
    }
  }

  mod upsert_character_skills {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_inserts_new_skills() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      repo.upsert_character_skills(1, &[make_skill(1, 3300)]).await.unwrap();

      let rows = repo.skills_for_character(1).await.unwrap();
      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].skill_id, 3300);
    }

    #[tokio::test]
    async fn it_updates_existing_skill_on_conflict() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      repo.upsert_character_skills(1, &[make_skill(1, 3300)]).await.unwrap();

      let mut updated = make_skill(1, 3300);
      updated.trained_level = 5;
      updated.skillpoints = 135_765;
      repo.upsert_character_skills(1, &[updated]).await.unwrap();

      let rows = repo.skills_for_character(1).await.unwrap();
      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].trained_level, 5);
      assert_eq!(rows[0].skillpoints, 135_765);
    }
  }
}
