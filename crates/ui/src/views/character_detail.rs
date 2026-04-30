//! Character detail view: tabbed panel showing clones, standings, contacts, notifications, and killlog.

pub mod clones_tab;
pub mod contacts_tab;
pub mod killlog_tab;
pub mod notifications_tab;
pub mod standings_tab;

use std::collections::HashMap;

use iced::{
  Background, Border, Element, Length, Padding, Theme,
  widget::{Space, column, container, image, row, text},
};
use pod_model::{
  Character, CharacterClone, CharacterContact, CharacterContactLabel, CharacterKillEntry, CharacterNotification,
  CharacterStanding,
};

use crate::{
  components::{TabStrip, character_picker},
  style::{color, spacing, typography as font},
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
    let header = detail_header(state);
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

fn detail_header(state: &State) -> Element<'_, Message> {
  let picker = state.picker.render().map(Message::Picker);

  let divider = || {
    container(Space::new().width(1.0).height(44.0)).style(|_| container::Style {
      background: Some(Background::Color(color::border::SUBTLE)),
      ..container::Style::default()
    })
  };

  let isk_formatted = format!("{} ISK", state.character.isk_formatted());
  let location_name = state
    .character
    .location_name()
    .clone()
    .unwrap_or_else(|| "\u{2014}".to_string());

  let total_sp: i64 = state.character.skills().iter().map(|s| s.skillpoints).sum();
  let sp_formatted = if total_sp >= 1_000_000 {
    format!("{:.1}M", total_sp as f64 / 1_000_000.0)
  } else if total_sp > 0 {
    format!("{:.0}K", total_sp as f64 / 1_000.0)
  } else {
    "\u{2014}".to_string()
  };

  let sp_stat = head_stat("Total SP", &sp_formatted);
  let isk_stat = head_stat("Liquid", &isk_formatted);
  let location_stat = head_stat("Location", &location_name);

  container(
    row([
      picker,
      divider().into(),
      sp_stat,
      divider().into(),
      isk_stat,
      divider().into(),
      location_stat,
      Space::new().width(Length::Fill).into(),
    ])
    .align_y(iced::alignment::Vertical::Center)
    .spacing(spacing::SPACE_4)
    .padding(Padding {
      top: 0.0,
      bottom: 0.0,
      left: spacing::SPACE_7,
      right: spacing::SPACE_7,
    }),
  )
  .width(Length::Fill)
  .center_y(spacing::layout::HEADER_HEIGHT)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::BASE)),
    border: Border {
      color: color::border::SUBTLE,
      width: 1.0,
      ..Border::default()
    },
    ..container::Style::default()
  })
  .into()
}

fn head_stat(label: &str, value: &str) -> Element<'static, Message> {
  column([
    text(label.to_uppercase())
      .font(font::mono::REGULAR)
      .size(9.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
    Space::new().height(4.0).into(),
    text(value.to_string())
      .font(font::mono::MEDIUM)
      .size(15.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
  ])
  .into()
}

fn detail_tab_strip(state: &State) -> Element<'_, Message> {
  let unread_notifications = state.unread_notification_count;

  let clones_count = match &state.clones {
    LoadState::Loaded(c) => Some(c.len()),
    _ => None,
  };
  let contacts_count = match &state.contacts {
    LoadState::Loaded(c) => Some(c.len()),
    _ => None,
  };
  let killlog_count = match &state.killlog {
    LoadState::Loaded(k) => Some(k.len()),
    _ => None,
  };
  let notifications_count = match &state.notifications {
    LoadState::Loaded(n) => Some(n.len()),
    _ => None,
  };
  let standings_count = match &state.standings {
    LoadState::Loaded(s) => Some(s.len()),
    _ => None,
  };

  let tabs = [
    (Tab::Clones, "Clones", clones_count, 0usize),
    (Tab::Contacts, "Contacts", contacts_count, 0),
    (Tab::Killlog, "Kill log", killlog_count, 0),
    (
      Tab::Notifications,
      "Notifications",
      notifications_count,
      unread_notifications,
    ),
    (Tab::Standings, "Standings", standings_count, 0),
  ];

  let active_index = tabs
    .iter()
    .position(|(tab, _, _, _)| *tab == state.active_tab)
    .unwrap_or(0);

  let items = tabs
    .iter()
    .map(|(_, label, count, _)| crate::components::tab_strip::TabItem {
      label: label.to_string(),
      count: *count,
    })
    .collect();

  let tab_ordering = [
    Tab::Clones,
    Tab::Contacts,
    Tab::Killlog,
    Tab::Notifications,
    Tab::Standings,
  ];

  TabStrip::new(items)
    .active(active_index)
    .render(move |i| Message::TabChanged(tab_ordering[i]))
}

fn tab_content(state: &State) -> Element<'_, Message> {
  match state.active_tab {
    Tab::Clones => clones_tab::Component::new(&state.clones, &state.implant_icons).render(),
    Tab::Contacts => {
      contacts_tab::Component::new(&state.contacts, &state.filtered_contacts, &state.contact_filter).render()
    }
    Tab::Killlog => killlog_tab::Component::new(
      &state.killlog,
      &state.filtered_killlog,
      &state.killlog_filter,
      &state.ship_icons,
    )
    .render(),
    Tab::Notifications => notifications_tab::Component::new(
      &state.notifications,
      &state.filtered_notifications,
      state.unread_notification_count,
      &state.notifications_filter,
    )
    .render(),
    Tab::Standings => standings_tab::Component::new(&state.standings).render(),
  }
}
