//! Corporation ESI response models.

use serde::{Deserialize, Serialize};

/// Public information about a corporation.
#[derive(Debug, Deserialize, Serialize)]
pub struct CorporationDetail {
  pub alliance_id: Option<i64>,
  pub ceo_id: i64,
  pub creator_id: i64,
  pub date_founded: Option<String>,
  pub description: Option<String>,
  pub faction_id: Option<i64>,
  pub home_station_id: Option<i64>,
  pub member_count: i32,
  pub name: String,
  pub shares: Option<i64>,
  pub tax_rate: f64,
  pub ticker: String,
  pub url: Option<String>,
  pub war_eligible: Option<bool>,
}

/// Icon URLs for a corporation.
#[derive(Debug, Deserialize, Serialize)]
pub struct CorporationIcons {
  pub px128x128: Option<String>,
  pub px256x256: Option<String>,
  pub px64x64: Option<String>,
}

/// One entry in a corporation's alliance history.
#[derive(Debug, Deserialize, Serialize)]
pub struct AllianceHistoryEntry {
  pub alliance_id: Option<i64>,
  pub is_deleted: Option<bool>,
  pub record_id: i64,
  pub start_date: String,
}

/// A corporation contact.
#[derive(Debug, Deserialize, Serialize)]
pub struct CorporationContact {
  pub contact_id: i64,
  pub contact_type: String,
  pub is_watched: Option<bool>,
  pub label_ids: Option<Vec<i64>>,
  pub standing: f64,
}

/// A corporation contact label.
#[derive(Debug, Deserialize, Serialize)]
pub struct CorporationContactLabel {
  pub label_id: i64,
  pub label_name: String,
}

/// A container access log entry.
#[derive(Debug, Deserialize, Serialize)]
pub struct ContainerLog {
  pub action: String,
  pub character_id: i64,
  pub container_id: i64,
  pub container_type_id: i32,
  pub location_flag: String,
  pub location_id: i64,
  pub logged_at: String,
  pub new_config_bitmask: Option<i32>,
  pub old_config_bitmask: Option<i32>,
  pub password_type: Option<String>,
  pub quantity: Option<i32>,
  pub type_id: Option<i32>,
}

/// A corporation contract.
#[derive(Debug, Deserialize, Serialize)]
pub struct CorporationContract {
  pub acceptor_id: i64,
  pub assignee_id: i64,
  pub availability: String,
  pub buyout: Option<f64>,
  pub collateral: Option<f64>,
  pub contract_id: i64,
  pub date_accepted: Option<String>,
  pub date_completed: Option<String>,
  pub date_expired: String,
  pub date_issued: String,
  pub days_to_complete: Option<i32>,
  pub end_location_id: Option<i64>,
  pub for_corporation: bool,
  pub issuer_corporation_id: i64,
  pub issuer_id: i64,
  pub price: Option<f64>,
  pub reward: Option<f64>,
  pub start_location_id: Option<i64>,
  pub status: String,
  pub title: Option<String>,
  pub r#type: String,
  pub volume: Option<f64>,
}

/// A bid on a corporation contract.
#[derive(Debug, Deserialize, Serialize)]
pub struct ContractBid {
  pub amount: f64,
  pub bid_id: i64,
  pub bidder_id: i64,
  pub date_bid: String,
}

/// An item in a corporation contract.
#[derive(Debug, Deserialize, Serialize)]
pub struct ContractItem {
  pub is_included: bool,
  pub is_singleton: bool,
  pub quantity: i32,
  pub raw_quantity: Option<i32>,
  pub record_id: i64,
  pub type_id: i32,
}

/// A customs office.
#[derive(Debug, Deserialize, Serialize)]
pub struct CustomsOffice {
  pub alliance_tax_rate: Option<f64>,
  pub allow_access_with_standings: bool,
  pub allow_alliance_access: bool,
  pub bad_standing_tax_rate: Option<f64>,
  pub corporation_tax_rate: Option<f64>,
  pub excellent_standing_tax_rate: Option<f64>,
  pub good_standing_tax_rate: Option<f64>,
  pub neutral_standing_tax_rate: Option<f64>,
  pub office_id: i64,
  pub reinforce_exit_end: i32,
  pub reinforce_exit_start: i32,
  pub solar_system_id: i64,
  pub standing_level: Option<String>,
  pub terrible_standing_tax_rate: Option<f64>,
}

/// Faction warfare stats for a corporation.
#[derive(Debug, Deserialize, Serialize)]
pub struct CorporationFwStats {
  pub enlisted_on: Option<String>,
  pub faction_id: Option<i64>,
  pub kills: crate::models::character::FwKills,
  pub pilots: Option<i32>,
  pub victory_points: crate::models::character::FwVictoryPoints,
}

