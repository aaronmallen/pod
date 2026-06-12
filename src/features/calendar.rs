// The module and its public surface (Message/State/update/view/load) are mounted by the app
// router in a follow-up change; until then the wiring is dead from the binary's perspective.
#![allow(dead_code)]

mod agenda;
mod day;
mod detail;
mod grid;
mod loaders;
mod month;
mod palette;
mod respond;
mod shell;
mod switcher;
mod tweaks;
mod week;
mod year;

use chrono::{DateTime, Utc};
use iced::{Element, Task};

pub use self::loaders::{CalendarEvent, RosterPilot};
use crate::{
  config::{CalendarTweaks, FeatureFlags},
  store::{Database, images, model::AttendeeTally},
};

pub const EMPTY_CALENDAR_SELECTION: i64 = 0;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Scope {
  All,
  #[default]
  Empty,
  Mine(i64),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum View {
  #[default]
  Agenda,
  Day,
  Month,
  Week,
  Year,
}

impl View {
  pub const ALL: [View; 5] = [View::Agenda, View::Day, View::Week, View::Month, View::Year];

  pub fn label(self) -> &'static str {
    match self {
      View::Agenda => "Agenda",
      View::Day => "Day",
      View::Month => "Month",
      View::Week => "Week",
      View::Year => "Year",
    }
  }
}

#[derive(Clone, Debug, Default)]
pub struct Loaded {
  events: Vec<CalendarEvent>,
  roster: Vec<RosterPilot>,
  scope: Scope,
}

#[derive(Clone, Debug)]
pub enum Message {
  CursorNext,
  CursorPrev,
  CursorToday,
  DatePicked(DateTime<Utc>, View),
  DetailAttendeesLoaded(Box<Option<AttendeeTally>>),
  DetailClosed,
  EventOpened(i64, i64),
  Loaded(Box<Loaded>),
  PickerToggled,
  ReauthRequested(i64),
  Responded(i64, i64, palette::Response),
  RsvpWritten,
  ScopeSelected(Scope),
  TweakChanged(tweaks::Tweak),
  TweaksToggled,
  ViewSelected(View),
}

#[derive(Debug)]
pub struct State {
  active: Scope,
  cursor: DateTime<Utc>,
  detail: Option<Detail>,
  events: Vec<CalendarEvent>,
  features: FeatureFlags,
  picker_open: bool,
  roster: Vec<RosterPilot>,
  tweaks: CalendarTweaks,
  tweaks_open: bool,
  view: View,
}

impl State {
  pub fn new(active: i64, cursor: DateTime<Utc>, tweaks: CalendarTweaks, features: FeatureFlags) -> Self {
    State {
      active: if active == EMPTY_CALENDAR_SELECTION {
        Scope::All
      } else {
        Scope::Mine(active)
      },
      cursor,
      detail: None,
      events: Vec::new(),
      features,
      picker_open: false,
      roster: Vec::new(),
      tweaks,
      tweaks_open: false,
      view: View::default(),
    }
  }

  pub fn active(&self) -> Scope {
    self.active
  }

  pub fn set_features(&mut self, features: FeatureFlags) {
    self.features = features;
  }

  pub fn stale_images(&self) -> Vec<(images::ImageKind, i64)> {
    self
      .roster
      .iter()
      .map(|pilot| &pilot.portrait)
      .filter_map(images::ImageState::stale_key)
      .filter(|(_, id)| *id > 0)
      .collect()
  }

  pub fn tweaks(&self) -> CalendarTweaks {
    self.tweaks
  }

  pub(super) fn color_index_for(&self, character_id: i64) -> usize {
    self
      .roster
      .iter()
      .position(|pilot| pilot.id == character_id)
      .unwrap_or(0)
  }

  pub(super) fn cursor(&self) -> DateTime<Utc> {
    self.cursor
  }

  pub(super) fn detail(&self) -> Option<&Detail> {
    self.detail.as_ref()
  }

  pub(super) fn picker_open(&self) -> bool {
    self.picker_open
  }

  pub(super) fn pilot(&self, character_id: i64) -> Option<&RosterPilot> {
    self.roster.iter().find(|pilot| pilot.id == character_id)
  }

  pub(super) fn roster(&self) -> &[RosterPilot] {
    &self.roster
  }

  pub(super) fn scope_gate(&self) -> Option<(i64, &str, Vec<&'static str>)> {
    let Scope::Mine(id) = self.active else {
      return None;
    };
    let pilot = self.roster.iter().find(|pilot| pilot.id == id)?;
    let missing = crate::ui::components::forbidden::missing_scopes(pilot.granted_scopes.as_deref(), registry_scopes());
    if missing.is_empty() {
      return None;
    }
    Some((id, pilot.name.as_str(), missing))
  }

  pub(super) fn tweaks_open(&self) -> bool {
    self.tweaks_open
  }

  pub(super) fn unauthorized_pilots(&self) -> Vec<&RosterPilot> {
    let required = registry_scopes();
    self
      .roster
      .iter()
      .filter(|pilot| {
        !crate::ui::components::forbidden::missing_scopes(pilot.granted_scopes.as_deref(), required).is_empty()
      })
      .collect()
  }

