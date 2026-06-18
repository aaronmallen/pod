pub mod about_tab;
pub mod accessibility_tab;
pub mod features_tab;
pub mod industry_tab;
pub mod log_export;
pub mod storage_tab;
pub mod tags_tab;
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
    components::{header, rule},
    style::{color, radius, spacing, typography},
  },
};

const CATEGORIES_PANE_WIDTH: f32 = 220.0;
const INDICATOR_WIDTH: f32 = 2.0;
const INDICATOR_INSET: f32 = 8.0;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Category {
  About,
  Accessibility,
  #[default]
  Features,
  Industry,
  Storage,
  Tags,
  Ui,
}

impl Category {
  /// The categories that appear in the normal top-of-rail list, in order. `About` is excluded here
  /// because it is pinned to the bottom of the rail, separated from the rest. `Industry` only appears
  /// when the Industry feature is enabled.
  fn list(settings: &Settings) -> Vec<Category> {
    let mut categories = vec![Category::Accessibility, Category::Features];
    if *settings.features().industry() {
      categories.push(Category::Industry);
    }
    categories.push(Category::Storage);
    categories.push(Category::Tags);
    categories.push(Category::Ui);
    categories
  }

  fn label(self) -> &'static str {
    match self {
      Category::About => "About",
      Category::Accessibility => "Accessibility",
      Category::Features => "Features",
      Category::Industry => "Industry",
      Category::Storage => "Storage",
      Category::Tags => "Tags",
      Category::Ui => "User Interface",
    }
  }
}

#[derive(Clone, Debug)]
pub enum Message {
  About(about_tab::Message),
  Accessibility(accessibility_tab::Message),
  CategorySelected(Category),
  Features(features_tab::Message),
  Industry(industry_tab::Message),
  ResetToDefaults,
  Storage(storage_tab::Message),
  Tags(tags_tab::Message),
  Ui(ui_tab::Message),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
  AccessibilityChanged,
  ExportLogs {
    end: DateTime<Utc>,
    start: DateTime<Utc>,
  },
  IndustryPin(crate::features::industry::PinnedStructure),
  IndustrySearch {
    activity: i64,
    generation: u64,
    query: String,
  },
  None,
  Persist,
  ReleaseLock,
  SetLogLevel(config::LogLevel),
  SyncNow,
  UiChanged,
}

#[derive(Debug)]
pub struct State {
  accessibility: accessibility_tab::State,
  active: Category,
  db: Database,
  features: features_tab::State,
  industry: industry_tab::State,
  settings: Settings,
  storage: storage_tab::State,
  tags: tags_tab::State,
  ui: ui_tab::State,
}

impl State {
  pub fn new(settings: Settings, db: Database) -> Self {
    let accessibility = accessibility_tab::State::from_settings(&settings);
    let features = features_tab::State::from_settings(&settings);
    let industry = industry_tab::State::from_settings(&settings);
    let storage = storage_tab::State::from_settings(&settings);
    let tags = tags_tab::State::new(db.clone());
    let ui = ui_tab::State::from_settings(&settings);
    State {
      accessibility,
      active: Category::default(),
      db,
      features,
      industry,
      settings,
      storage,
      tags,
      ui,
    }
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
  let tags = tags_tab::load(&state.db).map(Message::Tags);
  if !*state.settings.features().industry() {
    return tags;
  }

  let db = state.db.clone();
  let manufacturing = *state.settings.industry().manufacturing();
  let reactions = *state.settings.industry().reactions();
  let defaults = Task::perform(
    async move { crate::features::industry::resolve_default_facilities(db, manufacturing, reactions).await },
    |resolved| Message::Industry(industry_tab::Message::SelectionsResolved(resolved)),
  );

  Task::batch([tags, defaults])
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
    Message::CategorySelected(category) => {
      state.active = category;
      (Outcome::None, Task::none())
    }
    Message::Features(msg) => (
      features_tab::update(&mut state.features, msg, &mut state.settings),
      Task::none(),
    ),
    Message::Industry(msg) => (
      industry_tab::update(&mut state.industry, msg, &mut state.settings),
      Task::none(),
    ),
    Message::Storage(msg) => (
      storage_tab::update(&mut state.storage, msg, &mut state.settings),
      Task::none(),
    ),
    Message::Tags(msg) => {
      let (outcome, task) = tags_tab::update(&mut state.tags, msg);
      (outcome, task.map(Message::Tags))
    }
    Message::Ui(msg) => (ui_tab::update(&mut state.ui, msg, &mut state.settings), Task::none()),
    Message::ResetToDefaults => {
      let active = state.active;
      reset_active(state);
      // Resetting the scale must re-scale every open window, not just persist; only the
      // AccessibilityChanged outcome makes the app hoist the new scale factor live. The UI category
      // re-docks and reorders the rail live the same way via UiChanged.
      let outcome = match active {
        Category::Accessibility => Outcome::AccessibilityChanged,
        Category::Ui => Outcome::UiChanged,
        _ => Outcome::Persist,
      };
      (outcome, Task::none())
    }
  };
  if matches!(
    outcome,
    Outcome::AccessibilityChanged
      | Outcome::IndustryPin(_)
      | Outcome::Persist
      | Outcome::SetLogLevel(_)
      | Outcome::UiChanged
  ) {
    config::save(&state.settings);
  }
  (outcome, task)
}

