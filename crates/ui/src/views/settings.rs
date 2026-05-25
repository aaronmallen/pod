//! Settings view: feature-flag toggles and tag management.

pub mod features_tab;
pub mod tags_tab;

pub use tags_tab::TagSortMode;

mod sidebar;

use iced::{
  Background, Element, Length, Padding,
  alignment::Vertical,
  widget::{Space, column, container, row, text},
};

use crate::style::{color, spacing};

/// Which settings category is currently shown.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum Category {
  #[default]
  Features,
  Tags,
}

/// Builder for the settings view.
pub struct Component<'a> {
  state: &'a State,
}

impl<'a> Component<'a> {
  /// Create a new settings view builder for the given state.
  pub fn new(state: &'a State) -> Self {
    Self {
      state,
    }
  }

  /// Consume the builder and return the finished [`Element`].
  pub fn render(self) -> Element<'a, Message> {
    let state = self.state;
    let header = render_header();
    let categories = sidebar::render_categories_pane(state);
    let panel = match &state.active_category {
      Category::Features => features_tab::Component::new(&state.features)
        .render()
        .map(Message::FeaturesTab),
      Category::Tags => tags_tab::Component::new(&state.tags).render().map(Message::TagsTab),
    };
    let body: Element<'_, Message> = row([categories, panel]).width(Length::Fill).height(Length::Fill).into();
    container(column([header, body]).width(Length::Fill).height(Length::Fill))
      .width(Length::Fill)
      .height(Length::Fill)
      .style(|_| container::Style {
        background: Some(Background::Color(color::surface::BASE)),
        ..container::Style::default()
      })
      .into()
  }
}

/// Messages produced by the settings view.
#[derive(Clone, Debug)]
pub enum Message {
  /// A settings category was selected in the sidebar.
  CategorySelected(Category),
  /// A message from the features tab panel.
  FeaturesTab(features_tab::Message),
  /// All settings were reset to their defaults.
  ResetDefaults,
  /// A message from the tags tab panel.
  TagsTab(tags_tab::Message),
}

/// Runtime state for the settings view.
#[derive(Default)]
pub struct State {
  /// The currently active settings category.
  pub active_category: Category,
  /// State for the features tab panel.
  pub features: features_tab::State,
  /// State for the tags tab panel.
  pub tags: tags_tab::State,
}

fn render_header() -> Element<'static, Message> {
  let eyebrow = text("Pod · Preferences").size(9.0).color(color::text::SECONDARY);
  let title = text("Settings").size(22.0).color(color::text::PRIMARY);
  let left_col: Element<'_, Message> = column([eyebrow.into(), Space::new().height(6.0).into(), title.into()]).into();

  let reset_icon = crate::components::Icon::settings()
    .size(14.0)
    .color(color::text::SECONDARY)
    .render::<Message>();

  let reset_btn = crate::components::Button::ghost(
    row([
      reset_icon,
      text("Reset to defaults")
        .size(13.0)
        .color(color::text::SECONDARY)
        .into(),
    ])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center),
  )
  .on_press(Message::ResetDefaults);

  let header_row: Element<'_, Message> = row([left_col, Space::new().width(Length::Fill).into(), reset_btn.into()])
    .align_y(Vertical::Center)
    .padding(Padding {
      top: 20.0,
      bottom: 20.0,
      left: spacing::SPACE_7,
      right: spacing::SPACE_7,
    })
    .into();

  let border = container(Space::new().width(Length::Fill).height(1.0))
    .width(Length::Fill)
    .height(1.0)
    .style(|_| container::Style {
      background: Some(Background::Color(color::border::SUBTLE)),
      ..container::Style::default()
    });

  column([header_row, border.into()])
    .height(spacing::layout::HEADER_HEIGHT)
    .into()
}
