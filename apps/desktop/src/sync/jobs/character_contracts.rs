use std::collections::{HashMap, HashSet};

use super::killmail_value::PriceTable;
use crate::{
  clients::{
    Error,
    esi::{
      character::AuthenticatedClient,
      models::{
        character::{Contract, ContractBid, ContractItem},
        universe::NameRecord,
      },
    },
  },
  store::{
    model::{CharacterContract, CharacterContractBid, CharacterContractItem},
    repo::{character, finance},
  },
  sync::{job::JobCtx, jobs::names::resolve_names, outcome::Outcome, structure_resolution, subject::Subject},
};

const AUCTION_TYPE: &str = "auction";
const STRUCTURE_ID_FLOOR: i64 = 1_000_000_000_000;

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

  let location_ids: Vec<i64> = contracts
    .iter()
    .flat_map(|contract| contract.start_location_id.into_iter().chain(contract.end_location_id))
    .filter(|&id| id != 0)
    .collect();
  resolve_locations(ctx, &location_ids).await;

  let prices = PriceTable::from_market_prices(&finance::market_prices_all(ctx.db).await?);
  let client = ctx.esi.character_authenticated(grant);
  for contract in &contracts {
    persist_children(ctx, character_id, contract, &prices, &client).await?;
  }

  let rows: Vec<CharacterContract> = contracts
    .into_iter()
    .map(|contract| to_model(character_id, contract, &resolved))
    .collect();

  finance::replace_for_character(ctx.db, character_id, &rows).await?;
  Ok(Outcome::from_rows(rows.len()))
}

async fn persist_children(
  ctx: &JobCtx<'_>,
  character_id: i64,
  contract: &Contract,
  prices: &PriceTable,
  client: &AuthenticatedClient<'_>,
) -> Result<(), Error> {
  let contract_id = contract.contract_id;

  // A contract's item set never changes once the contract exists, so items already stored locally
  // are not refetched -- this keeps the per-sync ESI cost to one items call per newly seen contract.
  if finance::contract_items(ctx.db, character_id, contract_id)
    .await?
    .is_empty()
  {
    let items = client.contract_items(contract_id).await?;
    let rows = item_rows(character_id, contract_id, &items, prices);
    finance::replace_contract_items_for_character(ctx.db, character_id, contract_id, &rows).await?;
  }

  // Bids only ever change while an auction is live, so a terminal (finished/cancelled/etc.) auction
  // is immutable and its bids are never refetched.
  if is_auction(contract) && !is_terminal(contract) {
    let bids = client.contract_bids(contract_id).await?;
    let rows = bid_rows(character_id, contract_id, &bids);
    finance::replace_contract_bids_for_character(ctx.db, character_id, contract_id, &rows).await?;
  }

  Ok(())
}

async fn resolve_locations(ctx: &JobCtx<'_>, location_ids: &[i64]) {
  let mut seen = HashSet::new();
  let mut stations = Vec::new();
  let mut structures = Vec::new();
  for &location_id in location_ids {
    if !seen.insert(location_id) {
      continue;
    }
    if location_id >= STRUCTURE_ID_FLOOR {
      structures.push(location_id);
    } else {
      stations.push(location_id);
    }
  }

  // A player structure the character cannot dock at stays unresolved and is rendered as a raw
  // `Structure {id}` label at modal-load time, so an inaccessible structure must not fail the job.
  if let Err(error) = structure_resolution::resolve_asset_references(ctx, &[], &stations, &structures).await {
    tracing::warn!("character contracts: location resolution failed: {error}");
  }
}

fn bid_rows(character_id: i64, contract_id: i64, bids: &[ContractBid]) -> Vec<CharacterContractBid> {
  bids
    .iter()
    .map(|bid| CharacterContractBid {
      amount: bid.amount,
      bid_id: bid.bid_id,
      bidder_id: bid.bidder_id,
      character_id,
      contract_id,
      date_bid: bid.date_bid.clone(),
    })
    .collect()
}

