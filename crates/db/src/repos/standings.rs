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
