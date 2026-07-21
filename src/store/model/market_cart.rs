use sqlx::FromRow;

#[allow(dead_code)]
#[derive(Clone, Debug, Default, FromRow, PartialEq)]
pub struct Model {
  pub created_at: String,
  pub id: i64,
  pub is_live: bool,
  pub name: Option<String>,
  pub updated_at: String,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Default, FromRow, PartialEq)]
pub struct Line {
  pub cart_id: i64,
  pub id: i64,
  pub position: i64,
  pub quantity: i64,
  pub type_id: i64,
}
