use sqlx::FromRow;

// Blueprint activity storage foundation; consumed by the industry sync/UI once it lands. Exercised only by
// unit tests until then.
#[derive(Clone, Copy, Debug, Eq, FromRow, PartialEq)]
pub struct Model {
  pub activity_id: i64,
  pub blueprint_type_id: i64,
  pub product_type_id: i64,
  pub quantity: i64,
}
