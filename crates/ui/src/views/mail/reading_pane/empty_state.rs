//! Message empty state: placeholder shown when no message is selected.

use iced::{
  Background, Element, Length, Theme,
  widget::{container, text},
};

use crate::{
  style::{color, typography::body},
  views::mail::reading_pane::Message,
};

/// Builder for the reading pane empty state.
pub struct Component;

impl Component {
  /// Creates a new empty state component.
  pub fn new() -> Self {
    Self
  }

  /// Renders the empty state.
  pub fn render<'a>(self) -> Element<'a, Message> {
    container(
      text("Select a message")
        .font(body::REGULAR)
        .size(14.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        }),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::BASE)),
      ..container::Style::default()
    })
    .into()
  }
}
