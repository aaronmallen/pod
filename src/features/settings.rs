pub mod about_tab;
pub mod accessibility_tab;
pub mod captains_log_tab;
pub mod data_export;
pub mod facility_intel_import;
pub mod facility_intel_share;
pub mod facility_tab;
pub mod features_tab;
mod i18n;
pub mod log_export;
pub mod mcp_tab;
pub mod storage_tab;
pub mod tags_tab;
pub mod telemetry_tab;
pub mod ui_tab;

use chrono::{DateTime, Utc};
use iced::{
  Background, Border, Element, Length, Padding, Task,
  alignment::Vertical,
  widget::{Column, Row, Space, button, container, text},
};

use crate::{
  config::{self, Settings},
  store::Database,
  ui::{
    components::{button::Button, header, icon::Icon, rule},
    style::{color, radius, spacing, typography},
  },
};

const CATEGORIES_PANE_WIDTH: f32 = 220.0;
const CATEGORY_ICON_SIZE: f32 = 17.0;
const INDICATOR_WIDTH: f32 = 2.0;
const INDICATOR_INSET: f32 = 8.0;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Category {
  About,
  Accessibility,
  CaptainsLog,
  Facility,
  #[default]
  Features,
  Mcp,
  Storage,
  Tags,
  Telemetry,
  Ui,
}

impl Category {
  pub fn from_id(id: &str) -> Option<Category> {
    match id {
      "about" => Some(Category::About),
      "accessibility" => Some(Category::Accessibility),
      "captains-log" => Some(Category::CaptainsLog),
      "facilities" => Some(Category::Facility),
      "features" => Some(Category::Features),
      "mcp" => Some(Category::Mcp),
      "storage" => Some(Category::Storage),
      "tags" => Some(Category::Tags),
      "telemetry" => Some(Category::Telemetry),
      "ui" => Some(Category::Ui),
      _ => None,
    }
  }

  /// The categories that appear in the normal top-of-rail list, in order. `About` is excluded here
  /// because it is pinned to the bottom of the rail, separated from the rest. `Facility` only appears
  /// when the Industry feature is enabled.
  fn list(settings: &Settings) -> Vec<Category> {
    let mut categories = vec![Category::Accessibility, Category::CaptainsLog];
    if settings.features().is_enabled(config::Feature::Industry) {
      categories.push(Category::Facility);
    }
    categories.push(Category::Features);
    categories.push(Category::Mcp);
    categories.push(Category::Storage);
    categories.push(Category::Tags);
    categories.push(Category::Telemetry);
    categories.push(Category::Ui);
    categories
  }

  pub fn id(self) -> &'static str {
    match self {
      Category::About => "about",
      Category::Accessibility => "accessibility",
      Category::CaptainsLog => "captains-log",
      Category::Facility => "facilities",
      Category::Features => "features",
      Category::Mcp => "mcp",
      Category::Storage => "storage",
      Category::Tags => "tags",
      Category::Telemetry => "telemetry",
      Category::Ui => "ui",
    }
  }

  fn label(self) -> &'static str {
    match self {
      Category::About => i18n::tr_static("settings.shell.category_about"),
      Category::Accessibility => i18n::tr_static("settings.shell.category_accessibility"),
      Category::CaptainsLog => i18n::tr_static("settings.shell.category_captains_log"),
      Category::Facility => i18n::tr_static("settings.shell.category_facility"),
      Category::Features => i18n::tr_static("settings.shell.category_features"),
      Category::Mcp => i18n::tr_static("settings.shell.category_mcp"),
      Category::Storage => i18n::tr_static("settings.shell.category_storage"),
      Category::Tags => i18n::tr_static("settings.shell.category_tags"),
      Category::Telemetry => i18n::tr_static("settings.shell.category_telemetry"),
      Category::Ui => i18n::tr_static("settings.shell.category_ui"),
    }
  }

  fn icon(self) -> Icon {
    match self {
      Category::About => Icon::help(),
      Category::Accessibility => Icon::users(),
      Category::CaptainsLog => Icon::captains_log(),
      Category::Facility => Icon::facilities(),
      Category::Features => Icon::settings(),
      Category::Mcp => Icon::link(),
      Category::Storage => Icon::archive(),
      Category::Tags => Icon::star(),
      Category::Telemetry => Icon::pulse(),
      Category::Ui => Icon::layout(),
    }
  }
}

