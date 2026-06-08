use chrono::{Duration, Utc};

use crate::{
  clients::esi,
  store::{Database, model::TypePriceHistory, repo::finance},
};

pub const JITA_STATION_ID: i64 = 60_003_760;
pub const THE_FORGE_REGION_ID: i64 = 10_000_002;
pub const BATCH_SIZE: usize = 10;

fn batches(type_ids: &[i64]) -> Vec<Vec<i64>> {
  type_ids.chunks(BATCH_SIZE).map(<[i64]>::to_vec).collect()
}

fn retention_cutoff(today: chrono::NaiveDate) -> String {
  (today - Duration::days(finance::RETENTION_DAYS))
    .format("%Y-%m-%d")
    .to_string()
}

fn point(type_id: i64, date: &str, price: f64) -> TypePriceHistory {
  TypePriceHistory {
    close: price,
    date: date.to_owned(),
    high: price,
    low: price,
    open: price,
    type_id,
  }
}

pub async fn refresh(db: &Database, esi: &esi::Client, type_ids: &[i64]) -> usize {
  let today = Utc::now().date_naive();
  let date = today.format("%Y-%m-%d").to_string();
  let mut persisted = 0;

  for batch in batches(type_ids) {
    match fetch_batch(esi, &batch).await {
      Ok(points) => {
        let rows: Vec<TypePriceHistory> = points
          .into_iter()
          .map(|(type_id, price)| point(type_id, &date, price))
          .collect();
        if rows.is_empty() {
          continue;
        }
        match finance::price_history_upsert_many(db, &rows).await {
          Ok(()) => persisted += rows.len(),
          Err(error) => tracing::warn!(?batch, "prices: persist failed; continuing: {error}"),
        }
      }
      Err(error) => tracing::warn!(?batch, "prices: batch fetch failed; continuing: {error}"),
    }
  }

  let cutoff = retention_cutoff(today);
  if let Err(error) = finance::prune_before(db, &cutoff).await {
    tracing::warn!(%cutoff, "prices: prune failed; continuing: {error}");
  }

  persisted
}

async fn fetch_batch(esi: &esi::Client, type_ids: &[i64]) -> Result<Vec<(i64, f64)>, crate::clients::Error> {
  let mut out = Vec::with_capacity(type_ids.len());
  for &type_id in type_ids {
    if let Some(price) = esi
      .market()
      .lowest_sell(THE_FORGE_REGION_ID, type_id, JITA_STATION_ID)
      .await?
    {
      out.push((type_id, price));
    }
  }
  Ok(out)
}

#[cfg(test)]
mod tests {
  use super::*;

  mod batches {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_returns_no_batches_for_an_empty_input() {
      assert!(batches(&[]).is_empty());
    }

    #[test]
    fn it_keeps_a_short_input_in_a_single_batch() {
      let ids: Vec<i64> = (0..7).collect();

      let batched = batches(&ids);

      assert_eq!(batched.len(), 1);
      assert_eq!(batched[0].len(), 7);
    }

    #[test]
    fn it_chunks_into_full_batches_plus_a_remainder() {
      let ids: Vec<i64> = (0..25).collect();

      let batched = batches(&ids);

      assert_eq!(batched.iter().map(Vec::len).collect::<Vec<_>>(), [10, 10, 5]);
      assert_eq!(batched.concat(), ids, "every id is covered exactly once, in order");
    }
  }

  mod point {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_seeds_a_flat_ohlc_bucket_from_a_single_price() {
      let row = point(34, "2026-06-05", 5.5);

      assert_eq!(row.type_id(), 34);
      assert_eq!(row.date(), "2026-06-05");
      assert_eq!((row.open(), row.high(), row.low(), row.close()), (5.5, 5.5, 5.5, 5.5));
    }
  }

  mod retention_cutoff {
    use chrono::NaiveDate;
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_is_the_retention_window_before_today() {
      let today = NaiveDate::from_ymd_opt(2026, 6, 5).unwrap();

      assert_eq!(retention_cutoff(today), "2025-06-05");
    }
  }

  mod refresh {
    use pretty_assertions::assert_eq;
    use wiremock::{
      Mock, MockServer, ResponseTemplate,
      matchers::{method, path, query_param},
    };

    use super::*;
    use crate::{clients::http, store};

    async fn make_esi(base_url: &str, db: &store::Database) -> esi::Client {
      let cache = http::Cache::new(db.clone());
      let http = http::Client::builder(cache).build();
      esi::Client::with_base_url(http, base_url)
    }

