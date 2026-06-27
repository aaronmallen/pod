use std::collections::{HashMap, HashSet};

use sqlx::{QueryBuilder, Sqlite};

use crate::store::{
  Database, Error,
  asset_filter::{ColumnSchema, FilterContext, WhereClause, compile_query},
  model::{
    AbyssalCursor, AbyssalItem, AbyssalModuleStat, CharacterAsset, CorporationAbyssalItem, CorporationAsset,
    ENTITY_TYPE_ASSET, SavedAssetFilter, StatRange, StatTemplate, Stockpile, StockpileItem,
    abyssal_source_type_filter::SourceTypeFilter,
    asset_query::{
      AssetCompleteness, AssetRenderRow, GeoLocation, GeoLocationSql, InventoryCursor, InventoryQuery, InventoryRow,
      InventoryRowSql, InventoryTotals, NodeRollup, NodeRollupSql, ReferencedLocation, RenderRowSql, SortColumn,
      SortDirection, SortValue, TotalsRowSql,
    },
    stockpile_fill::{StockpileFill, StockpileItemFill, StockpileWithItems},
  },
  repo::{org, sde},
};

const RANGE_EPSILON: f64 = 1e-6;

pub async fn abyssal_type_ids(db: &Database) -> Result<Vec<i64>, Error> {
  let rows = sqlx::query_scalar::<_, i64>("SELECT DISTINCT abyssal_type_id FROM abyssal_module_stats")
    .fetch_all(&db.0)
    .await?;
  Ok(rows)
}

/// Count the abyssal items the page query would yield: the same [`page_for_characters`] WHERE clause
/// (character set, rolled-type, per-attribute stat ranges) minus the cursor/limit pagination.
pub async fn count_for_characters(
  db: &Database,
  character_ids: &[i64],
  source_type_id: Option<i64>,
  stat_ranges: &HashMap<i64, StatRange>,
) -> Result<i64, Error> {
  if character_ids.is_empty() {
    return Ok(0);
  }

  let mut builder = QueryBuilder::<Sqlite>::new("SELECT COUNT(*) FROM abyssal_items WHERE character_id IN (");
  let mut separated = builder.separated(", ");
  for id in character_ids {
    separated.push_bind(*id);
  }
  builder.push(")");

  if let Some(type_id) = source_type_id {
    builder.push(" AND type_id = ");
    builder.push_bind(type_id);
  }

  for (attribute_id, range) in stat_ranges {
    builder.push(
      " AND EXISTS (SELECT 1 FROM json_each(abyssal_items.dogma_attributes) je \
      WHERE json_extract(je.value, '$.attribute_id') = ",
    );
    builder.push_bind(*attribute_id);
    builder.push(" AND json_extract(je.value, '$.value') >= ");
    builder.push_bind(range.min - RANGE_EPSILON);
    builder.push(" AND json_extract(je.value, '$.value') <= ");
    builder.push_bind(range.max + RANGE_EPSILON);
    builder.push(")");
  }

  let count = builder.build_query_scalar::<i64>().fetch_one(&db.0).await?;
  Ok(count)
}

pub async fn delete_stale(db: &Database, character_id: i64, keep_ids: &[i64]) -> Result<(), Error> {
  if keep_ids.is_empty() {
    sqlx::query("DELETE FROM abyssal_items WHERE character_id = ?")
      .bind(character_id)
      .execute(db.writer())
      .await?;
    return Ok(());
  }

  let mut builder = QueryBuilder::<Sqlite>::new("DELETE FROM abyssal_items WHERE character_id = ");
  builder.push_bind(character_id);
  builder.push(" AND item_id NOT IN (");
  let mut separated = builder.separated(", ");
  for id in keep_ids {
    separated.push_bind(*id);
  }
  separated.push_unseparated(")");
  builder.build().execute(db.writer()).await?;
  Ok(())
}

