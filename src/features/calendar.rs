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
mod time;
mod week;
mod year;

use chrono::{DateTime, Utc};
use iced::{Element, Task};

pub use self::{
  loaders::{CalendarEvent, RosterPilot},
  palette::Response,
};
use crate::{
  config::{CalendarTweaks, FeatureFlags},
  store::{Database, images, model::AttendeeTally},
};

pub const EMPTY_CALENDAR_SELECTION: i64 = 0;

pub const EVENT_WINDOW_HEIGHT: f32 = 760.0;

pub const EVENT_WINDOW_WIDTH: f32 = 660.0;

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

  pub fn from_id(id: &str) -> Option<View> {
    match id {
      "agenda" => Some(View::Agenda),
      "day" => Some(View::Day),
      "month" => Some(View::Month),
      "week" => Some(View::Week),
      "year" => Some(View::Year),
      _ => None,
    }
  }

  pub fn id(self) -> &'static str {
    match self {
      View::Agenda => "agenda",
      View::Day => "day",
      View::Month => "month",
      View::Week => "week",
      View::Year => "year",
    }
  }

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
  EventOpened(i64, i64),
  FeaturesChanged(FeatureFlags),
  Loaded(Box<Loaded>),
  PickerToggled,
  ReauthRequested(i64),
  ScopeSelected(Scope),
  ViewSelected(View),
}

/// A per-window message for a detached calendar-event window. Each open event lives in its own native
/// OS window keyed by `window::Id`, so these never flow through the main calendar `update`; the app
/// shell routes them to [`event_window_update`].
#[derive(Clone, Debug)]
pub enum EventMessage {
  AttendeesLoaded(Box<Option<AttendeeTally>>),
  Responded(Response),
  RsvpWritten,
}

impl Message {
  /// Whether handling this message can surface new image-bearing rows (roster portraits), so the shell should
  /// recheck for stale images. Interaction-only messages return `false` to keep the staleness scan off the
  /// per-frame path.
  pub fn loads_data(&self) -> bool {
    matches!(self, Message::Loaded(_))
  }
}

#[derive(Debug)]
pub struct State {
  active: Scope,
  cursor: DateTime<Utc>,
  events: Vec<CalendarEvent>,
  features: FeatureFlags,
  picker_open: bool,
  roster: Vec<RosterPilot>,
  tweaks: CalendarTweaks,
  view: View,
}

impl State {
  pub fn new(active: i64, cursor: DateTime<Utc>, features: FeatureFlags) -> Self {
    State {
      active: if active == EMPTY_CALENDAR_SELECTION {
        Scope::All
      } else {
        Scope::Mine(active)
      },
      cursor,
      events: Vec::new(),
      features,
      picker_open: false,
      roster: Vec::new(),
      tweaks: CalendarTweaks::default(),
      view: View::default(),
    }
  }

  pub fn active(&self) -> Scope {
    self.active
  }

  pub fn active_view(&self) -> View {
    self.view
  }

