//! Loading / empty state panel for the values tab.

use iced::{
  Element, Length,
  widget::{container, text},
};

use super::Message;
use crate::style::{color, typography::mono};

/// Builder for the loading / empty state panel.
pub struct Component;

impl Default for Component {
  fn default() -> Self {
    Self::new()
  }
}

impl Component {
  /// Creates a new empty panel builder.
  pub fn new() -> Self {
    Self
  }

  /// Renders the empty panel into an iced element.
  pub fn render(self) -> Element<'static, Message> {
    container(
      text("Loading asset values\u{2026}")
        .font(mono::REGULAR)
        .size(12.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        }),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .center(Length::Fill)
    .into()
  }
}