#[derive(Clone, Debug)]
pub enum Message {
  About(about_tab::Message),
  Accessibility(accessibility_tab::Message),
  CaptainsLog(captains_log_tab::Message),
  CategorySelected(Category),
  Facility(facility_tab::Message),
  Features(features_tab::Message),
  Mcp(mcp_tab::Message),
  ResetToDefaults,
  Storage(storage_tab::Message),
  Tags(tags_tab::Message),
  Telemetry(telemetry_tab::Message),
  Ui(ui_tab::Message),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
  AccessibilityChanged,
  ExportData,
  ExportIntel {
    facilities: Vec<facility_intel_share::PortableFacility>,
  },
  ExportLogs {
    end: DateTime<Utc>,
    start: DateTime<Utc>,
  },
  ImportData {
    path: std::path::PathBuf,
  },
  ImportIntel {
    facilities: Vec<facility_intel_share::PortableFacility>,
  },
  IndustrySearch {
    activity: i64,
    generation: u64,
    query: String,
  },
  LanguageChanged(crate::services::i18n::Language),
  McpChanged,
  None,
  Persist,
  ReleaseLock,
  SetLogLevel(config::LogLevel),
  SyncNow,
  TagsChanged,
  UiChanged,
}

#[derive(Debug)]
pub struct State {
  accessibility: accessibility_tab::State,
  active: Category,
  captains_log: captains_log_tab::State,
  db: Database,
  facility: facility_tab::State,
  features: features_tab::State,
  mcp: mcp_tab::State,
  settings: Settings,
  storage: storage_tab::State,
  tags: tags_tab::State,
  telemetry: telemetry_tab::State,
  ui: ui_tab::State,
}

impl State {
  pub fn new(settings: Settings, db: Database) -> Self {
    let accessibility = accessibility_tab::State::from_settings(&settings);
    let captains_log = captains_log_tab::State::new(db.clone());
    let facility = facility_tab::State::new(db.clone());
    let features = features_tab::State::from_settings(&settings);
    let mcp = mcp_tab::State::from_settings(&settings);
    let storage = storage_tab::State::from_settings(&settings);
    let tags = tags_tab::State::new(db.clone());
    let telemetry = telemetry_tab::State::from_settings(&settings);
    let ui = ui_tab::State::from_settings(&settings);
    State {
      accessibility,
      active: Category::default(),
      captains_log,
      db,
      facility,
      features,
      mcp,
      settings,
      storage,
      tags,
      telemetry,
      ui,
    }
  }

  pub fn active_category(&self) -> Category {
    self.active
  }

  pub fn select_category_by_id(&mut self, id: &str) -> bool {
    match Category::from_id(id) {
      Some(category) => {
        self.active = category;
        true
      }
      None => false,
    }
  }

  pub fn set_clients(
    &mut self,
    esi: std::sync::Arc<crate::clients::esi::Client>,
    sso: std::sync::Arc<crate::clients::eve_sso::Client>,
  ) {
    self.facility.set_clients(esi, sso);
  }

  pub fn set_sync_status(&mut self, holder: Option<String>, last_synced: Option<chrono::DateTime<chrono::Utc>>) {
    self.storage.set_sync_status(holder, last_synced);
  }

  pub fn settings(&self) -> &Settings {
    &self.settings
  }

  pub fn take_storage_migration(&mut self) -> Option<storage_tab::MigrationRequest> {
    self.storage.take_migration()
  }
}

