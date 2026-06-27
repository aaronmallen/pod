use getset::{CopyGetters, Getters};
use sqlx::FromRow;

/// The keyset cursor for the next contacts page: the active sort column's value of the last row plus its
/// `contact_id` tiebreaker. `Name`/`Type` carry the text value; `Standing` carries the numeric value.
#[derive(Clone, Debug, PartialEq)]
pub enum ContactCursor {
  Number(f64, i64),
  Text(String, i64),
}

/// The column a contacts page is keyset-ordered by. Mirrors the address-book sort header so the UI can push its
/// active sort into SQL instead of holding the full set in memory and sorting client-side.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContactSortColumn {
  Name,
  Standing,
  Type,
}

/// Sort direction for a contacts page; pairs with [`ContactSortColumn`] to drive the keyset comparison.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContactSortDir {
  Asc,
  Desc,
}

#[derive(Clone, CopyGetters, Debug, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub character_id: i64,
  #[getset(get_copy = "pub")]
  pub contact_id: i64,
  #[getset(get = "pub")]
  pub contact_name: String,
  #[getset(get = "pub")]
  pub contact_type: String,
  #[getset(get_copy = "pub")]
  pub is_blocked: bool,
  #[getset(get_copy = "pub")]
  pub is_watched: bool,
  #[getset(get = "pub")]
  pub label_ids: String,
  #[getset(get_copy = "pub")]
  pub standing: f64,
}
