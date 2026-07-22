use getset::CopyGetters;
use sqlx::FromRow;

#[derive(Clone, Copy, CopyGetters, Debug, Eq, FromRow, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  character_id: i64,
  #[getset(get_copy = "pub")]
  position: i64,
  #[getset(get_copy = "pub")]
  squad_id: i64,
}
