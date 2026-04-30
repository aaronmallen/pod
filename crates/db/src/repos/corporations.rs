//! Repository for corporation persistence.

use pod_model::Corporation;
use sea_orm::{ActiveValue, DatabaseConnection, EntityTrait, sea_query::OnConflict};
use validator::Validate;

use crate::{
  Error,
  entities::corporation::{ActiveModel as CorpActive, Column as CorpColumn, Entity as CorpEntity},
};

/// Repository for corporation CRUD operations.
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

  /// Returns all corporations.
  pub async fn all(&self) -> Result<Vec<Corporation>, Error> {
    let rows = CorpEntity::find().all(self.db).await?;
    Ok(rows.into_iter().map(Corporation::from).collect())
  }

  /// Deletes a corporation by EVE corporation ID.
  pub async fn delete(&self, corporation_id: i64) -> Result<(), Error> {
    CorpEntity::delete_by_id(corporation_id).exec(self.db).await?;
    Ok(())
  }

  /// Finds a corporation by EVE corporation ID.
  pub async fn find(&self, id: i64) -> Result<Option<Corporation>, Error> {
    let Some(row) = CorpEntity::find_by_id(id).one(self.db).await? else {
      return Ok(None);
    };
    Ok(Some(Corporation::from(row)))
  }

  /// Updates only the OAuth token fields for a corporation.
  pub async fn update_token(
    &self,
    corporation_id: i64,
    access_token: &str,
    refresh_token: &str,
    expires_at: i64,
  ) -> Result<(), Error> {
    let active = CorpActive {
      id: ActiveValue::Set(corporation_id),
      access_token: ActiveValue::Set(access_token.to_string()),
      refresh_token: ActiveValue::Set(refresh_token.to_string()),
      token_expires_at: ActiveValue::Set(expires_at),
      ..Default::default()
    };
    CorpEntity::update(active).exec(self.db).await?;
    Ok(())
  }

  /// Inserts or updates a corporation row, validating first.
  pub async fn upsert(&self, corporation: &Corporation) -> Result<(), Error> {
    corporation.validate()?;
    let active = CorpActive::from(corporation.clone());
    CorpEntity::insert(active)
      .on_conflict(
        OnConflict::column(CorpColumn::Id)
          .update_columns([
            CorpColumn::AccessToken,
            CorpColumn::AllianceId,
            CorpColumn::AllianceName,
            CorpColumn::AuthCharacterId,
            CorpColumn::CeoCharacterId,
            CorpColumn::DateFounded,
            CorpColumn::Description,
            CorpColumn::FactionId,
            CorpColumn::HomeStationId,
            CorpColumn::IconData,
            CorpColumn::MemberCount,
            CorpColumn::Name,
            CorpColumn::RefreshToken,
            CorpColumn::Scopes,
            CorpColumn::Shares,
            CorpColumn::TaxRate,
            CorpColumn::Ticker,
            CorpColumn::TokenExpiresAt,
            CorpColumn::Url,
            CorpColumn::WarEligible,
          ])
          .to_owned(),
      )
      .exec(self.db)
      .await?;
    Ok(())
  }
}
