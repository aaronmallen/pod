//! Domain model for wallet journal entries.

use validator::Validate;

#[derive(Clone, Debug, Validate)]
pub struct Model {
  pub character_id: i64,
  pub entry_id: i64,
  #[validate(length(min = 1))]
  pub ref_type: String,
  pub amount: Option<f64>,
  pub balance: Option<f64>,
  #[validate(length(min = 1))]
  pub date: String,
  pub description: String,
  pub first_party_id: Option<i64>,
  pub second_party_id: Option<i64>,
}

#[cfg(test)]
mod tests {
  use validator::Validate;

  use super::*;

  fn make_entry() -> Model {
    Model {
      character_id: 90_000_001,
      entry_id: 22_001,
      ref_type: "market_transaction".into(),
      amount: Some(-50_000.0),
      balance: Some(4_950_000.0),
      date: "2024-06-01T12:00:00Z".into(),
      description: "Market sell order".into(),
      first_party_id: Some(90_000_001),
      second_party_id: None,
    }
  }

  mod validate {
    use super::*;

    #[test]
    fn it_passes_for_valid_entry() {
      let entry = make_entry();
      assert!(entry.validate().is_ok());
    }

    #[test]
    fn it_fails_when_ref_type_is_empty() {
      let mut entry = make_entry();
      entry.ref_type = String::new();
      assert!(entry.validate().is_err());
    }

    #[test]
    fn it_fails_when_date_is_empty() {
      let mut entry = make_entry();
      entry.date = String::new();
      assert!(entry.validate().is_err());
    }

    #[test]
    fn it_accepts_optional_fields_as_none() {
      let mut entry = make_entry();
      entry.amount = None;
      entry.balance = None;
      entry.first_party_id = None;
      entry.second_party_id = None;
      assert!(entry.validate().is_ok());
    }
  }
}
