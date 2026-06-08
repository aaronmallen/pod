use crate::{
  clients::Error,
  store::{model::MarketPrice, repo::finance},
  sync::{job::JobCtx, outcome::Outcome},
};

pub async fn run(ctx: &JobCtx<'_>) -> Result<Outcome, Error> {
  let prices: Vec<MarketPrice> = ctx
    .esi
    .market()
    .prices()
    .await?
    .into_iter()
    .map(|price| MarketPrice {
      adjusted_price: price.adjusted_price,
      average_price: price.average_price,
      type_id: price.type_id,
    })
    .collect();
  finance::market_prices_upsert_many(ctx.db, &prices).await?;
  Ok(Outcome::from_rows(prices.len()))
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
    store::{self, images, repo::finance},
    sync::{
      job::{JobKey, JobKind},
      subject::Subject,
    },
  };

  async fn mount_prices(server: &MockServer, body: serde_json::Value) {
    Mock::given(method("GET"))
      .and(path("/markets/prices/"))
      .respond_with(ResponseTemplate::new(200).set_body_json(body))
      .mount(server)
      .await;
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
    }
  }

  mod run {
    use pretty_assertions::assert_eq;

    use super::*;

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

    #[tokio::test]
    async fn it_overwrites_existing_rows_on_a_re_run() {
      let server = MockServer::start().await;
      mount_prices(
        &server,
        serde_json::json!([{ "adjusted_price": 9.0, "average_price": 10.0, "type_id": 34 }]),
      )
      .await;
      let db = store::open_test().await.unwrap();
      finance::market_prices_upsert_many(
        &db,
        &[MarketPrice {
          adjusted_price: Some(1.0),
          average_price: Some(2.0),
          type_id: 34,
        }],
      )
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
  }
}
