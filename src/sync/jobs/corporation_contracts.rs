use std::collections::{HashMap, HashSet};

use super::killmail_value::PriceTable;
use crate::{
  clients::{
    Error,
    esi::models::{
      character::{Contract, ContractBid, ContractItem},
      universe::NameRecord,
    },
  },
  store::{
    model::{CorporationContract, CorporationContractBid, CorporationContractItem},
    repo::{finance, org},
  },
  sync::{job::JobCtx, jobs::names::resolve_names, outcome::Outcome, subject::Subject},
};

const CONTRACT_TYPE_AUCTION: &str = "auction";
const DETAIL_FETCH_BUDGET: usize = 50;

pub async fn run(ctx: &JobCtx<'_>) -> Result<Outcome, Error> {
  let Subject::Corporation(corporation_id) = ctx.key.subject else {
    return Ok(Outcome::synced());
  };
  let Some(grant) = ctx.grant else {
    return Err(Error::Internal(format!(
      "corporation contracts job for {corporation_id} requires a grant"
    )));
  };
  if org::get_corporation(ctx.db, corporation_id).await?.is_none() {
    return Err(Error::NotReady);
  }
  let authenticated = ctx.esi.corporation_authenticated(grant);

  let contracts = authenticated.contracts(corporation_id).await?;

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

  // Contracts already persisted had their items and bids fetched on a prior run, so the per-run
  // budget is spent only on contracts new to this corporation.
  let already_persisted: HashSet<i64> = finance::corporation_contracts(ctx.db, corporation_id)
    .await?
    .into_iter()
    .map(|contract| contract.contract_id())
    .collect();
  let prices = PriceTable::from_market_prices(&finance::market_prices_all(ctx.db).await?);

  let headers: Vec<CorporationContract> = contracts
    .iter()
    .map(|contract| to_header(corporation_id, contract, &resolved))
    .collect();
  finance::upsert_for_corporation(ctx.db, corporation_id, &headers).await?;

  let mut budget = DETAIL_FETCH_BUDGET;
  for contract in &contracts {
    if already_persisted.contains(&contract.contract_id) || budget == 0 {
      continue;
    }
    budget -= 1;

    let items = authenticated
      .contract_items(corporation_id, contract.contract_id)
      .await?
      .into_iter()
      .map(|item| to_item(corporation_id, contract.contract_id, item, &prices))
      .collect::<Vec<_>>();
    finance::replace_contract_items_for_corporation(ctx.db, corporation_id, contract.contract_id, &items).await?;

    if contract.contract_type.as_deref() == Some(CONTRACT_TYPE_AUCTION) {
      let bids = authenticated
        .contract_bids(corporation_id, contract.contract_id)
        .await?
        .into_iter()
        .map(|bid| to_bid(corporation_id, contract.contract_id, bid))
        .collect::<Vec<_>>();
      finance::replace_contract_bids_for_corporation(ctx.db, corporation_id, contract.contract_id, &bids).await?;
    }
  }

  Ok(Outcome::from_rows(headers.len()))
}

fn resolved_name(resolved: &HashMap<i64, NameRecord>, id: i64) -> Option<String> {
  resolved.get(&id).map(|record| record.name.clone())
}

fn to_bid(corporation_id: i64, contract_id: i64, bid: ContractBid) -> CorporationContractBid {
  CorporationContractBid {
    amount: bid.amount,
    bid_id: bid.bid_id,
    bidder_id: bid.bidder_id,
    contract_id,
    corporation_id,
    date_bid: bid.date_bid,
  }
}

fn to_header(corporation_id: i64, contract: &Contract, resolved: &HashMap<i64, NameRecord>) -> CorporationContract {
  let issuer_id = contract.issuer_id.unwrap_or_default();
  CorporationContract {
    acceptor_id: contract.acceptor_id,
    acceptor_name: contract.acceptor_id.and_then(|id| resolved_name(resolved, id)),
    assignee_id: contract.assignee_id,
    assignee_name: contract.assignee_id.and_then(|id| resolved_name(resolved, id)),
    availability: contract.availability.clone(),
    collateral: contract.collateral,
    contract_id: contract.contract_id,
    corporation_id,
    date_accepted: contract.date_accepted.clone(),
    date_completed: contract.date_completed.clone(),
    date_expired: contract.date_expired.clone(),
    date_issued: contract.date_issued.clone().unwrap_or_default(),
    days_to_complete: contract.days_to_complete.map(i64::from),
    end_location_id: contract.end_location_id,
    for_corporation: contract.for_corporation.unwrap_or(false),
    issuer_corporation_id: contract.issuer_corporation_id,
    issuer_id,
    issuer_name: resolved_name(resolved, issuer_id),
    price: contract.price,
    reward: contract.reward,
    start_location_id: contract.start_location_id,
    status: contract.status.clone().unwrap_or_default(),
    title: contract.title.clone(),
    r#type: contract.contract_type.clone().unwrap_or_default(),
    volume: contract.volume,
  }
}

