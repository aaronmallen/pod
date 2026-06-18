use getset::CopyGetters;
use sqlx::FromRow;

#[derive(Clone, Copy, CopyGetters, Debug, Eq, FromRow, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub agent_id: i64,
  #[getset(get_copy = "pub")]
  pub skill_type_id: i64,
}
