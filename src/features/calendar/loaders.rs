use std::collections::HashMap;

use chrono::{DateTime, Duration, Utc};

use super::palette::{OwnerType, Response};
use crate::{
  config::{Feature, FeatureFlags, SubFeature},
  features::{
    industry::{Activity, Colony, colonies_for_character},
    skills::queue_timing::roman,
  },
  store::{
    Database, images,
    model::{
      AttendeeTally, CharacterCalendarEvent, CharacterContract, CharacterIndustryJob, CharacterSkillqueue,
      CorporationIndustryJob, CorporationMiningExtraction, MarketOrder, OwnerType as CredentialOwner,
    },
    repo::{calendar, character, finance, industry, infra, org, sde},
  },
};

const OVERLAY_OWNER: &str = "pod";
const SOURCE_COLONY_STORAGE: &str = "colony-storage";
const SOURCE_CONTRACT: &str = "contract";
/// Discriminant only: chunk-arrival and natural-decay events both display `source = SOURCE_EXTRACTION`,
/// but take distinct `synthetic_id` ranges so the two timers off one extraction never collide.
const SOURCE_EXTRACTION: &str = "extraction";
const SOURCE_EXTRACTION_ARRIVAL: &str = "extraction-arrival";
const SOURCE_EXTRACTION_DECAY: &str = "extraction-decay";
const SOURCE_INDUSTRY: &str = "industry";
/// Discriminant only: corp jobs display `source = SOURCE_INDUSTRY`, but get a distinct `synthetic_id`
/// range so character and corporation job-id spaces never collide.
const SOURCE_INDUSTRY_CORP: &str = "industry-corp";
const SOURCE_MARKET: &str = "market";
const SOURCE_SKILL: &str = "skill";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CalendarEvent {
  pub body: Option<String>,
  pub character_id: i64,
  pub duration_minutes: i64,
  pub event_id: i64,
  pub importance: i64,
  pub owner_name: String,
  pub owner_type: String,
  pub response: String,
  pub source: Option<String>,
  pub timestamp: String,
  pub title: String,
}

impl CalendarEvent {
  pub fn end(&self) -> Option<DateTime<Utc>> {
    self
      .start()
      .map(|start| start + chrono::Duration::minutes(self.duration_minutes.max(0)))
  }

  pub fn is_all_day(&self) -> bool {
    self.duration_minutes >= 1440
  }

  pub fn owner_kind(&self) -> OwnerType {
    OwnerType::from_esi(&self.owner_type)
  }

  #[cfg(test)]
  pub fn is_synthetic(&self) -> bool {
    self.owner_type == OVERLAY_OWNER
  }

  pub fn response_kind(&self) -> Response {
    Response::from_esi(&self.response)
  }

  pub fn start(&self) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(&self.timestamp)
      .ok()
      .map(|dt| dt.with_timezone(&Utc))
  }

  fn from_row(row: CharacterCalendarEvent) -> Self {
    CalendarEvent {
      body: row.body().clone(),
      character_id: row.character_id(),
      duration_minutes: row.duration_minutes(),
      event_id: row.event_id(),
      importance: row.importance(),
      owner_name: row.owner_name().to_owned(),
      owner_type: row.owner_type().to_owned(),
      response: row.response().to_owned(),
      source: None,
      timestamp: row.timestamp().to_owned(),
      title: row.title().to_owned(),
    }
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RosterPilot {
  pub corp: String,
  pub granted_scopes: Option<String>,
  pub id: i64,
  pub name: String,
  pub portrait: images::ImageState,
}

struct IndustryJobView<'a> {
  activity_id: i64,
  blueprint_type_id: i64,
  character_id: i64,
  completed_date: Option<&'a str>,
  cost: Option<f64>,
  end_date: &'a str,
  id_source: &'a str,
  job_id: i64,
  pause_date: Option<&'a str>,
  product_type_id: Option<i64>,
  runs: i64,
  status: &'a str,
}

#[derive(Debug, Default)]
struct TypeNames {
  cache: HashMap<i64, Option<String>>,
}

impl TypeNames {
  fn new() -> Self {
    TypeNames::default()
  }

  async fn resolve(&mut self, db: &Database, type_id: i64) -> Option<String> {
    if let Some(name) = self.cache.get(&type_id) {
      return name.clone();
    }
    let name = sde::get_item_type(db, type_id)
      .await
      .ok()
      .flatten()
      .map(|item| item.name().to_owned());
    self.cache.insert(type_id, name.clone());
    name
  }
}

pub(super) async fn load_attendees(db: &Database, character_id: i64, event_id: i64) -> Option<AttendeeTally> {
  let tally = calendar::attendee_tally(db, character_id, event_id).await.ok()?;
  (tally.invited > 0).then_some(tally)
}

pub(super) async fn load_combined(db: &Database) -> Vec<CalendarEvent> {
  calendar::combined(db)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(CalendarEvent::from_row)
    .collect()
}

pub(super) async fn load_events(db: &Database, character_id: i64) -> Vec<CalendarEvent> {
  calendar::events(db, character_id)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(CalendarEvent::from_row)
    .collect()
}

struct OverlaySources {
  colonies: bool,
  industry: bool,
  skills: bool,
  wallet: bool,
}

pub(super) async fn load_overlays(db: &Database, character_ids: &[i64], features: FeatureFlags) -> Vec<CalendarEvent> {
  let sources = OverlaySources {
    colonies: features.is_sub_enabled(SubFeature::Colonies),
    industry: features.is_enabled(Feature::Industry),
    skills: features.is_enabled(Feature::SkillMonitoring),
    wallet: features.is_enabled(Feature::Wallet),
  };
  if !sources.skills && !sources.wallet && !sources.industry && !sources.colonies {
    return Vec::new();
  }

  let mut names = TypeNames::new();
  let mut overlays = Vec::new();
  for &character_id in character_ids {
    collect_character_overlays(db, &mut names, character_id, &sources, &mut overlays).await;
  }
  if sources.industry {
    collect_corporation_overlays(db, &mut names, &mut overlays).await;
  }
  overlays
}

