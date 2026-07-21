use std::{
  collections::{HashMap, HashSet},
  sync::Arc,
};

use crate::{
  clients::{esi, esi::models::market::RegionOrder, eve_sso},
  services::location_search::{LocationTier, first_owned_grant},
  store::{Database, repo::sde},
};

pub type BestSellPrices = HashMap<i64, Option<f64>>;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct MarketScope {
  pub region_id: Option<i64>,
  pub scope_id: i64,
  pub tier: LocationTier,
}

#[allow(dead_code)]
impl MarketScope {
  pub fn new(scope_id: i64, region_id: Option<i64>) -> Self {
    Self {
      region_id,
      scope_id,
      tier: LocationTier::from_id(scope_id).unwrap_or(LocationTier::Region),
    }
  }
}

enum ScopeFilter {
  All,
  Station(i64),
  System(i64),
  Systems(HashSet<i64>),
}

impl ScopeFilter {
  fn matches(&self, order: &RegionOrder) -> bool {
    match self {
      Self::All => true,
      Self::Station(location_id) => order.location_id == *location_id,
      Self::System(system_id) => order.system_id == *system_id,
      Self::Systems(system_ids) => system_ids.contains(&order.system_id),
    }
  }
}

#[allow(dead_code)]
pub async fn resolve_best_sell(
  db: Database,
  esi: Arc<esi::Client>,
  sso: Arc<eve_sso::Client>,
  scope: MarketScope,
  type_ids: Vec<i64>,
) -> BestSellPrices {
  let ids = dedup(type_ids);
  match scope.tier {
    LocationTier::Structure => structure_prices(&db, &esi, &sso, scope.scope_id, &ids).await,
    _ => region_prices(&db, &esi, &scope, &ids).await,
  }
}

async fn constellation_systems(db: &Database, constellation_id: i64) -> HashSet<i64> {
  sde::all_solar_systems(db)
    .await
    .unwrap_or_default()
    .into_iter()
    .filter(|system| system.constellation_id() == constellation_id)
    .map(|system| system.id())
    .collect()
}

fn dedup(mut type_ids: Vec<i64>) -> Vec<i64> {
  type_ids.sort_unstable();
  type_ids.dedup();
  type_ids
}

fn lowest_sell<'a>(orders: impl IntoIterator<Item = &'a RegionOrder>) -> Option<f64> {
  orders
    .into_iter()
    .filter(|order| !order.is_buy_order)
    .map(|order| order.price)
    .min_by(f64::total_cmp)
}

async fn region_prices(db: &Database, esi: &esi::Client, scope: &MarketScope, type_ids: &[i64]) -> BestSellPrices {
  let Some(region_id) = scope.region_id else {
    return unresolved(type_ids);
  };
  let filter = scope_filter(db, scope).await;
  let mut prices = BestSellPrices::new();
  for &type_id in type_ids {
    prices.insert(type_id, region_type_price(esi, region_id, type_id, &filter).await);
  }
  prices
}

async fn region_type_price(esi: &esi::Client, region_id: i64, type_id: i64, filter: &ScopeFilter) -> Option<f64> {
  match esi.market().sell_orders(region_id, type_id).await {
    Ok(orders) => lowest_sell(orders.iter().filter(|order| filter.matches(order))),
    Err(error) => {
      tracing::warn!(target: "pod::market_prices", %error, region_id, type_id, "sell order fetch failed");
      None
    }
  }
}

fn price_types_from_book(orders: &[RegionOrder], type_ids: &[i64]) -> BestSellPrices {
  type_ids
    .iter()
    .map(|&type_id| {
      (
        type_id,
        lowest_sell(orders.iter().filter(|order| order.type_id == type_id)),
      )
    })
    .collect()
}

async fn scope_filter(db: &Database, scope: &MarketScope) -> ScopeFilter {
  match scope.tier {
    LocationTier::Constellation => ScopeFilter::Systems(constellation_systems(db, scope.scope_id).await),
    LocationTier::Station => ScopeFilter::Station(scope.scope_id),
    LocationTier::System => ScopeFilter::System(scope.scope_id),
    _ => ScopeFilter::All,
  }
}

async fn structure_prices(
  db: &Database,
  esi: &esi::Client,
  sso: &eve_sso::Client,
  structure_id: i64,
  type_ids: &[i64],
) -> BestSellPrices {
  let Some(grant) = first_owned_grant(db, sso).await else {
    return unresolved(type_ids);
  };
  match esi.market().structure_orders(structure_id, &grant).await {
    Ok(orders) => price_types_from_book(&orders, type_ids),
    Err(error) => {
      tracing::warn!(target: "pod::market_prices", %error, structure_id, "structure order fetch failed");
      unresolved(type_ids)
    }
  }
}

fn unresolved(type_ids: &[i64]) -> BestSellPrices {
  type_ids.iter().map(|&type_id| (type_id, None)).collect()
}

