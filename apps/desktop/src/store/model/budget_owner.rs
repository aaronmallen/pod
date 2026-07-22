const OWNER_KIND_CHARACTER: &str = "character";

const OWNER_KIND_CORPORATION: &str = "corporation";

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BudgetOwner {
  Character(i64),
  Corporation(i64),
}

impl BudgetOwner {
  pub fn from_key(owner_kind: &str, owner_id: i64) -> Option<Self> {
    match owner_kind {
      OWNER_KIND_CHARACTER => Some(BudgetOwner::Character(owner_id)),
      OWNER_KIND_CORPORATION => Some(BudgetOwner::Corporation(owner_id)),
      _ => None,
    }
  }

  pub fn owner_id(self) -> i64 {
    match self {
      BudgetOwner::Character(id) | BudgetOwner::Corporation(id) => id,
    }
  }

  pub fn owner_kind(self) -> &'static str {
    match self {
      BudgetOwner::Character(_) => OWNER_KIND_CHARACTER,
      BudgetOwner::Corporation(_) => OWNER_KIND_CORPORATION,
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
    fn it_maps_the_known_kinds() {
      assert_eq!(BudgetOwner::from_key("character", 7), Some(BudgetOwner::Character(7)));
      assert_eq!(
        BudgetOwner::from_key("corporation", 9),
        Some(BudgetOwner::Corporation(9))
      );
    }

    #[test]
    fn it_rejects_an_unknown_kind() {
      assert_eq!(BudgetOwner::from_key("alliance", 1), None);
    }
  }

  mod round_trip {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_round_trips_through_the_persisted_key() {
      for owner in [BudgetOwner::Character(3), BudgetOwner::Corporation(8)] {
        assert_eq!(BudgetOwner::from_key(owner.owner_kind(), owner.owner_id()), Some(owner));
      }
    }
  }
}
