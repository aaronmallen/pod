//! Stacked stat label for the inventory stats pill.

use iced::{
  Element, Theme,
  widget::{column, text},
};

use super::Message;
use crate::style::{color, typography::mono};

/// Builder for a stacked label/value stat display.
pub struct StatLabel {
  label: &'static str,
  value: String,
}

impl StatLabel {
  /// Creates a new stat label with the given label and value.
  pub fn new(label: &'static str, value: impl Into<String>) -> Self {
    Self {
      label,
      value: value.into(),
    }
  }

  /// Renders the stat label into an iced element.
  pub fn render(self) -> Element<'static, Message> {
    column([
      text(self.label)
        .font(mono::REGULAR)
        .size(8.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        })
        .into(),
      text(self.value)
        .font(mono::MEDIUM)
        .size(12.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::PRIMARY),
        })
        .into(),
    ])
    .spacing(1.0)
    .into()
  }
}