pub async fn for_character_abyssal(db: &Database, character_id: i64) -> Result<Vec<AbyssalItem>, Error> {
  let rows = sqlx::query_as::<_, AbyssalItem>(
    "SELECT character_id, dogma_attributes, item_id, muta_price_isk, muta_price_synced, mutator_type_id, \
    source_type_id, synced_at, type_id FROM abyssal_items WHERE character_id = ? ORDER BY item_id",
  )
  .bind(character_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
pub async fn filtered_for_characters(
  db: &Database,
  character_ids: &[i64],
  source_type_id: Option<i64>,
  stat_ranges: &HashMap<i64, StatRange>,
) -> Result<Vec<AbyssalItem>, Error> {
  page_for_characters(db, character_ids, source_type_id, stat_ranges, None, None).await
}

/// Fetch one cursor-delimited page of abyssal items for the given characters.
///
/// Pass `cursor: None` for the first page and `limit: None` for the unbounded
/// full set (the in-memory fallback path / tests). The shared filter clauses
/// (rolled-type and per-attribute stat ranges) match [`filtered_for_characters`].
pub async fn page_for_characters(
  db: &Database,
  character_ids: &[i64],
  source_type_id: Option<i64>,
  stat_ranges: &HashMap<i64, StatRange>,
  cursor: Option<AbyssalCursor>,
  limit: Option<i64>,
) -> Result<Vec<AbyssalItem>, Error> {
  if character_ids.is_empty() {
    return Ok(Vec::new());
  }

  let mut builder = QueryBuilder::<Sqlite>::new(
    "SELECT character_id, dogma_attributes, item_id, muta_price_isk, muta_price_synced, mutator_type_id, \
    source_type_id, synced_at, type_id FROM abyssal_items WHERE character_id IN (",
  );
  let mut separated = builder.separated(", ");
  for id in character_ids {
    separated.push_bind(*id);
  }
  builder.push(")");

  if let Some(type_id) = source_type_id {
    builder.push(" AND type_id = ");
    builder.push_bind(type_id);
  }

  for (attribute_id, range) in stat_ranges {
    builder.push(
      " AND EXISTS (SELECT 1 FROM json_each(abyssal_items.dogma_attributes) je \
      WHERE json_extract(je.value, '$.attribute_id') = ",
    );
    builder.push_bind(*attribute_id);
    builder.push(" AND json_extract(je.value, '$.value') >= ");
    builder.push_bind(range.min - RANGE_EPSILON);
    builder.push(" AND json_extract(je.value, '$.value') <= ");
    builder.push_bind(range.max + RANGE_EPSILON);
    builder.push(")");
  }

  // Keyset predicate: resume strictly after the cursor's (source_type_id, item_id).
  if let Some(cursor) = cursor {
    builder.push(" AND (source_type_id > ");
    builder.push_bind(cursor.source_type_id);
    builder.push(" OR (source_type_id = ");
    builder.push_bind(cursor.source_type_id);
    builder.push(" AND item_id > ");
    builder.push_bind(cursor.item_id);
    builder.push("))");
  }

  builder.push(" ORDER BY source_type_id, item_id");

  if let Some(limit) = limit {
    builder.push(" LIMIT ");
    builder.push_bind(limit);
  }

  let rows = builder.build_query_as::<AbyssalItem>().fetch_all(&db.0).await?;
  Ok(rows)
}

pub async fn module_stats_for_type(db: &Database, abyssal_type_id: i64) -> Result<Vec<AbyssalModuleStat>, Error> {
  let rows = sqlx::query_as::<_, AbyssalModuleStat>(
    "SELECT abyssal_type_id, attribute_id, max_mult, min_mult FROM abyssal_module_stats \
    WHERE abyssal_type_id = ? ORDER BY attribute_id",
  )
  .bind(abyssal_type_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
pub async fn stat_templates_for_type(db: &Database, type_id: i64) -> Result<Vec<StatTemplate>, Error> {
  build_stat_templates(db, type_id, type_id).await
}

pub async fn stat_templates_for_owned_type(
  db: &Database,
  character_ids: &[i64],
  type_id: i64,
) -> Result<Vec<StatTemplate>, Error> {
  let base_type_id = representative_source_type(db, character_ids, type_id)
    .await?
    .unwrap_or(type_id);
  build_stat_templates(db, type_id, base_type_id).await
}

async fn build_stat_templates(db: &Database, type_id: i64, base_type_id: i64) -> Result<Vec<StatTemplate>, Error> {
  let stats = module_stats_for_type(db, type_id).await?;
  if stats.is_empty() {
    return Ok(Vec::new());
  }

  let base_values = base_dogma_values(db, base_type_id).await?;
  let attribute_ids: Vec<i64> = stats.iter().map(AbyssalModuleStat::attribute_id).collect();
  let metadata: HashMap<i64, crate::store::model::DogmaAttribute> = sde::get_dogma_attributes(db, &attribute_ids)
    .await?
    .into_iter()
    .map(|attr| (attr.attribute_id(), attr))
    .collect();

  let mut templates: Vec<StatTemplate> = stats
    .iter()
    .map(|stat| {
      let attribute_id = stat.attribute_id();
      let base_value = base_values.get(&attribute_id).copied().unwrap_or(0.0);
      let lo = base_value * stat.min_mult();
      let hi = base_value * stat.max_mult();
      let meta = metadata.get(&attribute_id);
      StatTemplate {
        attribute_id,
        base_value,
        bound_hi: lo.max(hi),
        bound_lo: lo.min(hi),
        display_name: meta
          .and_then(|m| m.display_name().clone())
          .or_else(|| meta.map(|m| m.name().to_owned()))
          .unwrap_or_else(|| format!("Attr {attribute_id}")),
        high_is_good: meta
          .map(crate::store::model::DogmaAttribute::high_is_good)
          .unwrap_or(true),
        unit_id: meta.and_then(crate::store::model::DogmaAttribute::unit_id),
      }
    })
    .collect();
  templates.sort_by(|a, b| a.display_name.cmp(&b.display_name));
  Ok(templates)
}

async fn base_dogma_values(db: &Database, type_id: i64) -> Result<HashMap<i64, f64>, Error> {
  let Some(item_type) = sde::get_item_type(db, type_id).await? else {
    return Ok(HashMap::new());
  };

  #[derive(serde::Deserialize)]
  struct Entry {
    attribute_id: i64,
    value: f64,
  }

  Ok(
    serde_json::from_str::<Vec<Entry>>(item_type.dogma_attributes())
      .unwrap_or_default()
      .into_iter()
      .map(|entry| (entry.attribute_id, entry.value))
      .collect(),
  )
}

async fn representative_source_type(db: &Database, character_ids: &[i64], type_id: i64) -> Result<Option<i64>, Error> {
  if character_ids.is_empty() {
    return Ok(None);
  }

  let mut builder = QueryBuilder::<Sqlite>::new("SELECT source_type_id FROM abyssal_items WHERE type_id = ");
  builder.push_bind(type_id);
  builder.push(" AND character_id IN (");
  let mut separated = builder.separated(", ");
  for id in character_ids {
    separated.push_bind(*id);
  }
  builder.push(") LIMIT 1");

  let row = builder.build_query_scalar::<i64>().fetch_optional(&db.0).await?;
  Ok(row)
}

pub async fn locations_for_items(db: &Database, item_ids: &[i64]) -> Result<HashMap<i64, String>, Error> {
  if item_ids.is_empty() {
    return Ok(HashMap::new());
  }

  let mut builder = QueryBuilder::<Sqlite>::new(
    "WITH RECURSIVE loc(item_id, location_id, location_type) AS ( \
      SELECT item_id, location_id, location_type FROM character_assets WHERE item_id IN (",
  );
  let mut separated = builder.separated(", ");
  for id in item_ids {
    separated.push_bind(*id);
  }
  builder.push(
    ") \
      UNION ALL \
      SELECT loc.item_id, ca.location_id, ca.location_type \
      FROM loc JOIN character_assets ca ON ca.item_id = loc.location_id \
      WHERE loc.location_type = 'item' \
    ) \
    SELECT loc.item_id AS item_id, COALESCE(s.name, st.name, sys.name) AS location \
    FROM loc \
    LEFT JOIN stations s ON s.id = loc.location_id \
    LEFT JOIN structures st ON st.id = loc.location_id \
    LEFT JOIN solar_systems sys ON sys.id = loc.location_id \
    WHERE loc.location_type <> 'item'",
  );

  let rows = builder
    .build_query_as::<(i64, Option<String>)>()
    .fetch_all(&db.0)
    .await?;
  Ok(
    rows
      .into_iter()
      .filter_map(|(item_id, location)| location.map(|name| (item_id, name)))
      .collect(),
  )
}

pub async fn source_type_filters(db: &Database, character_ids: &[i64]) -> Result<Vec<SourceTypeFilter>, Error> {
  if character_ids.is_empty() {
    return Ok(Vec::new());
  }

  let mut builder = QueryBuilder::<Sqlite>::new(
    "SELECT DISTINCT COALESCE(cat.name, 'Other') AS category, src.id AS source_type_id, src.name AS source_type_name \
    FROM abyssal_items ai \
    JOIN item_types src ON src.id = ai.source_type_id \
    LEFT JOIN item_groups grp ON grp.id = src.group_id \
    LEFT JOIN item_categories cat ON cat.id = grp.category_id \
    WHERE ai.character_id IN (",
  );
  let mut separated = builder.separated(", ");
  for id in character_ids {
    separated.push_bind(*id);
  }
  separated.push_unseparated(") ");
  builder.push("ORDER BY category, source_type_name");

  let rows = builder.build_query_as::<SourceTypeFilter>().fetch_all(&db.0).await?;
  Ok(rows)
}

pub async fn update_price(db: &Database, item_id: i64, price_isk: Option<f64>, synced_at: i64) -> Result<(), Error> {
  sqlx::query("UPDATE abyssal_items SET muta_price_isk = ?, muta_price_synced = ? WHERE item_id = ?")
    .bind(price_isk)
    .bind(synced_at)
    .bind(item_id)
    .execute(db.writer())
    .await?;
  Ok(())
}

pub async fn upsert(db: &Database, item: &AbyssalItem) -> Result<(), Error> {
  sqlx::query(
    "INSERT INTO abyssal_items \
      (item_id, character_id, type_id, source_type_id, mutator_type_id, dogma_attributes, synced_at, \
      muta_price_isk, muta_price_synced) \
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) \
    ON CONFLICT(item_id) DO UPDATE SET \
      character_id      = excluded.character_id, \
      type_id           = excluded.type_id, \
      source_type_id    = excluded.source_type_id, \
      mutator_type_id   = excluded.mutator_type_id, \
      dogma_attributes  = excluded.dogma_attributes, \
      synced_at         = excluded.synced_at, \
      muta_price_isk    = excluded.muta_price_isk, \
      muta_price_synced = excluded.muta_price_synced",
  )
  .bind(item.item_id())
  .bind(item.character_id())
  .bind(item.type_id())
  .bind(item.source_type_id())
  .bind(item.mutator_type_id())
  .bind(item.dogma_attributes())
  .bind(item.synced_at())
  .bind(item.muta_price_isk())
  .bind(item.muta_price_synced())
  .execute(db.writer())
  .await?;
  Ok(())
}

pub async fn delete_stale_corporation(db: &Database, corporation_id: i64, keep_ids: &[i64]) -> Result<(), Error> {
  if keep_ids.is_empty() {
    sqlx::query("DELETE FROM corporation_abyssal_items WHERE corporation_id = ?")
      .bind(corporation_id)
      .execute(db.writer())
      .await?;
    return Ok(());
  }

  let mut builder = QueryBuilder::<Sqlite>::new("DELETE FROM corporation_abyssal_items WHERE corporation_id = ");
  builder.push_bind(corporation_id);
  builder.push(" AND item_id NOT IN (");
  let mut separated = builder.separated(", ");
  for id in keep_ids {
    separated.push_bind(*id);
  }
  separated.push_unseparated(")");
  builder.build().execute(db.writer()).await?;
  Ok(())
}

pub async fn for_corporation_abyssal(db: &Database, corporation_id: i64) -> Result<Vec<CorporationAbyssalItem>, Error> {
  let rows = sqlx::query_as::<_, CorporationAbyssalItem>(
    "SELECT corporation_id, dogma_attributes, item_id, muta_price_isk, muta_price_synced, mutator_type_id, \
    source_type_id, synced_at, type_id FROM corporation_abyssal_items WHERE corporation_id = ? ORDER BY item_id",
  )
  .bind(corporation_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn update_price_corporation(
  db: &Database,
  item_id: i64,
  price_isk: Option<f64>,
  synced_at: i64,
) -> Result<(), Error> {
  sqlx::query("UPDATE corporation_abyssal_items SET muta_price_isk = ?, muta_price_synced = ? WHERE item_id = ?")
    .bind(price_isk)
    .bind(synced_at)
    .bind(item_id)
    .execute(db.writer())
    .await?;
  Ok(())
}

pub async fn upsert_corporation(db: &Database, item: &CorporationAbyssalItem) -> Result<(), Error> {
  sqlx::query(
    "INSERT INTO corporation_abyssal_items \
      (item_id, corporation_id, type_id, source_type_id, mutator_type_id, dogma_attributes, synced_at, \
      muta_price_isk, muta_price_synced) \
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) \
    ON CONFLICT(item_id) DO UPDATE SET \
      corporation_id    = excluded.corporation_id, \
      type_id           = excluded.type_id, \
      source_type_id    = excluded.source_type_id, \
      mutator_type_id   = excluded.mutator_type_id, \
      dogma_attributes  = excluded.dogma_attributes, \
      synced_at         = excluded.synced_at, \
      muta_price_isk    = excluded.muta_price_isk, \
      muta_price_synced = excluded.muta_price_synced",
  )
  .bind(item.item_id())
  .bind(item.corporation_id())
  .bind(item.type_id())
  .bind(item.source_type_id())
  .bind(item.mutator_type_id())
  .bind(item.dogma_attributes())
  .bind(item.synced_at())
  .bind(item.muta_price_isk())
  .bind(item.muta_price_synced())
  .execute(db.writer())
  .await?;
  Ok(())
}

pub async fn upsert_module_stats(db: &Database, stats: &[AbyssalModuleStat]) -> Result<(), Error> {
  let mut tx = db.writer().begin().await?;

  for stat in stats {
    sqlx::query(
      "INSERT INTO abyssal_module_stats (abyssal_type_id, attribute_id, min_mult, max_mult) \
      VALUES (?, ?, ?, ?) \
      ON CONFLICT(abyssal_type_id, attribute_id) DO UPDATE SET \
        min_mult = excluded.min_mult, \
        max_mult = excluded.max_mult",
    )
    .bind(stat.abyssal_type_id())
    .bind(stat.attribute_id())
    .bind(stat.min_mult())
    .bind(stat.max_mult())
    .execute(&mut *tx)
    .await?;
  }

  tx.commit().await?;
  Ok(())
}

async fn corp_scope_visible(db: &Database, corporation_id: i64) -> Result<bool, Error> {
  org::corp_is_authorized(db, corporation_id).await
}

async fn authorized_corporation_ids(db: &Database, corporation_ids: &[i64]) -> Result<Vec<i64>, Error> {
  let mut visible = Vec::new();
  for &corporation_id in corporation_ids {
    if corp_scope_visible(db, corporation_id).await? {
      visible.push(corporation_id);
    }
  }
  Ok(visible)
}

const ASSET_WRITE_BATCH_SIZE: usize = 500;

pub async fn replace_for_character(db: &Database, character_id: i64, assets: &[CharacterAsset]) -> Result<(), Error> {
  replace_for_character_batched(db, character_id, assets, ASSET_WRITE_BATCH_SIZE).await
}

/// Reconciles a character's assets to `assets`, committing in batches rather than one atomic transaction.
///
/// Upserting the new set before pruning stale ids (instead of deleting all first) and committing each batch
/// releases SQLite's single write lock between batches so interactive writes can interleave. The cost is that a
/// concurrent reader may transiently observe a superset (a stale row not yet pruned) but never a missing current
/// row; the final state is identical to a delete-all-then-insert-all replace.
async fn replace_for_character_batched(
  db: &Database,
  character_id: i64,
  assets: &[CharacterAsset],
  batch_size: usize,
) -> Result<(), Error> {
  let new_ids: HashSet<i64> = assets.iter().map(CharacterAsset::item_id).collect();
  let existing: Vec<i64> = sqlx::query_scalar("SELECT item_id FROM character_assets WHERE character_id = ?")
    .bind(character_id)
    .fetch_all(&db.0)
    .await?;
  let stale: Vec<i64> = existing.into_iter().filter(|id| !new_ids.contains(id)).collect();

  let batch_size = batch_size.max(1);
  for chunk in assets.chunks(batch_size) {
    let mut tx = db.writer().begin().await?;
    for asset in chunk {
      insert_character_asset(&mut tx, asset).await?;
    }
    tx.commit().await?;
    tokio::task::yield_now().await;
  }
  for chunk in stale.chunks(batch_size) {
    delete_character_assets(db, character_id, chunk).await?;
    tokio::task::yield_now().await;
  }
  Ok(())
}

pub async fn replace_for_corporation(
  db: &Database,
  corporation_id: i64,
  assets: &[CorporationAsset],
) -> Result<(), Error> {
  replace_for_corporation_batched(db, corporation_id, assets, ASSET_WRITE_BATCH_SIZE).await
}

async fn replace_for_corporation_batched(
  db: &Database,
  corporation_id: i64,
  assets: &[CorporationAsset],
  batch_size: usize,
) -> Result<(), Error> {
  let new_ids: HashSet<i64> = assets.iter().map(CorporationAsset::item_id).collect();
  let existing: Vec<i64> = sqlx::query_scalar("SELECT item_id FROM corporation_assets WHERE corporation_id = ?")
    .bind(corporation_id)
    .fetch_all(&db.0)
    .await?;
  let stale: Vec<i64> = existing.into_iter().filter(|id| !new_ids.contains(id)).collect();

  let batch_size = batch_size.max(1);
  for chunk in assets.chunks(batch_size) {
    let mut tx = db.writer().begin().await?;
    for asset in chunk {
      insert_corporation_asset(&mut tx, asset).await?;
    }
    tx.commit().await?;
    tokio::task::yield_now().await;
  }
  for chunk in stale.chunks(batch_size) {
    delete_corporation_assets(db, corporation_id, chunk).await?;
    tokio::task::yield_now().await;
  }
  Ok(())
}

async fn delete_character_assets(db: &Database, character_id: i64, item_ids: &[i64]) -> Result<(), Error> {
  if item_ids.is_empty() {
    return Ok(());
  }
  let mut tx = db.writer().begin().await?;
  let mut builder = QueryBuilder::<Sqlite>::new("DELETE FROM character_assets WHERE character_id = ");
  builder.push_bind(character_id);
  builder.push(" AND item_id IN (");
  let mut separated = builder.separated(", ");
  for id in item_ids {
    separated.push_bind(*id);
  }
  builder.push(")");
  builder.build().execute(&mut *tx).await?;
  delete_asset_tag_memberships(&mut tx, item_ids).await?;
  tx.commit().await?;
  Ok(())
}

async fn delete_corporation_assets(db: &Database, corporation_id: i64, item_ids: &[i64]) -> Result<(), Error> {
  if item_ids.is_empty() {
    return Ok(());
  }
  let mut tx = db.writer().begin().await?;
  let mut builder = QueryBuilder::<Sqlite>::new("DELETE FROM corporation_assets WHERE corporation_id = ");
  builder.push_bind(corporation_id);
  builder.push(" AND item_id IN (");
  let mut separated = builder.separated(", ");
  for id in item_ids {
    separated.push_bind(*id);
  }
  builder.push(")");
  builder.build().execute(&mut *tx).await?;
  delete_asset_tag_memberships(&mut tx, item_ids).await?;
  tx.commit().await?;
  Ok(())
}

// entity_tags has no foreign key to the asset tables, so a stale item's ('asset', item_id) tag rows would be
// orphaned forever once the asset row is pruned. Delete them in the same transaction that removes the assets,
// scoped strictly to entity_type = 'asset' so character/corporation memberships sharing an id are untouched.
async fn delete_asset_tag_memberships(tx: &mut sqlx::Transaction<'_, Sqlite>, item_ids: &[i64]) -> Result<(), Error> {
  if item_ids.is_empty() {
    return Ok(());
  }
  let mut builder = QueryBuilder::<Sqlite>::new("DELETE FROM entity_tags WHERE entity_type = ");
  builder.push_bind(ENTITY_TYPE_ASSET);
  builder.push(" AND entity_id IN (");
  let mut separated = builder.separated(", ");
  for id in item_ids {
    separated.push_bind(*id);
  }
  builder.push(")");
  builder.build().execute(&mut **tx).await?;
  Ok(())
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
pub async fn upsert_character_asset(db: &Database, asset: &CharacterAsset) -> Result<(), Error> {
  let mut tx = db.writer().begin().await?;
  sqlx::query("DELETE FROM character_assets WHERE item_id = ?")
    .bind(asset.item_id())
    .execute(&mut *tx)
    .await?;
  insert_character_asset(&mut tx, asset).await?;
  tx.commit().await?;
  Ok(())
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
pub async fn upsert_corporation_asset(db: &Database, asset: &CorporationAsset) -> Result<(), Error> {
  let mut tx = db.writer().begin().await?;
  sqlx::query("DELETE FROM corporation_assets WHERE item_id = ?")
    .bind(asset.item_id())
    .execute(&mut *tx)
    .await?;
  insert_corporation_asset(&mut tx, asset).await?;
  tx.commit().await?;
  Ok(())
}

pub async fn for_character(db: &Database, character_id: i64) -> Result<Vec<CharacterAsset>, Error> {
  let rows = sqlx::query_as::<_, CharacterAsset>(
    "SELECT character_id, container_id, depth, is_active_ship, is_blueprint_copy, is_container, is_singleton, \
    item_id, location_flag, location_id, location_type, name, quantity, type_id FROM character_assets \
    WHERE character_id = ? ORDER BY item_id",
  )
  .bind(character_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
pub async fn for_corporation(db: &Database, corporation_id: i64) -> Result<Vec<CorporationAsset>, Error> {
  if !corp_scope_visible(db, corporation_id).await? {
    return Ok(Vec::new());
  }
  let rows = sqlx::query_as::<_, CorporationAsset>(
    "SELECT container_id, corporation_id, depth, is_blueprint_copy, is_container, is_singleton, item_id, \
    location_flag, location_id, location_type, name, quantity, type_id FROM corporation_assets \
    WHERE corporation_id = ? ORDER BY item_id",
  )
  .bind(corporation_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
pub async fn children_for_character(
  db: &Database,
  character_id: i64,
  container_id: i64,
) -> Result<Vec<CharacterAsset>, Error> {
  let rows = sqlx::query_as::<_, CharacterAsset>(
    "SELECT character_id, container_id, depth, is_active_ship, is_blueprint_copy, is_container, is_singleton, \
    item_id, location_flag, location_id, location_type, name, quantity, type_id FROM character_assets \
    WHERE character_id = ? AND container_id = ? ORDER BY item_id",
  )
  .bind(character_id)
  .bind(container_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
pub async fn children_for_corporation(
  db: &Database,
  corporation_id: i64,
  container_id: i64,
) -> Result<Vec<CorporationAsset>, Error> {
  if !corp_scope_visible(db, corporation_id).await? {
    return Ok(Vec::new());
  }
  let rows = sqlx::query_as::<_, CorporationAsset>(
    "SELECT container_id, corporation_id, depth, is_blueprint_copy, is_container, is_singleton, item_id, \
    location_flag, location_id, location_type, name, quantity, type_id FROM corporation_assets \
    WHERE corporation_id = ? AND container_id = ? ORDER BY item_id",
  )
  .bind(corporation_id)
  .bind(container_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
pub async fn roots_for_character(db: &Database, character_id: i64) -> Result<Vec<CharacterAsset>, Error> {
  let rows = sqlx::query_as::<_, CharacterAsset>(
    "SELECT character_id, container_id, depth, is_active_ship, is_blueprint_copy, is_container, is_singleton, \
    item_id, location_flag, location_id, location_type, name, quantity, type_id FROM character_assets \
    WHERE character_id = ? AND container_id IS NULL ORDER BY item_id",
  )
  .bind(character_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
pub async fn roots_for_characters(db: &Database, character_ids: &[i64]) -> Result<Vec<CharacterAsset>, Error> {
  if character_ids.is_empty() {
    return Ok(Vec::new());
  }
  let mut builder = QueryBuilder::<Sqlite>::new(
    "SELECT character_id, container_id, depth, is_active_ship, is_blueprint_copy, is_container, is_singleton, \
    item_id, location_flag, location_id, location_type, name, quantity, type_id FROM character_assets \
    WHERE character_id ",
  );
  push_owner_predicate(&mut builder, character_ids);
  builder.push(" AND container_id IS NULL ORDER BY item_id");
  let rows = builder.build_query_as::<CharacterAsset>().fetch_all(&db.0).await?;
  Ok(rows)
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
pub async fn roots_for_corporation(db: &Database, corporation_id: i64) -> Result<Vec<CorporationAsset>, Error> {
  if !corp_scope_visible(db, corporation_id).await? {
    return Ok(Vec::new());
  }
  let rows = sqlx::query_as::<_, CorporationAsset>(
    "SELECT container_id, corporation_id, depth, is_blueprint_copy, is_container, is_singleton, item_id, \
    location_flag, location_id, location_type, name, quantity, type_id FROM corporation_assets \
    WHERE corporation_id = ? AND container_id IS NULL ORDER BY item_id",
  )
  .bind(corporation_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
pub async fn count_for_character(db: &Database, character_id: i64) -> Result<i64, Error> {
  let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM character_assets WHERE character_id = ?")
    .bind(character_id)
    .fetch_one(&db.0)
    .await?;
  Ok(count)
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
pub async fn count_for_corporation(db: &Database, corporation_id: i64) -> Result<i64, Error> {
  if !corp_scope_visible(db, corporation_id).await? {
    return Ok(0);
  }
  let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM corporation_assets WHERE corporation_id = ?")
    .bind(corporation_id)
    .fetch_one(&db.0)
    .await?;
  Ok(count)
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
pub async fn referenced_locations(db: &Database) -> Result<Vec<ReferencedLocation>, Error> {
  let rows = sqlx::query_as::<_, ReferencedLocation>(
    "SELECT location_id, location_type FROM character_assets WHERE location_type <> 'item' \
    UNION \
    SELECT location_id, location_type FROM corporation_assets WHERE location_type <> 'item' \
    ORDER BY location_id",
  )
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn geo_locations_for_character(db: &Database, character_id: i64) -> Result<Vec<GeoLocation>, Error> {
  geo_locations(db, "character_assets", "character_id", &[character_id]).await
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
pub async fn geo_locations_for_characters(db: &Database, character_ids: &[i64]) -> Result<Vec<GeoLocation>, Error> {
  if character_ids.is_empty() {
    return Ok(Vec::new());
  }
  geo_locations(db, "character_assets", "character_id", character_ids).await
}

pub async fn geo_locations_for_corporation(db: &Database, corporation_id: i64) -> Result<Vec<GeoLocation>, Error> {
  if !corp_scope_visible(db, corporation_id).await? {
    return Ok(Vec::new());
  }
  geo_locations(db, "corporation_assets", "corporation_id", &[corporation_id]).await
}

pub async fn geo_locations_for_combined(
  db: &Database,
  character_ids: &[i64],
  corporation_ids: &[i64],
) -> Result<Vec<GeoLocation>, Error> {
  let corporation_ids = authorized_corporation_ids(db, corporation_ids).await?;
  if character_ids.is_empty() && corporation_ids.is_empty() {
    return Ok(Vec::new());
  }
  combined_geo_locations(db, character_ids, &corporation_ids).await
}

macro_rules! category_key_case {
  () => {
    "CASE ic.name \
      WHEN 'Ship' THEN 'ship' \
      WHEN 'Module' THEN 'module' \
      WHEN 'Drone' THEN 'drone' \
      WHEN 'Charge' THEN 'charge' \
      WHEN 'Implant' THEN 'implant' \
      WHEN 'Augmentation' THEN 'implant' \
      WHEN 'Blueprint' THEN 'blueprint' \
      WHEN 'Material' THEN 'material' \
      WHEN 'Mineral' THEN 'material' \
      WHEN 'Skill' THEN 'book' \
      WHEN 'Skillbook' THEN 'book' \
      WHEN 'Commodity' THEN 'commodity' \
      WHEN 'Ancient Relics' THEN 'commodity' \
      ELSE 'commodity' \
    END"
  };
}

macro_rules! location_label_expr {
  () => {
    "CASE WHEN ina.id IS NOT NULL THEN 'Inaccessible Structure' ELSE COALESCE(s.name, st.name) END"
  };
}

macro_rules! location_join_sql {
  ($owner:literal, $owner_type:literal) => {
    concat!(
      " LEFT JOIN stations s ON s.id = a.location_id \
        LEFT JOIN structures st ON st.id = a.location_id \
        LEFT JOIN inaccessible_structures ina \
          ON ina.id = a.location_id AND ina.owner_id = a.",
      $owner,
      " AND ina.owner_type = '",
      $owner_type,
      "' "
    )
  };
}

macro_rules! geo_extra_join_sql {
  () => {
    " LEFT JOIN solar_systems sys \
        ON sys.id = CASE WHEN a.location_type = 'solar_system' THEN a.location_id \
                        ELSE COALESCE(s.system_id, st.solar_system_id) END \
      LEFT JOIN constellations con ON con.id = sys.constellation_id \
      LEFT JOIN regions reg ON reg.id = con.region_id "
  };
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
const RENDER_CHARACTER_SQL: &str = concat!(
  "SELECT a.item_id, a.type_id, a.quantity, a.location_id, a.location_flag, a.container_id, a.depth, a.is_container, \
    a.name AS name, it.name AS type_name, ig.name AS group_name, ",
  category_key_case!(),
  " AS category, it.icon_id AS icon_id, COALESCE(it.packaged_volume, it.volume) AS volume, ",
  location_label_expr!(),
  " AS location_label \
  FROM character_assets a \
  JOIN item_types it ON it.id = a.type_id \
  JOIN item_groups ig ON ig.id = it.group_id \
  JOIN item_categories ic ON ic.id = ig.category_id",
  location_join_sql!("character_id", "character"),
  "WHERE a.character_id = ? ORDER BY a.item_id"
);

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
const RENDER_CORPORATION_SQL: &str = concat!(
  "SELECT a.item_id, a.type_id, a.quantity, a.location_id, a.location_flag, a.container_id, a.depth, a.is_container, \
    a.name AS name, it.name AS type_name, ig.name AS group_name, ",
  category_key_case!(),
  " AS category, it.icon_id AS icon_id, COALESCE(it.packaged_volume, it.volume) AS volume, ",
  location_label_expr!(),
  " AS location_label \
  FROM corporation_assets a \
  JOIN item_types it ON it.id = a.type_id \
  JOIN item_groups ig ON ig.id = it.group_id \
  JOIN item_categories ic ON ic.id = ig.category_id",
  location_join_sql!("corporation_id", "corporation"),
  "WHERE a.corporation_id = ? ORDER BY a.item_id"
);

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
pub async fn render_for_character(db: &Database, character_id: i64) -> Result<Vec<AssetRenderRow>, Error> {
  let rows = sqlx::query_as::<_, RenderRowSql>(RENDER_CHARACTER_SQL)
    .bind(character_id)
    .fetch_all(&db.0)
    .await?;
  Ok(rows.into_iter().map(RenderRowSql::into_row).collect())
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
pub async fn render_for_corporation(db: &Database, corporation_id: i64) -> Result<Vec<AssetRenderRow>, Error> {
  if !corp_scope_visible(db, corporation_id).await? {
    return Ok(Vec::new());
  }
  let rows = sqlx::query_as::<_, RenderRowSql>(RENDER_CORPORATION_SQL)
    .bind(corporation_id)
    .fetch_all(&db.0)
    .await?;
  Ok(rows.into_iter().map(RenderRowSql::into_row).collect())
}

macro_rules! abyssal_join_sql {
  ($abyssal:literal) => {
    concat!(" LEFT JOIN ", $abyssal, " ab ON ab.item_id = a.item_id ")
  };
}

macro_rules! query_join_sql {
  ($table:literal, $owner:literal, $owner_type:literal, $abyssal:literal) => {
    concat!(
      "FROM ",
      $table,
      " a \
      JOIN item_types it ON it.id = a.type_id \
      JOIN item_groups ig ON ig.id = it.group_id \
      JOIN item_categories ic ON ic.id = ig.category_id \
      LEFT JOIN market_prices mp ON mp.type_id = a.type_id",
      abyssal_join_sql!($abyssal),
      location_join_sql!($owner, $owner_type),
      geo_extra_join_sql!(),
      "WHERE a.",
      $owner,
      " "
    )
  };
}

macro_rules! display_name_expr {
  () => {
    "COALESCE(a.name, it.name)"
  };
}
macro_rules! type_name_expr {
  () => {
    "it.name"
  };
}
macro_rules! group_name_expr {
  () => {
    "ig.name"
  };
}

macro_rules! row_volume_expr {
  () => {
    "CAST(COALESCE(it.packaged_volume, it.volume, 0) * a.quantity AS REAL)"
  };
}
macro_rules! unit_price_expr {
  () => {
    "CAST(CASE WHEN a.is_blueprint_copy = 1 THEN 0 ELSE COALESCE(ab.muta_price_isk, mp.adjusted_price, mp.average_price, 0) END AS REAL)"
  };
}
macro_rules! value_expr {
  () => {
    concat!("CAST(a.quantity * ", unit_price_expr!(), " AS REAL)")
  };
}
// Per-type sum of each output material's mineral value, priced at the same global ESI prices the
// inventory already uses (unpriced materials COALESCE to 0, undervaluing rather than false-positiving).
macro_rules! reproc_per_unit_expr {
  () => {
    "(SELECT COALESCE(SUM(tm.quantity * COALESCE(mpm.adjusted_price, mpm.average_price, 0)), 0) \
      FROM type_materials tm \
      LEFT JOIN market_prices mpm ON mpm.type_id = tm.material_type_id \
      WHERE tm.type_id = a.type_id)"
  };
}
// `{reproc_yield}` is replaced at runtime with the configured flat refine yield (a controlled f64).
// Blueprint copies and stacks smaller than one portion (or types without a portion size) yield 0.
macro_rules! reproc_value_expr {
  () => {
    concat!(
      "CAST(CASE WHEN a.is_blueprint_copy = 1 OR COALESCE(it.portion_size, 0) <= 0 THEN 0 ELSE ",
      reproc_per_unit_expr!(),
      " * {reproc_yield} * CAST(a.quantity / it.portion_size AS INTEGER) END AS REAL)"
    )
  };
}

const DISPLAY_NAME_EXPR: &str = display_name_expr!();
const TYPE_NAME_EXPR: &str = type_name_expr!();
const GROUP_NAME_EXPR: &str = group_name_expr!();
const ROW_VOLUME_EXPR: &str = row_volume_expr!();
const UNIT_PRICE_EXPR: &str = unit_price_expr!();
const VALUE_EXPR: &str = value_expr!();
const CATEGORY_KEY_EXPR: &str = category_key_case!();

fn render_column_schema(owner_column: &'static str) -> ColumnSchema {
  ColumnSchema {
    category: CATEGORY_KEY_EXPR,
    character_id: owner_column,
    constellation_name: "con.name",
    group_name: GROUP_NAME_EXPR,
    is_blueprint_copy: "a.is_blueprint_copy",
    is_singleton: "a.is_singleton",
    item_id: "a.item_id",
    location_name: location_label_expr!(),
    name: "a.name",
    region_name: "reg.name",
    system_name: "sys.name",
    type_name: TYPE_NAME_EXPR,
  }
}

fn sort_column_expr(sort: SortColumn, owner_column: &'static str) -> &'static str {
  match sort {
    SortColumn::Category => CATEGORY_KEY_EXPR,
    SortColumn::Group => GROUP_NAME_EXPR,
    SortColumn::Name => DISPLAY_NAME_EXPR,
    SortColumn::Owner => owner_column,
    SortColumn::Quantity => "a.quantity",
    SortColumn::UnitPrice => UNIT_PRICE_EXPR,
    SortColumn::Value => VALUE_EXPR,
    SortColumn::Volume => ROW_VOLUME_EXPR,
  }
}

fn direction_keyword(direction: SortDirection) -> &'static str {
  match direction {
    SortDirection::Ascending => "ASC",
    SortDirection::Descending => "DESC",
  }
}

fn seek_operator(direction: SortDirection) -> &'static str {
  match direction {
    SortDirection::Ascending => ">",
    SortDirection::Descending => "<",
  }
}

pub async fn inventory_page_for_character(
  db: &Database,
  character_id: i64,
  query: &InventoryQuery<'_>,
) -> Result<Vec<InventoryRow>, Error> {
  inventory_page(db, "character_assets", "character_id", &[character_id], query).await
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
pub async fn inventory_page_for_characters(
  db: &Database,
  character_ids: &[i64],
  query: &InventoryQuery<'_>,
) -> Result<Vec<InventoryRow>, Error> {
  if character_ids.is_empty() {
    return Ok(Vec::new());
  }
  inventory_page(db, "character_assets", "character_id", character_ids, query).await
}

pub async fn inventory_page_for_corporation(
  db: &Database,
  corporation_id: i64,
  query: &InventoryQuery<'_>,
) -> Result<Vec<InventoryRow>, Error> {
  if !corp_scope_visible(db, corporation_id).await? {
    return Ok(Vec::new());
  }
  inventory_page(db, "corporation_assets", "corporation_id", &[corporation_id], query).await
}

pub async fn inventory_page_for_combined(
  db: &Database,
  character_ids: &[i64],
  corporation_ids: &[i64],
  query: &InventoryQuery<'_>,
) -> Result<Vec<InventoryRow>, Error> {
  let corporation_ids = authorized_corporation_ids(db, corporation_ids).await?;
  if character_ids.is_empty() && corporation_ids.is_empty() {
    return Ok(Vec::new());
  }
  combined_inventory_page(db, character_ids, &corporation_ids, query).await
}

pub async fn inventory_totals_for_character(
  db: &Database,
  character_id: i64,
  filter: &str,
  location_ids: &[i64],
  me_id: Option<i64>,
) -> Result<InventoryTotals, Error> {
  inventory_totals(
    db,
    "character_assets",
    "character_id",
    &[character_id],
    filter,
    location_ids,
    me_id,
  )
  .await
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
pub async fn inventory_totals_for_characters(
  db: &Database,
  character_ids: &[i64],
  filter: &str,
  location_ids: &[i64],
  me_id: Option<i64>,
) -> Result<InventoryTotals, Error> {
  if character_ids.is_empty() {
    return Ok(InventoryTotals::default());
  }
  inventory_totals(
    db,
    "character_assets",
    "character_id",
    character_ids,
    filter,
    location_ids,
    me_id,
  )
  .await
}

pub async fn inventory_totals_for_corporation(
  db: &Database,
  corporation_id: i64,
  filter: &str,
  location_ids: &[i64],
  me_id: Option<i64>,
) -> Result<InventoryTotals, Error> {
  if !corp_scope_visible(db, corporation_id).await? {
    return Ok(InventoryTotals::default());
  }
  inventory_totals(
    db,
    "corporation_assets",
    "corporation_id",
    &[corporation_id],
    filter,
    location_ids,
    me_id,
  )
  .await
}

pub async fn inventory_totals_for_combined(
  db: &Database,
  character_ids: &[i64],
  corporation_ids: &[i64],
  filter: &str,
  location_ids: &[i64],
  me_id: Option<i64>,
) -> Result<InventoryTotals, Error> {
  let corporation_ids = authorized_corporation_ids(db, corporation_ids).await?;
  if character_ids.is_empty() && corporation_ids.is_empty() {
    return Ok(InventoryTotals::default());
  }
  combined_inventory_totals(db, character_ids, &corporation_ids, filter, location_ids, me_id).await
}

/// Sums on-hand quantity per `(location_id, type_id)` for items sitting directly in a build-site hangar.
///
/// "Directly in the hangar" is the codebase's top-level marker `container_id IS NULL` (an item nested in a
/// container or ship carries its parent's `item_id` as `container_id`), intersected with the requested build-site
/// `location_id`s. Character assets are always counted; corporation assets only for authorized corporations.
pub async fn on_hand_at_build_sites(db: &Database, location_ids: &[i64]) -> Result<HashMap<(i64, i64), i64>, Error> {
  if location_ids.is_empty() {
    return Ok(HashMap::new());
  }

  let mut totals: HashMap<(i64, i64), i64> = HashMap::new();
  accumulate_on_hand(db, "character_assets", location_ids, None, &mut totals).await?;

  let corporation_ids = corporations_with_assets_at(db, location_ids).await?;
  for corporation_id in authorized_corporation_ids(db, &corporation_ids).await? {
    accumulate_on_hand(
      db,
      "corporation_assets",
      location_ids,
      Some(("corporation_id", corporation_id)),
      &mut totals,
    )
    .await?;
  }

  Ok(totals)
}

macro_rules! historical_unit_price_expr {
  () => {
    "CASE \
      WHEN a.is_blueprint_copy = 1 THEN 0 \
      ELSE COALESCE( \
        ab.muta_price_isk, \
        (SELECT h.close FROM type_price_histories h \
            WHERE h.type_id = a.type_id AND h.date <= ? ORDER BY h.date DESC LIMIT 1), \
        mp.adjusted_price, mp.average_price, 0) \
    END"
  };
}

macro_rules! asset_value_as_of_sql {
  ($table:literal, $owner:literal, $abyssal:literal) => {
    concat!(
      "SELECT CAST(COALESCE(SUM(a.quantity * ",
      historical_unit_price_expr!(),
      "), 0) AS REAL) FROM ",
      $table,
      " a LEFT JOIN market_prices mp ON mp.type_id = a.type_id",
      abyssal_join_sql!($abyssal),
      "WHERE a.",
      $owner,
      " = ?"
    )
  };
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
const ASSET_VALUE_AS_OF_CHARACTER: &str = asset_value_as_of_sql!("character_assets", "character_id", "abyssal_items");
// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
const ASSET_VALUE_AS_OF_CORPORATION: &str =
  asset_value_as_of_sql!("corporation_assets", "corporation_id", "corporation_abyssal_items");

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
pub async fn asset_value_as_of_for_character(db: &Database, character_id: i64, date: &str) -> Result<f64, Error> {
  asset_value_as_of(db, ASSET_VALUE_AS_OF_CHARACTER, character_id, date).await
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
pub async fn asset_value_as_of_for_corporation(db: &Database, corporation_id: i64, date: &str) -> Result<f64, Error> {
  if !corp_scope_visible(db, corporation_id).await? {
    return Ok(0.0);
  }
  asset_value_as_of(db, ASSET_VALUE_AS_OF_CORPORATION, corporation_id, date).await
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
async fn asset_value_as_of(db: &Database, sql: &'static str, owner_id: i64, date: &str) -> Result<f64, Error> {
  let value = sqlx::query_scalar::<_, f64>(sql)
    .bind(date)
    .bind(owner_id)
    .fetch_one(&db.0)
    .await?;
  Ok(value)
}

pub async fn children_render_for_character(
  db: &Database,
  character_id: i64,
  container_id: i64,
  reproc_yield: f64,
) -> Result<Vec<InventoryRow>, Error> {
  children_render(
    db,
    "character_assets",
    "character_id",
    &[character_id],
    container_id,
    reproc_yield,
  )
  .await
}

pub async fn children_render_for_characters(
  db: &Database,
  character_ids: &[i64],
  container_id: i64,
  reproc_yield: f64,
) -> Result<Vec<InventoryRow>, Error> {
  if character_ids.is_empty() {
    return Ok(Vec::new());
  }
  children_render(
    db,
    "character_assets",
    "character_id",
    character_ids,
    container_id,
    reproc_yield,
  )
  .await
}

pub async fn children_render_for_corporation(
  db: &Database,
  corporation_id: i64,
  container_id: i64,
  reproc_yield: f64,
) -> Result<Vec<InventoryRow>, Error> {
  if !corp_scope_visible(db, corporation_id).await? {
    return Ok(Vec::new());
  }
  children_render(
    db,
    "corporation_assets",
    "corporation_id",
    &[corporation_id],
    container_id,
    reproc_yield,
  )
  .await
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
pub async fn child_count_for_character(db: &Database, character_id: i64, container_id: i64) -> Result<i64, Error> {
  child_count(db, "character_assets", "character_id", &[character_id], container_id).await
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
pub async fn child_count_for_characters(db: &Database, character_ids: &[i64], container_id: i64) -> Result<i64, Error> {
  if character_ids.is_empty() {
    return Ok(0);
  }
  child_count(db, "character_assets", "character_id", character_ids, container_id).await
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
pub async fn child_count_for_corporation(db: &Database, corporation_id: i64, container_id: i64) -> Result<i64, Error> {
  if !corp_scope_visible(db, corporation_id).await? {
    return Ok(0);
  }
  child_count(
    db,
    "corporation_assets",
    "corporation_id",
    &[corporation_id],
    container_id,
  )
  .await
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
pub async fn node_rollup_for_character(
  db: &Database,
  character_id: i64,
  container_id: i64,
) -> Result<NodeRollup, Error> {
  node_rollup(db, "character_assets", "character_id", &[character_id], container_id).await
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
pub async fn node_rollup_for_characters(
  db: &Database,
  character_ids: &[i64],
  container_id: i64,
) -> Result<NodeRollup, Error> {
  if character_ids.is_empty() {
    return Ok(NodeRollup::default());
  }
  node_rollup(db, "character_assets", "character_id", character_ids, container_id).await
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
pub async fn node_rollup_for_corporation(
  db: &Database,
  corporation_id: i64,
  container_id: i64,
) -> Result<NodeRollup, Error> {
  if !corp_scope_visible(db, corporation_id).await? {
    return Ok(NodeRollup::default());
  }
  node_rollup(
    db,
    "corporation_assets",
    "corporation_id",
    &[corporation_id],
    container_id,
  )
  .await
}

pub async fn ancestors_of_match_for_character(
  db: &Database,
  character_id: i64,
  filter: &str,
  me_id: Option<i64>,
) -> Result<Vec<i64>, Error> {
  ancestors_of_match(db, "character_assets", "character_id", &[character_id], filter, me_id).await
}

pub async fn ancestors_of_match_for_characters(
  db: &Database,
  character_ids: &[i64],
  filter: &str,
  me_id: Option<i64>,
) -> Result<Vec<i64>, Error> {
  if character_ids.is_empty() {
    return Ok(Vec::new());
  }
  ancestors_of_match(db, "character_assets", "character_id", character_ids, filter, me_id).await
}

pub async fn ancestors_of_match_for_corporation(
  db: &Database,
  corporation_id: i64,
  filter: &str,
  me_id: Option<i64>,
) -> Result<Vec<i64>, Error> {
  if !corp_scope_visible(db, corporation_id).await? {
    return Ok(Vec::new());
  }
  ancestors_of_match(
    db,
    "corporation_assets",
    "corporation_id",
    &[corporation_id],
    filter,
    me_id,
  )
  .await
}

async fn children_render(
  db: &Database,
  table: &'static str,
  owner_column: &'static str,
  owner_ids: &[i64],
  container_id: i64,
  reproc_yield: f64,
) -> Result<Vec<InventoryRow>, Error> {
  let select_head = inventory_select_head(table, owner_column, reproc_yield);

  let mut builder = QueryBuilder::<Sqlite>::new(select_head);
  push_owner_predicate(&mut builder, owner_ids);
  builder.push(" AND a.container_id = ");
  builder.push_bind(container_id);
  builder.push(" ORDER BY a.item_id");

  let rows = builder.build_query_as::<InventoryRowSql>().fetch_all(&db.0).await?;
  Ok(rows.into_iter().map(InventoryRowSql::into_row).collect())
}

/// Renders full inventory rows for an explicit set of `item_id`s, regardless of the active filter.
///
/// Mirrors `children_render` but keys on `a.item_id IN (...)` instead of `a.container_id = ?`.
/// Used to recover the ancestor-container rows that a filtered page query drops (a container row
/// fails the filter itself), so they can be injected back into the list and auto-expanded to reveal
/// the matches nested inside them.
async fn rows_by_item_id(
  db: &Database,
  table: &'static str,
  owner_column: &'static str,
  owner_ids: &[i64],
  item_ids: &[i64],
  reproc_yield: f64,
) -> Result<Vec<InventoryRow>, Error> {
  if item_ids.is_empty() {
    return Ok(Vec::new());
  }
  let select_head = inventory_select_head(table, owner_column, reproc_yield);

  let mut builder = QueryBuilder::<Sqlite>::new(select_head);
  push_owner_predicate(&mut builder, owner_ids);
  builder.push(" AND a.item_id IN (");
  let mut separated = builder.separated(", ");
  for item_id in item_ids {
    separated.push_bind(*item_id);
  }
  builder.push(") ORDER BY a.item_id");

  let rows = builder.build_query_as::<InventoryRowSql>().fetch_all(&db.0).await?;
  Ok(rows.into_iter().map(InventoryRowSql::into_row).collect())
}

pub async fn rows_by_item_id_for_character(
  db: &Database,
  character_id: i64,
  item_ids: &[i64],
  reproc_yield: f64,
) -> Result<Vec<InventoryRow>, Error> {
  rows_by_item_id(
    db,
    "character_assets",
    "character_id",
    &[character_id],
    item_ids,
    reproc_yield,
  )
  .await
}

pub async fn rows_by_item_id_for_characters(
  db: &Database,
  character_ids: &[i64],
  item_ids: &[i64],
  reproc_yield: f64,
) -> Result<Vec<InventoryRow>, Error> {
  if character_ids.is_empty() {
    return Ok(Vec::new());
  }
  rows_by_item_id(
    db,
    "character_assets",
    "character_id",
    character_ids,
    item_ids,
    reproc_yield,
  )
  .await
}

pub async fn rows_by_item_id_for_corporation(
  db: &Database,
  corporation_id: i64,
  item_ids: &[i64],
  reproc_yield: f64,
) -> Result<Vec<InventoryRow>, Error> {
  if !corp_scope_visible(db, corporation_id).await? {
    return Ok(Vec::new());
  }
  rows_by_item_id(
    db,
    "corporation_assets",
    "corporation_id",
    &[corporation_id],
    item_ids,
    reproc_yield,
  )
  .await
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
async fn child_count(
  db: &Database,
  table: &'static str,
  owner_column: &'static str,
  owner_ids: &[i64],
  container_id: i64,
) -> Result<i64, Error> {
  let select_head = child_count_head(table, owner_column);

  let mut builder = QueryBuilder::<Sqlite>::new(select_head);
  push_owner_predicate(&mut builder, owner_ids);
  builder.push(" AND container_id = ");
  builder.push_bind(container_id);

  let count = builder.build_query_scalar::<i64>().fetch_one(&db.0).await?;
  Ok(count)
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
async fn node_rollup(
  db: &Database,
  table: &'static str,
  owner_column: &'static str,
  owner_ids: &[i64],
  container_id: i64,
) -> Result<NodeRollup, Error> {
  let (anchor_head, recurse_head, aggregate_head) = node_rollup_sql(table, owner_column);

  let mut builder = QueryBuilder::<Sqlite>::new(anchor_head);
  push_owner_predicate(&mut builder, owner_ids);
  builder.push(" AND container_id = ");
  builder.push_bind(container_id);
  builder.push(recurse_head);
  push_owner_predicate(&mut builder, owner_ids);
  builder.push(aggregate_head);
  push_owner_predicate(&mut builder, owner_ids);

  let row = builder.build_query_as::<NodeRollupSql>().fetch_one(&db.0).await?;
  Ok(NodeRollup {
    items: row.items.unwrap_or(0),
    value: row.value.unwrap_or(0.0),
  })
}

async fn ancestors_of_match(
  db: &Database,
  table: &'static str,
  owner_column: &'static str,
  owner_ids: &[i64],
  filter: &str,
  me_id: Option<i64>,
) -> Result<Vec<i64>, Error> {
  let Some(clause) = scoped_where(filter, owner_column, me_id) else {
    return Ok(Vec::new());
  };
  let (anchor_head, recurse_tail) = ancestors_of_match_sql(table, owner_column);

  let mut builder = QueryBuilder::<Sqlite>::new(anchor_head);
  push_owner_predicate(&mut builder, owner_ids);
  builder.push(" AND (");
  clause.bind_onto(&mut builder);
  builder.push(")");
  builder.push(recurse_tail);
  push_owner_predicate(&mut builder, owner_ids);
  builder.push(" AND a.container_id IS NOT NULL) SELECT DISTINCT container_id FROM ancestors ORDER BY container_id");

  let rows = builder.build_query_scalar::<i64>().fetch_all(&db.0).await?;
  Ok(rows)
}

async fn accumulate_on_hand(
  db: &Database,
  table: &'static str,
  location_ids: &[i64],
  owner: Option<(&'static str, i64)>,
  totals: &mut HashMap<(i64, i64), i64>,
) -> Result<(), Error> {
  let mut builder = QueryBuilder::<Sqlite>::new("SELECT location_id, type_id, SUM(quantity) FROM ");
  builder.push(table);
  builder.push(" WHERE container_id IS NULL AND location_id IN (");
  let mut separated = builder.separated(", ");
  for id in location_ids {
    separated.push_bind(*id);
  }
  builder.push(")");

  if let Some((owner_column, owner_id)) = owner {
    builder.push(" AND ");
    builder.push(owner_column);
    builder.push(" = ");
    builder.push_bind(owner_id);
  }

  builder.push(" GROUP BY location_id, type_id");

  let rows = builder.build_query_as::<(i64, i64, i64)>().fetch_all(&db.0).await?;
  for (location_id, type_id, quantity) in rows {
    *totals.entry((location_id, type_id)).or_insert(0) += quantity;
  }
  Ok(())
}

async fn corporations_with_assets_at(db: &Database, location_ids: &[i64]) -> Result<Vec<i64>, Error> {
  let mut builder =
    QueryBuilder::<Sqlite>::new("SELECT DISTINCT corporation_id FROM corporation_assets WHERE location_id IN (");
  let mut separated = builder.separated(", ");
  for id in location_ids {
    separated.push_bind(*id);
  }
  builder.push(")");

  let rows = builder.build_query_scalar::<i64>().fetch_all(&db.0).await?;
  Ok(rows)
}

fn push_owner_predicate(builder: &mut QueryBuilder<Sqlite>, owner_ids: &[i64]) {
  if let [owner_id] = owner_ids {
    builder.push("= ");
    builder.push_bind(*owner_id);
    return;
  }
  builder.push("IN (");
  let mut separated = builder.separated(", ");
  for owner_id in owner_ids {
    separated.push_bind(*owner_id);
  }
  builder.push(")");
}

fn scoped_where(filter: &str, owner_column: &'static str, me_id: Option<i64>) -> Option<WhereClause> {
  let schema = render_column_schema(owner_column);
  compile_query(
    filter,
    &schema,
    FilterContext {
      me_id,
    },
  )
}

async fn inventory_page(
  db: &Database,
  table: &'static str,
  owner_column: &'static str,
  owner_ids: &[i64],
  query: &InventoryQuery<'_>,
) -> Result<Vec<InventoryRow>, Error> {
  let select_head = inventory_select_head(table, owner_column, query.reproc_yield);
  let sort_expr = sort_column_expr(query.sort, owner_column);

  let mut builder = QueryBuilder::<Sqlite>::new(select_head);
  push_owner_predicate(&mut builder, owner_ids);
  builder.push(" AND a.container_id IS NULL");

  if !query.location_ids.is_empty() {
    builder.push(" AND a.location_id IN (");
    let mut separated = builder.separated(", ");
    for location_id in query.location_ids {
      separated.push_bind(*location_id);
    }
    builder.push(")");
  }

  if let Some(clause) = scoped_where(query.filter, owner_column, query.me_id) {
    builder.push(" AND (");
    clause.bind_onto(&mut builder);
    builder.push(")");
  }

  if let Some(cursor) = &query.cursor {
    push_keyset_seek(&mut builder, sort_expr, query.direction, cursor);
  }

  builder.push(" ORDER BY ");
  builder.push(sort_expr);
  builder.push(" ");
  builder.push(direction_keyword(query.direction));
  builder.push(", a.item_id ");
  builder.push(direction_keyword(query.direction));
  builder.push(" LIMIT ");
  builder.push_bind(query.limit);

  let rows = builder.build_query_as::<InventoryRowSql>().fetch_all(&db.0).await?;
  Ok(rows.into_iter().map(InventoryRowSql::into_row).collect())
}

fn push_keyset_seek(
  builder: &mut QueryBuilder<Sqlite>,
  sort_expr: &str,
  direction: SortDirection,
  cursor: &InventoryCursor,
) {
  let op = seek_operator(direction);
  builder.push(" AND (");
  builder.push(sort_expr);
  builder.push(" ");
  builder.push(op);
  builder.push(" ");
  bind_sort_value(builder, &cursor.sort_value);
  builder.push(" OR (");
  builder.push(sort_expr);
  builder.push(" = ");
  bind_sort_value(builder, &cursor.sort_value);
  builder.push(" AND a.item_id ");
  builder.push(op);
  builder.push(" ");
  builder.push_bind(cursor.item_id);
  builder.push("))");
}

fn bind_sort_value(builder: &mut QueryBuilder<Sqlite>, value: &SortValue) {
  match value {
    SortValue::Int(v) => builder.push_bind(*v),
    SortValue::Real(v) => builder.push_bind(*v),
    SortValue::Text(v) => builder.push_bind(v.clone()),
  };
}

async fn inventory_totals(
  db: &Database,
  table: &'static str,
  owner_column: &'static str,
  owner_ids: &[i64],
  filter: &str,
  location_ids: &[i64],
  me_id: Option<i64>,
) -> Result<InventoryTotals, Error> {
  let select_head = inventory_totals_head(table, owner_column);

  let mut builder = QueryBuilder::<Sqlite>::new(select_head);
  push_owner_predicate(&mut builder, owner_ids);

  if !location_ids.is_empty() {
    builder.push(" AND a.location_id IN (");
    let mut separated = builder.separated(", ");
    for location_id in location_ids {
      separated.push_bind(*location_id);
    }
    builder.push(")");
  }

  if let Some(clause) = scoped_where(filter, owner_column, me_id) {
    builder.push(" AND (");
    clause.bind_onto(&mut builder);
    builder.push(")");
  }

  let row = builder.build_query_as::<TotalsRowSql>().fetch_one(&db.0).await?;
  Ok(InventoryTotals {
    items: row.items.unwrap_or(0),
    locations: row.locations,
    value: row.value.unwrap_or(0.0),
    volume: row.volume.unwrap_or(0.0),
  })
}

async fn geo_locations(
  db: &Database,
  table: &'static str,
  owner_column: &'static str,
  owner_ids: &[i64],
) -> Result<Vec<GeoLocation>, Error> {
  let select_head = geo_select_head(table, owner_column);

  let mut builder = QueryBuilder::<Sqlite>::new(select_head);
  push_owner_predicate(&mut builder, owner_ids);
  builder.push(" AND a.location_type <> 'item' GROUP BY a.location_id, a.location_type");

  let rows = builder.build_query_as::<GeoLocationSql>().fetch_all(&db.0).await?;
  Ok(rows.into_iter().map(GeoLocationSql::into_geo).collect())
}

fn geo_select_head(table: &str, owner_column: &str) -> &'static str {
  match (table, owner_column) {
    ("character_assets", "character_id") => GEO_SELECT_CHARACTER,
    ("corporation_assets", "corporation_id") => GEO_SELECT_CORPORATION,
    _ => unreachable!("geo_select_head called with an unknown owner table"),
  }
}

fn combined_column_schema() -> ColumnSchema {
  ColumnSchema {
    category: "category",
    character_id: "owner_id",
    constellation_name: "constellation_name",
    group_name: "group_name",
    is_blueprint_copy: "is_blueprint_copy",
    is_singleton: "is_singleton",
    item_id: "item_id",
    location_name: "location_label",
    name: "name",
    region_name: "region_name",
    system_name: "system_name",
    type_name: "type_name",
  }
}

fn combined_sort_column_expr(sort: SortColumn) -> &'static str {
  match sort {
    SortColumn::Category => "category",
    SortColumn::Group => "group_name",
    SortColumn::Name => "COALESCE(name, type_name)",
    SortColumn::Owner => "owner_id",
    SortColumn::Quantity => "quantity",
    SortColumn::UnitPrice => "unit_price",
    SortColumn::Value => "value",
    SortColumn::Volume => "row_volume",
  }
}

fn combined_arm_head(table: &str, owner_column: &str, reproc_yield: f64) -> String {
  let head = match (table, owner_column) {
    ("character_assets", "character_id") => COMBINED_ARM_CHARACTER,
    ("corporation_assets", "corporation_id") => COMBINED_ARM_CORPORATION,
    _ => unreachable!("combined_arm_head called with an unknown owner table"),
  };
  bind_reproc_yield(head, reproc_yield)
}

fn push_combined_arm(
  builder: &mut QueryBuilder<Sqlite>,
  table: &'static str,
  owner_column: &'static str,
  owner_ids: &[i64],
  top_level_only: bool,
  reproc_yield: f64,
) {
  builder.push(combined_arm_head(table, owner_column, reproc_yield));
  push_owner_predicate(builder, owner_ids);
  if top_level_only {
    builder.push(" AND a.container_id IS NULL");
  }
}

fn push_combined_union(
  builder: &mut QueryBuilder<Sqlite>,
  character_ids: &[i64],
  corporation_ids: &[i64],
  top_level_only: bool,
  reproc_yield: f64,
) {
  let mut needs_union = false;
  if !character_ids.is_empty() {
    push_combined_arm(
      builder,
      "character_assets",
      "character_id",
      character_ids,
      top_level_only,
      reproc_yield,
    );
    needs_union = true;
  }
  if !corporation_ids.is_empty() {
    if needs_union {
      builder.push(" UNION ALL ");
    }
    push_combined_arm(
      builder,
      "corporation_assets",
      "corporation_id",
      corporation_ids,
      top_level_only,
      reproc_yield,
    );
  }
}

async fn combined_inventory_page(
  db: &Database,
  character_ids: &[i64],
  corporation_ids: &[i64],
  query: &InventoryQuery<'_>,
) -> Result<Vec<InventoryRow>, Error> {
  let schema = combined_column_schema();
  let sort_expr = combined_sort_column_expr(query.sort);

  let mut builder = QueryBuilder::<Sqlite>::new("SELECT * FROM (");
  push_combined_union(&mut builder, character_ids, corporation_ids, true, query.reproc_yield);
  // No-op WHERE seed so every optional facet below can append uniformly with "AND".
  builder.push(") a WHERE 1 = 1");

  if !query.location_ids.is_empty() {
    builder.push(" AND a.location_id IN (");
    let mut separated = builder.separated(", ");
    for location_id in query.location_ids {
      separated.push_bind(*location_id);
    }
    builder.push(")");
  }

  if let Some(clause) = compile_query(
    query.filter,
    &schema,
    FilterContext {
      me_id: query.me_id,
    },
  ) {
    builder.push(" AND (");
    clause.bind_onto(&mut builder);
    builder.push(")");
  }

  if let Some(cursor) = &query.cursor {
    push_keyset_seek(&mut builder, sort_expr, query.direction, cursor);
  }

  builder.push(" ORDER BY ");
  builder.push(sort_expr);
  builder.push(" ");
  builder.push(direction_keyword(query.direction));
  builder.push(", a.item_id ");
  builder.push(direction_keyword(query.direction));
  builder.push(" LIMIT ");
  builder.push_bind(query.limit);

  let rows = builder.build_query_as::<InventoryRowSql>().fetch_all(&db.0).await?;
  Ok(rows.into_iter().map(InventoryRowSql::into_row).collect())
}

async fn combined_inventory_totals(
  db: &Database,
  character_ids: &[i64],
  corporation_ids: &[i64],
  filter: &str,
  location_ids: &[i64],
  me_id: Option<i64>,
) -> Result<InventoryTotals, Error> {
  let schema = combined_column_schema();

  let mut builder = QueryBuilder::<Sqlite>::new(
    "SELECT SUM(a.quantity) AS items, COUNT(DISTINCT a.location_id) AS locations, SUM(a.value) AS value, \
    SUM(a.row_volume) AS volume FROM (",
  );
  // Totals never sum reproc value, so a 0 yield elides its contribution without changing the schema.
  push_combined_union(&mut builder, character_ids, corporation_ids, false, 0.0);
  builder.push(") a WHERE 1 = 1");

  if !location_ids.is_empty() {
    builder.push(" AND a.location_id IN (");
    let mut separated = builder.separated(", ");
    for location_id in location_ids {
      separated.push_bind(*location_id);
    }
    builder.push(")");
  }

  if let Some(clause) = compile_query(
    filter,
    &schema,
    FilterContext {
      me_id,
    },
  ) {
    builder.push(" AND (");
    clause.bind_onto(&mut builder);
    builder.push(")");
  }

  let row = builder.build_query_as::<TotalsRowSql>().fetch_one(&db.0).await?;
  Ok(InventoryTotals {
    items: row.items.unwrap_or(0),
    locations: row.locations,
    value: row.value.unwrap_or(0.0),
    volume: row.volume.unwrap_or(0.0),
  })
}

async fn combined_geo_locations(
  db: &Database,
  character_ids: &[i64],
  corporation_ids: &[i64],
) -> Result<Vec<GeoLocation>, Error> {
  let mut rows = Vec::new();
  if !character_ids.is_empty() {
    rows.extend(geo_locations(db, "character_assets", "character_id", character_ids).await?);
  }
  for &corporation_id in corporation_ids {
    rows.extend(geo_locations(db, "corporation_assets", "corporation_id", &[corporation_id]).await?);
  }
  Ok(merge_geo_locations(rows))
}

fn merge_geo_locations(rows: Vec<GeoLocation>) -> Vec<GeoLocation> {
  use std::collections::HashMap;

  let mut merged: HashMap<(i64, String), GeoLocation> = HashMap::new();
  for row in rows {
    let key = (row.location_id, row.location_type.clone());
    match merged.get_mut(&key) {
      Some(existing) => {
        existing.item_count += row.item_count;
        existing.value += row.value;
      }
      None => {
        merged.insert(key, row);
      }
    }
  }
  let mut out: Vec<GeoLocation> = merged.into_values().collect();
  out.sort_by(|a, b| {
    a.location_id
      .cmp(&b.location_id)
      .then(a.location_type.cmp(&b.location_type))
  });
  out
}

fn inventory_select_head(table: &str, owner_column: &str, reproc_yield: f64) -> String {
  let head = match (table, owner_column) {
    ("character_assets", "character_id") => INVENTORY_SELECT_CHARACTER,
    ("corporation_assets", "corporation_id") => INVENTORY_SELECT_CORPORATION,
    _ => unreachable!("inventory_select_head called with an unknown owner table"),
  };
  bind_reproc_yield(head, reproc_yield)
}

// Substitutes the `{reproc_yield}` token with the configured flat refine yield. The value is an
// internal config f64 (never user input), so formatting it into the SQL literal is safe; using
// {:?} guarantees a finite, parseable decimal (e.g. 0.5).
fn bind_reproc_yield(sql: &str, reproc_yield: f64) -> String {
  let safe_yield = if reproc_yield.is_finite() {
    reproc_yield.max(0.0)
  } else {
    0.0
  };
  sql.replace("{reproc_yield}", &format!("{safe_yield:?}"))
}

fn inventory_totals_head(table: &str, owner_column: &str) -> &'static str {
  match (table, owner_column) {
    ("character_assets", "character_id") => TOTALS_SELECT_CHARACTER,
    ("corporation_assets", "corporation_id") => TOTALS_SELECT_CORPORATION,
    _ => unreachable!("inventory_totals_head called with an unknown owner table"),
  }
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
fn child_count_head(table: &str, owner_column: &str) -> &'static str {
  match (table, owner_column) {
    ("character_assets", "character_id") => "SELECT COUNT(*) FROM character_assets WHERE character_id ",
    ("corporation_assets", "corporation_id") => "SELECT COUNT(*) FROM corporation_assets WHERE corporation_id ",
    _ => unreachable!("child_count_head called with an unknown owner table"),
  }
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
fn node_rollup_sql(table: &str, owner_column: &str) -> (&'static str, &'static str, &'static str) {
  match (table, owner_column) {
    ("character_assets", "character_id") => (
      NODE_ROLLUP_ANCHOR_CHARACTER,
      NODE_ROLLUP_RECURSE_CHARACTER,
      NODE_ROLLUP_AGGREGATE_CHARACTER,
    ),
    ("corporation_assets", "corporation_id") => (
      NODE_ROLLUP_ANCHOR_CORPORATION,
      NODE_ROLLUP_RECURSE_CORPORATION,
      NODE_ROLLUP_AGGREGATE_CORPORATION,
    ),
    _ => unreachable!("node_rollup_sql called with an unknown owner table"),
  }
}

fn ancestors_of_match_sql(table: &str, owner_column: &str) -> (&'static str, &'static str) {
  match (table, owner_column) {
    ("character_assets", "character_id") => (ANCESTORS_ANCHOR_CHARACTER, ANCESTORS_RECURSE_CHARACTER),
    ("corporation_assets", "corporation_id") => (ANCESTORS_ANCHOR_CORPORATION, ANCESTORS_RECURSE_CORPORATION),
    _ => unreachable!("ancestors_of_match_sql called with an unknown owner table"),
  }
}

macro_rules! inventory_select_sql {
  ($table:literal, $owner:literal, $owner_type:literal, $active_ship:literal, $abyssal:literal) => {
    concat!(
      "SELECT a.item_id, a.type_id, a.quantity, a.location_id, a.container_id, a.depth, a.is_container, ",
      $active_ship,
      " AS is_active_ship, a.is_blueprint_copy AS is_blueprint_copy, a.",
      $owner,
      " AS owner_id, a.name AS name, ",
      type_name_expr!(),
      " AS type_name, ",
      group_name_expr!(),
      " AS group_name, ",
      category_key_case!(),
      " AS category, ",
      row_volume_expr!(),
      " AS row_volume, ",
      unit_price_expr!(),
      " AS unit_price, ",
      value_expr!(),
      " AS value, ",
      reproc_value_expr!(),
      " AS reproc_value, ",
      location_label_expr!(),
      " AS location_label ",
      query_join_sql!($table, $owner, $owner_type, $abyssal)
    )
  };
}

macro_rules! combined_arm_sql {
  ($table:literal, $owner:literal, $owner_type:literal, $active_ship:literal, $abyssal:literal) => {
    concat!(
      "SELECT a.item_id, a.type_id, a.quantity, a.location_id, a.container_id, a.depth, a.is_container, ",
      $active_ship,
      " AS is_active_ship, a.is_blueprint_copy AS is_blueprint_copy, a.is_singleton AS is_singleton, a.",
      $owner,
      " AS owner_id, a.name AS name, ",
      type_name_expr!(),
      " AS type_name, ",
      group_name_expr!(),
      " AS group_name, ",
      category_key_case!(),
      " AS category, ",
      row_volume_expr!(),
      " AS row_volume, ",
      unit_price_expr!(),
      " AS unit_price, ",
      value_expr!(),
      " AS value, ",
      reproc_value_expr!(),
      " AS reproc_value, ",
      location_label_expr!(),
      " AS location_label, sys.name AS system_name, con.name AS constellation_name, reg.name AS region_name ",
      query_join_sql!($table, $owner, $owner_type, $abyssal)
    )
  };
}

macro_rules! totals_select_sql {
  ($table:literal, $owner:literal, $owner_type:literal, $abyssal:literal) => {
    concat!(
      "SELECT SUM(a.quantity) AS items, COUNT(DISTINCT a.location_id) AS locations, SUM(",
      value_expr!(),
      ") AS value, SUM(",
      row_volume_expr!(),
      ") AS volume ",
      query_join_sql!($table, $owner, $owner_type, $abyssal)
    )
  };
}

macro_rules! geo_label_expr {
  () => {
    "CASE WHEN ina.id IS NOT NULL THEN 'Inaccessible Structure' ELSE COALESCE(s.name, st.name, sys.name) END"
  };
}

macro_rules! geo_join_sql {
  ($owner:literal, $owner_type:literal) => {
    concat!(location_join_sql!($owner, $owner_type), geo_extra_join_sql!())
  };
}

macro_rules! geo_select_sql {
  ($table:literal, $owner:literal, $owner_type:literal, $abyssal:literal) => {
    concat!(
      "SELECT a.location_id, a.location_type, ",
      geo_label_expr!(),
      " AS location_label, sys.id AS system_id, sys.name AS system_name, sys.security_status AS security_status, \
        con.id AS constellation_id, \
        con.name AS constellation_name, reg.id AS region_id, reg.name AS region_name, \
        SUM(a.quantity) AS item_count, CAST(SUM(",
      value_expr!(),
      ") AS REAL) AS value \
      FROM ",
      $table,
      " a \
      JOIN item_types it ON it.id = a.type_id \
      JOIN item_groups ig ON ig.id = it.group_id \
      JOIN item_categories ic ON ic.id = ig.category_id \
      LEFT JOIN market_prices mp ON mp.type_id = a.type_id",
      abyssal_join_sql!($abyssal),
      geo_join_sql!($owner, $owner_type),
      "WHERE a.",
      $owner,
      " "
    )
  };
}

const GEO_SELECT_CHARACTER: &str = geo_select_sql!("character_assets", "character_id", "character", "abyssal_items");
const GEO_SELECT_CORPORATION: &str = geo_select_sql!(
  "corporation_assets",
  "corporation_id",
  "corporation",
  "corporation_abyssal_items"
);
const INVENTORY_SELECT_CHARACTER: &str = inventory_select_sql!(
  "character_assets",
  "character_id",
  "character",
  "a.is_active_ship",
  "abyssal_items"
);
const INVENTORY_SELECT_CORPORATION: &str = inventory_select_sql!(
  "corporation_assets",
  "corporation_id",
  "corporation",
  "0",
  "corporation_abyssal_items"
);
const TOTALS_SELECT_CHARACTER: &str =
  totals_select_sql!("character_assets", "character_id", "character", "abyssal_items");
const TOTALS_SELECT_CORPORATION: &str = totals_select_sql!(
  "corporation_assets",
  "corporation_id",
  "corporation",
  "corporation_abyssal_items"
);
const COMBINED_ARM_CHARACTER: &str = combined_arm_sql!(
  "character_assets",
  "character_id",
  "character",
  "a.is_active_ship",
  "abyssal_items"
);
const COMBINED_ARM_CORPORATION: &str = combined_arm_sql!(
  "corporation_assets",
  "corporation_id",
  "corporation",
  "0",
  "corporation_abyssal_items"
);

macro_rules! node_rollup_anchor_sql_lit {
  ($table:literal, $owner:literal) => {
    concat!(
      "WITH RECURSIVE subtree(item_id) AS ( \
        SELECT item_id FROM ",
      $table,
      " WHERE ",
      $owner,
      " "
    )
  };
}

macro_rules! node_rollup_recurse_sql_lit {
  ($table:literal, $owner:literal) => {
    concat!(
      " UNION ALL \
        SELECT a.item_id FROM ",
      $table,
      " a JOIN subtree s ON a.container_id = s.item_id WHERE a.",
      $owner,
      " "
    )
  };
}

macro_rules! node_rollup_aggregate_sql_lit {
  ($table:literal, $owner:literal, $abyssal:literal) => {
    concat!(
      " ) \
      SELECT SUM(",
      value_expr!(),
      ") AS value, SUM(a.quantity) AS items \
      FROM ",
      $table,
      " a \
      LEFT JOIN market_prices mp ON mp.type_id = a.type_id",
      abyssal_join_sql!($abyssal),
      "JOIN subtree s ON s.item_id = a.item_id \
      WHERE a.",
      $owner,
      " "
    )
  };
}

macro_rules! ancestors_anchor_sql_lit {
  ($table:literal, $owner:literal) => {
    concat!(
      "WITH RECURSIVE ancestors(container_id) AS ( \
        SELECT a.container_id FROM ",
      $table,
      " a \
        JOIN item_types it ON it.id = a.type_id \
        JOIN item_groups ig ON ig.id = it.group_id \
        JOIN item_categories ic ON ic.id = ig.category_id \
        LEFT JOIN market_prices mp ON mp.type_id = a.type_id \
        WHERE a.",
      $owner,
      " "
    )
  };
}

macro_rules! ancestors_recurse_sql_lit {
  ($table:literal, $owner:literal) => {
    concat!(
      " AND a.container_id IS NOT NULL \
        UNION \
        SELECT a.container_id FROM ",
      $table,
      " a \
        JOIN ancestors anc ON a.item_id = anc.container_id \
        WHERE a.",
      $owner,
      " "
    )
  };
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
const NODE_ROLLUP_ANCHOR_CHARACTER: &str = node_rollup_anchor_sql_lit!("character_assets", "character_id");
// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
const NODE_ROLLUP_ANCHOR_CORPORATION: &str = node_rollup_anchor_sql_lit!("corporation_assets", "corporation_id");
// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
const NODE_ROLLUP_RECURSE_CHARACTER: &str = node_rollup_recurse_sql_lit!("character_assets", "character_id");
// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
const NODE_ROLLUP_RECURSE_CORPORATION: &str = node_rollup_recurse_sql_lit!("corporation_assets", "corporation_id");
// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
const NODE_ROLLUP_AGGREGATE_CHARACTER: &str =
  node_rollup_aggregate_sql_lit!("character_assets", "character_id", "abyssal_items");
// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
const NODE_ROLLUP_AGGREGATE_CORPORATION: &str =
  node_rollup_aggregate_sql_lit!("corporation_assets", "corporation_id", "corporation_abyssal_items");
// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
const ANCESTORS_ANCHOR_CHARACTER: &str = ancestors_anchor_sql_lit!("character_assets", "character_id");
// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
const ANCESTORS_ANCHOR_CORPORATION: &str = ancestors_anchor_sql_lit!("corporation_assets", "corporation_id");
// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
const ANCESTORS_RECURSE_CHARACTER: &str = ancestors_recurse_sql_lit!("character_assets", "character_id");
// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
const ANCESTORS_RECURSE_CORPORATION: &str = ancestors_recurse_sql_lit!("corporation_assets", "corporation_id");

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
pub async fn completeness_for_character(db: &Database, character_id: i64) -> Result<AssetCompleteness, Error> {
  let distinct_type_ids =
    sqlx::query_scalar::<_, i64>("SELECT COUNT(DISTINCT type_id) FROM character_assets WHERE character_id = ?")
      .bind(character_id)
      .fetch_one(&db.0)
      .await?;
  let unresolved = sqlx::query_scalar::<_, i64>(
    "SELECT DISTINCT a.type_id FROM character_assets a \
    LEFT JOIN item_types it ON it.id = a.type_id \
    WHERE a.character_id = ? AND it.id IS NULL ORDER BY a.type_id",
  )
  .bind(character_id)
  .fetch_all(&db.0)
  .await?;
  let report = into_completeness(distinct_type_ids, unresolved);
  log_completeness("character", character_id, &report);
  Ok(report)
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
pub async fn completeness_for_corporation(db: &Database, corporation_id: i64) -> Result<AssetCompleteness, Error> {
  if !corp_scope_visible(db, corporation_id).await? {
    return Ok(AssetCompleteness::default());
  }
  let distinct_type_ids =
    sqlx::query_scalar::<_, i64>("SELECT COUNT(DISTINCT type_id) FROM corporation_assets WHERE corporation_id = ?")
      .bind(corporation_id)
      .fetch_one(&db.0)
      .await?;
  let unresolved = sqlx::query_scalar::<_, i64>(
    "SELECT DISTINCT a.type_id FROM corporation_assets a \
    LEFT JOIN item_types it ON it.id = a.type_id \
    WHERE a.corporation_id = ? AND it.id IS NULL ORDER BY a.type_id",
  )
  .bind(corporation_id)
  .fetch_all(&db.0)
  .await?;
  let report = into_completeness(distinct_type_ids, unresolved);
  log_completeness("corporation", corporation_id, &report);
  Ok(report)
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
fn into_completeness(distinct_type_ids: i64, unresolved: Vec<i64>) -> AssetCompleteness {
  AssetCompleteness {
    distinct_type_ids,
    resolved: distinct_type_ids - unresolved.len() as i64,
    unresolved,
  }
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
fn log_completeness(owner_kind: &str, owner_id: i64, report: &AssetCompleteness) {
  if !report.is_complete() {
    tracing::warn!(
      owner_kind,
      owner_id,
      unresolved_count = report.unresolved.len(),
      unresolved_type_ids = ?report.unresolved,
      "asset type_ids unresolved against the SDE item_types chain (seed/sync gap)"
    );
  }
}

async fn insert_character_asset(
  tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
  asset: &CharacterAsset,
) -> Result<(), Error> {
  // OR REPLACE evicts any row already holding this item_id under a different owner: item_id is a
  // global PK, and the upstream per-owner DELETE cannot clear the sending alt's stale snapshot.
  sqlx::query(
    "INSERT OR REPLACE INTO character_assets \
      (item_id, character_id, type_id, location_id, location_type, location_flag, quantity, is_singleton, \
      is_blueprint_copy, is_active_ship, name, container_id, depth, is_container) \
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
  )
  .bind(asset.item_id())
  .bind(asset.character_id())
  .bind(asset.type_id())
  .bind(asset.location_id())
  .bind(asset.location_type())
  .bind(asset.location_flag())
  .bind(asset.quantity())
  .bind(asset.is_singleton())
  .bind(asset.is_blueprint_copy())
  .bind(asset.is_active_ship())
  .bind(asset.name())
  .bind(asset.container_id())
  .bind(asset.depth())
  .bind(asset.is_container())
  .execute(&mut **tx)
  .await?;
  Ok(())
}

async fn insert_corporation_asset(
  tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
  asset: &CorporationAsset,
) -> Result<(), Error> {
  // OR REPLACE evicts any row already holding this item_id under a different corporation: item_id is
  // a global PK, and the upstream per-owner DELETE cannot clear the losing corp's stale snapshot.
  sqlx::query(
    "INSERT OR REPLACE INTO corporation_assets \
      (item_id, corporation_id, type_id, location_id, location_type, location_flag, quantity, is_singleton, \
      is_blueprint_copy, name, container_id, depth, is_container) \
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
  )
  .bind(asset.item_id())
  .bind(asset.corporation_id())
  .bind(asset.type_id())
  .bind(asset.location_id())
  .bind(asset.location_type())
  .bind(asset.location_flag())
  .bind(asset.quantity())
  .bind(asset.is_singleton())
  .bind(asset.is_blueprint_copy())
  .bind(asset.name())
  .bind(asset.container_id())
  .bind(asset.depth())
  .bind(asset.is_container())
  .execute(&mut **tx)
  .await?;
  Ok(())
}

pub async fn create(
  db: &Database,
  name: &str,
  character_scope: Option<String>,
  location_id: Option<i64>,
  items: &[(i64, i64)],
) -> Result<StockpileWithItems, Error> {
  let mut tx = db.writer().begin().await?;
  let stockpile = sqlx::query_as::<_, Stockpile>(
    "INSERT INTO stockpiles (name, character_scope, location_id) VALUES (?, ?, ?) \
    RETURNING character_scope, id, location_id, name",
  )
  .bind(name)
  .bind(character_scope)
  .bind(location_id)
  .fetch_one(&mut *tx)
  .await?;
  let items = insert_items(&mut tx, stockpile.id(), items).await?;
  tx.commit().await?;
  Ok(StockpileWithItems {
    stockpile,
    items,
  })
}

pub async fn update(
  db: &Database,
  id: i64,
  name: &str,
  character_scope: Option<String>,
  location_id: Option<i64>,
  items: &[(i64, i64)],
) -> Result<StockpileWithItems, Error> {
  let mut tx = db.writer().begin().await?;
  let stockpile = sqlx::query_as::<_, Stockpile>(
    "UPDATE stockpiles SET name = ?, character_scope = ?, location_id = ? WHERE id = ? \
    RETURNING character_scope, id, location_id, name",
  )
  .bind(name)
  .bind(character_scope)
  .bind(location_id)
  .bind(id)
  .fetch_one(&mut *tx)
  .await?;
  sqlx::query("DELETE FROM stockpile_items WHERE stockpile_id = ?")
    .bind(id)
    .execute(&mut *tx)
    .await?;
  let items = insert_items(&mut tx, id, items).await?;
  tx.commit().await?;
  Ok(StockpileWithItems {
    stockpile,
    items,
  })
}

pub async fn delete(db: &Database, id: i64) -> Result<(), Error> {
  sqlx::query("DELETE FROM stockpiles WHERE id = ?")
    .bind(id)
    .execute(db.writer())
    .await?;
  Ok(())
}

pub async fn get(db: &Database, id: i64) -> Result<Option<Stockpile>, Error> {
  let row =
    sqlx::query_as::<_, Stockpile>("SELECT character_scope, id, location_id, name FROM stockpiles WHERE id = ?")
      .bind(id)
      .fetch_optional(&db.0)
      .await?;
  Ok(row)
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[allow(dead_code)]
pub async fn with_items(db: &Database, id: i64) -> Result<Option<StockpileWithItems>, Error> {
  let Some(stockpile) = get(db, id).await? else {
    return Ok(None);
  };
  let items = items(db, id).await?;
  Ok(Some(StockpileWithItems {
    stockpile,
    items,
  }))
}

pub async fn list_with_items(db: &Database) -> Result<Vec<StockpileWithItems>, Error> {
  let stockpiles =
    sqlx::query_as::<_, Stockpile>("SELECT character_scope, id, location_id, name FROM stockpiles ORDER BY id")
      .fetch_all(&db.0)
      .await?;
  let mut result = Vec::with_capacity(stockpiles.len());
  for stockpile in stockpiles {
    let items = items(db, stockpile.id()).await?;
    result.push(StockpileWithItems {
      stockpile,
      items,
    });
  }
  Ok(result)
}

pub async fn items(db: &Database, stockpile_id: i64) -> Result<Vec<StockpileItem>, Error> {
  let rows = sqlx::query_as::<_, StockpileItem>(
    "SELECT id, stockpile_id, target_quantity, type_id FROM stockpile_items WHERE stockpile_id = ? ORDER BY id",
  )
  .bind(stockpile_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn fill_status(db: &Database, id: i64, scope: &[i64]) -> Result<Option<StockpileFill>, Error> {
  let Some(stockpile) = get(db, id).await? else {
    return Ok(None);
  };
  let location_id = stockpile.location_id();
  // An empty scope means "all characters". A non-empty scope is bound as a JSON array; json_array_length
  // gates the IN-list so that char assets are restricted to the scoped character ids and corp assets to
  // the corps those characters belong to.
  let scope_json = serde_json::to_string(scope).unwrap_or_else(|_| "[]".to_string());
  // loc_set needs no explicit "what kind of id is this?" detection: EVE id-spaces are disjoint ranges
  // (regions 10M, constellations 20M, systems 30M, stations 60M+, structures 1e12+), so each expanding
  // join matches only when location_id is genuinely that tier. A region pile rolls up every station and
  // structure in the region; a constellation pile every one in the constellation; a system pile every
  // one in the system; a station/structure pile stays itself.
  let rows = sqlx::query_as::<_, (i64, i64, i64)>(
    "WITH RECURSIVE \
    loc_set(location_id) AS ( \
      SELECT ? \
      UNION \
      SELECT id FROM stations WHERE system_id = ? \
      UNION \
      SELECT id FROM structures WHERE solar_system_id = ? \
      UNION \
      SELECT st.id FROM stations st JOIN solar_systems ss ON st.system_id = ss.id WHERE ss.constellation_id = ? \
      UNION \
      SELECT sr.id FROM structures sr JOIN solar_systems ss ON sr.solar_system_id = ss.id \
        WHERE ss.constellation_id = ? \
      UNION \
      SELECT st.id FROM stations st JOIN solar_systems ss ON st.system_id = ss.id \
        JOIN constellations c ON ss.constellation_id = c.id WHERE c.region_id = ? \
      UNION \
      SELECT sr.id FROM structures sr JOIN solar_systems ss ON sr.solar_system_id = ss.id \
        JOIN constellations c ON ss.constellation_id = c.id WHERE c.region_id = ? \
    ), \
    char_tree(item_id, type_id, quantity, root_location_id) AS ( \
      SELECT item_id, type_id, quantity, location_id \
      FROM character_assets \
      WHERE container_id IS NULL \
        AND (json_array_length(?) = 0 OR character_id IN (SELECT value FROM json_each(?))) \
      UNION ALL \
      SELECT ca.item_id, ca.type_id, ca.quantity, ct.root_location_id \
      FROM character_assets ca \
      JOIN char_tree ct ON ca.container_id = ct.item_id \
    ), \
    corp_tree(item_id, type_id, quantity, root_location_id) AS ( \
      SELECT item_id, type_id, quantity, location_id \
      FROM corporation_assets \
      WHERE container_id IS NULL \
        AND (json_array_length(?) = 0 \
          OR corporation_id IN ( \
            SELECT corporation_id FROM characters WHERE id IN (SELECT value FROM json_each(?)) \
          )) \
      UNION ALL \
      SELECT ca.item_id, ca.type_id, ca.quantity, parent.root_location_id \
      FROM corporation_assets ca \
      JOIN corp_tree parent ON ca.container_id = parent.item_id \
    ), \
    have(type_id, quantity) AS ( \
      SELECT type_id, quantity FROM char_tree \
      WHERE (? IS NULL OR root_location_id IN (SELECT location_id FROM loc_set)) \
      UNION ALL \
      SELECT type_id, quantity FROM corp_tree \
      WHERE (? IS NULL OR root_location_id IN (SELECT location_id FROM loc_set)) \
    ) \
    SELECT si.type_id, si.target_quantity, COALESCE(SUM(h.quantity), 0) AS have_quantity \
    FROM stockpile_items si \
    LEFT JOIN have h ON h.type_id = si.type_id \
    WHERE si.stockpile_id = ? \
    GROUP BY si.id, si.type_id, si.target_quantity \
    ORDER BY si.id",
  )
  .bind(location_id)
  .bind(location_id)
  .bind(location_id)
  .bind(location_id)
  .bind(location_id)
  .bind(location_id)
  .bind(location_id)
  .bind(scope_json.clone())
  .bind(scope_json.clone())
  .bind(scope_json.clone())
  .bind(scope_json)
  .bind(location_id)
  .bind(location_id)
  .bind(id)
  .fetch_all(&db.0)
  .await?;
  let items = rows
    .into_iter()
    .map(|(type_id, target_quantity, have_quantity)| StockpileItemFill {
      have_quantity,
      target_quantity,
      type_id,
    })
    .collect();
  Ok(Some(StockpileFill {
    items,
    stockpile_id: id,
  }))
}

pub async fn location_name(db: &Database, location_id: i64) -> Result<Option<String>, Error> {
  let name = sqlx::query_scalar::<_, String>(
    "SELECT name FROM stations WHERE id = ? \
    UNION ALL \
    SELECT name FROM structures WHERE id = ? \
    UNION ALL \
    SELECT name FROM solar_systems WHERE id = ? \
    UNION ALL \
    SELECT name FROM constellations WHERE id = ? \
    UNION ALL \
    SELECT name FROM regions WHERE id = ? \
    LIMIT 1",
  )
  .bind(location_id)
  .bind(location_id)
  .bind(location_id)
  .bind(location_id)
  .bind(location_id)
  .fetch_optional(&db.0)
  .await?;
  Ok(name)
}

/// Resolve a (possibly nested) location id to the id of its enclosing station / structure /
/// solar system by walking the asset container hierarchy across character and corporation
/// assets. Returns `None` when the id is not a tracked container item — callers fall back to a
/// direct lookup. Mirrors the recursive walk in `locations_for_items`.
pub async fn enclosing_location_id(db: &Database, location_id: i64) -> Result<Option<i64>, Error> {
  let enclosing = sqlx::query_scalar::<_, i64>(
    "WITH RECURSIVE assets(item_id, location_id, location_type) AS ( \
      SELECT item_id, location_id, location_type FROM character_assets \
      UNION ALL \
      SELECT item_id, location_id, location_type FROM corporation_assets \
    ), \
    loc(location_id, location_type) AS ( \
      SELECT location_id, location_type FROM assets WHERE item_id = ?1 \
      UNION ALL \
      SELECT a.location_id, a.location_type FROM loc \
        JOIN assets a ON a.item_id = loc.location_id \
        WHERE loc.location_type = 'item' \
    ) \
    SELECT location_id FROM loc WHERE location_type <> 'item' LIMIT 1",
  )
  .bind(location_id)
  .fetch_optional(&db.0)
  .await?;
  Ok(enclosing)
}

async fn insert_items(
  tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
  stockpile_id: i64,
  items: &[(i64, i64)],
) -> Result<Vec<StockpileItem>, Error> {
  let mut inserted = Vec::with_capacity(items.len());
  for &(type_id, target_quantity) in items {
    let item = sqlx::query_as::<_, StockpileItem>(
      "INSERT INTO stockpile_items (stockpile_id, type_id, target_quantity) VALUES (?, ?, ?) \
      RETURNING id, stockpile_id, target_quantity, type_id",
    )
    .bind(stockpile_id)
    .bind(type_id)
    .bind(target_quantity)
    .fetch_one(&mut **tx)
    .await?;
    inserted.push(item);
  }
  Ok(inserted)
}

pub async fn create_saved_filter(
  db: &Database,
  name: &str,
  query: &str,
  category: Option<&str>,
) -> Result<SavedAssetFilter, Error> {
  let filter = sqlx::query_as::<_, SavedAssetFilter>(
    "INSERT INTO saved_asset_filters (name, query, category) VALUES (?, ?, ?) \
    RETURNING category, id, name, query",
  )
  .bind(name)
  .bind(query)
  .bind(category)
  .fetch_one(&db.0)
  .await?;
  Ok(filter)
}

pub async fn delete_saved_filter(db: &Database, id: i64) -> Result<(), Error> {
  sqlx::query("DELETE FROM saved_asset_filters WHERE id = ?")
    .bind(id)
    .execute(db.writer())
    .await?;
  Ok(())
}

pub async fn saved_filters(db: &Database) -> Result<Vec<SavedAssetFilter>, Error> {
  let rows =
    sqlx::query_as::<_, SavedAssetFilter>("SELECT category, id, name, query FROM saved_asset_filters ORDER BY id ASC")
      .fetch_all(&db.0)
      .await?;
  Ok(rows)
}

#[cfg(test)]
mod abyssal_tests {
  use super::*;
  use crate::store::{
    self,
    model::{Alliance, Bloodline, Character, Corporation, Gender, Race},
    repo::character::insert_with_org,
  };

  async fn seed_character(db: &Database, id: i64) {
    let corp_id = 90_000_001;
    let alliance_id = 99_000_001;
    let alliance = Alliance::new(alliance_id, corp_id, id, "2003-01-01", "Test Alliance", "TST");
    let race = Race::new(2, alliance_id, "A race.", "Caldari");
    let mut corp = Corporation::new(corp_id, "Test Corp", "TSC");
    corp.set_ceo_id(id);
    corp.set_creator_id(id);
    corp.set_member_count(1);
    corp.set_tax_rate(0.0);
    let bloodline = Bloodline::new(1, corp_id, 2, 3, "A bloodline.", 4, 5, "Civire", 4, 4);
    let character = Character::new(id, 1, corp_id, 2, "2003-05-12", Gender::Male, "Pilot");
    insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
      .await
      .unwrap();
  }

  fn item(item_id: i64, character_id: i64) -> AbyssalItem {
    AbyssalItem::new(
      item_id,
      character_id,
      47_408,
      5975,
      47_297,
      r#"[{"attribute_id":6,"value":450.0}]"#.to_owned(),
      1_700_000_000,
    )
  }

  mod count_for_characters {
    use std::collections::HashMap;

    use pretty_assertions::assert_eq;

    use super::*;

    fn item_with(item_id: i64, character_id: i64, type_id: i64, dogma: &str) -> AbyssalItem {
      AbyssalItem::new(
        item_id,
        character_id,
        type_id,
        47_408,
        47_297,
        dogma.to_owned(),
        1_700_000_000,
      )
    }

    #[tokio::test]
    async fn it_counts_every_matching_item_minus_pagination() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      upsert(&db, &item_with(1, 42, 2410, r#"[{"attribute_id":50,"value":41.0}]"#))
        .await
        .unwrap();
      upsert(&db, &item_with(2, 42, 2281, r#"[{"attribute_id":50,"value":12.0}]"#))
        .await
        .unwrap();

      let count = count_for_characters(&db, &[42], None, &HashMap::new()).await.unwrap();
      let page = page_for_characters(&db, &[42], None, &HashMap::new(), None, None)
        .await
        .unwrap();

      assert_eq!(count, 2);
      assert_eq!(
        count,
        page.len() as i64,
        "the count matches the unpaginated page length"
      );
    }

    #[tokio::test]
    async fn it_is_zero_for_no_characters() {
      let db = store::open_test().await.unwrap();

      assert_eq!(count_for_characters(&db, &[], None, &HashMap::new()).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn it_mirrors_the_rolled_type_filter() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      upsert(&db, &item_with(1, 42, 2410, r#"[{"attribute_id":50,"value":41.0}]"#))
        .await
        .unwrap();
      upsert(&db, &item_with(2, 42, 2281, r#"[{"attribute_id":50,"value":12.0}]"#))
        .await
        .unwrap();

      let count = count_for_characters(&db, &[42], Some(2410), &HashMap::new())
        .await
        .unwrap();
      let page = page_for_characters(&db, &[42], Some(2410), &HashMap::new(), None, None)
        .await
        .unwrap();

      assert_eq!(count, 1);
      assert_eq!(count, page.len() as i64);
    }

    #[tokio::test]
    async fn it_mirrors_the_stat_range_filter() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      upsert(&db, &item_with(1, 42, 2410, r#"[{"attribute_id":50,"value":41.0}]"#))
        .await
        .unwrap();
      upsert(&db, &item_with(2, 42, 2410, r#"[{"attribute_id":50,"value":55.0}]"#))
        .await
        .unwrap();
      let mut ranges = HashMap::new();
      ranges.insert(
        50,
        StatRange {
          max: 50.0,
          min: 40.0,
        },
      );

      let count = count_for_characters(&db, &[42], None, &ranges).await.unwrap();
      let page = page_for_characters(&db, &[42], None, &ranges, None, None)
        .await
        .unwrap();

      assert_eq!(count, 1);
      assert_eq!(count, page.len() as i64);
    }
  }

  mod delete_stale {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_clears_all_rows_when_keep_set_is_empty() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      upsert(&db, &item(1, 42)).await.unwrap();

      delete_stale(&db, 42, &[]).await.unwrap();

      assert!(for_character_abyssal(&db, 42).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_deletes_items_not_in_the_keep_set() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      upsert(&db, &item(1, 42)).await.unwrap();
      upsert(&db, &item(2, 42)).await.unwrap();
      upsert(&db, &item(3, 42)).await.unwrap();

      delete_stale(&db, 42, &[1, 3]).await.unwrap();

      let ids: Vec<i64> = for_character_abyssal(&db, 42)
        .await
        .unwrap()
        .iter()
        .map(AbyssalItem::item_id)
        .collect();
      assert_eq!(ids, [1, 3]);
    }

    #[tokio::test]
    async fn it_leaves_other_characters_untouched() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_character(&db, 43).await;
      upsert(&db, &item(1, 42)).await.unwrap();
      upsert(&db, &item(2, 43)).await.unwrap();

      delete_stale(&db, 42, &[]).await.unwrap();

      assert!(for_character_abyssal(&db, 42).await.unwrap().is_empty());
      assert_eq!(for_character_abyssal(&db, 43).await.unwrap().len(), 1);
    }
  }

  mod filtered_for_characters {
    use std::collections::HashMap;

    use pretty_assertions::assert_eq;

    use super::*;

    fn item_with_dogma(item_id: i64, character_id: i64, type_id: i64, dogma: &str) -> AbyssalItem {
      AbyssalItem::new(
        item_id,
        character_id,
        type_id,
        47_408,
        47_297,
        dogma.to_owned(),
        1_700_000_000,
      )
    }

    #[tokio::test]
    async fn a_boundary_roll_is_included() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      upsert(
        &db,
        &item_with_dogma(1, 42, 2410, r#"[{"attribute_id":50,"value":41.0}]"#),
      )
      .await
      .unwrap();
      let mut ranges = HashMap::new();
      ranges.insert(
        50,
        StatRange {
          max: 41.0,
          min: 41.0,
        },
      );

      let rows = filtered_for_characters(&db, &[42], None, &ranges).await.unwrap();

      assert_eq!(rows.len(), 1);
    }

    #[tokio::test]
    async fn it_filters_by_rolled_type() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      upsert(
        &db,
        &item_with_dogma(1, 42, 2410, r#"[{"attribute_id":50,"value":41.0}]"#),
      )
      .await
      .unwrap();
      upsert(
        &db,
        &item_with_dogma(2, 42, 2281, r#"[{"attribute_id":50,"value":12.0}]"#),
      )
      .await
      .unwrap();

      let rows = filtered_for_characters(&db, &[42], Some(2410), &HashMap::new())
        .await
        .unwrap();

      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].item_id(), 1);
    }

    #[tokio::test]
    async fn it_filters_by_stat_range_over_json_each() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      upsert(
        &db,
        &item_with_dogma(1, 42, 2410, r#"[{"attribute_id":50,"value":41.0}]"#),
      )
      .await
      .unwrap();
      upsert(
        &db,
        &item_with_dogma(2, 42, 2410, r#"[{"attribute_id":50,"value":55.0}]"#),
      )
      .await
      .unwrap();
      let mut ranges = HashMap::new();
      ranges.insert(
        50,
        StatRange {
          max: 50.0,
          min: 40.0,
        },
      );

      let rows = filtered_for_characters(&db, &[42], None, &ranges).await.unwrap();

      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].item_id(), 1);
    }

    #[tokio::test]
    async fn it_is_empty_for_no_characters() {
      let db = store::open_test().await.unwrap();

      assert!(
        filtered_for_characters(&db, &[], None, &HashMap::new())
          .await
          .unwrap()
          .is_empty()
      );
    }

    #[tokio::test]
    async fn it_returns_every_item_with_no_filters() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      upsert(
        &db,
        &item_with_dogma(1, 42, 2410, r#"[{"attribute_id":50,"value":41.0}]"#),
      )
      .await
      .unwrap();
      upsert(
        &db,
        &item_with_dogma(2, 42, 2281, r#"[{"attribute_id":50,"value":12.0}]"#),
      )
      .await
      .unwrap();

      let rows = filtered_for_characters(&db, &[42], None, &HashMap::new())
        .await
        .unwrap();

      assert_eq!(rows.len(), 2);
    }
  }

  mod for_character {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_filters_by_character() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_character(&db, 43).await;
      upsert(&db, &item(1, 42)).await.unwrap();
      upsert(&db, &item(2, 43)).await.unwrap();

      let rows = for_character_abyssal(&db, 42).await.unwrap();

      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].character_id(), 42);
    }

    #[tokio::test]
    async fn it_returns_empty_when_none_exist() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;

      assert!(for_character_abyssal(&db, 42).await.unwrap().is_empty());
    }
  }

  mod locations_for_items {
    use pretty_assertions::assert_eq;

    use super::*;

    async fn disable_foreign_keys(db: &Database) {
      sqlx::query("PRAGMA foreign_keys = OFF")
        .execute(db.writer())
        .await
        .unwrap();
    }

    async fn insert_station(db: &Database, id: i64, name: &str) {
      sqlx::query(
        "INSERT INTO stations \
          (id, system_id, type_id, name, max_dockable_ship_volume, office_rental_cost, \
          reprocessing_efficiency, reprocessing_stations_take, position_x, position_y, position_z) \
        VALUES (?, 0, 0, ?, 0, 0, 0, 0, 0, 0, 0)",
      )
      .bind(id)
      .bind(name)
      .execute(db.writer())
      .await
      .unwrap();
    }

    async fn insert_asset(db: &Database, item_id: i64, location_id: i64, location_type: &str) {
      sqlx::query(
        "INSERT INTO character_assets \
          (item_id, character_id, type_id, location_id, location_type, location_flag, quantity) \
        VALUES (?, 42, 0, ?, ?, 'Hangar', 1)",
      )
      .bind(item_id)
      .bind(location_id)
      .bind(location_type)
      .execute(db.writer())
      .await
      .unwrap();
    }

    #[tokio::test]
    async fn enclosing_location_id_is_none_for_an_untracked_id() {
      let db = store::open_test().await.unwrap();
      disable_foreign_keys(&db).await;

      assert_eq!(enclosing_location_id(&db, 60_000_001).await.unwrap(), None);
    }

    #[tokio::test]
    async fn enclosing_location_id_walks_a_container_up_to_its_station() {
      // A blueprint's location_id points at a hangar container; the container is itself an asset
      // docked in a station. The walk returns the station id so the caller can name it.
      let db = store::open_test().await.unwrap();
      disable_foreign_keys(&db).await;
      insert_asset(&db, 1000, 60_000_001, "station").await;
      insert_asset(&db, 2000, 1000, "item").await;

      assert_eq!(enclosing_location_id(&db, 1000).await.unwrap(), Some(60_000_001));
      assert_eq!(enclosing_location_id(&db, 2000).await.unwrap(), Some(60_000_001));
    }

    #[tokio::test]
    async fn it_omits_items_with_no_resolvable_location() {
      let db = store::open_test().await.unwrap();
      disable_foreign_keys(&db).await;
      insert_asset(&db, 99, 70_000_000, "structure").await;

      let locations = locations_for_items(&db, &[99]).await.unwrap();

      assert!(locations.is_empty());
    }

    #[tokio::test]
    async fn it_resolves_a_directly_docked_item_to_its_station() {
      let db = store::open_test().await.unwrap();
      disable_foreign_keys(&db).await;
      insert_station(&db, 60_000_001, "Jita IV - Moon 4").await;
      insert_asset(&db, 99, 60_000_001, "station").await;

      let locations = locations_for_items(&db, &[99]).await.unwrap();

      assert_eq!(locations.get(&99).map(String::as_str), Some("Jita IV - Moon 4"));
    }

    #[tokio::test]
    async fn it_walks_a_nested_item_up_to_its_root_station() {
      let db = store::open_test().await.unwrap();
      disable_foreign_keys(&db).await;
      insert_station(&db, 60_000_001, "Jita IV - Moon 4").await;
      insert_asset(&db, 1000, 60_000_001, "station").await;
      insert_asset(&db, 99, 1000, "item").await;

      let locations = locations_for_items(&db, &[99]).await.unwrap();

      assert_eq!(locations.get(&99).map(String::as_str), Some("Jita IV - Moon 4"));
    }
  }

  mod module_stats {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_empty_for_an_unknown_type() {
      let db = store::open_test().await.unwrap();

      assert!(module_stats_for_type(&db, 99_999).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_upserts_lists_and_refreshes_bounds() {
      let db = store::open_test().await.unwrap();
      upsert_module_stats(
        &db,
        &[
          AbyssalModuleStat::new(47_408, 6, 0.6, 1.4),
          AbyssalModuleStat::new(47_408, 20, 0.9, 1.1),
        ],
      )
      .await
      .unwrap();

      let rows = module_stats_for_type(&db, 47_408).await.unwrap();
      assert_eq!(rows.len(), 2);
      assert_eq!(rows[0].attribute_id(), 6);

      upsert_module_stats(&db, &[AbyssalModuleStat::new(47_408, 6, 0.5, 1.5)])
        .await
        .unwrap();

      let rows = module_stats_for_type(&db, 47_408).await.unwrap();
      assert_eq!(rows.len(), 2);
      let attr6 = rows.iter().find(|r| r.attribute_id() == 6).unwrap();
      assert_eq!(attr6.min_mult(), 0.5);
      assert_eq!(attr6.max_mult(), 1.5);
    }
  }

  mod page_for_characters {
    use std::collections::HashMap;

    use pretty_assertions::assert_eq;

    use super::*;

    /// `item_id`, `type_id` and `source_type_id` are all distinct so ordering and
    /// the keyset cursor can be asserted unambiguously.
    fn item(item_id: i64, character_id: i64, source_type_id: i64) -> AbyssalItem {
      AbyssalItem::new(
        item_id,
        character_id,
        2410,
        source_type_id,
        47_297,
        r#"[{"attribute_id":50,"value":41.0}]"#.to_owned(),
        1_700_000_000,
      )
    }

    #[tokio::test]
    async fn it_caps_a_page_at_the_limit() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      for id in 1..=5 {
        upsert(&db, &item(id, 42, 100)).await.unwrap();
      }

      let rows = page_for_characters(&db, &[42], None, &HashMap::new(), None, Some(2))
        .await
        .unwrap();

      assert_eq!(rows.len(), 2);
      assert_eq!(rows[0].item_id(), 1);
      assert_eq!(rows[1].item_id(), 2);
    }

    #[tokio::test]
    async fn it_is_empty_for_no_characters() {
      let db = store::open_test().await.unwrap();

      assert!(
        page_for_characters(&db, &[], None, &HashMap::new(), None, Some(50))
          .await
          .unwrap()
          .is_empty()
      );
    }

    #[tokio::test]
    async fn it_orders_by_source_type_then_item_so_groups_stay_contiguous() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      // Interleave source types so insertion order can't accidentally satisfy the assert.
      upsert(&db, &item(3, 42, 100)).await.unwrap();
      upsert(&db, &item(1, 42, 200)).await.unwrap();
      upsert(&db, &item(2, 42, 100)).await.unwrap();
      upsert(&db, &item(4, 42, 200)).await.unwrap();

      let rows = page_for_characters(&db, &[42], None, &HashMap::new(), None, None)
        .await
        .unwrap();

      let keys: Vec<(i64, i64)> = rows.iter().map(|r| (r.source_type_id(), r.item_id())).collect();
      assert_eq!(keys, vec![(100, 2), (100, 3), (200, 1), (200, 4)]);
    }

    #[tokio::test]
    async fn it_resumes_strictly_after_the_cursor() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      upsert(&db, &item(1, 42, 100)).await.unwrap();
      upsert(&db, &item(2, 42, 100)).await.unwrap();
      upsert(&db, &item(3, 42, 200)).await.unwrap();

      let cursor = AbyssalCursor {
        item_id: 2,
        source_type_id: 100,
      };
      let rows = page_for_characters(&db, &[42], None, &HashMap::new(), Some(cursor), Some(10))
        .await
        .unwrap();

      // Only items after (100, 2) remain: (200, 3).
      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].item_id(), 3);
    }

    #[tokio::test]
    async fn it_walks_a_smaller_source_type_cursor_within_the_same_group() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      upsert(&db, &item(10, 42, 100)).await.unwrap();
      upsert(&db, &item(20, 42, 100)).await.unwrap();

      let cursor = AbyssalCursor {
        item_id: 10,
        source_type_id: 100,
      };
      let rows = page_for_characters(&db, &[42], None, &HashMap::new(), Some(cursor), Some(10))
        .await
        .unwrap();

      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].item_id(), 20);
    }
  }

  mod source_type_filters {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{
      model::{ItemCategory, ItemGroup, ItemType},
      repo::sde,
    };

    async fn seed_source_type(db: &Database, type_id: i64, name: &str, category: (i64, &str)) {
      let (category_id, category_name) = category;
      let group_id = category_id * 10;
      let cat = ItemCategory {
        icon_id: None,
        id: category_id,
        name: category_name.to_owned(),
        published: true,
      };
      let group = ItemGroup {
        category_id,
        icon_id: None,
        id: group_id,
        name: "Group".to_owned(),
        published: true,
      };
      let item_type = ItemType {
        capacity: None,
        description: Some("A module.".to_owned()),
        dogma_attributes: "[]".to_owned(),
        group_id,
        icon_id: None,
        id: type_id,
        market_group_id: None,
        name: name.to_owned(),
        packaged_volume: None,
        portion_size: None,
        published: true,
        radius: None,
        volume: None,
      };
      sde::insert_item_type_with_hierarchy(db, &item_type, &group, &cat)
        .await
        .unwrap();
    }

    fn item_with_source(item_id: i64, character_id: i64, source_type_id: i64) -> AbyssalItem {
      AbyssalItem::new(
        item_id,
        character_id,
        47_408,
        source_type_id,
        47_297,
        "[]".to_owned(),
        1_700_000_000,
      )
    }

    #[tokio::test]
    async fn it_exposes_the_distinct_set_of_seeded_types_as_abyssal_type_ids() {
      let db = store::open_test().await.unwrap();
      upsert_module_stats(
        &db,
        &[
          AbyssalModuleStat::new(47_408, 6, 0.6, 1.4),
          AbyssalModuleStat::new(47_408, 20, 0.9, 1.1),
          AbyssalModuleStat::new(47_410, 6, 0.7, 1.3),
        ],
      )
      .await
      .unwrap();

      let mut ids = abyssal_type_ids(&db).await.unwrap();
      ids.sort_unstable();

      assert_eq!(ids, [47_408, 47_410]);
    }

    #[tokio::test]
    async fn it_groups_distinct_owned_source_types_by_category_ordered_by_name() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_source_type(&db, 2048, "Damage Control II", (7, "Module")).await;
      seed_source_type(&db, 2281, "Adaptive Invulnerability Field II", (7, "Module")).await;
      upsert(&db, &item_with_source(1, 42, 2048)).await.unwrap();
      upsert(&db, &item_with_source(2, 42, 2048)).await.unwrap();
      upsert(&db, &item_with_source(3, 42, 2281)).await.unwrap();

      let filters = source_type_filters(&db, &[42]).await.unwrap();

      assert_eq!(filters.len(), 2);
      assert_eq!(filters[0].source_type_name, "Adaptive Invulnerability Field II");
      assert_eq!(filters[0].category, "Module");
      assert_eq!(filters[1].source_type_name, "Damage Control II");
    }

    #[tokio::test]
    async fn it_is_empty_for_no_characters() {
      let db = store::open_test().await.unwrap();

      assert!(source_type_filters(&db, &[]).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_only_offers_types_the_given_characters_own() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_character(&db, 43).await;
      seed_source_type(&db, 2048, "Damage Control II", (7, "Module")).await;
      seed_source_type(&db, 2281, "Adaptive Invulnerability Field II", (7, "Module")).await;
      upsert(&db, &item_with_source(1, 42, 2048)).await.unwrap();
      upsert(&db, &item_with_source(2, 43, 2281)).await.unwrap();

      let filters = source_type_filters(&db, &[42]).await.unwrap();

      assert_eq!(filters.len(), 1);
      assert_eq!(filters[0].source_type_id, 2048);
    }
  }

  mod stat_templates_for_owned_type {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{
      model::{DogmaAttribute, ItemCategory, ItemGroup, ItemType},
      repo::sde,
    };

    async fn seed_module_dogma(db: &Database, type_id: i64, name: &str, dogma: &str) {
      let cat = ItemCategory {
        icon_id: None,
        id: 7,
        name: "Module".to_owned(),
        published: true,
      };
      let group = ItemGroup {
        category_id: 7,
        icon_id: None,
        id: 70,
        name: "Group".to_owned(),
        published: true,
      };
      let item_type = ItemType {
        capacity: None,
        description: Some("A module.".to_owned()),
        dogma_attributes: dogma.to_owned(),
        group_id: 70,
        icon_id: None,
        id: type_id,
        market_group_id: None,
        name: name.to_owned(),
        packaged_volume: None,
        portion_size: None,
        published: true,
        radius: None,
        volume: None,
      };
      sde::insert_item_type_with_hierarchy(db, &item_type, &group, &cat)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn it_derives_bounds_from_the_owned_items_source_module() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_module_dogma(&db, 2488, "Source Module", r#"[{"attribute_id":50,"value":100.0}]"#).await;
      upsert_module_stats(&db, &[AbyssalModuleStat::new(47_408, 50, 0.8, 1.2)])
        .await
        .unwrap();
      sde::upsert_many_dogma_attributes(
        &db,
        &[DogmaAttribute {
          attribute_id: 50,
          default_value: None,
          description: None,
          display_name: Some("CPU Output".to_owned()),
          high_is_good: true,
          icon_id: None,
          name: "cpuOutput".to_owned(),
          published: true,
          stackable: false,
          unit_id: Some(115),
        }],
      )
      .await
      .unwrap();
      upsert(
        &db,
        &AbyssalItem::new(
          900,
          42,
          47_408,
          2488,
          47_297,
          r#"[{"attribute_id":50,"value":95.0}]"#.to_owned(),
          1_700_000_000,
        ),
      )
      .await
      .unwrap();

      let rolled = stat_templates_for_type(&db, 47_408).await.unwrap();
      assert_eq!(rolled.len(), 1);
      assert_eq!(rolled[0].base_value, 0.0);
      assert_eq!(rolled[0].bound_lo, 0.0);
      assert_eq!(rolled[0].bound_hi, 0.0);

      let templates = stat_templates_for_owned_type(&db, &[42], 47_408).await.unwrap();

      assert_eq!(templates.len(), 1);
      assert_eq!(templates[0].base_value, 100.0);
      assert_eq!(templates[0].bound_lo, 80.0);
      assert_eq!(templates[0].bound_hi, 120.0);
    }
  }

  mod stat_templates_for_type {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{
      model::{DogmaAttribute, ItemCategory, ItemGroup, ItemType},
      repo::sde,
    };

    async fn seed_typed_dogma(db: &Database, type_id: i64, dogma: &str) {
      let cat = ItemCategory {
        icon_id: None,
        id: 7,
        name: "Module".to_owned(),
        published: true,
      };
      let group = ItemGroup {
        category_id: 7,
        icon_id: None,
        id: 70,
        name: "Group".to_owned(),
        published: true,
      };
      let item_type = ItemType {
        capacity: None,
        description: Some("A module.".to_owned()),
        dogma_attributes: dogma.to_owned(),
        group_id: 70,
        icon_id: None,
        id: type_id,
        market_group_id: None,
        name: "Source Module".to_owned(),
        packaged_volume: None,
        portion_size: None,
        published: true,
        radius: None,
        volume: None,
      };
      sde::insert_item_type_with_hierarchy(db, &item_type, &group, &cat)
        .await
        .unwrap();
    }

    fn attribute(
      attribute_id: i64,
      name: &str,
      display: &str,
      unit_id: Option<i64>,
      high_is_good: bool,
    ) -> DogmaAttribute {
      DogmaAttribute {
        attribute_id,
        default_value: None,
        description: None,
        display_name: Some(display.to_owned()),
        high_is_good,
        icon_id: None,
        name: name.to_owned(),
        published: true,
        stackable: false,
        unit_id,
      }
    }

    #[tokio::test]
    async fn it_derives_bounds_from_base_dogma_times_mult_with_resolved_metadata() {
      let db = store::open_test().await.unwrap();
      seed_typed_dogma(&db, 47_408, r#"[{"attribute_id":50,"value":100.0}]"#).await;
      upsert_module_stats(&db, &[AbyssalModuleStat::new(47_408, 50, 0.8, 1.2)])
        .await
        .unwrap();
      sde::upsert_many_dogma_attributes(&db, &[attribute(50, "cpuOutput", "CPU Output", Some(115), true)])
        .await
        .unwrap();

      let templates = stat_templates_for_type(&db, 47_408).await.unwrap();

      assert_eq!(templates.len(), 1);
      assert_eq!(templates[0].attribute_id, 50);
      assert_eq!(templates[0].base_value, 100.0);
      assert_eq!(templates[0].bound_lo, 80.0);
      assert_eq!(templates[0].bound_hi, 120.0);
      assert_eq!(templates[0].display_name, "CPU Output");
      assert_eq!(templates[0].unit_id, Some(115));
      assert_eq!(templates[0].high_is_good, true);
    }

    #[tokio::test]
    async fn it_is_empty_when_the_type_has_no_module_stats() {
      let db = store::open_test().await.unwrap();

      assert!(stat_templates_for_type(&db, 99_999).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_orders_templates_by_display_name() {
      let db = store::open_test().await.unwrap();
      seed_typed_dogma(
        &db,
        47_408,
        r#"[{"attribute_id":50,"value":10.0},{"attribute_id":51,"value":20.0}]"#,
      )
      .await;
      upsert_module_stats(
        &db,
        &[
          AbyssalModuleStat::new(47_408, 50, 0.9, 1.1),
          AbyssalModuleStat::new(47_408, 51, 0.9, 1.1),
        ],
      )
      .await
      .unwrap();
      sde::upsert_many_dogma_attributes(
        &db,
        &[
          attribute(50, "zulu", "Zulu", None, true),
          attribute(51, "alpha", "Alpha", None, true),
        ],
      )
      .await
      .unwrap();

      let templates = stat_templates_for_type(&db, 47_408).await.unwrap();

      assert_eq!(templates[0].display_name, "Alpha");
      assert_eq!(templates[1].display_name, "Zulu");
    }
  }

  mod update_price {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_sets_only_the_price_fields() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      upsert(&db, &item(1, 42)).await.unwrap();

      update_price(&db, 1, Some(1_500_000_000.0), 1_700_000_100)
        .await
        .unwrap();

      let rows = for_character_abyssal(&db, 42).await.unwrap();
      assert_eq!(rows[0].muta_price_isk(), Some(1_500_000_000.0));
      assert_eq!(rows[0].muta_price_synced(), Some(1_700_000_100));
      assert_eq!(rows[0].synced_at(), 1_700_000_000);
    }
  }

  mod upsert {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_inserts_a_record() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;

      upsert(&db, &item(100, 42)).await.unwrap();

      let rows = for_character_abyssal(&db, 42).await.unwrap();
      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].item_id(), 100);
      assert_eq!(rows[0].muta_price_isk(), None);
    }

    #[tokio::test]
    async fn it_updates_on_conflict() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      upsert(&db, &item(100, 42)).await.unwrap();
      let mut updated = item(100, 42);
      updated.synced_at = 1_700_000_500;

      upsert(&db, &updated).await.unwrap();

      let rows = for_character_abyssal(&db, 42).await.unwrap();
      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].synced_at(), 1_700_000_500);
    }
  }
}

#[cfg(test)]
mod asset_tests {
  use super::*;
  use crate::store::{
    self,
    model::{
      Alliance, Bloodline, Character, Constellation, Corporation, CorporationMemberRole, Gender, ItemCategory,
      ItemGroup, ItemType, OwnerType, Race, Region, SolarSystem, Station, Structure,
    },
    repo::{character::insert_with_org, infra, org, sde},
  };

  const CORP_ID: i64 = 90_000_001;

  const DIRECTOR_ID: i64 = 42;

  const GEO_SYSTEM: i64 = 30_000_142;

  async fn seed_geography(db: &Database) {
    sde::upsert_region(
      db,
      &Region {
        description: None,
        id: 10_000_002,
        name: "The Forge".to_owned(),
      },
    )
    .await
    .unwrap();
    sde::upsert_constellation(
      db,
      &Constellation {
        id: 20_000_020,
        name: "Kimotoro".to_owned(),
        position_x: 0.0,
        position_y: 0.0,
        position_z: 0.0,
        region_id: 10_000_002,
      },
    )
    .await
    .unwrap();
    sde::upsert_solar_system(
      db,
      &SolarSystem {
        constellation_id: 20_000_020,
        id: GEO_SYSTEM,
        name: "Jita".to_owned(),
        position_x: 0.0,
        position_y: 0.0,
        position_z: 0.0,
        security_class: None,
        security_status: 0.9,
        star_id: None,
      },
    )
    .await
    .unwrap();
  }

  async fn seed_named_station(db: &Database, station_id: i64, name: &str) {
    seed_geography(db).await;
    sde::upsert_station(
      db,
      &Station {
        id: station_id,
        max_dockable_ship_volume: 0.0,
        name: name.to_owned(),
        office_rental_cost: 0.0,
        owner: None,
        position_x: 0.0,
        position_y: 0.0,
        position_z: 0.0,
        race_id: None,
        reprocessing_efficiency: 0.0,
        reprocessing_stations_take: 0.0,
        services: "[]".to_owned(),
        system_id: GEO_SYSTEM,
        type_id: 587,
      },
    )
    .await
    .unwrap();
  }

  async fn seed_named_structure(db: &Database, structure_id: i64, name: &str) {
    seed_geography(db).await;
    sde::upsert_structure(
      db,
      &Structure {
        id: structure_id,
        name: name.to_owned(),
        owner_id: CORP_ID,
        position_x: None,
        position_y: None,
        position_z: None,
        solar_system_id: GEO_SYSTEM,
        type_id: None,
      },
    )
    .await
    .unwrap();
  }

  async fn authorize_corp(db: &Database) {
    infra::upsert(
      db,
      CORP_ID,
      OwnerType::Corporation,
      "tok",
      "rt",
      4_102_444_800,
      Some(DIRECTOR_ID),
      None,
    )
    .await
    .unwrap();
    org::replace_for_corporation(
      db,
      CORP_ID,
      &[CorporationMemberRole::from((
        CORP_ID,
        DIRECTOR_ID,
        "Director".to_string(),
      ))],
    )
    .await
    .unwrap();
  }

  async fn seed_item_type(db: &Database, type_id: i64, name: &str, group_id: i64, group_name: &str, cat: &str) {
    let category = ItemCategory {
      id: group_id * 10,
      icon_id: None,
      name: cat.to_owned(),
      published: true,
    };
    let group = ItemGroup {
      category_id: category.id(),
      icon_id: None,
      id: group_id,
      name: group_name.to_owned(),
      published: true,
    };
    sde::upsert_item_category(db, &category).await.unwrap();
    sde::upsert_item_group(db, &group).await.unwrap();
    sqlx::query(
      "INSERT INTO item_types (id, group_id, description, name, published, icon_id, packaged_volume, volume) \
      VALUES (?, ?, ?, ?, 1, ?, 2.5, 27289.0)",
    )
    .bind(type_id)
    .bind(group_id)
    .bind("Test item")
    .bind(name)
    .bind(type_id + 1000)
    .execute(db.writer())
    .await
    .unwrap();
  }

  async fn seed_character(db: &Database, id: i64) {
    let alliance_id = 99_000_001;
    let alliance = Alliance::new(alliance_id, CORP_ID, id, "2003-01-01", "Test Alliance", "TST");
    let race = Race::new(2, alliance_id, "A race.", "Caldari");
    let mut corp = Corporation::new(CORP_ID, "Test Corp", "TSC");
    corp.set_ceo_id(id);
    corp.set_creator_id(id);
    corp.set_member_count(1);
    corp.set_tax_rate(0.0);
    let bloodline = Bloodline::new(1, CORP_ID, 2, 3, "A bloodline.", 4, 5, "Civire", 4, 4);
    let character = Character::new(id, 1, CORP_ID, 2, "2003-05-12", Gender::Male, "Pilot");
    insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
      .await
      .unwrap();
  }

  fn char_asset(item_id: i64, character_id: i64, container_id: Option<i64>) -> CharacterAsset {
    CharacterAsset {
      character_id,
      container_id,
      depth: container_id.map_or(0, |_| 1),
      is_active_ship: false,
      is_blueprint_copy: None,
      is_container: false,
      is_singleton: false,
      item_id,
      location_flag: "Hangar".to_owned(),
      location_id: 60_003_760,
      location_type: "station".to_owned(),
      name: None,
      quantity: 1,
      type_id: 587,
    }
  }

  fn corp_asset(item_id: i64, corporation_id: i64, container_id: Option<i64>) -> CorporationAsset {
    CorporationAsset {
      container_id,
      corporation_id,
      depth: container_id.map_or(0, |_| 1),
      is_blueprint_copy: None,
      is_container: false,
      is_singleton: false,
      item_id,
      location_flag: "CorpDeliveries".to_owned(),
      location_id: 60_003_760,
      location_type: "station".to_owned(),
      name: None,
      quantity: 1,
      type_id: 587,
    }
  }

  async fn seed_price(db: &Database, type_id: i64, adjusted: f64) {
    sqlx::query("INSERT INTO market_prices (type_id, adjusted_price, average_price) VALUES (?, ?, NULL)")
      .bind(type_id)
      .bind(adjusted)
      .execute(db.writer())
      .await
      .unwrap();
  }

  async fn seed_history(db: &Database, type_id: i64, date: &str, close: f64) {
    sqlx::query("INSERT INTO type_price_histories (type_id, date, open, high, low, close) VALUES (?, ?, ?, ?, ?, ?)")
      .bind(type_id)
      .bind(date)
      .bind(close)
      .bind(close)
      .bind(close)
      .bind(close)
      .execute(db.writer())
      .await
      .unwrap();
  }

  mod ancestors_of_match {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_computes_corporation_ancestors() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      authorize_corp(&db).await;
      seed_item_type(&db, 24, "Tritanium", 18, "Mineral", "Mineral").await;
      let mut root = corp_asset(100, CORP_ID, None);
      root.is_container = true;
      root.type_id = 24;
      let mut hit = corp_asset(101, CORP_ID, Some(100));
      hit.type_id = 24;
      replace_for_corporation(&db, CORP_ID, &[root, hit]).await.unwrap();

      let ancestors = ancestors_of_match_for_corporation(&db, CORP_ID, "name:trit", None)
        .await
        .unwrap();

      assert_eq!(ancestors, [100]);
    }

    #[tokio::test]
    async fn it_dedups_a_shared_ancestor_across_two_matches() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_item_type(&db, 24, "Tritanium", 18, "Mineral", "Mineral").await;
      let mut root = char_asset(100, 42, None);
      root.is_container = true;
      root.type_id = 24;
      let mut hit_a = char_asset(101, 42, Some(100));
      hit_a.type_id = 24;
      let mut hit_b = char_asset(102, 42, Some(100));
      hit_b.type_id = 24;
      replace_for_character(&db, 42, &[root, hit_a, hit_b]).await.unwrap();

      let ancestors = ancestors_of_match_for_character(&db, 42, "category:material", None)
        .await
        .unwrap();

      assert_eq!(ancestors, [100]);
    }

    #[tokio::test]
    async fn it_excludes_a_top_level_match_with_no_ancestors() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_item_type(&db, 587, "Rifter", 25, "Frigate", "Ship").await;
      let mut top = char_asset(100, 42, None);
      top.type_id = 587;
      replace_for_character(&db, 42, &[top]).await.unwrap();

      let ancestors = ancestors_of_match_for_character(&db, 42, "category:ship", None)
        .await
        .unwrap();

      assert!(ancestors.is_empty());
    }

    #[tokio::test]
    async fn it_returns_an_empty_set_for_an_empty_filter() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_item_type(&db, 587, "Rifter", 25, "Frigate", "Ship").await;
      let mut root = char_asset(100, 42, None);
      root.is_container = true;
      root.type_id = 587;
      let mut child = char_asset(101, 42, Some(100));
      child.type_id = 587;
      replace_for_character(&db, 42, &[root, child]).await.unwrap();

      let ancestors = ancestors_of_match_for_character(&db, 42, "", None).await.unwrap();

      assert!(ancestors.is_empty());
    }

    #[tokio::test]
    async fn it_returns_the_ancestor_chain_for_a_hit_nested_two_levels_deep() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_item_type(&db, 587, "Rifter", 25, "Frigate", "Ship").await;
      seed_item_type(&db, 24, "Tritanium", 18, "Mineral", "Mineral").await;
      let mut root = char_asset(100, 42, None);
      root.is_container = true;
      root.type_id = 587;
      let mut sub = char_asset(101, 42, Some(100));
      sub.is_container = true;
      sub.type_id = 587;
      let mut hit = char_asset(102, 42, Some(101));
      hit.type_id = 24;
      let mut sibling = char_asset(200, 42, None);
      sibling.type_id = 587;
      replace_for_character(&db, 42, &[root, sub, hit, sibling])
        .await
        .unwrap();

      let ancestors = ancestors_of_match_for_character(&db, 42, "category:material", None)
        .await
        .unwrap();

      assert_eq!(ancestors, [100, 101]);
    }
  }

  mod asset_value_as_of {
    use pretty_assertions::assert_eq;

    use super::*;

    fn asset(item_id: i64, type_id: i64, quantity: i64) -> CharacterAsset {
      let mut a = char_asset(item_id, 42, None);
      a.type_id = type_id;
      a.quantity = quantity;
      a
    }

    #[tokio::test]
    async fn it_falls_back_to_the_current_snapshot_when_no_history_precedes_the_date() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      replace_for_character(&db, 42, &[asset(100, 587, 4)]).await.unwrap();
      seed_price(&db, 587, 7.0).await;
      seed_history(&db, 587, "2026-06-10", 50.0).await;

      let value = asset_value_as_of_for_character(&db, 42, "2026-06-01").await.unwrap();

      assert_eq!(
        value, 28.0,
        "pre-history dates fall back to the current market_prices snapshot"
      );
    }

    #[tokio::test]
    async fn it_forward_fills_from_the_nearest_prior_close_for_a_gap_date() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      replace_for_character(&db, 42, &[asset(100, 587, 2)]).await.unwrap();
      seed_history(&db, 587, "2026-06-01", 5.0).await;
      seed_history(&db, 587, "2026-06-05", 9.0).await;

      let value = asset_value_as_of_for_character(&db, 42, "2026-06-03").await.unwrap();

      assert_eq!(value, 10.0);
    }

    #[tokio::test]
    async fn it_prices_corporation_holdings_for_an_authorized_scope() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, DIRECTOR_ID).await;
      authorize_corp(&db).await;
      let mut a = corp_asset(100, CORP_ID, None);
      a.type_id = 587;
      a.quantity = 2;
      replace_for_corporation(&db, CORP_ID, &[a]).await.unwrap();
      seed_history(&db, 587, "2026-06-03", 12.0).await;

      let value = asset_value_as_of_for_corporation(&db, CORP_ID, "2026-06-03")
        .await
        .unwrap();

      assert_eq!(value, 24.0);
    }

    #[tokio::test]
    async fn it_prices_holdings_with_the_historical_close_on_the_exact_date() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      replace_for_character(&db, 42, &[asset(100, 587, 3)]).await.unwrap();
      seed_price(&db, 587, 100.0).await;
      seed_history(&db, 587, "2026-06-03", 10.0).await;

      let value = asset_value_as_of_for_character(&db, 42, "2026-06-03").await.unwrap();

      assert_eq!(value, 30.0, "uses the 06-03 close (10), not the current snapshot (100)");
    }

    #[tokio::test]
    async fn it_returns_zero_for_a_scope_with_no_holdings() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;

      let value = asset_value_as_of_for_character(&db, 42, "2026-06-03").await.unwrap();

      assert_eq!(value, 0.0);
    }

    #[tokio::test]
    async fn it_returns_zero_for_an_unauthorized_corporation_scope() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let mut a = corp_asset(100, CORP_ID, None);
      a.type_id = 587;
      replace_for_corporation(&db, CORP_ID, &[a]).await.unwrap();
      seed_history(&db, 587, "2026-06-03", 12.0).await;

      let value = asset_value_as_of_for_corporation(&db, CORP_ID, "2026-06-03")
        .await
        .unwrap();

      assert_eq!(value, 0.0, "an unauthorized corp scope yields no value");
    }

    #[tokio::test]
    async fn it_treats_an_unpriced_type_as_zero_without_nulling_the_sum() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      replace_for_character(&db, 42, &[asset(100, 587, 3), asset(101, 999, 5)])
        .await
        .unwrap();
      seed_history(&db, 587, "2026-06-03", 10.0).await;

      let value = asset_value_as_of_for_character(&db, 42, "2026-06-03").await.unwrap();

      assert_eq!(value, 30.0, "type 999 has neither history nor a current price -> 0");
    }

    #[tokio::test]
    async fn it_values_a_blueprint_copy_at_zero() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let mut copy = asset(100, 587, 1);
      copy.is_blueprint_copy = Some(true);
      replace_for_character(&db, 42, &[copy]).await.unwrap();
      seed_history(&db, 587, "2026-06-03", 999.0).await;
      seed_price(&db, 587, 999.0).await;

      let value = asset_value_as_of_for_character(&db, 42, "2026-06-03").await.unwrap();

      assert_eq!(value, 0.0);
    }
  }

  mod muta_valuation {
    use pretty_assertions::assert_eq;

    use super::*;

    async fn financials_asset_value(db: &Database, character_id: i64) -> f64 {
      sqlx::query_scalar::<_, Option<f64>>("SELECT asset_value FROM character_financials WHERE character_id = ?")
        .bind(character_id)
        .fetch_one(&db.0)
        .await
        .unwrap()
        .unwrap_or(0.0)
    }

    async fn abyssal(db: &Database, item_id: i64, character_id: i64, muta: Option<f64>) {
      let mut item = AbyssalItem::new(
        item_id,
        character_id,
        587,
        5975,
        47_297,
        r#"[{"attribute_id":6,"value":450.0}]"#.to_owned(),
        1_700_000_000,
      );
      item.set_muta_price(muta, 1_700_000_000);
      upsert(db, &item).await.unwrap();
    }

    #[tokio::test]
    async fn an_abyssal_values_at_its_muta_price_over_the_base_type_price() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_item_type(&db, 587, "Rolled Module", 25, "Frigate", "Ship").await;
      seed_price(&db, 587, 100.0).await;
      let asset = char_asset(100, 42, None);
      replace_for_character(&db, 42, &[asset]).await.unwrap();
      abyssal(&db, 100, 42, Some(750_000.0)).await;

      let totals = inventory_totals_for_character(&db, 42, "", &[], None).await.unwrap();

      assert_eq!(
        totals.value, 750_000.0,
        "the per-item muta price wins over the base-type market price"
      );
    }

    #[tokio::test]
    async fn an_unlisted_abyssal_falls_back_to_the_base_type_price() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_item_type(&db, 587, "Rolled Module", 25, "Frigate", "Ship").await;
      seed_price(&db, 587, 100.0).await;
      let asset = char_asset(100, 42, None);
      replace_for_character(&db, 42, &[asset]).await.unwrap();
      abyssal(&db, 100, 42, None).await;

      let totals = inventory_totals_for_character(&db, 42, "", &[], None).await.unwrap();

      assert_eq!(
        totals.value, 100.0,
        "a null muta price falls back through the canonical chain"
      );
    }

    #[tokio::test]
    async fn a_blueprint_copy_abyssal_values_at_zero() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_item_type(&db, 587, "Rolled Module", 25, "Frigate", "Ship").await;
      seed_price(&db, 587, 100.0).await;
      let mut asset = char_asset(100, 42, None);
      asset.is_blueprint_copy = Some(true);
      replace_for_character(&db, 42, &[asset]).await.unwrap();
      abyssal(&db, 100, 42, Some(750_000.0)).await;

      let totals = inventory_totals_for_character(&db, 42, "", &[], None).await.unwrap();

      assert_eq!(totals.value, 0.0, "a blueprint copy is always valued at zero");
    }

    #[tokio::test]
    async fn the_page_query_and_the_financials_view_agree_per_asset() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_item_type(&db, 587, "Rolled Module", 25, "Frigate", "Ship").await;
      seed_item_type(&db, 24, "Tritanium", 18, "Mineral", "Mineral").await;
      seed_price(&db, 587, 100.0).await;
      seed_price(&db, 24, 5.0).await;
      let rolled = char_asset(100, 42, None);
      let mut mineral = char_asset(101, 42, None);
      mineral.type_id = 24;
      mineral.quantity = 3;
      replace_for_character(&db, 42, &[rolled, mineral]).await.unwrap();
      abyssal(&db, 100, 42, Some(750_000.0)).await;

      let totals = inventory_totals_for_character(&db, 42, "", &[], None).await.unwrap();
      let view_value = financials_asset_value(&db, 42).await;

      assert_eq!(totals.value, 750_015.0, "muta abyssal (750_000) + 3 tritanium @5 (15)");
      assert_eq!(
        view_value, totals.value,
        "the character_financials view and the page valuation produce the same total"
      );
    }

    #[tokio::test]
    async fn the_full_set_total_sums_every_asset_regardless_of_the_loaded_page() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_item_type(&db, 587, "Rolled Module", 25, "Frigate", "Ship").await;
      seed_price(&db, 587, 100.0).await;
      let assets: Vec<CharacterAsset> = (100..103).map(|item_id| char_asset(item_id, 42, None)).collect();
      replace_for_character(&db, 42, &assets).await.unwrap();
      abyssal(&db, 100, 42, Some(750_000.0)).await;

      let page = inventory_page_for_character(
        &db,
        42,
        &InventoryQuery {
          cursor: None,
          direction: SortDirection::Ascending,
          filter: "",
          limit: 1,
          location_ids: &[],
          me_id: None,
          reproc_yield: 0.5,
          sort: SortColumn::Value,
        },
      )
      .await
      .unwrap();
      let page_sum: f64 = page.iter().map(|row| row.value).sum();
      let totals = inventory_totals_for_character(&db, 42, "", &[], None).await.unwrap();
      let view_value = financials_asset_value(&db, 42).await;

      assert_eq!(page.len(), 1, "the page is limited to a single row");
      assert_eq!(
        totals.value, 750_200.0,
        "muta abyssal (750_000) + 2 base-priced modules @100 (200)"
      );
      assert!(
        page_sum < totals.value,
        "the page sum under-reports the full asset scope"
      );
      assert_eq!(
        view_value, totals.value,
        "the full-set total equals the net-worth asset value"
      );
    }

    #[tokio::test]
    async fn the_full_set_total_honors_the_active_filter() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_item_type(&db, 587, "Rolled Module", 25, "Frigate", "Ship").await;
      seed_item_type(&db, 24, "Tritanium", 18, "Mineral", "Mineral").await;
      seed_price(&db, 587, 100.0).await;
      seed_price(&db, 24, 5.0).await;
      let module = char_asset(100, 42, None);
      let mut mineral = char_asset(101, 42, None);
      mineral.type_id = 24;
      mineral.quantity = 3;
      replace_for_character(&db, 42, &[module, mineral]).await.unwrap();

      let filtered = inventory_totals_for_character(&db, 42, "Tritanium", &[], None)
        .await
        .unwrap();

      assert_eq!(
        filtered.value, 15.0,
        "the full-set total is scoped to the filtered rows (3 tritanium @5)"
      );
    }
  }

  mod corp_muta_valuation {
    use pretty_assertions::assert_eq;

    use super::*;

    async fn financials_asset_value(db: &Database, corporation_id: i64) -> f64 {
      asset_value_as_of_for_corporation(db, corporation_id, "2999-01-01")
        .await
        .unwrap()
    }

    async fn corp_abyssal(db: &Database, item_id: i64, corporation_id: i64, muta: Option<f64>) {
      let mut item = CorporationAbyssalItem::new(
        item_id,
        corporation_id,
        587,
        5975,
        47_297,
        r#"[{"attribute_id":6,"value":450.0}]"#.to_owned(),
        1_700_000_000,
      );
      item.set_muta_price(muta, 1_700_000_000);
      upsert_corporation(db, &item).await.unwrap();
    }

    #[tokio::test]
    async fn a_corp_abyssal_values_at_its_muta_price_over_the_base_type_price() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, DIRECTOR_ID).await;
      authorize_corp(&db).await;
      seed_item_type(&db, 587, "Rolled Module", 25, "Frigate", "Ship").await;
      seed_price(&db, 587, 100.0).await;
      replace_for_corporation(&db, CORP_ID, &[corp_asset(100, CORP_ID, None)])
        .await
        .unwrap();
      corp_abyssal(&db, 100, CORP_ID, Some(750_000.0)).await;

      let totals = inventory_totals_for_corporation(&db, CORP_ID, "", &[], None)
        .await
        .unwrap();

      assert_eq!(
        totals.value, 750_000.0,
        "the per-item corp muta price wins over the base-type market price"
      );
    }

    #[tokio::test]
    async fn an_unlisted_corp_abyssal_falls_back_to_the_base_type_price() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, DIRECTOR_ID).await;
      authorize_corp(&db).await;
      seed_item_type(&db, 587, "Rolled Module", 25, "Frigate", "Ship").await;
      seed_price(&db, 587, 100.0).await;
      replace_for_corporation(&db, CORP_ID, &[corp_asset(100, CORP_ID, None)])
        .await
        .unwrap();
      corp_abyssal(&db, 100, CORP_ID, None).await;

      let totals = inventory_totals_for_corporation(&db, CORP_ID, "", &[], None)
        .await
        .unwrap();

      assert_eq!(
        totals.value, 100.0,
        "a null corp muta price falls back through the canonical chain"
      );
    }

    #[tokio::test]
    async fn a_char_abyssal_table_never_values_a_corp_asset() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, DIRECTOR_ID).await;
      authorize_corp(&db).await;
      seed_item_type(&db, 587, "Rolled Module", 25, "Frigate", "Ship").await;
      seed_price(&db, 587, 100.0).await;
      replace_for_corporation(&db, CORP_ID, &[corp_asset(100, CORP_ID, None)])
        .await
        .unwrap();
      let mut wrong = AbyssalItem::new(
        100,
        DIRECTOR_ID,
        587,
        5975,
        47_297,
        r#"[{"attribute_id":6,"value":450.0}]"#.to_owned(),
        1_700_000_000,
      );
      wrong.set_muta_price(Some(750_000.0), 1_700_000_000);
      upsert(&db, &wrong).await.unwrap();

      let totals = inventory_totals_for_corporation(&db, CORP_ID, "", &[], None)
        .await
        .unwrap();

      assert_eq!(
        totals.value, 100.0,
        "corp valuation joins corporation_abyssal_items, so a char abyssal row never leaks in"
      );
    }

    #[tokio::test]
    async fn the_corp_page_query_and_the_as_of_aggregate_agree() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, DIRECTOR_ID).await;
      authorize_corp(&db).await;
      seed_item_type(&db, 587, "Rolled Module", 25, "Frigate", "Ship").await;
      seed_item_type(&db, 24, "Tritanium", 18, "Mineral", "Mineral").await;
      seed_price(&db, 587, 100.0).await;
      seed_price(&db, 24, 5.0).await;
      let rolled = corp_asset(100, CORP_ID, None);
      let mut mineral = corp_asset(101, CORP_ID, None);
      mineral.type_id = 24;
      mineral.quantity = 3;
      replace_for_corporation(&db, CORP_ID, &[rolled, mineral]).await.unwrap();
      corp_abyssal(&db, 100, CORP_ID, Some(750_000.0)).await;

      let totals = inventory_totals_for_corporation(&db, CORP_ID, "", &[], None)
        .await
        .unwrap();
      let as_of_value = financials_asset_value(&db, CORP_ID).await;

      assert_eq!(
        totals.value, 750_015.0,
        "corp muta abyssal (750_000) + 3 tritanium @5 (15)"
      );
      assert_eq!(
        as_of_value, totals.value,
        "the corp as-of aggregate and the page valuation produce the same total"
      );
    }
  }

  mod character_assets {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_cascade_deletes_with_the_character() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      replace_for_character(&db, 42, &[char_asset(100, 42, None)])
        .await
        .unwrap();

      sqlx::query("PRAGMA foreign_keys = ON")
        .execute(db.writer())
        .await
        .unwrap();
      sqlx::query("DELETE FROM characters WHERE id = ?")
        .bind(42_i64)
        .execute(db.writer())
        .await
        .unwrap();

      assert_eq!(count_for_character(&db, 42).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn it_counts_scoped_rows() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_character(&db, 43).await;
      replace_for_character(&db, 42, &[char_asset(100, 42, None), char_asset(101, 42, None)])
        .await
        .unwrap();
      replace_for_character(&db, 43, &[char_asset(200, 43, None)])
        .await
        .unwrap();

      assert_eq!(count_for_character(&db, 42).await.unwrap(), 2);
      assert_eq!(count_for_character(&db, 43).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn it_does_not_crash_on_an_item_id_duplicated_within_one_batch() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;

      replace_for_character(&db, 42, &[char_asset(100, 42, None), char_asset(100, 42, None)])
        .await
        .unwrap();

      let rows = for_character(&db, 42).await.unwrap();
      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].item_id(), 100);
    }

    #[tokio::test]
    async fn it_fetches_children_by_container_id_and_roots_by_null() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      replace_for_character(
        &db,
        42,
        &[
          char_asset(100, 42, None),
          char_asset(101, 42, Some(100)),
          char_asset(102, 42, Some(100)),
        ],
      )
      .await
      .unwrap();

      let children = children_for_character(&db, 42, 100).await.unwrap();
      let roots = roots_for_character(&db, 42).await.unwrap();

      assert_eq!(
        children.iter().map(CharacterAsset::item_id).collect::<Vec<_>>(),
        [101, 102]
      );
      assert_eq!(roots.iter().map(CharacterAsset::item_id).collect::<Vec<_>>(), [100]);
    }

    #[tokio::test]
    async fn it_leaves_other_characters_untouched() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_character(&db, 43).await;
      replace_for_character(&db, 42, &[char_asset(100, 42, None)])
        .await
        .unwrap();
      replace_for_character(&db, 43, &[char_asset(200, 43, None)])
        .await
        .unwrap();

      replace_for_character(&db, 42, &[]).await.unwrap();

      assert!(for_character(&db, 42).await.unwrap().is_empty());
      assert_eq!(for_character(&db, 43).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn it_cleans_up_asset_tag_memberships_when_an_item_goes_stale() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      replace_for_character(&db, 42, &[char_asset(100, 42, None), char_asset(101, 42, None)])
        .await
        .unwrap();
      let tag = infra::create(&db, "Loot", None, None).await.unwrap();
      infra::assign(&db, ENTITY_TYPE_ASSET, 100, tag.id()).await.unwrap();
      infra::assign(&db, ENTITY_TYPE_ASSET, 101, tag.id()).await.unwrap();
      infra::assign(&db, crate::store::model::ENTITY_TYPE_CHARACTER, 100, tag.id())
        .await
        .unwrap();

      // Item 100 disappears from the next sync; item 101 persists.
      replace_for_character(&db, 42, &[char_asset(101, 42, None)])
        .await
        .unwrap();

      let asset_members = infra::members(&db, tag.id(), ENTITY_TYPE_ASSET).await.unwrap();
      assert_eq!(asset_members, vec![101]);
      // The like-numbered character membership is a different scope and must survive.
      assert_eq!(
        infra::members(&db, tag.id(), crate::store::model::ENTITY_TYPE_CHARACTER)
          .await
          .unwrap(),
        vec![100]
      );
    }

    #[tokio::test]
    async fn it_reclaims_an_item_id_held_by_another_character() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_character(&db, 43).await;
      replace_for_character(&db, 42, &[char_asset(100, 42, None)])
        .await
        .unwrap();

      replace_for_character(&db, 43, &[char_asset(100, 43, None)])
        .await
        .unwrap();

      assert!(for_character(&db, 42).await.unwrap().is_empty());
      let rows = for_character(&db, 43).await.unwrap();
      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].item_id(), 100);
      assert_eq!(count_for_character(&db, 42).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn it_round_trips_a_replaced_batch_with_hierarchy_columns() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let mut root = char_asset(100, 42, None);
      root.is_container = true;
      let child = char_asset(101, 42, Some(100));

      replace_for_character(&db, 42, &[root, child]).await.unwrap();

      let rows = for_character(&db, 42).await.unwrap();
      assert_eq!(rows.len(), 2);
      assert_eq!(rows[0].item_id(), 100);
      assert_eq!(rows[0].container_id(), None);
      assert!(rows[0].is_container());
      assert_eq!(rows[1].container_id(), Some(100));
      assert_eq!(rows[1].depth(), 1);
    }

    #[tokio::test]
    async fn it_upserts_a_single_row_in_place() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      upsert_character_asset(&db, &char_asset(100, 42, None)).await.unwrap();
      let mut updated = char_asset(100, 42, Some(99));
      updated.quantity = 7;

      upsert_character_asset(&db, &updated).await.unwrap();

      let rows = for_character(&db, 42).await.unwrap();
      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].quantity(), 7);
      assert_eq!(rows[0].container_id(), Some(99));
    }

    #[tokio::test]
    async fn it_upserts_and_prunes_across_write_batch_boundaries() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let initial: Vec<_> = (100..105).map(|id| char_asset(id, 42, None)).collect();

      // Batch size 2 over 5 rows forces three separate upsert transactions.
      replace_for_character_batched(&db, 42, &initial, 2).await.unwrap();
      let mut ids: Vec<_> = for_character(&db, 42)
        .await
        .unwrap()
        .iter()
        .map(CharacterAsset::item_id)
        .collect();
      ids.sort_unstable();
      assert_eq!(
        ids,
        [100, 101, 102, 103, 104],
        "every row survives multiple upsert batches"
      );

      // Re-replace with a subset so three ids go stale and are pruned across multiple delete batches.
      let next: Vec<_> = (100..102).map(|id| char_asset(id, 42, None)).collect();
      replace_for_character_batched(&db, 42, &next, 2).await.unwrap();
      let mut ids: Vec<_> = for_character(&db, 42)
        .await
        .unwrap()
        .iter()
        .map(CharacterAsset::item_id)
        .collect();
      ids.sort_unstable();
      assert_eq!(
        ids,
        [100, 101],
        "stale rows are pruned across delete batches, final state matches the new set"
      );
    }

    #[tokio::test]
    async fn it_yields_the_current_set_not_duplicates_on_re_replace() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      replace_for_character(&db, 42, &[char_asset(100, 42, None), char_asset(101, 42, None)])
        .await
        .unwrap();

      replace_for_character(&db, 42, &[char_asset(101, 42, None)])
        .await
        .unwrap();

      let rows = for_character(&db, 42).await.unwrap();
      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].item_id(), 101);
    }
  }

  mod combined {
    use pretty_assertions::assert_eq;

    use super::*;

    fn query(sort: SortColumn) -> InventoryQuery<'static> {
      InventoryQuery {
        cursor: None,
        direction: SortDirection::Ascending,
        filter: "",
        limit: 100,
        location_ids: &[],
        me_id: None,
        reproc_yield: 0.5,
        sort,
      }
    }

    async fn seed_combined(db: &Database) {
      seed_character(db, 42).await;
      authorize_corp(db).await;
      seed_item_type(db, 587, "Rifter", 25, "Frigate", "Ship").await;
      seed_price(db, 587, 100.0).await;
    }

    #[tokio::test]
    async fn it_yields_nothing_for_an_all_scope_with_no_owners() {
      let db = store::open_test().await.unwrap();

      assert!(
        inventory_page_for_combined(&db, &[], &[], &query(SortColumn::Name))
          .await
          .unwrap()
          .is_empty()
      );
      assert_eq!(
        inventory_totals_for_combined(&db, &[], &[], "", &[], None)
          .await
          .unwrap(),
        InventoryTotals::default()
      );
      assert!(geo_locations_for_combined(&db, &[], &[]).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn the_geo_locations_fold_both_owners_at_a_shared_place() {
      let db = store::open_test().await.unwrap();
      seed_combined(&db).await;
      seed_named_station(&db, 60_003_760, "Jita IV - Moon 4").await;
      let mut a = char_asset(100, 42, None);
      a.type_id = 587;
      a.quantity = 2;
      let mut b = corp_asset(200, CORP_ID, None);
      b.type_id = 587;
      b.quantity = 3;
      replace_for_character(&db, 42, &[a]).await.unwrap();
      replace_for_corporation(&db, CORP_ID, &[b]).await.unwrap();

      let rows = geo_locations_for_combined(&db, &[42], &[CORP_ID]).await.unwrap();

      assert_eq!(
        rows.len(),
        1,
        "the shared station folds across the character and the corp"
      );
      assert_eq!(rows[0].item_count, 5);
      assert_eq!(rows[0].value, 500.0);
    }

    #[tokio::test]
    async fn the_page_applies_the_structured_filter_to_the_union() {
      let db = store::open_test().await.unwrap();
      seed_combined(&db).await;
      seed_item_type(&db, 24, "Tritanium", 18, "Mineral", "Mineral").await;
      let mut ship = char_asset(100, 42, None);
      ship.type_id = 587;
      let mut mineral = corp_asset(200, CORP_ID, None);
      mineral.type_id = 24;
      replace_for_character(&db, 42, &[ship]).await.unwrap();
      replace_for_corporation(&db, CORP_ID, &[mineral]).await.unwrap();

      let rows = inventory_page_for_combined(
        &db,
        &[42],
        &[CORP_ID],
        &InventoryQuery {
          filter: "category:ship",
          ..query(SortColumn::Name)
        },
      )
      .await
      .unwrap();

      assert_eq!(rows.iter().map(|r| r.item_id).collect::<Vec<_>>(), [100]);
    }

    #[tokio::test]
    async fn the_page_excludes_corporation_assets_the_user_is_not_authorized_to_see() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_item_type(&db, 587, "Rifter", 25, "Frigate", "Ship").await;
      seed_price(&db, 587, 100.0).await;
      let mut owned = char_asset(100, 42, None);
      owned.type_id = 587;
      let mut corp = corp_asset(200, CORP_ID, None);
      corp.type_id = 587;
      replace_for_character(&db, 42, &[owned]).await.unwrap();
      replace_for_corporation(&db, CORP_ID, &[corp]).await.unwrap();

      let rows = inventory_page_for_combined(&db, &[42], &[CORP_ID], &query(SortColumn::Name))
        .await
        .unwrap();

      assert_eq!(rows.iter().map(|r| r.item_id).collect::<Vec<_>>(), [100]);
    }

    #[tokio::test]
    async fn the_page_seeks_the_next_window_across_the_union_by_cursor() {
      let db = store::open_test().await.unwrap();
      seed_combined(&db).await;
      let mut a = char_asset(100, 42, None);
      a.type_id = 587;
      a.quantity = 1;
      let mut b = corp_asset(200, CORP_ID, None);
      b.type_id = 587;
      b.quantity = 2;
      let mut c = corp_asset(201, CORP_ID, None);
      c.type_id = 587;
      c.quantity = 3;
      replace_for_character(&db, 42, &[a]).await.unwrap();
      replace_for_corporation(&db, CORP_ID, &[b, c]).await.unwrap();

      let first = inventory_page_for_combined(
        &db,
        &[42],
        &[CORP_ID],
        &InventoryQuery {
          direction: SortDirection::Ascending,
          limit: 2,
          ..query(SortColumn::Quantity)
        },
      )
      .await
      .unwrap();
      assert_eq!(first.iter().map(|r| r.quantity).collect::<Vec<_>>(), [1, 2]);

      let cursor = first.last().unwrap().cursor(SortColumn::Quantity);
      let second = inventory_page_for_combined(
        &db,
        &[42],
        &[CORP_ID],
        &InventoryQuery {
          cursor: Some(cursor),
          direction: SortDirection::Ascending,
          limit: 2,
          ..query(SortColumn::Quantity)
        },
      )
      .await
      .unwrap();
      assert_eq!(second.iter().map(|r| r.quantity).collect::<Vec<_>>(), [3]);
    }

    #[tokio::test]
    async fn the_page_unions_character_and_corporation_assets_in_one_keyset_window() {
      let db = store::open_test().await.unwrap();
      seed_combined(&db).await;
      let mut owned = char_asset(100, 42, None);
      owned.type_id = 587;
      let mut corp = corp_asset(200, CORP_ID, None);
      corp.type_id = 587;
      replace_for_character(&db, 42, &[owned]).await.unwrap();
      replace_for_corporation(&db, CORP_ID, &[corp]).await.unwrap();

      let rows = inventory_page_for_combined(&db, &[42], &[CORP_ID], &query(SortColumn::Name))
        .await
        .unwrap();

      assert_eq!(
        rows.iter().map(|r| r.item_id).collect::<Vec<_>>(),
        [100, 200],
        "the corp row appears alongside the character row in the All page"
      );
      assert_eq!(
        rows.iter().map(|r| r.owner_id).collect::<Vec<_>>(),
        [42, CORP_ID],
        "each row carries its own owner id"
      );
    }

    #[tokio::test]
    async fn the_totals_drop_unauthorized_corporation_holdings() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_item_type(&db, 587, "Rifter", 25, "Frigate", "Ship").await;
      seed_price(&db, 587, 100.0).await;
      let mut a = char_asset(100, 42, None);
      a.type_id = 587;
      a.quantity = 2;
      let mut b = corp_asset(200, CORP_ID, None);
      b.type_id = 587;
      b.quantity = 3;
      replace_for_character(&db, 42, &[a]).await.unwrap();
      replace_for_corporation(&db, CORP_ID, &[b]).await.unwrap();

      let totals = inventory_totals_for_combined(&db, &[42], &[CORP_ID], "", &[], None)
        .await
        .unwrap();

      assert_eq!(totals.items, 2, "only the character holdings count");
      assert_eq!(totals.value, 200.0);
    }

    #[tokio::test]
    async fn the_totals_sum_character_and_authorized_corporation_holdings() {
      let db = store::open_test().await.unwrap();
      seed_combined(&db).await;
      let mut a = char_asset(100, 42, None);
      a.type_id = 587;
      a.quantity = 2;
      a.location_id = 60_000_001;
      let mut b = corp_asset(200, CORP_ID, None);
      b.type_id = 587;
      b.quantity = 3;
      b.location_id = 60_000_002;
      replace_for_character(&db, 42, &[a]).await.unwrap();
      replace_for_corporation(&db, CORP_ID, &[b]).await.unwrap();

      let totals = inventory_totals_for_combined(&db, &[42], &[CORP_ID], "", &[], None)
        .await
        .unwrap();

      assert_eq!(totals.items, 5, "2 character + 3 corp units");
      assert_eq!(totals.locations, 2);
      assert_eq!(totals.value, 500.0);
    }
  }

  mod completeness {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_counts_and_lists_unresolved_type_ids() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_item_type(&db, 587, "Rifter", 25, "Frigate", "Ship").await;
      let mut resolved = char_asset(100, 42, None);
      resolved.type_id = 587;
      let mut orphan_a = char_asset(101, 42, None);
      orphan_a.type_id = 999_999;
      let mut orphan_b = char_asset(102, 42, None);
      orphan_b.type_id = 888_888;
      let mut orphan_b_dup = char_asset(103, 42, None);
      orphan_b_dup.type_id = 888_888;
      replace_for_character(&db, 42, &[resolved, orphan_a, orphan_b, orphan_b_dup])
        .await
        .unwrap();

      let report = completeness_for_character(&db, 42).await.unwrap();

      assert!(!report.is_complete());
      assert_eq!(report.distinct_type_ids, 3);
      assert_eq!(report.resolved, 1);
      assert_eq!(report.unresolved, [888_888, 999_999]);
    }

    #[tokio::test]
    async fn it_reports_a_fully_resolved_set_as_complete() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_item_type(&db, 587, "Rifter", 25, "Frigate", "Ship").await;
      let mut a = char_asset(100, 42, None);
      a.type_id = 587;
      let mut b = char_asset(101, 42, None);
      b.type_id = 587;
      replace_for_character(&db, 42, &[a, b]).await.unwrap();

      let report = completeness_for_character(&db, 42).await.unwrap();

      assert!(report.is_complete());
      assert_eq!(report.distinct_type_ids, 1);
      assert_eq!(report.resolved, 1);
      assert!(report.unresolved.is_empty());
    }

    #[tokio::test]
    async fn it_reports_corporation_completeness() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      authorize_corp(&db).await;
      let mut orphan = corp_asset(100, CORP_ID, None);
      orphan.type_id = 999_999;
      replace_for_corporation(&db, CORP_ID, &[orphan]).await.unwrap();

      let report = completeness_for_corporation(&db, CORP_ID).await.unwrap();

      assert!(!report.is_complete());
      assert_eq!(report.unresolved, [999_999]);
    }
  }

  mod corp_scope_gating {
    use pretty_assertions::assert_eq;

    use super::*;

    fn query(sort: SortColumn) -> InventoryQuery<'static> {
      InventoryQuery {
        cursor: None,
        direction: SortDirection::Ascending,
        filter: "",
        limit: 100,
        location_ids: &[],
        me_id: None,
        reproc_yield: 0.5,
        sort,
      }
    }

    async fn seed_one_corp_asset(db: &Database) {
      seed_character(db, 42).await;
      seed_item_type(db, 587, "Rifter", 25, "Frigate", "Ship").await;
      seed_price(db, 587, 250.0).await;
      let mut asset = corp_asset(100, CORP_ID, None);
      asset.type_id = 587;
      asset.quantity = 4;
      replace_for_corporation(db, CORP_ID, &[asset]).await.unwrap();
    }

    #[tokio::test]
    async fn it_excludes_corp_rows_when_not_owned_even_if_a_director_role_exists() {
      let db = store::open_test().await.unwrap();
      seed_one_corp_asset(&db).await;
      org::replace_for_corporation(
        &db,
        CORP_ID,
        &[CorporationMemberRole::from((
          CORP_ID,
          DIRECTOR_ID,
          "Director".to_string(),
        ))],
      )
      .await
      .unwrap();

      let page = inventory_page_for_corporation(&db, CORP_ID, &query(SortColumn::Name))
        .await
        .unwrap();
      let totals = inventory_totals_for_corporation(&db, CORP_ID, "", &[], None)
        .await
        .unwrap();

      assert!(page.is_empty());
      assert_eq!(totals, InventoryTotals::default());
      assert!(render_for_corporation(&db, CORP_ID).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_excludes_corp_rows_when_owned_but_the_authorizer_lacks_the_role() {
      let db = store::open_test().await.unwrap();
      seed_one_corp_asset(&db).await;
      infra::upsert(
        &db,
        CORP_ID,
        OwnerType::Corporation,
        "tok",
        "rt",
        4_102_444_800,
        Some(DIRECTOR_ID),
        None,
      )
      .await
      .unwrap();
      org::replace_for_corporation(
        &db,
        CORP_ID,
        &[CorporationMemberRole::from((
          CORP_ID,
          DIRECTOR_ID,
          "Accountant".to_string(),
        ))],
      )
      .await
      .unwrap();

      let page = inventory_page_for_corporation(&db, CORP_ID, &query(SortColumn::Name))
        .await
        .unwrap();
      let totals = inventory_totals_for_corporation(&db, CORP_ID, "", &[], None)
        .await
        .unwrap();

      assert!(page.is_empty());
      assert_eq!(totals, InventoryTotals::default());
      assert!(for_corporation(&db, CORP_ID).await.unwrap().is_empty());
      assert_eq!(count_for_corporation(&db, CORP_ID).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn it_returns_corp_rows_when_owned_and_the_authorizer_holds_the_role() {
      let db = store::open_test().await.unwrap();
      seed_one_corp_asset(&db).await;
      authorize_corp(&db).await;

      let page = inventory_page_for_corporation(&db, CORP_ID, &query(SortColumn::Name))
        .await
        .unwrap();
      let totals = inventory_totals_for_corporation(&db, CORP_ID, "", &[], None)
        .await
        .unwrap();

      assert_eq!(page.len(), 1);
      assert_eq!(totals.items, 4);
      assert_eq!(totals.value, 1_000.0);
      assert_eq!(for_corporation(&db, CORP_ID).await.unwrap().len(), 1);
      assert_eq!(count_for_corporation(&db, CORP_ID).await.unwrap(), 1);
    }
  }

  mod corporation_assets {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_fetches_children_and_counts() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      authorize_corp(&db).await;
      replace_for_corporation(
        &db,
        CORP_ID,
        &[corp_asset(100, CORP_ID, None), corp_asset(101, CORP_ID, Some(100))],
      )
      .await
      .unwrap();

      let children = children_for_corporation(&db, CORP_ID, 100).await.unwrap();
      let roots = roots_for_corporation(&db, CORP_ID).await.unwrap();

      assert_eq!(
        children.iter().map(CorporationAsset::item_id).collect::<Vec<_>>(),
        [101]
      );
      assert_eq!(roots.iter().map(CorporationAsset::item_id).collect::<Vec<_>>(), [100]);
      assert_eq!(count_for_corporation(&db, CORP_ID).await.unwrap(), 2);
    }

    #[tokio::test]
    async fn it_reclaims_an_item_id_held_by_another_corporation() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let other_corp = 98_000_002;
      let mut corp = Corporation::new(other_corp, "Other Corp", "OTC");
      corp.set_ceo_id(42);
      corp.set_creator_id(42);
      corp.set_member_count(1);
      corp.set_tax_rate(0.0);
      org::upsert_corporation(&db, &corp).await.unwrap();
      replace_for_corporation(&db, CORP_ID, &[corp_asset(100, CORP_ID, None)])
        .await
        .unwrap();

      replace_for_corporation(&db, other_corp, &[corp_asset(100, other_corp, None)])
        .await
        .unwrap();

      let owner = sqlx::query_scalar::<_, i64>("SELECT corporation_id FROM corporation_assets WHERE item_id = ?")
        .bind(100_i64)
        .fetch_one(&db.0)
        .await
        .unwrap();
      let total = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM corporation_assets WHERE item_id = ?")
        .bind(100_i64)
        .fetch_one(&db.0)
        .await
        .unwrap();
      assert_eq!(owner, other_corp);
      assert_eq!(total, 1);
    }

    #[tokio::test]
    async fn it_cleans_up_asset_tag_memberships_when_a_corp_item_goes_stale() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      authorize_corp(&db).await;
      replace_for_corporation(
        &db,
        CORP_ID,
        &[corp_asset(100, CORP_ID, None), corp_asset(101, CORP_ID, None)],
      )
      .await
      .unwrap();
      let tag = infra::create(&db, "Stock", None, None).await.unwrap();
      infra::assign(&db, ENTITY_TYPE_ASSET, 100, tag.id()).await.unwrap();
      infra::assign(&db, ENTITY_TYPE_ASSET, 101, tag.id()).await.unwrap();

      // Item 100 disappears from the next sync; item 101 persists.
      replace_for_corporation(&db, CORP_ID, &[corp_asset(101, CORP_ID, None)])
        .await
        .unwrap();

      assert_eq!(
        infra::members(&db, tag.id(), ENTITY_TYPE_ASSET).await.unwrap(),
        vec![101]
      );
    }

    #[tokio::test]
    async fn it_round_trips_a_replaced_batch_with_hierarchy_columns() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      authorize_corp(&db).await;
      let mut root = corp_asset(100, CORP_ID, None);
      root.is_container = true;
      let child = corp_asset(101, CORP_ID, Some(100));

      replace_for_corporation(&db, CORP_ID, &[root, child]).await.unwrap();

      let rows = for_corporation(&db, CORP_ID).await.unwrap();
      assert_eq!(rows.len(), 2);
      assert_eq!(rows[0].container_id(), None);
      assert!(rows[0].is_container());
      assert_eq!(rows[1].container_id(), Some(100));
      assert_eq!(rows[1].depth(), 1);
    }

    #[tokio::test]
    async fn it_upserts_a_single_row_in_place() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      authorize_corp(&db).await;
      upsert_corporation_asset(&db, &corp_asset(100, CORP_ID, None))
        .await
        .unwrap();
      let mut updated = corp_asset(100, CORP_ID, Some(99));
      updated.quantity = 7;

      upsert_corporation_asset(&db, &updated).await.unwrap();

      let rows = for_corporation(&db, CORP_ID).await.unwrap();
      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].quantity(), 7);
      assert_eq!(rows[0].container_id(), Some(99));
    }

    #[tokio::test]
    async fn it_upserts_and_prunes_across_write_batch_boundaries() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      authorize_corp(&db).await;
      let initial: Vec<_> = (100..105).map(|id| corp_asset(id, CORP_ID, None)).collect();

      replace_for_corporation_batched(&db, CORP_ID, &initial, 2)
        .await
        .unwrap();
      let mut ids: Vec<_> = for_corporation(&db, CORP_ID)
        .await
        .unwrap()
        .iter()
        .map(CorporationAsset::item_id)
        .collect();
      ids.sort_unstable();
      assert_eq!(
        ids,
        [100, 101, 102, 103, 104],
        "every row survives multiple upsert batches"
      );

      let next: Vec<_> = (100..102).map(|id| corp_asset(id, CORP_ID, None)).collect();
      replace_for_corporation_batched(&db, CORP_ID, &next, 2).await.unwrap();
      let mut ids: Vec<_> = for_corporation(&db, CORP_ID)
        .await
        .unwrap()
        .iter()
        .map(CorporationAsset::item_id)
        .collect();
      ids.sort_unstable();
      assert_eq!(
        ids,
        [100, 101],
        "stale rows are pruned across delete batches, final state matches the new set"
      );
    }
  }

  mod cross_character_all_scope {
    use pretty_assertions::assert_eq;

    use super::*;

    fn query(sort: SortColumn) -> InventoryQuery<'static> {
      InventoryQuery {
        cursor: None,
        direction: SortDirection::Ascending,
        filter: "",
        limit: 200,
        location_ids: &[],
        me_id: None,
        reproc_yield: 0.5,
        sort,
      }
    }

    async fn ingest_two_characters(db: &Database) {
      seed_character(db, 42).await;
      seed_character(db, 43).await;
      seed_character(db, 44).await;
      seed_item_type(db, 587, "Rifter", 25, "Frigate", "Ship").await;
      seed_price(db, 587, 1_000.0).await;

      let mut a_root = char_asset(100, 42, None);
      a_root.is_container = true;
      a_root.type_id = 587;
      let mut a_child = char_asset(101, 42, Some(100));
      a_child.type_id = 587;
      replace_for_character(db, 42, &[a_root, a_child]).await.unwrap();

      let mut b_ship = char_asset(200, 43, None);
      b_ship.type_id = 587;
      replace_for_character(db, 43, &[b_ship]).await.unwrap();

      let mut c_ship = char_asset(300, 44, None);
      c_ship.type_id = 587;
      replace_for_character(db, 44, &[c_ship]).await.unwrap();
    }

    #[tokio::test]
    async fn it_aggregates_page_and_totals_across_the_owned_set() {
      let db = store::open_test().await.unwrap();
      ingest_two_characters(&db).await;

      let page = inventory_page_for_characters(&db, &[42, 43], &query(SortColumn::Name))
        .await
        .unwrap();
      let totals = inventory_totals_for_characters(&db, &[42, 43], "", &[], None)
        .await
        .unwrap();

      assert_eq!(
        page.iter().map(|r| r.item_id).collect::<Vec<_>>(),
        [100, 200],
        "only top-level rows paginate across the owned set; item 101 is nested and lazy-loads"
      );
      assert_eq!(totals.items, 3);
      assert_eq!(totals.value, 3_000.0);
    }

    #[tokio::test]
    async fn it_finds_a_buried_match_ancestor_across_the_owned_set() {
      let db = store::open_test().await.unwrap();
      ingest_two_characters(&db).await;

      let ancestors = ancestors_of_match_for_characters(&db, &[42, 43], "category:ship", None)
        .await
        .unwrap();

      assert_eq!(ancestors, [100]);
    }

    #[tokio::test]
    async fn it_merges_roots_across_the_owned_set_and_excludes_a_non_listed_character() {
      let db = store::open_test().await.unwrap();
      ingest_two_characters(&db).await;

      let roots = roots_for_characters(&db, &[42, 43]).await.unwrap();

      assert_eq!(
        roots.iter().map(CharacterAsset::item_id).collect::<Vec<_>>(),
        [100, 200]
      );
    }

    #[tokio::test]
    async fn it_resolves_children_count_and_rollup_for_a_container_under_one_of_the_owners() {
      let db = store::open_test().await.unwrap();
      ingest_two_characters(&db).await;

      let children = children_render_for_characters(&db, &[42, 43], 100, 0.5).await.unwrap();
      let count = child_count_for_characters(&db, &[42, 43], 100).await.unwrap();
      let rollup = node_rollup_for_characters(&db, &[42, 43], 100).await.unwrap();

      assert_eq!(children.iter().map(|r| r.item_id).collect::<Vec<_>>(), [101]);
      assert_eq!(count, 1);
      assert_eq!(rollup.items, 1);
      assert_eq!(rollup.value, 1_000.0);
    }

    #[tokio::test]
    async fn it_returns_nothing_for_an_empty_owned_set() {
      let db = store::open_test().await.unwrap();
      ingest_two_characters(&db).await;

      assert!(roots_for_characters(&db, &[]).await.unwrap().is_empty());
      assert!(
        inventory_page_for_characters(&db, &[], &query(SortColumn::Name))
          .await
          .unwrap()
          .is_empty()
      );
      assert_eq!(
        inventory_totals_for_characters(&db, &[], "", &[], None).await.unwrap(),
        InventoryTotals::default()
      );
    }
  }

  mod custom_name {
    use pretty_assertions::assert_eq;

    use super::*;

    fn query(sort: SortColumn) -> InventoryQuery<'static> {
      InventoryQuery {
        cursor: None,
        direction: SortDirection::Ascending,
        filter: "",
        limit: 100,
        location_ids: &[],
        me_id: None,
        reproc_yield: 0.5,
        sort,
      }
    }

    #[tokio::test]
    async fn it_finds_a_renamed_character_item_by_either_its_custom_or_type_name() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_item_type(&db, 587, "Rifter", 25, "Frigate", "Ship").await;
      let mut renamed = char_asset(100, 42, None);
      renamed.name = Some("My Scout".to_owned());
      replace_for_character(&db, 42, &[renamed]).await.unwrap();

      let by_custom = inventory_page_for_character(
        &db,
        42,
        &InventoryQuery {
          filter: "Scout",
          ..query(SortColumn::Name)
        },
      )
      .await
      .unwrap();
      let by_type = inventory_page_for_character(
        &db,
        42,
        &InventoryQuery {
          filter: "Rifter",
          ..query(SortColumn::Name)
        },
      )
      .await
      .unwrap();
      let by_miss = inventory_page_for_character(
        &db,
        42,
        &InventoryQuery {
          filter: "Velator",
          ..query(SortColumn::Name)
        },
      )
      .await
      .unwrap();

      assert_eq!(by_custom.iter().map(|r| r.item_id).collect::<Vec<_>>(), [100]);
      assert_eq!(by_type.iter().map(|r| r.item_id).collect::<Vec<_>>(), [100]);
      assert!(by_miss.is_empty());

      assert_eq!(by_custom[0].name.as_deref(), Some("My Scout"));
      assert_eq!(by_custom[0].type_name, "Rifter");
    }

    #[tokio::test]
    async fn it_finds_a_renamed_corporation_item_by_either_its_custom_or_type_name() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      authorize_corp(&db).await;
      seed_item_type(&db, 587, "Rifter", 25, "Frigate", "Ship").await;
      let mut renamed = corp_asset(100, CORP_ID, None);
      renamed.name = Some("My Scout".to_owned());
      replace_for_corporation(&db, CORP_ID, &[renamed]).await.unwrap();

      let by_custom = inventory_page_for_corporation(
        &db,
        CORP_ID,
        &InventoryQuery {
          filter: "Scout",
          ..query(SortColumn::Name)
        },
      )
      .await
      .unwrap();
      let by_type = inventory_page_for_corporation(
        &db,
        CORP_ID,
        &InventoryQuery {
          filter: "Rifter",
          ..query(SortColumn::Name)
        },
      )
      .await
      .unwrap();

      assert_eq!(by_custom.iter().map(|r| r.item_id).collect::<Vec<_>>(), [100]);
      assert_eq!(by_type.iter().map(|r| r.item_id).collect::<Vec<_>>(), [100]);

      assert_eq!(by_custom[0].name.as_deref(), Some("My Scout"));
      assert_eq!(by_custom[0].type_name, "Rifter");
    }

    #[tokio::test]
    async fn it_sorts_a_renamed_character_item_by_its_coalesced_display_name() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_item_type(&db, 1, "Zilch", 1, "G", "Ship").await;
      seed_item_type(&db, 2, "Mmm", 2, "G", "Ship").await;
      let mut renamed = char_asset(100, 42, None);
      renamed.type_id = 1;
      renamed.name = Some("Aaa".to_owned());
      let mut plain = char_asset(101, 42, None);
      plain.type_id = 2;
      replace_for_character(&db, 42, &[renamed, plain]).await.unwrap();

      let rows = inventory_page_for_character(&db, 42, &query(SortColumn::Name))
        .await
        .unwrap();

      assert_eq!(
        rows.iter().map(|r| r.item_id).collect::<Vec<_>>(),
        [100, 101],
        "the item renamed Aaa sorts ahead of the unnamed Mmm even though its type name is Zilch"
      );
    }

    #[tokio::test]
    async fn it_sorts_a_renamed_corporation_item_by_its_coalesced_display_name() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      authorize_corp(&db).await;
      seed_item_type(&db, 1, "Zilch", 1, "G", "Ship").await;
      seed_item_type(&db, 2, "Mmm", 2, "G", "Ship").await;
      let mut renamed = corp_asset(100, CORP_ID, None);
      renamed.type_id = 1;
      renamed.name = Some("Aaa".to_owned());
      let mut plain = corp_asset(101, CORP_ID, None);
      plain.type_id = 2;
      replace_for_corporation(&db, CORP_ID, &[renamed, plain]).await.unwrap();

      let rows = inventory_page_for_corporation(&db, CORP_ID, &query(SortColumn::Name))
        .await
        .unwrap();

      assert_eq!(
        rows.iter().map(|r| r.item_id).collect::<Vec<_>>(),
        [100, 101],
        "the corp item renamed Aaa sorts ahead of the unnamed Mmm despite its Zilch type name"
      );
    }
  }

  mod tag_filter {
    use pretty_assertions::assert_eq;

    use super::*;

    fn query(sort: SortColumn) -> InventoryQuery<'static> {
      InventoryQuery {
        cursor: None,
        direction: SortDirection::Ascending,
        filter: "",
        limit: 100,
        location_ids: &[],
        me_id: None,
        reproc_yield: 0.5,
        sort,
      }
    }

    async fn seed_tag(db: &Database, id: i64, name: &str) {
      sqlx::query("INSERT INTO tags (id, created_at, name, position, updated_at) VALUES (?, 0, ?, ?, 0)")
        .bind(id)
        .bind(name)
        .bind(id)
        .execute(&db.0)
        .await
        .unwrap();
    }

    async fn tag_asset(db: &Database, tag_id: i64, item_id: i64) {
      sqlx::query("INSERT INTO entity_tags (tag_id, entity_type, entity_id) VALUES (?, 'asset', ?)")
        .bind(tag_id)
        .bind(item_id)
        .execute(&db.0)
        .await
        .unwrap();
    }

    async fn seed_two_tagged_ships(db: &Database) {
      seed_character(db, 42).await;
      seed_item_type(db, 587, "Rifter", 25, "Frigate", "Ship").await;
      let mut sell = char_asset(100, 42, None);
      sell.type_id = 587;
      let mut junk = char_asset(101, 42, None);
      junk.type_id = 587;
      let mut plain = char_asset(102, 42, None);
      plain.type_id = 587;
      replace_for_character(db, 42, &[sell, junk, plain]).await.unwrap();
      seed_tag(db, 1, "Sell").await;
      seed_tag(db, 2, "Junk").await;
      tag_asset(db, 1, 100).await;
      tag_asset(db, 2, 101).await;
    }

    #[tokio::test]
    async fn it_matches_a_tag_exact_and_case_insensitively() {
      let db = store::open_test().await.unwrap();
      seed_two_tagged_ships(&db).await;

      let rows = inventory_page_for_character(
        &db,
        42,
        &InventoryQuery {
          filter: "tag:sell",
          ..query(SortColumn::Name)
        },
      )
      .await
      .unwrap();

      assert_eq!(
        rows.iter().map(|r| r.item_id).collect::<Vec<_>>(),
        [100],
        "tag: matches the Sell-tagged stack case-insensitively"
      );
    }

    #[tokio::test]
    async fn it_excludes_a_tag_and_keeps_untagged_stacks_with_negation() {
      let db = store::open_test().await.unwrap();
      seed_two_tagged_ships(&db).await;

      let rows = inventory_page_for_character(
        &db,
        42,
        &InventoryQuery {
          filter: "-tag:Junk",
          ..query(SortColumn::Name)
        },
      )
      .await
      .unwrap();

      assert_eq!(
        rows.iter().map(|r| r.item_id).collect::<Vec<_>>(),
        [100, 102],
        "-tag: drops the Junk-tagged stack but keeps the untagged one"
      );
    }

    #[tokio::test]
    async fn it_ors_multi_value_tags_within_one_token() {
      let db = store::open_test().await.unwrap();
      seed_two_tagged_ships(&db).await;

      let rows = inventory_page_for_character(
        &db,
        42,
        &InventoryQuery {
          filter: "tag:Sell,Junk",
          ..query(SortColumn::Name)
        },
      )
      .await
      .unwrap();

      assert_eq!(
        rows.iter().map(|r| r.item_id).collect::<Vec<_>>(),
        [100, 101],
        "tag:a,b matches stacks with either tag"
      );
    }

    #[tokio::test]
    async fn it_ands_separate_tag_tokens() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_item_type(&db, 587, "Rifter", 25, "Frigate", "Ship").await;
      let mut both = char_asset(100, 42, None);
      both.type_id = 587;
      let mut only_sell = char_asset(101, 42, None);
      only_sell.type_id = 587;
      replace_for_character(&db, 42, &[both, only_sell]).await.unwrap();
      seed_tag(&db, 1, "Sell").await;
      seed_tag(&db, 2, "Ship").await;
      tag_asset(&db, 1, 100).await;
      tag_asset(&db, 2, 100).await;
      tag_asset(&db, 1, 101).await;

      let rows = inventory_page_for_character(
        &db,
        42,
        &InventoryQuery {
          filter: "tag:Sell tag:Ship",
          ..query(SortColumn::Name)
        },
      )
      .await
      .unwrap();

      assert_eq!(
        rows.iter().map(|r| r.item_id).collect::<Vec<_>>(),
        [100],
        "separate tag tokens require every tag"
      );
    }

    #[tokio::test]
    async fn it_composes_a_tag_with_another_facet() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_item_type(&db, 587, "Rifter", 25, "Frigate", "Ship").await;
      seed_item_type(&db, 24, "Tritanium", 18, "Mineral", "Mineral").await;
      let mut ship = char_asset(100, 42, None);
      ship.type_id = 587;
      let mut mineral = char_asset(101, 42, None);
      mineral.type_id = 24;
      replace_for_character(&db, 42, &[ship, mineral]).await.unwrap();
      seed_tag(&db, 1, "Sell").await;
      tag_asset(&db, 1, 100).await;
      tag_asset(&db, 1, 101).await;

      let rows = inventory_page_for_character(
        &db,
        42,
        &InventoryQuery {
          filter: "tag:Sell category:ship",
          ..query(SortColumn::Name)
        },
      )
      .await
      .unwrap();

      assert_eq!(
        rows.iter().map(|r| r.item_id).collect::<Vec<_>>(),
        [100],
        "tag: composes with category: as AND"
      );
    }

    #[tokio::test]
    async fn it_filters_by_tag_in_the_combined_all_scope() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      authorize_corp(&db).await;
      seed_item_type(&db, 587, "Rifter", 25, "Frigate", "Ship").await;
      seed_price(&db, 587, 100.0).await;
      let mut owned = char_asset(100, 42, None);
      owned.type_id = 587;
      let mut corp = corp_asset(200, CORP_ID, None);
      corp.type_id = 587;
      replace_for_character(&db, 42, &[owned]).await.unwrap();
      replace_for_corporation(&db, CORP_ID, &[corp]).await.unwrap();
      seed_tag(&db, 1, "Sell").await;
      tag_asset(&db, 1, 100).await;
      tag_asset(&db, 1, 200).await;

      let rows = inventory_page_for_combined(
        &db,
        &[42],
        &[CORP_ID],
        &InventoryQuery {
          filter: "tag:Sell",
          ..query(SortColumn::Name)
        },
      )
      .await
      .unwrap();

      assert_eq!(
        rows.iter().map(|r| r.item_id).collect::<Vec<_>>(),
        [100, 200],
        "tag: filters across both owners in the combined scope"
      );

      let totals = inventory_totals_for_combined(&db, &[42], &[CORP_ID], "tag:Sell", &[], None)
        .await
        .unwrap();
      assert_eq!(totals.items, 2, "combined totals count only the tagged rows");
    }
  }

  mod e2e_integration {
    use pretty_assertions::assert_eq;

    use super::*;

    fn query(sort: SortColumn) -> InventoryQuery<'static> {
      InventoryQuery {
        cursor: None,
        direction: SortDirection::Ascending,
        filter: "",
        limit: 200,
        location_ids: &[],
        me_id: None,
        reproc_yield: 0.5,
        sort,
      }
    }

    async fn ingest_character_portfolio(db: &Database, character_id: i64) {
      seed_item_type(db, 587, "Rifter", 25, "Frigate", "Ship").await;
      seed_item_type(db, 34, "Tritanium", 18, "Mineral", "Mineral").await;
      seed_price(db, 587, 1_000.0).await;
      seed_price(db, 34, 5.0).await;

      let mut station_container = char_asset(100, character_id, None);
      station_container.is_container = true;
      station_container.type_id = 587;
      let mut ship = char_asset(101, character_id, Some(100));
      ship.is_container = true;
      ship.type_id = 587;
      let mut trit_a = char_asset(102, character_id, Some(101));
      trit_a.type_id = 34;
      trit_a.quantity = 1_000;
      let mut trit_b = char_asset(103, character_id, Some(101));
      trit_b.type_id = 34;
      trit_b.quantity = 500;

      replace_for_character(db, character_id, &[station_container, ship, trit_a, trit_b])
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn it_filters_page_totals_and_search_auto_expand_consistently_over_one_scope() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      ingest_character_portfolio(&db, 42).await;

      let page = inventory_page_for_character(
        &db,
        42,
        &InventoryQuery {
          filter: "category:material",
          ..query(SortColumn::Name)
        },
      )
      .await
      .unwrap();
      assert!(
        page.is_empty(),
        "the material hits are nested, so no top-level page row matches; auto-expand surfaces them"
      );

      let totals = inventory_totals_for_character(&db, 42, "category:material", &[], None)
        .await
        .unwrap();
      assert_eq!(
        totals.items, 1_500,
        "only the trit quantities, matching the filtered page"
      );
      assert_eq!(totals.value, 7_500.0);

      let ancestors = ancestors_of_match_for_character(&db, 42, "category:material", None)
        .await
        .unwrap();
      assert_eq!(
        ancestors,
        [100, 101],
        "force-expand reaches the collapsed container holding the hit"
      );
    }

    #[tokio::test]
    async fn it_gates_corp_assets_across_every_pane_off_then_on() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_item_type(&db, 587, "Rifter", 25, "Frigate", "Ship").await;
      seed_price(&db, 587, 250.0).await;
      let mut root = corp_asset(100, CORP_ID, None);
      root.is_container = true;
      root.type_id = 587;
      root.quantity = 1;
      let mut child = corp_asset(101, CORP_ID, Some(100));
      child.type_id = 587;
      child.quantity = 3;
      replace_for_corporation(&db, CORP_ID, &[root, child]).await.unwrap();

      assert!(
        inventory_page_for_corporation(&db, CORP_ID, &query(SortColumn::Name))
          .await
          .unwrap()
          .is_empty()
      );
      assert_eq!(
        inventory_totals_for_corporation(&db, CORP_ID, "", &[], None)
          .await
          .unwrap(),
        InventoryTotals::default()
      );
      assert!(roots_for_corporation(&db, CORP_ID).await.unwrap().is_empty());
      assert!(render_for_corporation(&db, CORP_ID).await.unwrap().is_empty());
      assert!(
        children_render_for_corporation(&db, CORP_ID, 100, 0.5)
          .await
          .unwrap()
          .is_empty()
      );
      assert!(
        ancestors_of_match_for_corporation(&db, CORP_ID, "name:rifter", None)
          .await
          .unwrap()
          .is_empty()
      );

      authorize_corp(&db).await;

      let page = inventory_page_for_corporation(&db, CORP_ID, &query(SortColumn::Name))
        .await
        .unwrap();
      assert_eq!(
        page.iter().map(|r| r.item_id).collect::<Vec<_>>(),
        [100],
        "only the top-level container paginates once gated in; the child lazy-loads"
      );
      let totals = inventory_totals_for_corporation(&db, CORP_ID, "", &[], None)
        .await
        .unwrap();
      assert_eq!(totals.items, 4, "1 + 3 quantities now visible");
      assert_eq!(totals.value, 1_000.0);
      assert_eq!(roots_for_corporation(&db, CORP_ID).await.unwrap().len(), 1);
      assert_eq!(render_for_corporation(&db, CORP_ID).await.unwrap().len(), 2);
      assert_eq!(
        children_render_for_corporation(&db, CORP_ID, 100, 0.5)
          .await
          .unwrap()
          .iter()
          .map(|r| r.item_id)
          .collect::<Vec<_>>(),
        [101]
      );
      assert_eq!(
        ancestors_of_match_for_corporation(&db, CORP_ID, "name:rifter", None)
          .await
          .unwrap(),
        [100],
        "the search auto-expand also unlocks once gated in"
      );
    }

    #[tokio::test]
    async fn it_pages_a_bounded_window_while_totals_aggregate_the_full_set() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_item_type(&db, 587, "Rifter", 25, "Frigate", "Ship").await;
      seed_price(&db, 587, 100.0).await;
      let assets: Vec<_> = (100..112)
        .map(|item_id| {
          let mut a = char_asset(item_id, 42, None);
          a.type_id = 587;
          a
        })
        .collect();
      replace_for_character(&db, 42, &assets).await.unwrap();

      let first = inventory_page_for_character(
        &db,
        42,
        &InventoryQuery {
          limit: 5,
          ..query(SortColumn::Name)
        },
      )
      .await
      .unwrap();
      assert_eq!(first.len(), 5, "the window is bounded to the limit, not the full set");

      let cursor = first.last().unwrap().cursor(SortColumn::Name);
      let second = inventory_page_for_character(
        &db,
        42,
        &InventoryQuery {
          cursor: Some(cursor),
          limit: 5,
          ..query(SortColumn::Name)
        },
      )
      .await
      .unwrap();
      assert_eq!(second.len(), 5);
      let first_ids: std::collections::HashSet<i64> = first.iter().map(|r| r.item_id).collect();
      assert!(
        second.iter().all(|r| !first_ids.contains(&r.item_id)),
        "the second window does not re-yield any first-window row"
      );

      let totals = inventory_totals_for_character(&db, 42, "", &[], None).await.unwrap();
      assert_eq!(totals.items, 12);
    }

    #[tokio::test]
    async fn it_runs_the_full_character_pipeline_from_sync_output_to_every_query_seam() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      ingest_character_portfolio(&db, 42).await;

      let totals = inventory_totals_for_character(&db, 42, "", &[], None).await.unwrap();
      assert_eq!(totals.items, 1_502);
      assert_eq!(totals.locations, 1);
      assert_eq!(totals.value, 9_500.0);

      let page = inventory_page_for_character(&db, 42, &query(SortColumn::Name))
        .await
        .unwrap();
      assert_eq!(
        page.iter().map(|r| r.item_id).collect::<Vec<_>>(),
        [100],
        "only the top-level container paginates; its nested contents lazy-load on expand"
      );
      assert!(
        page.iter().all(|row| !row.type_name.is_empty()),
        "no blank metadata (post-mortem #5)"
      );

      let root_children = children_render_for_character(&db, 42, 100, 0.5).await.unwrap();
      assert_eq!(root_children.iter().map(|r| r.item_id).collect::<Vec<_>>(), [101]);
      let ship_children = children_render_for_character(&db, 42, 101, 0.5).await.unwrap();
      assert_eq!(ship_children.iter().map(|r| r.item_id).collect::<Vec<_>>(), [102, 103]);
      assert_eq!(child_count_for_character(&db, 42, 100).await.unwrap(), 1);

      let rollup = node_rollup_for_character(&db, 42, 100).await.unwrap();
      assert_eq!(rollup.items, 1_501);
      assert_eq!(rollup.value, 8_500.0);
    }

    #[tokio::test]
    async fn it_switches_scope_character_to_corporation_across_every_pane() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      authorize_corp(&db).await;
      seed_item_type(&db, 587, "Rifter", 25, "Frigate", "Ship").await;
      seed_price(&db, 587, 1_000.0).await;

      let mut char_ship = char_asset(100, 42, None);
      char_ship.type_id = 587;
      replace_for_character(&db, 42, &[char_ship]).await.unwrap();

      let mut corp_root = corp_asset(200, CORP_ID, None);
      corp_root.is_container = true;
      corp_root.type_id = 587;
      let mut corp_child_a = corp_asset(201, CORP_ID, Some(200));
      corp_child_a.type_id = 587;
      let mut corp_child_b = corp_asset(202, CORP_ID, Some(200));
      corp_child_b.type_id = 587;
      replace_for_corporation(&db, CORP_ID, &[corp_root, corp_child_a, corp_child_b])
        .await
        .unwrap();

      let char_page = inventory_page_for_character(&db, 42, &query(SortColumn::Name))
        .await
        .unwrap();
      assert_eq!(char_page.iter().map(|r| r.item_id).collect::<Vec<_>>(), [100]);
      let char_totals = inventory_totals_for_character(&db, 42, "", &[], None).await.unwrap();
      assert_eq!(char_totals.items, 1);
      assert_eq!(char_totals.value, 1_000.0);
      assert_eq!(roots_for_character(&db, 42).await.unwrap().len(), 1);

      let corp_page = inventory_page_for_corporation(&db, CORP_ID, &query(SortColumn::Name))
        .await
        .unwrap();
      assert_eq!(
        corp_page.iter().map(|r| r.item_id).collect::<Vec<_>>(),
        [200],
        "only the top-level corp container paginates; its children lazy-load"
      );
      let corp_totals = inventory_totals_for_corporation(&db, CORP_ID, "", &[], None)
        .await
        .unwrap();
      assert_eq!(corp_totals.items, 3);
      assert_eq!(corp_totals.value, 3_000.0);
      let corp_children = children_render_for_corporation(&db, CORP_ID, 200, 0.5).await.unwrap();
      assert_eq!(corp_children.iter().map(|r| r.item_id).collect::<Vec<_>>(), [201, 202]);
      assert_eq!(node_rollup_for_corporation(&db, CORP_ID, 200).await.unwrap().items, 2);
    }
  }

  mod geo_facets {
    use pretty_assertions::assert_eq;

    use super::*;

    const DODIXIE_STATION: i64 = 60_011_866;

    const DODIXIE_SYSTEM: i64 = 30_002_659;

    const JITA_STATION: i64 = 60_003_760;

    async fn seed_dodixie(db: &Database) {
      sde::upsert_region(
        db,
        &Region {
          description: None,
          id: 10_000_032,
          name: "Sinq Laison".to_owned(),
        },
      )
      .await
      .unwrap();
      sde::upsert_constellation(
        db,
        &Constellation {
          id: 20_000_369,
          name: "Coriault".to_owned(),
          position_x: 0.0,
          position_y: 0.0,
          position_z: 0.0,
          region_id: 10_000_032,
        },
      )
      .await
      .unwrap();
      sde::upsert_solar_system(
        db,
        &SolarSystem {
          constellation_id: 20_000_369,
          id: DODIXIE_SYSTEM,
          name: "Dodixie".to_owned(),
          position_x: 0.0,
          position_y: 0.0,
          position_z: 0.0,
          security_class: None,
          security_status: 0.9,
          star_id: None,
        },
      )
      .await
      .unwrap();
      sde::upsert_station(
        db,
        &Station {
          id: DODIXIE_STATION,
          max_dockable_ship_volume: 0.0,
          name: "Dodixie IX - Moon 20 - Federation Navy Assembly Plant".to_owned(),
          office_rental_cost: 0.0,
          owner: None,
          position_x: 0.0,
          position_y: 0.0,
          position_z: 0.0,
          race_id: None,
          reprocessing_efficiency: 0.0,
          reprocessing_stations_take: 0.0,
          services: "[]".to_owned(),
          system_id: DODIXIE_SYSTEM,
          type_id: 587,
        },
      )
      .await
      .unwrap();
    }

    fn query(sort: SortColumn) -> InventoryQuery<'static> {
      InventoryQuery {
        cursor: None,
        direction: SortDirection::Ascending,
        filter: "",
        limit: 100,
        location_ids: &[],
        me_id: None,
        reproc_yield: 0.5,
        sort,
      }
    }

    // Character 42 holds item 100 in Jita (The Forge) and item 101 in Dodixie (Sinq Laison).
    async fn seed_single_scope(db: &Database) {
      seed_character(db, 42).await;
      seed_item_type(db, 587, "Rifter", 25, "Frigate", "Ship").await;
      seed_named_station(db, JITA_STATION, "Jita IV - Moon 4 - Caldari Navy Assembly Plant").await;
      seed_dodixie(db).await;

      let mut at_jita = char_asset(100, 42, None);
      at_jita.type_id = 587;
      at_jita.location_id = JITA_STATION;
      let mut at_dodixie = char_asset(101, 42, None);
      at_dodixie.type_id = 587;
      at_dodixie.location_id = DODIXIE_STATION;
      replace_for_character(db, 42, &[at_jita, at_dodixie]).await.unwrap();
    }

    async fn page_items(db: &Database, filter: &'static str) -> Vec<i64> {
      inventory_page_for_character(
        db,
        42,
        &InventoryQuery {
          filter,
          ..query(SortColumn::Name)
        },
      )
      .await
      .unwrap()
      .iter()
      .map(|r| r.item_id)
      .collect()
    }

    // Character 42 holds item 100 in Jita; the corp holds item 200 in Dodixie.
    async fn seed_combined_scope(db: &Database) {
      seed_character(db, 42).await;
      authorize_corp(db).await;
      seed_item_type(db, 587, "Rifter", 25, "Frigate", "Ship").await;
      seed_price(db, 587, 100.0).await;
      seed_named_station(db, JITA_STATION, "Jita IV - Moon 4 - Caldari Navy Assembly Plant").await;
      seed_dodixie(db).await;

      let mut at_jita = char_asset(100, 42, None);
      at_jita.type_id = 587;
      at_jita.location_id = JITA_STATION;
      let mut at_dodixie = corp_asset(200, CORP_ID, None);
      at_dodixie.type_id = 587;
      at_dodixie.location_id = DODIXIE_STATION;
      replace_for_character(db, 42, &[at_jita]).await.unwrap();
      replace_for_corporation(db, CORP_ID, &[at_dodixie]).await.unwrap();
    }

    async fn combined_items(db: &Database, filter: &'static str) -> Vec<i64> {
      inventory_page_for_combined(
        db,
        &[42],
        &[CORP_ID],
        &InventoryQuery {
          filter,
          ..query(SortColumn::Name)
        },
      )
      .await
      .unwrap()
      .iter()
      .map(|r| r.item_id)
      .collect()
    }

    #[tokio::test]
    async fn it_filters_by_constellation_facet() {
      let db = store::open_test().await.unwrap();
      seed_single_scope(&db).await;

      assert_eq!(page_items(&db, "constellation:Kimotoro").await, [100]);
    }

    #[tokio::test]
    async fn it_filters_by_location_facet_and_its_loc_alias() {
      let db = store::open_test().await.unwrap();
      seed_single_scope(&db).await;

      assert_eq!(page_items(&db, "location:Dodixie").await, [101]);
      assert_eq!(page_items(&db, "loc:Dodixie").await, [101]);
    }

    #[tokio::test]
    async fn it_filters_by_region_facet() {
      let db = store::open_test().await.unwrap();
      seed_single_scope(&db).await;

      assert_eq!(page_items(&db, "region:\"The Forge\"").await, [100]);
    }

    #[tokio::test]
    async fn it_filters_by_system_facet() {
      let db = store::open_test().await.unwrap();
      seed_single_scope(&db).await;

      assert_eq!(page_items(&db, "system:Jita").await, [100]);
    }

    #[tokio::test]
    async fn it_filters_the_combined_union_by_a_location_free_text() {
      let db = store::open_test().await.unwrap();
      seed_combined_scope(&db).await;

      assert_eq!(combined_items(&db, "Dodixie").await, [200]);
    }

    #[tokio::test]
    async fn it_filters_the_combined_union_by_region() {
      let db = store::open_test().await.unwrap();
      seed_combined_scope(&db).await;

      assert_eq!(combined_items(&db, "region:\"The Forge\"").await, [100]);
    }

    #[tokio::test]
    async fn it_filters_the_combined_union_by_system() {
      let db = store::open_test().await.unwrap();
      seed_combined_scope(&db).await;

      assert_eq!(combined_items(&db, "system:Dodixie").await, [200]);
    }

    #[tokio::test]
    async fn it_matches_a_location_name_via_free_text() {
      let db = store::open_test().await.unwrap();
      seed_single_scope(&db).await;

      assert_eq!(page_items(&db, "Dodixie").await, [101]);
    }

    #[tokio::test]
    async fn it_negates_a_geo_facet() {
      let db = store::open_test().await.unwrap();
      seed_single_scope(&db).await;

      assert_eq!(page_items(&db, "-system:Jita").await, [101]);
    }

    #[tokio::test]
    async fn it_ors_comma_separated_geo_values() {
      let db = store::open_test().await.unwrap();
      seed_single_scope(&db).await;

      assert_eq!(page_items(&db, "system:Jita,Dodixie").await, [100, 101]);
    }

    #[tokio::test]
    async fn the_combined_totals_reflect_a_geo_facet() {
      let db = store::open_test().await.unwrap();
      seed_combined_scope(&db).await;

      let totals = inventory_totals_for_combined(&db, &[42], &[CORP_ID], "constellation:Coriault", &[], None)
        .await
        .unwrap();

      assert_eq!(
        totals.items, 1,
        "only the Dodixie corp holding survives the constellation filter"
      );
      assert_eq!(totals.locations, 1);
    }

    #[tokio::test]
    async fn the_totals_reflect_a_geo_facet() {
      let db = store::open_test().await.unwrap();
      seed_single_scope(&db).await;

      let totals = inventory_totals_for_character(&db, 42, "region:\"The Forge\"", &[], None)
        .await
        .unwrap();

      assert_eq!(totals.items, 1, "only the Jita holding survives the region filter");
      assert_eq!(totals.locations, 1);
    }
  }

  mod geo_locations {
    use pretty_assertions::assert_eq;

    use super::*;

    const STRUCTURE_ID: i64 = 1_021_000_000_000;

    #[tokio::test]
    async fn it_aggregates_geo_locations_across_an_owned_character_set() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_character(&db, 43).await;
      seed_item_type(&db, 587, "Rifter", 25, "Frigate", "Ship").await;
      seed_price(&db, 587, 100.0).await;
      seed_named_station(&db, 60_003_760, "Jita IV - Moon 4").await;
      let mut a = char_asset(100, 42, None);
      a.type_id = 587;
      let mut b = char_asset(200, 43, None);
      b.type_id = 587;
      replace_for_character(&db, 42, &[a]).await.unwrap();
      replace_for_character(&db, 43, &[b]).await.unwrap();

      let rows = geo_locations_for_characters(&db, &[42, 43]).await.unwrap();

      assert_eq!(rows.len(), 1, "the shared station folds across both owners");
      assert_eq!(rows[0].item_count, 2);
      assert!(geo_locations_for_characters(&db, &[]).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_excludes_container_nested_rows_from_the_location_aggregate() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_item_type(&db, 587, "Rifter", 25, "Frigate", "Ship").await;
      seed_price(&db, 587, 100.0).await;
      seed_named_station(&db, 60_003_760, "Jita IV - Moon 4").await;
      let mut root = char_asset(100, 42, None);
      root.is_container = true;
      root.type_id = 587;
      let mut child = char_asset(101, 42, Some(100));
      child.type_id = 587;
      child.location_id = 100;
      child.location_type = "item".to_owned();
      replace_for_character(&db, 42, &[root, child]).await.unwrap();

      let rows = geo_locations_for_character(&db, 42).await.unwrap();

      assert_eq!(rows.len(), 1);
      assert_eq!(
        rows[0].item_count, 1,
        "only the top-level container counts; the item-nested child is excluded"
      );
    }

    #[tokio::test]
    async fn it_gates_corporation_geo_locations_on_authorization() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_item_type(&db, 587, "Rifter", 25, "Frigate", "Ship").await;
      seed_price(&db, 587, 250.0).await;
      seed_named_station(&db, 60_003_760, "Jita IV - Moon 4").await;
      let mut a = corp_asset(100, CORP_ID, None);
      a.type_id = 587;
      a.quantity = 4;
      replace_for_corporation(&db, CORP_ID, &[a]).await.unwrap();

      assert!(
        geo_locations_for_corporation(&db, CORP_ID).await.unwrap().is_empty(),
        "no rows before the corp is authorized"
      );

      authorize_corp(&db).await;

      let rows = geo_locations_for_corporation(&db, CORP_ID).await.unwrap();
      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].item_count, 4);
      assert_eq!(rows[0].value, 1_000.0);
    }

    #[tokio::test]
    async fn it_labels_in_space_holdings_by_their_system_name() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_item_type(&db, 587, "Rifter", 25, "Frigate", "Ship").await;
      seed_geography(&db).await;
      let mut a = char_asset(100, 42, None);
      a.type_id = 587;
      a.location_id = GEO_SYSTEM;
      a.location_type = "solar_system".to_owned();
      replace_for_character(&db, 42, &[a]).await.unwrap();

      let rows = geo_locations_for_character(&db, 42).await.unwrap();

      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].location_type, "solar_system");
      assert_eq!(rows[0].location_label.as_deref(), Some("Jita"));
      assert_eq!(rows[0].system_id, Some(GEO_SYSTEM));
    }

    #[tokio::test]
    async fn it_marks_an_inaccessible_structure_and_leaves_its_geo_chain_unresolved() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_item_type(&db, 587, "Rifter", 25, "Frigate", "Ship").await;
      let mut a = char_asset(100, 42, None);
      a.type_id = 587;
      a.location_id = STRUCTURE_ID;
      a.location_type = "structure".to_owned();
      replace_for_character(&db, 42, &[a]).await.unwrap();
      sde::mark_inaccessible_structure(&db, 42, OwnerType::Character, STRUCTURE_ID)
        .await
        .unwrap();

      let rows = geo_locations_for_character(&db, 42).await.unwrap();

      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].location_label.as_deref(), Some("Inaccessible Structure"));
      assert_eq!(rows[0].system_id, None, "an unresolved structure has no geo chain");
    }

    #[tokio::test]
    async fn it_resolves_a_structure_location_through_its_solar_system() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_item_type(&db, 587, "Rifter", 25, "Frigate", "Ship").await;
      seed_named_structure(&db, STRUCTURE_ID, "Jita Citadel").await;
      let mut a = char_asset(100, 42, None);
      a.type_id = 587;
      a.location_id = STRUCTURE_ID;
      a.location_type = "structure".to_owned();
      replace_for_character(&db, 42, &[a]).await.unwrap();

      let rows = geo_locations_for_character(&db, 42).await.unwrap();

      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].location_label.as_deref(), Some("Jita Citadel"));
      assert_eq!(rows[0].system_name.as_deref(), Some("Jita"));
      assert_eq!(rows[0].region_name.as_deref(), Some("The Forge"));
    }

    #[tokio::test]
    async fn it_resolves_the_full_geo_chain_and_aggregates_value_and_count_for_a_station() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_item_type(&db, 587, "Rifter", 25, "Frigate", "Ship").await;
      seed_price(&db, 587, 100.0).await;
      seed_named_station(&db, 60_003_760, "Jita IV - Moon 4 - Caldari Navy Assembly Plant").await;
      let mut a = char_asset(100, 42, None);
      a.type_id = 587;
      a.quantity = 2;
      let mut b = char_asset(101, 42, None);
      b.type_id = 587;
      b.quantity = 3;
      replace_for_character(&db, 42, &[a, b]).await.unwrap();

      let rows = geo_locations_for_character(&db, 42).await.unwrap();

      assert_eq!(rows.len(), 1, "both assets fold into one location row");
      let row = &rows[0];
      assert_eq!(row.location_id, 60_003_760);
      assert_eq!(row.location_type, "station");
      assert_eq!(
        row.location_label.as_deref(),
        Some("Jita IV - Moon 4 - Caldari Navy Assembly Plant")
      );
      assert_eq!(row.system_id, Some(GEO_SYSTEM));
      assert_eq!(row.system_name.as_deref(), Some("Jita"));
      assert_eq!(row.constellation_id, Some(20_000_020));
      assert_eq!(row.constellation_name.as_deref(), Some("Kimotoro"));
      assert_eq!(row.region_id, Some(10_000_002));
      assert_eq!(row.region_name.as_deref(), Some("The Forge"));
      assert_eq!(row.item_count, 5);
      assert_eq!(row.value, 500.0);
    }
  }

  mod inventory_page {
    use pretty_assertions::assert_eq;

    use super::*;

    fn query(sort: SortColumn) -> InventoryQuery<'static> {
      InventoryQuery {
        cursor: None,
        direction: SortDirection::Ascending,
        filter: "",
        limit: 100,
        location_ids: &[],
        me_id: None,
        reproc_yield: 0.5,
        sort,
      }
    }

    #[tokio::test]
    async fn it_applies_the_structured_filter_to_the_window() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_item_type(&db, 587, "Rifter", 25, "Frigate", "Ship").await;
      seed_item_type(&db, 24, "Tritanium", 18, "Mineral", "Mineral").await;
      let mut ship = char_asset(100, 42, None);
      ship.type_id = 587;
      let mut mineral = char_asset(101, 42, None);
      mineral.type_id = 24;
      replace_for_character(&db, 42, &[ship, mineral]).await.unwrap();

      let rows = inventory_page_for_character(
        &db,
        42,
        &InventoryQuery {
          filter: "category:ship",
          ..query(SortColumn::Name)
        },
      )
      .await
      .unwrap();

      assert_eq!(rows.iter().map(|r| r.item_id).collect::<Vec<_>>(), [100]);
    }

    #[tokio::test]
    async fn it_does_not_materialize_excluded_pages_and_orders_by_a_join_derived_column() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_item_type(&db, 1, "Zilch", 1, "G", "Ship").await;
      seed_item_type(&db, 2, "Alpha", 2, "G", "Ship").await;
      let mut z = char_asset(100, 42, None);
      z.type_id = 1;
      let mut a = char_asset(101, 42, None);
      a.type_id = 2;
      replace_for_character(&db, 42, &[z, a]).await.unwrap();

      let names: Vec<_> = inventory_page_for_character(&db, 42, &query(SortColumn::Name))
        .await
        .unwrap()
        .into_iter()
        .map(|r| r.type_name)
        .collect();

      assert_eq!(names, ["Alpha", "Zilch"]);
    }

    #[tokio::test]
    async fn it_paginates_only_top_level_rows_excluding_container_children() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_item_type(&db, 587, "Rifter", 25, "Frigate", "Ship").await;
      let mut root = char_asset(100, 42, None);
      root.is_container = true;
      root.type_id = 587;
      let mut child = char_asset(101, 42, Some(100));
      child.type_id = 587;
      let mut sibling = char_asset(102, 42, None);
      sibling.type_id = 587;
      replace_for_character(&db, 42, &[root, child, sibling]).await.unwrap();

      let rows = inventory_page_for_character(&db, 42, &query(SortColumn::Name))
        .await
        .unwrap();

      assert_eq!(
        rows.iter().map(|r| r.item_id).collect::<Vec<_>>(),
        [100, 102],
        "the nested child (101) never appears as a page row"
      );
    }

    #[tokio::test]
    async fn it_prices_value_per_row_and_zeroes_blueprint_copies() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_item_type(&db, 587, "Rifter", 25, "Frigate", "Ship").await;
      seed_item_type(&db, 700, "Blueprint", 70, "Blueprints", "Blueprint").await;
      seed_price(&db, 587, 1_000.0).await;
      seed_price(&db, 700, 9_999.0).await;
      let mut priced = char_asset(100, 42, None);
      priced.type_id = 587;
      priced.quantity = 3;
      let mut bpc = char_asset(101, 42, None);
      bpc.type_id = 700;
      bpc.is_blueprint_copy = Some(true);
      replace_for_character(&db, 42, &[priced, bpc]).await.unwrap();

      let rows = inventory_page_for_character(&db, 42, &query(SortColumn::Name))
        .await
        .unwrap();
      let by_item: std::collections::HashMap<_, _> = rows.into_iter().map(|r| (r.item_id, r)).collect();

      assert_eq!(by_item[&100].unit_price, 1_000.0);
      assert_eq!(by_item[&100].value, 3_000.0);
      assert_eq!(by_item[&101].unit_price, 0.0);
      assert_eq!(by_item[&101].value, 0.0);
    }

    #[tokio::test]
    async fn it_restricts_the_page_to_the_supplied_location_ids_and_treats_empty_as_all() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_item_type(&db, 587, "Rifter", 25, "Frigate", "Ship").await;
      let mut here = char_asset(100, 42, None);
      here.type_id = 587;
      here.location_id = 60_000_001;
      let mut there = char_asset(101, 42, None);
      there.type_id = 587;
      there.location_id = 60_000_002;
      replace_for_character(&db, 42, &[here, there]).await.unwrap();

      let filtered = inventory_page_for_character(
        &db,
        42,
        &InventoryQuery {
          location_ids: &[60_000_001],
          ..query(SortColumn::Name)
        },
      )
      .await
      .unwrap();
      assert_eq!(
        filtered.iter().map(|r| r.item_id).collect::<Vec<_>>(),
        [100],
        "only rows at the selected location remain"
      );

      let unfiltered = inventory_page_for_character(&db, 42, &query(SortColumn::Name))
        .await
        .unwrap();
      assert_eq!(
        unfiltered.iter().map(|r| r.item_id).collect::<Vec<_>>(),
        [100, 101],
        "an empty location_ids predicate yields every location"
      );

      let multi = inventory_page_for_character(
        &db,
        42,
        &InventoryQuery {
          location_ids: &[60_000_001, 60_000_002],
          ..query(SortColumn::Name)
        },
      )
      .await
      .unwrap();
      assert_eq!(multi.iter().map(|r| r.item_id).collect::<Vec<_>>(), [100, 101]);
    }

    #[tokio::test]
    async fn it_returns_a_keyset_window_and_seeks_the_next_page_by_cursor() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_item_type(&db, 587, "Rifter", 25, "Frigate", "Ship").await;
      let assets: Vec<_> = (100..105)
        .map(|item_id| {
          let mut a = char_asset(item_id, 42, None);
          a.type_id = 587;
          a
        })
        .collect();
      replace_for_character(&db, 42, &assets).await.unwrap();

      let first = inventory_page_for_character(
        &db,
        42,
        &InventoryQuery {
          limit: 2,
          ..query(SortColumn::Name)
        },
      )
      .await
      .unwrap();
      assert_eq!(first.iter().map(|r| r.item_id).collect::<Vec<_>>(), [100, 101]);

      let cursor = first.last().unwrap().cursor(SortColumn::Name);
      let second = inventory_page_for_character(
        &db,
        42,
        &InventoryQuery {
          cursor: Some(cursor),
          limit: 2,
          ..query(SortColumn::Name)
        },
      )
      .await
      .unwrap();
      assert_eq!(second.iter().map(|r| r.item_id).collect::<Vec<_>>(), [102, 103]);
    }

    #[tokio::test]
    async fn it_sorts_descending_and_seeks_in_the_same_direction() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_item_type(&db, 587, "Rifter", 25, "Frigate", "Ship").await;
      for (item_id, qty) in [(100, 5), (101, 30), (102, 10)] {
        let mut a = char_asset(item_id, 42, None);
        a.type_id = 587;
        a.quantity = qty;
        upsert_character_asset(&db, &a).await.unwrap();
      }

      let page = inventory_page_for_character(
        &db,
        42,
        &InventoryQuery {
          direction: SortDirection::Descending,
          limit: 2,
          ..query(SortColumn::Quantity)
        },
      )
      .await
      .unwrap();

      assert_eq!(page.iter().map(|r| r.quantity).collect::<Vec<_>>(), [30, 10]);
    }
  }

  mod inventory_totals {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_aggregates_corporation_totals() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      authorize_corp(&db).await;
      seed_item_type(&db, 587, "Rifter", 25, "Frigate", "Ship").await;
      seed_price(&db, 587, 250.0).await;
      let mut a = corp_asset(100, CORP_ID, None);
      a.type_id = 587;
      a.quantity = 4;
      replace_for_corporation(&db, CORP_ID, &[a]).await.unwrap();

      let totals = inventory_totals_for_corporation(&db, CORP_ID, "", &[], None)
        .await
        .unwrap();

      assert_eq!(totals.items, 4);
      assert_eq!(totals.value, 1_000.0);
    }

    #[tokio::test]
    async fn it_aggregates_the_four_header_totals_in_sql_over_a_scope() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_item_type(&db, 587, "Rifter", 25, "Frigate", "Ship").await;
      seed_price(&db, 587, 100.0).await;
      let mut a = char_asset(100, 42, None);
      a.type_id = 587;
      a.quantity = 2;
      a.location_id = 60_000_001;
      let mut b = char_asset(101, 42, None);
      b.type_id = 587;
      b.quantity = 3;
      b.location_id = 60_000_002;
      replace_for_character(&db, 42, &[a, b]).await.unwrap();

      let totals = inventory_totals_for_character(&db, 42, "", &[], None).await.unwrap();

      assert_eq!(totals.items, 5);
      assert_eq!(totals.locations, 2);
      assert_eq!(totals.value, 500.0);
      assert_eq!(totals.volume, 12.5);
    }

    #[tokio::test]
    async fn it_restricts_the_totals_to_the_filtered_set() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_item_type(&db, 587, "Rifter", 25, "Frigate", "Ship").await;
      seed_item_type(&db, 24, "Tritanium", 18, "Mineral", "Mineral").await;
      seed_price(&db, 587, 100.0).await;
      seed_price(&db, 24, 5.0).await;
      let mut ship = char_asset(100, 42, None);
      ship.type_id = 587;
      ship.quantity = 1;
      let mut mineral = char_asset(101, 42, None);
      mineral.type_id = 24;
      mineral.quantity = 1_000;
      replace_for_character(&db, 42, &[ship, mineral]).await.unwrap();

      let all = inventory_totals_for_character(&db, 42, "", &[], None).await.unwrap();
      let ships_only = inventory_totals_for_character(&db, 42, "category:ship", &[], None)
        .await
        .unwrap();

      assert_eq!(all.items, 1_001);
      assert_eq!(all.value, 5_100.0);
      assert_eq!(ships_only.items, 1);
      assert_eq!(ships_only.value, 100.0);
      assert_eq!(ships_only.locations, 1);
    }

    #[tokio::test]
    async fn it_restricts_the_totals_to_the_selected_locations() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_item_type(&db, 587, "Rifter", 25, "Frigate", "Ship").await;
      let mut here = char_asset(100, 42, None);
      here.type_id = 587;
      here.quantity = 2;
      here.location_id = 60_000_001;
      let mut there = char_asset(101, 42, None);
      there.type_id = 587;
      there.quantity = 3;
      there.location_id = 60_000_002;
      replace_for_character(&db, 42, &[here, there]).await.unwrap();

      let all = inventory_totals_for_character(&db, 42, "", &[], None).await.unwrap();
      let one = inventory_totals_for_character(&db, 42, "", &[60_000_001], None)
        .await
        .unwrap();

      assert_eq!(all.items, 5, "an empty location predicate counts every location");
      assert_eq!(
        one.items, 2,
        "the badge total honors the same location filter as the page"
      );
    }

    #[tokio::test]
    async fn it_yields_zeroed_totals_for_an_empty_scope() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;

      let totals = inventory_totals_for_character(&db, 42, "", &[], None).await.unwrap();

      assert_eq!(totals, InventoryTotals::default());
    }
  }

  mod lazy_children {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_counts_direct_children_without_loading_them() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let mut root = char_asset(100, 42, None);
      root.is_container = true;
      replace_for_character(
        &db,
        42,
        &[
          root,
          char_asset(101, 42, Some(100)),
          char_asset(102, 42, Some(100)),
          char_asset(103, 42, Some(101)),
        ],
      )
      .await
      .unwrap();

      assert_eq!(child_count_for_character(&db, 42, 100).await.unwrap(), 2);
      assert_eq!(child_count_for_character(&db, 42, 101).await.unwrap(), 1);
      assert_eq!(child_count_for_character(&db, 42, 102).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn it_fetches_corporation_children() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      authorize_corp(&db).await;
      seed_item_type(&db, 587, "Rifter", 25, "Frigate", "Ship").await;
      let mut root = corp_asset(100, CORP_ID, None);
      root.is_container = true;
      root.type_id = 587;
      let mut child = corp_asset(101, CORP_ID, Some(100));
      child.type_id = 587;
      replace_for_corporation(&db, CORP_ID, &[root, child]).await.unwrap();

      let children = children_render_for_corporation(&db, CORP_ID, 100, 0.5).await.unwrap();

      assert_eq!(children.iter().map(|r| r.item_id).collect::<Vec<_>>(), [101]);
      assert_eq!(child_count_for_corporation(&db, CORP_ID, 100).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn it_fetches_render_ready_direct_children_one_level_by_container_id() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_item_type(&db, 587, "Rifter", 25, "Frigate", "Ship").await;
      let mut root = char_asset(100, 42, None);
      root.is_container = true;
      root.type_id = 587;
      let mut child_a = char_asset(101, 42, Some(100));
      child_a.is_container = true;
      child_a.type_id = 587;
      let mut child_b = char_asset(102, 42, Some(100));
      child_b.type_id = 587;
      let mut grandchild = char_asset(103, 42, Some(101));
      grandchild.type_id = 587;
      replace_for_character(&db, 42, &[root, child_a, child_b, grandchild])
        .await
        .unwrap();

      let children = children_render_for_character(&db, 42, 100, 0.5).await.unwrap();

      assert_eq!(children.iter().map(|r| r.item_id).collect::<Vec<_>>(), [101, 102]);
      assert_eq!(children[0].type_name, "Rifter");
      assert_eq!(children[0].container_id, Some(100));
    }
  }

  mod location_label {
    use pretty_assertions::assert_eq;

    use super::*;

    const STRUCTURE_ID: i64 = 1_021_000_000_000;

    #[tokio::test]
    async fn it_carries_the_resolved_label_through_the_inventory_row() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_item_type(&db, 587, "Rifter", 25, "Frigate", "Ship").await;
      seed_named_station(&db, 60_003_760, "Jita IV - Moon 4 - Caldari Navy Assembly Plant").await;
      replace_for_character(&db, 42, &[char_asset(100, 42, None)])
        .await
        .unwrap();

      let query = InventoryQuery {
        cursor: None,
        direction: SortDirection::Ascending,
        filter: "",
        limit: 100,
        location_ids: &[],
        me_id: None,
        reproc_yield: 0.5,
        sort: SortColumn::Name,
      };
      let rows = inventory_page_for_character(&db, 42, &query).await.unwrap();

      assert_eq!(
        rows[0].location_label.as_deref(),
        Some("Jita IV - Moon 4 - Caldari Navy Assembly Plant")
      );
    }

    #[tokio::test]
    async fn it_leaves_the_render_label_null_when_unresolved() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_item_type(&db, 587, "Rifter", 25, "Frigate", "Ship").await;
      replace_for_character(&db, 42, &[char_asset(100, 42, None)])
        .await
        .unwrap();

      let rows = render_for_character(&db, 42).await.unwrap();

      assert_eq!(
        rows[0].location_label, None,
        "an unresolved location never renders a raw id"
      );
    }

    #[tokio::test]
    async fn it_marks_inaccessible_per_owner_not_across_owners() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_character(&db, 43).await;
      seed_item_type(&db, 587, "Rifter", 25, "Frigate", "Ship").await;
      seed_named_structure(&db, STRUCTURE_ID, "Some Citadel").await;
      let mut asset = char_asset(100, 43, None);
      asset.location_id = STRUCTURE_ID;
      asset.location_type = "structure".to_owned();
      replace_for_character(&db, 43, &[asset]).await.unwrap();
      sde::mark_inaccessible_structure(&db, 42, OwnerType::Character, STRUCTURE_ID)
        .await
        .unwrap();

      let rows = render_for_character(&db, 43).await.unwrap();

      assert_eq!(
        rows[0].location_label.as_deref(),
        Some("Some Citadel"),
        "43 sees the resolved name; 42's inaccessible mark does not leak across owners"
      );
    }

    #[tokio::test]
    async fn it_renders_inaccessible_structure_for_a_marked_location() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_item_type(&db, 587, "Rifter", 25, "Frigate", "Ship").await;
      let mut asset = char_asset(100, 42, None);
      asset.location_id = STRUCTURE_ID;
      asset.location_type = "structure".to_owned();
      replace_for_character(&db, 42, &[asset]).await.unwrap();
      sde::mark_inaccessible_structure(&db, 42, OwnerType::Character, STRUCTURE_ID)
        .await
        .unwrap();

      let rows = render_for_character(&db, 42).await.unwrap();

      assert_eq!(rows[0].location_label.as_deref(), Some("Inaccessible Structure"));
    }

    #[tokio::test]
    async fn it_resolves_a_station_name_into_the_render_label() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_item_type(&db, 587, "Rifter", 25, "Frigate", "Ship").await;
      seed_named_station(&db, 60_003_760, "Jita IV - Moon 4 - Caldari Navy Assembly Plant").await;
      replace_for_character(&db, 42, &[char_asset(100, 42, None)])
        .await
        .unwrap();

      let rows = render_for_character(&db, 42).await.unwrap();

      assert_eq!(
        rows[0].location_label.as_deref(),
        Some("Jita IV - Moon 4 - Caldari Navy Assembly Plant")
      );
    }
  }

  mod node_rollup {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_rolls_up_an_empty_container_to_zero() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let mut root = char_asset(100, 42, None);
      root.is_container = true;
      replace_for_character(&db, 42, &[root]).await.unwrap();

      assert_eq!(
        node_rollup_for_character(&db, 42, 100).await.unwrap(),
        NodeRollup::default()
      );
    }

    #[tokio::test]
    async fn it_rolls_up_value_and_count_over_the_whole_subtree_in_sql() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_item_type(&db, 587, "Rifter", 25, "Frigate", "Ship").await;
      seed_price(&db, 587, 100.0).await;
      let mut root = char_asset(100, 42, None);
      root.is_container = true;
      root.type_id = 587;
      let mut child_a = char_asset(101, 42, Some(100));
      child_a.type_id = 587;
      child_a.quantity = 2;
      let mut child_b = char_asset(102, 42, Some(100));
      child_b.is_container = true;
      child_b.type_id = 587;
      child_b.quantity = 1;
      let mut grandchild = char_asset(103, 42, Some(102));
      grandchild.type_id = 587;
      grandchild.quantity = 5;
      replace_for_character(&db, 42, &[root, child_a, child_b, grandchild])
        .await
        .unwrap();

      let rollup = node_rollup_for_character(&db, 42, 100).await.unwrap();

      assert_eq!(rollup.items, 8);
      assert_eq!(rollup.value, 800.0);
    }
  }

  mod on_hand_at_build_sites {
    use pretty_assertions::assert_eq;

    use super::*;

    const SITE_A: i64 = 60_003_760;

    const SITE_B: i64 = 60_008_494;

    #[tokio::test]
    async fn it_is_empty_with_no_locations() {
      let db = store::open_test().await.unwrap();

      assert!(on_hand_at_build_sites(&db, &[]).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_keys_by_location_and_type() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let mut at_a = char_asset(100, 42, None);
      at_a.location_id = SITE_A;
      at_a.type_id = 34;
      at_a.quantity = 10;
      let mut at_b = char_asset(101, 42, None);
      at_b.location_id = SITE_B;
      at_b.type_id = 34;
      at_b.quantity = 5;

      replace_for_character(&db, 42, &[at_a, at_b]).await.unwrap();

      let totals = on_hand_at_build_sites(&db, &[SITE_A, SITE_B]).await.unwrap();
      assert_eq!(totals.get(&(SITE_A, 34)).copied(), Some(10));
      assert_eq!(totals.get(&(SITE_B, 34)).copied(), Some(5));
    }

    #[tokio::test]
    async fn it_omits_corporation_stock_when_unauthorized() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let mut corp_stock = corp_asset(300, CORP_ID, None);
      corp_stock.location_id = SITE_A;
      corp_stock.type_id = 34;
      corp_stock.quantity = 8;

      replace_for_corporation(&db, CORP_ID, &[corp_stock]).await.unwrap();

      assert!(on_hand_at_build_sites(&db, &[SITE_A]).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_resolves_the_on_hand_group_by_through_the_location_index() {
      use sqlx::Row;

      let db = store::open_test().await.unwrap();

      let rows = sqlx::query(
        "EXPLAIN QUERY PLAN SELECT location_id, type_id, SUM(quantity) FROM character_assets \
        WHERE container_id IS NULL AND location_id IN (?) GROUP BY location_id, type_id",
      )
      .bind(SITE_A)
      .fetch_all(&db.0)
      .await
      .unwrap();
      let plan: Vec<String> = rows.iter().map(|row| row.get::<String, _>("detail")).collect();

      assert!(
        plan
          .iter()
          .any(|step| step.contains("idx_character_assets_location_id")),
        "accumulate_on_hand searches via the location index, not a full scan: {plan:?}"
      );
      assert!(
        !plan.iter().any(|step| step.contains("SCAN")),
        "the location index avoids a full table scan: {plan:?}"
      );
      assert!(
        !plan.iter().any(|step| step.contains("TEMP B-TREE FOR GROUP BY")),
        "the composite index orders the GROUP BY, so no temporary b-tree is built: {plan:?}"
      );
    }

    #[tokio::test]
    async fn it_sums_character_and_corporation_stock_together() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      authorize_corp(&db).await;
      let mut char_stock = char_asset(100, 42, None);
      char_stock.location_id = SITE_A;
      char_stock.type_id = 34;
      char_stock.quantity = 12;
      let mut corp_stock = corp_asset(300, CORP_ID, None);
      corp_stock.location_id = SITE_A;
      corp_stock.type_id = 34;
      corp_stock.quantity = 8;
      let mut corp_nested = corp_asset(301, CORP_ID, Some(300));
      corp_nested.location_id = SITE_A;
      corp_nested.type_id = 34;
      corp_nested.quantity = 100;

      replace_for_character(&db, 42, &[char_stock]).await.unwrap();
      replace_for_corporation(&db, CORP_ID, &[corp_stock, corp_nested])
        .await
        .unwrap();

      let totals = on_hand_at_build_sites(&db, &[SITE_A]).await.unwrap();
      assert_eq!(totals.get(&(SITE_A, 34)).copied(), Some(20));
    }

    #[tokio::test]
    async fn it_sums_only_in_hangar_items_excluding_nested_ones() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let mut hangar = char_asset(100, 42, None);
      hangar.type_id = 34;
      hangar.quantity = 30;
      let mut container = char_asset(200, 42, None);
      container.is_container = true;
      container.type_id = 11_488;
      let mut nested = char_asset(201, 42, Some(200));
      nested.type_id = 34;
      nested.quantity = 7;

      replace_for_character(&db, 42, &[hangar, container, nested])
        .await
        .unwrap();

      let totals = on_hand_at_build_sites(&db, &[SITE_A]).await.unwrap();
      assert_eq!(totals.get(&(SITE_A, 34)).copied(), Some(30));
      assert_eq!(totals.get(&(SITE_A, 11_488)).copied(), Some(1));
    }
  }

  mod referenced_locations {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_deduplicates_a_place_referenced_by_a_character_and_a_corporation() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      replace_for_character(&db, 42, &[char_asset(100, 42, None)])
        .await
        .unwrap();
      replace_for_corporation(&db, CORP_ID, &[corp_asset(200, CORP_ID, None)])
        .await
        .unwrap();

      let locations = referenced_locations(&db).await.unwrap();

      assert_eq!(
        locations,
        vec![ReferencedLocation {
          location_id: 60_003_760,
          location_type: "station".to_owned(),
        }],
        "the shared station appears once despite two owners referencing it"
      );
    }

    #[tokio::test]
    async fn it_excludes_item_nested_roots_and_keeps_distinct_places() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let mut in_station = char_asset(100, 42, None);
      in_station.location_id = 60_003_760;
      in_station.location_type = "station".to_owned();
      let mut in_structure = char_asset(101, 42, None);
      in_structure.location_id = 1_021_000_000_000;
      in_structure.location_type = "structure".to_owned();
      let mut nested = char_asset(102, 42, Some(100));
      nested.location_id = 100;
      nested.location_type = "item".to_owned();
      replace_for_character(&db, 42, &[in_station, in_structure, nested])
        .await
        .unwrap();

      let locations = referenced_locations(&db).await.unwrap();

      assert_eq!(
        locations,
        vec![
          ReferencedLocation {
            location_id: 60_003_760,
            location_type: "station".to_owned(),
          },
          ReferencedLocation {
            location_id: 1_021_000_000_000,
            location_type: "structure".to_owned(),
          },
        ],
        "distinct places are kept (ordered by id) and the `item`-nested row is excluded"
      );
    }
  }

  mod render_join {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_carries_the_custom_name_alongside_the_type_name_on_render_rows() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      authorize_corp(&db).await;
      seed_item_type(&db, 587, "Rifter", 25, "Frigate", "Ship").await;
      let mut named = char_asset(100, 42, None);
      named.name = Some("My Scout".to_owned());
      let plain = char_asset(101, 42, None);
      replace_for_character(&db, 42, &[named, plain]).await.unwrap();
      let mut corp_named = corp_asset(200, CORP_ID, None);
      corp_named.name = Some("Corp Stash".to_owned());
      replace_for_corporation(&db, CORP_ID, &[corp_named]).await.unwrap();

      let char_rows = render_for_character(&db, 42).await.unwrap();
      let corp_rows = render_for_corporation(&db, CORP_ID).await.unwrap();

      assert_eq!(char_rows[0].name.as_deref(), Some("My Scout"));
      assert_eq!(char_rows[0].type_name, "Rifter");
      assert_eq!(char_rows[1].name, None, "an unnamed item keeps a null custom name");

      assert_eq!(corp_rows[0].name.as_deref(), Some("Corp Stash"));
      assert_eq!(corp_rows[0].type_name, "Rifter");
    }

    #[tokio::test]
    async fn it_excludes_assets_whose_type_is_unseeded_rather_than_yielding_a_blank_row() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_item_type(&db, 587, "Rifter", 25, "Frigate", "Ship").await;
      let mut resolved = char_asset(100, 42, None);
      resolved.type_id = 587;
      let mut orphan = char_asset(101, 42, None);
      orphan.type_id = 999_999;
      replace_for_character(&db, 42, &[resolved, orphan]).await.unwrap();

      let rows = render_for_character(&db, 42).await.unwrap();

      assert_eq!(rows.iter().map(|r| r.item_id).collect::<Vec<_>>(), [100]);
    }

    #[tokio::test]
    async fn it_falls_back_to_assembled_volume_when_packaged_is_null() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let category = ItemCategory {
        id: 60,
        icon_id: None,
        name: "Ship".to_owned(),
        published: true,
      };
      let group = ItemGroup {
        category_id: 60,
        icon_id: None,
        id: 25,
        name: "Frigate".to_owned(),
        published: true,
      };
      let item_type = ItemType {
        capacity: None,
        description: Some("Test item".to_owned()),
        dogma_attributes: "[]".to_owned(),
        group_id: 25,
        icon_id: None,
        id: 587,
        market_group_id: None,
        name: "Rifter".to_owned(),
        packaged_volume: None,
        portion_size: None,
        published: true,
        radius: None,
        volume: Some(27_289.0),
      };
      sde::insert_item_type_with_hierarchy(&db, &item_type, &group, &category)
        .await
        .unwrap();
      let mut asset = char_asset(100, 42, None);
      asset.type_id = 587;
      replace_for_character(&db, 42, &[asset]).await.unwrap();

      let rows = render_for_character(&db, 42).await.unwrap();

      assert_eq!(rows[0].volume, Some(27_289.0));
    }

    #[tokio::test]
    async fn it_maps_category_names_to_render_keys() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_item_type(&db, 1, "Augmenter", 1, "Cyber", "Augmentation").await;
      seed_item_type(&db, 2, "Tritanium", 2, "Mineral", "Mineral").await;
      seed_item_type(&db, 3, "Skillbook", 3, "Skills", "Skill").await;
      seed_item_type(&db, 4, "Widget", 4, "Misc", "Totally Unknown").await;
      let assets: Vec<_> = [1, 2, 3, 4]
        .into_iter()
        .map(|t| {
          let mut a = char_asset(100 + t, 42, None);
          a.type_id = t;
          a
        })
        .collect();
      replace_for_character(&db, 42, &assets).await.unwrap();

      let keys: Vec<_> = render_for_character(&db, 42)
        .await
        .unwrap()
        .into_iter()
        .map(|r| r.category)
        .collect();

      assert_eq!(keys, ["implant", "material", "book", "commodity"]);
    }

    #[tokio::test]
    async fn it_populates_every_metadata_cell_from_the_sde_for_a_resolved_type() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_item_type(&db, 587, "Rifter", 25, "Frigate", "Ship").await;
      let mut asset = char_asset(100, 42, None);
      asset.type_id = 587;
      replace_for_character(&db, 42, &[asset]).await.unwrap();

      let rows = render_for_character(&db, 42).await.unwrap();

      assert_eq!(rows.len(), 1);
      let row = &rows[0];
      assert_eq!(row.item_id, 100);
      assert_eq!(row.type_id, 587);
      assert_eq!(row.type_name, "Rifter");
      assert_eq!(row.group_name, "Frigate");
      assert_eq!(row.category, "ship");
      assert_eq!(row.icon_id, Some(1587));
      assert_eq!(row.volume, Some(2.5));
    }

    #[tokio::test]
    async fn it_renders_corporation_assets_from_the_sde() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      authorize_corp(&db).await;
      seed_item_type(&db, 587, "Rifter", 25, "Frigate", "Ship").await;
      let mut asset = corp_asset(100, CORP_ID, None);
      asset.type_id = 587;
      replace_for_corporation(&db, CORP_ID, &[asset]).await.unwrap();

      let rows = render_for_corporation(&db, CORP_ID).await.unwrap();

      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].type_name, "Rifter");
      assert_eq!(rows[0].category, "ship");
    }
  }

  mod reproc_value {
    use pretty_assertions::assert_eq;

    use super::*;

    const TRIT: i64 = 34;
    const PYE: i64 = 35;

    async fn seed_reprocessable_type(db: &Database, type_id: i64, portion_size: i64) {
      seed_item_type(db, type_id, "Refinable", 25, "Frigate", "Ship").await;
      sqlx::query("UPDATE item_types SET portion_size = ? WHERE id = ?")
        .bind(portion_size)
        .bind(type_id)
        .execute(db.writer())
        .await
        .unwrap();
    }

    async fn seed_materials(db: &Database, type_id: i64, materials: &[(i64, i64)]) {
      let rows: Vec<crate::store::model::TypeMaterial> = materials
        .iter()
        .map(|&(material_type_id, quantity)| crate::store::model::TypeMaterial {
          material_type_id,
          quantity,
          type_id,
        })
        .collect();
      sde::seed_many_type_materials(db, &rows).await.unwrap();
    }

    fn query() -> InventoryQuery<'static> {
      InventoryQuery {
        cursor: None,
        direction: SortDirection::Descending,
        filter: "",
        limit: 100,
        location_ids: &[],
        me_id: None,
        reproc_yield: 0.5,
        sort: SortColumn::Value,
      }
    }

    #[tokio::test]
    async fn it_computes_reproc_value_from_materials_prices_yield_and_portion() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_reprocessable_type(&db, 587, 100).await;
      seed_price(&db, TRIT, 5.0).await;
      seed_price(&db, PYE, 10.0).await;
      seed_materials(&db, 587, &[(TRIT, 1_000), (PYE, 500)]).await;
      let mut asset = char_asset(100, 42, None);
      asset.type_id = 587;
      asset.quantity = 300;
      replace_for_character(&db, 42, &[asset]).await.unwrap();

      let rows = inventory_page_for_character(&db, 42, &query()).await.unwrap();

      // per_unit = 1000*5 + 500*10 = 10_000; floor(300/100) = 3; yield 0.5 => 10_000 * 0.5 * 3.
      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].reproc_value, 15_000.0);
    }

    #[tokio::test]
    async fn it_scales_the_reproc_value_with_the_configured_yield() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_reprocessable_type(&db, 587, 1).await;
      seed_price(&db, TRIT, 5.0).await;
      seed_materials(&db, 587, &[(TRIT, 100)]).await;
      let mut asset = char_asset(100, 42, None);
      asset.type_id = 587;
      asset.quantity = 1;
      replace_for_character(&db, 42, &[asset]).await.unwrap();

      let mut query = query();
      query.reproc_yield = 1.0;
      let rows = inventory_page_for_character(&db, 42, &query).await.unwrap();

      // per_unit = 100 * 5 = 500; full yield, one portion.
      assert_eq!(rows[0].reproc_value, 500.0);
    }

    #[tokio::test]
    async fn it_flags_worth_reprocessing_only_when_reproc_beats_sell() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_reprocessable_type(&db, 587, 1).await;
      // Material value 1000/unit; refined at 0.5 yield => 500/unit reproc.
      seed_price(&db, TRIT, 1_000.0).await;
      seed_materials(&db, 587, &[(TRIT, 1)]).await;
      // Sell value below the 500 reproc => worth reprocessing.
      seed_price(&db, 587, 100.0).await;
      let mut asset = char_asset(100, 42, None);
      asset.type_id = 587;
      asset.quantity = 1;
      replace_for_character(&db, 42, &[asset]).await.unwrap();

      let rows = inventory_page_for_character(&db, 42, &query()).await.unwrap();

      assert_eq!(rows[0].reproc_value, 500.0);
      assert_eq!(rows[0].value, 100.0);
      assert!(rows[0].worth_reprocessing());

      // Raise the sell price above the reproc value => no longer worth it.
      sqlx::query("UPDATE market_prices SET adjusted_price = 1000.0 WHERE type_id = 587")
        .execute(db.writer())
        .await
        .unwrap();
      let rows = inventory_page_for_character(&db, 42, &query()).await.unwrap();
      assert!(!rows[0].worth_reprocessing());
    }

    #[tokio::test]
    async fn it_yields_zero_for_a_partial_stack_below_one_portion() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_reprocessable_type(&db, 587, 100).await;
      seed_price(&db, TRIT, 5.0).await;
      seed_materials(&db, 587, &[(TRIT, 1_000)]).await;
      let mut asset = char_asset(100, 42, None);
      asset.type_id = 587;
      asset.quantity = 50;
      replace_for_character(&db, 42, &[asset]).await.unwrap();

      let rows = inventory_page_for_character(&db, 42, &query()).await.unwrap();

      assert_eq!(rows[0].reproc_value, 0.0);
      assert!(!rows[0].worth_reprocessing());
    }

    #[tokio::test]
    async fn it_yields_zero_for_a_type_with_no_materials() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_reprocessable_type(&db, 587, 1).await;
      let mut asset = char_asset(100, 42, None);
      asset.type_id = 587;
      asset.quantity = 10;
      replace_for_character(&db, 42, &[asset]).await.unwrap();

      let rows = inventory_page_for_character(&db, 42, &query()).await.unwrap();

      assert_eq!(rows[0].reproc_value, 0.0);
    }

    #[tokio::test]
    async fn it_yields_zero_for_a_blueprint_copy() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_reprocessable_type(&db, 587, 1).await;
      seed_price(&db, TRIT, 5.0).await;
      seed_materials(&db, 587, &[(TRIT, 1_000)]).await;
      let mut asset = char_asset(100, 42, None);
      asset.type_id = 587;
      asset.quantity = 10;
      asset.is_blueprint_copy = Some(true);
      replace_for_character(&db, 42, &[asset]).await.unwrap();

      let rows = inventory_page_for_character(&db, 42, &query()).await.unwrap();

      assert_eq!(rows[0].reproc_value, 0.0);
      assert!(!rows[0].worth_reprocessing());
    }

    #[tokio::test]
    async fn it_matches_reproc_value_across_combined_scope() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      authorize_corp(&db).await;
      seed_reprocessable_type(&db, 587, 100).await;
      seed_price(&db, TRIT, 5.0).await;
      seed_materials(&db, 587, &[(TRIT, 1_000)]).await;

      let mut char_a = char_asset(100, 42, None);
      char_a.type_id = 587;
      char_a.quantity = 300;
      replace_for_character(&db, 42, &[char_a]).await.unwrap();

      let mut corp_a = corp_asset(200, CORP_ID, None);
      corp_a.type_id = 587;
      corp_a.quantity = 300;
      replace_for_corporation(&db, CORP_ID, &[corp_a]).await.unwrap();

      let single = inventory_page_for_character(&db, 42, &query()).await.unwrap();
      let combined = inventory_page_for_combined(&db, &[42], &[CORP_ID], &query())
        .await
        .unwrap();

      // per_unit = 1000*5 = 5000; floor(300/100)=3; yield 0.5 => 7500 per stack, both scopes.
      assert_eq!(single[0].reproc_value, 7_500.0);
      assert_eq!(combined.len(), 2);
      for row in &combined {
        assert_eq!(row.reproc_value, 7_500.0);
      }
    }
  }
}

