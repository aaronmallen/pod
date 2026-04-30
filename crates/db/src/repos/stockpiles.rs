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
