const SCOPE_KIND_ALL: &str = "all";

const SCOPE_KIND_CHARACTER: &str = "character";

const SCOPE_KIND_CORPORATION: &str = "corporation";

// Budget storage foundation (B1); the keyed variants and `from_key` are constructed by the Budget
// feature layer in B2+, and exercised only by unit tests until then.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum BudgetScope {
  #[default]
  All,
  Character(i64),
  Corporation(i64),
}

impl BudgetScope {
  #[cfg_attr(not(test), expect(dead_code))]
  pub fn from_key(scope_kind: &str, scope_id: Option<i64>) -> Option<Self> {
    match (scope_kind, scope_id) {
      (SCOPE_KIND_ALL, _) => Some(BudgetScope::All),
      (SCOPE_KIND_CHARACTER, Some(id)) => Some(BudgetScope::Character(id)),
      (SCOPE_KIND_CORPORATION, Some(id)) => Some(BudgetScope::Corporation(id)),
      _ => None,
    }
  }

  pub fn scope_id(self) -> Option<i64> {
    match self {
      BudgetScope::All => None,
      BudgetScope::Character(id) | BudgetScope::Corporation(id) => Some(id),
    }
  }

  pub fn scope_kind(self) -> &'static str {
    match self {
      BudgetScope::All => SCOPE_KIND_ALL,
      BudgetScope::Character(_) => SCOPE_KIND_CHARACTER,
      BudgetScope::Corporation(_) => SCOPE_KIND_CORPORATION,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod from_key {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_maps_all_ignoring_scope_id() {
      assert_eq!(BudgetScope::from_key("all", None), Some(BudgetScope::All));
      assert_eq!(BudgetScope::from_key("all", Some(42)), Some(BudgetScope::All));
    }

    #[test]
    fn it_maps_character_and_corporation_with_an_id() {
      assert_eq!(
        BudgetScope::from_key("character", Some(7)),
        Some(BudgetScope::Character(7))
      );
      assert_eq!(
        BudgetScope::from_key("corporation", Some(9)),
        Some(BudgetScope::Corporation(9))
      );
    }

    #[test]
    fn it_rejects_a_keyed_scope_without_an_id() {
      assert_eq!(BudgetScope::from_key("character", None), None);
      assert_eq!(BudgetScope::from_key("corporation", None), None);
    }

    #[test]
    fn it_rejects_an_unknown_kind() {
      assert_eq!(BudgetScope::from_key("alliance", Some(1)), None);
    }
  }

  mod round_trip {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_round_trips_through_the_persisted_key() {
      for scope in [BudgetScope::All, BudgetScope::Character(3), BudgetScope::Corporation(8)] {
        assert_eq!(BudgetScope::from_key(scope.scope_kind(), scope.scope_id()), Some(scope));
      }
    }
  }
}
