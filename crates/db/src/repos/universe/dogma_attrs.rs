//! Repository for dogma attribute definition persistence.

use pod_model::DogmaAttr;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, sea_query::OnConflict};

use crate::{
  Error,
  entities::dogma_attr::{ActiveModel, Column, Entity},
};

/// Repository for dogma attribute CRUD operations.
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

  /// Returns all dogma attribute definitions.
  #[tracing::instrument(level = "trace", skip(self))]
  pub async fn all(&self) -> Result<Vec<DogmaAttr>, Error> {
    let rows = Entity::find().all(self.db).await?;
    Ok(rows.into_iter().map(Into::into).collect())
  }

  /// Returns dogma attributes matching the given attribute IDs.
  #[tracing::instrument(level = "trace", skip(self))]
  pub async fn find_by_ids(&self, attribute_ids: &[i32]) -> Result<Vec<DogmaAttr>, Error> {
    if attribute_ids.is_empty() {
      return Ok(Vec::new());
    }
    let rows = Entity::find()
      .filter(Column::AttributeId.is_in(attribute_ids.to_vec()))
      .all(self.db)
      .await?;
    Ok(rows.into_iter().map(Into::into).collect())
  }

  /// Bulk-upserts dogma attribute rows in chunks of 500.
  #[tracing::instrument(level = "trace", skip(self))]
  pub async fn upsert_many(&self, records: &[DogmaAttr]) -> Result<(), Error> {
    if records.is_empty() {
      return Ok(());
    }
    let active: Vec<ActiveModel> = records.iter().cloned().map(Into::into).collect();
    for chunk in active.chunks(500) {
      Entity::insert_many(chunk.to_vec())
        .on_conflict(
          OnConflict::column(Column::AttributeId)
            .update_columns([
              Column::DefaultValue,
              Column::Description,
              Column::DisplayName,
              Column::HighIsGood,
              Column::IconId,
              Column::Name,
              Column::Published,
              Column::Stackable,
              Column::UnitId,
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
  use pod_model::DogmaAttr;
  use sea_orm::{Database, DatabaseConnection};

  use super::*;

  async fn setup_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    crate::migrations::run(&db).await.unwrap();
    db
  }

  fn make_attr(id: i32, name: &str) -> DogmaAttr {
    DogmaAttr::new(id, name)
  }

  mod all {
    use super::*;

    #[tokio::test]
    async fn it_returns_empty_when_no_attrs() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      assert!(repo.all().await.unwrap().is_empty());
    }
  }

  mod find_by_ids {
    use super::*;

    #[tokio::test]
    async fn it_returns_empty_for_empty_ids() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      let result = repo.find_by_ids(&[]).await.unwrap();

      assert!(result.is_empty());
    }

    #[tokio::test]
    async fn it_returns_matching_attrs() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      let records = vec![
        make_attr(6, "capacitorNeed"),
        make_attr(50, "cpu"),
        make_attr(77, "mass"),
      ];
      repo.upsert_many(&records).await.unwrap();

      let result = repo.find_by_ids(&[6, 77]).await.unwrap();

      assert_eq!(result.len(), 2);
      let ids: Vec<i32> = result.iter().map(|a| *a.attribute_id()).collect();
      assert!(ids.contains(&6));
      assert!(ids.contains(&77));
    }

    #[tokio::test]
    async fn it_returns_empty_when_ids_not_found() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      let result = repo.find_by_ids(&[999]).await.unwrap();

      assert!(result.is_empty());
    }
  }

  mod upsert_many {
    use super::*;

    #[tokio::test]
    async fn it_does_nothing_when_empty() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      repo.upsert_many(&[]).await.unwrap();

      assert!(repo.all().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_inserts_multiple_attrs() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      let records = vec![make_attr(6, "capacitorNeed"), make_attr(50, "cpu")];

      repo.upsert_many(&records).await.unwrap();

      assert_eq!(repo.all().await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn it_updates_on_conflict() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      let mut attr = make_attr(6, "capacitorNeed");
      repo.upsert_many(&[attr.clone()]).await.unwrap();
      attr.set_icon_id(Some(1400));

      repo.upsert_many(&[attr]).await.unwrap();

      let result = repo.all().await.unwrap();
      assert_eq!(result.len(), 1);
      assert_eq!(*result[0].icon_id(), Some(1400));
    }
  }
}