pub fn load(state: &State) -> Task<Message> {
  let mut tasks = vec![
    tags_tab::load(&state.db).map(Message::Tags),
    captains_log_tab::load(&state.db).map(Message::CaptainsLog),
  ];
  if state.settings.features().is_enabled(config::Feature::Industry) {
    tasks.push(facility_tab::load(&state.facility).map(Message::Facility));
  }
  Task::batch(tasks)
}

pub fn subscription(state: &State) -> iced::Subscription<Message> {
  iced::Subscription::batch([
    tags_tab::subscription(&state.tags).map(Message::Tags),
    ui_tab::subscription(&state.ui).map(Message::Ui),
  ])
}

pub fn update(state: &mut State, message: Message) -> (Outcome, Task<Message>) {
  let (outcome, task) = match message {
    Message::About(msg) => (about_tab::update(msg), Task::none()),
    Message::Accessibility(msg) => (
      accessibility_tab::update(&mut state.accessibility, msg, &mut state.settings),
      Task::none(),
    ),
    Message::CaptainsLog(msg) => {
      let (outcome, task) = captains_log_tab::update(&mut state.captains_log, msg);
      (outcome, task.map(Message::CaptainsLog))
    }
    Message::CategorySelected(category) => {
      state.active = category;
      (Outcome::None, Task::none())
    }
    Message::Facility(msg) => {
      let (outcome, task) = facility_tab::update(&mut state.facility, msg, &mut state.settings);
      (outcome, task.map(Message::Facility))
    }
    Message::Features(msg) => {
      record_feature_toggle(&msg);
      (
        features_tab::update(&mut state.features, msg, &mut state.settings),
        Task::none(),
      )
    }
    Message::Mcp(msg) => {
      let (outcome, task) = mcp_tab::update(&mut state.mcp, msg, &mut state.settings);
      (outcome, task.map(Message::Mcp))
    }
    Message::Storage(msg) => (
      storage_tab::update(&mut state.storage, msg, &mut state.settings),
      Task::none(),
    ),
    Message::Tags(msg) => {
      let (outcome, task) = tags_tab::update(&mut state.tags, msg);
      (outcome, task.map(Message::Tags))
    }
    Message::Telemetry(msg) => (
      telemetry_tab::update(&mut state.telemetry, msg, &mut state.settings),
      Task::none(),
    ),
    Message::Ui(msg) => (ui_tab::update(&mut state.ui, msg, &mut state.settings), Task::none()),
    Message::ResetToDefaults => {
      let active = state.active;
      let reset_task = reset_active(state);
      let outcome = match active {
        Category::Accessibility => Outcome::AccessibilityChanged,
        Category::Mcp => Outcome::McpChanged,
        Category::Ui => Outcome::UiChanged,
        _ => Outcome::Persist,
      };
      (outcome, reset_task)
    }
  };
  if matches!(
    outcome,
    Outcome::AccessibilityChanged
      | Outcome::LanguageChanged(_)
      | Outcome::McpChanged
      | Outcome::Persist
      | Outcome::SetLogLevel(_)
      | Outcome::UiChanged
  ) {
    config::save(&state.settings);
  }
  (outcome, task)
}

/// Record a `feature_toggle` usage event for an actual feature toggle, keyed by
/// a stable per-feature config token (§8.1). Search/no-op feature messages emit
/// nothing. The telemetry switches live in [`config::TelemetryConfig`], not the
/// feature flags, so they never reach this path — inspecting/flipping telemetry
/// emits no usage event (§7.6). A structural no-op unless telemetry is built.
fn record_feature_toggle(message: &features_tab::Message) {
  if let Some((token, on)) = feature_toggle_event(message) {
    crate::services::telemetry::record_feature_toggle(token, on);
  }
}

