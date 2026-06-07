use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Race {
  pub alliance_id: i64,
  pub description: String,
  pub name: String,
  pub race_id: i32,
}