async fn collect_character_overlays(
  db: &Database,
  names: &mut TypeNames,
  character_id: i64,
  sources: &OverlaySources,
  overlays: &mut Vec<CalendarEvent>,
) {
  if sources.skills {
    overlays.extend(skill_overlays(db, names, character_id).await);
  }
  if sources.wallet {
    overlays.extend(market_overlays(db, names, character_id).await);
    overlays.extend(contract_overlays(db, character_id).await);
  }
  if sources.industry {
    overlays.extend(character_industry_overlays(db, names, character_id).await);
  }
  if sources.colonies {
    overlays.extend(colony_storage_overlays(db, character_id).await);
  }
}

async fn collect_corporation_overlays(db: &Database, names: &mut TypeNames, overlays: &mut Vec<CalendarEvent>) {
  for corporation in org::all_owned_corporations(db).await.unwrap_or_default() {
    overlays.extend(corporation_industry_overlays(db, names, corporation.id()).await);
    overlays.extend(corporation_extraction_overlays(db, corporation.id()).await);
  }
}

async fn skill_overlays(db: &Database, names: &mut TypeNames, character_id: i64) -> Vec<CalendarEvent> {
  let mut events = Vec::new();
  for entry in character::skillqueue(db, character_id).await.unwrap_or_default() {
    if let Some(event) = skill_overlay(db, names, &entry).await {
      events.push(event);
    }
  }
  events
}

async fn market_overlays(db: &Database, names: &mut TypeNames, character_id: i64) -> Vec<CalendarEvent> {
  let mut events = Vec::new();
  for order in finance::for_character(db, character_id).await.unwrap_or_default() {
    if let Some(event) = market_overlay(db, names, &order).await {
      events.push(event);
    }
  }
  events
}

async fn contract_overlays(db: &Database, character_id: i64) -> Vec<CalendarEvent> {
  finance::contracts(db, character_id)
    .await
    .unwrap_or_default()
    .iter()
    .filter_map(contract_overlay)
    .collect()
}

async fn character_industry_overlays(db: &Database, names: &mut TypeNames, character_id: i64) -> Vec<CalendarEvent> {
  let mut events = Vec::new();
  for job in industry::list_for_character(db, character_id).await.unwrap_or_default() {
    if let Some(event) = character_industry_overlay(db, names, &job).await {
      events.push(event);
    }
  }
  events
}

async fn colony_storage_overlays(db: &Database, character_id: i64) -> Vec<CalendarEvent> {
  let now = Utc::now();
  let mut events = Vec::new();
  for colony in colonies_for_character(db, character_id).await {
    if let Some(eta) = colony.storage_full_eta(now) {
      events.push(colony_storage_overlay(character_id, &colony, eta));
    }
  }
  events
}

fn colony_storage_overlay(character_id: i64, colony: &Colony, eta: DateTime<Utc>) -> CalendarEvent {
  overlay_event(
    character_id,
    synthetic_id(SOURCE_COLONY_STORAGE, colony.planet_id),
    SOURCE_COLONY_STORAGE,
    t!("calendar.overlay.colony_storage_title", colony => colony.name.clone()).into_owned(),
    t!("calendar.overlay.colony_storage_body").into_owned(),
    eta.to_rfc3339(),
  )
}

async fn corporation_industry_overlays(
  db: &Database,
  names: &mut TypeNames,
  corporation_id: i64,
) -> Vec<CalendarEvent> {
  let mut events = Vec::new();
  for job in industry::list_for_corporation(db, corporation_id)
    .await
    .unwrap_or_default()
  {
    if let Some(event) = corporation_industry_overlay(db, names, &job).await {
      events.push(event);
    }
  }
  events
}

async fn corporation_extraction_overlays(db: &Database, corporation_id: i64) -> Vec<CalendarEvent> {
  org::corporation_mining_extractions(db, corporation_id)
    .await
    .unwrap_or_default()
    .iter()
    .flat_map(extraction_overlays)
    .collect()
}

pub(super) async fn load_roster(db: &Database) -> Vec<RosterPilot> {
  let characters = character::all_owned(db).await.unwrap_or_default();
  let credentials = infra::all(db).await.unwrap_or_default();
  let scopes_by_id: HashMap<i64, Option<String>> = credentials
    .into_iter()
    .filter(|cred| cred.owner_type() == CredentialOwner::Character)
    .map(|cred| (cred.owner_id(), cred.scopes().clone()))
    .collect();

  let mut roster = Vec::with_capacity(characters.len());
  for character in &characters {
    let corp = org::get_corporation(db, character.corporation_id())
      .await
      .ok()
      .flatten()
      .map(|c| c.ticker().to_owned())
      .unwrap_or_default();
    let portrait = images::resolve(
      &images::default_store(),
      images::ImageKind::CharacterPortrait,
      character.id(),
    );
    roster.push(RosterPilot {
      corp,
      granted_scopes: scopes_by_id.get(&character.id()).cloned().flatten(),
      id: character.id(),
      name: character.name().to_owned(),
      portrait,
    });
  }
  roster
}

async fn character_industry_overlay(
  db: &Database,
  names: &mut TypeNames,
  job: &CharacterIndustryJob,
) -> Option<CalendarEvent> {
  industry_overlay(
    db,
    names,
    IndustryJobView {
      activity_id: job.activity_id(),
      blueprint_type_id: job.blueprint_type_id(),
      character_id: job.character_id(),
      completed_date: job.completed_date().as_deref(),
      cost: job.cost(),
      end_date: job.end_date(),
      id_source: SOURCE_INDUSTRY,
      job_id: job.job_id(),
      pause_date: job.pause_date().as_deref(),
      product_type_id: job.product_type_id(),
      runs: job.runs(),
      status: job.status(),
    },
  )
  .await
}

