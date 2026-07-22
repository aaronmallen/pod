use sqlx::FromRow;

#[derive(Clone, Copy, Debug, Eq, FromRow, PartialEq)]
pub struct Model {
  pub activity_id: i64,
  pub blueprint_type_id: i64,
  pub material_type_id: i64,
  pub quantity: i64,
}
