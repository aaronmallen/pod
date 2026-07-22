use getset::{CopyGetters, Getters};
use sqlx::FromRow;

use crate::clients::esi::models::corporation::CorporationWalletTransaction;

#[derive(Clone, CopyGetters, Debug, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub client_id: i64,
  #[getset(get_copy = "pub")]
  pub corporation_id: i64,
  #[getset(get = "pub")]
  pub date: String,
  #[getset(get_copy = "pub")]
  pub division: i64,
  #[getset(get_copy = "pub")]
  pub is_buy: bool,
  #[getset(get_copy = "pub")]
  pub journal_ref_id: i64,
  #[getset(get_copy = "pub")]
  pub location_id: i64,
  #[getset(get_copy = "pub")]
  pub quantity: i64,
  #[getset(get_copy = "pub")]
  pub transaction_id: i64,
  #[getset(get_copy = "pub")]
  pub type_id: i64,
  #[getset(get_copy = "pub")]
  pub unit_price: f64,
}

impl From<(i64, i64, CorporationWalletTransaction)> for Model {
  fn from((corporation_id, division, transaction): (i64, i64, CorporationWalletTransaction)) -> Self {
    Self {
      client_id: transaction.client_id,
      corporation_id,
      date: transaction.date,
      division,
      is_buy: transaction.is_buy,
      journal_ref_id: transaction.journal_ref_id,
      location_id: transaction.location_id,
      quantity: i64::from(transaction.quantity),
      transaction_id: transaction.transaction_id,
      type_id: i64::from(transaction.type_id),
      unit_price: transaction.unit_price,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod from {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_attaches_corporation_id_and_division_and_widens_ids() {
      let transaction = CorporationWalletTransaction {
        client_id: 1_000_035,
        date: "2026-05-30T12:00:00Z".to_owned(),
        is_buy: true,
        journal_ref_id: 123_456_789,
        location_id: 60_003_760,
        quantity: 10,
        transaction_id: 987_654_321,
        type_id: 34,
        unit_price: 5.5,
      };

      let model = Model::from((90_000_001, 1, transaction));

      assert_eq!(model.corporation_id(), 90_000_001);
      assert_eq!(model.division(), 1);
      assert_eq!(model.transaction_id(), 987_654_321);
      assert_eq!(model.type_id(), 34);
      assert_eq!(model.quantity(), 10);
      assert_eq!(model.journal_ref_id(), 123_456_789);
      assert!(model.is_buy());
      assert_eq!(model.unit_price(), 5.5);
    }
  }
}
