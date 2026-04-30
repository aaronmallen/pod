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
