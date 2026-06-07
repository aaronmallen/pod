use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Bloodline {
  pub bloodline_id: i32,
  pub charisma: i32,
  pub corporation_id: i64,
  pub description: String,
  pub intelligence: i32,
  pub memory: i32,
  pub name: String,
  pub perception: i32,
  pub race_id: i32,
  pub ship_type_id: Option<i32>,
  pub willpower: i32,
}
