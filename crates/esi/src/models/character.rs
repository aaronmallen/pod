//! Character ESI response models.

use serde::{Deserialize, Serialize};

/// Character affiliation to alliance, corporation, and faction.
#[derive(Debug, Deserialize, Serialize)]
pub struct CharacterAffiliation {
  pub alliance_id: Option<i64>,
  pub character_id: i64,
  pub corporation_id: i64,
  pub faction_id: Option<i64>,
}

/// Public information about a character.
#[derive(Debug, Deserialize, Serialize)]
pub struct CharacterDetail {
  pub alliance_id: Option<i64>,
  pub birthday: String,
  pub bloodline_id: i32,
  pub corporation_id: i64,
  pub description: Option<String>,
  pub gender: String,
  pub name: String,
  pub race_id: i32,
  pub security_status: Option<f64>,
  pub title: Option<String>,
}

/// Portrait image URLs for a character.
#[derive(Debug, Deserialize, Serialize)]
pub struct CharacterPortrait {
  pub px1024x1024: Option<String>,
  pub px128x128: Option<String>,
  pub px256x256: Option<String>,
  pub px512x512: Option<String>,
  pub px64x64: Option<String>,
}

/// One entry in a character's corporation history.
#[derive(Debug, Deserialize, Serialize)]
pub struct CorporationHistoryEntry {
  pub corporation_id: i64,
  pub is_deleted: Option<bool>,
  pub record_id: i64,
  pub start_date: String,
}

/// A character asset item.
#[derive(Debug, Deserialize, Serialize)]
pub struct Asset {
  pub is_blueprint_copy: Option<bool>,
  pub is_singleton: bool,
  pub item_id: i64,
  pub location_flag: String,
  pub location_id: i64,
  pub location_type: String,
  pub quantity: i32,
  pub type_id: i32,
}

/// Location coordinates for an asset.
#[derive(Debug, Deserialize, Serialize)]
pub struct AssetLocation {
  pub item_id: i64,
  pub position: AssetPosition,
}

/// 3D position of an asset.
#[derive(Debug, Deserialize, Serialize)]
pub struct AssetPosition {
  pub x: f64,
  pub y: f64,
  pub z: f64,
}

/// Name of an asset item.
#[derive(Debug, Deserialize, Serialize)]
pub struct AssetName {
  pub item_id: i64,
  pub name: String,
}

/// A character blueprint.
#[derive(Debug, Deserialize, Serialize)]
pub struct Blueprint {
  pub item_id: i64,
  pub location_flag: String,
  pub location_id: i64,
  pub material_efficiency: i32,
  pub quantity: i32,
  pub runs: i32,
  pub time_efficiency: i32,
  pub type_id: i32,
}

/// A calendar event summary.
#[derive(Debug, Deserialize, Serialize)]
pub struct CalendarEvent {
  pub duration: Option<i32>,
  pub event_date: Option<String>,
  pub event_id: i64,
  pub event_response: Option<String>,
  pub importance: i32,
  pub title: String,
}

/// An attendee of a calendar event.
#[derive(Debug, Deserialize, Serialize)]
pub struct CalendarAttendee {
  pub character_id: Option<i64>,
  pub event_response: Option<String>,
}

/// Full detail of a calendar event.
#[derive(Debug, Deserialize, Serialize)]
pub struct CalendarEventDetail {
  pub date: String,
  pub duration: i32,
  pub event_id: i64,
  pub importance: i32,
  pub owner_id: i64,
  pub owner_name: String,
  pub owner_type: String,
  pub response: String,
  pub text: String,
  pub title: String,
}

/// Response value for responding to a calendar event.
#[derive(Debug, Deserialize, Serialize)]
pub struct CalendarResponse {
  pub response: String,
}

/// Clone information for a character.
#[derive(Debug, Deserialize, Serialize)]
pub struct Clones {
  pub home_location: Option<CloneLocation>,
  pub jump_clones: Vec<JumpClone>,
  pub last_clone_jump_date: Option<String>,
  pub last_station_change_date: Option<String>,
}

/// Location of a clone.
#[derive(Debug, Deserialize, Serialize)]
pub struct CloneLocation {
  pub location_id: Option<i64>,
  pub location_type: Option<String>,
}

/// A jump clone.
#[derive(Debug, Deserialize, Serialize)]
pub struct JumpClone {
  pub clone_id: Option<i64>,
  #[serde(default)]
  pub implants: Vec<i32>,
  pub location_id: i64,
  pub location_type: String,
  pub name: Option<String>,
}