/// A corporation industry job.
#[derive(Debug, Deserialize, Serialize)]
pub struct CorporationIndustryJob {
  pub activity_id: i32,
  pub blueprint_id: i64,
  pub blueprint_location_id: i64,
  pub blueprint_type_id: i32,
  pub completed_character_id: Option<i64>,
  pub completed_date: Option<String>,
  pub cost: Option<f64>,
  pub duration: i32,
  pub end_date: String,
  pub facility_id: i64,
  pub installer_id: i64,
  pub job_id: i64,
  pub licensed_runs: Option<i32>,
  pub location_id: i64,
  pub output_location_id: i64,
  pub pause_date: Option<String>,
  pub probability: Option<f64>,
  pub product_type_id: Option<i32>,
  pub runs: i32,
  pub start_date: String,
  pub status: String,
  pub successful_runs: Option<i32>,
}

/// A moon mining extraction.
#[derive(Debug, Deserialize, Serialize)]
pub struct MiningExtraction {
  pub chunk_arrival_time: String,
  pub extraction_start_time: String,
  pub moon_id: i64,
  pub natural_decay_time: String,
  pub structure_id: i64,
}

/// A mining observer.
#[derive(Debug, Deserialize, Serialize)]
pub struct MiningObserver {
  pub last_updated: String,
  pub observer_id: i64,
  pub observer_type: String,
}

/// A mining observer ledger entry.
#[derive(Debug, Deserialize, Serialize)]
pub struct MiningObserverEntry {
  pub character_id: i64,
  pub last_updated: String,
  pub quantity: i64,
  pub recorded_corporation_id: i64,
  pub type_id: i32,
}

/// Roles assigned to a corporation member.
#[derive(Debug, Deserialize, Serialize)]
pub struct MemberRoles {
  pub character_id: i64,
  pub grantable_roles: Option<Vec<String>>,
  pub grantable_roles_at_base: Option<Vec<String>>,
  pub grantable_roles_at_hq: Option<Vec<String>>,
  pub grantable_roles_at_other: Option<Vec<String>>,
  pub roles: Option<Vec<String>>,
  pub roles_at_base: Option<Vec<String>>,
  pub roles_at_hq: Option<Vec<String>>,
  pub roles_at_other: Option<Vec<String>>,
}

/// Titles held by a corporation member.
#[derive(Debug, Deserialize, Serialize)]
pub struct MemberTitleEntry {
  pub character_id: i64,
  pub titles: Option<Vec<i64>>,
}

/// Tracking data for a corporation member.
#[derive(Debug, Deserialize, Serialize)]
pub struct MemberTracking {
  pub base_id: Option<i64>,
  pub character_id: i64,
  pub location_id: Option<i64>,
  pub logoff_date: Option<String>,
  pub logon_date: Option<String>,
  pub ship_type_id: Option<i32>,
  pub start_date: Option<String>,
}

/// Role history entry for a corporation member.
#[derive(Debug, Deserialize, Serialize)]
pub struct RoleHistoryEntry {
  pub changed_at: String,
  pub character_id: i64,
  pub issuer_id: i64,
  pub new_roles: Vec<String>,
  pub old_roles: Vec<String>,
  pub role_type: String,
}

/// Wallet and hangar division names for a corporation.
#[derive(Debug, Deserialize, Serialize)]
pub struct CorporationDivisions {
  pub hangar: Option<Vec<DivisionEntry>>,
  pub wallet: Option<Vec<DivisionEntry>>,
}

/// A named division entry.
#[derive(Debug, Deserialize, Serialize)]
pub struct DivisionEntry {
  pub division: i32,
  pub name: Option<String>,
}

/// A medal created by a corporation.
#[derive(Debug, Deserialize, Serialize)]
pub struct CorporationMedal {
  pub created_at: String,
  pub creator_id: i64,
  pub description: String,
  pub medal_id: i64,
  pub title: String,
}

/// A standing entry for a corporation.
#[derive(Debug, Deserialize, Serialize)]
pub struct CorporationStanding {
  pub from_id: i64,
  pub from_type: String,
  pub standing: f64,
}

