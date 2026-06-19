use getset::{CopyGetters, Getters};
use sqlx::FromRow;

#[derive(Clone, CopyGetters, Debug, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub corporation_id: i64,
  #[getset(get = "pub")]
  pub dogma_attributes: String,
  #[getset(get_copy = "pub")]
  pub item_id: i64,
  #[getset(get_copy = "pub")]
  pub muta_price_isk: Option<f64>,
  #[getset(get_copy = "pub")]
  pub muta_price_synced: Option<i64>,
  #[getset(get_copy = "pub")]
  pub mutator_type_id: i64,
  #[getset(get_copy = "pub")]
  pub source_type_id: i64,
  #[getset(get_copy = "pub")]
  pub synced_at: i64,
  #[getset(get_copy = "pub")]
  pub type_id: i64,
}

impl Model {
  #[allow(clippy::too_many_arguments)]
  pub fn new(
    item_id: i64,
    corporation_id: i64,
    type_id: i64,
    source_type_id: i64,
    mutator_type_id: i64,
    dogma_attributes: String,
    synced_at: i64,
  ) -> Self {
    Self {
      corporation_id,
      dogma_attributes,
      item_id,
      muta_price_isk: None,
      muta_price_synced: None,
      mutator_type_id,
      source_type_id,
      synced_at,
      type_id,
    }
  }

  #[allow(dead_code)]
  pub fn set_muta_price(&mut self, price_isk: Option<f64>, synced_at: i64) -> &mut Self {
    self.muta_price_isk = price_isk;
    self.muta_price_synced = Some(synced_at);
    self
  }
}