fn reset_active(state: &mut State) {
  let defaults = Settings::default();
  match state.active {
    Category::Accessibility => *state.settings.accessibility_mut() = *defaults.accessibility(),
    Category::Features => *state.settings.features_mut() = *defaults.features(),
    Category::Industry if *state.settings.features().industry() => {
      *state.settings.industry_mut() = *defaults.industry();
      state.industry = industry_tab::State::from_settings(&state.settings);
    }
    Category::Industry => {}
    Category::Storage => *state.settings.storage_mut() = defaults.storage().clone(),
    Category::Ui => *state.settings.ui_mut() = defaults.ui().clone(),
    Category::Tags | Category::About => {}
  }
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
  let eyebrow = text("Pod \u{00b7} Preferences")
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(|_| text::Style {
      color: Some(color::text::secondary()),
    });
  let title = text("Settings")
    .font(typography::body::MEDIUM)
    .size(typography::size::LG)
    .style(|_| text::Style {
      color: Some(color::text::PRIMARY),
    });
  let identity = Column::with_children(vec![eyebrow.into(), title.into()]).spacing(spacing::UNIT);

  let reset = button(
    text("Reset to defaults")
      .font(typography::body::REGULAR)
      .size(typography::size::MD),
  )
  .padding(Padding {
    top: spacing::SPACE_2,
    right: spacing::SPACE_3_5,
    bottom: spacing::SPACE_2,
    left: spacing::SPACE_3_5,
  })
  .on_press(Message::ResetToDefaults)
  .style(|_, status| {
    let border_alpha = match status {
      button::Status::Hovered | button::Status::Pressed => 0.18,
      _ => 0.1,
    };
    button::Style {
      background: Some(Background::Color(iced::Color::TRANSPARENT)),
      text_color: color::text::secondary(),
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, border_alpha),
        width: 1.0,
        radius: radius::CONTROL.into(),
      },
      ..button::Style::default()
    }
  });

  header::header(vec![identity.into()], vec![reset.into()])
}

fn categories_pane(state: &State) -> Element<'_, Message> {
  let heading = container(
    text("Categories")
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

  // Push About to the bottom of the rail and fence it off from the working categories so it reads as
  // a separate, always-available surface rather than another preference group.
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
    color::accent::PLASMA
  } else {
    color::text::secondary()
  };

  let mut row_children: Vec<Element<'_, Message>> = vec![
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
      background: active.then(|| Background::Color(color::with_alpha(color::accent::PLASMA, 0.1))),
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
          background: Some(Background::Color(color::accent::PLASMA)),
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
    Category::Features => features_tab::badge(&state.settings),
    Category::Industry => industry_tab::badge(&state.settings),
    Category::Storage => storage_tab::badge(&state.settings),
    Category::Tags => tags_tab::badge(&state.tags),
    Category::Ui => ui_tab::badge(&state.settings),
  }
}

fn active_panel(state: &State) -> Element<'_, Message> {
  // A disabled Industry feature must never render its panel, even if it was the active category when
  // the feature was switched off; fall back to the default category in that case.
  let active = if state.active == Category::Industry && !*state.settings.features().industry() {
    Category::default()
  } else {
    state.active
  };

  match active {
    Category::About => about_tab::view().map(Message::About),
    Category::Accessibility => {
      accessibility_tab::view(&state.accessibility, &state.settings).map(Message::Accessibility)
    }
    Category::Features => features_tab::view(&state.features, &state.settings).map(Message::Features),
    Category::Industry => industry_tab::view(&state.industry, &state.settings).map(Message::Industry),
    Category::Storage => storage_tab::view(&state.storage, &state.settings).map(Message::Storage),
    Category::Tags => tags_tab::view(&state.tags, &state.settings).map(Message::Tags),
    Category::Ui => ui_tab::view(&state.ui, &state.settings).map(Message::Ui),
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
  async fn a_disabled_industry_panel_falls_back_to_the_default_category() {
    let mut state = state().await;
    state.settings.features_mut().set_industry(false);
    state.active = Category::Industry;

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
  async fn reset_on_industry_is_a_no_op_when_the_feature_is_disabled() {
    let mut state = state().await;
    state.settings.features_mut().set_industry(false);
    state.settings.industry_mut().set_manufacturing(Some(60_003_760));
    state.active = Category::Industry;

    let (outcome, _task) = update(&mut state, Message::ResetToDefaults);

    assert_eq!(outcome, Outcome::Persist);
    assert_eq!(
      *state.settings.industry().manufacturing(),
      Some(60_003_760),
      "a disabled Industry category must leave the industry defaults untouched"
    );
  }

  #[tokio::test]
  async fn reset_on_industry_restores_defaults_when_the_feature_is_enabled() {
    let mut state = state().await;
    state.settings.features_mut().set_industry(true);
    state.settings.industry_mut().set_manufacturing(Some(60_003_760));
    state.active = Category::Industry;

    let (outcome, _task) = update(&mut state, Message::ResetToDefaults);

    assert_eq!(outcome, Outcome::Persist);
    assert_eq!(*state.settings.industry().manufacturing(), None);
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
    state.settings.features_mut().set_wallet(false);
    assert!(!state.settings.features().wallet());

    let _task = update(&mut state, Message::ResetToDefaults);

    assert!(
      state.settings.features().wallet(),
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
  async fn the_industry_category_appears_when_the_feature_is_enabled() {
    let mut settings = Settings::default();
    settings.features_mut().set_industry(true);

    assert!(Category::list(&settings).contains(&Category::Industry));
  }

  #[tokio::test]
  async fn the_industry_category_is_hidden_when_the_feature_is_disabled() {
    let mut settings = Settings::default();
    settings.features_mut().set_industry(false);

    assert!(!Category::list(&settings).contains(&Category::Industry));
  }

  #[tokio::test]
  async fn view_renders_each_category() {
    let categories = Category::list(&Settings::default())
      .into_iter()
      .chain([Category::Industry, Category::About]);

    for category in categories {
      let mut state = state().await;
      state.settings.features_mut().set_industry(true);
      state.active = category;
      let _el: Element<'_, Message> = view(&state);
    }
  }
}
