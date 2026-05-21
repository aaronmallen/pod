//! Repository for stockpile persistence.

use sea_orm::{
  ActiveValue, ColumnTrait, DatabaseConnection, DbBackend, EntityTrait, FromQueryResult, QueryFilter, Statement,
};

use crate::{
  Error,
  entities::{
    stockpile::{ActiveModel as StockpileActive, Entity as StockpileEntity},
    stockpile_item::{ActiveModel as ItemActive, Column as ItemColumn, Entity as ItemEntity},
  },
};

/// A stockpile with all of its item requirements loaded.
#[derive(Clone, Debug)]
pub struct StockpileWithItems {
  /// Primary key of the stockpile.
  pub id: i64,
  /// Display name.
  pub name: String,
  /// Optional location scope; `None` means all locations.
  pub location_id: Option<i64>,
  /// Optional character scope; `None` means all characters.
  pub character_id: Option<i64>,
  /// Item requirements belonging to this stockpile.
  pub items: Vec<StockpileItem>,
}

/// A single item requirement row belonging to a stockpile.
#[derive(Clone, Debug)]
pub struct StockpileItem {
  /// Primary key of the stockpile_item row.
  pub id: i64,
  /// EVE type ID.
  pub type_id: i32,
  /// Desired quantity to keep stocked.
  pub target_quantity: i32,
}

/// Fill-status row for one item in a stockpile.
#[derive(Clone, Debug)]
pub struct StockpileItemStatus {
  /// EVE type ID.
  pub type_id: i32,
  /// Desired quantity to keep stocked.
  pub target_quantity: i32,
  /// Sum of matching character_assets quantities.
  pub have_quantity: i64,
  /// Human-readable item name from item_types.
  pub type_name: String,
}

#[derive(Debug, FromQueryResult)]
struct FillRow {
  type_id: i32,
  target_quantity: i32,
  have_quantity: i64,
  type_name: String,
}

