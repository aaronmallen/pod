use sqlx::FromRow;

#[derive(Clone, Copy, Debug, Eq, FromRow, PartialEq)]
pub struct TypeMaterial {
  pub material_type_id: i64,
  pub quantity: i64,
  pub type_id: i64,
}
