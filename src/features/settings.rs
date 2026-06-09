pub mod features_tab;
pub mod storage_tab;
pub mod tags_tab;

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
  #[default]
  Features,
  Storage,
  Tags,
}

impl Category {
  const ALL: [Category; 3] = [Category::Features, Category::Storage, Category::Tags];

  fn label(self) -> &'static str {
    match self {
      Category::Features => "Features",
      Category::Storage => "Storage",
      Category::Tags => "Tags",
    }
  }
}

#[derive(Clone, Debug)]
pub enum Message {
  CategorySelected(Category),
  Features(features_tab::Message),
  ResetToDefaults,
  Storage(storage_tab::Message),
  Tags(tags_tab::Message),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Outcome {
  None,
  Persist,
  ReleaseLock,
  SyncNow,
}

#[derive(Debug)]
pub struct State {
  active: Category,
  db: Database,
  features: features_tab::State,
  settings: Settings,
  storage: storage_tab::State,
  tags: tags_tab::State,
}

impl State {
  pub fn new(settings: Settings, db: Database) -> Self {
    let features = features_tab::State::from_settings(&settings);
    let storage = storage_tab::State::from_settings(&settings);
    let tags = tags_tab::State::new(db.clone());
    State {
      active: Category::default(),
      db,
      features,
      settings,
      storage,
      tags,
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
  tags_tab::load(&state.db).map(Message::Tags)
}

pub fn subscription(state: &State) -> iced::Subscription<Message> {
  tags_tab::subscription(&state.tags).map(Message::Tags)
}

pub fn update(state: &mut State, message: Message) -> (Outcome, Task<Message>) {
  let (outcome, task) = match message {
    Message::CategorySelected(category) => {
      state.active = category;
      (Outcome::None, Task::none())
    }
    Message::Features(msg) => (
      features_tab::update(&mut state.features, msg, &mut state.settings),
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
    Message::ResetToDefaults => {
      reset_active(state);
      (Outcome::Persist, Task::none())
    }
  };
  if outcome == Outcome::Persist {
    config::save(&state.settings);
  }
  (outcome, task)
}

fn reset_active(state: &mut State) {
  let defaults = Settings::default();
  match state.active {
    Category::Features => *state.settings.features_mut() = *defaults.features(),
    Category::Storage => *state.settings.storage_mut() = defaults.storage().clone(),
    Category::Tags => {}
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
      color: Some(color::text::SECONDARY),
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
      text_color: color::text::SECONDARY,
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
        color: Some(color::text::SECONDARY),
      }),
  )
  .padding(Padding {
    top: 0.0,
    right: spacing::SPACE_2,
    bottom: spacing::SPACE_2_5,
    left: spacing::SPACE_2,
  });

  let mut rows: Vec<Element<'_, Message>> = vec![heading.into()];
  for category in Category::ALL {
    rows.push(category_row(state, category));
  }

  let column = Column::with_children(rows).width(Length::Fill).padding(Padding {
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

fn category_row(state: &State, category: Category) -> Element<'_, Message> {
  let active = state.active == category;
  let label_color = if active {
    color::text::PRIMARY
  } else {
    color::text::SECONDARY
  };
  let badge_color = if active {
    color::accent::PLASMA
  } else {
    color::text::SECONDARY
  };

  let row = Row::with_children(vec![
    text(category.label())
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .width(Length::Fill)
      .style(move |_| text::Style {
        color: Some(label_color),
      })
      .into(),
    text(badge_for(state, category))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(move |_| text::Style {
        color: Some(badge_color),
      })
      .into(),
  ])
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
    Category::Features => features_tab::badge(&state.settings),
    Category::Storage => storage_tab::badge(&state.settings),
    Category::Tags => tags_tab::badge(&state.tags),
  }
}

fn active_panel(state: &State) -> Element<'_, Message> {
  match state.active {
    Category::Features => features_tab::view(&state.features, &state.settings).map(Message::Features),
    Category::Storage => storage_tab::view(&state.storage, &state.settings).map(Message::Storage),
    Category::Tags => tags_tab::view(&state.tags, &state.settings).map(Message::Tags),
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
  async fn it_defaults_to_the_features_category() {
    assert_eq!(state().await.active, Category::Features);
  }

  #[tokio::test]
  async fn category_selected_switches_the_active_category() {
    let mut state = state().await;

    let _task = update(&mut state, Message::CategorySelected(Category::Storage));

    assert_eq!(state.active, Category::Storage);
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
  async fn view_renders_each_category() {
    for category in Category::ALL {
      let mut state = state().await;
      state.active = category;
      let _el: Element<'_, Message> = view(&state);
    }
  }
}
