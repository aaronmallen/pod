//! 1 × 32 px vertical rule matching the header's subtle border colour.

use iced::{
  Background, Element,
  widget::{Space, container},
};

use crate::{style::color, views::wallet::Message};

/// 1 × 32 px vertical rule used between stats cells in the wallet header.
pub struct VerticalSeparator;

impl Default for VerticalSeparator {
  fn default() -> Self {
    Self::new()
  }
}

impl VerticalSeparator {
  /// Creates a new vertical separator.
  pub fn new() -> Self {
    Self
  }

  /// Renders the separator into an iced element.
  pub fn render(self) -> Element<'static, Message> {
    container(Space::new().width(1.0).height(32.0))
      .width(1.0)
      .height(32.0)
      .style(|_| container::Style {
        background: Some(Background::Color(color::border::SUBTLE)),
        ..container::Style::default()
      })
      .into()
  }
}
