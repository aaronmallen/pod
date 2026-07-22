use getset::{CopyGetters, Getters};
use sqlx::FromRow;

use crate::clients::esi::models::universe::MarketGroup;

#[derive(Clone, CopyGetters, Debug, Eq, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get = "pub")]
  pub description: String,
  #[getset(get_copy = "pub")]
  pub has_types: bool,
  #[getset(get_copy = "pub")]
  pub icon_id: Option<i64>,
  #[getset(get_copy = "pub")]
  pub id: i64,
  #[getset(get = "pub")]
  pub name: String,
  #[getset(get_copy = "pub")]
  pub parent_id: Option<i64>,
}

impl From<MarketGroup> for Model {
  fn from(group: MarketGroup) -> Self {
    Self {
      description: group.description,
      has_types: !group.types.is_empty(),
      icon_id: None,
      id: i64::from(group.market_group_id),
      name: group.name,
      parent_id: group.parent_group_id.map(i64::from),
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
    fn it_derives_has_types_false_from_an_empty_list() {
      let group = MarketGroup {
        description: "Empty".to_owned(),
        market_group_id: 5,
        name: "Empty".to_owned(),
        parent_group_id: None,
        types: vec![],
      };

      let model = Model::from(group);

      assert_eq!(model.has_types(), false);
      assert_eq!(model.parent_id(), None);
    }

    #[test]
    fn it_derives_has_types_from_a_populated_list() {
      let group = MarketGroup {
        description: "Ships".to_owned(),
        market_group_id: 4,
        name: "Ships".to_owned(),
        parent_group_id: Some(2),
        types: vec![587, 588],
      };

      let model = Model::from(group);

      assert_eq!(model.id(), 4);
      assert_eq!(model.has_types(), true);
      assert_eq!(model.parent_id(), Some(2));
    }
  }
}
