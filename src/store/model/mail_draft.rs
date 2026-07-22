use getset::{CopyGetters, Getters};
use sqlx::FromRow;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DraftInput {
  pub body: String,
  pub character_id: i64,
  pub kind: String,
  pub quote: Option<String>,
  pub recipients_cc: String,
  pub recipients_to: String,
  pub subject: String,
}

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
