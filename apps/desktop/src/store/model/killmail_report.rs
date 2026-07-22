use getset::{CopyGetters, Getters};
use sqlx::FromRow;

#[allow(dead_code)]
#[derive(Clone, CopyGetters, Debug, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub character_id: i64,
  #[getset(get = "pub")]
  pub created_at: String,
  #[getset(get = "pub")]
  pub different: Option<String>,
  #[getset(get = "pub")]
  pub happened: String,
  #[getset(get_copy = "pub")]
  pub killmail_id: i64,
  #[getset(get = "pub")]
  pub outcome: String,
  #[getset(get = "pub")]
  pub takeaway: Option<String>,
  #[getset(get = "pub")]
  pub updated_at: String,
}
