use sqlx::{QueryBuilder, Sqlite};

use crate::store::{Database, Error};

/// Dogma attribute ids used in the CASE expressions below: 30 = power (PG) draw, 50 = CPU draw, 1547 = rig
/// calibration size — fixed ids from the EVE SDE dogma attribute schema, not arbitrary constants.
const MODULE_SELECT: &str = "SELECT it.id AS type_id, it.name AS name, it.group_id AS group_id, \
  MAX(CASE WHEN CAST(json_extract(attr.value, '$.attribute_id') AS INTEGER) = 30 \
    THEN CAST(json_extract(attr.value, '$.value') AS REAL) END) AS power, \
  MAX(CASE WHEN CAST(json_extract(attr.value, '$.attribute_id') AS INTEGER) = 50 \
    THEN CAST(json_extract(attr.value, '$.value') AS REAL) END) AS cpu, \
  MAX(CASE WHEN CAST(json_extract(attr.value, '$.attribute_id') AS INTEGER) = 1547 \
    THEN CAST(json_extract(attr.value, '$.value') AS REAL) END) AS rig_size \
  FROM item_types it LEFT JOIN json_each(it.dogma_attributes) attr ON 1 = 1 ";

#[derive(Clone, Debug, PartialEq, sqlx::FromRow)]
pub struct FittingModuleRow {
  pub cpu: Option<f64>,
  pub group_id: i64,
  pub name: String,
  pub power: Option<f64>,
  pub rig_size: Option<f64>,
  pub type_id: i64,
}

#[derive(Clone, Debug, PartialEq, sqlx::FromRow)]
pub struct HullCapacityRow {
  pub cpu_output: Option<f64>,
  pub power_output: Option<f64>,
}

pub async fn modules_by_names(db: &Database, names: &[String]) -> Result<Vec<FittingModuleRow>, Error> {
  if names.is_empty() {
    return Ok(Vec::new());
  }
  let mut builder = QueryBuilder::<Sqlite>::new(MODULE_SELECT);
  builder.push("WHERE it.name COLLATE NOCASE IN (");
  let mut separated = builder.separated(", ");
  for name in names {
    separated.push_bind(name.as_str());
  }
  separated.push_unseparated(") GROUP BY it.id");
  let rows = builder.build_query_as::<FittingModuleRow>().fetch_all(&db.0).await?;
  Ok(rows)
}

#[allow(dead_code)]
pub async fn modules_by_ids(db: &Database, type_ids: &[i64]) -> Result<Vec<FittingModuleRow>, Error> {
  if type_ids.is_empty() {
    return Ok(Vec::new());
  }
  let mut builder = QueryBuilder::<Sqlite>::new(MODULE_SELECT);
  builder.push("WHERE it.id IN (");
  let mut separated = builder.separated(", ");
  for id in type_ids {
    separated.push_bind(*id);
  }
  separated.push_unseparated(") GROUP BY it.id");
  let rows = builder.build_query_as::<FittingModuleRow>().fetch_all(&db.0).await?;
  Ok(rows)
}

