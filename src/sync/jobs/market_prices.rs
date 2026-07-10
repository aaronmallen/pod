use crate::{
  clients::{Error, zkillboard},
  store::{model::MarketPrice, repo::finance},
  sync::{job::JobCtx, outcome::Outcome},
};

pub async fn run(ctx: &JobCtx<'_>) -> Result<Outcome, Error> {
  run_with_zkill(ctx, &zkillboard::Client::new(ctx.esi.http())).await
}

async fn run_with_zkill(ctx: &JobCtx<'_>, zkill: &zkillboard::Client) -> Result<Outcome, Error> {
  let prices: Vec<MarketPrice> = ctx
    .esi
    .market()
    .prices()
    .await?
    .into_iter()
    .map(|price| MarketPrice::esi(price.type_id, price.adjusted_price, price.average_price))
    .collect();
  finance::market_prices_upsert_many(ctx.db, &prices).await?;

  let swept = sweep_from_zkill(ctx, zkill).await;
  Ok(Outcome::from_rows(prices.len() + swept))
}

async fn sweep_from_zkill(ctx: &JobCtx<'_>, zkill: &zkillboard::Client) -> usize {
  let type_ids = match finance::market_prices_zkill_sweep_type_ids(ctx.db).await {
    Ok(type_ids) => type_ids,
    Err(error) => {
      tracing::warn!("market_prices: failed to select zKill sweep set: {error}");
      return 0;
    }
  };

  let mut priced = Vec::new();
  for type_id in type_ids {
    match zkill.prices(type_id).await {
      Ok(Some(price)) => priced.push(MarketPrice::zkill(type_id, price)),
      Ok(None) => {}
      Err(error) => {
        tracing::warn!(type_id, "market_prices: zKill price fetch failed: {error}")
      }
    }
  }

  if priced.is_empty() {
    tracing::info!(zkill_types = 0, "market_prices: swept held types through zKill");
    return 0;
  }
  let count = priced.len();
  if let Err(error) = finance::market_prices_upsert_many(ctx.db, &priced).await {
    tracing::warn!("market_prices: failed to upsert zKill sweep prices: {error}");
    return 0;
  }
  tracing::info!(zkill_types = count, "market_prices: swept held types through zKill");
  count
}

