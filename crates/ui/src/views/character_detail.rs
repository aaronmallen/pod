//! Character detail view: tabbed panel showing clones, standings, contacts, notifications, and killlog.

pub mod clones_tab;
pub mod contacts_tab;
pub mod detail_header;
pub mod killlog_tab;
pub mod notifications_tab;
pub mod standings_tab;

use std::collections::HashMap;

use iced::{
  Background, Element, Length, Padding,
  widget::{column, container, image},
};
use pod_model::{
  Character, CharacterClone, CharacterContact, CharacterContactLabel, CharacterKillEntry, CharacterNotification,
  CharacterStanding, missing_scopes,
};

use crate::{
  components::{ScopeMissing, TabStrip, character_picker, scope_missing},
  style::{color, spacing},
};

/// Builder for the character detail panel element.
pub struct Component<'a> {
  state: &'a State,
}

impl<'a> Component<'a> {
  /// Creates a new component for the given state.
  pub fn new(state: &'a State) -> Self {
    Self {
      state,
    }
  }

  /// Renders the character detail panel.
  pub fn render(self) -> Element<'a, Message> {
    use iced::widget::stack;

    let state = self.state;
    let header = detail_header::Component::new(state).render();
    let tab_bar = detail_tab_strip(state);
    let content = tab_content(state);

    let base: Element<'a, Message> = container(
      column([header, tab_bar, content])
        .width(Length::Fill)
        .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::BASE)),
      ..container::Style::default()
    })
    .into();

    if state.picker.is_open {
      let dropdown = state.picker.dropdown().map(Message::Picker);
      let overlay = container(dropdown)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(iced::alignment::Horizontal::Left)
        .padding(Padding {
          top: spacing::layout::HEADER_HEIGHT + 8.0,
          left: spacing::SPACE_8,
          ..Padding::ZERO
        })
        .into();
      stack([base, overlay]).into()
    } else {
      base
    }
  }
}

/// Filter for the contacts tab.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum ContactFilter {
  /// Show all contacts.
  #[default]
  All,
  /// Show only alliance contacts.
  Alliance,
  /// Show only character contacts.
  Character,
  /// Show only corporation contacts.
  Corp,
}

/// Filter for the killlog tab.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum KilllogFilter {
  /// Show all entries.
  #[default]
  All,
  /// Show kills only.
  Kill,
  /// Show losses only.
  Loss,
}

/// Async load state for a data set.
#[derive(Clone, Debug)]
pub enum LoadState<T> {
  /// An error occurred during loading.
  Error(String),
  /// Data has been successfully loaded.
  Loaded(T),
  /// Data is being fetched.
  Loading,
}

/// Messages produced by the character detail view.
#[derive(Clone, Debug)]
pub enum Message {
  /// The character header switched to a different character.
  CharacterSwitched(i64),
  /// Clones finished loading.
  ClonesLoaded(Result<Vec<CharacterClone>, String>),
  /// The contacts filter changed.
  ContactsFilterChanged(ContactFilter),
  /// Contacts and labels finished loading.
  ContactsLoaded(Result<(Vec<CharacterContact>, Vec<CharacterContactLabel>), String>),
  /// Implant icon bytes finished loading; keyed by type_id.
  ImplantIconsLoaded(Vec<(i32, Vec<u8>)>),
  /// The killlog filter changed.
  KilllogFilterChanged(KilllogFilter),
  /// Killlog entries finished loading.
  KilllogLoaded(Result<Vec<CharacterKillEntry>, String>),
  /// Navigate to a different character's detail view.
  NavigateToDetail(i64),
  /// The notifications filter changed.
  NotificationsFilterChanged(NotificationsFilter),
  /// Notifications finished loading.
  NotificationsLoaded(Result<Vec<CharacterNotification>, String>),
  /// A notification was clicked; mark it read locally.
  NotificationRead(i64),
  /// A message from the character picker component.
  Picker(character_picker::Message),
  /// Re-authorize the character (scope gap detected in a tab).
  ReauthorizeCharacter(i64),
  /// Ship icon bytes finished loading; keyed by type_id.
  ShipIconsLoaded(Vec<(i32, Vec<u8>)>),
  /// Standings finished loading.
  StandingsLoaded(Result<Vec<CharacterStanding>, String>),
  /// The active tab changed.
  TabChanged(Tab),
}

