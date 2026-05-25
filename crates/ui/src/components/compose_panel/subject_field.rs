//! Subject input field row for the compose panel.

use iced::{Background, Border, Color, Element, widget::text_input};

use super::Message;
use crate::style::{color, typography as font};

/// Builder for the compose subject field row.
pub struct ComposeSubjectField<'a> {
  subject: &'a str,
}

impl<'a> ComposeSubjectField<'a> {
  /// Creates a new subject field builder.
  pub fn new(subject: &'a str) -> Self {
    Self {
      subject,
    }
  }

  /// Renders the subject field row.
  pub fn render(self) -> Element<'a, Message> {
    subject_field(self.subject)
  }
}

pub(super) fn subject_field(subject: &str) -> Element<'_, Message> {
  let input = text_input("—", subject)
    .on_input(Message::SubjectChanged)
    .size(15.0)
    .font(font::body::MEDIUM)
    .style(|_, _| text_input::Style {
      background: Background::Color(Color::TRANSPARENT),
      border: Border::default(),
      icon: color::text::SECONDARY,
      placeholder: color::text::TERTIARY,
      value: color::text::PRIMARY,
      selection: color::state::SELECTION,
    });
  super::compose_field_row("Subject", input.into())
}
