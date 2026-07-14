use getset::{CopyGetters, Getters};
use sqlx::FromRow;

use crate::clients::esi::models::market::CorporationMarketOrder as EsiCorporationMarketOrder;

const STATE_OPEN: &str = "open";

#[derive(Clone, CopyGetters, Debug, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub corporation_id: i64,
  #[getset(get_copy = "pub")]
  pub duration: i64,
  #[getset(get_copy = "pub")]
  pub escrow: f64,
  #[getset(get_copy = "pub")]
  pub is_buy_order: bool,
  #[getset(get = "pub")]
  pub issued: String,
  #[getset(get_copy = "pub")]
  pub location_id: i64,
  #[getset(get_copy = "pub")]
  pub order_id: i64,
  #[getset(get_copy = "pub")]
  pub price: f64,
  #[getset(get = "pub")]
  pub range: String,
  #[getset(get_copy = "pub")]
  pub region_id: i64,
  #[getset(get = "pub")]
  pub state: String,
  #[getset(get_copy = "pub")]
  pub type_id: i64,
  #[getset(get_copy = "pub")]
  pub volume_remain: i64,
  #[getset(get_copy = "pub")]
  pub volume_total: i64,
}

impl From<(i64, EsiCorporationMarketOrder)> for Model {
  fn from((corporation_id, order): (i64, EsiCorporationMarketOrder)) -> Self {
    Self {
      corporation_id,
      duration: order.duration,
      escrow: order.escrow,
      is_buy_order: order.is_buy_order,
      issued: order.issued,
      location_id: order.location_id,
      order_id: order.order_id,
      price: order.price,
      range: order.range,
      region_id: order.region_id,
      state: STATE_OPEN.to_owned(),
      type_id: order.type_id,
      volume_remain: order.volume_remain,
      volume_total: order.volume_total,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod from {
    use pretty_assertions::assert_eq;

    use super::*;

    fn make_order() -> EsiCorporationMarketOrder {
      EsiCorporationMarketOrder {
        duration: 90,
        escrow: 550.0,
        is_buy_order: true,
        issued: "2026-06-01T12:00:00Z".to_owned(),
        location_id: 60_003_760,
        order_id: 1001,
        price: 5.5,
        range: "region".to_owned(),
        region_id: 10_000_002,
        type_id: 34,
        volume_remain: 100,
        volume_total: 200,
      }
    }

    #[test]
    fn it_attaches_corporation_id_and_derives_state_open() {
      let model = Model::from((2000, make_order()));

      assert_eq!(model.corporation_id(), 2000);
      assert_eq!(model.order_id(), 1001);
      assert_eq!(model.escrow(), 550.0);
      assert!(model.is_buy_order());
      assert_eq!(model.state(), "open");
    }

    #[test]
    fn it_maps_a_sell_order_with_zero_escrow() {
      let mut order = make_order();
      order.is_buy_order = false;
      order.escrow = 0.0;

      let model = Model::from((2000, order));

      assert!(!model.is_buy_order());
      assert_eq!(model.escrow(), 0.0);
      assert_eq!(model.state(), "open");
    }
  }
}
