use getset::{CopyGetters, Getters};
use sqlx::FromRow;

#[derive(Clone, CopyGetters, Debug, Eq, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  character_id: i64,
  #[getset(get_copy = "pub")]
  corporation_id: i64,
  #[getset(get = "pub")]
  role: String,
}

impl From<(i64, i64, String)> for Model {
  fn from((corporation_id, character_id, role): (i64, i64, String)) -> Self {
    Self {
      character_id,
      corporation_id,
      role,
    }
  }
}
