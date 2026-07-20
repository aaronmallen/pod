// Foundation module consumed by later market sub-specs (structure outbid, alerting).
#![allow(dead_code)]

use crate::store::model::MarketOrder;

pub trait BookQuote {
  fn is_buy_order(&self) -> bool;
  fn location_id(&self) -> i64;
  fn price(&self) -> f64;
  fn type_id(&self) -> i64;
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quote {
  pub is_buy_order: bool,
  pub location_id: i64,
  pub price: f64,
  pub type_id: i64,
}

impl BookQuote for Quote {
  fn is_buy_order(&self) -> bool {
    self.is_buy_order
  }

  fn location_id(&self) -> i64 {
    self.location_id
  }

  fn price(&self) -> f64 {
    self.price
  }

  fn type_id(&self) -> i64 {
    self.type_id
  }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Annotation {
  pub best: Option<f64>,
  pub gap: Option<f64>,
  pub gap_pct: Option<f64>,
  pub outbid: bool,
}

pub fn annotate<Q: BookQuote>(order: &MarketOrder, book: &[Q]) -> Annotation {
  let best = best_competing_price(order, book);

  let Some(best) = best else {
    return Annotation::default();
  };

  let gap = if order.is_buy_order() {
    best - order.price()
  } else {
    order.price() - best
  };

  let outbid = gap > 0.0;

  let gap_pct = if order.price() == 0.0 {
    None
  } else {
    Some(gap / order.price() * 100.0)
  };

  Annotation {
    best: Some(best),
    gap: Some(gap),
    gap_pct,
    outbid,
  }
}

pub fn annotate_all<Q: BookQuote>(orders: &[MarketOrder], book: &[Q]) -> Vec<Annotation> {
  orders.iter().map(|order| annotate(order, book)).collect()
}

fn best_competing_price<Q: BookQuote>(order: &MarketOrder, book: &[Q]) -> Option<f64> {
  let mut self_skipped = false;
  let mut best: Option<f64> = None;

  for quote in book {
    if quote.location_id() != order.location_id()
      || quote.type_id() != order.type_id()
      || quote.is_buy_order() != order.is_buy_order()
    {
      continue;
    }

    if !self_skipped && quote.price() == order.price() {
      self_skipped = true;
      continue;
    }

    best = Some(match best {
      None => quote.price(),
      Some(current) => {
        if order.is_buy_order() {
          current.max(quote.price())
        } else {
          current.min(quote.price())
        }
      }
    });
  }

  best
}

#[cfg(test)]
mod tests {
  use pretty_assertions::assert_eq;

  use super::*;
  use crate::clients::esi::models::character::MarketOrder as EsiMarketOrder;

  fn order(is_buy_order: bool, location_id: i64, type_id: i64, price: f64) -> MarketOrder {
    MarketOrder::from((
      1,
      EsiMarketOrder {
        duration: 90,
        escrow: 0.0,
        is_buy_order,
        is_corporation: false,
        issued: "2026-07-13T00:00:00Z".to_owned(),
        location_id,
        min_volume: Some(1),
        order_id: 42,
        price,
        range: "station".to_owned(),
        region_id: 10_000_002,
        type_id,
        volume_remain: 100,
        volume_total: 100,
      },
    ))
  }

  fn quote(is_buy_order: bool, location_id: i64, type_id: i64, price: f64) -> Quote {
    Quote {
      is_buy_order,
      location_id,
      price,
      type_id,
    }
  }

  #[test]
  fn it_marks_a_sell_outbid_by_a_cheaper_competing_sell() {
    let mine = order(false, 60_003_760, 34, 100.0);
    let book = [
      quote(false, 60_003_760, 34, 100.0), // my own order in the public book
      quote(false, 60_003_760, 34, 90.0),  // competitor undercutting me
    ];

    let annotation = annotate(&mine, &book);

    assert!(annotation.outbid);
    assert_eq!(annotation.best, Some(90.0));
    assert_eq!(annotation.gap, Some(10.0));
    assert_eq!(annotation.gap_pct, Some(10.0));
  }

  #[test]
  fn it_marks_a_buy_outbid_by_a_higher_competing_buy() {
    let mine = order(true, 60_003_760, 34, 5.0);
    let book = [quote(true, 60_003_760, 34, 5.0), quote(true, 60_003_760, 34, 6.0)];

    let annotation = annotate(&mine, &book);

    assert!(annotation.outbid);
    assert_eq!(annotation.best, Some(6.0));
    assert_eq!(annotation.gap, Some(1.0));
    assert_eq!(annotation.gap_pct, Some(20.0));
  }

  #[test]
  fn it_does_not_mark_the_best_priced_sell_as_outbid() {
    let mine = order(false, 60_003_760, 34, 90.0);
    let book = [quote(false, 60_003_760, 34, 90.0), quote(false, 60_003_760, 34, 100.0)];

    let annotation = annotate(&mine, &book);

    assert!(!annotation.outbid);
    assert_eq!(annotation.best, Some(100.0));
    assert_eq!(annotation.gap, Some(-10.0));
  }

  #[test]
  fn it_ignores_a_cheaper_sell_at_a_different_station() {
    let mine = order(false, 60_003_760, 34, 100.0);
    let book = [quote(false, 60_008_494, 34, 90.0)];

    let annotation = annotate(&mine, &book);

    assert!(!annotation.outbid);
    assert_eq!(annotation.best, None);
    assert_eq!(annotation.gap, None);
    assert_eq!(annotation.gap_pct, None);
  }

  #[test]
  fn it_ignores_a_competitor_for_a_different_type() {
    let mine = order(false, 60_003_760, 34, 100.0);
    let book = [quote(false, 60_003_760, 35, 90.0)];

    assert!(!annotate(&mine, &book).outbid);
  }

  #[test]
  fn it_ignores_the_opposite_side_of_the_book() {
    let mine = order(false, 60_003_760, 34, 100.0);
    let book = [quote(true, 60_003_760, 34, 90.0)];

    let annotation = annotate(&mine, &book);

    assert!(!annotation.outbid);
    assert_eq!(annotation.best, None);
  }

  #[test]
  fn it_returns_a_neutral_annotation_for_an_empty_book() {
    let mine = order(false, 60_003_760, 34, 100.0);

    let annotation = annotate(&mine, &[] as &[Quote]);

    assert_eq!(annotation, Annotation::default());
    assert!(!annotation.outbid);
  }

  #[test]
  fn it_excludes_only_the_players_own_order_from_competitors() {
    let mine = order(false, 60_003_760, 34, 100.0);
    let book = [
      quote(false, 60_003_760, 34, 100.0), // mine
      quote(false, 60_003_760, 34, 100.0), // a genuine competitor at the same price
    ];

    let annotation = annotate(&mine, &book);

    assert_eq!(annotation.best, Some(100.0));
    assert!(!annotation.outbid);
    assert_eq!(annotation.gap, Some(0.0));
  }

  #[test]
  fn it_annotates_a_batch_of_orders_against_a_shared_book() {
    let orders = [order(false, 60_003_760, 34, 100.0), order(false, 60_003_760, 35, 50.0)];
    let book = [quote(false, 60_003_760, 34, 90.0), quote(false, 60_003_760, 35, 60.0)];

    let annotations = annotate_all(&orders, &book);

    assert_eq!(annotations.len(), 2);
    assert!(annotations[0].outbid);
    assert!(!annotations[1].outbid);
  }
}