/// A saved ship fitting.
#[derive(Debug, Deserialize, Serialize)]
pub struct Fitting {
  pub description: String,
  pub fitting_id: i64,
  pub items: Vec<FittingItem>,
  pub name: String,
  pub ship_type_id: i32,
}

/// An item within a fitting.
#[derive(Debug, Deserialize, Serialize)]
pub struct FittingItem {
  pub flag: String,
  pub quantity: i32,
  pub type_id: i32,
}

/// ID returned when creating a new fitting.
#[derive(Debug, Deserialize, Serialize)]
pub struct FittingId {
  pub fitting_id: i64,
}

/// Body for creating a new fitting.
#[derive(Debug, Deserialize, Serialize)]
pub struct NewFitting {
  pub description: String,
  pub items: Vec<FittingItem>,
  pub name: String,
  pub ship_type_id: i32,
}

/// A contact in a character contact list.
#[derive(Debug, Deserialize, Serialize)]
pub struct CharacterContact {
  pub contact_id: i64,
  pub contact_type: String,
  pub is_blocked: Option<bool>,
  pub is_watched: Option<bool>,
  pub label_ids: Option<Vec<i64>>,
  pub standing: f64,
}

/// A contact label.
#[derive(Debug, Deserialize, Serialize)]
pub struct ContactLabel {
  pub label_id: i64,
  pub label_name: String,
}

/// Body for adding contacts.
#[derive(Debug, Deserialize, Serialize)]
pub struct AddContacts {
  pub contact_ids: Vec<i64>,
  pub label_ids: Option<Vec<i64>>,
  pub standing: f64,
  pub watched: Option<bool>,
}

/// Body for updating contacts.
#[derive(Debug, Deserialize, Serialize)]
pub struct UpdateContacts {
  pub contact_ids: Vec<i64>,
  pub label_ids: Option<Vec<i64>>,
  pub standing: f64,
  pub watched: Option<bool>,
}

