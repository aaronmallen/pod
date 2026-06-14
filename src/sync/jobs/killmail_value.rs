use std::collections::HashMap;

use crate::{
  clients::{self, esi::models::killmail::Item, zkillboard},
  store::model::MarketPrice,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ItemValue {
  pub destroyed: f64,
  pub dropped: f64,
  /// Full stack value for this line (destroyed + dropped), not destroyed-only.
  pub value_isk: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LocalValue {
  pub destroyed: f64,
  pub destroyed_and_dropped: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PriceTable {
  prices: HashMap<i64, f64>,
}

impl PriceTable {
  pub fn from_market_prices(prices: &[MarketPrice]) -> Self {
    Self {
      prices: prices
        .iter()
        .map(|price| {
          (
            price.type_id(),
            price.adjusted_price().or(price.average_price()).unwrap_or(0.0),
          )
        })
        .collect(),
    }
  }

  pub fn unit_price(&self, type_id: i64) -> f64 {
    self.prices.get(&type_id).copied().unwrap_or(0.0)
  }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Resolution {
  pub destroyed: f64,
  pub source: ValueSource,
  pub value: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValueSource {
  Local,
  Zkill,
}

impl ValueSource {
  pub fn as_str(self) -> &'static str {
    match self {
      ValueSource::Local => "local",
      ValueSource::Zkill => "zkill",
    }
  }
}

/// Prices each item at its unit price and returns one [`ItemValue`] per input item in slice order.
///
/// The ordering guarantee is load-bearing: callers pair each result with the stored killmail item
/// by index. Summing `value_isk` across all items and adding the hull price reconciles with
/// [`LocalValue::destroyed_and_dropped`].
pub fn item_values(items: &[Item], prices: &PriceTable) -> Vec<ItemValue> {
  items
    .iter()
    .map(|item| {
      let unit = prices.unit_price(item.type_id);
      let destroyed = unit * item.quantity_destroyed.unwrap_or(0).max(0) as f64;
      let dropped = unit * item.quantity_dropped.unwrap_or(0).max(0) as f64;
      ItemValue {
        destroyed,
        dropped,
        value_isk: destroyed + dropped,
      }
    })
    .collect()
}

/// Computes destroyed-only and destroyed+dropped totals using the same item basis as zKill's
/// `totalValue` (hull + destroyed + dropped), so that replacing the local fallback with a zKill
/// value causes no visible discontinuity in the displayed figure.
pub fn local_value(items: &[Item], ship_type_id: i64, prices: &PriceTable) -> LocalValue {
  let hull = prices.unit_price(ship_type_id);

  let mut destroyed = hull;
  let mut dropped = 0.0;
  for value in item_values(items, prices) {
    destroyed += value.destroyed;
    dropped += value.dropped;
  }

  LocalValue {
    destroyed,
    destroyed_and_dropped: destroyed + dropped,
  }
}

pub async fn resolve(
  zkill: &zkillboard::Client,
  killmail_id: i64,
  items: &[Item],
  ship_type_id: i64,
  prices: &PriceTable,
) -> Result<Resolution, clients::Error> {
  let local = local_value(items, ship_type_id, prices);
  match zkill.value_for_kill(killmail_id).await? {
    Some(value) => Ok(Resolution {
      destroyed: local.destroyed,
      source: ValueSource::Zkill,
      value,
    }),
    None => Ok(Resolution {
      destroyed: local.destroyed,
      source: ValueSource::Local,
      value: local.destroyed_and_dropped,
    }),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn item(type_id: i64, quantity_destroyed: Option<i64>, quantity_dropped: Option<i64>) -> Item {
    Item {
      flag: 0,
      items: Vec::new(),
      quantity_destroyed,
      quantity_dropped,
      type_id,
    }
  }

  fn prices() -> PriceTable {
    PriceTable::from_market_prices(&[
      MarketPrice {
        adjusted_price: Some(1_000.0),
        average_price: Some(900.0),
        type_id: 587,
      },
      MarketPrice {
        adjusted_price: None,
        average_price: Some(50.0),
        type_id: 34,
      },
      MarketPrice {
        adjusted_price: Some(200.0),
        average_price: Some(150.0),
        type_id: 2488,
      },
    ])
  }

  mod item_values {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_emits_one_value_per_item_in_slice_order() {
      let items = [item(34, Some(3), None), item(2488, None, Some(1))];

      let values = super::item_values(&items, &prices());

      assert_eq!(
        values,
        vec![
          ItemValue {
            destroyed: 150.0,
            dropped: 0.0,
            value_isk: 150.0,
          },
          ItemValue {
            destroyed: 0.0,
            dropped: 200.0,
            value_isk: 200.0,
          },
        ]
      );
    }

    #[test]
    fn it_prices_a_stack_destroyed_and_dropped_from_the_same_unit_price() {
      let items = [item(34, Some(3), Some(2))];

      let values = super::item_values(&items, &prices());

      assert_eq!(
        values[0],
        ItemValue {
          destroyed: 150.0,
          dropped: 100.0,
          value_isk: 250.0,
        }
      );
    }

    #[test]
    fn it_reconciles_the_per_item_sum_plus_hull_with_the_total() {
      let items = [item(34, Some(3), None), item(2488, None, Some(1))];
      let prices = prices();

      let per_item: f64 = super::item_values(&items, &prices)
        .iter()
        .map(|value| value.value_isk)
        .sum();
      let hull = prices.unit_price(587);
      let total = super::local_value(&items, 587, &prices);

      assert_eq!(per_item + hull, total.destroyed_and_dropped);
    }

    #[test]
    fn it_treats_an_unpriced_type_as_zero() {
      let items = [item(77, Some(5), Some(2))];

      let values = super::item_values(&items, &prices());

      assert_eq!(
        values[0],
        ItemValue {
          destroyed: 0.0,
          dropped: 0.0,
          value_isk: 0.0,
        }
      );
    }
  }

  mod local_value {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_adds_dropped_items_on_top_of_the_destroyed_basis() {
      let items = [item(34, Some(3), None), item(2488, None, Some(1))];

      let value = super::local_value(&items, 587, &prices());

      assert_eq!(value.destroyed_and_dropped, 1_350.0);
    }

    #[test]
    fn it_falls_back_to_the_average_price_when_no_adjusted_price_exists() {
      let items = [item(34, Some(2), None)];

      let value = super::local_value(&items, 999, &prices());

      assert_eq!(value.destroyed, 100.0);
    }

    #[test]
    fn it_prices_the_hull_and_destroyed_items_into_the_destroyed_only_figure() {
      let items = [item(34, Some(3), None), item(2488, None, Some(1))];

      let value = super::local_value(&items, 587, &prices());

      assert_eq!(value.destroyed, 1_150.0);
    }

    #[test]
    fn it_treats_an_unpriced_type_as_zero() {
      let items = [item(77, Some(5), None)];

      let value = super::local_value(&items, 999, &prices());

      assert_eq!(value.destroyed_and_dropped, 0.0);
    }
  }

  mod resolve {
    use pretty_assertions::assert_eq;
    use wiremock::{
      Mock, MockServer, ResponseTemplate,
      matchers::{method, path},
    };

    use super::*;
    use crate::{clients::http, store};

    async fn make_http() -> std::sync::Arc<http::Client> {
      let db = store::open_test().await.unwrap();
      http::Client::builder(http::Cache::new(db)).build()
    }

    #[tokio::test]
    async fn it_falls_back_to_the_local_destroyed_and_dropped_value_when_absent() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/killID/500/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw("[]", "application/json"))
        .mount(&server)
        .await;
      let zkill = zkillboard::Client::with_base_url(make_http().await, server.uri());
      let items = [item(34, Some(3), None), item(2488, None, Some(1))];

      let resolution = super::resolve(&zkill, 500, &items, 587, &prices()).await.unwrap();

      assert_eq!(resolution.source, ValueSource::Local);
      assert_eq!(resolution.value, 1_350.0);
      assert_eq!(resolution.destroyed, 1_150.0);
    }

    #[tokio::test]
    async fn it_returns_the_zkill_value_when_the_kill_is_present() {
      let server = MockServer::start().await;
      let body = r#"[{"killmail_id": 500, "zkb": {"hash": "abc123", "totalValue": 4242.5}}]"#;
      Mock::given(method("GET"))
        .and(path("/killID/500/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
        .mount(&server)
        .await;
      let zkill = zkillboard::Client::with_base_url(make_http().await, server.uri());
      let items = [item(34, Some(3), None), item(2488, None, Some(1))];

      let resolution = super::resolve(&zkill, 500, &items, 587, &prices()).await.unwrap();

      assert_eq!(resolution.source, ValueSource::Zkill);
      assert_eq!(resolution.value, 4242.5);
      assert_eq!(resolution.destroyed, 1_150.0);
    }
  }
}
