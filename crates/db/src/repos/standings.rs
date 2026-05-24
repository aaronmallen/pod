//! Repository for character standing persistence.

use sea_orm::{ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, sea_query::OnConflict};

use crate::{
  Error,
  entities::character_standing::{
    ActiveModel as StandingActive, Column as StandingColumn, Entity as StandingEntity, Model as StandingModel,
  },
};

/// Repository for character standing CRUD operations.
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

  /// Returns all standing rows for the given character.
  pub async fn find_for_character(&self, character_id: i64) -> Result<Vec<StandingModel>, Error> {
    let rows = StandingEntity::find()
      .filter(StandingColumn::CharacterId.eq(character_id))
      .all(self.db)
      .await?;
    Ok(rows)
  }

  /// Upserts all standing rows for the given character using ON CONFLICT DO UPDATE.
  pub async fn upsert_for_character(&self, character_id: i64, standings: &[StandingModel]) -> Result<(), Error> {
    for standing in standings {
      let active = StandingActive {
        character_id: ActiveValue::Set(character_id),
        effective_standing: ActiveValue::Set(standing.effective_standing),
        from_id: ActiveValue::Set(standing.from_id),
        from_name: ActiveValue::Set(standing.from_name.clone()),
        from_type: ActiveValue::Set(standing.from_type.clone()),
        id: ActiveValue::NotSet,
        raw_standing: ActiveValue::Set(standing.raw_standing),
        synced_at: ActiveValue::Set(standing.synced_at.clone()),
      };
      StandingEntity::insert(active)
        .on_conflict(
          OnConflict::columns([StandingColumn::CharacterId, StandingColumn::FromId])
            .update_columns([
              StandingColumn::EffectiveStanding,
              StandingColumn::FromName,
              StandingColumn::FromType,
              StandingColumn::RawStanding,
              StandingColumn::SyncedAt,
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

  async fn insert_character(db: &DatabaseConnection, id: i64, name: &str) {
    use sea_orm::ActiveValue::Set;
    crate::entities::character::Entity::insert(crate::entities::character::ActiveModel {
      access_token: Set(String::new()),
      charisma: Set(None),
      corp_id: Set(0),
      corp_name: Set(String::new()),
      granted_scopes: Set(None),
      id: Set(id),
      intelligence: Set(None),
      isk_balance: Set(None),
      location_docked: Set(None),
      location_name: Set(None),
      memory: Set(None),
      name: Set(name.to_string()),
      perception: Set(None),
      portrait_tone: Set(0),
      refresh_token: Set(String::new()),
      sort_order: Set(0),
      token_expires_at: Set(0),
      willpower: Set(None),
    })
    .exec(db)
    .await
    .unwrap();
  }

  fn make_standing(character_id: i64, from_id: i32, from_name: &str, standing: f64) -> StandingModel {
    StandingModel {
      character_id,
      effective_standing: standing,
      from_id,
      from_name: from_name.to_string(),
      from_type: "faction".to_string(),
      id: 0,
      raw_standing: standing,
      synced_at: "2025-01-01T00:00:00Z".to_string(),
    }
  }

  mod find_for_character {
    use super::*;

    #[tokio::test]
    async fn returns_empty_when_no_standings() {
      let db = setup_db().await;
      insert_character(&db, 1, "Alice").await;
      let repo = Repo::new(&db);

      let result = repo.find_for_character(1).await.unwrap();
      assert!(result.is_empty());
    }

    #[tokio::test]
    async fn returns_standings_after_upsert() {
      let db = setup_db().await;
      insert_character(&db, 1, "Alice").await;
      let repo = Repo::new(&db);

      let standing = make_standing(1, 500001, "Caldari State", 5.0);
      repo.upsert_for_character(1, &[standing]).await.unwrap();

      let result = repo.find_for_character(1).await.unwrap();
      assert_eq!(result.len(), 1);
      assert_eq!(result[0].from_id, 500001);
      assert_eq!(result[0].from_name, "Caldari State");
    }

    #[tokio::test]
    async fn does_not_return_standings_for_other_characters() {
      let db = setup_db().await;
      insert_character(&db, 1, "Alice").await;
      insert_character(&db, 2, "Bob").await;
      let repo = Repo::new(&db);

      let standing = make_standing(1, 500001, "Caldari State", 5.0);
      repo.upsert_for_character(1, &[standing]).await.unwrap();

      let result = repo.find_for_character(2).await.unwrap();
      assert!(result.is_empty());
    }
  }

  mod upsert_for_character {
    use super::*;

    #[tokio::test]
    async fn upserts_multiple_standings() {
      let db = setup_db().await;
      insert_character(&db, 1, "Alice").await;
      let repo = Repo::new(&db);

      let s1 = make_standing(1, 500001, "Caldari State", 5.0);
      let s2 = make_standing(1, 500002, "Minmatar Republic", -5.0);
      repo.upsert_for_character(1, &[s1, s2]).await.unwrap();

      let result = repo.find_for_character(1).await.unwrap();
      assert_eq!(result.len(), 2);
    }

    #[tokio::test]
    async fn updates_existing_standing_on_conflict() {
      let db = setup_db().await;
      insert_character(&db, 1, "Alice").await;
      let repo = Repo::new(&db);

      let s1 = make_standing(1, 500001, "Caldari State", 0.0);
      repo.upsert_for_character(1, &[s1]).await.unwrap();

      let s2 = make_standing(1, 500001, "Caldari State", 9.5);
      repo.upsert_for_character(1, &[s2]).await.unwrap();

      let result = repo.find_for_character(1).await.unwrap();
      assert_eq!(result.len(), 1);
      assert_eq!(result[0].effective_standing, 9.5);
    }
  }
}
