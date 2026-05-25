//! Stockpile empty state: placeholder shown when no stockpiles exist.

use iced::{
  Element, Length, Theme,
  widget::{container, text},
};

use crate::{
  style::{color, typography::body},
  views::assets::stockpiles_tab::Message,
};

/// Builder for the stockpile empty state.
pub struct Component;

impl Default for Component {
  fn default() -> Self {
    Self::new()
  }
}

impl Component {
  /// Creates a new stockpile empty state.
  pub fn new() -> Self {
    Self
  }

  /// Renders the empty state into an iced element.
  pub fn render(self) -> Element<'static, Message> {
    container(
      text("No stockpiles yet. Create one with the button above.")
        .font(body::REGULAR)
        .size(13.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        }),
    )
    .padding(48.0)
    .width(Length::Fill)
    .center_x(Length::Fill)
    .height(Length::Fill)
    .center_y(Length::Fill)
    .into()
  }
}