/// A character contract.
#[derive(Debug, Deserialize, Serialize)]
pub struct CharacterContract {
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

/// A bid on a contract.
#[derive(Debug, Deserialize, Serialize)]
pub struct ContractBid {
  pub amount: f64,
  pub bid_id: i64,
  pub bidder_id: i64,
  pub date_bid: String,
}

/// An item in a contract.
#[derive(Debug, Deserialize, Serialize)]
pub struct ContractItem {
  pub is_included: bool,
  pub is_singleton: bool,
  pub quantity: i32,
  pub raw_quantity: Option<i32>,
  pub record_id: i64,
  pub type_id: i32,
}

/// Industry job for a character.
#[derive(Debug, Deserialize, Serialize)]
pub struct IndustryJob {
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
  pub output_location_id: i64,
  pub pause_date: Option<String>,
  pub probability: Option<f64>,
  pub product_type_id: Option<i32>,
  pub runs: i32,
  pub start_date: String,
  pub station_id: i64,
  pub status: String,
  pub successful_runs: Option<i32>,
}

/// Agent research data for a character.
#[derive(Debug, Deserialize, Serialize)]
pub struct AgentResearch {
  pub agent_id: i64,
  pub points_per_day: f64,
  pub remainder_points: f64,
  pub skill_type_id: i32,
  pub started_at: String,
}

/// Jump fatigue data for a character.
#[derive(Debug, Deserialize, Serialize)]
pub struct JumpFatigue {
  pub jump_fatigue_expire_date: Option<String>,
  pub last_jump_date: Option<String>,
  pub last_update_date: Option<String>,
}

/// A mining ledger entry.
#[derive(Debug, Deserialize, Serialize)]
pub struct MiningEntry {
  pub date: String,
  pub quantity: i64,
  pub solar_system_id: i64,
  pub type_id: i32,
}

/// Fleet membership info for a character.
#[derive(Debug, Deserialize, Serialize)]
pub struct CharacterFleet {
  pub fleet_id: i64,
  pub role: String,
  pub squad_id: i64,
  pub wing_id: i64,
}

/// Current location of a character.
#[derive(Debug, Deserialize, Serialize)]
pub struct CharacterLocation {
  pub solar_system_id: i64,
  pub station_id: Option<i64>,
  pub structure_id: Option<i64>,
}

/// Online status of a character.
#[derive(Debug, Deserialize, Serialize)]
pub struct CharacterOnline {
  pub last_login: Option<String>,
  pub last_logout: Option<String>,
  pub logins: Option<i32>,
  pub online: bool,
}

/// Current ship a character is flying.
#[derive(Debug, Deserialize, Serialize)]
pub struct CharacterShip {
  pub ship_item_id: i64,
  pub ship_name: String,
  pub ship_type_id: i32,
}

/// A loyalty point balance entry.
#[derive(Debug, Deserialize, Serialize)]
pub struct LoyaltyPoint {
  pub corporation_id: i64,
  pub loyalty_points: i32,
}

/// A recent killmail reference.
#[derive(Debug, Deserialize, Serialize)]
pub struct RecentKillmail {
  pub killmail_hash: String,
  pub killmail_id: i64,
}

/// Search results across multiple entity categories.
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct SearchResults {
  pub agents: Option<Vec<i64>>,
  pub alliances: Option<Vec<i64>>,
  pub characters: Option<Vec<i64>>,
  pub constellations: Option<Vec<i64>>,
  pub corporations: Option<Vec<i64>>,
  pub factions: Option<Vec<i64>>,
  pub inventory_types: Option<Vec<i64>>,
  pub regions: Option<Vec<i64>>,
  pub solar_systems: Option<Vec<i64>>,
  pub stations: Option<Vec<i64>>,
  pub structures: Option<Vec<i64>>,
}

/// Mail header summary.
#[derive(Debug, Deserialize, Serialize)]
pub struct MailHeader {
  pub from: Option<i64>,
  pub is_read: Option<bool>,
  pub labels: Option<Vec<i64>>,
  pub mail_id: Option<i64>,
  pub recipients: Option<Vec<MailRecipient>>,
  pub subject: Option<String>,
  pub timestamp: Option<String>,
}

/// A mail recipient.
#[derive(Debug, Deserialize, Serialize)]
pub struct MailRecipient {
  pub recipient_id: i64,
  pub recipient_type: String,
}

/// Mail labels and unread count.
#[derive(Debug, Deserialize, Serialize)]
pub struct MailLabels {
  pub labels: Option<Vec<MailLabel>>,
  pub total_unread_count: Option<i32>,
}

/// A mail label.
#[derive(Debug, Deserialize, Serialize)]
pub struct MailLabel {
  pub color: Option<String>,
  pub label_id: Option<i64>,
  pub name: Option<String>,
  pub unread_count: Option<i32>,
}

/// A mailing list subscription.
#[derive(Debug, Deserialize, Serialize)]
pub struct MailList {
  pub mailing_list_id: i64,
  pub name: String,
}

/// Full contents of a mail message.
#[derive(Debug, Deserialize, Serialize)]
pub struct MailMessage {
  pub body: Option<String>,
  pub from: Option<i64>,
  pub labels: Option<Vec<i64>>,
  pub read: Option<bool>,
  pub recipients: Option<Vec<MailRecipient>>,
  pub subject: Option<String>,
  pub timestamp: Option<String>,
}

/// Body for sending a new mail.
#[derive(Debug, Deserialize, Serialize)]
pub struct NewMail {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub approved_cost: Option<i64>,
  pub body: String,
  pub recipients: Vec<MailRecipient>,
  pub subject: String,
}

/// Body for creating a new mail label.
#[derive(Debug, Deserialize, Serialize)]
pub struct NewMailLabel {
  pub color: Option<String>,
  pub name: String,
}

/// Body for updating a mail's read state or labels.
#[derive(Debug, Deserialize, Serialize)]
pub struct UpdateMail {
  pub labels: Option<Vec<i64>>,
  pub read: Option<bool>,
}

/// Attribute point allocation for a character.
#[derive(Debug, Deserialize, Serialize)]
pub struct CharacterAttributes {
  pub accrued_remap_cooldown_date: Option<String>,
  pub bonus_remaps: Option<i32>,
  pub charisma: i32,
  pub intelligence: i32,
  pub last_remap_date: Option<String>,
  pub memory: i32,
  pub perception: i32,
  pub willpower: i32,
}

/// Faction warfare stats for a character.
#[derive(Debug, Deserialize, Serialize)]
pub struct CharacterFwStats {
  pub current_rank: Option<i32>,
  pub enlisted_on: Option<String>,
  pub faction_id: Option<i64>,
  pub highest_rank: Option<i32>,
  pub kills: FwKills,
  pub victory_points: FwVictoryPoints,
}

/// Kill counts grouped by time range.
#[derive(Debug, Deserialize, Serialize)]
pub struct FwKills {
  pub last_week: i32,
  pub total: i32,
  pub yesterday: i32,
}

/// Victory point counts grouped by time range.
#[derive(Debug, Deserialize, Serialize)]
pub struct FwVictoryPoints {
  pub last_week: i32,
  pub total: i32,
  pub yesterday: i32,
}

/// A medal awarded to a character.
#[derive(Debug, Deserialize, Serialize)]
pub struct CharacterMedal {
  pub corporation_id: i64,
  pub date: String,
  pub description: String,
  pub graphics: Vec<serde_json::Value>,
  pub issuer_id: i64,
  pub medal_id: i64,
  pub reason: String,
  pub status: String,
  pub title: String,
}

/// A character notification.
#[derive(Debug, Deserialize, Serialize)]
pub struct CharacterNotification {
  pub is_read: Option<bool>,
  pub notification_id: i64,
  pub sender_id: i64,
  pub sender_type: String,
  pub text: Option<String>,
  pub timestamp: String,
  pub r#type: String,
}

/// A planet managed by a character.
#[derive(Debug, Deserialize, Serialize)]
pub struct CharacterPlanet {
  pub last_update: String,
  pub num_pins: i32,
  pub owner_id: i64,
  pub planet_id: i64,
  pub planet_type: String,
  pub solar_system_id: i64,
  pub upgrade_level: i32,
}

/// Corporation roles held by a character.
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct CharacterRoles {
  pub roles: Option<Vec<String>>,
  pub roles_at_base: Option<Vec<String>>,
  pub roles_at_hq: Option<Vec<String>>,
  pub roles_at_other: Option<Vec<String>>,
}

/// Skill training data for a character.
#[derive(Debug, Deserialize, Serialize)]
pub struct CharacterSkills {
  pub skills: Vec<SkillEntry>,
  pub total_sp: i64,
  pub unallocated_sp: Option<i32>,
}

/// A single trained skill entry.
#[derive(Debug, Deserialize, Serialize)]
pub struct SkillEntry {
  pub active_skill_level: i32,
  pub skill_id: i32,
  pub skillpoints_in_skill: i64,
  pub trained_skill_level: i32,
}

/// A standing entry for a character.
#[derive(Debug, Deserialize, Serialize)]
pub struct CharacterStanding {
  pub from_id: i64,
  pub from_type: String,
  pub standing: f64,
}

/// A title held by a character.
#[derive(Debug, Deserialize, Serialize)]
pub struct CharacterTitle {
  pub name: Option<String>,
  pub title_id: Option<i64>,
}

/// A contact notification.
#[derive(Debug, Deserialize, Serialize)]
pub struct ContactNotification {
  pub message: String,
  pub notification_id: i64,
  pub send_date: String,
  pub sender_character_id: i64,
  pub standing_level: f64,
}

/// Colony details for a planetary installation.
#[derive(Debug, Deserialize, Serialize)]
pub struct PlanetColony {
  pub links: Vec<serde_json::Value>,
  pub pins: Vec<serde_json::Value>,
  pub routes: Vec<serde_json::Value>,
}

/// An entry in the skill training queue.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SkillQueueEntry {
  pub finish_date: Option<String>,
  pub finished_level: i32,
  pub level_end_sp: Option<i32>,
  pub level_start_sp: Option<i32>,
  pub queue_position: i32,
  pub skill_id: i32,
  pub start_date: Option<String>,
  pub training_start_sp: Option<i32>,
}

/// A character market order.
#[derive(Debug, Deserialize, Serialize)]
pub struct CharacterOrder {
  pub duration: i32,
  pub escrow: Option<f64>,
  pub is_buy_order: Option<bool>,
  pub is_corporation: bool,
  pub issued: String,
  pub location_id: i64,
  pub min_volume: Option<i32>,
  pub order_id: i64,
  pub price: f64,
  pub range: String,
  pub region_id: i64,
  pub type_id: i32,
  pub volume_remain: i32,
  pub volume_total: i32,
}

/// Wallet balance for a character.
#[derive(Debug, Deserialize, Serialize)]
pub struct CharacterWalletBalance(pub f64);

/// A wallet journal entry.
#[derive(Debug, Deserialize, Serialize)]
pub struct WalletJournalEntry {
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

/// A wallet transaction.
#[derive(Debug, Deserialize, Serialize)]
pub struct WalletTransaction {
  pub client_id: i64,
  pub date: String,
  pub is_buy: bool,
  pub is_personal: bool,
  pub journal_ref_id: i64,
  pub location_id: i64,
  pub quantity: i32,
  pub transaction_id: i64,
  pub type_id: i32,
  pub unit_price: f64,
}