/// Filter for the notifications tab.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum NotificationsFilter {
  /// Show all notifications.
  #[default]
  All,
  /// Show combat notifications.
  Combat,
  /// Show corp notifications.
  Corp,
  /// Show structure notifications.
  Structure,
  /// Show unread notifications only.
  Unread,
  /// Show war notifications.
  War,
}

/// View state for the character detail panel.
#[derive(Debug)]
pub struct State {
  /// Currently selected tab.
  pub active_tab: Tab,
  /// The character whose detail is displayed.
  pub character: Character,
  /// EVE character ID.
  pub character_id: i64,
  /// Loaded clones (active + jump clones).
  pub clones: LoadState<Vec<CharacterClone>>,
  /// Filter state for the contacts tab.
  pub contact_filter: ContactFilter,
  /// Loaded contact label definitions.
  pub contact_labels: Vec<CharacterContactLabel>,
  /// Loaded contacts.
  pub contacts: LoadState<Vec<CharacterContact>>,
  /// Feature: clone_monitoring enabled.
  pub feat_clone_monitoring: bool,
  /// Feature: contacts enabled.
  pub feat_contacts: bool,
  /// Feature: combat_log enabled.
  pub feat_combat_log: bool,
  /// Feature: eve_notifications enabled.
  pub feat_eve_notifications: bool,
  /// Feature: location_tracking enabled.
  pub feat_location_tracking: bool,
  /// Feature: skill_monitoring enabled.
  pub feat_skill_monitoring: bool,
  /// Feature: standings enabled.
  pub feat_standings: bool,
  /// Feature: wallet enabled.
  pub feat_wallet: bool,
  /// Pre-filtered contacts for the contacts tab (updated by controller on load/filter change).
  pub filtered_contacts: Vec<CharacterContact>,
  /// Pre-filtered kill/loss entries for the killlog tab (updated by controller on load/filter change).
  pub filtered_killlog: Vec<CharacterKillEntry>,
  /// Pre-filtered notifications for the notifications tab (updated by controller on load/filter change).
  pub filtered_notifications: Vec<CharacterNotification>,
  /// Cached implant icon handles keyed by EVE type_id.
  pub implant_icons: HashMap<i32, image::Handle>,
  /// Loaded kill/loss entries.
  pub killlog: LoadState<Vec<CharacterKillEntry>>,
  /// Filter state for the killlog tab.
  pub killlog_filter: KilllogFilter,
  /// Loaded notifications.
  pub notifications: LoadState<Vec<CharacterNotification>>,
  /// Filter state for the notifications tab.
  pub notifications_filter: NotificationsFilter,
  /// Character picker component.
  pub picker: character_picker::Component,
  /// Cached ship icon handles keyed by EVE type_id.
  pub ship_icons: HashMap<i32, image::Handle>,
  /// Loaded standings.
  pub standings: LoadState<Vec<CharacterStanding>>,
  /// Total unread notification count across all notifications (updated by controller).
  pub unread_notification_count: usize,
}

/// Active tab in the character detail panel.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Tab {
  #[default]
  Clones,
  Contacts,
  Killlog,
  Notifications,
  Standings,
}

fn loaded_count<T>(state: &LoadState<Vec<T>>) -> Option<usize> {
  match state {
    LoadState::Loaded(v) => Some(v.len()),
    _ => None,
  }
}

