use getset::CopyGetters;
use sqlx::FromRow;

#[derive(Clone, Copy, CopyGetters, Debug, Eq, FromRow, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub advanced: i64,
  #[getset(get_copy = "pub")]
  pub basic: i64,
  #[getset(get_copy = "pub")]
  pub certificate_id: i64,
  #[getset(get_copy = "pub")]
  pub elite: i64,
  #[getset(get_copy = "pub")]
  pub improved: i64,
  #[getset(get_copy = "pub")]
  pub skill_id: i64,
}
