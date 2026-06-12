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
  let title = format!("{side} order expires \u{2014} {item} \u{00D7}{}", order.volume_remain());
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
    Some(sp) => format!("Queue entry finishes training. +{sp} SP."),
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
}
