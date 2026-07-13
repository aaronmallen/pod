use crate::clients::esi::models::market::RegionOrder;

#[derive(Clone, Debug, PartialEq)]
pub struct BookRow {
  pub order_id: i64,
  pub location_id: i64,
  pub system_id: i64,
  pub price: f64,
  pub volume_remain: i64,
  pub min_volume: i64,
  pub range: String,
  pub duration: i64,
  pub issued: String,
  pub is_buy_order: bool,
}

impl BookRow {
  fn from_order(order: RegionOrder) -> Self {
    Self {
      order_id: order.order_id,
      location_id: order.location_id,
      system_id: order.system_id,
      price: order.price,
      volume_remain: order.volume_remain,
      min_volume: order.min_volume,
      range: order.range,
      duration: order.duration,
      issued: order.issued,
      is_buy_order: order.is_buy_order,
    }
  }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct OrderBook {
  pub sell: Vec<BookRow>,
  pub buy: Vec<BookRow>,
  pub best_sell: Option<f64>,
  pub best_buy: Option<f64>,
  pub spread_pct: Option<f64>,
}

pub fn build_order_book(orders: Vec<RegionOrder>) -> OrderBook {
  let mut sell = Vec::new();
  let mut buy = Vec::new();
  for order in orders {
    let row = BookRow::from_order(order);
    if row.is_buy_order {
      buy.push(row);
    } else {
      sell.push(row);
    }
  }

  sell.sort_by(|a, b| a.price.total_cmp(&b.price));
  buy.sort_by(|a, b| b.price.total_cmp(&a.price));

  let best_sell = sell.first().map(|row| row.price);
  let best_buy = buy.first().map(|row| row.price);
  let spread_pct = match (best_sell, best_buy) {
    (Some(best_sell), Some(best_buy)) if best_sell > 0.0 => Some((best_sell - best_buy) / best_sell * 100.0),
    _ => None,
  };

  OrderBook {
    sell,
    buy,
    best_sell,
    best_buy,
    spread_pct,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn order(is_buy_order: bool, price: f64) -> RegionOrder {
    RegionOrder {
      is_buy_order,
      price,
      type_id: 34,
      ..Default::default()
    }
  }

  mod build_order_book {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_splits_orders_by_side_and_sorts_best_first() {
      let book = build_order_book(vec![
        order(false, 8.0),
        order(false, 5.5),
        order(false, 6.5),
        order(true, 4.0),
        order(true, 5.0),
        order(true, 3.0),
      ]);

      assert_eq!(
        book.sell.iter().map(|row| row.price).collect::<Vec<_>>(),
        vec![5.5, 6.5, 8.0]
      );
      assert_eq!(
        book.buy.iter().map(|row| row.price).collect::<Vec<_>>(),
        vec![5.0, 4.0, 3.0]
      );
    }

    #[test]
    fn it_reports_best_sell_best_buy_and_spread() {
      let book = build_order_book(vec![
        order(false, 5.0),
        order(false, 8.0),
        order(true, 4.0),
        order(true, 2.0),
      ]);

      assert_eq!(book.best_sell, Some(5.0));
      assert_eq!(book.best_buy, Some(4.0));
      assert_eq!(book.spread_pct, Some((5.0 - 4.0) / 5.0 * 100.0));
    }

    #[test]
    fn it_leaves_prices_and_spread_none_when_a_side_is_empty() {
      let sell_only = build_order_book(vec![order(false, 5.0)]);
      assert_eq!(sell_only.best_sell, Some(5.0));
      assert_eq!(sell_only.best_buy, None);
      assert_eq!(sell_only.spread_pct, None);

      let empty = build_order_book(vec![]);
      assert_eq!(empty.best_sell, None);
      assert_eq!(empty.best_buy, None);
      assert_eq!(empty.spread_pct, None);
    }

    #[test]
    fn it_carries_the_book_column_fields_onto_rows() {
      let order = RegionOrder {
        duration: 90,
        is_buy_order: false,
        issued: "2026-07-13T12:00:00Z".to_owned(),
        location_id: 60_003_760,
        min_volume: 10,
        order_id: 6001,
        price: 5.5,
        range: "station".to_owned(),
        system_id: 30_000_142,
        type_id: 34,
        volume_remain: 1200,
      };

      let book = build_order_book(vec![order]);
      let row = &book.sell[0];

      assert_eq!(row.order_id, 6001);
      assert_eq!(row.location_id, 60_003_760);
      assert_eq!(row.system_id, 30_000_142);
      assert_eq!(row.volume_remain, 1200);
      assert_eq!(row.min_volume, 10);
      assert_eq!(row.range, "station");
      assert_eq!(row.duration, 90);
      assert_eq!(row.issued, "2026-07-13T12:00:00Z");
    }
  }
}
