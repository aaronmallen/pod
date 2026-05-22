//! Domain model for character contracts.

use validator::Validate;

#[derive(Clone, Debug, Validate)]
pub struct Model {
  pub character_id: i64,
  pub contract_id: i64,
  pub contract_type: String,
  pub status: String,
  pub title: String,
  pub issuer_id: i64,
  pub assignee_id: i64,
  pub acceptor_id: i64,
  pub price: Option<f64>,
  pub collateral: Option<f64>,
  pub date_issued: String,
  pub date_expired: String,
  pub start_location_id: Option<i64>,
}

#[cfg(test)]
mod tests {
  use validator::Validate;

  use super::*;

  fn make_contract() -> Model {
    Model {
      character_id: 90_000_001,
      contract_id: 1_000_001,
      contract_type: "item_exchange".into(),
      status: "outstanding".into(),
      title: "Rifter x10".into(),
      issuer_id: 90_000_001,
      assignee_id: 0,
      acceptor_id: 0,
      price: Some(5_000_000.0),
      collateral: None,
      date_issued: "2024-01-01T00:00:00Z".into(),
      date_expired: "2024-01-15T00:00:00Z".into(),
      start_location_id: Some(60_003_760),
    }
  }

  mod validate {
    use super::*;

    #[test]
    fn it_passes_for_valid_contract() {
      let contract = make_contract();
      assert!(contract.validate().is_ok());
    }

    #[test]
    fn it_accepts_optional_fields_as_none() {
      let mut contract = make_contract();
      contract.price = None;
      contract.collateral = None;
      contract.start_location_id = None;
      assert!(contract.validate().is_ok());
    }
  }
}
