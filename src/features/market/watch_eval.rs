use std::collections::HashMap;

use crate::store::model::WatchDirection;

pub type BookKey = (i64, i64);

pub type PriceMap = HashMap<BookKey, BestPrices>;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BestPrices {
  pub access: PriceAccess,
  pub best_buy: Option<f64>,
  pub best_sell: Option<f64>,
}

impl BestPrices {
  pub fn available(best_buy: Option<f64>, best_sell: Option<f64>) -> Self {
    BestPrices {
      access: PriceAccess::Ok,
      best_buy,
      best_sell,
    }
  }

  pub fn inaccessible() -> Self {
    BestPrices {
      access: PriceAccess::Inaccessible,
      best_buy: None,
      best_sell: None,
    }
  }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PriceAccess {
  Inaccessible,
  #[default]
  Ok,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WatchOutcome {
  pub current: Option<f64>,
  pub met: bool,
}

#[allow(dead_code)]
pub fn current_price(direction: WatchDirection, prices: &BestPrices) -> Option<f64> {
  match direction {
    WatchDirection::Buy => prices.best_sell,
    WatchDirection::Sell => prices.best_buy,
  }
}

#[allow(dead_code)]
pub fn is_target_met(direction: WatchDirection, current: f64, target: f64) -> bool {
  match direction {
    WatchDirection::Buy => current <= target,
    WatchDirection::Sell => current >= target,
  }
}

#[allow(dead_code)]
pub fn evaluate(direction: WatchDirection, target: Option<f64>, prices: &BestPrices) -> WatchOutcome {
  let current = current_price(direction, prices);
  let met = match (current, target) {
    (Some(current), Some(target)) => is_target_met(direction, current, target),
    _ => false,
  };
  WatchOutcome {
    current,
    met,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn prices(best_buy: Option<f64>, best_sell: Option<f64>) -> BestPrices {
    BestPrices::available(best_buy, best_sell)
  }

  mod best_prices {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_flags_an_inaccessible_scope_with_no_prices() {
      let prices = BestPrices::inaccessible();

      assert_eq!(prices.access, PriceAccess::Inaccessible);
      assert_eq!(prices.best_buy, None);
      assert_eq!(prices.best_sell, None);
    }

    #[test]
    fn it_marks_available_prices_ok() {
      let prices = BestPrices::available(Some(9.0), Some(11.0));

      assert_eq!(prices.access, PriceAccess::Ok);
      assert_eq!(prices.best_buy, Some(9.0));
    }
  }

  mod current_price {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_watches_the_best_sell_for_a_buy_watch() {
      assert_eq!(
        current_price(WatchDirection::Buy, &prices(Some(9.0), Some(11.0))),
        Some(11.0)
      );
    }

    #[test]
    fn it_watches_the_best_buy_for_a_sell_watch() {
      assert_eq!(
        current_price(WatchDirection::Sell, &prices(Some(9.0), Some(11.0))),
        Some(9.0)
      );
    }

    #[test]
    fn it_reports_no_price_when_the_watched_side_is_empty() {
      assert_eq!(current_price(WatchDirection::Buy, &prices(Some(9.0), None)), None);
      assert_eq!(current_price(WatchDirection::Sell, &prices(None, Some(11.0))), None);
    }
  }

  mod evaluate {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_meets_a_buy_watch_when_the_sell_price_is_below_target() {
      let outcome = evaluate(WatchDirection::Buy, Some(10.0), &prices(Some(8.0), Some(9.0)));
      assert_eq!(
        outcome,
        WatchOutcome {
          current: Some(9.0),
          met: true,
        }
      );
    }

    #[test]
    fn it_meets_a_buy_watch_when_the_sell_price_equals_target() {
      let outcome = evaluate(WatchDirection::Buy, Some(10.0), &prices(Some(8.0), Some(10.0)));
      assert!(outcome.met);
    }

    #[test]
    fn it_misses_a_buy_watch_when_the_sell_price_is_above_target() {
      let outcome = evaluate(WatchDirection::Buy, Some(10.0), &prices(Some(8.0), Some(11.0)));
      assert_eq!(
        outcome,
        WatchOutcome {
          current: Some(11.0),
          met: false,
        }
      );
    }

    #[test]
    fn it_meets_a_sell_watch_when_the_buy_price_is_above_target() {
      let outcome = evaluate(WatchDirection::Sell, Some(10.0), &prices(Some(12.0), Some(13.0)));
      assert_eq!(
        outcome,
        WatchOutcome {
          current: Some(12.0),
          met: true,
        }
      );
    }

    #[test]
    fn it_meets_a_sell_watch_when_the_buy_price_equals_target() {
      let outcome = evaluate(WatchDirection::Sell, Some(10.0), &prices(Some(10.0), Some(13.0)));
      assert!(outcome.met);
    }

    #[test]
    fn it_misses_a_sell_watch_when_the_buy_price_is_below_target() {
      let outcome = evaluate(WatchDirection::Sell, Some(10.0), &prices(Some(9.0), Some(13.0)));
      assert!(!outcome.met);
    }

    #[test]
    fn it_never_meets_an_empty_market() {
      let buy = evaluate(WatchDirection::Buy, Some(10.0), &prices(None, None));
      assert_eq!(
        buy,
        WatchOutcome {
          current: None,
          met: false,
        }
      );

      let sell = evaluate(WatchDirection::Sell, Some(10.0), &prices(None, None));
      assert_eq!(
        sell,
        WatchOutcome {
          current: None,
          met: false,
        }
      );
    }

    #[test]
    fn it_never_meets_a_watch_without_a_target() {
      let outcome = evaluate(WatchDirection::Buy, None, &prices(Some(8.0), Some(9.0)));
      assert_eq!(
        outcome,
        WatchOutcome {
          current: Some(9.0),
          met: false,
        }
      );
    }
  }
}
