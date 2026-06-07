use getset::{CopyGetters, Getters};
use sqlx::FromRow;

#[derive(Clone, CopyGetters, Debug, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub attribute_id: i64,
  #[getset(get_copy = "pub")]
  pub default_value: Option<f64>,
  #[getset(get = "pub")]
  pub description: Option<String>,
  #[getset(get = "pub")]
  pub display_name: Option<String>,
  #[getset(get_copy = "pub")]
  pub high_is_good: bool,
  #[getset(get_copy = "pub")]
  pub icon_id: Option<i64>,
  #[getset(get = "pub")]
  pub name: String,
  #[getset(get_copy = "pub")]
  pub published: bool,
  #[getset(get_copy = "pub")]
  pub stackable: bool,
  #[getset(get_copy = "pub")]
  pub unit_id: Option<i64>,
}