fn contract_overlay(contract: &CharacterContract) -> Option<CalendarEvent> {
  if contract.status() != "outstanding" {
    return None;
  }
  let expires = contract.date_expired().clone()?;
  let label = humanize(contract.r#type());
  let body = match contract
    .assignee_name()
    .clone()
    .or_else(|| contract.issuer_name().clone())
  {
    Some(party) => t!("calendar.overlay.contract_body", party => party).into_owned(),
    None => t!("calendar.overlay.contract_body_generic").into_owned(),
  };
  Some(overlay_event(
    contract.character_id(),
    synthetic_id(SOURCE_CONTRACT, contract.contract_id()),
    SOURCE_CONTRACT,
    t!("calendar.overlay.contract_title", label => label).into_owned(),
    body,
    expires,
  ))
}

async fn corporation_industry_overlay(
  db: &Database,
  names: &mut TypeNames,
  job: &CorporationIndustryJob,
) -> Option<CalendarEvent> {
  industry_overlay(
    db,
    names,
    IndustryJobView {
      activity_id: job.activity_id(),
      blueprint_type_id: job.blueprint_type_id(),
      character_id: job.installer_id(),
      completed_date: job.completed_date().as_deref(),
      cost: job.cost(),
      end_date: job.end_date(),
      id_source: SOURCE_INDUSTRY_CORP,
      job_id: job.job_id(),
      pause_date: job.pause_date().as_deref(),
      product_type_id: job.product_type_id(),
      runs: job.runs(),
      status: job.status(),
    },
  )
  .await
}

fn extraction_overlays(extraction: &CorporationMiningExtraction) -> Vec<CalendarEvent> {
  let moon = extraction
    .moon_name()
    .clone()
    .unwrap_or_else(|| t!("calendar.overlay.moon_fallback", id => extraction.moon_id()).into_owned());
  let mut events = Vec::new();
  if let Some(arrival) = extraction.chunk_arrival_time().clone() {
    events.push(overlay_event(
      extraction.corporation_id(),
      synthetic_id(SOURCE_EXTRACTION_ARRIVAL, extraction.structure_id()),
      SOURCE_EXTRACTION,
      t!("calendar.overlay.extraction_arrival_title", moon => moon).into_owned(),
      t!("calendar.overlay.extraction_arrival_body").into_owned(),
      arrival,
    ));
  }
  if let Some(decay) = extraction.natural_decay_time().clone() {
    events.push(overlay_event(
      extraction.corporation_id(),
      synthetic_id(SOURCE_EXTRACTION_DECAY, extraction.structure_id()),
      SOURCE_EXTRACTION,
      t!("calendar.overlay.extraction_decay_title", moon => moon).into_owned(),
      t!("calendar.overlay.extraction_decay_body").into_owned(),
      decay,
    ));
  }
  events
}

fn group_thousands(value: i64) -> String {
  let negative = value < 0;
  let digits = value.unsigned_abs().to_string();
  let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
  for (index, digit) in digits.chars().enumerate() {
    if index > 0 && (digits.len() - index).is_multiple_of(3) {
      grouped.push(',');
    }
    grouped.push(digit);
  }
  if negative { format!("-{grouped}") } else { grouped }
}

fn humanize(value: &str) -> String {
  if value.is_empty() {
    return t!("calendar.overlay.contract_fallback").into_owned();
  }
  value
    .split('_')
    .map(|word| {
      let mut chars = word.chars();
      match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
      }
    })
    .collect::<Vec<_>>()
    .join(" ")
}

async fn industry_overlay(db: &Database, names: &mut TypeNames, job: IndustryJobView<'_>) -> Option<CalendarEvent> {
  let activity = Activity::from_id(job.activity_id);
  let product = match job.product_type_id {
    Some(type_id) => names.resolve(db, type_id).await,
    None => None,
  };
  let label = match product {
    Some(name) => name,
    None => match names.resolve(db, job.blueprint_type_id).await {
      Some(name) => name,
      None => activity.label().to_owned(),
    },
  };
  let verb = industry_verb(job.status, job.completed_date, job.pause_date);
  let title = t!(
    "calendar.overlay.industry_title",
    label => label,
    runs => group_thousands(job.runs),
    verb => verb,
    activity => activity.label()
  )
  .into_owned();
  let body = match job.cost {
    Some(cost) => t!(
      "calendar.overlay.industry_body_cost",
      activity => activity.label(),
      runs => group_thousands(job.runs),
      cost => group_thousands(cost.round() as i64)
    )
    .into_owned(),
    None => t!(
      "calendar.overlay.industry_body",
      activity => activity.label(),
      runs => group_thousands(job.runs)
    )
    .into_owned(),
  };
  Some(overlay_event(
    job.character_id,
    synthetic_id(job.id_source, job.job_id),
    SOURCE_INDUSTRY,
    title,
    body,
    job.end_date.to_owned(),
  ))
}

fn industry_verb(status: &str, completed_date: Option<&str>, pause_date: Option<&str>) -> String {
  match status {
    "delivered" | "ready" => t!("calendar.overlay.verb.delivered"),
    "cancelled" | "reverted" => t!("calendar.overlay.verb.cancelled"),
    "paused" => t!("calendar.overlay.verb.paused"),
    _ if completed_date.is_some() => t!("calendar.overlay.verb.delivered"),
    _ if pause_date.is_some() => t!("calendar.overlay.verb.paused"),
    _ => t!("calendar.overlay.verb.completes"),
  }
  .into_owned()
}

async fn market_overlay(db: &Database, names: &mut TypeNames, order: &MarketOrder) -> Option<CalendarEvent> {
  if order.state() != "open" {
    return None;
  }
  let issued = DateTime::parse_from_rfc3339(order.issued())
    .ok()
    .map(|dt| dt.with_timezone(&Utc))?;
  let expires = (issued + Duration::days(order.duration())).to_rfc3339();
  let item = names
    .resolve(db, order.type_id())
    .await
    .unwrap_or_else(|| t!("calendar.overlay.type_fallback", id => order.type_id()).into_owned());
  let side = if order.is_buy_order() {
    t!("calendar.overlay.market_side_buy")
  } else {
    t!("calendar.overlay.market_side_sell")
  };
  let title = t!(
    "calendar.overlay.market_title",
    side => side,
    item => item,
    volume => group_thousands(order.volume_remain())
  )
  .into_owned();
  Some(overlay_event(
    order.character_id(),
    synthetic_id(SOURCE_MARKET, order.order_id()),
    SOURCE_MARKET,
    title,
    t!("calendar.overlay.market_body").into_owned(),
    expires,
  ))
}

