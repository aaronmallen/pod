#![allow(dead_code)]

use getset::{CopyGetters, Getters};
use sqlx::FromRow;

#[derive(Clone, CopyGetters, Debug, Eq, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub character_id: i64,
  #[getset(get = "pub")]
  pub folder: String,
  #[getset(get_copy = "pub")]
  pub id: i64,
  #[getset(get_copy = "pub")]
  pub mail_id: i64,
  #[getset(get_copy = "pub")]
  pub remap_label_id: Option<i64>,
  #[getset(get_copy = "pub")]
  pub soft_delete_intent: bool,
}
