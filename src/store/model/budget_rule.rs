use getset::{CopyGetters, Getters};
use sqlx::FromRow;

use crate::store::model::BudgetScope;

const FIELD_AMOUNT: &str = "amount";

const FIELD_CHARACTER: &str = "character";

const FIELD_DIRECTION: &str = "direction";

const FIELD_ITEM: &str = "item";

const FIELD_LOCATION: &str = "loc";

const FIELD_PARTY: &str = "party";

const FIELD_REFERENCE: &str = "ref";

const FIELD_TEXT: &str = "text";

const FIELD_TYPE: &str = "type";

const MATCH_MODE_ALL: &str = "all";

const MATCH_MODE_ANY: &str = "any";

const OP_BETWEEN: &str = "between";

const OP_CONTAINS: &str = "contains";

const OP_GREATER_THAN: &str = "gt";

const OP_IS: &str = "is";

const OP_IS_NOT: &str = "nis";

const OP_LESS_THAN: &str = "lt";

const OP_NOT_CONTAINS: &str = "ncontains";

const OP_STARTS_WITH: &str = "starts";

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum MatchMode {
  #[default]
  All,
  Any,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NewRule {
  pub category_id: i64,
  pub enabled: bool,
  pub match_mode: MatchMode,
  pub name: String,
  pub position: i64,
  pub scope: BudgetScope,
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum RuleField {
  Amount,
  Character,
  Direction,
  Item,
  Location,
  Party,
  Reference,
  #[default]
  Text,
  Type,
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum RuleOp {
  Between,
  #[default]
  Contains,
  GreaterThan,
  Is,
  IsNot,
  LessThan,
  NotContains,
  StartsWith,
}

#[derive(Clone, CopyGetters, Debug, Getters, PartialEq)]
pub struct Rule {
  #[getset(get_copy = "pub")]
  pub category_id: i64,
  #[getset(get = "pub")]
  pub conditions: Vec<RuleCondition>,
  #[getset(get_copy = "pub")]
  pub enabled: bool,
  #[getset(get_copy = "pub")]
  pub id: i64,
  #[getset(get_copy = "pub")]
  pub match_mode: MatchMode,
  #[getset(get = "pub")]
  pub name: String,
}

#[derive(Clone, CopyGetters, Debug, Getters, PartialEq)]
pub struct RuleCondition {
  #[getset(get_copy = "pub")]
  pub field: RuleField,
  #[getset(get_copy = "pub")]
  pub op: RuleOp,
  #[getset(get = "pub")]
  pub value: String,
  #[getset(get = "pub")]
  pub value2: Option<String>,
}

#[derive(Clone, Debug, FromRow, PartialEq)]
pub(crate) struct RuleConditionRow {
  pub field: String,
  pub op: String,
  pub rule_id: i64,
  pub value: String,
  pub value2: Option<String>,
}

#[derive(Clone, Debug, FromRow, PartialEq)]
pub(crate) struct RuleRow {
  pub category_id: i64,
  pub enabled: i64,
  pub id: i64,
  pub match_mode: String,
  pub name: String,
}

impl MatchMode {
  pub fn as_str(self) -> &'static str {
    match self {
      MatchMode::All => MATCH_MODE_ALL,
      MatchMode::Any => MATCH_MODE_ANY,
    }
  }

  pub fn from_key(key: &str) -> Self {
    match key {
      MATCH_MODE_ANY => MatchMode::Any,
      _ => MatchMode::All,
    }
  }
}

impl RuleField {
  pub fn as_str(self) -> &'static str {
    match self {
      RuleField::Amount => FIELD_AMOUNT,
      RuleField::Character => FIELD_CHARACTER,
      RuleField::Direction => FIELD_DIRECTION,
      RuleField::Item => FIELD_ITEM,
      RuleField::Location => FIELD_LOCATION,
      RuleField::Party => FIELD_PARTY,
      RuleField::Reference => FIELD_REFERENCE,
      RuleField::Text => FIELD_TEXT,
      RuleField::Type => FIELD_TYPE,
    }
  }

  pub fn from_key(key: &str) -> Self {
    match key {
      FIELD_AMOUNT => RuleField::Amount,
      FIELD_CHARACTER => RuleField::Character,
      FIELD_DIRECTION => RuleField::Direction,
      FIELD_ITEM => RuleField::Item,
      FIELD_LOCATION => RuleField::Location,
      FIELD_PARTY => RuleField::Party,
      FIELD_REFERENCE => RuleField::Reference,
      FIELD_TYPE => RuleField::Type,
      _ => RuleField::Text,
    }
  }
}

impl RuleOp {
  pub fn as_str(self) -> &'static str {
    match self {
      RuleOp::Between => OP_BETWEEN,
      RuleOp::Contains => OP_CONTAINS,
      RuleOp::GreaterThan => OP_GREATER_THAN,
      RuleOp::Is => OP_IS,
      RuleOp::IsNot => OP_IS_NOT,
      RuleOp::LessThan => OP_LESS_THAN,
      RuleOp::NotContains => OP_NOT_CONTAINS,
      RuleOp::StartsWith => OP_STARTS_WITH,
    }
  }

  pub fn from_key(key: &str) -> Self {
    match key {
      OP_BETWEEN => RuleOp::Between,
      OP_GREATER_THAN => RuleOp::GreaterThan,
      OP_IS => RuleOp::Is,
      OP_IS_NOT => RuleOp::IsNot,
      OP_LESS_THAN => RuleOp::LessThan,
      OP_NOT_CONTAINS => RuleOp::NotContains,
      OP_STARTS_WITH => RuleOp::StartsWith,
      _ => RuleOp::Contains,
    }
  }
}

impl RuleConditionRow {
  pub(crate) fn into_condition(self) -> RuleCondition {
    RuleCondition {
      field: RuleField::from_key(&self.field),
      op: RuleOp::from_key(&self.op),
      value: self.value,
      value2: self.value2,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod match_mode {
    use super::*;

    mod from_key {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_defaults_unknown_keys_to_all() {
        assert_eq!(MatchMode::from_key("garbage"), MatchMode::All);
      }
    }

    mod round_trip {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_round_trips_through_the_persisted_key() {
        for mode in [MatchMode::All, MatchMode::Any] {
          assert_eq!(MatchMode::from_key(mode.as_str()), mode);
        }
      }
    }
  }

  mod rule_field {
    use super::*;

    mod from_key {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_defaults_unknown_keys_to_text() {
        assert_eq!(RuleField::from_key("garbage"), RuleField::Text);
      }
    }

    mod round_trip {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_round_trips_every_field() {
        for field in [
          RuleField::Amount,
          RuleField::Character,
          RuleField::Direction,
          RuleField::Item,
          RuleField::Location,
          RuleField::Party,
          RuleField::Reference,
          RuleField::Text,
          RuleField::Type,
        ] {
          assert_eq!(RuleField::from_key(field.as_str()), field);
        }
      }
    }
  }

  mod rule_op {
    use super::*;

    mod from_key {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_defaults_unknown_keys_to_contains() {
        assert_eq!(RuleOp::from_key("garbage"), RuleOp::Contains);
      }
    }

    mod round_trip {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_round_trips_every_op() {
        for op in [
          RuleOp::Between,
          RuleOp::Contains,
          RuleOp::GreaterThan,
          RuleOp::Is,
          RuleOp::IsNot,
          RuleOp::LessThan,
          RuleOp::NotContains,
          RuleOp::StartsWith,
        ] {
          assert_eq!(RuleOp::from_key(op.as_str()), op);
        }
      }
    }
  }
}
