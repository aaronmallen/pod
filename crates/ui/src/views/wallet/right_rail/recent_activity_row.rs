//! Single recent-activity row in the wallet right rail.

use iced::{
  Element, Length, Padding, Theme,
  widget::{container, row, text},
};

use crate::{
  format,
  style::{
    color,
    typography::{body, mono},
  },
  views::wallet::{JournalEntry, Message, journal_type_glyph},
};

/// Builder for a single recent-activity journal row.
pub struct Component<'a> {
  entry: &'a JournalEntry,
}

impl<'a> Component<'a> {
  /// Creates a new recent activity row component.
  pub fn new(entry: &'a JournalEntry) -> Self {
    Self {
      entry,
    }
  }

  /// Renders the recent activity row into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let (_, is_in) = journal_type_glyph(&self.entry.entry_type);
    let delta_color = if is_in {
      color::status::ONLINE
    } else {
      color::status::DANGER
    };
    let delta_str = format!(
      "{}{}",
      if is_in { "+" } else { "−" },
      format::fmt_isk(self.entry.delta.abs())
    );
    container(
      row([
        text(&self.entry.reference)
          .font(body::REGULAR)
          .size(11.0)
          .style(|_: &Theme| iced::widget::text::Style {
            color: Some(color::text::SECONDARY),
          })
          .width(Length::Fill)
          .into(),
        text(delta_str)
          .font(mono::REGULAR)
          .size(10.0)
          .style(move |_: &Theme| iced::widget::text::Style {
            color: Some(delta_color),
          })
          .into(),
      ])
      .spacing(8.0)
      .align_y(iced::alignment::Vertical::Center),
    )
    .padding(Padding {
      top: 8.0,
      bottom: 8.0,
      left: 20.0,
      right: 20.0,
    })
    .width(Length::Fill)
    .into()
  }
}
