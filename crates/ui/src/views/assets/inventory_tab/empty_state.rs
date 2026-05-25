//! Empty-state placeholder for the inventory tab.

use iced::{
  Element, Length, Theme,
  widget::{container, text},
};

use super::Message;
use crate::style::{color, typography::body};

/// Builder for the inventory tab empty-state placeholder.
pub struct EmptyState<'a> {
  msg: &'a str,
}

impl<'a> EmptyState<'a> {
  /// Creates a new empty-state component with the given message.
  pub fn new(msg: &'a str) -> Self {
    Self {
      msg,
    }
  }

  /// Renders the empty state into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    container(
      text(self.msg)
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
