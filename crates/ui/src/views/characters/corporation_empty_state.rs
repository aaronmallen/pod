//! Empty-state view for the Corporations tab.

use iced::{
  Element, Length,
  alignment::Horizontal,
  widget::{column, container, text},
};

use crate::style::{color, spacing, typography};

/// Builder for the corporation empty-state element.
///
/// Use [`CorporationEmptyState::new`] for the unfiltered form and
/// call [`.filtered`](CorporationEmptyState::filtered) to show the
/// "no results" variant.
pub struct CorporationEmptyState<'a> {
  /// Optional search query; when `Some`, the filtered variant is rendered.
  query: Option<&'a str>,
}

impl<'a> Default for CorporationEmptyState<'a> {
  fn default() -> Self {
    Self::new()
  }
}

impl<'a> CorporationEmptyState<'a> {
  /// Creates a new unfiltered corporation empty-state builder.
  pub fn new() -> Self {
    Self {
      query: None,
    }
  }

  /// Switches to the filtered ("no results") variant with the given query string.
  pub fn filtered(mut self, query: &'a str) -> Self {
    self.query = Some(query);
    self
  }

  /// Renders the corporation empty state into an iced element.
  pub fn render<MSG: 'static>(self) -> Element<'a, MSG> {
    match self.query {
      None => container(
        text("Add your first corporation to get started")
          .font(typography::body::REGULAR)
          .size(15.0)
          .style(|_| text::Style {
            color: Some(color::text::SECONDARY),
          }),
      )
      .width(Length::Fill)
      .height(Length::Fill)
      .center_x(Length::Fill)
      .center_y(Length::Fill)
      .into(),

      Some(q) => container(
        column([
          text("No results")
            .font(typography::body::MEDIUM)
            .size(15.0)
            .style(|_| text::Style {
              color: Some(color::text::PRIMARY),
            })
            .into(),
          text(format!("No corporations match \"{q}\""))
            .font(typography::body::REGULAR)
            .size(13.0)
            .style(|_| text::Style {
              color: Some(color::text::SECONDARY),
            })
            .into(),
        ])
        .spacing(spacing::SPACE_1)
        .align_x(Horizontal::Center),
      )
      .height(Length::Fill)
      .width(Length::Fill)
      .center_x(Length::Fill)
      .center_y(Length::Fill)
      .into(),
    }
  }
}

/// Re-export for the parent module.
pub use CorporationEmptyState as Component;