fn overlay_event(
  character_id: i64,
  event_id: i64,
  source: &str,
  title: String,
  body: String,
  timestamp: String,
) -> CalendarEvent {
  CalendarEvent {
    body: Some(body),
    character_id,
    duration_minutes: 0,
    event_id,
    importance: 0,
    owner_name: t!("calendar.overlay.owner_name").into_owned(),
    owner_type: OVERLAY_OWNER.to_owned(),
    response: Response::NotResponded.as_esi().to_owned(),
    source: Some(source.to_owned()),
    timestamp,
    title,
  }
}

async fn skill_overlay(db: &Database, names: &mut TypeNames, entry: &CharacterSkillqueue) -> Option<CalendarEvent> {
  let finish = entry.finish_date().clone()?;
  let skill = names
    .resolve(db, entry.skill_id())
    .await
    .unwrap_or_else(|| t!("calendar.overlay.skill_fallback", id => entry.skill_id()).into_owned());
  let level = roman(entry.finished_level());
  let body = match entry.level_end_sp() {
    Some(sp) => t!("calendar.overlay.skill_body_sp", sp => group_thousands(sp)).into_owned(),
    None => t!("calendar.overlay.skill_body").into_owned(),
  };
  Some(overlay_event(
    entry.character_id(),
    synthetic_id(SOURCE_SKILL, entry.skill_id()),
    SOURCE_SKILL,
    t!("calendar.overlay.skill_title", skill => skill, level => level).into_owned(),
    body,
    finish,
  ))
}

/// Returns a negative event ID that cannot collide with a real ESI calendar event ID (always positive).
///
/// Each source gets a disjoint billion-wide range via a numeric discriminant: skill=1, market=2,
/// contract=3, character industry=4, corporation industry=5, extraction arrival=6, extraction
/// decay=7, colony storage=8.  Overlapping key spaces (character vs corporation job ids, an extraction's two timers off
/// the same structure id) take distinct discriminants.  The low nine digits carry the original key, so
/// IDs are stable across reloads.
fn synthetic_id(source: &str, key: i64) -> i64 {
  let discriminant = match source {
    SOURCE_MARKET => 2,
    SOURCE_CONTRACT => 3,
    SOURCE_INDUSTRY => 4,
    SOURCE_INDUSTRY_CORP => 5,
    SOURCE_EXTRACTION_ARRIVAL => 6,
    SOURCE_EXTRACTION_DECAY => 7,
    SOURCE_COLONY_STORAGE => 8,
    _ => 1,
  };
  -(discriminant * 1_000_000_000 + (key % 1_000_000_000))
}

#[cfg(test)]
mod tests {
  use super::*;

  fn event(timestamp: &str, duration_minutes: i64) -> CalendarEvent {
    CalendarEvent {
      body: None,
      character_id: 1,
      duration_minutes,
      event_id: 1,
      importance: 0,
      owner_name: "Corp".to_owned(),
      owner_type: "corporation".to_owned(),
      response: "accepted".to_owned(),
      source: None,
      timestamp: timestamp.to_owned(),
      title: "Op".to_owned(),
    }
  }

  mod group_thousands {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_groups_digits_into_thousands() {
      assert_eq!(group_thousands(5_000), "5,000");
      assert_eq!(group_thousands(256_000), "256,000");
      assert_eq!(group_thousands(42), "42");
    }
  }

  mod humanize {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_falls_back_for_an_empty_type() {
      assert_eq!(humanize(""), "Contract");
    }

    #[test]
    fn it_title_cases_an_underscored_type() {
      assert_eq!(humanize("item_exchange"), "Item Exchange");
    }
  }

  mod industry_verb {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_falls_back_to_completed_date_when_status_is_unknown() {
      assert_eq!(
        industry_verb("mystery", Some("2026-06-20T00:00:00Z"), None),
        "delivered"
      );
    }

    #[test]
    fn it_falls_back_to_pause_date_when_status_is_unknown() {
      assert_eq!(industry_verb("mystery", None, Some("2026-06-19T00:00:00Z")), "paused");
    }

    #[test]
    fn it_maps_cancelled_jobs_to_cancelled() {
      assert_eq!(industry_verb("cancelled", None, None), "cancelled");
    }

    #[test]
    fn it_maps_delivered_jobs_to_delivered() {
      assert_eq!(industry_verb("delivered", None, None), "delivered");
    }

    #[test]
    fn it_maps_paused_jobs_to_paused() {
      assert_eq!(industry_verb("paused", None, None), "paused");
    }

    #[test]
    fn it_maps_running_jobs_to_completes() {
      assert_eq!(industry_verb("active", None, None), "completes");
    }
  }

  mod is_all_day {
    use super::*;

    #[test]
    fn it_treats_a_full_day_span_as_all_day() {
      assert!(event("2026-06-20T00:00:00Z", 1440).is_all_day());
      assert!(!event("2026-06-20T00:00:00Z", 90).is_all_day());
    }
  }

  mod load_overlays {
    use pretty_assertions::{assert_eq, assert_ne};

    use super::*;
    use crate::store::{
      self,
      model::{
        Alliance, Bloodline, Character, CharacterContract, CharacterIndustryJob, CharacterSkillqueue, Constellation,
        Corporation, CorporationIndustryJob, CorporationMemberRole, CorporationMiningExtraction, Gender, ItemCategory,
        ItemGroup, ItemType, MarketOrder, Moon, OwnerType as CredentialOwner, Race, Region, SolarSystem,
      },
      repo::{character, finance, industry, infra, org, sde},
    };

