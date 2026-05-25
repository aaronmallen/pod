//! Message list empty state: placeholder when a search yields no results.

use iced::{
  Element, Length, Theme,
  widget::{container, text},
};

use crate::{
  style::{color, typography::body},
  views::mail::message_list_pane::Message,
};

/// Builder for the message list empty state.
pub struct Component<'a> {
  query: &'a str,
}

impl<'a> Component<'a> {
  /// Creates a new empty state for the given search query.
  pub fn new(query: &'a str) -> Self {
    Self {
      query,
    }
  }

  /// Renders the empty state.
  pub fn render(self) -> Element<'a, Message> {
    container(
      text(format!("No messages match \"{}\".", self.query))
        .font(body::REGULAR)
        .size(13.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        }),
    )
    .padding(32.0)
    .width(Length::Fill)
    .center_x(Length::Fill)
    .into()
  }
}
