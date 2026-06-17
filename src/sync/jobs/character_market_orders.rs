use crate::{
  clients::Error,
  store::{
    model::MarketOrder,
    repo::{character, finance},
  },
  sync::{job::JobCtx, outcome::Outcome, subject::Subject},
};

pub async fn run(ctx: &JobCtx<'_>) -> Result<Outcome, Error> {
  let Subject::Character(character_id) = ctx.key.subject else {
    return Ok(Outcome::synced());
  };
  let Some(grant) = ctx.grant else {
    return Err(Error::Internal(format!(
      "character market orders job for {character_id} requires a grant"
    )));
  };
  if character::get(ctx.db, character_id).await?.is_none() {
    return Err(Error::NotReady);
  }

  let orders: Vec<MarketOrder> = ctx
    .esi
    .character_authenticated(grant)
    .orders()
    .await?
    .into_iter()
    .map(|order| MarketOrder::from((character_id, order)))
    .collect();
  finance::replace(ctx.db, character_id, &orders).await?;
  Ok(Outcome::from_rows(orders.len()))
}

#[cfg(test)]
mod tests {
  use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
  };

  use super::*;
  use crate::{
    clients::{esi, eve_image, eve_sso::Grant, http},
    store::{self, images},
    sync::job::{JobKey, JobKind},
  };

  async fn seed_character(db: &store::Database, id: i64) {
    use store::{
      model::{Alliance, Bloodline, Character, Corporation, Gender, Race},
      repo::character,
    };
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

  async fn mount_orders(server: &MockServer, character_id: i64, body: serde_json::Value) {
    Mock::given(method("GET"))
      .and(path(format!("/characters/{character_id}/orders/")))
      .respond_with(ResponseTemplate::new(200).set_body_json(body))
      .mount(server)
      .await;
  }

  fn ctx_with_grant<'a>(
    db: &'a store::Database,
    esi: &'a esi::Client,
    image: &'a eve_image::Client,
    image_store: &'a images::Store,
    grant: Option<&'a Grant>,
    character_id: i64,
  ) -> JobCtx<'a> {
    JobCtx {
      db,
      esi,
      image,
      image_store,
      key: JobKey::new(JobKind::CharacterMarketOrders, Subject::Character(character_id)),
      grant,
      sso: None,
    }
  }

  mod run {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_errors_when_the_grant_is_missing() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/characters/42/orders/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .expect(0)
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, None, 42);

      let result = run(&ctx).await;

      assert!(result.is_err());
      assert!(finance::for_character(&db, 42).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_persists_the_characters_open_orders_as_state_open() {
      let server = MockServer::start().await;
      mount_orders(
        &server,
        42,
        serde_json::json!([
          { "order_id": 1001, "type_id": 34, "region_id": 10_000_002, "location_id": 60_003_760,
            "range": "region", "is_buy_order": true, "price": 5.5, "volume_remain": 100, "volume_total": 200,
            "escrow": 550.0, "duration": 90, "issued": "2026-06-01T12:00:00Z" },
        ]),
      )
      .await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test("token", 42);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, Some(&grant), 42);

      run(&ctx).await.unwrap();

      let stored = finance::for_character(&db, 42).await.unwrap();
      assert_eq!(stored.len(), 1);
      assert_eq!(stored[0].order_id(), 1001);
      assert_eq!(stored[0].escrow(), 550.0);
      assert_eq!(stored[0].state(), "open");
      assert_eq!(finance::open_escrow(&db, 42).await.unwrap(), 550.0);
    }

    #[tokio::test]
    async fn it_replaces_the_prior_set_so_closed_orders_drop_out() {
      let server = MockServer::start().await;
      mount_orders(
        &server,
        42,
        serde_json::json!([
          { "order_id": 2002, "type_id": 35, "region_id": 10_000_002, "location_id": 60_003_760,
            "range": "station", "is_buy_order": false, "price": 12.3, "volume_remain": 5, "volume_total": 5,
            "duration": 30, "issued": "2026-06-02T00:00:00Z" },
        ]),
      )
      .await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      finance::replace(
        &db,
        42,
        &[MarketOrder {
          character_id: 42,
          duration: 90,
          escrow: 999.0,
          is_buy_order: true,
          issued: "2026-05-01T00:00:00Z".to_owned(),
          location_id: 60_003_760,
          order_id: 1001,
          price: 1.0,
          range: "region".to_owned(),
          region_id: 10_000_002,
          state: "open".to_owned(),
          type_id: 34,
          volume_remain: 1,
          volume_total: 1,
        }],
      )
      .await
      .unwrap();
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test("token", 42);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, Some(&grant), 42);

      run(&ctx).await.unwrap();

      let stored = finance::for_character(&db, 42).await.unwrap();
      assert_eq!(stored.len(), 1);
      assert_eq!(stored[0].order_id(), 2002);
      assert_eq!(finance::open_escrow(&db, 42).await.unwrap(), 0.0);
    }

    #[tokio::test]
    async fn it_skips_without_fetching_when_the_character_is_not_yet_persisted() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/characters/42/orders/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .expect(0)
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test("token", 42);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, Some(&grant), 42);

      let result = run(&ctx).await;

      assert!(
        matches!(result, Err(Error::NotReady)),
        "a missing parent row must surface NotReady for a short token-free retry, not a clean Ok"
      );
      assert!(finance::for_character(&db, 42).await.unwrap().is_empty());
    }
  }
}
