//! Repository for abyssal item persistence.

use pod_model::AbyssalItemRecord;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set, sea_query::OnConflict};

use crate::{
  Error,
  entities::abyssal_item::{ActiveModel, Column, Entity},
};

/// Repository for abyssal item CRUD operations.
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

  /// Returns all abyssal items for the given character.
  #[tracing::instrument(level = "trace", skip(self))]
  pub async fn abyssals_for_character(&self, character_id: i64) -> Result<Vec<AbyssalItemRecord>, Error> {
    let rows = Entity::find()
      .filter(Column::CharacterId.eq(character_id))
      .all(self.db)
      .await?;
    Ok(rows.into_iter().map(Into::into).collect())
  }

  /// Returns all abyssal items across all characters.
  #[tracing::instrument(level = "trace", skip(self))]
  pub async fn all_abyssals(&self) -> Result<Vec<AbyssalItemRecord>, Error> {
    let rows = Entity::find().all(self.db).await?;
    Ok(rows.into_iter().map(Into::into).collect())
  }

  /// Inserts or updates an abyssal item record.
  #[tracing::instrument(level = "trace", skip(self))]
  pub async fn upsert_abyssal(&self, record: AbyssalItemRecord) -> Result<(), Error> {
    let active: ActiveModel = record.into();
    Entity::insert(active)
      .on_conflict(
        OnConflict::column(Column::ItemId)
          .update_columns([
            Column::CharacterId,
            Column::DogmaAttributes,
            Column::MutaPriceIsk,
            Column::MutaPriceSynced,
            Column::MutatorTypeId,
            Column::SourceTypeId,
            Column::SyncedAt,
            Column::TypeId,
          ])
          .to_owned(),
      )
      .exec(self.db)
      .await?;
    Ok(())
  }

  /// Deletes all abyssal items for the given character whose `item_id` is not
  /// in `keep_ids`. Used to prune items that are no longer in character assets.
  #[tracing::instrument(level = "trace", skip(self))]
  pub async fn delete_stale_abyssals(&self, character_id: i64, keep_ids: &[i64]) -> Result<(), Error> {
    Entity::delete_many()
      .filter(Column::CharacterId.eq(character_id))
      .filter(Column::ItemId.is_not_in(keep_ids.to_vec()))
      .exec(self.db)
      .await?;
    Ok(())
  }

  /// Updates the MutaMarket price fields for the given item.
  #[tracing::instrument(level = "trace", skip(self))]
  pub async fn update_abyssal_price(&self, item_id: i64, price_isk: Option<f64>, synced_at: i64) -> Result<(), Error> {
    use sea_orm::ActiveValue::Unchanged;
    let active = ActiveModel {
      item_id: Unchanged(item_id),
      muta_price_isk: Set(price_isk),
      muta_price_synced: Set(Some(synced_at)),
      ..Default::default()
    };
    Entity::update(active).exec(self.db).await?;
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use pod_model::{AbyssalAttribute, AbyssalItemRecord};
  use sea_orm::{Database, DatabaseConnection};

  use super::*;

  async fn setup_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    crate::migrations::run(&db).await.unwrap();
    db
  }

  fn make_record(item_id: i64, character_id: i64, type_id: i32) -> AbyssalItemRecord {
    AbyssalItemRecord::new(
      item_id,
      character_id,
      type_id,
      5975,
      47297,
      vec![AbyssalAttribute::new(6, 450.0)],
      1_700_000_000,
    )
  }

  mod upsert_abyssal {
    use super::*;

    #[tokio::test]
    async fn it_inserts_a_record() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      let record = make_record(100, 1, 47408);

      repo.upsert_abyssal(record).await.unwrap();

      let result = repo.abyssals_for_character(1).await.unwrap();
      assert_eq!(result.len(), 1);
      assert_eq!(*result[0].item_id(), 100);
    }

    #[tokio::test]
    async fn it_updates_on_conflict() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      let record = make_record(100, 1, 47408);
      repo.upsert_abyssal(record).await.unwrap();
      let updated = AbyssalItemRecord::new(
        100,
        1,
        47408,
        5975,
        47297,
        vec![AbyssalAttribute::new(6, 500.0)],
        1_700_000_001,
      );

      repo.upsert_abyssal(updated).await.unwrap();

      let result = repo.abyssals_for_character(1).await.unwrap();
      assert_eq!(result.len(), 1);
      assert_eq!(*result[0].synced_at(), 1_700_000_001);
    }
  }

  mod abyssals_for_character {
    use super::*;

    #[tokio::test]
    async fn it_returns_empty_when_no_items() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      assert!(repo.abyssals_for_character(1).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_filters_by_character_id() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      repo.upsert_abyssal(make_record(1, 100, 47408)).await.unwrap();
      repo.upsert_abyssal(make_record(2, 200, 47408)).await.unwrap();

      let result = repo.abyssals_for_character(100).await.unwrap();

      assert_eq!(result.len(), 1);
      assert_eq!(*result[0].character_id(), 100);
    }
  }

  mod all_abyssals {
    use super::*;

    #[tokio::test]
    async fn it_returns_all_items() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      repo.upsert_abyssal(make_record(1, 100, 47408)).await.unwrap();
      repo.upsert_abyssal(make_record(2, 200, 47410)).await.unwrap();

      assert_eq!(repo.all_abyssals().await.unwrap().len(), 2);
    }
  }

  mod delete_stale_abyssals {
    use super::*;

    #[tokio::test]
    async fn it_deletes_items_not_in_keep_list() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      repo.upsert_abyssal(make_record(1, 100, 47408)).await.unwrap();
      repo.upsert_abyssal(make_record(2, 100, 47410)).await.unwrap();
      repo.upsert_abyssal(make_record(3, 100, 47412)).await.unwrap();

      repo.delete_stale_abyssals(100, &[1, 3]).await.unwrap();

      let result = repo.abyssals_for_character(100).await.unwrap();
      assert_eq!(result.len(), 2);
      let ids: Vec<i64> = result.iter().map(|r| *r.item_id()).collect();
      assert!(ids.contains(&1));
      assert!(ids.contains(&3));
    }

    #[tokio::test]
    async fn it_does_not_delete_other_characters_items() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      repo.upsert_abyssal(make_record(1, 100, 47408)).await.unwrap();
      repo.upsert_abyssal(make_record(2, 200, 47408)).await.unwrap();

      repo.delete_stale_abyssals(100, &[]).await.unwrap();

      assert!(repo.abyssals_for_character(100).await.unwrap().is_empty());
      assert_eq!(repo.abyssals_for_character(200).await.unwrap().len(), 1);
    }
  }

  mod update_abyssal_price {
    use super::*;

    #[tokio::test]
    async fn it_updates_price_fields() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      repo.upsert_abyssal(make_record(1, 100, 47408)).await.unwrap();

      repo
        .update_abyssal_price(1, Some(1_500_000_000.0), 1_700_000_100)
        .await
        .unwrap();

      let result = repo.abyssals_for_character(100).await.unwrap();
      assert_eq!(*result[0].muta_price_isk(), Some(1_500_000_000.0));
      assert_eq!(*result[0].muta_price_synced(), Some(1_700_000_100));
    }
  }
}
