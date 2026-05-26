//! Repository for character assets.

use pod_model::CharacterAsset;
use sea_orm::{ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, sea_query::OnConflict};
use validator::Validate;

use crate::{
  Error,
  entities::character_asset::{ActiveModel, Column, Entity},
};

/// Repository for character asset read and write operations.
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

  /// Returns all asset rows for the given character IDs.
  #[tracing::instrument(level = "trace", skip(self))]
  pub async fn character_assets_for_character_ids(&self, ids: &[i64]) -> Result<Vec<CharacterAsset>, Error> {
    let rows = Entity::find()
      .filter(Column::CharacterId.is_in(ids.to_vec()))
      .all(self.db)
      .await?;
    Ok(rows.into_iter().map(CharacterAsset::from).collect())
  }

  /// Deletes character asset rows whose `item_id` is not in `keep_ids`.
  ///
  /// If `keep_ids` is empty, all assets for the character are removed.
  #[tracing::instrument(level = "trace", skip(self), fields(character_id = character_id))]
  pub async fn delete_stale_character_assets(&self, character_id: i64, keep_ids: &[i64]) -> Result<u64, Error> {
    let result = if keep_ids.is_empty() {
      Entity::delete_many()
        .filter(Column::CharacterId.eq(character_id))
        .exec(self.db)
        .await?
    } else {
      Entity::delete_many()
        .filter(Column::CharacterId.eq(character_id))
        .filter(Column::ItemId.is_not_in(keep_ids.to_vec()))
        .exec(self.db)
        .await?
    };
    Ok(result.rows_affected)
  }

  /// Upserts all character asset rows for the given character.
  #[tracing::instrument(level = "trace", skip(self), fields(character_id = character_id))]
  pub async fn upsert_character_assets(&self, character_id: i64, assets: &[CharacterAsset]) -> Result<(), Error> {
    for asset in assets {
      asset.validate()?;
      let active = ActiveModel {
        character_id: ActiveValue::Set(character_id),
        is_active_ship: ActiveValue::Set(asset.is_active_ship),
        is_blueprint_copy: ActiveValue::Set(asset.is_blueprint_copy),
        is_singleton: ActiveValue::Set(asset.is_singleton),
        item_id: ActiveValue::Set(asset.item_id),
        location_flag: ActiveValue::Set(asset.location_flag.clone()),
        location_id: ActiveValue::Set(asset.location_id),
        location_type: ActiveValue::Set(asset.location_type.clone()),
        quantity: ActiveValue::Set(asset.quantity),
        ship_name: ActiveValue::Set(asset.ship_name.clone()),
        type_id: ActiveValue::Set(asset.type_id),
      };
      Entity::insert(active)
        .on_conflict(
          OnConflict::column(Column::ItemId)
            .update_columns([
              Column::CharacterId,
              Column::IsActiveShip,
              Column::IsBlueprintCopy,
              Column::IsSingleton,
              Column::LocationFlag,
              Column::LocationId,
              Column::LocationType,
              Column::Quantity,
              Column::ShipName,
              Column::TypeId,
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

  fn make_asset(character_id: i64, item_id: i64) -> CharacterAsset {
    CharacterAsset {
      character_id,
      is_active_ship: false,
      is_blueprint_copy: None,
      is_singleton: false,
      item_id,
      location_flag: "Hangar".to_string(),
      location_id: 60_003_760,
      location_type: "station".to_string(),
      quantity: 1,
      ship_name: None,
      type_id: 587,
    }
  }

  mod character_assets_for_character_ids {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_empty_when_no_assets_exist() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      let result = repo.character_assets_for_character_ids(&[1]).await.unwrap();

      assert_eq!(result.len(), 0);
    }

    #[tokio::test]
    async fn it_returns_assets_for_given_character_ids() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      repo
        .upsert_character_assets(1, &[make_asset(1, 100), make_asset(1, 200)])
        .await
        .unwrap();
      repo.upsert_character_assets(2, &[make_asset(2, 300)]).await.unwrap();

      let result = repo.character_assets_for_character_ids(&[1]).await.unwrap();

      assert_eq!(result.len(), 2);
    }

    #[tokio::test]
    async fn it_returns_assets_for_multiple_character_ids() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      repo.upsert_character_assets(1, &[make_asset(1, 100)]).await.unwrap();
      repo.upsert_character_assets(2, &[make_asset(2, 200)]).await.unwrap();

      let result = repo.character_assets_for_character_ids(&[1, 2]).await.unwrap();

      assert_eq!(result.len(), 2);
    }
  }

  mod delete_stale_character_assets {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_deletes_all_when_keep_ids_is_empty() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      repo
        .upsert_character_assets(1, &[make_asset(1, 100), make_asset(1, 200)])
        .await
        .unwrap();

      let deleted = repo.delete_stale_character_assets(1, &[]).await.unwrap();

      assert_eq!(deleted, 2);
    }

    #[tokio::test]
    async fn it_keeps_specified_ids_and_deletes_others() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      repo
        .upsert_character_assets(1, &[make_asset(1, 100), make_asset(1, 200), make_asset(1, 300)])
        .await
        .unwrap();

      let deleted = repo.delete_stale_character_assets(1, &[100, 300]).await.unwrap();

      assert_eq!(deleted, 1);
      let remaining = repo.character_assets_for_character_ids(&[1]).await.unwrap();
      assert_eq!(remaining.len(), 2);
    }

    #[tokio::test]
    async fn it_does_not_delete_assets_for_other_characters() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      repo.upsert_character_assets(1, &[make_asset(1, 100)]).await.unwrap();
      repo.upsert_character_assets(2, &[make_asset(2, 200)]).await.unwrap();

      repo.delete_stale_character_assets(1, &[]).await.unwrap();

      let remaining = repo.character_assets_for_character_ids(&[2]).await.unwrap();
      assert_eq!(remaining.len(), 1);
    }
  }

  mod upsert_character_assets {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_inserts_new_assets() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      repo.upsert_character_assets(1, &[make_asset(1, 100)]).await.unwrap();

      let rows = repo.character_assets_for_character_ids(&[1]).await.unwrap();
      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].item_id, 100);
    }

    #[tokio::test]
    async fn it_updates_existing_asset_on_conflict() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      repo.upsert_character_assets(1, &[make_asset(1, 100)]).await.unwrap();

      let mut updated = make_asset(1, 100);
      updated.quantity = 42;
      repo.upsert_character_assets(1, &[updated]).await.unwrap();

      let rows = repo.character_assets_for_character_ids(&[1]).await.unwrap();
      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].quantity, 42);
    }
  }
}
