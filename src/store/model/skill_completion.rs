use sqlx::FromRow;

#[allow(dead_code)]
#[derive(Clone, Debug, Eq, FromRow, PartialEq)]
pub struct Model {
  pub character_id: i64,
  pub completed_at: String,
  pub created_at: String,
  pub id: i64,
  pub level: i64,
  pub skill_id: i64,
  pub updated_at: String,
  pub verified: bool,
}