type TabEntry = (Tab, &'static str, Option<usize>, usize, bool);

fn all_tab_entries(state: &State) -> [TabEntry; 5] {
  let unread = state.unread_notification_count;
  [
    (
      Tab::Clones,
      "Clones",
      loaded_count(&state.clones),
      0usize,
      state.feat_clone_monitoring,
    ),
    (
      Tab::Contacts,
      "Contacts",
      loaded_count(&state.contacts),
      0,
      state.feat_contacts,
    ),
    (
      Tab::Killlog,
      "Kill log",
      loaded_count(&state.killlog),
      0,
      state.feat_combat_log,
    ),
    (
      Tab::Notifications,
      "Notifications",
      loaded_count(&state.notifications),
      unread,
      state.feat_eve_notifications,
    ),
    (
      Tab::Standings,
      "Standings",
      loaded_count(&state.standings),
      0,
      state.feat_standings,
    ),
  ]
}

fn detail_tab_strip(state: &State) -> Element<'_, Message> {
  let all_tabs = all_tab_entries(state);
  let visible_tabs: Vec<_> = all_tabs.iter().filter(|(_, _, _, _, enabled)| *enabled).collect();
  let active_index = visible_tabs
    .iter()
    .position(|(tab, _, _, _, _)| *tab == state.active_tab)
    .unwrap_or(0);
  let items = visible_tabs
    .iter()
    .map(|(_, label, count, _, _)| crate::components::tab_strip::TabItem {
      label: label.to_string(),
      count: *count,
    })
    .collect();
  let tab_ordering: Vec<Tab> = visible_tabs.iter().map(|(tab, _, _, _, _)| *tab).collect();
  TabStrip::new(items)
    .active(active_index)
    .render(move |i| Message::TabChanged(tab_ordering[i]))
}

fn scope_gate<'a>(char_id: i64, feature: &'static str) -> Element<'a, Message> {
  ScopeMissing::new(char_id, feature).render().map(|m| match m {
    scope_missing::Message::ReauthorizePressed(id) => Message::ReauthorizeCharacter(id),
  })
}

fn tab_content(state: &State) -> Element<'_, Message> {
  let granted = state.character.granted_scopes_list();
  let char_id = state.character_id;
  match state.active_tab {
    Tab::Clones => tab_clones(state, &granted, char_id),
    Tab::Contacts => tab_contacts(state, &granted, char_id),
    Tab::Killlog => tab_killlog(state, &granted, char_id),
    Tab::Notifications => tab_notifications(state, &granted, char_id),
    Tab::Standings => tab_standings(state, &granted, char_id),
  }
}

fn tab_clones<'a>(state: &'a State, granted: &[&str], char_id: i64) -> Element<'a, Message> {
  let required: &[&'static str] = &["esi-clones.read_clones.v1", "esi-clones.read_implants.v1"];
  if !missing_scopes(granted, required).is_empty() {
    return scope_gate(char_id, "clone monitoring");
  }
  clones_tab::Component::new(&state.clones, &state.implant_icons).render()
}

fn tab_contacts<'a>(state: &'a State, granted: &[&str], char_id: i64) -> Element<'a, Message> {
  let required: &[&'static str] = &["esi-characters.read_contacts.v1"];
  if !missing_scopes(granted, required).is_empty() {
    return scope_gate(char_id, "contacts");
  }
  contacts_tab::Component::new(&state.contacts, &state.filtered_contacts, &state.contact_filter).render()
}

fn tab_killlog<'a>(state: &'a State, granted: &[&str], char_id: i64) -> Element<'a, Message> {
  let required: &[&'static str] = &["esi-killmails.read_killmails.v1"];
  if !missing_scopes(granted, required).is_empty() {
    return scope_gate(char_id, "kill log");
  }
  killlog_tab::Component::new(
    &state.killlog,
    &state.filtered_killlog,
    &state.killlog_filter,
    &state.ship_icons,
  )
  .render()
}

fn tab_notifications<'a>(state: &'a State, granted: &[&str], char_id: i64) -> Element<'a, Message> {
  let required: &[&'static str] = &["esi-characters.read_notifications.v1"];
  if !missing_scopes(granted, required).is_empty() {
    return scope_gate(char_id, "notifications");
  }
  notifications_tab::Component::new(
    &state.notifications,
    &state.filtered_notifications,
    state.unread_notification_count,
    &state.notifications_filter,
  )
  .render()
}

fn tab_standings<'a>(state: &'a State, granted: &[&str], char_id: i64) -> Element<'a, Message> {
  let required: &[&'static str] = &["esi-characters.read_standings.v1"];
  if !missing_scopes(granted, required).is_empty() {
    return scope_gate(char_id, "standings");
  }
  standings_tab::Component::new(&state.standings).render()
}
