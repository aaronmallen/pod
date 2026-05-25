//! Loading placeholder shown while a picker tab's data is being fetched.

use iced::{
  Element, Length, Padding,
  widget::{container, text},
};

use super::super::Message;
use crate::style::{color, typography::body};

/// Builder for a full-height loading placeholder.
pub struct Component {
  label: &'static str,
}

impl Component {
  /// Creates a new loading placeholder with the given label.
  pub fn new(label: &'static str) -> Self {
    Self {
      label,
    }
  }

  /// Renders the placeholder into an iced element.
  pub fn render<'a>(self) -> Element<'a, Message> {
    container(
      text(self.label)
        .font(body::REGULAR)
        .size(13.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        }),
    )
    .padding(Padding::from([24.0, 16.0]))
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
  }
}