    async fn seed_character(db: &Database, id: i64) {
      let corp_id = 90_000_001;
      let alliance_id = 99_000_001;
      let alliance = Alliance::new(alliance_id, corp_id, id, "2003-01-01", "Test Alliance", "TST");
      let race = Race::new(2, alliance_id, "A race.", "Caldari");
      let mut corp = Corporation::new(corp_id, "Test Corp", "TSC");
      corp.set_ceo_id(id);
      corp.set_creator_id(id);
      corp.set_member_count(1);
      corp.set_tax_rate(0.0);
      let bloodline = Bloodline::new(1, corp_id, 2, 3, "A bloodline.", 4, 5, "Civire", 4, 4);
      let character = Character::new(id, 1, corp_id, 2, "2003-05-12", Gender::Male, "Pilot");
      character::insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
        .await
        .unwrap();
    }

    async fn seed_item(db: &Database, id: i64, name: &str) {
      let category = ItemCategory {
        icon_id: None,
        id: 6,
        name: "Category".to_owned(),
        published: true,
      };
      let group = ItemGroup {
        category_id: 6,
        icon_id: None,
        id: 18,
        name: "Group".to_owned(),
        published: true,
      };
      let item = ItemType {
        capacity: None,
        description: Some("An item.".to_owned()),
        dogma_attributes: "[]".to_owned(),
        group_id: 18,
        icon_id: None,
        id,
        market_group_id: None,
        name: name.to_owned(),
        packaged_volume: None,
        portion_size: None,
        published: true,
        radius: None,
        volume: None,
      };
      sde::insert_item_type_with_hierarchy(db, &item, &group, &category)
        .await
        .unwrap();
    }

    async fn seed_sources(db: &Database, character_id: i64) {
      seed_item(db, 3300, "Capacitor Management").await;
      seed_item(db, 34, "Tritanium").await;

      let skill = CharacterSkillqueue {
        character_id,
        finish_date: Some("2026-06-20T06:14:00Z".to_owned()),
        finished_level: 5,
        level_end_sp: Some(256_000),
        level_start_sp: Some(45_255),
        queue_position: 0,
        skill_id: 3300,
        start_date: Some("2026-05-01T00:00:00Z".to_owned()),
        training_start_sp: Some(45_255),
      };
      character::replace_skillqueue(db, character_id, &[skill]).await.unwrap();

      let order = MarketOrder {
        character_id,
        duration: 90,
        escrow: 0.0,
        is_buy_order: false,
        is_corporation: false,
        issued: "2026-06-01T12:00:00Z".to_owned(),
        location_id: 60_003_760,
        order_id: 1001,
        price: 5.5,
        range: "region".to_owned(),
        region_id: 10_000_002,
        state: "open".to_owned(),
        type_id: 34,
        volume_remain: 5_000,
        volume_total: 5_000,
      };
      finance::replace(db, character_id, &[order]).await.unwrap();

      let contract = CharacterContract {
        acceptor_id: None,
        acceptor_name: None,
        assignee_id: Some(2002),
        assignee_name: Some("Buyer".to_owned()),
        availability: None,
        character_id,
        collateral: None,
        contract_id: 42,
        date_accepted: None,
        date_completed: None,
        date_expired: Some("2026-06-14T11:00:00Z".to_owned()),
        date_issued: "2026-06-01T00:00:00Z".to_owned(),
        days_to_complete: None,
        end_location_id: None,
        for_corporation: false,
        issuer_corporation_id: None,
        issuer_id: character_id,
        issuer_name: Some("Pilot".to_owned()),
        price: Some(1_400_000_000.0),
        reward: None,
        start_location_id: None,
        status: "outstanding".to_owned(),
        title: None,
        r#type: "item_exchange".to_owned(),
        volume: None,
      };
      finance::replace_for_character(db, character_id, &[contract])
        .await
        .unwrap();
    }

    fn character_job(
      character_id: i64,
      job_id: i64,
      activity_id: i64,
      product_type_id: Option<i64>,
      status: &str,
    ) -> CharacterIndustryJob {
      CharacterIndustryJob {
        activity_id,
        blueprint_id: 1_000_000_000 + job_id,
        blueprint_location_id: 60_003_760,
        blueprint_type_id: 962,
        character_id,
        completed_character_id: None,
        completed_date: None,
        cost: Some(1_250.0),
        duration: 3_600,
        end_date: "2026-06-21T08:00:00Z".to_owned(),
        facility_id: 60_003_760,
        installer_id: character_id,
        job_id,
        licensed_runs: None,
        output_location_id: 60_003_760,
        pause_date: None,
        probability: None,
        product_type_id,
        runs: 10,
        start_date: "2026-06-20T08:00:00Z".to_owned(),
        station_id: Some(60_003_760),
        status: status.to_owned(),
        successful_runs: None,
      }
    }

    fn corporation_job(corporation_id: i64, installer_id: i64, job_id: i64) -> CorporationIndustryJob {
      CorporationIndustryJob {
        activity_id: 1,
        blueprint_id: 2_000_000_000 + job_id,
        blueprint_location_id: 60_003_760,
        blueprint_type_id: 962,
        completed_character_id: None,
        completed_date: None,
        corporation_id,
        cost: Some(9_000.0),
        duration: 7_200,
        end_date: "2026-06-22T09:00:00Z".to_owned(),
        facility_id: 60_003_760,
        installer_id,
        job_id,
        licensed_runs: None,
        output_location_id: 60_003_760,
        pause_date: None,
        probability: None,
        product_type_id: Some(34),
        runs: 3,
        start_date: "2026-06-21T09:00:00Z".to_owned(),
        station_id: Some(60_003_760),
        status: "active".to_owned(),
        successful_runs: None,
      }
    }

