//! Domain model for wallet transactions.

use validator::Validate;

#[derive(Clone, Debug, Validate)]
pub struct Model {
  pub character_id: i64,
  pub transaction_id: i64,
  pub type_id: i32,
  pub quantity: i32,
  pub unit_price: f64,
  pub is_buy: bool,
  #[validate(length(min = 1))]
  pub date: String,
  pub location_id: i64,
  pub client_id: i64,
}

#[cfg(test)]
mod tests {
  use validator::Validate;

  use super::*;

  fn make_transaction() -> Model {
    Model {
      character_id: 90_000_001,
      transaction_id: 55_001,
      type_id: 587,
      quantity: 1,
      unit_price: 600_000.0,
      is_buy: false,
      date: "2024-06-01T12:00:00Z".into(),
      location_id: 60_003_760,
      client_id: 90_000_002,
    }
  }

  mod validate {
    use super::*;

    #[test]
    fn it_passes_for_valid_transaction() {
      let tx = make_transaction();
      assert!(tx.validate().is_ok());
    }

    #[test]
    fn it_fails_when_date_is_empty() {
      let mut tx = make_transaction();
      tx.date = String::new();
      assert!(tx.validate().is_err());
    }

    #[test]
    fn it_accepts_buy_transactions() {
      let mut tx = make_transaction();
      tx.is_buy = true;
      assert!(tx.validate().is_ok());
    }
  }
}