/// Repository for stockpile CRUD operations.
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

  /// Returns all stockpiles with their item requirements loaded.
  pub async fn list_stockpiles(&self) -> Result<Vec<StockpileWithItems>, Error> {
    let rows = StockpileEntity::find().all(self.db).await?;
    let mut result = Vec::with_capacity(rows.len());
    for row in rows {
      let item_rows = ItemEntity::find()
        .filter(ItemColumn::StockpileId.eq(row.id))
        .all(self.db)
        .await?;
      let items = item_rows
        .into_iter()
        .map(|r| StockpileItem {
          id: r.id,
          type_id: r.type_id,
          target_quantity: r.target_quantity,
        })
        .collect();
      result.push(StockpileWithItems {
        id: row.id,
        name: row.name,
        location_id: row.location_id,
        character_id: row.character_id,
        items,
      });
    }
    Ok(result)
  }

  /// Inserts a new stockpile with the given items and returns the new stockpile ID.
  ///
  /// `items` is a slice of `(type_id, target_quantity)` pairs.
  pub async fn create_stockpile(
    &self,
    name: &str,
    location_id: Option<i64>,
    character_id: Option<i64>,
    items: &[(i32, i32)],
  ) -> Result<i64, Error> {
    let active = StockpileActive {
      id: ActiveValue::NotSet,
      name: ActiveValue::Set(name.to_string()),
      location_id: ActiveValue::Set(location_id),
      character_id: ActiveValue::Set(character_id),
    };
    let res = StockpileEntity::insert(active).exec(self.db).await?;
    let new_id = res.last_insert_id;
    self.insert_items(new_id, items).await?;
    Ok(new_id)
  }

  /// Replaces all fields and items for the given stockpile atomically.
  ///
  /// Existing item rows are deleted and re-inserted from `items`.
  pub async fn update_stockpile(
    &self,
    id: i64,
    name: &str,
    location_id: Option<i64>,
    character_id: Option<i64>,
    items: &[(i32, i32)],
  ) -> Result<(), Error> {
    let active = StockpileActive {
      id: ActiveValue::Set(id),
      name: ActiveValue::Set(name.to_string()),
      location_id: ActiveValue::Set(location_id),
      character_id: ActiveValue::Set(character_id),
    };
    StockpileEntity::update(active).exec(self.db).await?;
    ItemEntity::delete_many()
      .filter(ItemColumn::StockpileId.eq(id))
      .exec(self.db)
      .await?;
    self.insert_items(id, items).await?;
    Ok(())
  }

  /// Deletes a stockpile by ID; items cascade via the FK constraint.
  pub async fn delete_stockpile(&self, id: i64) -> Result<(), Error> {
    StockpileEntity::delete_by_id(id).exec(self.db).await?;
    Ok(())
  }

  /// Returns fill-status rows for all items in the given stockpile.
  ///
  /// For each item, the `have_quantity` is the sum of `quantity` from
  /// `character_assets` filtered by the stockpile's `location_id` (when set)
  /// and `character_id` (when set). `type_name` is resolved from `item_types`.
  pub async fn stockpile_fill_status(&self, id: i64) -> Result<Vec<StockpileItemStatus>, Error> {
    let Some(pile) = StockpileEntity::find_by_id(id).one(self.db).await? else {
      return Ok(Vec::new());
    };

    let items = ItemEntity::find()
      .filter(ItemColumn::StockpileId.eq(id))
      .all(self.db)
      .await?;

    if items.is_empty() {
      return Ok(Vec::new());
    }

    let loc_filter = if pile.location_id.is_some() {
      " AND ca.location_id = ?"
    } else {
      ""
    };
    let char_filter = if pile.character_id.is_some() {
      " AND ca.character_id = ?"
    } else {
      ""
    };

    let sql = format!(
      r#"
      SELECT
        si.type_id,
        si.target_quantity,
        COALESCE(SUM(ca.quantity), 0) AS have_quantity,
        COALESCE(it.name, 'Type ' || CAST(si.type_id AS TEXT)) AS type_name
      FROM stockpile_items si
      LEFT JOIN character_assets ca
        ON ca.type_id = si.type_id{loc_filter}{char_filter}
      LEFT JOIN item_types it ON it.id = si.type_id
      WHERE si.stockpile_id = ?
      GROUP BY si.type_id, si.target_quantity, it.name
      ORDER BY it.name
      "#,
    );

    let mut bind_values: Vec<sea_orm::Value> = Vec::new();
    if let Some(loc) = pile.location_id {
      bind_values.push(loc.into());
    }
    if let Some(char) = pile.character_id {
      bind_values.push(char.into());
    }
    bind_values.push(pile.id.into());

    let rows = FillRow::find_by_statement(Statement::from_sql_and_values(DbBackend::Sqlite, &sql, bind_values))
      .all(self.db)
      .await?;

    Ok(
      rows
        .into_iter()
        .map(|r| StockpileItemStatus {
          type_id: r.type_id,
          target_quantity: r.target_quantity,
          have_quantity: r.have_quantity,
          type_name: r.type_name,
        })
        .collect(),
    )
  }

  async fn insert_items(&self, stockpile_id: i64, items: &[(i32, i32)]) -> Result<(), Error> {
    for &(type_id, target_quantity) in items {
      let active = ItemActive {
        id: ActiveValue::NotSet,
        stockpile_id: ActiveValue::Set(stockpile_id),
        type_id: ActiveValue::Set(type_id),
        target_quantity: ActiveValue::Set(target_quantity),
      };
      ItemEntity::insert(active).exec(self.db).await?;
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

  static NEXT_ITEM_ID: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(100_000);

  async fn insert_asset(db: &DatabaseConnection, character_id: i64, type_id: i32, location_id: i64, quantity: i32) {
    use sea_orm::ActiveValue::Set;

    use crate::entities::character_asset::{ActiveModel as AssetActive, Entity as AssetEntity};
    let item_id = NEXT_ITEM_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    AssetEntity::insert(AssetActive {
      character_id: Set(character_id),
      is_blueprint_copy: Set(None),
      is_singleton: Set(false),
      item_id: Set(item_id),
      location_flag: Set("Hangar".to_string()),
      location_id: Set(location_id),
      location_type: Set("station".to_string()),
      quantity: Set(quantity),
      type_id: Set(type_id),
    })
    .exec(db)
    .await
    .unwrap();
  }

  mod list_stockpiles {
    use super::*;

    #[tokio::test]
    async fn returns_empty_when_no_stockpiles() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      let result = repo.list_stockpiles().await.unwrap();
      assert!(result.is_empty());
    }

    #[tokio::test]
    async fn returns_stockpile_with_items() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      repo
        .create_stockpile("Supply Cache", None, None, &[(34, 1000), (35, 500)])
        .await
        .unwrap();

      let result = repo.list_stockpiles().await.unwrap();
      assert_eq!(result.len(), 1);
      assert_eq!(result[0].name, "Supply Cache");
      assert_eq!(result[0].items.len(), 2);
    }
  }

  mod create_stockpile {
    use super::*;

    #[tokio::test]
    async fn returns_new_id() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      let id = repo.create_stockpile("Alpha", None, None, &[]).await.unwrap();
      assert!(id > 0);
    }

    #[tokio::test]
    async fn creates_with_optional_location_and_character() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      let id = repo
        .create_stockpile("Filtered", Some(60003760), Some(1001), &[(34, 100)])
        .await
        .unwrap();

      let piles = repo.list_stockpiles().await.unwrap();
      let pile = piles.iter().find(|p| p.id == id).unwrap();
      assert_eq!(pile.location_id, Some(60003760));
      assert_eq!(pile.character_id, Some(1001));
    }
  }

  mod update_stockpile {
    use super::*;

    #[tokio::test]
    async fn replaces_items_and_updates_fields() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      let id = repo
        .create_stockpile("Old Name", None, None, &[(34, 100)])
        .await
        .unwrap();
      repo
        .update_stockpile(id, "New Name", None, None, &[(35, 200)])
        .await
        .unwrap();

      let piles = repo.list_stockpiles().await.unwrap();
      let pile = piles.iter().find(|p| p.id == id).unwrap();
      assert_eq!(pile.name, "New Name");
      assert_eq!(pile.items.len(), 1);
      assert_eq!(pile.items[0].type_id, 35);
    }
  }

  mod delete_stockpile {
    use super::*;

    #[tokio::test]
    async fn removes_the_stockpile() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      let id = repo.create_stockpile("To Delete", None, None, &[]).await.unwrap();
      repo.delete_stockpile(id).await.unwrap();

      let piles = repo.list_stockpiles().await.unwrap();
      assert!(piles.iter().all(|p| p.id != id));
    }
  }

  mod stockpile_fill_status {
    use super::*;

    #[tokio::test]
    async fn returns_empty_when_stockpile_does_not_exist() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      let result = repo.stockpile_fill_status(9999).await.unwrap();
      assert!(result.is_empty());
    }

    #[tokio::test]
    async fn returns_empty_when_stockpile_has_no_items() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      let id = repo.create_stockpile("Empty", None, None, &[]).await.unwrap();
      let result = repo.stockpile_fill_status(id).await.unwrap();
      assert!(result.is_empty());
    }

    #[tokio::test]
    async fn returns_zero_have_quantity_when_no_assets() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      let id = repo
        .create_stockpile("Unfilled", None, None, &[(34, 500)])
        .await
        .unwrap();
      let result = repo.stockpile_fill_status(id).await.unwrap();
      assert_eq!(result.len(), 1);
      assert_eq!(result[0].type_id, 34);
      assert_eq!(result[0].target_quantity, 500);
      assert_eq!(result[0].have_quantity, 0);
    }

    #[tokio::test]
    async fn sums_asset_quantities_without_filter() {
      let db = setup_db().await;
      insert_character(&db, 1).await;
      insert_character(&db, 2).await;
      insert_asset(&db, 1, 34, 60003760, 300).await;
      insert_asset(&db, 2, 34, 60004588, 200).await;

      let repo = Repo::new(&db);
      let id = repo.create_stockpile("Global", None, None, &[(34, 500)]).await.unwrap();
      let result = repo.stockpile_fill_status(id).await.unwrap();
      assert_eq!(result[0].have_quantity, 500);
    }

    #[tokio::test]
    async fn filters_by_location_id_when_set() {
      let db = setup_db().await;
      insert_character(&db, 1).await;
      insert_asset(&db, 1, 34, 60003760, 300).await;
      insert_asset(&db, 1, 34, 60004588, 200).await;

      let repo = Repo::new(&db);
      let id = repo
        .create_stockpile("Location Filtered", Some(60003760), None, &[(34, 500)])
        .await
        .unwrap();
      let result = repo.stockpile_fill_status(id).await.unwrap();
      assert_eq!(result[0].have_quantity, 300);
    }

    #[tokio::test]
    async fn filters_by_character_id_when_set() {
      let db = setup_db().await;
      insert_character(&db, 1).await;
      insert_character(&db, 2).await;
      insert_asset(&db, 1, 34, 60003760, 300).await;
      insert_asset(&db, 2, 34, 60003760, 200).await;

      let repo = Repo::new(&db);
      let id = repo
        .create_stockpile("Char Filtered", None, Some(1), &[(34, 500)])
        .await
        .unwrap();
      let result = repo.stockpile_fill_status(id).await.unwrap();
      assert_eq!(result[0].have_quantity, 300);
    }

    #[tokio::test]
    async fn filters_by_both_location_and_character_when_both_set() {
      let db = setup_db().await;
      insert_character(&db, 1).await;
      insert_character(&db, 2).await;
      insert_asset(&db, 1, 34, 60003760, 100).await;
      insert_asset(&db, 1, 34, 60004588, 50).await;
      insert_asset(&db, 2, 34, 60003760, 200).await;

      let repo = Repo::new(&db);
      let id = repo
        .create_stockpile("Both Filtered", Some(60003760), Some(1), &[(34, 500)])
        .await
        .unwrap();
      let result = repo.stockpile_fill_status(id).await.unwrap();
      assert_eq!(result[0].have_quantity, 100);
    }
  }
}
