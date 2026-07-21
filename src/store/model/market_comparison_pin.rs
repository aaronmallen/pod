use sqlx::FromRow;

#[allow(dead_code)]
#[derive(Clone, Debug, Default, FromRow, PartialEq)]
pub struct Model {
  pub id: i64,
  pub position: i64,
  pub type_id: i64,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Default, FromRow, PartialEq)]
pub struct Market {
  pub id: i64,
  pub pin_id: i64,
  pub place_id: i64,
  pub position: i64,
  pub tier: String,
}
