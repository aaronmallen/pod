use crate::{
  clients::Error,
  store::{
    model::{CharacterWalletJournal, CharacterWalletTransaction},
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
      assert!(character::skills(&db, 42).await.unwrap().is_empty());
      assert!(character::skillqueue(&db, 42).await.unwrap().is_empty());
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
      assert!(character::skills(&db, 42).await.unwrap().is_empty());
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
