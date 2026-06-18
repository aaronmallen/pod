use getset::{CopyGetters, Getters};
use sqlx::FromRow;

#[derive(Clone, CopyGetters, Debug, Eq, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get = "pub")]
  pub body: String,
  #[getset(get_copy = "pub")]
  pub character_id: i64,
  #[getset(get = "pub")]
  pub created_at: String,
  #[getset(get_copy = "pub")]
  pub id: i64,
  #[getset(get = "pub")]
  pub kind: String,
  #[getset(get = "pub")]
  pub quote: Option<String>,
  #[getset(get = "pub")]
  pub recipients_cc: String,
  #[getset(get = "pub")]
  pub recipients_to: String,
  #[getset(get = "pub")]
  pub subject: String,
  #[getset(get = "pub")]
  pub updated_at: String,
}