/// Derive the stable `(token, on)` for a feature-toggle message, or `None` for
/// non-toggle feature messages (search). The token is a fixed per-feature config
/// key (group key, or `group.sub` for a sub-feature), never user text. The
/// telemetry switches are NOT feature flags, so they never produce a token here.
fn feature_toggle_event(message: &features_tab::Message) -> Option<(String, bool)> {
  use features_tab::Message;

  use crate::services::telemetry::feature_token;

  match message {
    Message::GroupToggled(group, on) => Some((group.telemetry_key().to_owned(), *on)),
    Message::SubToggled(sub, on) => Some((feature_token(sub.group().legacy_key(), Some(sub.key())), *on)),
    Message::Toggled(feature, on) => Some((feature_token(feature.legacy_key(), None), *on)),
    Message::SearchChanged(_) => None,
  }
}

fn reset_active(state: &mut State) -> Task<Message> {
  let defaults = Settings::default();
  match state.active {
    Category::Accessibility => *state.settings.accessibility_mut() = *defaults.accessibility(),
    Category::CaptainsLog => {
      return captains_log_tab::reset_to_defaults(&mut state.captains_log).map(Message::CaptainsLog);
    }
    Category::Facility if state.settings.features().is_enabled(config::Feature::Industry) => {
      return facility_tab::reset_to_defaults(&state.facility).map(Message::Facility);
    }
    Category::Facility => {}
    Category::Features => *state.settings.features_mut() = *defaults.features(),
    Category::Mcp => {
      *state.settings.mcp_mut() = defaults.mcp().clone();
      state.settings.mcp_mut().token_or_generate();
      state.mcp = mcp_tab::State::from_settings(&state.settings);
    }
    Category::Storage => *state.settings.storage_mut() = defaults.storage().clone(),
    Category::Telemetry => {
      *state.settings.telemetry_mut() = *defaults.telemetry();
      state.telemetry = telemetry_tab::State::from_settings(&state.settings);
    }
    Category::Ui => {
      *state.settings.ui_mut() = defaults.ui().clone();
      state.ui = ui_tab::State::from_settings(&state.settings);
    }
    Category::Tags | Category::About => {}
  }
  Task::none()
}

pub fn view(state: &State) -> Element<'_, Message> {
  let body = Row::with_children(vec![categories_pane(state), active_panel(state)])
    .width(Length::Fill)
    .height(Length::Fill);

  Column::with_children(vec![header(), body.into()])
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn header<'a>() -> Element<'a, Message> {
  let eyebrow = text(t!("settings.shell.eyebrow"))
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(|_| text::Style {
      color: Some(color::text::secondary()),
    });
  let title = text(t!("settings.shell.title"))
    .font(typography::body::MEDIUM)
    .size(typography::size::LG)
    .style(|_| text::Style {
      color: Some(color::text::PRIMARY),
    });
  let identity = Column::with_children(vec![eyebrow.into(), title.into()]).spacing(spacing::UNIT);

  let reset = Button::secondary(t!("settings.shell.reset_to_defaults"))
    .icon(Icon::reset())
    .on_press(Message::ResetToDefaults);

  header::header(vec![identity.into()], vec![reset.into()])
}

fn categories_pane(state: &State) -> Element<'_, Message> {
  let heading = container(
    text(t!("settings.shell.categories"))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      }),
  )
  .padding(Padding {
    top: 0.0,
    right: spacing::SPACE_2,
    bottom: spacing::SPACE_2_5,
    left: spacing::SPACE_2,
  });

  let mut rows: Vec<Element<'_, Message>> = vec![heading.into()];
  for category in Category::list(&state.settings) {
    rows.push(category_row(state, category, badge_for(state, category)));
  }

  rows.push(Space::new().height(Length::Fill).into());
  rows.push(
    container(rule::horizontal())
      .padding(Padding {
        top: 0.0,
        right: 0.0,
        bottom: spacing::SPACE_2,
        left: 0.0,
      })
      .into(),
  );
  rows.push(category_row(state, Category::About, String::new()));

  let column = Column::with_children(rows)
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(Padding {
      top: 18.0,
      right: spacing::SPACE_3_5,
      bottom: spacing::SPACE_2,
      left: spacing::SPACE_3_5,
    });

  let pane = container(column)
    .width(Length::Fixed(CATEGORIES_PANE_WIDTH))
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      ..container::Style::default()
    });

  Row::with_children(vec![pane.into(), rule::vertical_fill(0.1)])
    .height(Length::Fill)
    .into()
}

