use crate::clients::{
  self,
  esi::{
    Client as EsiClient,
    models::market::{MarketPrice, RegionOrder},
  },
};

pub struct Client<'a> {
  esi: &'a EsiClient,
}

impl<'a> Client<'a> {
  pub fn new(esi: &'a EsiClient) -> Self {
    Self {
      esi,
    }
  }

  pub async fn prices(&self) -> Result<Vec<MarketPrice>, clients::Error> {
    let url = self.esi.url("markets/prices/");
    self.esi.get_json(&url, None).await
  }

  pub async fn sell_orders(&self, region_id: i64, type_id: i64) -> Result<Vec<RegionOrder>, clients::Error> {
    let url = self.esi.url(&format!(
      "markets/{region_id}/orders/?order_type=sell&type_id={type_id}"
    ));
    self.esi.get_json_paginated(&url, None).await
  }

  pub async fn lowest_sell(
    &self,
    region_id: i64,
    type_id: i64,
    location_id: i64,
  ) -> Result<Option<f64>, clients::Error> {
    let orders = self.sell_orders(region_id, type_id).await?;
    Ok(lowest_sell_at(&orders, location_id))
  }
}

fn lowest_sell_at(orders: &[RegionOrder], location_id: i64) -> Option<f64> {
  orders
    .iter()
    .filter(|order| !order.is_buy_order && order.location_id == location_id)
    .map(|order| order.price)
    .min_by(|a, b| a.total_cmp(b))
}

#[cfg(test)]
mod tests {
  use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
  };

  use super::*;
  use crate::{clients::http, store};

  async fn make_esi(base_url: &str) -> EsiClient {
    let db = store::open_test().await.unwrap();
    let cache = http::Cache::new(db);
    let http = http::Client::builder(cache).build();
    EsiClient::with_base_url(http, base_url)
  }

  mod lowest_sell {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_none_when_the_type_has_no_station_sell_orders() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/markets/10000002/orders/"))
        .respond_with(
          ResponseTemplate::new(200)
            .insert_header("X-Pages", "1")
            .set_body_raw("[]", "application/json"),
        )
        .mount(&server)
        .await;
      let esi = make_esi(&server.uri()).await;

      let price = esi.market().lowest_sell(10_000_002, 34, 60_003_760).await.unwrap();

      assert_eq!(price, None);
    }

    #[tokio::test]
    async fn it_returns_the_lowest_jita_station_sell_price_across_pages() {
      let server = MockServer::start().await;
      let page_one = r#"[{"is_buy_order":false,"location_id":60003760,"price":8.0,"type_id":34},{"is_buy_order":true,"location_id":60003760,"price":1.0,"type_id":34}]"#;
      let page_two = r#"[{"is_buy_order":false,"location_id":60003760,"price":6.5,"type_id":34},{"is_buy_order":false,"location_id":99,"price":0.1,"type_id":34}]"#;
      Mock::given(method("GET"))
        .and(path("/markets/10000002/orders/"))
        .and(wiremock::matchers::query_param("page", "1"))
        .respond_with(
          ResponseTemplate::new(200)
            .insert_header("X-Pages", "2")
            .set_body_raw(page_one, "application/json"),
        )
        .mount(&server)
        .await;
      Mock::given(method("GET"))
        .and(path("/markets/10000002/orders/"))
        .and(wiremock::matchers::query_param("page", "2"))
        .respond_with(
          ResponseTemplate::new(200)
            .insert_header("X-Pages", "2")
            .set_body_raw(page_two, "application/json"),
        )
        .mount(&server)
        .await;
      let esi = make_esi(&server.uri()).await;

      let price = esi.market().lowest_sell(10_000_002, 34, 60_003_760).await.unwrap();

      assert_eq!(price, Some(6.5));
    }
  }

  mod lowest_sell_at {
    use pretty_assertions::assert_eq;

    use super::*;

    fn order(is_buy_order: bool, location_id: i64, price: f64) -> RegionOrder {
      RegionOrder {
        is_buy_order,
        location_id,
        price,
      }
    }

    #[test]
    fn it_picks_the_minimum_sell_price_at_the_station_only() {
      let orders = [
        order(false, 60_003_760, 9.0),
        order(false, 60_003_760, 5.5),
        order(false, 999, 0.1),
        order(true, 60_003_760, 0.5),
      ];

      assert_eq!(lowest_sell_at(&orders, 60_003_760), Some(5.5));
    }

    #[test]
    fn it_returns_none_when_no_sell_order_is_at_the_station() {
      let orders = [order(false, 999, 5.0), order(true, 60_003_760, 1.0)];

      assert_eq!(lowest_sell_at(&orders, 60_003_760), None);
    }
  }

  mod prices {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_http_error_on_5xx() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/markets/prices/"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;
      let esi = make_esi(&server.uri()).await;

      let result = esi.market().prices().await;

      assert!(matches!(result, Err(clients::Error::Http(_))));
    }

    #[tokio::test]
    async fn it_returns_prices_with_independently_optional_fields() {
      let server = MockServer::start().await;
      let body = r#"[{"adjusted_price":5.5,"type_id":34},{"average_price":6.25,"type_id":35},{"adjusted_price":7.0,"average_price":8.0,"type_id":36}]"#;
      Mock::given(method("GET"))
        .and(path("/markets/prices/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
        .mount(&server)
        .await;
      let esi = make_esi(&server.uri()).await;

      let prices = esi.market().prices().await.unwrap();

      assert_eq!(prices.len(), 3);
      assert_eq!(prices[0].type_id, 34);
      assert_eq!(prices[0].adjusted_price, Some(5.5));
      assert_eq!(prices[0].average_price, None);
      assert_eq!(prices[1].adjusted_price, None);
      assert_eq!(prices[1].average_price, Some(6.25));
      assert_eq!(prices[2].adjusted_price, Some(7.0));
      assert_eq!(prices[2].average_price, Some(8.0));
    }
  }
}
