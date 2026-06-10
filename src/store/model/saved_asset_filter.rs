use getset::{CopyGetters, Getters};
use sqlx::FromRow;

#[derive(Clone, CopyGetters, Debug, Eq, FromRow, Getters, PartialEq)]
pub struct Model {
  /// `None` means "All categories" (no category restriction).
  #[getset(get = "pub")]
  pub category: Option<String>,
  #[getset(get_copy = "pub")]
  pub id: i64,
  #[getset(get = "pub")]
  pub name: String,
  #[getset(get = "pub")]
  pub query: String,
}
