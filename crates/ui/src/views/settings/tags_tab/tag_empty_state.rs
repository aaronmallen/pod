//! Empty-state placeholder for the tag list body.

use iced::{
  Element, Length, Padding,
  alignment::Horizontal,
  widget::{container, scrollable, text},
};

use super::Message;
use crate::style::{color, typography};

/// Builder for the tag list empty state.
///
/// Renders a centred message inside a full-size scrollable container.
/// When no query is set the generic "No tags yet" message is shown; when
/// a query is provided the filtered "No tags match …" variant is shown.
pub struct Component<'a> {
  /// Optional search query that produced no results.
  query: Option<&'a str>,
}

impl<'a> Default for Component<'a> {
  fn default() -> Self {
    Self::new()
  }
}

impl<'a> Component<'a> {
  /// Create a new empty-state builder with no query set.
  pub fn new() -> Self {
    Self {
      query: None,
    }
  }

  /// Set the search query that produced no results.
  pub fn query(mut self, q: &'a str) -> Self {
    self.query = Some(q);
    self
  }

  /// Consume the builder and return the finished [`Element`].
  pub fn render(self) -> Element<'a, Message> {
    let message = match self.query {
      None => "No tags yet. Create one above.".to_string(),
      Some(q) => format!("No tags match \"{q}\"."),
    };

    scrollable(
      container(
        text(message)
          .font(typography::body::REGULAR)
          .size(13.0)
          .style(|_| iced::widget::text::Style {
            color: Some(color::text::SECONDARY),
          }),
      )
      .width(Length::Fill)
      .padding(Padding::new(80.0))
      .align_x(Horizontal::Center),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
  }
}