  pub fn select_view_by_id(&mut self, id: &str) -> bool {
    match View::from_id(id) {
      Some(view) => {
        self.view = view;
        true
      }
      None => false,
    }
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

  /// Resolves the data a detached event window needs from the live calendar: the event itself plus
  /// the owning pilot's display name. Returns `None` when the (character, event) pair is no longer
  /// visible (e.g. the scope changed before the click resolved).
  pub fn event_for(&self, character_id: i64, event_id: i64) -> Option<(CalendarEvent, Option<String>)> {
    let event = self
      .visible_events()
      .into_iter()
      .find(|event| event.character_id == character_id && event.event_id == event_id)?
      .clone();
    let pilot_name = self.pilot(character_id).map(|pilot| pilot.name.clone());
    Some((event, pilot_name))
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

  fn set_features(&mut self, features: FeatureFlags) {
    self.features = features;
  }
}

/// Per-window state for a detached calendar-event window: the resolved event, its loaded attendee
/// tally (once the endpoint resolves), the owning pilot's display name, whether to mirror EVE times
/// into local time, and the response value last seen locally (used to compensate an RSVP write).
#[derive(Clone, Debug)]
pub struct EventWindow {
  attendees: Option<AttendeeTally>,
  event: CalendarEvent,
  local_time: bool,
  pilot_name: Option<String>,
  previous_response: String,
}

impl EventWindow {
  pub fn new(event: CalendarEvent, pilot_name: Option<String>, local_time: bool, previous_response: String) -> Self {
    EventWindow {
      attendees: None,
      event,
      local_time,
      pilot_name,
      previous_response,
    }
  }

  #[cfg(test)]
  pub(super) fn with_attendees(mut self, attendees: Option<AttendeeTally>) -> Self {
    self.attendees = attendees;
    self
  }

  #[cfg(test)]
  pub(super) fn character_id(&self) -> i64 {
    self.event.character_id
  }

  #[cfg(test)]
  pub(super) fn event_id(&self) -> i64 {
    self.event.event_id
  }

  pub fn title(&self) -> &str {
    &self.event.title
  }

  pub(super) fn owner_kind(&self) -> palette::OwnerType {
    self.event.owner_kind()
  }

  fn set_attendees(&mut self, attendees: Option<AttendeeTally>) {
    self.attendees = attendees;
  }
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
    // Opening an event is intercepted by the app shell to spawn a native window before this update
    // runs, so reaching this arm is a no-op safety net.
    Message::EventOpened(_, _) => Task::none(),
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
    Message::FeaturesChanged(features) => {
      state.set_features(features);
      Task::none()
    }
    Message::PickerToggled => {
      state.picker_open = !state.picker_open;
      Task::none()
    }
    Message::ReauthRequested(_) => Task::none(),
    Message::ScopeSelected(scope) => {
      state.active = scope;
      state.picker_open = false;
      reload(db, scope, state.features)
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

/// Loads the attendee tally for a freshly opened event window. The result threads back as
/// [`EventMessage::AttendeesLoaded`], routed to that window by its `window::Id` in the app shell.
pub fn load_event_attendees(db: &Database, character_id: i64, event_id: i64) -> Task<EventMessage> {
  Task::perform(load_attendees(db.clone(), character_id, event_id), |tally| {
    EventMessage::AttendeesLoaded(Box::new(tally))
  })
}

/// Applies a per-window event message to its [`EventWindow`]: adopting a loaded attendee tally,
/// optimistically flipping the local response and enqueuing the RSVP write, or absorbing the
/// write-completed acknowledgement. RSVP writes need the database, so this returns the follow-up task.
pub fn event_window_update(window: &mut EventWindow, message: EventMessage, db: &Database) -> Task<EventMessage> {
  match message {
    EventMessage::AttendeesLoaded(tally) => {
      window.set_attendees(*tally);
      Task::none()
    }
    EventMessage::Responded(response) => {
      let previous = std::mem::replace(&mut window.previous_response, response.as_esi().to_owned());
      window.event.response = response.as_esi().to_owned();
      Task::perform(
        respond::respond(
          db.clone(),
          window.event.character_id,
          window.event.event_id,
          response.as_esi().to_owned(),
          previous,
        ),
        |()| EventMessage::RsvpWritten,
      )
    }
    EventMessage::RsvpWritten => Task::none(),
  }
}

/// The detached event window's content: an in-content header (subject as the OS-mirrored title plus a
/// close affordance) above the scrollable event card.
pub fn event_window_view(window: &EventWindow) -> Element<'_, EventMessage> {
  shell::event_window(window)
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

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn it_round_trips_every_view_through_its_catalog_id() {
    use pretty_assertions::assert_eq;

    assert_eq!(View::Agenda.id(), "agenda");
    assert_eq!(View::Day.id(), "day");
    assert_eq!(View::Month.id(), "month");
    assert_eq!(View::Week.id(), "week");
    assert_eq!(View::Year.id(), "year");

    for view in View::ALL {
      assert_eq!(View::from_id(view.id()), Some(view));
    }
  }

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
    let mut state = State::new(EMPTY_CALENDAR_SELECTION, now(), FeatureFlags::default());
    state.active = active;
    state.roster = roster;
    state.events = events;
    state
  }

  fn granted_str() -> String {
    format!(
      "{} {}",
      crate::clients::esi::scopes::CHARACTER_CALENDAR_READ,
      crate::clients::esi::scopes::CHARACTER_CALENDAR_RESPOND
    )
  }

  #[allow(clippy::too_many_arguments)]
  fn ev(
    character_id: i64,
    event_id: i64,
    owner_type: &str,
    response: &str,
    timestamp: &str,
    duration_minutes: i64,
    importance: i64,
    source: Option<&str>,
  ) -> CalendarEvent {
    CalendarEvent {
      body: Some("<p>Form up at Jita.</p>".to_owned()),
      character_id,
      duration_minutes,
      event_id,
      importance,
      owner_name: "Owner".to_owned(),
      owner_type: owner_type.to_owned(),
      response: response.to_owned(),
      source: source.map(str::to_owned),
      timestamp: timestamp.to_owned(),
      title: "Fleet Op".to_owned(),
    }
  }

  /// A combined-scope state with an authorized pilot, an unauthorized pilot, and a spread of events
  /// on the cursor day (timed, overlapping, all-day, instant, and a pod overlay) plus a later event,
  /// exercising every view's render branches.
  fn populated() -> State {
    let granted = granted_str();
    let roster = vec![pilot(1, Some(&granted)), pilot(2, None)];
    let events = vec![
      ev(1, 10, "corporation", "accepted", "2026-06-12T19:00:00Z", 90, 1, None),
      ev(1, 11, "faction", "tentative", "2026-06-12T19:30:00Z", 60, 0, None),
      ev(1, 12, "alliance", "declined", "2026-06-12T00:00:00Z", 1440, 0, None),
      ev(1, 13, "character", "not_responded", "2026-06-12T12:00:00Z", 0, 0, None),
      ev(
        1,
        14,
        "pod",
        "not_responded",
        "2026-06-12T08:00:00Z",
        30,
        0,
        Some("skill"),
      ),
      ev(1, 15, "character", "accepted", "2026-06-20T19:00:00Z", 90, 0, None),
    ];
    state_with(Scope::All, roster, events)
  }

  mod dispatch {
    use super::*;

    #[tokio::test]
    async fn it_dispatches_every_message_variant() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = populated();
      let n = now();

      let _ = update(&mut state, Message::ViewSelected(View::Week), &db, n);
      let _ = update(&mut state, Message::CursorNext, &db, n);
      let _ = update(&mut state, Message::CursorPrev, &db, n);
      let _ = update(&mut state, Message::CursorToday, &db, n);
      let _ = update(&mut state, Message::DatePicked(n, View::Day), &db, n);
      // EventOpened is intercepted by the app shell to spawn a window, so its update arm is a no-op.
      let _ = update(&mut state, Message::EventOpened(1, 10), &db, n);
      let _ = update(&mut state, Message::PickerToggled, &db, n);
      let _ = update(&mut state, Message::ReauthRequested(1), &db, n);
      let _ = update(&mut state, Message::ScopeSelected(Scope::Mine(1)), &db, n);

      // A load that matches the active scope is adopted; a stale one is dropped.
      let fresh = Loaded {
        events: Vec::new(),
        roster: Vec::new(),
        scope: state.active,
      };
      let _ = update(&mut state, Message::Loaded(Box::new(fresh)), &db, n);
      let stale = Loaded {
        events: Vec::new(),
        roster: Vec::new(),
        scope: Scope::Mine(424_242),
      };
      let _ = update(&mut state, Message::Loaded(Box::new(stale)), &db, n);
    }
  }

  mod loading {
    use super::*;

    #[tokio::test]
    async fn it_loads_each_scope_against_an_empty_store() {
      let db = crate::store::open_test().await.unwrap();
      let features = FeatureFlags::default();

      let combined = load_calendar(db.clone(), Scope::All, features).await;
      assert!(combined.events.is_empty());

      let mine = load_calendar(db.clone(), Scope::Mine(1), features).await;
      assert!(mine.events.is_empty());

      let empty = load_calendar(db.clone(), Scope::Empty, features).await;
      assert!(empty.events.is_empty());
    }
  }

  mod rendering {
    use super::*;

    fn render_every_view(state: &mut State) {
      for selected in View::ALL {
        state.view = selected;
        let _el: Element<'_, Message> = view(state, now());
      }
    }

    #[test]
    fn it_renders_every_view_with_owner_coloring_dots_and_a_compact_week() {
      let mut state = populated();
      state.tweaks.set_color_by_pilot(false);
      state.tweaks.set_local_time(false);
      state.tweaks.set_month_chips(false);
      state.tweaks.set_show_weekends(false);
      state.tweaks.set_density(crate::config::CalendarDensity::Compact);
      state.tweaks.set_week_start(crate::config::CalendarWeekStart::Sunday);
      render_every_view(&mut state);
    }

    #[test]
    fn it_renders_every_view_with_the_default_tweaks() {
      let mut state = populated();
      render_every_view(&mut state);
    }

    #[test]
    fn it_renders_the_account_picker_overlay() {
      let mut state = populated();
      state.picker_open = true;
      let _el: Element<'_, Message> = view(&state, now());
    }

    #[test]
    fn it_renders_the_empty_agenda_state() {
      let mut state = state_with(Scope::All, Vec::new(), Vec::new());
      state.view = View::Agenda;
      let _el: Element<'_, Message> = view(&state, now());
    }

    #[test]
    fn it_renders_the_forbidden_gate_for_an_unauthorized_pilot() {
      let mut state = populated();
      state.active = Scope::Mine(2);
      let _el: Element<'_, Message> = view(&state, now());
    }

    #[test]
    fn it_renders_a_pod_overlay_event_window() {
      let state = populated();
      let (event, pilot_name) = state.event_for(1, 14).unwrap();
      let window = EventWindow::new(
        event,
        pilot_name,
        false,
        palette::Response::NotResponded.as_esi().to_owned(),
      );

      let _el: Element<'_, EventMessage> = event_window_view(&window);
    }

    #[test]
    fn it_renders_a_respondable_event_window_with_attendees() {
      let state = populated();
      let (event, pilot_name) = state.event_for(1, 10).unwrap();
      let window = EventWindow::new(
        event,
        pilot_name,
        true,
        palette::Response::NotResponded.as_esi().to_owned(),
      )
      .with_attendees(Some(AttendeeTally {
        accepted: 3,
        declined: 1,
        invited: 6,
        tentative: 2,
      }));

      let _el: Element<'_, EventMessage> = event_window_view(&window);
    }
  }

  mod event_window {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_loads_attendees_and_writes_an_optimistic_response() {
      let db = crate::store::open_test().await.unwrap();
      let state = populated();
      let (event, pilot_name) = state.event_for(1, 10).unwrap();
      let mut window = EventWindow::new(
        event,
        pilot_name,
        true,
        palette::Response::NotResponded.as_esi().to_owned(),
      );

      let _ = load_event_attendees(&db, window.character_id(), window.event_id());
      let _ = event_window_update(
        &mut window,
        EventMessage::AttendeesLoaded(Box::new(Some(AttendeeTally {
          accepted: 1,
          declined: 0,
          invited: 2,
          tentative: 1,
        }))),
        &db,
      );
      assert!(window.attendees.is_some());

      let _ = event_window_update(&mut window, EventMessage::Responded(palette::Response::Accepted), &db);
      assert_eq!(window.event.response, "accepted");

      let _ = event_window_update(&mut window, EventMessage::RsvpWritten, &db);
    }

    #[test]
    fn it_returns_none_for_an_event_outside_the_visible_set() {
      let state = populated();
      assert!(state.event_for(1, 9_999).is_none());
    }
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
      fn it_does_not_gate_an_authorized_mine_scope() {
        let granted = granted();
        let state = state_with(Scope::Mine(1), vec![pilot(1, Some(&granted))], Vec::new());

        assert!(state.scope_gate().is_none());
      }

      #[test]
      fn it_gates_a_mine_scope_when_the_active_pilot_lacks_the_scope() {
        let state = state_with(Scope::Mine(1), vec![pilot(1, None)], Vec::new());

        assert!(state.scope_gate().is_some());
      }

      #[test]
      fn it_never_gates_the_combined_scope() {
        let state = state_with(Scope::All, vec![pilot(1, None)], Vec::new());

        assert!(state.scope_gate().is_none());
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
  }
}
