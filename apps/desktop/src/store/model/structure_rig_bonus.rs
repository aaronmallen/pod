use sqlx::FromRow;

/// One structure-rig dogma bonus from the SDE: the raw `value` of `attribute_id` carried by the rig
/// `type_id`, alongside its display `name`.
#[derive(Clone, Debug, FromRow, PartialEq)]
pub struct StructureRigBonus {
  pub attribute_id: i64,
  pub name: String,
  pub type_id: i64,
  pub value: f64,
}