#[cfg(test)]
mod tests {
  use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
  };

  use super::*;
  use crate::{
    clients::{esi, eve_image, http},
    store::{
      self, images,
      model::{Alliance, Bloodline, Character, Corporation, Gender, Race},
      repo::{character::insert_with_org, finance},
    },
    sync::{
      job::{JobKey, JobKind},
      subject::Subject,
    },
  };

  async fn insert_character_asset(db: &store::Database, item_id: i64, type_id: i64, is_blueprint_copy: i64) {
    sqlx::query(
      "INSERT INTO character_assets \
        (item_id, character_id, type_id, location_id, location_type, location_flag, quantity, is_blueprint_copy) \
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(item_id)
    .bind(42_i64)
    .bind(type_id)
    .bind(60_003_760_i64)
    .bind("station")
    .bind("Hangar")
    .bind(1_i64)
    .bind(is_blueprint_copy)
    .execute(db.writer())
    .await
    .unwrap();
  }

  async fn mount_prices(server: &MockServer, body: serde_json::Value) {
    Mock::given(method("GET"))
      .and(path("/markets/prices/"))
      .respond_with(ResponseTemplate::new(200).set_body_json(body))
      .mount(server)
      .await;
  }

  async fn mount_zkill_price(server: &MockServer, type_id: i64, body: &str) {
    Mock::given(method("GET"))
      .and(path(format!("/prices/{type_id}/")))
      .respond_with(ResponseTemplate::new(200).set_body_raw(body.to_string(), "application/json"))
      .mount(server)
      .await;
  }

  async fn seed_character(db: &store::Database, id: i64) {
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

  fn ctx<'a>(
    db: &'a store::Database,
    esi: &'a esi::Client,
    image: &'a eve_image::Client,
    image_store: &'a images::Store,
  ) -> JobCtx<'a> {
    JobCtx {
      db,
      esi,
      image,
      image_store,
      key: JobKey::new(JobKind::MarketPrices, Subject::Character(0)),
      grant: None,
      sso: None,
    }
  }

  mod run {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_errors_and_persists_nothing_when_the_fetch_fails() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/markets/prices/"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let ctx = ctx(&db, &esi, &image, &image_store);

      let result = run(&ctx).await;

      assert!(result.is_err());
      assert!(finance::market_prices_all(&db).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_overwrites_existing_rows_on_a_re_run() {
      let server = MockServer::start().await;
      mount_prices(
        &server,
        serde_json::json!([{ "adjusted_price": 9.0, "average_price": 10.0, "type_id": 34 }]),
      )
      .await;
      let db = store::open_test().await.unwrap();
      finance::market_prices_upsert_many(&db, &[MarketPrice::esi(34, Some(1.0), Some(2.0))])
        .await
        .unwrap();
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let ctx = ctx(&db, &esi, &image, &image_store);

      run(&ctx).await.unwrap();

      let rows = finance::market_prices_all(&db).await.unwrap();
      assert_eq!(rows.len(), 1, "the same type_id must overwrite, not append");
      assert_eq!(rows[0].adjusted_price(), Some(9.0));
      assert_eq!(rows[0].average_price(), Some(10.0));
    }

    #[tokio::test]
    async fn it_upserts_every_returned_row_without_a_grant() {
      let server = MockServer::start().await;
      mount_prices(
        &server,
        serde_json::json!([
          { "adjusted_price": 5.5, "type_id": 34 },
          { "average_price": 6.25, "type_id": 35 },
          { "adjusted_price": 7.0, "average_price": 8.0, "type_id": 36 },
        ]),
      )
      .await;
      let db = store::open_test().await.unwrap();
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let ctx = ctx(&db, &esi, &image, &image_store);

      run(&ctx).await.unwrap();

      let rows = finance::market_prices_all(&db).await.unwrap();
      assert_eq!(rows.iter().map(MarketPrice::type_id).collect::<Vec<_>>(), [34, 35, 36]);
      assert_eq!(rows[0].adjusted_price(), Some(5.5));
      assert_eq!(rows[0].average_price(), None);
      assert_eq!(rows[1].adjusted_price(), None);
      assert_eq!(rows[1].average_price(), Some(6.25));
      assert_eq!(rows[2].adjusted_price(), Some(7.0));
      assert_eq!(rows[2].average_price(), Some(8.0));
    }
  }

  mod sweep_from_zkill {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::clients::zkillboard;

    fn find(rows: &[MarketPrice], type_id: i64) -> &MarketPrice {
      rows.iter().find(|row| row.type_id() == type_id).expect("row present")
    }

    #[tokio::test]
    async fn it_fills_an_esi_gap_type_from_zkill_tagged_zkill() {
      let esi_server = MockServer::start().await;
      mount_prices(&esi_server, serde_json::json!([])).await;
      let zkill_server = MockServer::start().await;
      mount_zkill_price(&zkill_server, 671, r#"{"typeID": 671, "2024-01-01": 5000000.0}"#).await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      insert_character_asset(&db, 1, 671, 0).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), esi_server.uri());
      let image = eve_image::Client::with_base_url(http.clone(), esi_server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let ctx = ctx(&db, &esi, &image, &image_store);
      let zkill = zkillboard::Client::with_base_url(http, zkill_server.uri());

      run_with_zkill(&ctx, &zkill).await.unwrap();

      let rows = finance::market_prices_all(&db).await.unwrap();
      let row = find(&rows, 671);
      assert_eq!(row.source(), "zkill");
      assert_eq!(row.average_price(), Some(5_000_000.0));
    }

    #[tokio::test]
    async fn it_fills_an_esi_adjusted_only_super_from_zkill_current_price() {
      let esi_server = MockServer::start().await;
      mount_prices(
        &esi_server,
        serde_json::json!([{ "adjusted_price": 1_000.0, "type_id": 23_773 }]),
      )
      .await;
      let zkill_server = MockServer::start().await;
      mount_zkill_price(
        &zkill_server,
        23_773,
        r#"{"typeID": 23773, "currentPrice": 90000000000.0}"#,
      )
      .await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      insert_character_asset(&db, 1, 23_773, 0).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), esi_server.uri());
      let image = eve_image::Client::with_base_url(http.clone(), esi_server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let ctx = ctx(&db, &esi, &image, &image_store);
      let zkill = zkillboard::Client::with_base_url(http, zkill_server.uri());

      run_with_zkill(&ctx, &zkill).await.unwrap();

      let rows = finance::market_prices_all(&db).await.unwrap();
      let row = find(&rows, 23_773);
      assert_eq!(row.source(), "zkill");
      assert_eq!(row.average_price(), Some(90_000_000_000.0));
      assert_eq!(row.adjusted_price(), None);
    }

    #[tokio::test]
    async fn it_sweeps_a_market_traded_type_with_an_esi_average_to_zkill() {
      let esi_server = MockServer::start().await;
      mount_prices(
        &esi_server,
        serde_json::json!([{ "adjusted_price": 9.0, "average_price": 10.0, "type_id": 34 }]),
      )
      .await;
      let zkill_server = MockServer::start().await;
      mount_zkill_price(&zkill_server, 34, r#"{"typeID": 34, "2024-01-01": 999.0}"#).await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      insert_character_asset(&db, 1, 34, 0).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), esi_server.uri());
      let image = eve_image::Client::with_base_url(http.clone(), esi_server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let ctx = ctx(&db, &esi, &image, &image_store);
      let zkill = zkillboard::Client::with_base_url(http, zkill_server.uri());

      run_with_zkill(&ctx, &zkill).await.unwrap();

      let rows = finance::market_prices_all(&db).await.unwrap();
      let row = find(&rows, 34);
      assert_eq!(row.source(), "zkill");
      assert_eq!(row.adjusted_price(), None);
      assert_eq!(row.average_price(), Some(999.0));
    }

    #[tokio::test]
    async fn it_refetches_an_existing_zkill_row() {
      let esi_server = MockServer::start().await;
      mount_prices(&esi_server, serde_json::json!([])).await;
      let zkill_server = MockServer::start().await;
      mount_zkill_price(&zkill_server, 671, r#"{"typeID": 671, "2024-02-01": 7000000.0}"#).await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      insert_character_asset(&db, 1, 671, 0).await;
      finance::market_prices_upsert_many(&db, &[MarketPrice::zkill(671, 1_000.0)])
        .await
        .unwrap();
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), esi_server.uri());
      let image = eve_image::Client::with_base_url(http.clone(), esi_server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let ctx = ctx(&db, &esi, &image, &image_store);
      let zkill = zkillboard::Client::with_base_url(http, zkill_server.uri());

      run_with_zkill(&ctx, &zkill).await.unwrap();

      let rows = finance::market_prices_all(&db).await.unwrap();
      let row = find(&rows, 671);
      assert_eq!(row.source(), "zkill");
      assert_eq!(row.average_price(), Some(7_000_000.0));
    }

    #[tokio::test]
    async fn it_keeps_the_esi_row_when_zkill_returns_no_price() {
      let esi_server = MockServer::start().await;
      mount_prices(
        &esi_server,
        serde_json::json!([{ "average_price": 50.0, "type_id": 35 }]),
      )
      .await;
      let zkill_server = MockServer::start().await;
      mount_zkill_price(&zkill_server, 35, r#"{"typeID": 35}"#).await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      insert_character_asset(&db, 1, 35, 0).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), esi_server.uri());
      let image = eve_image::Client::with_base_url(http.clone(), esi_server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let ctx = ctx(&db, &esi, &image, &image_store);
      let zkill = zkillboard::Client::with_base_url(http, zkill_server.uri());

      run_with_zkill(&ctx, &zkill).await.unwrap();

      let rows = finance::market_prices_all(&db).await.unwrap();
      let row = find(&rows, 35);
      assert_eq!(row.source(), "esi");
      assert_eq!(row.average_price(), Some(50.0));
    }

    #[tokio::test]
    async fn it_lets_esi_reclaim_a_previously_zkill_row() {
      let esi_server = MockServer::start().await;
      mount_prices(
        &esi_server,
        serde_json::json!([{ "average_price": 4_200.0, "type_id": 671 }]),
      )
      .await;
      let zkill_server = MockServer::start().await;
      mount_zkill_price(&zkill_server, 671, r#"{"typeID": 671}"#).await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      insert_character_asset(&db, 1, 671, 0).await;
      finance::market_prices_upsert_many(&db, &[MarketPrice::zkill(671, 1_000.0)])
        .await
        .unwrap();
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), esi_server.uri());
      let image = eve_image::Client::with_base_url(http.clone(), esi_server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let ctx = ctx(&db, &esi, &image, &image_store);
      let zkill = zkillboard::Client::with_base_url(http, zkill_server.uri());

      run_with_zkill(&ctx, &zkill).await.unwrap();

      let rows = finance::market_prices_all(&db).await.unwrap();
      let row = find(&rows, 671);
      assert_eq!(row.source(), "esi");
      assert_eq!(row.average_price(), Some(4_200.0));
    }

    #[tokio::test]
    async fn it_excludes_blueprint_copies_from_the_sweep() {
      let esi_server = MockServer::start().await;
      mount_prices(&esi_server, serde_json::json!([])).await;
      let zkill_server = MockServer::start().await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      insert_character_asset(&db, 1, 671, 1).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), esi_server.uri());
      let image = eve_image::Client::with_base_url(http.clone(), esi_server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let ctx = ctx(&db, &esi, &image, &image_store);
      let zkill = zkillboard::Client::with_base_url(http, zkill_server.uri());

      run_with_zkill(&ctx, &zkill).await.unwrap();

      assert!(finance::market_prices_all(&db).await.unwrap().is_empty());
    }
  }
}