pub async fn hull_capacity(db: &Database, hull_type_id: i64) -> Result<Option<HullCapacityRow>, Error> {
  // Hull supply uses different dogma attribute ids than a module's draw above: 11 = power output, 48 = CPU output.
  let row = sqlx::query_as::<_, HullCapacityRow>(
    "SELECT it.id AS id, \
      MAX(CASE WHEN CAST(json_extract(attr.value, '$.attribute_id') AS INTEGER) = 11 \
        THEN CAST(json_extract(attr.value, '$.value') AS REAL) END) AS power_output, \
      MAX(CASE WHEN CAST(json_extract(attr.value, '$.attribute_id') AS INTEGER) = 48 \
        THEN CAST(json_extract(attr.value, '$.value') AS REAL) END) AS cpu_output \
    FROM item_types it LEFT JOIN json_each(it.dogma_attributes) attr ON 1 = 1 \
    WHERE it.id = ? GROUP BY it.id",
  )
  .bind(hull_type_id)
  .fetch_optional(&db.0)
  .await?;
  Ok(row)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::store;

  async fn seed_type(db: &Database, id: i64, group_id: i64, name: &str, dogma: &str) {
    sqlx::query("INSERT OR IGNORE INTO item_categories (id, name, published) VALUES (65, 'Structure', 1)")
      .execute(db.writer())
      .await
      .unwrap();
    sqlx::query("INSERT OR IGNORE INTO item_groups (id, category_id, name, published) VALUES (?, 65, 'Grp', 1)")
      .bind(group_id)
      .execute(db.writer())
      .await
      .unwrap();
    sqlx::query(
      "INSERT INTO item_types (id, group_id, description, name, published, dogma_attributes) \
      VALUES (?, ?, '', ?, 1, ?)",
    )
    .bind(id)
    .bind(group_id)
    .bind(name)
    .bind(dogma)
    .execute(db.writer())
    .await
    .unwrap();
  }

  mod modules_by_names {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_empty_for_no_names() {
      let db = store::open_test().await.unwrap();

      let rows = super::modules_by_names(&db, &[]).await.unwrap();

      assert_eq!(rows.len(), 0);
    }

    #[tokio::test]
    async fn it_resolves_names_case_insensitively_with_pg_and_cpu() {
      let db = store::open_test().await.unwrap();
      seed_type(
        &db,
        35921,
        1327,
        "Standup Anticapital Missile Launcher I",
        r#"[{"attribute_id":30,"value":150000.0},{"attribute_id":50,"value":1500.0}]"#,
      )
      .await;

      let rows = super::modules_by_names(&db, &["standup anticapital missile launcher i".to_owned()])
        .await
        .unwrap();

      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].type_id, 35921);
      assert_eq!(rows[0].group_id, 1327);
      assert_eq!(rows[0].power, Some(150000.0));
      assert_eq!(rows[0].cpu, Some(1500.0));
      assert_eq!(rows[0].rig_size, None);
    }

    #[tokio::test]
    async fn it_omits_names_absent_from_the_sde() {
      let db = store::open_test().await.unwrap();
      seed_type(
        &db,
        35921,
        1327,
        "Known Module",
        r#"[{"attribute_id":30,"value":10.0}]"#,
      )
      .await;

      let rows = super::modules_by_names(&db, &["Known Module".to_owned(), "Totally Made Up Module".to_owned()])
        .await
        .unwrap();

      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].name, "Known Module");
    }

    #[tokio::test]
    async fn it_surfaces_rig_size_and_no_draw_for_rigs() {
      let db = store::open_test().await.unwrap();
      seed_type(
        &db,
        43920,
        1816,
        "Standup M-Set Equipment Manufacturing Material Efficiency I",
        r#"[{"attribute_id":1547,"value":2.0},{"attribute_id":1153,"value":100.0}]"#,
      )
      .await;

      let rows = super::modules_by_names(
        &db,
        &["Standup M-Set Equipment Manufacturing Material Efficiency I".to_owned()],
      )
      .await
      .unwrap();

      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].rig_size, Some(2.0));
      assert_eq!(rows[0].power, None);
      assert_eq!(rows[0].cpu, None);
    }

    #[tokio::test]
    async fn it_keeps_types_with_empty_dogma() {
      let db = store::open_test().await.unwrap();
      seed_type(&db, 56201, 4086, "Astrahus Upwell Quantum Core", "[]").await;

      let rows = super::modules_by_names(&db, &["Astrahus Upwell Quantum Core".to_owned()])
        .await
        .unwrap();

      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].type_id, 56201);
      assert_eq!(rows[0].group_id, 4086);
      assert_eq!(rows[0].power, None);
      assert_eq!(rows[0].cpu, None);
    }
  }

  mod modules_by_ids {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_only_requested_ids() {
      let db = store::open_test().await.unwrap();
      seed_type(&db, 35921, 1327, "A", r#"[{"attribute_id":30,"value":1.0}]"#).await;
      seed_type(&db, 35943, 1441, "B", r#"[{"attribute_id":50,"value":2.0}]"#).await;

      let mut rows = super::modules_by_ids(&db, &[35943]).await.unwrap();
      rows.sort_by_key(|r| r.type_id);

      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].type_id, 35943);
      assert_eq!(rows[0].cpu, Some(2.0));
    }
  }

  mod hull_capacity {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_power_and_cpu_output_for_a_hull() {
      let db = store::open_test().await.unwrap();
      seed_type(
        &db,
        35832,
        1657,
        "Astrahus",
        r#"[{"attribute_id":11,"value":1500000.0},{"attribute_id":48,"value":24000.0}]"#,
      )
      .await;

      let cap = super::hull_capacity(&db, 35832).await.unwrap().unwrap();

      assert_eq!(cap.power_output, Some(1500000.0));
      assert_eq!(cap.cpu_output, Some(24000.0));
    }

    #[tokio::test]
    async fn it_returns_none_for_an_unknown_hull() {
      let db = store::open_test().await.unwrap();

      let cap = super::hull_capacity(&db, 999999).await.unwrap();

      assert_eq!(cap, None);
    }
  }
}