  pub(super) fn view(&self) -> View {
    self.view
  }

  pub(super) fn visible_events(&self) -> Vec<&CalendarEvent> {
    let show_overlays = self.tweaks.pod_overlays();
    self
      .events
      .iter()
      .filter(|event| show_overlays || event.owner_type != "pod")
      .filter(|event| self.is_authorized(event.character_id))
      .collect()
  }

  fn is_authorized(&self, character_id: i64) -> bool {
    self
      .roster
      .iter()
      .find(|pilot| pilot.id == character_id)
      .map(|pilot| {
        crate::ui::components::forbidden::missing_scopes(pilot.granted_scopes.as_deref(), registry_scopes()).is_empty()
      })
      // Pilots absent from the roster are treated as authorized so that events belonging
      // to characters not yet loaded (e.g. during initial sync) are not silently hidden.
      .unwrap_or(true)
  }
}

#[derive(Debug)]
pub(super) struct Detail {
  attendees: Option<AttendeeTally>,
  character_id: i64,
  event_id: i64,
}

pub fn load(db: &Database, character: i64, features: FeatureFlags) -> Task<Message> {
  let scope = if character == EMPTY_CALENDAR_SELECTION {
    Scope::All
  } else {
    Scope::Mine(character)
  };
  reload(db, scope, features)
}

pub fn reload(db: &Database, scope: Scope, features: FeatureFlags) -> Task<Message> {
  Task::perform(load_calendar(db.clone(), scope, features), |loaded| {
    Message::Loaded(Box::new(loaded))
  })
}

pub fn subscription(_state: &State) -> iced::Subscription<Message> {
  iced::Subscription::none()
}

pub fn update(state: &mut State, message: Message, db: &Database, now: DateTime<Utc>) -> Task<Message> {
  match message {
    Message::CursorNext => {
      state.cursor = advance(state.cursor, state.view, 1);
      Task::none()
    }
    Message::CursorPrev => {
      state.cursor = advance(state.cursor, state.view, -1);
      Task::none()
    }
    Message::CursorToday => {
      state.cursor = now;
      Task::none()
    }
    Message::DatePicked(date, view) => {
      state.cursor = date;
      state.view = view;
      Task::none()
    }
    Message::DetailAttendeesLoaded(tally) => {
      if let Some(detail) = state.detail.as_mut() {
        detail.attendees = *tally;
      }
      Task::none()
    }
    Message::DetailClosed => {
      state.detail = None;
      Task::none()
    }
    Message::EventOpened(character_id, event_id) => {
      state.detail = Some(Detail {
        attendees: None,
        character_id,
        event_id,
      });
      Task::perform(load_attendees(db.clone(), character_id, event_id), |tally| {
        Message::DetailAttendeesLoaded(Box::new(tally))
      })
    }
    Message::Loaded(loaded) => {
      let Loaded {
        events,
        roster,
        scope,
      } = *loaded;
      // Drop results that belong to a scope the user already navigated away from;
      // the in-flight task resolves asynchronously and would otherwise overwrite the
      // data that the newer scope's load already wrote.
      if scope == state.active {
        state.events = events;
        state.roster = roster;
      }
      Task::none()
    }
    Message::PickerToggled => {
      state.picker_open = !state.picker_open;
      Task::none()
    }
    Message::ReauthRequested(_) => Task::none(),
    Message::Responded(character_id, event_id, response) => respond_to(state, db, character_id, event_id, response),
    Message::RsvpWritten => reload(db, state.active, state.features),
    Message::ScopeSelected(scope) => {
      state.active = scope;
      state.picker_open = false;
      state.detail = None;
      reload(db, scope, state.features)
    }
    Message::TweakChanged(tweak) => {
      tweak.apply(&mut state.tweaks);
      Task::none()
    }
    Message::TweaksToggled => {
      state.tweaks_open = !state.tweaks_open;
      Task::none()
    }
    Message::ViewSelected(view) => {
      state.view = view;
      Task::none()
    }
  }
}

pub fn view(state: &State, now: DateTime<Utc>) -> Element<'_, Message> {
  shell::shell(state, now)
}

fn advance(cursor: DateTime<Utc>, view: View, direction: i64) -> DateTime<Utc> {
  let step = match view {
    View::Agenda | View::Day => chrono::Duration::days(1),
    View::Week => chrono::Duration::weeks(1),
    View::Month => chrono::Duration::days(30),
    View::Year => chrono::Duration::days(365),
  };
  cursor + step * direction as i32
}

async fn load_attendees(db: Database, character_id: i64, event_id: i64) -> Option<AttendeeTally> {
  loaders::load_attendees(&db, character_id, event_id).await
}