    fn extraction(corporation_id: i64, structure_id: i64, moon_id: i64) -> CorporationMiningExtraction {
      CorporationMiningExtraction {
        chunk_arrival_time: Some("2026-06-20T00:00:00Z".to_owned()),
        corporation_id,
        extraction_start_time: Some("2026-06-13T00:00:00Z".to_owned()),
        moon_id,
        moon_name: None,
        natural_decay_time: Some("2026-06-21T00:00:00Z".to_owned()),
        security_status: None,
        solar_system_id: None,
        structure_id,
      }
    }

    async fn seed_moon(db: &Database, moon_id: i64, solar_system_id: i64) {
      sde::upsert_region(
        db,
        &Region {
          description: None,
          id: 10_000_001,
          name: "Test Region".to_owned(),
        },
      )
      .await
      .unwrap();
      sde::upsert_constellation(
        db,
        &Constellation {
          id: 20_000_001,
          name: "Test Constellation".to_owned(),
          position_x: 0.0,
          position_y: 0.0,
          position_z: 0.0,
          region_id: 10_000_001,
        },
      )
      .await
      .unwrap();
      sde::upsert_solar_system(
        db,
        &SolarSystem {
          constellation_id: 20_000_001,
          id: solar_system_id,
          name: "Test System".to_owned(),
          position_x: 0.0,
          position_y: 0.0,
          position_z: 0.0,
          security_class: None,
          security_status: 0.5,
          star_id: None,
        },
      )
      .await
      .unwrap();
      sde::upsert_many_moons(
        db,
        &[Moon {
          id: moon_id,
          name: "Test System I - Moon 1".to_owned(),
          orbit_index: Some(1),
          planet_id: Some(40_000_001),
          position_x: 0.0,
          position_y: 0.0,
          position_z: 0.0,
          radius: None,
          solar_system_id,
          type_id: Some(14),
        }],
      )
      .await
      .unwrap();
    }

    async fn own_corporation(db: &Database, corporation_id: i64, authorized_by: i64) {
      infra::upsert(
        db,
        corporation_id,
        CredentialOwner::Corporation,
        "tok",
        "rt",
        9_999,
        Some(authorized_by),
        None,
      )
      .await
      .unwrap();
      let role = CorporationMemberRole::from((corporation_id, authorized_by, "Director".to_owned()));
      org::replace_for_corporation(db, corporation_id, &[role]).await.unwrap();
    }

    #[tokio::test]
    async fn it_derives_nothing_when_all_overlay_features_are_disabled() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_sources(&db, 42).await;
      let mut features = FeatureFlags::default();
      features.set_enabled(Feature::SkillMonitoring, false);
      features.set_enabled(Feature::Wallet, false);
      features.set_enabled(Feature::Industry, false);

      let overlays = super::super::load_overlays(&db, &[42], features).await;

      assert!(overlays.is_empty());
    }

    #[tokio::test]
    async fn it_derives_one_overlay_per_source_when_both_features_are_enabled() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_sources(&db, 42).await;

      let overlays = super::super::load_overlays(&db, &[42], FeatureFlags::default()).await;

