//! Empty state shown when a plan has no entries.

use iced::{
  Background, Border, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Space, column, container, text},
};

use super::super::Message;
use crate::{
  components,
  style::{color, spacing, typography::body},
};

/// The empty-state card rendered when a plan contains no skill entries.
pub struct EmptyState;

impl EmptyState {
  /// Creates a new `EmptyState`.
  pub fn new() -> Self {
    Self
  }

  /// Renders the empty state into an [`Element`].
  pub fn render(self) -> Element<'static, Message> {
    let inner = empty_state_card();
    container(inner)
      .width(Length::Fill)
      .height(Length::Fill)
      .align_x(Horizontal::Center)
      .align_y(Vertical::Center)
      .padding(Padding {
        top: spacing::SPACE_4,
        bottom: spacing::SPACE_4,
        left: spacing::SPACE_7,
        right: spacing::SPACE_7,
      })
      .into()
  }
}

impl Default for EmptyState {
  fn default() -> Self {
    Self::new()
  }
}

fn empty_state_card<'a>() -> iced::widget::Container<'a, Message> {
  container(
    column([
      text("No skills in this plan yet")
        .font(body::MEDIUM)
        .size(16.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::PRIMARY),
        })
        .into(),
      Space::new().height(6.0).into(),
      text("Add your first skill using the skill picker on the left.")
        .font(body::REGULAR)
        .size(13.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        })
        .into(),
      Space::new().height(spacing::SPACE_4).into(),
      components::Button::ghost(text("Open skill picker").font(body::REGULAR).size(13.0))
        .on_press(Message::PickerToggled)
        .into(),
    ])
    .align_x(Horizontal::Center),
  )
  .padding(Padding {
    top: spacing::SPACE_8,
    bottom: spacing::SPACE_8,
    left: spacing::SPACE_7,
    right: spacing::SPACE_7,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::RAISED)),
    border: Border {
      color: color::border::SUBTLE,
      radius: 10.0.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .width(Length::Fill)
}
