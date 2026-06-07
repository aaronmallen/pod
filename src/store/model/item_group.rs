use getset::{CopyGetters, Getters};
use sqlx::FromRow;

use crate::clients::esi::models::universe::ItemGroup;

#[derive(Clone, CopyGetters, Debug, Eq, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub category_id: i64,
  #[getset(get_copy = "pub")]
  pub icon_id: Option<i64>,
  #[getset(get_copy = "pub")]
  pub id: i64,
  #[getset(get = "pub")]
  pub name: String,
  #[getset(get_copy = "pub")]
  pub published: bool,
}

impl From<ItemGroup> for Model {
  fn from(group: ItemGroup) -> Self {
    Self {
      category_id: i64::from(group.category_id),
      icon_id: None,
      id: i64::from(group.group_id),
      name: group.name,
      published: group.published,
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
    fn it_widens_ids_and_leaves_icon_none() {
      let group = ItemGroup {
        category_id: 6,
        group_id: 25,
        name: "Frigate".to_owned(),
        published: true,
        types: vec![587, 588],
      };

      let model = Model::from(group);

      assert_eq!(model.id(), 25);
      assert_eq!(model.category_id(), 6);
      assert_eq!(model.icon_id(), None);
    }
  }
}
