use serde::Deserialize;

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct AssetName {
  pub item_id: i64,
  pub name: String,
}