fn category_row(state: &State, category: Category, badge: String) -> Element<'_, Message> {
  let active = state.active == category;
  let label_color = if active {
    color::text::PRIMARY
  } else {
    color::text::secondary()
  };
  let badge_color = if active {
    color::accent()
  } else {
    color::text::secondary()
  };
  let icon_color = if active {
    color::accent()
  } else {
    color::text::secondary()
  };

  let mut row_children: Vec<Element<'_, Message>> = vec![
    category.icon().size(CATEGORY_ICON_SIZE).color(icon_color).render(),
    text(category.label())
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .width(Length::Fill)
      .style(move |_| text::Style {
        color: Some(label_color),
      })
      .into(),
  ];
  if !badge.is_empty() {
    row_children.push(
      text(badge)
        .font(typography::mono::REGULAR)
        .size(typography::size::XS_PLUS)
        .style(move |_| text::Style {
          color: Some(badge_color),
        })
        .into(),
    );
  }
  let row = Row::with_children(row_children)
    .align_y(Vertical::Center)
    .spacing(spacing::SPACE_2_5);

  let cell = container(row)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_2_5,
      right: spacing::SPACE_3,
      bottom: spacing::SPACE_2_5,
      left: spacing::SPACE_3,
    })
    .style(move |_| container::Style {
      background: active.then(|| Background::Color(color::with_alpha(color::accent(), 0.1))),
      border: Border {
        radius: radius::CONTROL.into(),
        ..Border::default()
      },
      ..container::Style::default()
    });

  let body: Element<'_, Message> = if active {
    let indicator = container(
      container(Space::new())
        .width(Length::Fixed(INDICATOR_WIDTH))
        .height(Length::Fill)
        .style(|_| container::Style {
          background: Some(Background::Color(color::accent())),
          border: Border {
            radius: radius::SUBTLE.into(),
            ..Border::default()
          },
          ..container::Style::default()
        }),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(iced::alignment::Horizontal::Left)
    .padding(Padding {
      top: INDICATOR_INSET,
      right: 0.0,
      bottom: INDICATOR_INSET,
      left: 0.0,
    });

    iced::widget::stack![cell, indicator].into()
  } else {
    cell.into()
  };

  button(body)
    .padding(0)
    .width(Length::Fill)
    .on_press(Message::CategorySelected(category))
    .style(|_, _| button::Style {
      background: Some(Background::Color(iced::Color::TRANSPARENT)),
      ..button::Style::default()
    })
    .into()
}

fn badge_for(state: &State, category: Category) -> String {
  match category {
    Category::About => String::new(),
    Category::Accessibility => accessibility_tab::badge(&state.settings),
    Category::CaptainsLog => captains_log_tab::badge(&state.captains_log),
    Category::Facility => facility_tab::badge(&state.facility),
    Category::Features => features_tab::badge(&state.settings),
    Category::Mcp => mcp_tab::badge(&state.settings),
    Category::Storage => storage_tab::badge(&state.settings),
    Category::Tags => tags_tab::badge(&state.tags),
    Category::Telemetry => telemetry_tab::badge(&state.settings),
    Category::Ui => ui_tab::badge(&state.settings),
  }
}

fn active_panel(state: &State) -> Element<'_, Message> {
  let facility_off = !state.settings.features().is_enabled(config::Feature::Industry);
  let active = if state.active == Category::Facility && facility_off {
    Category::default()
  } else {
    state.active
  };

  match active {
    Category::About => about_tab::view().map(Message::About),
    Category::Accessibility => {
      accessibility_tab::view(&state.accessibility, &state.settings).map(Message::Accessibility)
    }
    Category::CaptainsLog => captains_log_tab::view(&state.captains_log).map(Message::CaptainsLog),
    Category::Facility => facility_tab::view(&state.facility, &state.settings).map(Message::Facility),
    Category::Features => features_tab::view(&state.features, &state.settings).map(Message::Features),
    Category::Mcp => mcp_tab::view(&state.mcp, &state.settings).map(Message::Mcp),
    Category::Storage => storage_tab::view(&state.storage, &state.settings).map(Message::Storage),
    Category::Tags => tags_tab::view(&state.tags, &state.settings).map(Message::Tags),
    Category::Telemetry => telemetry_tab::view(&state.telemetry, &state.settings).map(Message::Telemetry),
    Category::Ui => ui_tab::view(&state.ui, &state.settings).map(Message::Ui),
  }
}