    async fn mount_type(server: &MockServer, type_id: i64, price: f64) {
      let body =
        format!(r#"[{{"is_buy_order":false,"location_id":{JITA_STATION_ID},"price":{price},"type_id":{type_id}}}]"#);
      Mock::given(method("GET"))
        .and(path(format!("/markets/{THE_FORGE_REGION_ID}/orders/")))
        .and(query_param("type_id", type_id.to_string()))
        .respond_with(
          ResponseTemplate::new(200)
            .insert_header("X-Pages", "1")
            .set_body_raw(body, "application/json"),
        )
        .mount(server)
        .await;
    }

    #[tokio::test]
    async fn it_fetches_and_persists_the_lowest_sell_price_per_type() {
      let server = MockServer::start().await;
      mount_type(&server, 34, 5.5).await;
      mount_type(&server, 35, 11.0).await;
      let db = store::open_test().await.unwrap();
      let esi = make_esi(&server.uri(), &db).await;

      let persisted = refresh(&db, &esi, &[34, 35]).await;

      assert_eq!(persisted, 2);
      let date = Utc::now().format("%Y-%m-%d").to_string();
      assert_eq!(finance::close_as_of(&db, 34, &date).await.unwrap(), Some(5.5));
      assert_eq!(finance::close_as_of(&db, 35, &date).await.unwrap(), Some(11.0));
    }

    #[tokio::test]
    async fn it_rolls_repeated_same_day_runs_into_a_correct_ohlc() {
      let db = store::open_test().await.unwrap();
      let date = Utc::now().format("%Y-%m-%d").to_string();

      for price in [5.0_f64, 8.0, 6.0] {
        let server = MockServer::start().await;
        mount_type(&server, 34, price).await;
        let esi = make_esi(&server.uri(), &db).await;
        assert_eq!(refresh(&db, &esi, &[34]).await, 1);
      }

      let row = &finance::series(&db, 34).await.unwrap()[0];
      assert_eq!(row.date(), &date, "all samples land in a single daily bucket");
      assert_eq!(row.open(), 5.0, "open holds the first sample of the day");
      assert_eq!(row.high(), 8.0, "high is the running max");
      assert_eq!(row.low(), 5.0, "low is the running min");
      assert_eq!(row.close(), 6.0, "close is the latest sample");
    }

    #[tokio::test]
    async fn it_prunes_daily_rows_older_than_the_retention_window() {
      let db = store::open_test().await.unwrap();
      finance::price_history_upsert_many(
        &db,
        &[TypePriceHistory {
          close: 1.0,
          date: "2020-01-01".to_owned(),
          high: 1.0,
          low: 1.0,
          open: 1.0,
          type_id: 34,
        }],
      )
      .await
      .unwrap();
      let server = MockServer::start().await;
      mount_type(&server, 34, 5.5).await;
      let esi = make_esi(&server.uri(), &db).await;

      refresh(&db, &esi, &[34]).await;

      let dates: Vec<String> = finance::series(&db, 34)
        .await
        .unwrap()
        .iter()
        .map(|row| row.date().clone())
        .collect();
      assert!(!dates.contains(&"2020-01-01".to_owned()), "the stale row is pruned");
      assert!(
        dates.contains(&Utc::now().format("%Y-%m-%d").to_string()),
        "today's freshly persisted row is kept"
      );
    }

    #[tokio::test]
    async fn it_skips_types_with_no_jita_sell_order() {
      let server = MockServer::start().await;
      mount_type(&server, 34, 5.5).await;
      Mock::given(method("GET"))
        .and(path(format!("/markets/{THE_FORGE_REGION_ID}/orders/")))
        .and(query_param("type_id", "35"))
        .respond_with(
          ResponseTemplate::new(200)
            .insert_header("X-Pages", "1")
            .set_body_raw("[]", "application/json"),
        )
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      let esi = make_esi(&server.uri(), &db).await;

      let persisted = refresh(&db, &esi, &[34, 35]).await;

      assert_eq!(persisted, 1);
      let date = Utc::now().format("%Y-%m-%d").to_string();
      assert_eq!(finance::close_as_of(&db, 35, &date).await.unwrap(), None);
    }

    #[tokio::test]
    async fn it_isolates_a_failing_batch_and_persists_the_rest() {
      let mut type_ids: Vec<i64> = (1..=BATCH_SIZE as i64).collect();
      type_ids.push(1000);
      let server = MockServer::start().await;
      for &type_id in &type_ids[..BATCH_SIZE] {
        Mock::given(method("GET"))
          .and(path(format!("/markets/{THE_FORGE_REGION_ID}/orders/")))
          .and(query_param("type_id", type_id.to_string()))
          .respond_with(ResponseTemplate::new(500))
          .mount(&server)
          .await;
      }
      mount_type(&server, 1000, 42.0).await;
      let db = store::open_test().await.unwrap();
      let esi = make_esi(&server.uri(), &db).await;

      let persisted = refresh(&db, &esi, &type_ids).await;

      assert_eq!(
        persisted, 1,
        "the surviving batch still persists despite the failed batch"
      );
      let date = Utc::now().format("%Y-%m-%d").to_string();
      assert_eq!(finance::close_as_of(&db, 1000, &date).await.unwrap(), Some(42.0));
    }
  }
}
