use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct CorporationAsset {
  #[serde(default)]
  pub is_blueprint_copy: Option<bool>,
  pub is_singleton: bool,
  pub item_id: i64,
  pub location_flag: String,
  pub location_id: i64,
  pub location_type: String,
  pub quantity: i32,
  pub type_id: i32,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct CorporationCustomsOffice {
  #[serde(default)]
  pub alliance_tax_rate: Option<f64>,
  pub allow_access_with_standings: bool,
  pub allow_alliance_access: bool,
  #[serde(default)]
  pub bad_standing_tax_rate: Option<f64>,
  #[serde(default)]
  pub corporation_tax_rate: Option<f64>,
  #[serde(default)]
  pub excellent_standing_tax_rate: Option<f64>,
  #[serde(default)]
  pub good_standing_tax_rate: Option<f64>,
  #[serde(default)]
  pub neutral_standing_tax_rate: Option<f64>,
  pub office_id: i64,
  pub reinforce_exit_end: i32,
  pub reinforce_exit_start: i32,
  pub standing_level: String,
  pub system_id: i64,
  #[serde(default)]
  pub terrible_standing_tax_rate: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct CorporationDivisionName {
  pub division: i32,
  #[serde(default)]
  pub name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CorporationDivisions {
  #[serde(default)]
  pub hangar: Vec<CorporationDivisionName>,
  #[serde(default)]
  pub wallet: Vec<CorporationDivisionName>,
}

#[derive(Debug, Deserialize)]
pub struct CorporationInfo {
  #[serde(default)]
  pub alliance_id: Option<i64>,
  pub ceo_id: i64,
  pub creator_id: i64,
  #[serde(default)]
  pub date_founded: Option<String>,
  #[serde(default)]
  pub description: Option<String>,
  #[serde(default)]
  pub faction_id: Option<i64>,
  #[serde(default)]
  pub home_station_id: Option<i64>,
  pub member_count: i32,
  pub name: String,
  #[serde(default)]
  pub shares: Option<i64>,
  pub tax_rate: f64,
  pub ticker: String,
  #[serde(default)]
  pub url: Option<String>,
  #[serde(default)]
  pub war_eligible: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct CorporationStructure {
  pub corporation_id: i64,
  #[serde(default)]
  pub fuel_expires: Option<String>,
  #[serde(default)]
  pub name: Option<String>,
  #[serde(default)]
  pub next_reinforce_apply: Option<String>,
  #[serde(default)]
  pub next_reinforce_hour: Option<i32>,
  #[serde(default)]
  pub next_reinforce_weekday: Option<i32>,
  #[serde(default)]
  #[allow(dead_code, reason = "unpersisted")]
  pub profile_id: Option<i64>,
  #[serde(default)]
  pub reinforce_hour: Option<i32>,
  #[serde(default)]
  pub services: Vec<CorporationStructureService>,
  #[serde(default)]
  pub state: Option<String>,
  #[serde(default)]
  pub state_timer_end: Option<String>,
  #[serde(default)]
  pub state_timer_start: Option<String>,
  pub structure_id: i64,
  pub system_id: i64,
  pub type_id: i32,
  #[serde(default)]
  pub unanchors_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CorporationStructureService {
  pub name: String,
  pub state: String,
}

#[derive(Debug, Deserialize)]
pub struct CorporationWalletBalance {
  pub balance: f64,
  pub division: i32,
}

#[derive(Debug, Deserialize)]
pub struct CorporationWalletJournalEntry {
  #[serde(default)]
  pub amount: Option<f64>,
  #[serde(default)]
  pub balance: Option<f64>,
  #[serde(default)]
  pub context_id: Option<i64>,
  #[serde(default)]
  pub context_id_type: Option<String>,
  pub date: String,
  pub description: String,
  #[serde(default)]
  pub first_party_id: Option<i64>,
  pub id: i64,
  #[serde(default)]
  pub reason: Option<String>,
  pub ref_type: String,
  #[serde(default)]
  pub second_party_id: Option<i64>,
  #[serde(default)]
  pub tax: Option<f64>,
  #[serde(default)]
  pub tax_receiver_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct CorporationWalletTransaction {
  pub client_id: i64,
  pub date: String,
  pub is_buy: bool,
  pub journal_ref_id: i64,
  pub location_id: i64,
  pub quantity: i32,
  pub transaction_id: i64,
  pub type_id: i32,
  pub unit_price: f64,
}

#[derive(Debug, Deserialize)]
#[expect(dead_code)]
pub struct MemberRole {
  pub character_id: i64,
  #[serde(default)]
  pub grantable_roles: Vec<String>,
  #[serde(default)]
  pub grantable_roles_at_base: Vec<String>,
  #[serde(default)]
  pub grantable_roles_at_hq: Vec<String>,
  #[serde(default)]
  pub grantable_roles_at_other: Vec<String>,
  #[serde(default)]
  pub roles: Vec<String>,
  #[serde(default)]
  pub roles_at_base: Vec<String>,
  #[serde(default)]
  pub roles_at_hq: Vec<String>,
  #[serde(default)]
  pub roles_at_other: Vec<String>,
}