fn to_item(corporation_id: i64, contract_id: i64, item: ContractItem, prices: &PriceTable) -> CorporationContractItem {
  let quantity = i64::from(item.quantity);
  let type_id = i64::from(item.type_id);
  CorporationContractItem {
    contract_id,
    corporation_id,
    is_included: item.is_included,
    is_singleton: item.is_singleton,
    quantity,
    raw_quantity: item.raw_quantity.map(i64::from),
    record_id: item.record_id,
    type_id,
    value_isk: prices.unit_price(type_id) * quantity.max(0) as f64,
  }
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
    store::{self, images, model::Corporation},
    sync::job::{JobKey, JobKind},
  };

  async fn mount_json(server: &MockServer, route: &str, body: serde_json::Value) {
    Mock::given(method("GET"))
      .and(path(route))
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

  async fn seed_corporation(db: &store::Database, corporation_id: i64) {
    let mut corp = Corporation::new(corporation_id, "Test Corp", "TSC");
    corp.set_ceo_id(100);
    corp.set_creator_id(100);
    corp.set_member_count(1);
    corp.set_tax_rate(0.0);
    org::upsert_corporation(db, &corp).await.unwrap();
  }

  fn ctx_with_grant<'a>(
    db: &'a store::Database,
    esi: &'a esi::Client,
    image: &'a eve_image::Client,
    image_store: &'a images::Store,
    grant: Option<&'a Grant>,
    corporation_id: i64,
  ) -> JobCtx<'a> {
    JobCtx {
      db,
      esi,
      image,
      image_store,
      key: JobKey::new(JobKind::CorporationContracts, Subject::Corporation(corporation_id)),
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
        .and(path("/corporations/2000/contracts/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .expect(0)
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      seed_corporation(&db, 2000).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, None, 2000);

      let result = run(&ctx).await;

      assert!(result.is_err());
      assert!(finance::corporation_contracts(&db, 2000).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_persists_contracts_items_and_auction_bids_with_resolved_party_names() {
      let server = MockServer::start().await;
      mount_json(
        &server,
        "/corporations/2000/contracts/",
        serde_json::json!([
          { "contract_id": 9, "type": "auction", "status": "outstanding", "issuer_id": 1001,
            "assignee_id": 2002, "acceptor_id": 3003, "reward": 1234.5, "collateral": 6789.0, "volume": 250.0,
            "date_issued": "2024-01-01T00:00:00Z", "for_corporation": true },
          { "contract_id": 10, "type": "item_exchange", "status": "finished", "issuer_id": 1001,
            "price": 50.0, "date_issued": "2024-02-01T00:00:00Z", "for_corporation": true },
        ]),
      )
      .await;
      mount_json(
        &server,
        "/corporations/2000/contracts/9/items/",
        serde_json::json!([
          { "record_id": 1, "type_id": 34, "quantity": 100, "is_included": true, "is_singleton": false },
        ]),
      )
      .await;
      mount_json(
        &server,
        "/corporations/2000/contracts/9/bids/",
        serde_json::json!([
          { "bid_id": 7, "bidder_id": 4004, "amount": 500.0, "date_bid": "2024-01-02T00:00:00Z" },
        ]),
      )
      .await;
      mount_json(
        &server,
        "/corporations/2000/contracts/10/items/",
        serde_json::json!([
          { "record_id": 2, "type_id": 35, "quantity": 5, "is_included": true, "is_singleton": false },
        ]),
      )
      .await;
      mount_names(
        &server,
        serde_json::json!([
          { "category": "character", "id": 1001, "name": "Issuer Pilot" },
          { "category": "corporation", "id": 2002, "name": "Assignee Corp" },
          { "category": "character", "id": 3003, "name": "Acceptor Pilot" },
        ]),
      )
      .await;
      let db = store::open_test().await.unwrap();
      seed_corporation(&db, 2000).await;
      finance::market_prices_upsert_many(&db, &[store::model::MarketPrice::esi(34, Some(10.0), Some(9.0))])
        .await
        .unwrap();
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test("corp-token", 2000);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, Some(&grant), 2000);

      let outcome = run(&ctx).await.unwrap();

      assert_eq!(
        outcome,
        Outcome::Synced {
          rows_touched: 2
        }
      );
      let stored = finance::corporation_contracts(&db, 2000).await.unwrap();
      assert_eq!(stored.len(), 2);
      let auction = stored.iter().find(|c| c.contract_id() == 9).unwrap();
      assert_eq!(auction.issuer_name().as_deref(), Some("Issuer Pilot"));
      assert_eq!(auction.assignee_name().as_deref(), Some("Assignee Corp"));
      assert_eq!(auction.acceptor_name().as_deref(), Some("Acceptor Pilot"));
      let items = finance::corporation_contract_items(&db, 2000, 9).await.unwrap();
      assert_eq!(items.len(), 1);
      assert_eq!(items[0].value_isk(), 1000.0);
      let bids = finance::corporation_contract_bids(&db, 2000, 9).await.unwrap();
      assert_eq!(bids.len(), 1);
      assert_eq!(bids[0].amount(), 500.0);
      let exchange_items = finance::corporation_contract_items(&db, 2000, 10).await.unwrap();
      assert_eq!(exchange_items.len(), 1);
      assert!(
        finance::corporation_contract_bids(&db, 2000, 10)
          .await
          .unwrap()
          .is_empty(),
        "a non-auction contract must not fetch bids"
      );
    }

    #[tokio::test]
    async fn it_keeps_a_stored_contract_the_new_response_omits() {
      let server = MockServer::start().await;
      mount_json(
        &server,
        "/corporations/2000/contracts/",
        serde_json::json!([
          { "contract_id": 20, "type": "item_exchange", "status": "outstanding", "issuer_id": 1001,
            "date_issued": "2024-03-01T00:00:00Z", "for_corporation": true },
        ]),
      )
      .await;
      mount_json(&server, "/corporations/2000/contracts/20/items/", serde_json::json!([])).await;
      mount_names(
        &server,
        serde_json::json!([{ "category": "character", "id": 1001, "name": "Issuer Pilot" }]),
      )
      .await;
      let db = store::open_test().await.unwrap();
      seed_corporation(&db, 2000).await;
      finance::upsert_for_corporation(
        &db,
        2000,
        &[CorporationContract {
          acceptor_id: None,
          acceptor_name: None,
          assignee_id: None,
          assignee_name: None,
          availability: None,
          collateral: None,
          contract_id: 999,
          corporation_id: 2000,
          date_accepted: None,
          date_completed: None,
          date_expired: None,
          date_issued: "2023-01-01T00:00:00Z".to_owned(),
          days_to_complete: None,
          end_location_id: None,
          for_corporation: true,
          issuer_corporation_id: None,
          issuer_id: 1001,
          issuer_name: Some("Old Pilot".to_owned()),
          price: None,
          reward: None,
          start_location_id: None,
          status: "finished".to_owned(),
          title: None,
          r#type: "item_exchange".to_owned(),
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
      let grant = Grant::new_test("corp-token", 2000);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, Some(&grant), 2000);

      run(&ctx).await.unwrap();

      let stored = finance::corporation_contracts(&db, 2000).await.unwrap();
      let mut ids: Vec<i64> = stored.iter().map(|contract| contract.contract_id()).collect();
      ids.sort_unstable();

      assert_eq!(ids, vec![20, 999]);
    }

    #[tokio::test]
    async fn it_updates_a_stored_contract_the_new_response_carries() {
      let server = MockServer::start().await;
      mount_json(
        &server,
        "/corporations/2000/contracts/",
        serde_json::json!([
          { "contract_id": 999, "type": "item_exchange", "status": "finished", "issuer_id": 1001,
            "date_issued": "2023-01-01T00:00:00Z", "date_completed": "2024-03-05T00:00:00Z",
            "acceptor_id": 1002, "for_corporation": true },
        ]),
      )
      .await;
      mount_json(
        &server,
        "/corporations/2000/contracts/999/items/",
        serde_json::json!([]),
      )
      .await;
      mount_names(
        &server,
        serde_json::json!([
          { "category": "character", "id": 1001, "name": "Issuer Pilot" },
          { "category": "character", "id": 1002, "name": "Acceptor Pilot" },
        ]),
      )
      .await;
      let db = store::open_test().await.unwrap();
      seed_corporation(&db, 2000).await;
      finance::upsert_for_corporation(
        &db,
        2000,
        &[CorporationContract {
          acceptor_id: None,
          acceptor_name: None,
          assignee_id: None,
          assignee_name: None,
          availability: None,
          collateral: None,
          contract_id: 999,
          corporation_id: 2000,
          date_accepted: None,
          date_completed: None,
          date_expired: None,
          date_issued: "2023-01-01T00:00:00Z".to_owned(),
          days_to_complete: None,
          end_location_id: None,
          for_corporation: true,
          issuer_corporation_id: None,
          issuer_id: 1001,
          issuer_name: Some("Issuer Pilot".to_owned()),
          price: None,
          reward: None,
          start_location_id: None,
          status: "outstanding".to_owned(),
          title: None,
          r#type: "item_exchange".to_owned(),
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
      let grant = Grant::new_test("corp-token", 2000);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, Some(&grant), 2000);

      run(&ctx).await.unwrap();

      let stored = finance::corporation_contracts(&db, 2000).await.unwrap();
      assert_eq!(stored.len(), 1);
      assert_eq!(stored[0].status(), "finished");
      assert_eq!(stored[0].acceptor_id(), Some(1002));
      assert_eq!(stored[0].date_completed().as_deref(), Some("2024-03-05T00:00:00Z"));
    }

    #[tokio::test]
    async fn it_skips_detail_fetches_for_already_persisted_contracts() {
      let server = MockServer::start().await;
      mount_json(
        &server,
        "/corporations/2000/contracts/",
        serde_json::json!([
          { "contract_id": 30, "type": "item_exchange", "status": "outstanding", "issuer_id": 1001,
            "date_issued": "2024-04-01T00:00:00Z", "for_corporation": true },
        ]),
      )
      .await;
      Mock::given(method("GET"))
        .and(path("/corporations/2000/contracts/30/items/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .expect(0)
        .mount(&server)
        .await;
      mount_names(
        &server,
        serde_json::json!([{ "category": "character", "id": 1001, "name": "Issuer Pilot" }]),
      )
      .await;
      let db = store::open_test().await.unwrap();
      seed_corporation(&db, 2000).await;
      finance::upsert_for_corporation(
        &db,
        2000,
        &[CorporationContract {
          acceptor_id: None,
          acceptor_name: None,
          assignee_id: None,
          assignee_name: None,
          availability: None,
          collateral: None,
          contract_id: 30,
          corporation_id: 2000,
          date_accepted: None,
          date_completed: None,
          date_expired: None,
          date_issued: "2024-04-01T00:00:00Z".to_owned(),
          days_to_complete: None,
          end_location_id: None,
          for_corporation: true,
          issuer_corporation_id: None,
          issuer_id: 1001,
          issuer_name: Some("Issuer Pilot".to_owned()),
          price: None,
          reward: None,
          start_location_id: None,
          status: "outstanding".to_owned(),
          title: None,
          r#type: "item_exchange".to_owned(),
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
      let grant = Grant::new_test("corp-token", 2000);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, Some(&grant), 2000);

      run(&ctx).await.unwrap();

      assert_eq!(finance::corporation_contracts(&db, 2000).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn it_skips_without_fetching_when_the_corporation_is_not_yet_persisted() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/corporations/2000/contracts/"))
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
      let grant = Grant::new_test("corp-token", 2000);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, Some(&grant), 2000);

      let result = run(&ctx).await;

      assert!(
        matches!(result, Err(Error::NotReady)),
        "a missing parent row must surface NotReady for a short token-free retry, not a clean Ok"
      );
      assert!(finance::corporation_contracts(&db, 2000).await.unwrap().is_empty());
    }
  }
}
