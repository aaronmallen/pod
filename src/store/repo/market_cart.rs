use crate::store::{
  Database, Error,
  model::{MarketCart, MarketCartLine},
};

#[allow(dead_code)]
pub async fn add_to_live(db: &Database, type_id: i64, quantity: i64) -> Result<(), Error> {
  let cart_id = live_cart_id(db).await?;
  upsert_line(db, cart_id, type_id, quantity).await?;
  touch(db, cart_id).await
}

pub async fn set_quantity(db: &Database, type_id: i64, quantity: i64) -> Result<u64, Error> {
  let cart_id = live_cart_id(db).await?;
  let result = sqlx::query("UPDATE market_cart_line SET quantity = ? WHERE cart_id = ? AND type_id = ?")
    .bind(quantity)
    .bind(cart_id)
    .bind(type_id)
    .execute(db.writer())
    .await?;
  touch(db, cart_id).await?;
  Ok(result.rows_affected())
}

pub async fn remove_line(db: &Database, type_id: i64) -> Result<u64, Error> {
  let cart_id = live_cart_id(db).await?;
  let result = sqlx::query("DELETE FROM market_cart_line WHERE cart_id = ? AND type_id = ?")
    .bind(cart_id)
    .bind(type_id)
    .execute(db.writer())
    .await?;
  touch(db, cart_id).await?;
  Ok(result.rows_affected())
}

pub async fn clear_live(db: &Database) -> Result<(), Error> {
  let cart_id = live_cart_id(db).await?;
  sqlx::query("DELETE FROM market_cart_line WHERE cart_id = ?")
    .bind(cart_id)
    .execute(db.writer())
    .await?;
  touch(db, cart_id).await
}

pub async fn live_lines(db: &Database) -> Result<Vec<MarketCartLine>, Error> {
  let cart_id = live_cart_id(db).await?;
  lines(db, cart_id).await
}

pub async fn lines(db: &Database, cart_id: i64) -> Result<Vec<MarketCartLine>, Error> {
  let rows = sqlx::query_as::<_, MarketCartLine>(
    "SELECT cart_id, id, position, quantity, type_id FROM market_cart_line WHERE cart_id = ? ORDER BY position, id",
  )
  .bind(cart_id)
  .fetch_all(db.reader())
  .await?;
  Ok(rows)
}

pub async fn list_saved(db: &Database) -> Result<Vec<MarketCart>, Error> {
  let rows = sqlx::query_as::<_, MarketCart>(
    "SELECT created_at, id, is_live, name, updated_at FROM market_cart WHERE is_live = 0 ORDER BY id",
  )
  .fetch_all(db.reader())
  .await?;
  Ok(rows)
}

pub async fn save_from_live(db: &Database, name: Option<&str>) -> Result<MarketCart, Error> {
  let live_id = live_cart_id(db).await?;
  let name = resolve_name(db, name).await?;
  let now = chrono::Utc::now().to_rfc3339();
  let cart = sqlx::query_as::<_, MarketCart>(
    "INSERT INTO market_cart (name, is_live, created_at, updated_at) VALUES (?, 0, ?, ?) \
    RETURNING created_at, id, is_live, name, updated_at",
  )
  .bind(&name)
  .bind(&now)
  .bind(&now)
  .fetch_one(db.writer())
  .await?;
  copy_lines(db, live_id, cart.id).await?;
  Ok(cart)
}

pub async fn rename(db: &Database, cart_id: i64, name: &str) -> Result<u64, Error> {
  let now = chrono::Utc::now().to_rfc3339();
  let result = sqlx::query("UPDATE market_cart SET name = ?, updated_at = ? WHERE id = ? AND is_live = 0")
    .bind(name)
    .bind(&now)
    .bind(cart_id)
    .execute(db.writer())
    .await?;
  Ok(result.rows_affected())
}

pub async fn delete(db: &Database, cart_id: i64) -> Result<u64, Error> {
  let result = sqlx::query("DELETE FROM market_cart WHERE id = ? AND is_live = 0")
    .bind(cart_id)
    .execute(db.writer())
    .await?;
  Ok(result.rows_affected())
}

pub async fn load_into_live(db: &Database, cart_id: i64) -> Result<(), Error> {
  let live_id = live_cart_id(db).await?;
  sqlx::query("DELETE FROM market_cart_line WHERE cart_id = ?")
    .bind(live_id)
    .execute(db.writer())
    .await?;
  copy_lines(db, cart_id, live_id).await?;
  touch(db, live_id).await
}

pub async fn merge_into_live(db: &Database, cart_id: i64) -> Result<(), Error> {
  let live_id = live_cart_id(db).await?;
  for line in lines(db, cart_id).await? {
    upsert_line(db, live_id, line.type_id, line.quantity).await?;
  }
  touch(db, live_id).await
}

