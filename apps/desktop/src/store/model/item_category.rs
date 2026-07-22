use getset::{CopyGetters, Getters};
use sqlx::FromRow;

use crate::clients::esi::models::universe::ItemCategory;

#[derive(Clone, CopyGetters, Debug, Eq, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub icon_id: Option<i64>,
  #[getset(get_copy = "pub")]
  pub id: i64,
  #[getset(get = "pub")]
  pub name: String,
  #[getset(get_copy = "pub")]
  pub published: bool,
}

impl From<ItemCategory> for Model {
  fn from(category: ItemCategory) -> Self {
    Self {
      icon_id: None,
      id: i64::from(category.category_id),
      name: category.name,
      published: category.published,
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
    fn it_widens_the_id_and_leaves_icon_none() {
      let category = ItemCategory {
        category_id: 6,
        groups: vec![25, 26],
        name: "Ship".to_owned(),
        published: true,
      };

      let model = Model::from(category);

      assert_eq!(model.id(), 6);
      assert_eq!(model.icon_id(), None);
      assert_eq!(model.published(), true);
    }
  }
}
