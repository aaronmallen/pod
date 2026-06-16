use std::collections::HashMap;

use crate::{
  clients::{
    Error,
    esi::models::{character::Contract, universe::NameRecord},
  },
  store::{
    model::CharacterContract,
    repo::{character, finance},
  },
  sync::{job::JobCtx, jobs::names::resolve_names, outcome::Outcome, subject::Subject},
};

pub async fn run(ctx: &JobCtx<'_>) -> Result<Outcome, Error> {
  let Subject::Character(character_id) = ctx.key.subject else {
    return Ok(Outcome::synced());
  };
  let Some(grant) = ctx.grant else {
    return Err(Error::Internal(format!(
      "character contracts job for {character_id} requires a grant"
    )));
  };
  if character::get(ctx.db, character_id).await?.is_none() {
    return Err(Error::NotReady);
  }

  let contracts = ctx.esi.character_authenticated(grant).contracts().await?;

  let resolver_ids: Vec<i64> = contracts
    .iter()
    .flat_map(|contract| {
      contract
        .issuer_id
        .into_iter()
        .chain(contract.assignee_id)
        .chain(contract.acceptor_id)
    })
    .filter(|&id| id != 0)
    .collect();
  let resolved = resolve_names(ctx, &resolver_ids).await?;

  let rows: Vec<CharacterContract> = contracts
    .into_iter()
    .map(|contract| to_model(character_id, contract, &resolved))
    .collect();

  finance::replace_for_character(ctx.db, character_id, &rows).await?;
  Ok(Outcome::from_rows(rows.len()))
}

fn to_model(character_id: i64, contract: Contract, resolved: &HashMap<i64, NameRecord>) -> CharacterContract {
  let issuer_id = contract.issuer_id.unwrap_or_default();
  CharacterContract {
    acceptor_id: contract.acceptor_id,
    acceptor_name: contract.acceptor_id.and_then(|id| resolved_name(resolved, id)),
    assignee_id: contract.assignee_id,
    assignee_name: contract.assignee_id.and_then(|id| resolved_name(resolved, id)),
    availability: contract.availability,
    character_id,
    collateral: contract.collateral,
    contract_id: contract.contract_id,
    date_accepted: contract.date_accepted,
    date_completed: contract.date_completed,
    date_expired: contract.date_expired,
    date_issued: contract.date_issued.unwrap_or_default(),
    days_to_complete: contract.days_to_complete.map(i64::from),
    end_location_id: contract.end_location_id,
    for_corporation: contract.for_corporation.unwrap_or(false),
    issuer_corporation_id: contract.issuer_corporation_id,
    issuer_id,
    issuer_name: resolved_name(resolved, issuer_id),
    price: contract.price,
    reward: contract.reward,
    start_location_id: contract.start_location_id,
    status: contract.status.unwrap_or_default(),
    title: contract.title,
    r#type: contract.contract_type.unwrap_or_default(),
    volume: contract.volume,
  }
}

fn resolved_name(resolved: &HashMap<i64, NameRecord>, id: i64) -> Option<String> {
  resolved.get(&id).map(|record| record.name.clone())
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

  async fn mount_contracts(server: &MockServer, character_id: i64, body: serde_json::Value) {
    Mock::given(method("GET"))
      .and(path(format!("/characters/{character_id}/contracts/")))
      .respond_with(ResponseTemplate::new(200).set_body_json(body))
      .mount(server)
      .await;
  }

  async fn mount_names(server: &MockServer, body: serde_json::Value) {
    Mock::given(method("POST"))
      .and(path("/universe/names/"))
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
      key: JobKey::new(JobKind::CharacterContracts, Subject::Character(character_id)),
      grant,
      sso: None,
    }
  }

  mod run {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::repo::finance;

    #[tokio::test]
    async fn it_persists_contracts_with_resolved_party_names() {
      let server = MockServer::start().await;
      mount_contracts(
        &server,
        42,
        serde_json::json!([
          { "contract_id": 9, "type": "courier", "status": "outstanding", "issuer_id": 1001,
            "assignee_id": 2002, "acceptor_id": 3003, "reward": 1234.5, "collateral": 6789.0, "volume": 250.0,
            "date_issued": "2024-01-01T00:00:00Z", "for_corporation": false },
          { "contract_id": 10, "type": "item_exchange", "status": "finished", "issuer_id": 1001,
            "price": 50.0, "date_issued": "2024-02-01T00:00:00Z", "for_corporation": true },
        ]),
      )
      .await;
      mount_names(
        &server,
        serde_json::json!([
          { "category": "character", "id": 1001, "name": "Issuer Pilot" },
          { "category": "character", "id": 2002, "name": "Assignee Pilot" },
          { "category": "character", "id": 3003, "name": "Acceptor Pilot" },
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

      let stored = finance::contracts(&db, 42).await.unwrap();
      assert_eq!(stored.len(), 2);
      let courier = stored.iter().find(|c| c.contract_id() == 9).unwrap();
      assert_eq!(courier.status(), "outstanding");
      assert_eq!(courier.r#type(), "courier");
      assert_eq!(courier.issuer_name().as_deref(), Some("Issuer Pilot"));
      assert_eq!(courier.assignee_name().as_deref(), Some("Assignee Pilot"));
      assert_eq!(courier.acceptor_name().as_deref(), Some("Acceptor Pilot"));
      assert_eq!(courier.collateral(), Some(6789.0));
      let exchange = stored.iter().find(|c| c.contract_id() == 10).unwrap();
      assert!(exchange.for_corporation());
      assert_eq!(exchange.price(), Some(50.0));
      assert!(exchange.assignee_id().is_none());
    }

    #[tokio::test]
    async fn it_replaces_the_prior_set() {
      let server = MockServer::start().await;
      mount_contracts(
        &server,
        42,
        serde_json::json!([
          { "contract_id": 20, "type": "courier", "status": "outstanding", "issuer_id": 1001,
            "date_issued": "2024-03-01T00:00:00Z", "for_corporation": false },
        ]),
      )
      .await;
      mount_names(
        &server,
        serde_json::json!([{ "category": "character", "id": 1001, "name": "Issuer Pilot" }]),
      )
      .await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      finance::replace_for_character(
        &db,
        42,
        &[CharacterContract {
          acceptor_id: None,
          acceptor_name: None,
          assignee_id: None,
          assignee_name: None,
          availability: None,
          character_id: 42,
          collateral: None,
          contract_id: 999,
          date_accepted: None,
          date_completed: None,
          date_expired: None,
          date_issued: "2023-01-01T00:00:00Z".to_owned(),
          days_to_complete: None,
          end_location_id: None,
          for_corporation: false,
          issuer_corporation_id: None,
          issuer_id: 1001,
          issuer_name: Some("Old Pilot".to_owned()),
          price: None,
          reward: None,
          start_location_id: None,
          status: "finished".to_owned(),
          title: None,
          r#type: "courier".to_owned(),
          volume: None,
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

      let stored = finance::contracts(&db, 42).await.unwrap();
      assert_eq!(stored.len(), 1);
      assert_eq!(stored[0].contract_id(), 20);
    }

    #[tokio::test]
    async fn it_errors_when_the_grant_is_missing() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/characters/42/contracts/"))
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
      assert!(finance::contracts(&db, 42).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_skips_without_fetching_when_the_character_is_not_yet_persisted() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/characters/42/contracts/"))
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
      assert!(finance::contracts(&db, 42).await.unwrap().is_empty());
    }
  }
}
