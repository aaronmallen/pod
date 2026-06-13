use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct Asset {
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

#[derive(Debug, Deserialize)]
pub struct Attributes {
  #[serde(default)]
  pub accrued_remap_cooldown_date: Option<String>,
  #[serde(default)]
  pub bonus_remaps: i32,
  pub charisma: i32,
  pub intelligence: i32,
  #[serde(default)]
  pub last_remap_date: Option<String>,
  pub memory: i32,
  pub perception: i32,
  pub willpower: i32,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct CalendarAttendee {
  #[serde(default)]
  pub character_id: Option<i64>,
  #[serde(default)]
  pub event_response: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct CalendarEvent {
  #[serde(default)]
  pub event_date: Option<String>,
  pub event_id: i64,
  #[serde(default)]
  pub event_response: Option<String>,
  #[serde(default)]
  pub importance: Option<i32>,
  #[serde(default)]
  pub title: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct CalendarEventDetail {
  #[serde(default)]
  pub date: Option<String>,
  #[serde(default)]
  pub duration: Option<i32>,
  pub event_id: i64,
  #[serde(default)]
  pub importance: Option<i32>,
  #[serde(default)]
  pub owner_id: Option<i64>,
  #[serde(default)]
  pub owner_name: Option<String>,
  #[serde(default)]
  pub owner_type: Option<String>,
  #[serde(default)]
  pub response: Option<String>,
  #[serde(default)]
  pub text: Option<String>,
  #[serde(default)]
  pub title: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CharacterInfo {
  #[serde(default)]
  pub alliance_id: Option<i64>,
  pub birthday: String,
  pub bloodline_id: i32,
  pub corporation_id: i64,
  #[serde(default)]
  pub description: Option<String>,
  #[serde(default)]
  pub faction_id: Option<i64>,
  pub gender: String,
  pub name: String,
  pub race_id: i32,
  #[serde(default)]
  pub security_status: Option<f64>,
  #[serde(default)]
  pub title: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct CharacterSkills {
  pub skills: Vec<Skill>,
  pub total_sp: i64,
  #[serde(default)]
  pub unallocated_sp: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct CloneHomeLocation {
  #[serde(default)]
  pub location_id: Option<i64>,
  #[serde(default)]
  pub location_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Clones {
  pub home_location: CloneHomeLocation,
  #[serde(default)]
  pub jump_clones: Vec<JumpClone>,
  #[serde(default)]
  pub last_clone_jump_date: Option<String>,
  #[serde(default)]
  pub last_station_change_date: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Contact {
  pub contact_id: i64,
  pub contact_type: String,
  #[serde(default)]
  pub is_blocked: Option<bool>,
  #[serde(default)]
  pub is_watched: Option<bool>,
  #[serde(default)]
  pub label_ids: Vec<i64>,
  #[serde(default)]
  pub standing: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct ContactLabel {
  pub label_id: i64,
  pub label_name: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct Contract {
  #[serde(default)]
  pub acceptor_id: Option<i64>,
  #[serde(default)]
  pub assignee_id: Option<i64>,
  #[serde(default)]
  pub availability: Option<String>,
  #[serde(default)]
  pub buyout: Option<f64>,
  #[serde(default)]
  pub collateral: Option<f64>,
  pub contract_id: i64,
  #[serde(default)]
  pub date_accepted: Option<String>,
  #[serde(default)]
  pub date_completed: Option<String>,
  #[serde(default)]
  pub date_expired: Option<String>,
  #[serde(default)]
  pub date_issued: Option<String>,
  #[serde(default)]
  pub days_to_complete: Option<i32>,
  #[serde(default)]
  pub end_location_id: Option<i64>,
  #[serde(default)]
  pub for_corporation: Option<bool>,
  #[serde(default)]
  pub issuer_corporation_id: Option<i64>,
  #[serde(default)]
  pub issuer_id: Option<i64>,
  #[serde(default)]
  pub price: Option<f64>,
  #[serde(default)]
  pub reward: Option<f64>,
  #[serde(default)]
  pub start_location_id: Option<i64>,
  #[serde(default)]
  pub status: Option<String>,
  #[serde(default)]
  pub title: Option<String>,
  #[serde(rename = "type", default)]
  pub contract_type: Option<String>,
  #[serde(default)]
  pub volume: Option<f64>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct CreateMailLabelRequest {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub color: Option<String>,
  pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct JumpClone {
  #[serde(default)]
  pub implants: Vec<i32>,
  pub jump_clone_id: i64,
  pub location_id: i64,
  pub location_type: String,
  #[serde(default)]
  pub name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Location {
  pub solar_system_id: i64,
  #[serde(default)]
  pub station_id: Option<i64>,
  #[serde(default)]
  pub structure_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct MailBody {
  pub body: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct MailHeader {
  #[serde(default)]
  pub from: Option<i64>,
  #[serde(default)]
  pub is_read: Option<bool>,
  #[serde(default)]
  pub labels: Vec<i64>,
  pub mail_id: i64,
  #[serde(default)]
  pub recipients: Vec<MailRecipient>,
  #[serde(default)]
  pub subject: Option<String>,
  #[serde(default)]
  pub timestamp: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct MailLabel {
  #[serde(default)]
  pub color: Option<String>,
  pub label_id: i64,
  #[serde(default)]
  pub name: Option<String>,
  #[serde(default)]
  pub unread_count: Option<i64>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct MailLabels {
  #[serde(default)]
  pub labels: Vec<MailLabel>,
  #[serde(default)]
  pub total_unread_count: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct MailRecipient {
  pub recipient_id: i64,
  pub recipient_type: String,
}

#[derive(Debug, Serialize)]
pub struct MarkReadRequest {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub labels: Option<Vec<i64>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub read: Option<bool>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct MarketOrder {
  pub duration: i64,
  #[serde(default)]
  pub escrow: f64,
  #[serde(default)]
  pub is_buy_order: bool,
  pub issued: String,
  pub location_id: i64,
  #[serde(default)]
  pub min_volume: Option<i64>,
  pub order_id: i64,
  pub price: f64,
  pub range: String,
  pub region_id: i64,
  pub type_id: i64,
  pub volume_remain: i64,
  pub volume_total: i64,
}

#[derive(Debug, Deserialize)]
pub struct Notification {
  #[serde(default)]
  pub is_read: Option<bool>,
  #[serde(rename = "type")]
  pub notif_type: String,
  pub notification_id: i64,
  #[serde(default)]
  pub sender_id: Option<i64>,
  #[serde(default)]
  pub sender_type: Option<String>,
  #[serde(default)]
  pub text: Option<String>,
  pub timestamp: String,
}

#[derive(Debug, Deserialize)]
pub struct Online {
  pub online: bool,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct RecentKillmail {
  pub killmail_hash: String,
  pub killmail_id: i64,
}

#[derive(Debug, Serialize)]
pub struct RespondRequest {
  pub response: String,
}

#[derive(Debug, Serialize)]
pub struct SendMailRecipient {
  pub recipient_id: i64,
  pub recipient_type: String,
}

#[derive(Debug, Serialize)]
pub struct SendMailRequest {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub approved_cost: Option<i64>,
  pub body: String,
  pub recipients: Vec<SendMailRecipient>,
  pub subject: String,
}

#[derive(Debug, Deserialize)]
pub struct Ship {
  pub ship_item_id: i64,
  pub ship_name: String,
  pub ship_type_id: i32,
}

#[derive(Debug, Deserialize)]
pub struct Skill {
  pub active_skill_level: i32,
  pub skill_id: i32,
  pub skillpoints_in_skill: i64,
  pub trained_skill_level: i32,
}

#[derive(Debug, Deserialize)]
pub struct SkillQueueEntry {
  #[serde(default)]
  pub finish_date: Option<String>,
  pub finished_level: i32,
  #[serde(default)]
  pub level_end_sp: Option<i64>,
  #[serde(default)]
  pub level_start_sp: Option<i64>,
  pub queue_position: i32,
  pub skill_id: i32,
  #[serde(default)]
  pub start_date: Option<String>,
  #[serde(default)]
  pub training_start_sp: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct Standing {
  pub from_id: i64,
  pub from_type: String,
  pub standing: f64,
}

#[derive(Debug, Deserialize)]
pub struct WalletJournalEntry {
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