#[cfg(test)]
mod stockpile_tests {
  use super::*;
  use crate::store::{
    self,
    model::{Alliance, Bloodline, Character, Corporation, Gender, Race},
    repo::character,
  };

  async fn seed_character(db: &Database, id: i64) {
    let corp_id = 90_000_001;
    let alliance_id = 99_000_001;
    let alliance = Alliance::new(alliance_id, corp_id, id, "2003-01-01", "Test Alliance", "TST");
    let race = Race::new(2, alliance_id, "A race.", "Caldari");
    let mut corp = Corporation::new(corp_id, "Test Corp", "TSC");
    corp.set_ceo_id(id);
    corp.set_creator_id(id);
    corp.set_member_count(1);
    corp.set_tax_rate(0.0);
    let bloodline = Bloodline::new(1, corp_id, 2, 3, "A bloodline.", 4, 5, "Civire", 4, 4);
    let character = Character::new(id, 1, corp_id, 2, "2003-05-12", Gender::Male, "Pilot");
    character::insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
      .await
      .unwrap();
  }

  async fn seed_asset(db: &Database, item_id: i64, character_id: i64, type_id: i64, location_id: i64, quantity: i64) {
    sqlx::query(
      "INSERT INTO character_assets \
        (item_id, character_id, type_id, location_id, location_type, location_flag, quantity) \
      VALUES (?, ?, ?, ?, 'station', 'Hangar', ?)",
    )
    .bind(item_id)
    .bind(character_id)
    .bind(type_id)
    .bind(location_id)
    .bind(quantity)
    .execute(db.writer())
    .await
    .unwrap();
  }

