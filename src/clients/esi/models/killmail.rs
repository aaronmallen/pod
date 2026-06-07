use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Attacker {
  #[serde(default)]
  pub character_id: Option<i64>,
  #[serde(default)]
  pub final_blow: bool,
}

#[derive(Debug, Deserialize)]
pub struct Killmail {
  #[serde(default)]
  pub attackers: Vec<Attacker>,
  pub killmail_id: i64,
  pub killmail_time: String,
  pub solar_system_id: i64,
  pub victim: Victim,
}

#[derive(Debug, Deserialize)]
pub struct Victim {
  #[serde(default)]
  pub character_id: Option<i64>,
  #[serde(default)]
  pub corporation_id: Option<i64>,
  pub ship_type_id: i64,
}