async fn load_calendar(db: Database, scope: Scope, features: FeatureFlags) -> Loaded {
  let roster = loaders::load_roster(&db).await;
  let mut events = match scope {
    Scope::All => loaders::load_combined(&db).await,
    Scope::Empty => Vec::new(),
    Scope::Mine(id) => loaders::load_events(&db, id).await,
  };
  let overlay_ids: Vec<i64> = match scope {
    Scope::All => roster.iter().map(|pilot| pilot.id).collect(),
    Scope::Empty => Vec::new(),
    Scope::Mine(id) => vec![id],
  };
  if !overlay_ids.is_empty() {
    events.extend(loaders::load_overlays(&db, &overlay_ids, features).await);
  }
  Loaded {
    events,
    roster,
    scope,
  }
}

fn registry_scopes() -> &'static [&'static str] {
  crate::features::registry::descriptor(crate::config::Feature::Calendar).scopes
}

fn respond_to(
  state: &State,
  db: &Database,
  character_id: i64,
  event_id: i64,
  response: palette::Response,
) -> Task<Message> {
  let previous = state
    .events
    .iter()
    .find(|event| event.character_id == character_id && event.event_id == event_id)
    .map(|event| event.response.clone())
    .unwrap_or_else(|| palette::Response::NotResponded.as_esi().to_owned());

  Task::perform(
    respond::respond(
      db.clone(),
      character_id,
      event_id,
      response.as_esi().to_owned(),
      previous,
    ),
    |()| Message::RsvpWritten,
  )
}

#[cfg(test)]
mod tests {
  use super::*;

  fn now() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-06-12T14:22:00Z")
      .unwrap()
      .with_timezone(&Utc)
  }

  fn pilot(id: i64, scopes: Option<&str>) -> RosterPilot {
    RosterPilot {
      corp: "TST".to_owned(),
      granted_scopes: scopes.map(str::to_owned),
      id,
      name: format!("Pilot {id}"),
      portrait: images::ImageState::Stale {
        id,
        kind: images::ImageKind::CharacterPortrait,
      },
    }
  }

  fn event(character_id: i64, event_id: i64, owner_type: &str, response: &str) -> CalendarEvent {
    CalendarEvent {
      body: None,
      character_id,
      duration_minutes: 90,
      event_id,
      importance: 0,
      owner_name: "Corp".to_owned(),
      owner_type: owner_type.to_owned(),
      response: response.to_owned(),
      source: None,
      timestamp: "2026-06-20T19:00:00Z".to_owned(),
      title: "Op".to_owned(),
    }
  }

  fn state_with(active: Scope, roster: Vec<RosterPilot>, events: Vec<CalendarEvent>) -> State {
    let mut state = State::new(
      EMPTY_CALENDAR_SELECTION,
      now(),
      CalendarTweaks::default(),
      FeatureFlags::default(),
    );
    state.active = active;
    state.roster = roster;
    state.events = events;
    state
  }

  mod state {
    use super::*;
    use crate::clients::esi::scopes;

    fn granted() -> String {
      format!(
        "{} {}",
        scopes::CHARACTER_CALENDAR_READ,
        scopes::CHARACTER_CALENDAR_RESPOND
      )
    }

    mod scope_gate {
      use super::*;

      #[test]
      fn it_gates_a_mine_scope_when_the_active_pilot_lacks_the_scope() {
        let state = state_with(Scope::Mine(1), vec![pilot(1, None)], Vec::new());

        assert!(state.scope_gate().is_some());
      }

      #[test]
      fn it_does_not_gate_an_authorized_mine_scope() {
        let granted = granted();
        let state = state_with(Scope::Mine(1), vec![pilot(1, Some(&granted))], Vec::new());

        assert!(state.scope_gate().is_none());
      }

      #[test]
      fn it_never_gates_the_combined_scope() {
        let state = state_with(Scope::All, vec![pilot(1, None)], Vec::new());

        assert!(state.scope_gate().is_none());
      }
    }

    mod visible_events {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_drops_unauthorized_pilots_from_the_combined_view() {
        let granted = granted();
        let state = state_with(
          Scope::All,
          vec![pilot(1, Some(&granted)), pilot(2, None)],
          vec![
            event(1, 10, "corporation", "accepted"),
            event(2, 11, "corporation", "accepted"),
          ],
        );

        let visible = state.visible_events();

        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].character_id, 1);
      }

      #[test]
      fn it_hides_pod_overlays_unless_the_tweak_is_on() {
        let granted = granted();
        let mut state = state_with(
          Scope::All,
          vec![pilot(1, Some(&granted))],
          vec![
            event(1, 10, "corporation", "accepted"),
            event(1, 11, "pod", "not_responded"),
          ],
        );

        state.tweaks.set_pod_overlays(false);
        assert_eq!(state.visible_events().len(), 1);

        state.tweaks.set_pod_overlays(true);
        assert_eq!(state.visible_events().len(), 2);
      }
    }

    mod unauthorized_pilots {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_names_pilots_missing_the_calendar_scope() {
        let granted = granted();
        let state = state_with(Scope::All, vec![pilot(1, Some(&granted)), pilot(2, None)], Vec::new());

        let unauthorized = state.unauthorized_pilots();

        assert_eq!(unauthorized.len(), 1);
        assert_eq!(unauthorized[0].id, 2);
      }
    }
  }
}
