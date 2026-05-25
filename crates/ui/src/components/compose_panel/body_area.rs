//! Body text editor area for the compose panel.

use iced::{
  Background, Border, Color, Element, Length, Padding,
  widget::{container, text_editor},
};

use super::Message;
use crate::style::{color, typography as font};

/// Builder for the compose body editor area.
pub struct ComposeBodyArea<'a> {
  body: &'a text_editor::Content,
}

impl<'a> ComposeBodyArea<'a> {
  /// Creates a new body area builder.
  pub fn new(body: &'a text_editor::Content) -> Self {
    Self {
      body,
    }
  }

  /// Renders the body editor area.
  pub fn render(self) -> Element<'a, Message> {
    body_area(self.body)
  }
}

pub(super) fn body_area(body: &text_editor::Content) -> Element<'_, Message> {
  container(
    text_editor(body)
      .on_action(Message::BodyAction)
      .height(Length::Fill)
      .size(14.0)
      .font(font::body::REGULAR)
      .padding(Padding::ZERO)
      .style(|_, _| text_editor::Style {
        background: Background::Color(Color::TRANSPARENT),
        border: Border::default(),
        placeholder: color::text::TERTIARY,
        value: color::text::PRIMARY,
        selection: color::state::SELECTION,
      }),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .padding(Padding {
    top: 16.0,
    bottom: 16.0,
    left: 16.0,
    right: 16.0,
  })
  .into()
}