async fn live_cart_id(db: &Database) -> Result<i64, Error> {
  let existing = sqlx::query_scalar::<_, i64>("SELECT id FROM market_cart WHERE is_live = 1")
    .fetch_optional(db.reader())
    .await?;
  if let Some(id) = existing {
    return Ok(id);
  }
  let now = chrono::Utc::now().to_rfc3339();
  let id = sqlx::query_scalar::<_, i64>(
    "INSERT INTO market_cart (name, is_live, created_at, updated_at) VALUES (NULL, 1, ?, ?) RETURNING id",
  )
  .bind(&now)
  .bind(&now)
  .fetch_one(db.writer())
  .await?;
  Ok(id)
}

async fn upsert_line(db: &Database, cart_id: i64, type_id: i64, quantity: i64) -> Result<(), Error> {
  sqlx::query(
    "INSERT INTO market_cart_line (cart_id, type_id, quantity, position) \
    VALUES (?, ?, ?, (SELECT COALESCE(MAX(position), 0) + 1 FROM market_cart_line WHERE cart_id = ?)) \
    ON CONFLICT(cart_id, type_id) DO UPDATE SET quantity = quantity + excluded.quantity",
  )
  .bind(cart_id)
  .bind(type_id)
  .bind(quantity)
  .bind(cart_id)
  .execute(db.writer())
  .await?;
  Ok(())
}

async fn copy_lines(db: &Database, from_cart_id: i64, to_cart_id: i64) -> Result<(), Error> {
  sqlx::query(
    "INSERT INTO market_cart_line (cart_id, type_id, quantity, position) \
    SELECT ?, type_id, quantity, position FROM market_cart_line WHERE cart_id = ?",
  )
  .bind(to_cart_id)
  .bind(from_cart_id)
  .execute(db.writer())
  .await?;
  Ok(())
}

async fn touch(db: &Database, cart_id: i64) -> Result<(), Error> {
  let now = chrono::Utc::now().to_rfc3339();
  sqlx::query("UPDATE market_cart SET updated_at = ? WHERE id = ?")
    .bind(&now)
    .bind(cart_id)
    .execute(db.writer())
    .await?;
  Ok(())
}

async fn resolve_name(db: &Database, name: Option<&str>) -> Result<String, Error> {
  if let Some(trimmed) = name.map(str::trim).filter(|n| !n.is_empty()) {
    return Ok(trimmed.to_owned());
  }
  auto_name(db).await
}

