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
