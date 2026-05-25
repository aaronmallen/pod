//! Price sync service: aggregates OHLC history and fetches fresh Jita prices.

use std::collections::HashMap;

use chrono::Utc;

/// Aggregates any pending OHLC dates and fetches current Jita prices for all
/// tracked types. Safe to call at startup or on a recurring timer.
pub async fn sync(db: &pod_db::Repo, esi: &pod_esi::Client) {
  tracing::debug!("prices: sync started");
  let today = Utc::now().date_naive();
  aggregate_pending_dates(db, today).await;

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

  let adjusted_prices = fetch_adjusted_prices(esi).await;
  spawn_price_fetches(db, esi, type_ids, adjusted_prices).await;
}

async fn aggregate_pending_dates(db: &pod_db::Repo, today: chrono::NaiveDate) {
  let Ok(dates) = db.prices().dates_needing_aggregation(today).await else {
    return;
  };
  for date in dates {
    if let Err(e) = db.prices().aggregate_and_prune(date).await {
      tracing::warn!("prices: failed to aggregate OHLC for {date}: {e}");
    }
  }
}

async fn fetch_adjusted_prices(esi: &pod_esi::Client) -> HashMap<i32, f64> {
  match esi.market().prices().await {
    Ok(rows) => rows
      .into_iter()
      .filter_map(|r| r.adjusted_price.map(|p| (r.type_id, p)))
      .collect(),
    Err(e) => {
      tracing::warn!("prices: failed to fetch ESI market prices: {e}");
      HashMap::new()
    }
  }
}

async fn spawn_price_fetches(
  db: &pod_db::Repo,
  esi: &pod_esi::Client,
  type_ids: Vec<i32>,
  adjusted_prices: HashMap<i32, f64>,
) {
  let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(10));
  let mut handles = Vec::with_capacity(type_ids.len());

  for type_id in type_ids {
    let permit = semaphore.clone().acquire_owned().await.expect("semaphore closed");
    let esi = esi.clone();
    let db = db.clone();
    let adjusted_price = adjusted_prices.get(&type_id).copied();
    handles.push(tokio::spawn(async move {
      let _permit = permit;
      fetch_and_insert_price(type_id, adjusted_price, &esi, &db).await;
    }));
  }

  for handle in handles {
    let _ = handle.await;
  }
}

async fn fetch_and_insert_price(type_id: i32, adjusted_price: Option<f64>, esi: &pod_esi::Client, db: &pod_db::Repo) {
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
}