#[cfg(test)]
mod tests {
  use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path, query_param},
  };

  use super::*;
  use crate::{clients::http, store};

  const REGION_ID: i64 = 10_000_002;

  async fn make_esi(base_url: &str, db: &store::Database) -> esi::Client {
    let cache = http::Cache::new(db.clone());
    let http = http::Client::builder(cache).build();
    esi::Client::with_base_url(http, base_url)
  }

  async fn make_sso(db: &store::Database) -> eve_sso::Client {
    let http = http::Client::builder(http::Cache::new(db.clone())).build();
    eve_sso::Client::new(http, "test-client")
  }

  async fn mount_sell_orders(server: &MockServer, type_id: i64, body: &str) {
    Mock::given(method("GET"))
      .and(path(format!("/markets/{REGION_ID}/orders/")))
      .and(query_param("order_type", "sell"))
      .and(query_param("type_id", type_id.to_string()))
      .respond_with(
        ResponseTemplate::new(200)
          .insert_header("X-Pages", "1")
          .set_body_raw(body.to_owned(), "application/json"),
      )
      .expect(1)
      .mount(server)
      .await;
  }

  async fn resolve(
    db: &store::Database,
    server: &MockServer,
    scope: MarketScope,
    type_ids: Vec<i64>,
  ) -> BestSellPrices {
    let esi = Arc::new(make_esi(&server.uri(), db).await);
    let sso = Arc::new(make_sso(db).await);
    resolve_best_sell(db.clone(), esi, sso, scope, type_ids).await
  }

  mod new {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_derives_the_tier_from_the_scope_id() {
      assert_eq!(
        MarketScope::new(60_003_760, Some(REGION_ID)).tier,
        LocationTier::Station
      );
      assert_eq!(MarketScope::new(1_035_466_617_946, None).tier, LocationTier::Structure);
      assert_eq!(MarketScope::new(30_000_142, Some(REGION_ID)).tier, LocationTier::System);
    }

    #[test]
    fn it_falls_back_to_region_for_an_unrecognized_id() {
      assert_eq!(MarketScope::new(5, Some(REGION_ID)).tier, LocationTier::Region);
    }
  }

  mod matches {
    use super::*;

    fn order(location_id: i64, system_id: i64) -> RegionOrder {
      RegionOrder {
        location_id,
        system_id,
        price: 1.0,
        type_id: 34,
        ..Default::default()
      }
    }

    #[test]
    fn it_scopes_orders_by_station_system_or_member_systems() {
      let at_station = order(60_003_760, 30_000_142);
      let elsewhere = order(60_000_004, 30_000_001);

      assert!(ScopeFilter::All.matches(&elsewhere));
      assert!(ScopeFilter::Station(60_003_760).matches(&at_station));
      assert!(!ScopeFilter::Station(60_003_760).matches(&elsewhere));
      assert!(ScopeFilter::System(30_000_142).matches(&at_station));
      assert!(!ScopeFilter::System(30_000_142).matches(&elsewhere));
      assert!(ScopeFilter::Systems(HashSet::from([30_000_142])).matches(&at_station));
      assert!(!ScopeFilter::Systems(HashSet::from([30_000_142])).matches(&elsewhere));
    }
  }

  mod resolve_best_sell {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_resolves_the_region_wide_lowest_sell_per_type() {
      let server = MockServer::start().await;
      mount_sell_orders(
        &server,
        34,
        r#"[{"is_buy_order":false,"location_id":60003760,"price":8.0,"type_id":34},{"is_buy_order":false,"location_id":60000004,"price":6.5,"type_id":34}]"#,
      )
      .await;
      mount_sell_orders(&server, 35, "[]").await;
      let db = store::open_test().await.unwrap();

      let prices = resolve(&db, &server, MarketScope::new(REGION_ID, Some(REGION_ID)), vec![34, 35]).await;

      assert_eq!(prices.get(&34), Some(&Some(6.5)));
      assert_eq!(prices.get(&35), Some(&None));
    }

    #[tokio::test]
    async fn it_narrows_a_station_scope_to_orders_at_that_station() {
      let server = MockServer::start().await;
      mount_sell_orders(
        &server,
        34,
        r#"[{"is_buy_order":false,"location_id":60003760,"price":8.0,"type_id":34},{"is_buy_order":false,"location_id":60000004,"price":6.5,"type_id":34}]"#,
      )
      .await;
      let db = store::open_test().await.unwrap();

      let prices = resolve(&db, &server, MarketScope::new(60_003_760, Some(REGION_ID)), vec![34]).await;

      assert_eq!(prices.get(&34), Some(&Some(8.0)));
    }

    #[tokio::test]
    async fn it_narrows_a_system_scope_by_system_id() {
      let server = MockServer::start().await;
      mount_sell_orders(
        &server,
        34,
        r#"[{"is_buy_order":false,"location_id":60003760,"price":8.0,"system_id":30000142,"type_id":34},{"is_buy_order":false,"location_id":60000004,"price":6.5,"system_id":30000001,"type_id":34}]"#,
      )
      .await;
      let db = store::open_test().await.unwrap();

      let prices = resolve(&db, &server, MarketScope::new(30_000_142, Some(REGION_ID)), vec![34]).await;

      assert_eq!(prices.get(&34), Some(&Some(8.0)));
    }

    #[tokio::test]
    async fn it_narrows_a_constellation_scope_to_member_systems() {
      let server = MockServer::start().await;
      mount_sell_orders(
        &server,
        34,
        r#"[{"is_buy_order":false,"location_id":60003760,"price":8.0,"system_id":30000142,"type_id":34},{"is_buy_order":false,"location_id":60000004,"price":6.5,"system_id":30000001,"type_id":34}]"#,
      )
      .await;
      let db = store::open_test().await.unwrap();
      seed_system(&db, 30_000_142, 20_000_020).await;
      seed_system(&db, 30_000_001, 20_000_001).await;

      let prices = resolve(&db, &server, MarketScope::new(20_000_020, Some(REGION_ID)), vec![34]).await;

      assert_eq!(prices.get(&34), Some(&Some(8.0)));
    }

    #[tokio::test]
    async fn it_ignores_buy_orders() {
      let server = MockServer::start().await;
      mount_sell_orders(
        &server,
        34,
        r#"[{"is_buy_order":true,"location_id":60003760,"price":0.5,"type_id":34},{"is_buy_order":false,"location_id":60003760,"price":8.0,"type_id":34}]"#,
      )
      .await;
      let db = store::open_test().await.unwrap();

      let prices = resolve(&db, &server, MarketScope::new(REGION_ID, Some(REGION_ID)), vec![34]).await;

      assert_eq!(prices.get(&34), Some(&Some(8.0)));
    }

    #[tokio::test]
    async fn it_continues_past_a_failing_type_fetch() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path(format!("/markets/{REGION_ID}/orders/")))
        .and(query_param("type_id", "34"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
      mount_sell_orders(
        &server,
        35,
        r#"[{"is_buy_order":false,"location_id":60003760,"price":11.0,"type_id":35}]"#,
      )
      .await;
      let db = store::open_test().await.unwrap();

      let prices = resolve(&db, &server, MarketScope::new(REGION_ID, Some(REGION_ID)), vec![34, 35]).await;

      assert_eq!(prices.get(&34), Some(&None));
      assert_eq!(prices.get(&35), Some(&Some(11.0)));
    }

    #[tokio::test]
    async fn it_fetches_each_requested_type_once_despite_duplicates() {
      let server = MockServer::start().await;
      mount_sell_orders(
        &server,
        34,
        r#"[{"is_buy_order":false,"location_id":60003760,"price":8.0,"type_id":34}]"#,
      )
      .await;
      let db = store::open_test().await.unwrap();

      let prices = resolve(
        &db,
        &server,
        MarketScope::new(REGION_ID, Some(REGION_ID)),
        vec![34, 34, 34],
      )
      .await;

      assert_eq!(prices.len(), 1);
      assert_eq!(prices.get(&34), Some(&Some(8.0)));
    }

    #[tokio::test]
    async fn it_marks_every_type_unresolved_for_a_structure_without_a_grant() {
      let server = MockServer::start().await;
      let db = store::open_test().await.unwrap();

      let prices = resolve(&db, &server, MarketScope::new(1_035_466_617_946, None), vec![34, 35]).await;

      assert_eq!(prices.get(&34), Some(&None));
      assert_eq!(prices.get(&35), Some(&None));
    }

    async fn seed_system(db: &store::Database, id: i64, constellation_id: i64) {
      sde::upsert_region(
        db,
        &crate::store::model::Region {
          description: None,
          id: REGION_ID,
          name: "Test Region".to_owned(),
        },
      )
      .await
      .unwrap();
      sde::upsert_constellation(
        db,
        &crate::store::model::Constellation {
          id: constellation_id,
          name: format!("Constellation {constellation_id}"),
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
        &crate::store::model::SolarSystem {
          constellation_id,
          id,
          name: format!("System {id}"),
          position_x: 0.0,
          position_y: 0.0,
          position_z: 0.0,
          security_class: None,
          security_status: 1.0,
          star_id: None,
        },
      )
      .await
      .unwrap();
    }
  }

  mod price_types_from_book {
    use pretty_assertions::assert_eq;

    use super::*;

    fn order(is_buy_order: bool, price: f64, type_id: i64) -> RegionOrder {
      RegionOrder {
        is_buy_order,
        location_id: 1_035_466_617_946,
        price,
        type_id,
        ..Default::default()
      }
    }

    #[test]
    fn it_prices_each_requested_type_from_a_single_book() {
      let orders = [order(false, 8.0, 34), order(false, 6.5, 34), order(true, 9.0, 35)];

      let prices = price_types_from_book(&orders, &[34, 35, 36]);

      assert_eq!(prices.get(&34), Some(&Some(6.5)));
      assert_eq!(prices.get(&35), Some(&None), "buy orders never price a type");
      assert_eq!(prices.get(&36), Some(&None), "an unlisted type resolves to no price");
    }
  }

  mod unresolved {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_keys_every_requested_type_with_no_price() {
      let prices = unresolved(&[34, 35]);

      assert_eq!(prices.len(), 2);
      assert_eq!(prices.get(&34), Some(&None));
      assert_eq!(prices.get(&35), Some(&None));
    }
  }
}
