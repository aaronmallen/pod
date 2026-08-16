use crate::{
  clients::Error,
  store::{
    model::{CharacterWalletJournal, CharacterWalletTransaction},
    repo::{character, finance},
  },
  sync::{job::JobCtx, outcome::Outcome, structure_resolution, subject::Subject},
};

pub async fn run(ctx: &JobCtx<'_>) -> Result<Outcome, Error> {
  let Subject::Character(character_id) = ctx.key.subject else {
    return Ok(Outcome::synced());
  };
  let Some(grant) = ctx.grant else {
    return Err(Error::Internal(format!(
      "character wallet job for {character_id} requires a grant"
    )));
  };
  if character::get(ctx.db, character_id).await?.is_none() {
    return Err(Error::NotReady);
  }
  let authenticated = ctx.esi.character_authenticated(grant);

  let journal: Vec<CharacterWalletJournal> = authenticated
    .wallet_journal()
    .await?
    .into_iter()
    .map(|entry| CharacterWalletJournal::from((character_id, entry)))
    .collect();
  finance::append_wallet_journal(ctx.db, &journal).await?;

  let transactions: Vec<CharacterWalletTransaction> = authenticated
    .wallet_transactions()
    .await?
    .into_iter()
    .map(|transaction| CharacterWalletTransaction::from((character_id, transaction)))
    .collect();
  finance::append_wallet_transaction(ctx.db, &transactions).await?;

  let location_ids: Vec<i64> = transactions
    .iter()
    .map(CharacterWalletTransaction::location_id)
    .collect();
  structure_resolution::resolve_location_ids(ctx, &location_ids).await;

  Ok(Outcome::from_rows(journal.len() + transactions.len()))
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

  async fn mount_json(server: &MockServer, route: &str, body: serde_json::Value) {
    Mock::given(method("GET"))
      .and(path(route))
      .respond_with(ResponseTemplate::new(200).set_body_json(body))
      .mount(server)
      .await;
  }

  async fn mount_paginated(server: &MockServer, route: &str, body: serde_json::Value) {
    Mock::given(method("GET"))
      .and(path(route))
      .respond_with(
        ResponseTemplate::new(200)
          .insert_header("X-Pages", "1")
          .set_body_json(body),
      )
      .mount(server)
      .await;
  }

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

  async fn mount_journal(server: &MockServer, character_id: i64) {
    mount_paginated(
      server,
      &format!("/characters/{character_id}/wallet/journal/"),
      serde_json::json!([
        { "amount": 1000.0, "balance": 50000.0, "date": "2026-05-30T12:00:00Z", "description": "Donation",
          "id": 123456789_i64, "ref_type": "player_donation" },
      ]),
    )
    .await;
  }

  async fn mount_transactions(server: &MockServer, character_id: i64) {
    mount_json(
      server,
      &format!("/characters/{character_id}/wallet/transactions/"),
      serde_json::json!([
        { "client_id": 1000035, "date": "2026-05-30T12:00:00Z", "is_buy": true, "is_personal": true,
          "journal_ref_id": 123456789_i64, "location_id": 60003760, "quantity": 10, "transaction_id": 987654321_i64,
          "type_id": 34, "unit_price": 5.5 },
      ]),
    )
    .await;
  }

  async fn mount_transactions_at(server: &MockServer, character_id: i64, location_id: i64) {
    mount_json(
      server,
      &format!("/characters/{character_id}/wallet/transactions/"),
      serde_json::json!([
        { "client_id": 1000035, "date": "2026-05-30T12:00:00Z", "is_buy": true, "is_personal": true,
          "journal_ref_id": 123456789_i64, "location_id": location_id, "quantity": 10,
          "transaction_id": 987654321_i64, "type_id": 34, "unit_price": 5.5 },
      ]),
    )
    .await;
  }

  async fn mount_transfer_leg(server: &MockServer, character_id: i64, journal_id: i64, amount: f64) {
    mount_paginated(
      server,
      &format!("/characters/{character_id}/wallet/journal/"),
      serde_json::json!([
        { "amount": amount, "balance": 1000.0, "date": "2026-05-30T12:00:00Z", "description": "Internal transfer",
          "id": journal_id, "ref_type": "player_donation" },
      ]),
    )
    .await;
    mount_json(
      server,
      &format!("/characters/{character_id}/wallet/transactions/"),
      serde_json::json!([]),
    )
    .await;
  }

  fn ctx_with_grant<'a>(
    db: &'a store::Database,
    esi: &'a esi::Client,
    image: &'a eve_image::Client,
    image_store: &'a images::Store,
    grant: &'a Grant,
    character_id: i64,
  ) -> JobCtx<'a> {
    JobCtx {
      db,
      esi,
      image,
      image_store,
      key: JobKey::new(JobKind::CharacterWallet, Subject::Character(character_id)),
      grant: Some(grant),
      sso: None,
    }
  }

  mod run {
    use super::*;
    use crate::{
      clients::esi::scopes,
      store::{
        model::{Constellation, OwnerType, Region, SolarSystem},
        repo::sde,
      },
    };

    const CONSTELLATION_ID: i64 = 20_000_020;
    const OWNER_CORP_ID: i64 = 90_000_001;
    const REGION_ID: i64 = 10_000_002;
    const STRUCTURE_ID: i64 = 1_051_885_479_017;
    const SYSTEM_ID: i64 = 30_000_142;

    async fn seed_geography(db: &store::Database) {
      sde::upsert_region(
        db,
        &Region {
          description: None,
          id: REGION_ID,
          name: "The Forge".to_owned(),
        },
      )
      .await
      .unwrap();
      sde::upsert_constellation(
        db,
        &Constellation {
          id: CONSTELLATION_ID,
          name: "Kimotoro".to_owned(),
          position_x: 0.0,
          position_y: 0.0,
          position_z: 0.0,
          region_id: REGION_ID,
        },
      )
      .await
      .unwrap();
      sde::upsert_solar_system(
        db,
        &SolarSystem {
          constellation_id: CONSTELLATION_ID,
          id: SYSTEM_ID,
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

    fn structure_grant(character_id: i64) -> Grant {
      Grant::new_test_with_scopes("token", character_id, vec![scopes::UNIVERSE_STRUCTURES.to_owned()])
    }

    #[tokio::test]
    async fn it_resolves_a_structure_a_transaction_traded_in() {
      let server = MockServer::start().await;
      mount_journal(&server, 42).await;
      mount_transactions_at(&server, 42, STRUCTURE_ID).await;
      Mock::given(method("GET"))
        .and(path(format!("/universe/structures/{STRUCTURE_ID}/")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
          "name": "A Player Structure",
          "owner_id": OWNER_CORP_ID,
          "solar_system_id": SYSTEM_ID,
        })))
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_geography(&db).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = structure_grant(42);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant, 42);

      run(&ctx).await.unwrap();

      let structure = sde::get_structure(&db, STRUCTURE_ID).await.unwrap();
      assert_eq!(
        structure.map(|row| row.name().clone()),
        Some("A Player Structure".to_owned())
      );
    }

    #[tokio::test]
    async fn it_marks_a_structure_it_cannot_reach_inaccessible() {
      let server = MockServer::start().await;
      mount_journal(&server, 42).await;
      mount_transactions_at(&server, 42, STRUCTURE_ID).await;
      Mock::given(method("GET"))
        .and(path(format!("/universe/structures/{STRUCTURE_ID}/")))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = structure_grant(42);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant, 42);

      run(&ctx).await.unwrap();

      assert!(sde::get_structure(&db, STRUCTURE_ID).await.unwrap().is_none());
      assert!(
        sde::is_structure_inaccessible(&db, 42, OwnerType::Character, STRUCTURE_ID)
          .await
          .unwrap(),
        "a 403 records the structure inaccessible instead of retrying it every sync"
      );
      assert_eq!(finance::wallet_transactions(&db, 42).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn it_errors_and_persists_nothing_when_the_journal_fetch_fails() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/characters/42/wallet/journal/"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test("token", 42);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant, 42);

      let result = run(&ctx).await;

      assert!(result.is_err());
      assert!(finance::wallet_journal(&db, 42).await.unwrap().is_empty());
      assert!(character::skills(&db, 42, chrono::Utc::now()).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_persists_journal_and_transactions_without_touching_skills() {
      let server = MockServer::start().await;
      mount_journal(&server, 42).await;
      mount_transactions(&server, 42).await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test("token", 42);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant, 42);

      run(&ctx).await.unwrap();

      assert_eq!(finance::wallet_journal(&db, 42).await.unwrap().len(), 1);
      assert_eq!(finance::wallet_transactions(&db, 42).await.unwrap().len(), 1);
      assert!(character::skills(&db, 42, chrono::Utc::now()).await.unwrap().is_empty());
      assert!(character::skillqueue(&db, 42).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_persists_both_legs_of_a_transfer_sharing_one_eve_id_across_characters() {
      let server = MockServer::start().await;
      mount_transfer_leg(&server, 42, 500, -250.0).await;
      mount_transfer_leg(&server, 43, 500, 250.0).await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_character(&db, 43).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let sender = Grant::new_test("token", 42);
      let receiver = Grant::new_test("token", 43);

      run(&ctx_with_grant(&db, &esi, &image, &image_store, &sender, 42))
        .await
        .unwrap();
      run(&ctx_with_grant(&db, &esi, &image, &image_store, &receiver, 43))
        .await
        .unwrap();

      let sender_leg = finance::wallet_journal(&db, 42).await.unwrap();
      let receiver_leg = finance::wallet_journal(&db, 43).await.unwrap();
      assert_eq!(
        sender_leg.len(),
        1,
        "the sender's -N leg persists under the per-wallet key"
      );
      assert_eq!(
        receiver_leg.len(),
        1,
        "the receiver's +N leg shares the same EVE id but a different character, so it persists too"
      );
      assert_eq!(sender_leg[0].id(), receiver_leg[0].id());
      assert_eq!(sender_leg[0].amount(), Some(-250.0));
      assert_eq!(receiver_leg[0].amount(), Some(250.0));
    }

    #[tokio::test]
    async fn it_skips_without_fetching_when_the_character_is_not_yet_persisted() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/characters/42/wallet/journal/"))
        .respond_with(
          ResponseTemplate::new(200)
            .insert_header("X-Pages", "1")
            .set_body_json(serde_json::json!([])),
        )
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
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, &grant, 42);

      let result = run(&ctx).await;

      assert!(
        matches!(result, Err(Error::NotReady)),
        "a missing parent row must surface NotReady for a short token-free retry, not a clean Ok"
      );
      assert!(finance::wallet_journal(&db, 42).await.unwrap().is_empty());
    }
  }
}
