//! Market order endpoints — lowest Jita sell price lookup.

use serde::Deserialize;

use crate::{Error, clients::markets::Client};

/// Minimal ESI market order fields required for price extraction.
#[derive(Debug, Deserialize)]
struct OrderRow {
  is_buy_order: bool,
  price: f64,
}

impl Client<'_> {
  /// Returns the lowest sell-order price for `type_id` in Jita (region 10000002),
  /// or `None` when no sell orders exist.
  pub async fn lowest_jita_sell(&self, type_id: i32) -> Result<Option<f64>, Error> {
    let url = self
      .esi
      .url_builder()
      .path("v1/markets/10000002/orders/")
      .param("type_id", type_id.to_string())
      .param("order_type", "sell")
      .param("page", "1")
      .build();

    let orders: Vec<OrderRow> = self.esi.http().get_json(&url, None).await?;

    let min = orders
      .into_iter()
      .filter(|o| !o.is_buy_order)
      .map(|o| o.price)
      .reduce(f64::min);

    Ok(min)
  }
}

#[cfg(test)]
mod tests {
  use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
  };

  fn make_esi(server_uri: &str) -> crate::Client {
    crate::Client::builder("test-client")
      .base_url(server_uri)
      .build()
      .unwrap()
  }

  mod lowest_jita_sell {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_lowest_sell_price_from_sell_orders() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v1/markets/10000002/orders/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
          {"is_buy_order": false, "price": 500.0},
          {"is_buy_order": false, "price": 450.0},
          {"is_buy_order": true,  "price": 400.0}
        ])))
        .mount(&server)
        .await;

      let esi = make_esi(&server.uri());
      let result = esi.markets().lowest_jita_sell(35i32).await.unwrap();

      assert_eq!(result, Some(450.0f64));
    }

    #[tokio::test]
    async fn it_returns_none_when_no_sell_orders_exist() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v1/markets/10000002/orders/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&server)
        .await;

      let esi = make_esi(&server.uri());
      let result = esi.markets().lowest_jita_sell(35i32).await.unwrap();

      assert_eq!(result, None);
    }

    #[tokio::test]
    async fn it_returns_error_on_400() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v1/markets/10000002/orders/"))
        .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({"error": "Bad request"})))
        .mount(&server)
        .await;

      let esi = make_esi(&server.uri());
      let result = esi.markets().lowest_jita_sell(35i32).await;

      assert!(result.is_err());
    }
  }
}