/// A structure owned by a corporation.
#[derive(Debug, Deserialize, Serialize)]
pub struct CorporationStructure {
  pub corporation_id: i64,
  pub fuel_expires: Option<String>,
  pub next_reinforce_apply: Option<String>,
  pub next_reinforce_hour: Option<i32>,
  pub profile_id: i32,
  pub reinforce_hour: i32,
  pub services: Option<Vec<StructureService>>,
  pub state: String,
  pub state_timer_end: Option<String>,
  pub state_timer_start: Option<String>,
  pub structure_id: i64,
  pub system_id: i64,
  pub type_id: i32,
  pub unanchors_at: Option<String>,
}

/// A service running on a structure.
#[derive(Debug, Deserialize, Serialize)]
pub struct StructureService {
  pub name: String,
  pub state: String,
}

/// A title defined in a corporation.
#[derive(Debug, Deserialize, Serialize)]
pub struct CorporationTitle {
  pub grantable_roles: Option<Vec<String>>,
  pub grantable_roles_at_base: Option<Vec<String>>,
  pub grantable_roles_at_hq: Option<Vec<String>>,
  pub grantable_roles_at_other: Option<Vec<String>>,
  pub name: Option<String>,
  pub roles: Option<Vec<String>>,
  pub roles_at_base: Option<Vec<String>>,
  pub roles_at_hq: Option<Vec<String>>,
  pub roles_at_other: Option<Vec<String>>,
  pub title_id: Option<i64>,
}

/// A facility used by a corporation.
#[derive(Debug, Deserialize, Serialize)]
pub struct Facility {
  pub facility_id: i64,
  pub solar_system_id: i64,
  pub type_id: i32,
}

/// A medal issued by a corporation to a member.
#[derive(Debug, Deserialize, Serialize)]
pub struct IssuedMedal {
  pub character_id: i64,
  pub issued_at: String,
  pub issuer_id: i64,
  pub medal_id: i64,
  pub reason: String,
  pub status: String,
}

/// A corporation starbase (POS).
#[derive(Debug, Deserialize, Serialize)]
pub struct Starbase {
  pub moon_id: i64,
  pub onlined_since: Option<String>,
  pub reinforced_until: Option<String>,
  pub starbase_id: i64,
  pub state: Option<String>,
  pub system_id: i64,
  pub type_id: i32,
  pub unanchor_at: Option<String>,
}

/// Detailed information about a starbase.
#[derive(Debug, Deserialize, Serialize)]
pub struct StarbaseDetail {
  pub allow_alliance_members: bool,
  pub allow_corporation_members: bool,
  pub anchor: String,
  pub attack_if_at_war: bool,
  pub attack_if_other_security_status_dropping: bool,
  pub attack_security_status_threshold: Option<f64>,
  pub attack_standing_threshold: Option<f64>,
  pub fuel_bay_take: String,
  pub fuel_bay_view: String,
  pub fuels: Option<Vec<StarbaseFuel>>,
  pub offline: String,
  pub online: String,
  pub unanchor: String,
  pub use_alliance_standings: bool,
}

/// Fuel for a starbase.
#[derive(Debug, Deserialize, Serialize)]
pub struct StarbaseFuel {
  pub quantity: i32,
  pub type_id: i32,
}

/// A corporation market order.
#[derive(Debug, Deserialize, Serialize)]
pub struct CorporationOrder {
  pub duration: i32,
  pub escrow: Option<f64>,
  pub is_buy_order: Option<bool>,
  pub issued: String,
  pub issued_by: i64,
  pub location_id: i64,
  pub min_volume: Option<i32>,
  pub order_id: i64,
  pub price: f64,
  pub range: String,
  pub region_id: i64,
  pub type_id: i32,
  pub volume_remain: i32,
  pub volume_total: i32,
  pub wallet_division: i32,
}

/// A corporation wallet.
#[derive(Debug, Deserialize, Serialize)]
pub struct CorporationWallet {
  pub balance: f64,
  pub division: i32,
}

/// A corporation wallet journal entry.
#[derive(Debug, Deserialize, Serialize)]
pub struct CorporationWalletJournalEntry {
  pub amount: Option<f64>,
  pub balance: Option<f64>,
  pub context_id: Option<i64>,
  pub context_id_type: Option<String>,
  pub date: String,
  pub description: String,
  pub first_party_id: Option<i64>,
  pub id: i64,
  pub reason: Option<String>,
  pub ref_type: String,
  pub second_party_id: Option<i64>,
  pub tax: Option<f64>,
  pub tax_receiver_id: Option<i64>,
}

/// A corporation wallet transaction.
#[derive(Debug, Deserialize, Serialize)]
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

/// A corporation shareholder.
#[derive(Debug, Deserialize, Serialize)]
pub struct Shareholder {
  pub share_count: i64,
  pub shareholder_id: i64,
  pub shareholder_type: String,
}