fn item_rows(
  character_id: i64,
  contract_id: i64,
  items: &[ContractItem],
  prices: &PriceTable,
) -> Vec<CharacterContractItem> {
  items
    .iter()
    .map(|item| {
      let quantity = i64::from(item.quantity);
      CharacterContractItem {
        character_id,
        contract_id,
        is_included: item.is_included,
        is_singleton: item.is_singleton,
        quantity,
        raw_quantity: item.raw_quantity.map(i64::from),
        record_id: item.record_id,
        type_id: i64::from(item.type_id),
        value_isk: prices.unit_price(i64::from(item.type_id)) * quantity.max(0) as f64,
      }
    })
    .collect()
}

fn is_auction(contract: &Contract) -> bool {
  contract.contract_type.as_deref() == Some(AUCTION_TYPE)
}

fn is_terminal(contract: &Contract) -> bool {
  matches!(
    contract.status.as_deref(),
    Some(
      "cancelled"
        | "deleted"
        | "failed"
        | "finished"
        | "finished_contractor"
        | "finished_issuer"
        | "rejected"
        | "reversed"
    )
  )
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

  async fn mount_items(server: &MockServer, contract_id: i64, body: serde_json::Value) {
    Mock::given(method("GET"))
      .and(path(format!("/characters/42/contracts/{contract_id}/items/")))
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
    async fn it_does_not_fetch_bids_for_a_finished_auction() {
      let server = MockServer::start().await;
      mount_contracts(
        &server,
        42,
        serde_json::json!([
          { "contract_id": 14, "type": "auction", "status": "finished", "issuer_id": 1001,
            "date_issued": "2024-07-01T00:00:00Z", "for_corporation": false },
        ]),
      )
      .await;
      mount_names(
        &server,
        serde_json::json!([{ "category": "character", "id": 1001, "name": "Issuer Pilot" }]),
      )
      .await;
      mount_items(&server, 14, serde_json::json!([])).await;
      Mock::given(method("GET"))
        .and(path("/characters/42/contracts/14/bids/"))
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
      let grant = Grant::new_test("token", 42);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, Some(&grant), 42);

      run(&ctx).await.unwrap();

      assert!(finance::contract_bids(&db, 42, 14).await.unwrap().is_empty());
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
    async fn it_fetches_and_persists_items_for_an_item_exchange_contract() {
      let server = MockServer::start().await;
      mount_contracts(
        &server,
        42,
        serde_json::json!([
          { "contract_id": 12, "type": "item_exchange", "status": "outstanding", "issuer_id": 1001,
            "date_issued": "2024-05-01T00:00:00Z", "for_corporation": false },
        ]),
      )
      .await;
      mount_names(
        &server,
        serde_json::json!([{ "category": "character", "id": 1001, "name": "Issuer Pilot" }]),
      )
      .await;
      mount_items(
        &server,
        12,
        serde_json::json!([
          { "record_id": 1000, "type_id": 34, "quantity": 5, "is_included": true, "is_singleton": false },
          { "record_id": 1001, "type_id": 99, "quantity": 1, "is_included": false, "is_singleton": true },
        ]),
      )
      .await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      finance::market_prices_upsert_many(&db, &[store::model::MarketPrice::esi(34, Some(10.0), None)])
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

      let items = finance::contract_items(&db, 42, 12).await.unwrap();
      assert_eq!(items.len(), 2);
      let included = items.iter().find(|i| i.record_id() == 1000).unwrap();
      assert_eq!(included.type_id(), 34);
      assert_eq!(included.quantity(), 5);
      assert!(included.is_included());
      assert_eq!(included.value_isk(), 50.0);
      let unpriced = items.iter().find(|i| i.record_id() == 1001).unwrap();
      assert_eq!(unpriced.value_isk(), 0.0);
      assert!(unpriced.is_singleton());
    }

    #[tokio::test]
    async fn it_fetches_bids_for_an_outstanding_auction() {
      let server = MockServer::start().await;
      mount_contracts(
        &server,
        42,
        serde_json::json!([
          { "contract_id": 13, "type": "auction", "status": "outstanding", "issuer_id": 1001,
            "date_issued": "2024-06-01T00:00:00Z", "for_corporation": false },
        ]),
      )
      .await;
      mount_names(
        &server,
        serde_json::json!([{ "category": "character", "id": 1001, "name": "Issuer Pilot" }]),
      )
      .await;
      mount_items(&server, 13, serde_json::json!([])).await;
      Mock::given(method("GET"))
        .and(path("/characters/42/contracts/13/bids/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
          { "bid_id": 70, "bidder_id": 5005, "amount": 1500.0, "date_bid": "2024-06-02T00:00:00Z" },
        ])))
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
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, Some(&grant), 42);

      run(&ctx).await.unwrap();

      let bids = finance::contract_bids(&db, 42, 13).await.unwrap();
      assert_eq!(bids.len(), 1);
      assert_eq!(bids[0].bid_id(), 70);
      assert_eq!(bids[0].bidder_id(), 5005);
      assert_eq!(bids[0].amount(), 1500.0);
    }

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
      mount_items(&server, 9, serde_json::json!([])).await;
      mount_items(&server, 10, serde_json::json!([])).await;
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
    async fn it_persists_the_new_header_fields() {
      let server = MockServer::start().await;
      mount_contracts(
        &server,
        42,
        serde_json::json!([
          { "contract_id": 11, "type": "item_exchange", "status": "in_progress", "issuer_id": 1001,
            "issuer_corporation_id": 90000001, "title": "Heavy haul", "availability": "personal",
            "days_to_complete": 7, "start_location_id": 60003760, "end_location_id": 1030000000001_i64,
            "date_accepted": "2024-04-02T00:00:00Z", "date_issued": "2024-04-01T00:00:00Z",
            "for_corporation": false },
        ]),
      )
      .await;
      mount_names(
        &server,
        serde_json::json!([{ "category": "character", "id": 1001, "name": "Issuer Pilot" }]),
      )
      .await;
      mount_items(&server, 11, serde_json::json!([])).await;
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
      let contract = stored.iter().find(|c| c.contract_id() == 11).unwrap();
      assert_eq!(contract.title().as_deref(), Some("Heavy haul"));
      assert_eq!(contract.availability().as_deref(), Some("personal"));
      assert_eq!(contract.days_to_complete(), Some(7));
      assert_eq!(contract.start_location_id(), Some(60003760));
      assert_eq!(contract.end_location_id(), Some(1030000000001));
      assert_eq!(contract.date_accepted().as_deref(), Some("2024-04-02T00:00:00Z"));
      assert_eq!(contract.issuer_corporation_id(), Some(90000001));
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
      mount_items(&server, 20, serde_json::json!([])).await;
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
    async fn it_skips_item_fetch_for_a_contract_whose_items_are_already_stored() {
      let server = MockServer::start().await;
      mount_contracts(
        &server,
        42,
        serde_json::json!([
          { "contract_id": 15, "type": "item_exchange", "status": "finished", "issuer_id": 1001,
            "date_issued": "2024-08-01T00:00:00Z", "for_corporation": false },
        ]),
      )
      .await;
      mount_names(
        &server,
        serde_json::json!([{ "category": "character", "id": 1001, "name": "Issuer Pilot" }]),
      )
      .await;
      Mock::given(method("GET"))
        .and(path("/characters/42/contracts/15/items/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .expect(0)
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      finance::replace_contract_items_for_character(
        &db,
        42,
        15,
        &[CharacterContractItem {
          character_id: 42,
          contract_id: 15,
          is_included: true,
          is_singleton: false,
          quantity: 3,
          raw_quantity: None,
          record_id: 5000,
          type_id: 34,
          value_isk: 30.0,
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

      let items = finance::contract_items(&db, 42, 15).await.unwrap();
      assert_eq!(items.len(), 1);
      assert_eq!(items[0].record_id(), 5000);
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
