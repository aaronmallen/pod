use getset::{CopyGetters, Getters};
use sqlx::FromRow;

use crate::clients::esi::models::corporation::{CorporationDivisionName, CorporationWalletBalance};

#[derive(Clone, CopyGetters, Debug, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub balance: Option<f64>,
  #[getset(get_copy = "pub")]
  pub corporation_id: i64,
  #[getset(get_copy = "pub")]
  pub division: i64,
  #[getset(get = "pub")]
  pub name: Option<String>,
}

impl From<(i64, CorporationDivisionName)> for Model {
  fn from((corporation_id, division): (i64, CorporationDivisionName)) -> Self {
    Self {
      balance: None,
      corporation_id,
      division: i64::from(division.division),
      name: division.name,
    }
  }
}

impl From<(i64, CorporationWalletBalance)> for Model {
  fn from((corporation_id, balance): (i64, CorporationWalletBalance)) -> Self {
    Self {
      balance: Some(balance.balance),
      corporation_id,
      division: i64::from(balance.division),
      name: None,
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
    fn it_maps_a_division_name_entry_and_leaves_balance_unset() {
      let entry = CorporationDivisionName {
        division: 3,
        name: Some("Logistics".to_owned()),
      };

      let model = Model::from((90_000_001, entry));

      assert_eq!(model.corporation_id(), 90_000_001);
      assert_eq!(model.division(), 3);
      assert_eq!(model.name(), &Some("Logistics".to_owned()));
      assert_eq!(model.balance(), None);
    }

    #[test]
    fn it_maps_a_wallet_balance_entry_and_leaves_name_unset() {
      let entry = CorporationWalletBalance {
        balance: 1_234_567.89,
        division: 1,
      };

      let model = Model::from((90_000_001, entry));

      assert_eq!(model.corporation_id(), 90_000_001);
      assert_eq!(model.division(), 1);
      assert_eq!(model.balance(), Some(1_234_567.89));
      assert_eq!(model.name(), &None);
    }
  }
}
