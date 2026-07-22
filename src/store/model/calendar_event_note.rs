use sqlx::FromRow;

#[derive(Clone, Debug, Default, Eq, FromRow, PartialEq)]
pub struct Model {
  pub character_id: i64,
  pub created_at: String,
  pub event_id: i64,
  pub note: String,
  pub updated_at: String,
}
