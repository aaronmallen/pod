//! Price sync service: aggregates OHLC history and fetches fresh Jita prices.

use std::collections::HashMap;

use chrono::Utc;

/// Aggregates any pending OHLC dates and fetches current Jita prices for all
/// tracked types. Safe to call at startup or on a recurring timer.
pub async fn sync(db: &pod_db::Repo, esi: &pod_esi::Client) {
  let today = Utc::now().date_naive();

  if let Ok(dates) = db.prices().dates_needing_aggregation(today).await {
    for date in dates {
      let _ = db.prices().aggregate_and_prune(date).await;
    }
  }

  let type_ids = match db.prices().types_to_track().await {
    Ok(ids) => ids,
    Err(e) => {
      tracing::warn!("prices: failed to get types to track: {e}");
      return;
    }
  };

  if type_ids.is_empty() {
    return;
  }

  let adjusted_prices: HashMap<i32, f64> = match esi.market().prices().await {
    Ok(rows) => rows
      .into_iter()
      .filter_map(|r| r.adjusted_price.map(|p| (r.type_id, p)))
      .collect(),
    Err(e) => {
      tracing::warn!("prices: failed to fetch ESI market prices: {e}");
      HashMap::new()
    }
  };

  let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(10));
  let mut handles = Vec::with_capacity(type_ids.len());

  for type_id in type_ids {
    let permit = semaphore.clone().acquire_owned().await.expect("semaphore closed");
    let esi = esi.clone();
    let db = db.clone();
    let adjusted_price = adjusted_prices.get(&type_id).copied();
    handles.push(tokio::spawn(async move {
      let _permit = permit;
      let now = Utc::now();
      match esi.markets().lowest_jita_sell(type_id).await {
        Ok(Some(price)) => {
          let _ = db.prices().insert_price(type_id, price, adjusted_price, now).await;
        }
        Ok(None) => {}
        Err(e) => {
          tracing::warn!("prices: price fetch failed for type {type_id}: {e}");
        }
      }
    }));
  }

  for handle in handles {
    let _ = handle.await;
  }
}