pub fn escape_dismiss(state: &State) -> Option<Message> {
  let facility_off = !state.settings.features().is_enabled(config::Feature::Industry);
  let active = if state.active == Category::Facility && facility_off {
    Category::default()
  } else {
    state.active
  };

  match active {
    Category::Facility => facility_tab::escape_dismiss(&state.facility).map(Message::Facility),
    Category::Storage => storage_tab::escape_dismiss(&state.storage).map(Message::Storage),
    _ => None,
  }
}

#[cfg(test)]
mod tests {
  use pretty_assertions::assert_eq;

  use super::*;

  async fn state() -> State {
    let db = crate::store::open_test().await.unwrap();
    State::new(Settings::default(), db)
  }

  #[tokio::test]
  async fn a_disabled_facility_panel_falls_back_to_the_default_category() {
    let mut state = state().await;
    state
      .settings
      .features_mut()
      .set_enabled(config::Feature::Industry, false);
    state.active = Category::Facility;

    let _el: Element<'_, Message> = view(&state);
  }

  #[tokio::test]
  async fn category_selected_switches_the_active_category() {
    let mut state = state().await;

    let _task = update(&mut state, Message::CategorySelected(Category::Storage));

    assert_eq!(state.active, Category::Storage);
  }

  #[tokio::test]
  async fn it_defaults_to_the_features_category() {
    assert_eq!(state().await.active, Category::Features);
  }

  #[test]
  fn it_round_trips_every_category_through_its_catalog_id() {
    let categories = [
      Category::About,
      Category::Accessibility,
      Category::CaptainsLog,
      Category::Facility,
      Category::Features,
      Category::Mcp,
      Category::Storage,
      Category::Tags,
      Category::Telemetry,
      Category::Ui,
    ];

    assert_eq!(Category::About.id(), "about");
    assert_eq!(Category::Accessibility.id(), "accessibility");
    assert_eq!(Category::CaptainsLog.id(), "captains-log");
    assert_eq!(Category::Facility.id(), "facilities");
    assert_eq!(Category::Features.id(), "features");
    assert_eq!(Category::Mcp.id(), "mcp");
    assert_eq!(Category::Storage.id(), "storage");
    assert_eq!(Category::Tags.id(), "tags");
    assert_eq!(Category::Telemetry.id(), "telemetry");
    assert_eq!(Category::Ui.id(), "ui");

    for category in categories {
      assert_eq!(Category::from_id(category.id()), Some(category));
    }
  }

  #[test]
  fn feature_toggle_events_carry_a_stable_contract_shaped_token() {
    use features_tab::Message;

    use crate::services::telemetry::is_well_formed_token;

    let (token, on) = feature_toggle_event(&Message::GroupToggled(features_tab::Group::Wallet, false)).unwrap();
    assert_eq!(token, "wallet");
    assert!(!on);

    let (token, on) = feature_toggle_event(&Message::SubToggled(config::SubFeature::Budget, true)).unwrap();
    assert_eq!(token, "wallet.budget");
    assert!(on);

    let (token, _) = feature_toggle_event(&Message::Toggled(config::Feature::AssetTracking, true)).unwrap();
    assert_eq!(token, "asset_tracking");

    assert!(feature_toggle_event(&Message::SearchChanged("x".to_owned())).is_none());

    for sub in config::SubFeature::ALL {
      let (token, _) = feature_toggle_event(&Message::SubToggled(sub, true)).unwrap();
      assert!(is_well_formed_token(&token), "sub token `{token}` is malformed");
    }
    for feature in config::Feature::ALL {
      let (token, _) = feature_toggle_event(&Message::Toggled(feature, true)).unwrap();
      assert!(is_well_formed_token(&token), "feature token `{token}` is malformed");
    }
  }

