use std::collections::HashMap;

use chrono::{DateTime, Utc};

use super::palette::{OwnerType, Response};
use crate::store::{
  Database, images,
  model::{AttendeeTally, CharacterCalendarEvent, OwnerType as CredentialOwner},
  repo::{calendar, character, infra, org},
};

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
