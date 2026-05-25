//! Horizontal rule divider used in the wallet right rail.

use iced::{
  Background, Element, Length,
  widget::{Space, container},
};

use crate::{style::color, views::wallet::Message};

/// Builder for a 1 px horizontal divider line.
pub struct Component;

impl Default for Component {
  fn default() -> Self {
    Self::new()
  }
}

impl Component {
  /// Creates a new divider component.
  pub fn new() -> Self {
    Self
  }

  /// Renders the divider into an iced element.
  pub fn render(self) -> Element<'static, Message> {
    container(Space::new().width(Length::Fill).height(1.0))
      .width(Length::Fill)
      .height(1.0)
      .style(|_| container::Style {
        background: Some(Background::Color(color::border::SUBTLE)),
        ..container::Style::default()
      })
      .into()
  }
}
