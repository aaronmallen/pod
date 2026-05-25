//! Abyssal items sync service.
//!
//! Detects owned abyssal modules from character assets, fetches their rolled
//! stats from ESI, stores them in the DB, and prices them via MutaMarket.

use std::time::{SystemTime, UNIX_EPOCH};

use pod_model::{AbyssalAttribute, AbyssalItemRecord};

use crate::services::muta_market;

fn now_unix() -> i64 {
  SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map(|d| d.as_secs() as i64)
    .unwrap_or(0)
}

/// Syncs abyssal items for the given character.
///
/// 1. Queries character assets for singleton items.
/// 2. Batch-looks up type IDs to find those with `is_abyssal = true`.
/// 3. For each abyssal item not synced within 12 hours, fetches dogma data
///    from ESI and upserts the record.
/// 4. Prunes items no longer present in character assets.
/// 5. Refreshes MutaMarket prices for items older than 24 hours.
pub async fn sync_abyssals(character_id: i64, esi: &pod_esi::Client, muta: &muta_market::Client, db: &pod_db::Repo) {
  let assets = match db.characters().assets_for_character_ids(&[character_id]).await {
    Ok(rows) => rows,
    Err(e) => {
      tracing::warn!("abyssals: failed to load assets for character {character_id}: {e}");
      return;
    }
  };

  let singleton_pairs: Vec<(i32, i64)> = assets
    .iter()
    .filter(|a| a.is_singleton)
    .map(|a| (a.type_id, a.item_id))
    .collect();

  if singleton_pairs.is_empty() {
    return;
  }

  let type_ids: Vec<i32> = singleton_pairs.iter().map(|(tid, _)| *tid).collect();
  let type_ids_deduped: Vec<i32> = {
    let mut ids = type_ids.clone();
    ids.sort_unstable();
    ids.dedup();
    ids
  };

  let item_type_rows = match db.universe().item_types().find_by_ids(&type_ids_deduped).await {
    Ok(rows) => rows,
    Err(e) => {
      tracing::warn!("abyssals: failed to look up item types: {e}");
      return;
    }
  };

  let abyssal_type_ids: std::collections::HashSet<i32> =
    item_type_rows.iter().filter(|t| t.is_abyssal).map(|t| t.id).collect();

  let abyssal_pairs: Vec<(i32, i64)> = singleton_pairs
    .into_iter()
    .filter(|(tid, _)| abyssal_type_ids.contains(tid))
    .collect();

  if abyssal_pairs.is_empty() {
    prune_stale(character_id, &[], db).await;
    return;
  }

  let current_item_ids: Vec<i64> = abyssal_pairs.iter().map(|(_, iid)| *iid).collect();

  let existing = match db.abyssals().abyssals_for_character(character_id).await {
    Ok(rows) => rows,
    Err(e) => {
      tracing::warn!("abyssals: failed to load existing records for {character_id}: {e}");
      return;
    }
  };

  let now = now_unix();
  let stale_threshold = now - 12 * 3600;

  let existing_synced: std::collections::HashMap<i64, i64> =
    existing.iter().map(|r| (*r.item_id(), *r.synced_at())).collect();

  for (type_id, item_id) in &abyssal_pairs {
    let last_synced = existing_synced.get(item_id).copied().unwrap_or(0);
    if last_synced >= stale_threshold {
      continue;
    }

    match esi.dogma().dynamic_item(*type_id as i64, *item_id).await {
      Ok(dynamic) => {
        let attrs: Vec<AbyssalAttribute> = dynamic
          .dogma_attributes
          .iter()
          .map(|v| AbyssalAttribute::new(v.attribute_id, v.value))
          .collect();

        let record = AbyssalItemRecord::new(
          *item_id,
          character_id,
          *type_id,
          dynamic.source_type_id,
          dynamic.mutator_type_id,
          attrs,
          now,
        );

        if let Err(e) = db.abyssals().upsert_abyssal(record).await {
          tracing::warn!("abyssals: failed to upsert item {item_id}: {e}");
        }
      }
      Err(e) => {
        tracing::warn!("abyssals: ESI fetch failed for item {item_id} type {type_id}: {e}");
      }
    }
  }

  prune_stale(character_id, &current_item_ids, db).await;
  price_abyssals(character_id, muta, db).await;
}

async fn prune_stale(character_id: i64, keep_ids: &[i64], db: &pod_db::Repo) {
  if let Err(e) = db.abyssals().delete_stale_abyssals(character_id, keep_ids).await {
    tracing::warn!("abyssals: failed to prune stale items for {character_id}: {e}");
  }
}

async fn price_abyssals(character_id: i64, muta: &muta_market::Client, db: &pod_db::Repo) {
  let items = match db.abyssals().abyssals_for_character(character_id).await {
    Ok(rows) => rows,
    Err(e) => {
      tracing::warn!("abyssals: failed to load items for pricing {character_id}: {e}");
      return;
    }
  };

  let now = now_unix();
  let price_stale_threshold = now - 24 * 3600;

  for item in items {
    let last_priced = item.muta_price_synced().unwrap_or(0);
    if last_priced >= price_stale_threshold {
      continue;
    }

    match muta.item_price(*item.type_id(), *item.item_id()).await {
      Ok(Some(price)) => {
        if let Err(e) = db
          .abyssals()
          .update_abyssal_price(*item.item_id(), Some(price), now)
          .await
        {
          tracing::warn!("abyssals: failed to save price for item {}: {e}", item.item_id());
        }
      }
      Ok(None) => {
        let _ = db.abyssals().update_abyssal_price(*item.item_id(), None, now).await;
      }
      Err(e) => {
        tracing::warn!(
          "abyssals: MutaMarket price fetch failed for item {}: {e}",
          item.item_id()
        );
      }
    }
  }
}