async fn auto_name(db: &Database) -> Result<String, Error> {
  let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM market_cart WHERE is_live = 0")
    .fetch_one(db.reader())
    .await?;
  let mut n = count + 1;
  loop {
    let candidate = format!("Cart {n}");
    let taken = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM market_cart WHERE name = ?")
      .bind(&candidate)
      .fetch_one(db.reader())
      .await?;
    if taken == 0 {
      return Ok(candidate);
    }
    n += 1;
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::store;

  async fn quantities(db: &Database) -> Vec<(i64, i64)> {
    live_lines(db)
      .await
      .unwrap()
      .into_iter()
      .map(|line| (line.type_id, line.quantity))
      .collect()
  }

  mod live_cart {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_accumulates_quantity_on_repeat_adds() {
      let db = store::open_test().await.unwrap();
      add_to_live(&db, 34, 5).await.unwrap();
      add_to_live(&db, 34, 3).await.unwrap();

      assert_eq!(quantities(&db).await, vec![(34, 8)]);
    }

    #[tokio::test]
    async fn it_sets_a_line_quantity() {
      let db = store::open_test().await.unwrap();
      add_to_live(&db, 34, 5).await.unwrap();

      let affected = set_quantity(&db, 34, 12).await.unwrap();

      assert_eq!(affected, 1);
      assert_eq!(quantities(&db).await, vec![(34, 12)]);
    }

    #[tokio::test]
    async fn it_removes_a_line() {
      let db = store::open_test().await.unwrap();
      add_to_live(&db, 34, 5).await.unwrap();
      add_to_live(&db, 35, 2).await.unwrap();

      let affected = remove_line(&db, 34).await.unwrap();

      assert_eq!(affected, 1);
      assert_eq!(quantities(&db).await, vec![(35, 2)]);
    }

    #[tokio::test]
    async fn it_clears_all_lines() {
      let db = store::open_test().await.unwrap();
      add_to_live(&db, 34, 5).await.unwrap();
      add_to_live(&db, 35, 2).await.unwrap();

      clear_live(&db).await.unwrap();

      assert!(live_lines(&db).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_orders_lines_by_position_and_appends_at_the_tail() {
      let db = store::open_test().await.unwrap();
      add_to_live(&db, 34, 1).await.unwrap();
      add_to_live(&db, 35, 1).await.unwrap();
      add_to_live(&db, 36, 1).await.unwrap();
      remove_line(&db, 35).await.unwrap();
      add_to_live(&db, 37, 1).await.unwrap();

      let ordered: Vec<(i64, i64)> = live_lines(&db)
        .await
        .unwrap()
        .into_iter()
        .map(|line| (line.type_id, line.position))
        .collect();

      assert_eq!(ordered, vec![(34, 1), (36, 3), (37, 4)]);
    }

    #[tokio::test]
    async fn it_keeps_the_original_position_on_repeat_adds() {
      let db = store::open_test().await.unwrap();
      add_to_live(&db, 34, 1).await.unwrap();
      add_to_live(&db, 35, 1).await.unwrap();
      add_to_live(&db, 34, 1).await.unwrap();

      assert_eq!(quantities(&db).await, vec![(34, 2), (35, 1)]);
    }
  }

  mod saved_carts {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_saves_the_live_cart_under_an_explicit_name() {
      let db = store::open_test().await.unwrap();
      add_to_live(&db, 34, 5).await.unwrap();

      let cart = save_from_live(&db, Some("Doctrine")).await.unwrap();

      assert_eq!(cart.name.as_deref(), Some("Doctrine"));
      assert!(!cart.is_live);
      let saved: Vec<(i64, i64)> = lines(&db, cart.id)
        .await
        .unwrap()
        .into_iter()
        .map(|line| (line.type_id, line.quantity))
        .collect();
      assert_eq!(saved, vec![(34, 5)]);
    }

    #[tokio::test]
    async fn it_auto_names_blank_saves_cart_n() {
      let db = store::open_test().await.unwrap();
      add_to_live(&db, 34, 5).await.unwrap();

      let first = save_from_live(&db, None).await.unwrap();
      let second = save_from_live(&db, Some("  ")).await.unwrap();

      assert_eq!(first.name.as_deref(), Some("Cart 1"));
      assert_eq!(second.name.as_deref(), Some("Cart 2"));
    }

    #[tokio::test]
    async fn it_skips_taken_auto_names() {
      let db = store::open_test().await.unwrap();
      save_from_live(&db, Some("Cart 2")).await.unwrap();
      save_from_live(&db, None).await.unwrap();

      let names: Vec<Option<String>> = list_saved(&db).await.unwrap().into_iter().map(|c| c.name).collect();

      assert_eq!(names, vec![Some("Cart 2".to_owned()), Some("Cart 3".to_owned())]);
    }

    #[tokio::test]
    async fn it_renames_a_saved_cart() {
      let db = store::open_test().await.unwrap();
      let cart = save_from_live(&db, Some("Old")).await.unwrap();

      let affected = rename(&db, cart.id, "New").await.unwrap();

      assert_eq!(affected, 1);
      let names: Vec<Option<String>> = list_saved(&db).await.unwrap().into_iter().map(|c| c.name).collect();
      assert_eq!(names, vec![Some("New".to_owned())]);
    }

    #[tokio::test]
    async fn it_deletes_a_saved_cart_and_its_lines() {
      let db = store::open_test().await.unwrap();
      add_to_live(&db, 34, 5).await.unwrap();
      let cart = save_from_live(&db, Some("Doomed")).await.unwrap();

      let affected = delete(&db, cart.id).await.unwrap();

      assert_eq!(affected, 1);
      assert!(list_saved(&db).await.unwrap().is_empty());
      assert!(lines(&db, cart.id).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_refuses_to_delete_the_live_cart() {
      let db = store::open_test().await.unwrap();
      add_to_live(&db, 34, 5).await.unwrap();
      let live_id = live_cart_id(&db).await.unwrap();

      let affected = delete(&db, live_id).await.unwrap();

      assert_eq!(affected, 0);
      assert_eq!(quantities(&db).await, vec![(34, 5)]);
    }

    #[tokio::test]
    async fn it_loads_a_saved_cart_replacing_the_live_cart() {
      let db = store::open_test().await.unwrap();
      add_to_live(&db, 34, 5).await.unwrap();
      let cart = save_from_live(&db, Some("Restock")).await.unwrap();
      clear_live(&db).await.unwrap();
      add_to_live(&db, 35, 9).await.unwrap();

      load_into_live(&db, cart.id).await.unwrap();

      assert_eq!(quantities(&db).await, vec![(34, 5)]);
    }

    #[tokio::test]
    async fn it_merges_a_saved_cart_accumulating_quantities() {
      let db = store::open_test().await.unwrap();
      add_to_live(&db, 34, 5).await.unwrap();
      add_to_live(&db, 35, 2).await.unwrap();
      let cart = save_from_live(&db, Some("Restock")).await.unwrap();
      clear_live(&db).await.unwrap();
      add_to_live(&db, 34, 1).await.unwrap();
      add_to_live(&db, 36, 4).await.unwrap();

      merge_into_live(&db, cart.id).await.unwrap();

      assert_eq!(quantities(&db).await, vec![(34, 6), (36, 4), (35, 2)]);
    }
  }
}
