//! Date group separator label.

use iced::{
  Background, Element, Length, Padding, Theme,
  widget::{container, text},
};

use super::Message;
use crate::style::{color, typography::mono};

/// Builder for a day/date group separator in the message list.
pub struct Component {
  day: String,
}

impl Component {
  /// Create a new day header with the given day label.
  pub fn new(day: impl Into<String>) -> Self {
    Self {
      day: day.into(),
    }
  }

  /// Render into an iced element.
  pub fn render(self) -> Element<'static, Message> {
    container(
      text(self.day.to_uppercase())
        .font(mono::REGULAR)
        .size(9.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        }),
    )
    .padding(Padding {
      top: 14.0,
      bottom: 6.0,
      left: 16.0,
      right: 16.0,
    })
    .width(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::BASE)),
      ..container::Style::default()
    })
    .into()
  }
}
