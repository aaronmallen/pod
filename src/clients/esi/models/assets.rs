use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct AssetName {
  pub item_id: i64,
  pub name: String,
}