      assert_eq!(overlays.len(), 3);
      assert!(overlays.iter().all(CalendarEvent::is_synthetic));
      assert!(overlays.iter().all(|event| !event.owner_kind().respondable()));
    }

    #[tokio::test]
    async fn it_drops_market_and_contract_overlays_when_wallet_is_disabled() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_sources(&db, 42).await;
      let mut features = FeatureFlags::default();
      features.set_enabled(Feature::Wallet, false);

      let overlays = super::super::load_overlays(&db, &[42], features).await;

      assert_eq!(overlays.len(), 1);
      assert_eq!(overlays[0].source.as_deref(), Some(SOURCE_SKILL));
    }

    #[tokio::test]
    async fn it_drops_skill_overlays_when_skill_monitoring_is_disabled() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_sources(&db, 42).await;
      let mut features = FeatureFlags::default();
      features.set_enabled(Feature::SkillMonitoring, false);

      let overlays = super::super::load_overlays(&db, &[42], features).await;

      assert!(
        overlays
          .iter()
          .all(|event| event.source.as_deref() != Some(SOURCE_SKILL))
      );
      assert_eq!(overlays.len(), 2);
    }

    #[tokio::test]
    async fn it_falls_back_to_the_blueprint_name_for_a_null_product() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_item(&db, 962, "Ibis Blueprint").await;
      industry::replace_for_character(&db, 42, &[character_job(42, 1, 5, None, "active")])
        .await
        .unwrap();
      let mut features = FeatureFlags::default();
      features.set_enabled(Feature::SkillMonitoring, false);
      features.set_enabled(Feature::Wallet, false);

      let overlays = super::super::load_overlays(&db, &[42], features).await;

      assert_eq!(overlays.len(), 1);
      assert_eq!(
        overlays[0].title,
        "Ibis Blueprint \u{00D7}10 completes \u{2014} Copying"
      );
    }

    #[tokio::test]
    async fn it_falls_back_to_the_moon_id_when_the_moon_is_not_in_the_sde() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      own_corporation(&db, 90_000_001, 42).await;
      org::replace_extractions_for_corporation(
        &db,
        90_000_001,
        &[extraction(90_000_001, 1_021_000_000_001, 40_000_999)],
      )
      .await
      .unwrap();
      let mut features = FeatureFlags::default();
      features.set_enabled(Feature::SkillMonitoring, false);
      features.set_enabled(Feature::Wallet, false);

      let overlays = super::super::load_overlays(&db, &[42], features).await;

      assert!(
        overlays
          .iter()
          .any(|event| event.title == "Moon 40000999 \u{2014} chunk arrival")
      );
    }

    #[tokio::test]
    async fn it_keeps_character_and_corporation_job_ids_disjoint() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_item(&db, 962, "Ibis Blueprint").await;
      seed_item(&db, 587, "Rifter").await;
      seed_item(&db, 34, "Tritanium").await;
      own_corporation(&db, 90_000_001, 42).await;
      industry::replace_for_character(&db, 42, &[character_job(42, 5, 1, Some(587), "active")])
        .await
        .unwrap();
      industry::replace_for_corporation(&db, 90_000_001, &[corporation_job(90_000_001, 42, 5)])
        .await
        .unwrap();
      let mut features = FeatureFlags::default();
      features.set_enabled(Feature::SkillMonitoring, false);
      features.set_enabled(Feature::Wallet, false);

      let overlays = super::super::load_overlays(&db, &[42], features).await;

      let ids: Vec<i64> = overlays.iter().map(|event| event.event_id).collect();
      assert_eq!(overlays.len(), 2);
      assert_ne!(ids[0], ids[1]);
    }

    #[tokio::test]
    async fn it_keeps_extraction_arrival_and_decay_synthetic_ids_disjoint() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      own_corporation(&db, 90_000_001, 42).await;
      org::replace_extractions_for_corporation(
        &db,
        90_000_001,
        &[extraction(90_000_001, 1_021_000_000_001, 40_000_999)],
      )
      .await
      .unwrap();
      let mut features = FeatureFlags::default();
      features.set_enabled(Feature::SkillMonitoring, false);
      features.set_enabled(Feature::Wallet, false);

      let overlays = super::super::load_overlays(&db, &[42], features).await;

      let ids: Vec<i64> = overlays.iter().map(|event| event.event_id).collect();
      assert_eq!(overlays.len(), 2);
      assert_ne!(ids[0], ids[1]);
    }

    #[tokio::test]
    async fn it_removes_extraction_overlays_when_the_industry_feature_is_disabled() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      own_corporation(&db, 90_000_001, 42).await;
      org::replace_extractions_for_corporation(
        &db,
        90_000_001,
        &[extraction(90_000_001, 1_021_000_000_001, 40_000_999)],
      )
      .await
      .unwrap();
      let mut features = FeatureFlags::default();
      features.set_enabled(Feature::SkillMonitoring, false);
      features.set_enabled(Feature::Wallet, false);
      features.set_enabled(Feature::Industry, false);

      let overlays = super::super::load_overlays(&db, &[42], features).await;

      assert!(overlays.is_empty());
    }

    #[tokio::test]
    async fn it_removes_industry_overlays_when_the_feature_is_disabled() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_item(&db, 962, "Ibis Blueprint").await;
      seed_item(&db, 587, "Rifter").await;
      own_corporation(&db, 90_000_001, 42).await;
      industry::replace_for_character(&db, 42, &[character_job(42, 1, 1, Some(587), "active")])
        .await
        .unwrap();
      industry::replace_for_corporation(&db, 90_000_001, &[corporation_job(90_000_001, 42, 7)])
        .await
        .unwrap();
      let mut features = FeatureFlags::default();
      features.set_enabled(Feature::SkillMonitoring, false);
      features.set_enabled(Feature::Wallet, false);
      features.set_enabled(Feature::Industry, false);

      let overlays = super::super::load_overlays(&db, &[42], features).await;

      assert!(overlays.is_empty());
    }

    #[tokio::test]
    async fn it_skips_extraction_events_with_a_missing_timestamp() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      own_corporation(&db, 90_000_001, 42).await;
      let mut without_decay = extraction(90_000_001, 1_021_000_000_001, 40_000_999);
      without_decay.natural_decay_time = None;
      org::replace_extractions_for_corporation(&db, 90_000_001, &[without_decay])
        .await
        .unwrap();
      let mut features = FeatureFlags::default();
      features.set_enabled(Feature::SkillMonitoring, false);
      features.set_enabled(Feature::Wallet, false);

      let overlays = super::super::load_overlays(&db, &[42], features).await;

      assert_eq!(overlays.len(), 1);
      assert!(overlays[0].title.ends_with("chunk arrival"));
    }

    #[tokio::test]
    async fn it_surfaces_a_character_industry_job_at_its_end_date() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_item(&db, 962, "Ibis Blueprint").await;
      seed_item(&db, 587, "Rifter").await;
      industry::replace_for_character(&db, 42, &[character_job(42, 1, 1, Some(587), "active")])
        .await
        .unwrap();
      let mut features = FeatureFlags::default();
      features.set_enabled(Feature::SkillMonitoring, false);
      features.set_enabled(Feature::Wallet, false);

      let overlays = super::super::load_overlays(&db, &[42], features).await;

      assert_eq!(overlays.len(), 1);
      assert_eq!(overlays[0].source.as_deref(), Some(SOURCE_INDUSTRY));
      assert_eq!(overlays[0].title, "Rifter \u{00D7}10 completes \u{2014} Manufacturing");
      assert_eq!(overlays[0].timestamp, "2026-06-21T08:00:00Z");
      assert!(overlays[0].is_synthetic());
      assert!(!overlays[0].owner_kind().respondable());
    }

    #[tokio::test]
    async fn it_surfaces_a_corporation_industry_job() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_item(&db, 962, "Ibis Blueprint").await;
      seed_item(&db, 34, "Tritanium").await;
      own_corporation(&db, 90_000_001, 42).await;
      industry::replace_for_corporation(&db, 90_000_001, &[corporation_job(90_000_001, 42, 7)])
        .await
        .unwrap();
      let mut features = FeatureFlags::default();
      features.set_enabled(Feature::SkillMonitoring, false);
      features.set_enabled(Feature::Wallet, false);

      let overlays = super::super::load_overlays(&db, &[42], features).await;

      assert_eq!(overlays.len(), 1);
      assert_eq!(overlays[0].source.as_deref(), Some(SOURCE_INDUSTRY));
      assert_eq!(
        overlays[0].title,
        "Tritanium \u{00D7}3 completes \u{2014} Manufacturing"
      );
      assert_eq!(overlays[0].timestamp, "2026-06-22T09:00:00Z");
    }

    #[tokio::test]
    async fn it_surfaces_a_mining_extraction_as_arrival_and_decay_events() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_moon(&db, 40_000_001, 30_000_001).await;
      own_corporation(&db, 90_000_001, 42).await;
      org::replace_extractions_for_corporation(
        &db,
        90_000_001,
        &[extraction(90_000_001, 1_021_000_000_001, 40_000_001)],
      )
      .await
      .unwrap();
      let mut features = FeatureFlags::default();
      features.set_enabled(Feature::SkillMonitoring, false);
      features.set_enabled(Feature::Wallet, false);

      let overlays = super::super::load_overlays(&db, &[42], features).await;

      assert_eq!(overlays.len(), 2);
      assert!(
        overlays
          .iter()
          .all(|event| event.source.as_deref() == Some(SOURCE_EXTRACTION))
      );
      let arrival = overlays
        .iter()
        .find(|event| event.title == "Test System I - Moon 1 \u{2014} chunk arrival")
        .unwrap();
      let decay = overlays
        .iter()
        .find(|event| event.title == "Test System I - Moon 1 \u{2014} fracture")
        .unwrap();
      assert_eq!(arrival.timestamp, "2026-06-20T00:00:00Z");
      assert_eq!(decay.timestamp, "2026-06-21T00:00:00Z");
    }

    #[tokio::test]
    async fn it_surfaces_jobs_of_every_status() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_item(&db, 962, "Ibis Blueprint").await;
      seed_item(&db, 587, "Rifter").await;
      industry::replace_for_character(
        &db,
        42,
        &[
          character_job(42, 1, 1, Some(587), "active"),
          character_job(42, 2, 1, Some(587), "delivered"),
          character_job(42, 3, 1, Some(587), "cancelled"),
          character_job(42, 4, 1, Some(587), "paused"),
        ],
      )
      .await
      .unwrap();
      let mut features = FeatureFlags::default();
      features.set_enabled(Feature::SkillMonitoring, false);
      features.set_enabled(Feature::Wallet, false);

      let overlays = super::super::load_overlays(&db, &[42], features).await;

      let verbs: Vec<&str> = overlays
        .iter()
        .filter_map(|event| event.title.split(" \u{2014} ").next())
        .filter_map(|head| head.rsplit(' ').next())
        .collect();
      assert_eq!(overlays.len(), 4);
      assert!(verbs.contains(&"completes"));
      assert!(verbs.contains(&"delivered"));
      assert!(verbs.contains(&"cancelled"));
      assert!(verbs.contains(&"paused"));
    }

    #[tokio::test]
    async fn it_tags_each_overlay_with_a_resolved_title_and_source() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_sources(&db, 42).await;

      let overlays = super::super::load_overlays(&db, &[42], FeatureFlags::default()).await;

      let skill = overlays
        .iter()
        .find(|e| e.source.as_deref() == Some(SOURCE_SKILL))
        .unwrap();
      assert_eq!(skill.title, "Capacitor Management V completes");

      let market = overlays
        .iter()
        .find(|e| e.source.as_deref() == Some(SOURCE_MARKET))
        .unwrap();
      assert_eq!(market.title, "Sell order expires \u{2014} Tritanium \u{00D7}5,000");

      let contract = overlays
        .iter()
        .find(|e| e.source.as_deref() == Some(SOURCE_CONTRACT))
        .unwrap();
      assert_eq!(contract.title, "Contract expires \u{2014} Item Exchange");
    }
  }

  mod start {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_adds_the_duration_to_compute_the_end() {
      let parsed = event("2026-06-20T19:00:00Z", 90);

      let span = parsed.end().unwrap() - parsed.start().unwrap();

      assert_eq!(span.num_minutes(), 90);
    }

    #[test]
    fn it_is_none_for_an_unparseable_timestamp() {
      let parsed = event("not-a-date", 0);

      assert!(parsed.start().is_none());
    }

    #[test]
    fn it_parses_an_rfc3339_timestamp() {
      let parsed = event("2026-06-20T19:00:00Z", 0);

      assert!(parsed.start().is_some());
    }
  }

  mod synthetic_id {
    use pretty_assertions::{assert_eq, assert_ne};

    use super::*;

    #[test]
    fn it_is_negative_so_it_cannot_collide_with_an_esi_event_id() {
      assert!(synthetic_id(SOURCE_SKILL, 3300) < 0);
      assert!(synthetic_id(SOURCE_MARKET, 1001) < 0);
      assert!(synthetic_id(SOURCE_CONTRACT, 42) < 0);
      assert!(synthetic_id(SOURCE_INDUSTRY, 5) < 0);
      assert!(synthetic_id(SOURCE_INDUSTRY_CORP, 5) < 0);
    }

    #[test]
    fn it_is_stable_across_reloads() {
      assert_eq!(
        synthetic_id(SOURCE_INDUSTRY, 12_345),
        synthetic_id(SOURCE_INDUSTRY, 12_345)
      );
    }

    #[test]
    fn it_keeps_character_and_corporation_industry_jobs_distinct_for_the_same_job_id() {
      assert_ne!(
        synthetic_id(SOURCE_INDUSTRY, 99),
        synthetic_id(SOURCE_INDUSTRY_CORP, 99)
      );
    }

    #[test]
    fn it_keeps_sources_in_disjoint_ranges() {
      assert_ne!(synthetic_id(SOURCE_SKILL, 7), synthetic_id(SOURCE_MARKET, 7));
      assert_ne!(synthetic_id(SOURCE_MARKET, 7), synthetic_id(SOURCE_CONTRACT, 7));
      assert_ne!(synthetic_id(SOURCE_CONTRACT, 7), synthetic_id(SOURCE_INDUSTRY, 7));
      assert_ne!(synthetic_id(SOURCE_INDUSTRY, 7), synthetic_id(SOURCE_INDUSTRY_CORP, 7));
    }

    #[test]
    fn it_gives_colony_storage_a_negative_range_disjoint_from_every_other_source() {
      let colony = synthetic_id(SOURCE_COLONY_STORAGE, 40_000_001);

      assert!(colony < 0);
      assert_ne!(colony, synthetic_id(SOURCE_SKILL, 40_000_001));
      assert_ne!(colony, synthetic_id(SOURCE_EXTRACTION_DECAY, 40_000_001));
      assert_ne!(colony, synthetic_id(SOURCE_INDUSTRY, 40_000_001));
    }
  }
}
