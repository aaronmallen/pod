use std::collections::HashMap;

use chrono::{DateTime, Duration, Utc};

use super::palette::{OwnerType, Response};
use crate::{
  config::{Feature, FeatureFlags},
  features::skills::queue_timing::roman,
  store::{
    Database, images,
    model::{
      AttendeeTally, CharacterCalendarEvent, CharacterContract, CharacterSkillqueue, MarketOrder,
      OwnerType as CredentialOwner,
    },
    repo::{calendar, character, finance, infra, org, sde},
  },
};

const OVERLAY_OWNER: &str = "pod";
const SOURCE_CONTRACT: &str = "contract";
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

pub(super) async fn load_overlays(db: &Database, character_ids: &[i64], features: FeatureFlags) -> Vec<CalendarEvent> {
  let skills = features.is_enabled(Feature::SkillMonitoring);
  let wallet = features.is_enabled(Feature::Wallet);
  if !skills && !wallet {
    return Vec::new();
  }

  let mut names = TypeNames::new();
  let mut overlays = Vec::new();
  for &character_id in character_ids {
    if skills {
      for entry in character::skillqueue(db, character_id).await.unwrap_or_default() {
        if let Some(event) = skill_overlay(db, &mut names, &entry).await {
          overlays.push(event);
        }
      }
    }
    if wallet {
      for order in finance::for_character(db, character_id).await.unwrap_or_default() {
        if let Some(event) = market_overlay(db, &mut names, &order).await {
          overlays.push(event);
        }
      }
      for contract in finance::contracts(db, character_id).await.unwrap_or_default() {
        if let Some(event) = contract_overlay(&contract) {
          overlays.push(event);
        }
      }
    }
  }
  overlays
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
    Some(party) => format!("Contract to {party} lapses. Relist or let it expire."),
    None => "Relist or let it expire.".to_owned(),
  };
  Some(overlay_event(
    contract.character_id(),
    synthetic_id(SOURCE_CONTRACT, contract.contract_id()),
    SOURCE_CONTRACT,
    format!("Contract expires \u{2014} {label}"),
    body,
    expires,
  ))
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
    return "Contract".to_owned();
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
    .unwrap_or_else(|| format!("Type {}", order.type_id()));
  let side = if order.is_buy_order() { "Buy" } else { "Sell" };
  let title = format!(
    "{side} order expires \u{2014} {item} \u{00D7}{}",
    group_thousands(order.volume_remain())
  );
  Some(overlay_event(
    order.character_id(),
    synthetic_id(SOURCE_MARKET, order.order_id()),
    SOURCE_MARKET,
    title,
    "Relist or reprice before the order lapses.".to_owned(),
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
    owner_name: "Pod".to_owned(),
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
    .unwrap_or_else(|| format!("Skill {}", entry.skill_id()));
  let level = roman(entry.finished_level());
  let body = match entry.level_end_sp() {
    Some(sp) => format!("Queue entry finishes training. +{} SP.", group_thousands(sp)),
    None => "Queue entry finishes training.".to_owned(),
  };
  Some(overlay_event(
    entry.character_id(),
    synthetic_id(SOURCE_SKILL, entry.skill_id()),
    SOURCE_SKILL,
    format!("{skill} {level} completes"),
    body,
    finish,
  ))
}

/// Returns a negative event ID that cannot collide with a real ESI calendar event ID (always positive).
///
/// Each source gets a disjoint billion-wide range via a numeric discriminant: skill=1, market=2,
/// contract=3.  The low nine digits carry the original key, so IDs are stable across reloads.
fn synthetic_id(source: &str, key: i64) -> i64 {
  let discriminant = match source {
    SOURCE_MARKET => 2,
    SOURCE_CONTRACT => 3,
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

  mod start {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_parses_an_rfc3339_timestamp() {
      let parsed = event("2026-06-20T19:00:00Z", 0);

      assert!(parsed.start().is_some());
    }

    #[test]
    fn it_is_none_for_an_unparseable_timestamp() {
      let parsed = event("not-a-date", 0);

      assert!(parsed.start().is_none());
    }

    #[test]
    fn it_adds_the_duration_to_compute_the_end() {
      let parsed = event("2026-06-20T19:00:00Z", 90);

      let span = parsed.end().unwrap() - parsed.start().unwrap();

      assert_eq!(span.num_minutes(), 90);
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
    fn it_title_cases_an_underscored_type() {
      assert_eq!(humanize("item_exchange"), "Item Exchange");
    }

    #[test]
    fn it_falls_back_for_an_empty_type() {
      assert_eq!(humanize(""), "Contract");
    }
  }

  mod synthetic_id {
    use pretty_assertions::assert_ne;

    use super::*;

    #[test]
    fn it_is_negative_so_it_cannot_collide_with_an_esi_event_id() {
      assert!(synthetic_id(SOURCE_SKILL, 3300) < 0);
      assert!(synthetic_id(SOURCE_MARKET, 1001) < 0);
      assert!(synthetic_id(SOURCE_CONTRACT, 42) < 0);
    }

    #[test]
    fn it_keeps_sources_in_disjoint_ranges() {
      assert_ne!(synthetic_id(SOURCE_SKILL, 7), synthetic_id(SOURCE_MARKET, 7));
      assert_ne!(synthetic_id(SOURCE_MARKET, 7), synthetic_id(SOURCE_CONTRACT, 7));
    }
  }

  mod load_overlays {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{
      self,
      model::{
        Alliance, Bloodline, Character, CharacterContract, CharacterSkillqueue, Corporation, Gender, ItemCategory,
        ItemGroup, ItemType, MarketOrder, Race,
      },
      repo::{character, finance, sde},
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
        character_id,
        collateral: None,
        contract_id: 42,
        date_completed: None,
        date_expired: Some("2026-06-14T11:00:00Z".to_owned()),
        date_issued: "2026-06-01T00:00:00Z".to_owned(),
        for_corporation: false,
        issuer_id: character_id,
        issuer_name: Some("Pilot".to_owned()),
        price: Some(1_400_000_000.0),
        reward: None,
        status: "outstanding".to_owned(),
        r#type: "item_exchange".to_owned(),
        volume: None,
      };
      finance::replace_for_character(db, character_id, &[contract])
        .await
        .unwrap();
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
    async fn it_derives_nothing_when_both_features_are_disabled() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_sources(&db, 42).await;
      let mut features = FeatureFlags::default();
      features.set_enabled(Feature::SkillMonitoring, false);
      features.set_enabled(Feature::Wallet, false);

      let overlays = super::super::load_overlays(&db, &[42], features).await;

      assert!(overlays.is_empty());
    }
  }
}
