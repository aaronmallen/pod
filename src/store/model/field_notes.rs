use sqlx::FromRow;

#[derive(Clone, Debug, Default, Eq, FromRow, PartialEq)]
pub struct Model {
  pub created_at: String,
  pub date: String,
  pub id: i64,
  pub text: String,
  pub updated_at: String,
}
