use getset::{CopyGetters, Getters};
use sqlx::FromRow;

const ENTRY_KIND_JOURNAL: &str = "journal";

const ENTRY_KIND_MARKET: &str = "market";

// Per-entry budget assignment storage (child A); the keyed variants and conversions are consumed by
// the Budget derivation/UI in children B/C, and exercised only by unit tests until then.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum BudgetEntryKind {
  #[default]
  Journal,
  Market,
}

#[derive(Clone, CopyGetters, Debug, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub category_id: i64,
  #[getset(get = "pub")]
  pub created_at: String,
  #[getset(get = "pub")]
  pub entry_kind: String,
  #[getset(get_copy = "pub")]
  pub entry_id: i64,
  #[getset(get_copy = "pub")]
  pub id: i64,
  #[getset(get_copy = "pub")]
  pub owner_id: i64,
  #[getset(get = "pub")]
  pub owner_kind: String,
  #[getset(get_copy = "pub")]
  pub scope_id: Option<i64>,
  #[getset(get = "pub")]
  pub scope_kind: String,
  #[getset(get = "pub")]
  pub updated_at: String,
}

impl BudgetEntryKind {
  pub fn from_kind(kind: &str) -> Option<Self> {
    match kind {
      ENTRY_KIND_JOURNAL => Some(BudgetEntryKind::Journal),
      ENTRY_KIND_MARKET => Some(BudgetEntryKind::Market),
      _ => None,
    }
  }

  pub fn as_str(self) -> &'static str {
    match self {
      BudgetEntryKind::Journal => ENTRY_KIND_JOURNAL,
      BudgetEntryKind::Market => ENTRY_KIND_MARKET,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod from_kind {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_maps_the_known_kinds() {
      assert_eq!(BudgetEntryKind::from_kind("journal"), Some(BudgetEntryKind::Journal));
      assert_eq!(BudgetEntryKind::from_kind("market"), Some(BudgetEntryKind::Market));
    }

    #[test]
    fn it_rejects_an_unknown_kind() {
      assert_eq!(BudgetEntryKind::from_kind("escrow"), None);
    }
  }

  mod round_trip {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_round_trips_through_the_persisted_kind() {
      for kind in [BudgetEntryKind::Journal, BudgetEntryKind::Market] {
        assert_eq!(BudgetEntryKind::from_kind(kind.as_str()), Some(kind));
      }
    }
  }
}
