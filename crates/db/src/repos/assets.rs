//! Repository for corporation assets and asset sync state.

use pod_model::{AssetSyncState, CorporationAsset};
use sea_orm::{ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, sea_query::OnConflict};
use validator::Validate;

use crate::{
  Error,
  entities::{
    asset_sync_state::{ActiveModel as SyncStateActive, Column as SyncStateColumn, Entity as SyncStateEntity},
    corporation_asset::{ActiveModel as CorpAssetActive, Column as CorpAssetColumn, Entity as CorpAssetEntity},
  },
};

/// Repository for corporation assets and cross-cutting asset sync state.
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

  /// Returns raw corporation asset entity rows for all given corporation IDs.
  pub async fn corporation_assets_for_corporation_ids(
    &self,
    corp_ids: &[i64],
  ) -> Result<Vec<crate::entities::corporation_asset::Model>, Error> {
    let rows = CorpAssetEntity::find()
      .filter(CorpAssetColumn::CorporationId.is_in(corp_ids.to_vec()))
      .all(self.db)
      .await?;
    Ok(rows)
  }

  /// Deletes corporation asset rows whose `item_id` is not in `keep_ids`.
  /// If `keep_ids` is empty all assets for the corporation are removed.
  pub async fn delete_stale_corporation_assets(&self, corporation_id: i64, keep_ids: &[i64]) -> Result<u64, Error> {
    let result = if keep_ids.is_empty() {
      CorpAssetEntity::delete_many()
        .filter(CorpAssetColumn::CorporationId.eq(corporation_id))
        .exec(self.db)
        .await?
    } else {
      CorpAssetEntity::delete_many()
        .filter(CorpAssetColumn::CorporationId.eq(corporation_id))
        .filter(CorpAssetColumn::ItemId.is_not_in(keep_ids.to_vec()))
        .exec(self.db)
        .await?
    };
    Ok(result.rows_affected)
  }

  /// Returns the sync state record for the given owner, if present.
  pub async fn get_asset_sync_state(&self, owner_type: &str, owner_id: i64) -> Result<Option<AssetSyncState>, Error> {
    let row = SyncStateEntity::find_by_id((owner_id, owner_type.to_string()))
      .one(self.db)
      .await?;
    Ok(row.map(AssetSyncState::from))
  }

  /// Upserts all corporation asset rows for the given corporation.
  pub async fn upsert_corporation_assets(&self, corporation_id: i64, assets: &[CorporationAsset]) -> Result<(), Error> {
    for asset in assets {
      asset.validate()?;
      let active = CorpAssetActive {
        corporation_id: ActiveValue::Set(corporation_id),
        is_blueprint_copy: ActiveValue::Set(asset.is_blueprint_copy),
        is_singleton: ActiveValue::Set(asset.is_singleton),
        item_id: ActiveValue::Set(asset.item_id),
        location_flag: ActiveValue::Set(asset.location_flag.clone()),
        location_id: ActiveValue::Set(asset.location_id),
        location_type: ActiveValue::Set(asset.location_type.clone()),
        quantity: ActiveValue::Set(asset.quantity),
        type_id: ActiveValue::Set(asset.type_id),
      };
      CorpAssetEntity::insert(active)
        .on_conflict(
          OnConflict::column(CorpAssetColumn::ItemId)
            .update_columns([
              CorpAssetColumn::CorporationId,
              CorpAssetColumn::IsBlueprintCopy,
              CorpAssetColumn::IsSingleton,
              CorpAssetColumn::LocationFlag,
              CorpAssetColumn::LocationId,
              CorpAssetColumn::LocationType,
              CorpAssetColumn::Quantity,
              CorpAssetColumn::TypeId,
            ])
            .to_owned(),
        )
        .exec(self.db)
        .await?;
    }
    Ok(())
  }

  /// Upserts the sync state record for the given owner.
  pub async fn upsert_asset_sync_state(
    &self,
    owner_type: &str,
    owner_id: i64,
    last_synced_at: Option<i64>,
    cache_expires_at: Option<i64>,
  ) -> Result<(), Error> {
    let active = SyncStateActive {
      cache_expires_at: ActiveValue::Set(cache_expires_at),
      last_synced_at: ActiveValue::Set(last_synced_at),
      owner_id: ActiveValue::Set(owner_id),
      owner_type: ActiveValue::Set(owner_type.to_string()),
    };
    SyncStateEntity::insert(active)
      .on_conflict(
        OnConflict::columns([SyncStateColumn::OwnerType, SyncStateColumn::OwnerId])
          .update_columns([SyncStateColumn::LastSyncedAt, SyncStateColumn::CacheExpiresAt])
          .to_owned(),
      )
      .exec(self.db)
      .await?;
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

  fn make_corp_asset(corporation_id: i64, item_id: i64) -> CorporationAsset {
    CorporationAsset {
      corporation_id,
      is_blueprint_copy: None,
      is_singleton: false,
      item_id,
      location_flag: "CorpSAG1".to_string(),
      location_id: 60_003_760,
      location_type: "station".to_string(),
      quantity: 1,
      type_id: 34,
    }
  }

  mod delete_stale_corporation_assets {
    use super::*;

    #[tokio::test]
    async fn it_deletes_all_when_keep_ids_empty() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      repo
        .upsert_corporation_assets(1, &[make_corp_asset(1, 100), make_corp_asset(1, 200)])
        .await
        .unwrap();

      let deleted = repo.delete_stale_corporation_assets(1, &[]).await.unwrap();

      assert_eq!(deleted, 2);
    }

    #[tokio::test]
    async fn it_keeps_specified_ids_and_deletes_others() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      repo
        .upsert_corporation_assets(
          1,
          &[
            make_corp_asset(1, 100),
            make_corp_asset(1, 200),
            make_corp_asset(1, 300),
          ],
        )
        .await
        .unwrap();

      let deleted = repo.delete_stale_corporation_assets(1, &[100, 300]).await.unwrap();

      assert_eq!(deleted, 1);
      let remaining = repo.corporation_assets_for_corporation_ids(&[1]).await.unwrap();
      assert_eq!(remaining.len(), 2);
    }

    #[tokio::test]
    async fn it_does_not_delete_assets_for_other_corporations() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      repo
        .upsert_corporation_assets(1, &[make_corp_asset(1, 100)])
        .await
        .unwrap();
      repo
        .upsert_corporation_assets(2, &[make_corp_asset(2, 200)])
        .await
        .unwrap();

      repo.delete_stale_corporation_assets(1, &[]).await.unwrap();

      let remaining = repo.corporation_assets_for_corporation_ids(&[2]).await.unwrap();
      assert_eq!(remaining.len(), 1);
    }
  }

  mod get_asset_sync_state {
    use super::*;

    #[tokio::test]
    async fn it_returns_none_when_no_record_exists() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      let result = repo.get_asset_sync_state("character", 1).await.unwrap();

      assert!(result.is_none());
    }

    #[tokio::test]
    async fn it_returns_the_record_after_upsert() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      repo
        .upsert_asset_sync_state("character", 1, Some(1_000_000), Some(1_003_600))
        .await
        .unwrap();

      let result = repo.get_asset_sync_state("character", 1).await.unwrap();

      assert!(result.is_some());
      let state = result.unwrap();
      assert_eq!(state.last_synced_at, Some(1_000_000));
      assert_eq!(state.cache_expires_at, Some(1_003_600));
    }
  }

  mod upsert_asset_sync_state {
    use super::*;

    #[tokio::test]
    async fn it_overwrites_existing_record_on_conflict() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      repo
        .upsert_asset_sync_state("character", 1, Some(100), Some(3700))
        .await
        .unwrap();
      repo
        .upsert_asset_sync_state("character", 1, Some(200), Some(3800))
        .await
        .unwrap();

      let result = repo.get_asset_sync_state("character", 1).await.unwrap().unwrap();

      assert_eq!(result.last_synced_at, Some(200));
      assert_eq!(result.cache_expires_at, Some(3800));
    }

    #[tokio::test]
    async fn it_stores_character_and_corporation_records_independently() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      repo
        .upsert_asset_sync_state("character", 1, Some(100), Some(3700))
        .await
        .unwrap();
      repo
        .upsert_asset_sync_state("corporation", 1, Some(200), Some(3800))
        .await
        .unwrap();

      let char_state = repo.get_asset_sync_state("character", 1).await.unwrap().unwrap();
      let corp_state = repo.get_asset_sync_state("corporation", 1).await.unwrap().unwrap();

      assert_eq!(char_state.last_synced_at, Some(100));
      assert_eq!(corp_state.last_synced_at, Some(200));
    }
  }

  mod upsert_corporation_assets {
    use super::*;

    #[tokio::test]
    async fn it_inserts_new_assets() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      repo
        .upsert_corporation_assets(1, &[make_corp_asset(1, 100)])
        .await
        .unwrap();

      let rows = repo.corporation_assets_for_corporation_ids(&[1]).await.unwrap();
      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].item_id, 100);
    }

    #[tokio::test]
    async fn it_updates_existing_asset_on_conflict() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      repo
        .upsert_corporation_assets(1, &[make_corp_asset(1, 100)])
        .await
        .unwrap();

      let mut updated = make_corp_asset(1, 100);
      updated.quantity = 99;
      repo.upsert_corporation_assets(1, &[updated]).await.unwrap();

      let rows = repo.corporation_assets_for_corporation_ids(&[1]).await.unwrap();
      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].quantity, 99);
    }
  }
}
