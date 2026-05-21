//! Repository for character killmail persistence.

use sea_orm::{ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, sea_query::OnConflict};

use crate::{
  Error,
  entities::character_killmail::{
    ActiveModel as KillActive, Column as KillColumn, Entity as KillEntity, Model as KillModel,
  },
};

/// Repository for character killmail CRUD operations.
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

  /// Returns all killmail rows for the given character.
  pub async fn find_for_character(&self, character_id: i64) -> Result<Vec<KillModel>, Error> {
    let rows = KillEntity::find()
      .filter(KillColumn::CharacterId.eq(character_id))
      .all(self.db)
      .await?;
    Ok(rows)
  }

  /// Upserts all killmail rows for the given character using ON CONFLICT DO UPDATE.
  pub async fn upsert_for_character(&self, character_id: i64, kills: &[KillModel]) -> Result<(), Error> {
    for kill in kills {
      let active = KillActive {
        attacker_count: ActiveValue::Set(kill.attacker_count),
        character_id: ActiveValue::Set(character_id),
        final_blow: ActiveValue::Set(kill.final_blow),
        id: ActiveValue::NotSet,
        is_kill: ActiveValue::Set(kill.is_kill),
        kill_hash: ActiveValue::Set(kill.kill_hash.clone()),
        kill_time: ActiveValue::Set(kill.kill_time.clone()),
        killmail_id: ActiveValue::Set(kill.killmail_id),
        ship_name: ActiveValue::Set(kill.ship_name.clone()),
        ship_type_id: ActiveValue::Set(kill.ship_type_id),
        synced_at: ActiveValue::Set(kill.synced_at.clone()),
        system_id: ActiveValue::Set(kill.system_id),
        system_name: ActiveValue::Set(kill.system_name.clone()),
        system_sec: ActiveValue::Set(kill.system_sec),
        value_isk: ActiveValue::Set(kill.value_isk),
        victim_corp_name: ActiveValue::Set(kill.victim_corp_name.clone()),
        victim_name: ActiveValue::Set(kill.victim_name.clone()),
      };
      KillEntity::insert(active)
        .on_conflict(
          OnConflict::columns([KillColumn::CharacterId, KillColumn::KillmailId])
            .update_columns([
              KillColumn::AttackerCount,
              KillColumn::FinalBlow,
              KillColumn::IsKill,
              KillColumn::KillHash,
              KillColumn::KillTime,
              KillColumn::ShipName,
              KillColumn::ShipTypeId,
              KillColumn::SyncedAt,
              KillColumn::SystemId,
              KillColumn::SystemName,
              KillColumn::SystemSec,
              KillColumn::ValueIsk,
              KillColumn::VictimCorpName,
              KillColumn::VictimName,
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

  async fn insert_character(db: &DatabaseConnection, id: i64) {
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
      name: Set(format!("Character {id}")),
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

  fn make_kill(character_id: i64, killmail_id: i32, is_kill: bool) -> KillModel {
    KillModel {
      attacker_count: 5,
      character_id,
      final_blow: is_kill,
      id: 0,
      is_kill,
      kill_hash: format!("hash{killmail_id}"),
      kill_time: "2025-01-01T12:00:00Z".to_string(),
      killmail_id,
      ship_name: "Rifter".to_string(),
      ship_type_id: 587,
      synced_at: "2025-01-01T00:00:00Z".to_string(),
      system_id: 30000142,
      system_name: "Jita".to_string(),
      system_sec: 0.9,
      value_isk: 1_000_000.0,
      victim_corp_name: "Some Corp".to_string(),
      victim_name: "Enemy Pilot".to_string(),
    }
  }

  mod find_for_character {
    use super::*;

    #[tokio::test]
    async fn returns_empty_when_no_killmails() {
      let db = setup_db().await;
      insert_character(&db, 1).await;
      let repo = Repo::new(&db);

      let result = repo.find_for_character(1).await.unwrap();
      assert!(result.is_empty());
    }

    #[tokio::test]
    async fn returns_killmails_after_upsert() {
      let db = setup_db().await;
      insert_character(&db, 1).await;
      let repo = Repo::new(&db);

      let kill = make_kill(1, 9001, true);
      repo.upsert_for_character(1, &[kill]).await.unwrap();

      let result = repo.find_for_character(1).await.unwrap();
      assert_eq!(result.len(), 1);
      assert_eq!(result[0].killmail_id, 9001);
      assert!(result[0].is_kill);
    }

    #[tokio::test]
    async fn does_not_return_killmails_for_other_characters() {
      let db = setup_db().await;
      insert_character(&db, 1).await;
      insert_character(&db, 2).await;
      let repo = Repo::new(&db);

      let kill = make_kill(1, 9001, true);
      repo.upsert_for_character(1, &[kill]).await.unwrap();

      let result = repo.find_for_character(2).await.unwrap();
      assert!(result.is_empty());
    }
  }

  mod upsert_for_character {
    use super::*;

    #[tokio::test]
    async fn upserts_multiple_killmails() {
      let db = setup_db().await;
      insert_character(&db, 1).await;
      let repo = Repo::new(&db);

      let k1 = make_kill(1, 9001, true);
      let k2 = make_kill(1, 9002, false);
      repo.upsert_for_character(1, &[k1, k2]).await.unwrap();

      let result = repo.find_for_character(1).await.unwrap();
      assert_eq!(result.len(), 2);
    }

    #[tokio::test]
    async fn updates_existing_killmail_on_conflict() {
      let db = setup_db().await;
      insert_character(&db, 1).await;
      let repo = Repo::new(&db);

      let mut kill = make_kill(1, 9001, true);
      kill.value_isk = 1_000_000.0;
      repo.upsert_for_character(1, &[kill]).await.unwrap();

      let mut updated = make_kill(1, 9001, true);
      updated.value_isk = 2_000_000.0;
      repo.upsert_for_character(1, &[updated]).await.unwrap();

      let result = repo.find_for_character(1).await.unwrap();
      assert_eq!(result.len(), 1);
      assert_eq!(result[0].value_isk, 2_000_000.0);
    }
  }
}