  #[test]
  fn every_category_renders_a_rail_cascade_icon() {
    let categories = [
      Category::About,
      Category::Accessibility,
      Category::CaptainsLog,
      Category::Facility,
      Category::Features,
      Category::Mcp,
      Category::Storage,
      Category::Tags,
      Category::Telemetry,
      Category::Ui,
    ];

    for category in categories {
      let _icon: Element<'_, Message> = category.icon().size(CATEGORY_ICON_SIZE).render();
    }
  }

  #[tokio::test]
  async fn category_rows_render_their_icon_before_the_label() {
    let state = state().await;
    for category in [Category::Accessibility, Category::Features, Category::About] {
      let _row: Element<'_, Message> = category_row(&state, category, String::new());
    }
  }

  #[tokio::test]
  async fn reset_on_accessibility_clears_high_contrast() {
    let mut state = state().await;
    state.active = Category::Accessibility;
    state.settings.accessibility_mut().set_high_contrast(true);

    let (outcome, _task) = update(&mut state, Message::ResetToDefaults);

    assert_eq!(outcome, Outcome::AccessibilityChanged);
    assert!(!*state.settings.accessibility().high_contrast());
  }

  #[tokio::test]
  async fn reset_on_accessibility_restores_the_default_scale_and_signals_a_live_change() {
    let mut state = state().await;
    state.active = Category::Accessibility;
    state.settings.accessibility_mut().set_scale(125);

    let (outcome, _task) = update(&mut state, Message::ResetToDefaults);

    assert_eq!(outcome, Outcome::AccessibilityChanged);
    assert_eq!(*state.settings.accessibility().scale(), 100);
  }

  #[tokio::test]
  async fn reset_on_features_does_not_touch_storage() {
    let mut state = state().await;
    state.settings.storage_mut().set_network(true);
    state.active = Category::Features;

    let _task = update(&mut state, Message::ResetToDefaults);

    assert!(
      state.settings.storage().network(),
      "resetting Features must leave the Storage category alone"
    );
  }

  #[tokio::test]
  async fn reset_on_facility_persists_when_the_feature_is_disabled() {
    let mut state = state().await;
    state
      .settings
      .features_mut()
      .set_enabled(config::Feature::Industry, false);
    state.active = Category::Facility;

    let (outcome, _task) = update(&mut state, Message::ResetToDefaults);

    assert_eq!(outcome, Outcome::Persist);
  }

  #[tokio::test]
  async fn reset_on_facility_persists_when_the_feature_is_enabled() {
    let mut state = state().await;
    state
      .settings
      .features_mut()
      .set_enabled(config::Feature::Industry, true);
    state.active = Category::Facility;

    let (outcome, _task) = update(&mut state, Message::ResetToDefaults);

    assert_eq!(outcome, Outcome::Persist);
  }

  #[tokio::test]
  async fn reset_on_storage_restores_the_default_network_setting() {
    let mut state = state().await;
    state.settings.storage_mut().set_network(true);
    state.active = Category::Storage;

    let (outcome, _task) = update(&mut state, Message::ResetToDefaults);

    assert_eq!(outcome, Outcome::Persist);
    assert!(!state.settings.storage().network());
  }

  #[tokio::test]
  async fn reset_on_tags_is_a_no_op() {
    let mut state = state().await;
    state.active = Category::Tags;

    let (outcome, _task) = update(&mut state, Message::ResetToDefaults);

    assert_eq!(outcome, Outcome::Persist);
  }

