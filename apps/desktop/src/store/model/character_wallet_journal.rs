use getset::{CopyGetters, Getters};
use sqlx::FromRow;

use crate::clients::esi::models::character::WalletJournalEntry;

#[derive(Clone, CopyGetters, Debug, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub amount: Option<f64>,
  #[getset(get_copy = "pub")]
  pub balance: Option<f64>,
  #[getset(get_copy = "pub")]
  pub character_id: i64,
  #[getset(get_copy = "pub")]
  pub context_id: Option<i64>,
  #[getset(get = "pub")]
  pub context_id_type: Option<String>,
  #[getset(get = "pub")]
  pub date: String,
  #[getset(get = "pub")]
  pub description: String,
  #[getset(get_copy = "pub")]
  pub first_party_id: Option<i64>,
  #[getset(get_copy = "pub")]
  pub id: i64,
  #[getset(get = "pub")]
  pub reason: Option<String>,
  #[getset(get = "pub")]
  pub ref_type: String,
  #[getset(get_copy = "pub")]
  pub second_party_id: Option<i64>,
  #[getset(get_copy = "pub")]
  pub tax: Option<f64>,
  #[getset(get_copy = "pub")]
  pub tax_receiver_id: Option<i64>,
}

impl From<(i64, WalletJournalEntry)> for Model {
  fn from((character_id, entry): (i64, WalletJournalEntry)) -> Self {
    Self {
      amount: entry.amount,
      balance: entry.balance,
      character_id,
      context_id: entry.context_id,
      context_id_type: entry.context_id_type,
      date: entry.date,
      description: entry.description,
      first_party_id: entry.first_party_id,
      id: entry.id,
      reason: entry.reason,
      ref_type: entry.ref_type,
      second_party_id: entry.second_party_id,
      tax: entry.tax,
      tax_receiver_id: entry.tax_receiver_id,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod from {
    use pretty_assertions::assert_eq;

    use super::*;

    fn make_entry() -> WalletJournalEntry {
      WalletJournalEntry {
        amount: Some(-1_000.5),
        balance: Some(50_000.25),
        context_id: Some(60_003_760),
        context_id_type: Some("station_id".to_owned()),
        date: "2026-05-30T12:00:00Z".to_owned(),
        description: "Market escrow".to_owned(),
        first_party_id: Some(90_000_001),
        id: 123_456_789,
        reason: Some("buy order".to_owned()),
        ref_type: "market_escrow".to_owned(),
        second_party_id: Some(1_000_035),
        tax: Some(2.5),
        tax_receiver_id: Some(1_000_132),
      }
    }

    #[test]
    fn it_attaches_character_id_and_passes_through_amount_and_balance() {
      let model = Model::from((42, make_entry()));

      assert_eq!(model.character_id(), 42);
      assert_eq!(model.id(), 123_456_789);
      assert_eq!(model.amount(), Some(-1_000.5));
      assert_eq!(model.balance(), Some(50_000.25));
      assert_eq!(model.ref_type(), "market_escrow");
      assert_eq!(model.description(), "Market escrow");
    }

    #[test]
    fn it_passes_through_null_amount_and_balance() {
      let mut entry = make_entry();
      entry.amount = None;
      entry.balance = None;

      let model = Model::from((42, entry));

      assert_eq!(model.amount(), None);
      assert_eq!(model.balance(), None);
    }
  }
}