  mod create {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_inserts_an_unscoped_stockpile_with_items() {
      let db = store::open_test().await.unwrap();

      let created = create(&db, "Supply Cache", None, None, &[(34, 1000), (35, 500)])
        .await
        .unwrap();

      assert!(created.stockpile.id() > 0);
      assert_eq!(created.stockpile.name(), "Supply Cache");
      assert_eq!(created.stockpile.character_scope(), &None);
      assert_eq!(created.stockpile.location_id(), None);
      assert_eq!(created.items.len(), 2);
      assert_eq!(created.items[0].type_id(), 34);
      assert_eq!(created.items[0].target_quantity(), 1000);
    }

    #[tokio::test]
    async fn it_records_optional_character_and_location_scopes() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1001).await;

      let created = create(
        &db,
        "Filtered",
        Some("corp:cobalt".to_string()),
        Some(60_003_760),
        &[(34, 100)],
      )
      .await
      .unwrap();

      assert_eq!(created.stockpile.character_scope(), &Some("corp:cobalt".to_string()));
      assert_eq!(created.stockpile.location_id(), Some(60_003_760));
    }
  }

  mod delete {
    use super::*;

    #[tokio::test]
    async fn it_is_a_no_op_for_a_missing_stockpile() {
      let db = store::open_test().await.unwrap();

      delete(&db, 999_999).await.unwrap();
    }

    #[tokio::test]
    async fn it_removes_the_stockpile_and_cascades_its_items() {
      let db = store::open_test().await.unwrap();
      let created = create(&db, "Doomed", None, None, &[(34, 1)]).await.unwrap();
      let id = created.stockpile.id();

      delete(&db, id).await.unwrap();

      assert!(get(&db, id).await.unwrap().is_none());
      assert!(items(&db, id).await.unwrap().is_empty());
    }
  }

  mod fill_status {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{
      model::{Constellation, ItemCategory, ItemGroup, Region, SolarSystem, Station, Structure},
      repo::{org, sde},
    };

    const CONSTELLATION_ID: i64 = 20_000_020;

    const OTHER_STATION: i64 = 60_003_761;

    const OTHER_SYSTEM_ID: i64 = 30_000_142;

    const OWNER_CORP: i64 = 90_000_001;

    const REGION_ID: i64 = 10_000_002;

    const STATION_A: i64 = 60_003_760;

    const STATION_B: i64 = 60_008_494;

    const STATION_TYPE: i64 = 1529;

    const SYSTEM_ID: i64 = 31_000_005;

    const SYSTEM_STATION: i64 = 60_015_150;

    const SYSTEM_STATION_2: i64 = 60_015_151;

    const SYSTEM_STRUCTURE: i64 = 1_021_000_000_001;

    async fn seed_contained_asset(
      db: &Database,
      item_id: i64,
      character_id: i64,
      type_id: i64,
      container_id: i64,
      quantity: i64,
    ) {
      sqlx::query(
        "INSERT INTO character_assets \
          (item_id, character_id, type_id, location_id, location_type, location_flag, quantity, container_id, depth) \
        VALUES (?, ?, ?, ?, 'item', 'Cargo', ?, ?, 1)",
      )
      .bind(item_id)
      .bind(character_id)
      .bind(type_id)
      .bind(container_id)
      .bind(quantity)
      .bind(container_id)
      .execute(db.writer())
      .await
      .unwrap();
    }

    async fn seed_corp_asset(
      db: &Database,
      item_id: i64,
      corporation_id: i64,
      type_id: i64,
      location_id: i64,
      quantity: i64,
    ) {
      sqlx::query(
        "INSERT INTO corporation_assets \
          (item_id, corporation_id, type_id, location_id, location_type, location_flag, quantity) \
        VALUES (?, ?, ?, ?, 'station', 'CorpSAG1', ?)",
      )
      .bind(item_id)
      .bind(corporation_id)
      .bind(type_id)
      .bind(location_id)
      .bind(quantity)
      .execute(db.writer())
      .await
      .unwrap();
    }

    async fn seed_corporation(db: &Database, id: i64) {
      let mut corp = Corporation::new(id, "Other Corp", "OTH");
      corp.set_ceo_id(1);
      corp.set_creator_id(1);
      corp.set_member_count(1);
      corp.set_tax_rate(0.0);
      org::upsert_corporation(db, &corp).await.unwrap();
    }

    async fn seed_geography(db: &Database, region_id: i64, constellation_id: i64, system_id: i64) {
      sde::upsert_region(
        db,
        &Region {
          description: None,
          id: region_id,
          name: "Region".to_owned(),
        },
      )
      .await
      .unwrap();
      sde::upsert_constellation(
        db,
        &Constellation {
          id: constellation_id,
          name: "Constellation".to_owned(),
          position_x: 0.0,
          position_y: 0.0,
          position_z: 0.0,
          region_id,
        },
      )
      .await
      .unwrap();
      sde::upsert_solar_system(
        db,
        &SolarSystem {
          constellation_id,
          id: system_id,
          name: "System".to_owned(),
          position_x: 0.0,
          position_y: 0.0,
          position_z: 0.0,
          security_class: None,
          security_status: 0.5,
          star_id: None,
        },
      )
      .await
      .unwrap();
    }

    async fn seed_station(db: &Database, station_id: i64, system_id: i64) {
      seed_station_type(db).await;
      sde::upsert_station(
        db,
        &Station {
          id: station_id,
          max_dockable_ship_volume: 0.0,
          name: "Station".to_owned(),
          office_rental_cost: 0.0,
          owner: None,
          position_x: 0.0,
          position_y: 0.0,
          position_z: 0.0,
          race_id: None,
          reprocessing_efficiency: 0.0,
          reprocessing_stations_take: 0.0,
          services: "[]".to_owned(),
          system_id,
          type_id: STATION_TYPE,
        },
      )
      .await
      .unwrap();
    }

    async fn seed_station_in(db: &Database, station_id: i64, system_id: i64) {
      seed_system(db, system_id).await;
      seed_station(db, station_id, system_id).await;
    }

    async fn seed_station_in_region(
      db: &Database,
      station_id: i64,
      region_id: i64,
      constellation_id: i64,
      system_id: i64,
    ) {
      seed_geography(db, region_id, constellation_id, system_id).await;
      seed_station(db, station_id, system_id).await;
    }

    async fn seed_station_type(db: &Database) {
      sde::upsert_item_category(
        db,
        &ItemCategory {
          icon_id: None,
          id: 6,
          name: "Station".to_owned(),
          published: true,
        },
      )
      .await
      .unwrap();
      sde::upsert_item_group(
        db,
        &ItemGroup {
          category_id: 6,
          icon_id: None,
          id: 15,
          name: "Station".to_owned(),
          published: true,
        },
      )
      .await
      .unwrap();
      sqlx::query("INSERT OR IGNORE INTO item_types (id, group_id, description, name, published) VALUES (?, 15, 'Station', 'Station', 1)")
        .bind(STATION_TYPE)
        .execute(db.writer())
        .await
        .unwrap();
    }

    async fn seed_structure_in(db: &Database, structure_id: i64, system_id: i64) {
      seed_system(db, system_id).await;
      sde::upsert_structure(
        db,
        &Structure {
          id: structure_id,
          name: "Structure".to_owned(),
          owner_id: OWNER_CORP,
          position_x: None,
          position_y: None,
          position_z: None,
          solar_system_id: system_id,
          type_id: None,
        },
      )
      .await
      .unwrap();
    }

    async fn seed_system(db: &Database, id: i64) {
      seed_geography(db, REGION_ID, CONSTELLATION_ID, id).await;
    }

    #[tokio::test]
    async fn a_character_scope_restricts_to_that_character_and_their_corp() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1001).await;
      seed_character(&db, 1002).await;
      seed_corporation(&db, 90_000_002).await;
      seed_station_in(&db, SYSTEM_STATION, SYSTEM_ID).await;
      let created = create(&db, "Thera", None, Some(SYSTEM_ID), &[(34, 1000)])
        .await
        .unwrap();
      seed_asset(&db, 1, 1001, 34, SYSTEM_STATION, 100).await;
      seed_asset(&db, 2, 1002, 34, SYSTEM_STATION, 50).await;
      seed_corp_asset(&db, 3, OWNER_CORP, 34, SYSTEM_STATION, 30).await;
      seed_corp_asset(&db, 4, 90_000_002, 34, SYSTEM_STATION, 70).await;

      let scoped = super::super::fill_status(&db, created.stockpile.id(), &[1001])
        .await
        .unwrap()
        .unwrap();
      let scoped_pair = super::super::fill_status(&db, created.stockpile.id(), &[1001, 1002])
        .await
        .unwrap()
        .unwrap();
      let unscoped = super::super::fill_status(&db, created.stockpile.id(), &[])
        .await
        .unwrap()
        .unwrap();

      assert_eq!(scoped.items[0].have_quantity, 130);
      assert_eq!(scoped_pair.items[0].have_quantity, 180);
      assert_eq!(unscoped.items[0].have_quantity, 250);
    }

    #[tokio::test]
    async fn a_constellation_pile_counts_an_asset_at_a_station_in_the_constellation() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1001).await;
      seed_station_in(&db, SYSTEM_STATION, SYSTEM_ID).await;
      let created = create(&db, "Const", None, Some(CONSTELLATION_ID), &[(34, 10)])
        .await
        .unwrap();
      seed_asset(&db, 1, 1001, 34, SYSTEM_STATION, 5).await;

      let fill = super::super::fill_status(&db, created.stockpile.id(), &[])
        .await
        .unwrap()
        .unwrap();

      assert_eq!(fill.items[0].have_quantity, 5);
    }

    #[tokio::test]
    async fn a_fully_stocked_stockpile_is_full() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1001).await;
      let created = create(&db, "Cache", None, None, &[(34, 100), (35, 200)]).await.unwrap();
      seed_asset(&db, 1, 1001, 34, STATION_A, 100).await;
      seed_asset(&db, 2, 1001, 35, STATION_A, 250).await;

      let fill = super::super::fill_status(&db, created.stockpile.id(), &[])
        .await
        .unwrap()
        .unwrap();

      assert_eq!(fill.overall_pct(), 1.0);
      assert!(fill.is_full());
    }

    #[tokio::test]
    async fn a_pile_counts_a_corp_asset_at_the_location() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1001).await;
      seed_station_in(&db, SYSTEM_STATION, SYSTEM_ID).await;
      let created = create(&db, "Thera", None, Some(SYSTEM_ID), &[(34, 10)]).await.unwrap();
      seed_corp_asset(&db, 1, OWNER_CORP, 34, SYSTEM_STATION, 9).await;

      let fill = super::super::fill_status(&db, created.stockpile.id(), &[])
        .await
        .unwrap()
        .unwrap();

      assert_eq!(fill.items[0].have_quantity, 9);
    }

    #[tokio::test]
    async fn a_region_pile_counts_an_asset_at_a_station_in_the_region() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1001).await;
      seed_station_in(&db, SYSTEM_STATION, SYSTEM_ID).await;
      let created = create(&db, "Region", None, Some(REGION_ID), &[(34, 10)]).await.unwrap();
      seed_asset(&db, 1, 1001, 34, SYSTEM_STATION, 5).await;

      let fill = super::super::fill_status(&db, created.stockpile.id(), &[])
        .await
        .unwrap()
        .unwrap();

      assert_eq!(fill.items[0].have_quantity, 5);
    }

    #[tokio::test]
    async fn a_region_pile_excludes_an_asset_in_a_different_region() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1001).await;
      seed_station_in(&db, SYSTEM_STATION, SYSTEM_ID).await;
      seed_station_in_region(&db, OTHER_STATION, 10_000_003, 20_000_021, OTHER_SYSTEM_ID).await;
      let created = create(&db, "Region", None, Some(REGION_ID), &[(34, 10)]).await.unwrap();
      seed_asset(&db, 1, 1001, 34, SYSTEM_STATION, 5).await;
      seed_asset(&db, 2, 1001, 34, OTHER_STATION, 8).await;

      let fill = super::super::fill_status(&db, created.stockpile.id(), &[])
        .await
        .unwrap()
        .unwrap();

      assert_eq!(fill.items[0].have_quantity, 5);
    }

    #[tokio::test]
    async fn a_station_pile_counts_only_that_station_not_the_whole_system() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1001).await;
      seed_station_in(&db, SYSTEM_STATION, SYSTEM_ID).await;
      seed_station_in(&db, SYSTEM_STATION_2, SYSTEM_ID).await;
      let created = create(&db, "Dock", None, Some(SYSTEM_STATION), &[(34, 10)])
        .await
        .unwrap();
      seed_asset(&db, 1, 1001, 34, SYSTEM_STATION, 3).await;
      seed_asset(&db, 2, 1001, 34, SYSTEM_STATION_2, 8).await;

      let fill = super::super::fill_status(&db, created.stockpile.id(), &[])
        .await
        .unwrap()
        .unwrap();

      assert_eq!(fill.items[0].have_quantity, 3);
    }

    #[tokio::test]
    async fn a_system_pile_counts_an_asset_at_a_station_in_that_system() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1001).await;
      seed_station_in(&db, SYSTEM_STATION, SYSTEM_ID).await;
      let created = create(&db, "Thera", None, Some(SYSTEM_ID), &[(34, 10)]).await.unwrap();
      seed_asset(&db, 1, 1001, 34, SYSTEM_STATION, 7).await;

      let fill = super::super::fill_status(&db, created.stockpile.id(), &[])
        .await
        .unwrap()
        .unwrap();

      assert_eq!(fill.items[0].have_quantity, 7);
    }

    #[tokio::test]
    async fn a_system_pile_counts_an_asset_at_a_structure_in_that_system() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1001).await;
      seed_structure_in(&db, SYSTEM_STRUCTURE, SYSTEM_ID).await;
      let created = create(&db, "Thera", None, Some(SYSTEM_ID), &[(34, 10)]).await.unwrap();
      seed_asset(&db, 1, 1001, 34, SYSTEM_STRUCTURE, 4).await;

      let fill = super::super::fill_status(&db, created.stockpile.id(), &[])
        .await
        .unwrap()
        .unwrap();

      assert_eq!(fill.items[0].have_quantity, 4);
    }

    #[tokio::test]
    async fn a_system_pile_counts_an_asset_nested_in_a_container() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1001).await;
      seed_station_in(&db, SYSTEM_STATION, SYSTEM_ID).await;
      let created = create(&db, "Thera", None, Some(SYSTEM_ID), &[(34, 10)]).await.unwrap();
      seed_asset(&db, 1, 1001, 99, SYSTEM_STATION, 1).await;
      seed_contained_asset(&db, 2, 1001, 34, 1, 6).await;

      let fill = super::super::fill_status(&db, created.stockpile.id(), &[])
        .await
        .unwrap()
        .unwrap();

      assert_eq!(fill.items[0].have_quantity, 6);
    }

    #[tokio::test]
    async fn a_system_pile_excludes_an_asset_in_a_different_system() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1001).await;
      seed_station_in(&db, SYSTEM_STATION, SYSTEM_ID).await;
      seed_station_in(&db, OTHER_STATION, OTHER_SYSTEM_ID).await;
      let created = create(&db, "Thera", None, Some(SYSTEM_ID), &[(34, 10)]).await.unwrap();
      seed_asset(&db, 1, 1001, 34, OTHER_STATION, 5).await;

      let fill = super::super::fill_status(&db, created.stockpile.id(), &[])
        .await
        .unwrap()
        .unwrap();

      assert_eq!(fill.items[0].have_quantity, 0);
    }

    #[tokio::test]
    async fn an_item_less_stockpile_is_fully_met_and_empty() {
      let db = store::open_test().await.unwrap();
      let created = create(&db, "Empty", None, None, &[]).await.unwrap();

      let fill = super::super::fill_status(&db, created.stockpile.id(), &[])
        .await
        .unwrap()
        .unwrap();

      assert!(fill.items.is_empty());
      assert_eq!(fill.overall_pct(), 1.0);
      assert!(fill.is_full());
    }

    #[tokio::test]
    async fn character_scope_narrows_the_on_hand_sum() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1001).await;
      seed_character(&db, 1002).await;
      let created = create(&db, "Char Scoped", None, None, &[(34, 1000)]).await.unwrap();
      seed_asset(&db, 1, 1001, 34, STATION_A, 400).await;
      seed_asset(&db, 2, 1002, 34, STATION_A, 999).await;

      let fill = super::super::fill_status(&db, created.stockpile.id(), &[1001])
        .await
        .unwrap()
        .unwrap();

      assert_eq!(fill.items[0].have_quantity, 400);
    }

    #[tokio::test]
    async fn it_is_none_for_a_missing_stockpile() {
      let db = store::open_test().await.unwrap();

      assert!(super::super::fill_status(&db, 999_999, &[]).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn it_reports_under_met_and_over_target_items() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1001).await;
      let created = create(&db, "Cache", None, None, &[(34, 1000), (35, 500), (36, 200)])
        .await
        .unwrap();
      seed_asset(&db, 1, 1001, 34, STATION_A, 400).await;
      seed_asset(&db, 2, 1001, 35, STATION_A, 500).await;
      seed_asset(&db, 3, 1001, 36, STATION_A, 350).await;

      let fill = super::super::fill_status(&db, created.stockpile.id(), &[])
        .await
        .unwrap()
        .unwrap();

      assert_eq!(fill.stockpile_id, created.stockpile.id());
      assert_eq!(fill.items.len(), 3);

      let under = fill.items[0];
      assert_eq!(under.type_id, 34);
      assert_eq!(under.have_quantity, 400);
      assert_eq!(under.target_quantity, 1000);
      assert_eq!(under.pct(), 0.4);

      let met = fill.items[1];
      assert_eq!(met.have_quantity, 500);
      assert_eq!(met.pct(), 1.0);

      let over = fill.items[2];
      assert_eq!(over.have_quantity, 350);
      assert_eq!(over.target_quantity, 200);
      assert_eq!(over.pct(), 1.0);

      assert!(!fill.is_full());
    }

    #[tokio::test]
    async fn it_reports_zero_on_hand_when_no_assets_match() {
      let db = store::open_test().await.unwrap();
      let created = create(&db, "Cache", None, None, &[(34, 100)]).await.unwrap();

      let fill = super::super::fill_status(&db, created.stockpile.id(), &[])
        .await
        .unwrap()
        .unwrap();

      assert_eq!(fill.items[0].have_quantity, 0);
      assert_eq!(fill.items[0].pct(), 0.0);
    }

    #[tokio::test]
    async fn it_sums_multiple_asset_rows_of_the_same_type() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1001).await;
      let created = create(&db, "Cache", None, None, &[(34, 1000)]).await.unwrap();
      seed_asset(&db, 1, 1001, 34, STATION_A, 300).await;
      seed_asset(&db, 2, 1001, 34, STATION_B, 250).await;

      let fill = super::super::fill_status(&db, created.stockpile.id(), &[])
        .await
        .unwrap()
        .unwrap();

      assert_eq!(fill.items[0].have_quantity, 550);
    }

    #[tokio::test]
    async fn location_scope_narrows_the_on_hand_sum() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1001).await;
      let created = create(&db, "Loc Scoped", None, Some(STATION_A), &[(34, 1000)])
        .await
        .unwrap();
      seed_asset(&db, 1, 1001, 34, STATION_A, 400).await;
      seed_asset(&db, 2, 1001, 34, STATION_B, 999).await;

      let fill = super::super::fill_status(&db, created.stockpile.id(), &[])
        .await
        .unwrap()
        .unwrap();

      assert_eq!(fill.items[0].have_quantity, 400);
    }

    #[tokio::test]
    async fn overall_pct_caps_each_item_at_its_target() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1001).await;
      let created = create(&db, "Cache", None, None, &[(34, 1000), (35, 1000)])
        .await
        .unwrap();
      seed_asset(&db, 1, 1001, 34, STATION_A, 200).await;
      seed_asset(&db, 2, 1001, 35, STATION_A, 5000).await;

      let fill = super::super::fill_status(&db, created.stockpile.id(), &[])
        .await
        .unwrap()
        .unwrap();

      assert_eq!(fill.overall_pct(), 0.6);
      assert!(!fill.is_full());
    }

    #[tokio::test]
    async fn unscoped_stockpile_sums_across_characters_and_locations() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1001).await;
      seed_character(&db, 1002).await;
      let created = create(&db, "Unscoped", None, None, &[(34, 1000)]).await.unwrap();
      seed_asset(&db, 1, 1001, 34, STATION_A, 400).await;
      seed_asset(&db, 2, 1002, 34, STATION_B, 350).await;

      let fill = super::super::fill_status(&db, created.stockpile.id(), &[])
        .await
        .unwrap()
        .unwrap();

      assert_eq!(fill.items[0].have_quantity, 750);
    }
  }

  mod list_with_items {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_is_empty_when_there_are_no_stockpiles() {
      let db = store::open_test().await.unwrap();

      assert!(list_with_items(&db).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_returns_every_stockpile_with_its_items() {
      let db = store::open_test().await.unwrap();
      let a = create(&db, "First", None, None, &[(34, 1)]).await.unwrap();
      let b = create(&db, "Second", None, None, &[(35, 2), (36, 3)]).await.unwrap();

      let all = list_with_items(&db).await.unwrap();

      assert_eq!(
        all.iter().map(|s| s.stockpile.id()).collect::<Vec<_>>(),
        [a.stockpile.id(), b.stockpile.id()]
      );
      assert_eq!(all[0].items.len(), 1);
      assert_eq!(all[1].items.len(), 2);
    }
  }

  mod location_name {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{
      model::{Constellation, Region, SolarSystem, Structure},
      repo::sde,
    };

    async fn seed_structure(db: &Database, id: i64, name: &str) {
      seed_character(db, 1001).await;
      sde::upsert_region(
        db,
        &Region {
          description: None,
          id: 10_000_002,
          name: "The Forge".to_owned(),
        },
      )
      .await
      .unwrap();
      sde::upsert_constellation(
        db,
        &Constellation {
          id: 20_000_020,
          name: "Kimotoro".to_owned(),
          position_x: 0.0,
          position_y: 0.0,
          position_z: 0.0,
          region_id: 10_000_002,
        },
      )
      .await
      .unwrap();
      sde::upsert_solar_system(
        db,
        &SolarSystem {
          constellation_id: 20_000_020,
          id: 30_000_142,
          name: "Jita".to_owned(),
          position_x: 0.0,
          position_y: 0.0,
          position_z: 0.0,
          security_class: None,
          security_status: 0.9,
          star_id: None,
        },
      )
      .await
      .unwrap();
      sde::upsert_structure(
        db,
        &Structure {
          id,
          name: name.to_owned(),
          owner_id: 90_000_001,
          position_x: None,
          position_y: None,
          position_z: None,
          solar_system_id: 30_000_142,
          type_id: None,
        },
      )
      .await
      .unwrap();
    }

    #[tokio::test]
    async fn it_resolves_a_constellation_name() {
      let db = store::open_test().await.unwrap();
      seed_structure(&db, 1_030_000_000_001, "Jita Trade Hub").await;

      let name = location_name(&db, 20_000_020).await.unwrap();

      assert_eq!(name.as_deref(), Some("Kimotoro"));
    }

    #[tokio::test]
    async fn it_resolves_a_region_name() {
      let db = store::open_test().await.unwrap();
      seed_structure(&db, 1_030_000_000_001, "Jita Trade Hub").await;

      let name = location_name(&db, 10_000_002).await.unwrap();

      assert_eq!(name.as_deref(), Some("The Forge"));
    }

    #[tokio::test]
    async fn it_resolves_a_solar_system_name() {
      let db = store::open_test().await.unwrap();
      seed_structure(&db, 1_030_000_000_001, "Jita Trade Hub").await;

      let name = location_name(&db, 30_000_142).await.unwrap();

      assert_eq!(name.as_deref(), Some("Jita"));
    }

    #[tokio::test]
    async fn it_resolves_a_structure_name() {
      let db = store::open_test().await.unwrap();
      seed_structure(&db, 1_030_000_000_001, "Jita Trade Hub").await;

      let name = location_name(&db, 1_030_000_000_001).await.unwrap();

      assert_eq!(name.as_deref(), Some("Jita Trade Hub"));
    }

    #[tokio::test]
    async fn it_returns_none_for_an_unknown_location() {
      let db = store::open_test().await.unwrap();

      assert_eq!(location_name(&db, 60_003_760).await.unwrap(), None);
    }
  }

  mod update {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_clears_items_when_given_none() {
      let db = store::open_test().await.unwrap();
      let created = create(&db, "Has Items", None, None, &[(34, 1), (35, 2)]).await.unwrap();

      update(&db, created.stockpile.id(), "Has Items", None, None, &[])
        .await
        .unwrap();

      assert!(items(&db, created.stockpile.id()).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_replaces_fields_and_items() {
      let db = store::open_test().await.unwrap();
      let created = create(&db, "Old", None, None, &[(34, 100)]).await.unwrap();

      let updated = update(
        &db,
        created.stockpile.id(),
        "New",
        Some("corp:cobalt".to_string()),
        Some(60_003_760),
        &[(35, 200)],
      )
      .await
      .unwrap();

      assert_eq!(updated.stockpile.name(), "New");
      assert_eq!(updated.stockpile.character_scope(), &Some("corp:cobalt".to_string()));
      assert_eq!(updated.stockpile.location_id(), Some(60_003_760));
      assert_eq!(updated.items.len(), 1);
      assert_eq!(updated.items[0].type_id(), 35);
      assert_eq!(items(&db, created.stockpile.id()).await.unwrap().len(), 1);
    }
  }

  mod with_items {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_is_none_for_a_missing_stockpile() {
      let db = store::open_test().await.unwrap();

      assert!(with_items(&db, 999_999).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn it_returns_a_stockpile_and_its_items() {
      let db = store::open_test().await.unwrap();
      let created = create(&db, "Cache", None, None, &[(34, 10)]).await.unwrap();

      let loaded = with_items(&db, created.stockpile.id()).await.unwrap().unwrap();

      assert_eq!(loaded, created);
    }
  }
}

#[cfg(test)]
mod saved_filter_tests {
  use super::*;
  use crate::store;

  mod create_saved_filter {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_inserts_a_filter_with_a_category() {
      let db = store::open_test().await.unwrap();

      let created = create_saved_filter(&db, "Ships", "category:ship", Some("ship"))
        .await
        .unwrap();

      assert!(created.id() > 0);
      assert_eq!(created.name(), "Ships");
      assert_eq!(created.query(), "category:ship");
      assert_eq!(created.category().as_deref(), Some("ship"));
    }

    #[tokio::test]
    async fn it_inserts_a_filter_with_a_null_category() {
      let db = store::open_test().await.unwrap();

      let created = create_saved_filter(&db, "Everything", "tritanium", None).await.unwrap();

      assert_eq!(created.category(), &None);
      assert_eq!(created.query(), "tritanium");
    }
  }

  mod delete_saved_filter {
    use super::*;

    #[tokio::test]
    async fn it_is_a_no_op_for_a_missing_filter() {
      let db = store::open_test().await.unwrap();

      delete_saved_filter(&db, 999_999).await.unwrap();
    }

    #[tokio::test]
    async fn it_removes_the_filter() {
      let db = store::open_test().await.unwrap();
      let created = create_saved_filter(&db, "Doomed", "", None).await.unwrap();

      delete_saved_filter(&db, created.id()).await.unwrap();

      assert!(saved_filters(&db).await.unwrap().is_empty());
    }
  }

  mod saved_filters {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_is_empty_when_there_are_no_filters() {
      let db = store::open_test().await.unwrap();

      assert!(saved_filters(&db).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_returns_every_filter_ordered_by_id() {
      let db = store::open_test().await.unwrap();
      let first = create_saved_filter(&db, "All", "", None).await.unwrap();
      let second = create_saved_filter(&db, "Modules", "category:module", Some("module"))
        .await
        .unwrap();

      let all = saved_filters(&db).await.unwrap();

      assert_eq!(
        all.iter().map(|f| f.id()).collect::<Vec<_>>(),
        [first.id(), second.id()]
      );
      assert_eq!(all[0].category(), &None);
      assert_eq!(all[1].category().as_deref(), Some("module"));
    }
  }
}