  #[tokio::test]
  async fn reset_on_telemetry_restores_every_stream_to_on() {
    let mut state = state().await;
    state.settings.telemetry_mut().set_enabled(false);
    state.settings.telemetry_mut().set_usage(false);
    state.settings.telemetry_mut().set_performance(false);
    state.settings.telemetry_mut().set_crashes(false);
    state.settings.telemetry_mut().set_environment(false);
    state.active = Category::Telemetry;

    let (outcome, _task) = update(&mut state, Message::ResetToDefaults);

    assert_eq!(outcome, Outcome::Persist);
    assert!(*state.settings.telemetry().enabled());
    assert!(*state.settings.telemetry().usage());
    assert!(*state.settings.telemetry().performance());
    assert!(*state.settings.telemetry().crashes());
    assert!(*state.settings.telemetry().environment());
  }

  #[tokio::test]
  async fn reset_on_ui_restores_the_default_layout_and_signals_a_live_change() {
    let mut state = state().await;
    state
      .settings
      .ui_mut()
      .set_nav_location(crate::config::NavLocation::Right);
    state.active = Category::Ui;

    let (outcome, _task) = update(&mut state, Message::ResetToDefaults);

    assert_eq!(outcome, Outcome::UiChanged);
    assert_eq!(*state.settings.ui().nav_location(), crate::config::NavLocation::Left);
  }

  #[tokio::test]
  async fn reset_to_defaults_restores_the_active_category() {
    let mut state = state().await;
    state
      .settings
      .features_mut()
      .set_enabled(crate::config::Feature::Wallet, false);
    assert!(!state.settings.features().is_enabled(crate::config::Feature::Wallet));

    let _task = update(&mut state, Message::ResetToDefaults);

    assert!(
      state.settings.features().is_enabled(crate::config::Feature::Wallet),
      "Features reset should re-enable wallet"
    );
  }

  #[tokio::test]
  async fn the_about_category_can_be_selected() {
    let mut state = state().await;

    let _task = update(&mut state, Message::CategorySelected(Category::About));

    assert_eq!(state.active, Category::About);
  }

  #[tokio::test]
  async fn the_facility_category_appears_when_the_feature_is_enabled() {
    let mut settings = Settings::default();
    settings.features_mut().set_enabled(config::Feature::Industry, true);

    assert!(Category::list(&settings).contains(&Category::Facility));
  }

  #[tokio::test]
  async fn the_facility_category_is_hidden_when_the_feature_is_disabled() {
    let mut settings = Settings::default();
    settings.features_mut().set_enabled(config::Feature::Industry, false);

    assert!(!Category::list(&settings).contains(&Category::Facility));
  }

  #[tokio::test]
  async fn escape_on_storage_defers_to_the_storage_tab() {
    let mut state = state().await;
    state.active = Category::Storage;

    assert!(escape_dismiss(&state).is_none());
  }

  #[tokio::test]
  async fn escape_on_facility_defers_to_the_facility_tab() {
    let mut state = state().await;
    state
      .settings
      .features_mut()
      .set_enabled(config::Feature::Industry, true);
    state.active = Category::Facility;

    assert!(escape_dismiss(&state).is_none());
  }

  #[tokio::test]
  async fn escape_on_a_disabled_facility_falls_back_to_the_default_category() {
    let mut state = state().await;
    state
      .settings
      .features_mut()
      .set_enabled(config::Feature::Industry, false);
    state.active = Category::Facility;

    assert!(escape_dismiss(&state).is_none());
  }

  #[tokio::test]
  async fn escape_on_another_category_dismisses_nothing() {
    let mut state = state().await;
    state.active = Category::Features;

    assert!(escape_dismiss(&state).is_none());
  }

  #[tokio::test]
  async fn view_renders_each_category() {
    let categories = Category::list(&Settings::default())
      .into_iter()
      .chain([Category::Facility, Category::About]);

    for category in categories {
      let mut state = state().await;
      state
        .settings
        .features_mut()
        .set_enabled(config::Feature::Industry, true);
      state.active = category;
      let _el: Element<'_, Message> = view(&state);
    }
  }
}
