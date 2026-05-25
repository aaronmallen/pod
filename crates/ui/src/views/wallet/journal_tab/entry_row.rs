//! Journal entry row — glyph badge + left text column + right delta/timestamp column.

use iced::{
  Border, Color, Element, Length, Padding, Theme,
  alignment::Horizontal,
  widget::{Space, column, container, row, text},
};

use super::Message;
use crate::{
  components::GlyphBadge,
  format,
  style::{
    color,
    typography::{body, mono},
  },
  views::wallet::{JournalEntry, journal_type_glyph, ts_label},
};

/// Builder for a single journal entry row.
pub struct Component<'a> {
  /// The journal entry to render.
  entry: &'a JournalEntry,
}

impl<'a> Component<'a> {
  /// Creates a new journal entry row builder.
  pub fn new(entry: &'a JournalEntry) -> Self {
    Self {
      entry,
    }
  }

  /// Renders the journal entry row into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    use crate::style::spacing;

    let entry = self.entry;
    let (glyph, is_in) = journal_type_glyph(&entry.entry_type);
    let delta_color = if is_in {
      color::status::ONLINE
    } else {
      color::status::DANGER
    };
    let delta_str = format!(
      "{}{}",
      if is_in { "+" } else { "−" },
      format::fmt_isk(entry.delta.abs())
    );

    let left_col = entry_left_col(&entry.reference, &entry.party);
    let right_col = entry_right_col(delta_str, delta_color, entry.ts_secs);

    let inner = row([
      GlyphBadge::new(glyph, is_in).render(),
      Space::new().width(12.0).into(),
      left_col,
      Space::new().width(spacing::SPACE_3).into(),
      right_col,
    ])
    .align_y(iced::alignment::Vertical::Center)
    .padding(Padding {
      top: 12.0,
      bottom: 12.0,
      left: spacing::SPACE_4,
      right: spacing::SPACE_4,
    });

    container(inner)
      .width(Length::Fill)
      .style(|_| container::Style {
        border: Border {
          color: color::border::SUBTLE,
          width: 1.0,
          ..Border::default()
        },
        ..container::Style::default()
      })
      .into()
  }
}

fn entry_left_col<'a>(reference: &'a str, party: &'a str) -> Element<'a, Message> {
  column([
    text(reference)
      .font(body::REGULAR)
      .size(13.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .width(Length::Fill)
      .into(),
    text(party)
      .font(mono::REGULAR)
      .size(10.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
  ])
  .width(Length::Fill)
  .into()
}

fn entry_right_col(delta_str: String, delta_color: Color, ts_secs: u64) -> Element<'static, Message> {
  column([
    container(
      text(delta_str)
        .font(mono::MEDIUM)
        .size(13.0)
        .style(move |_: &Theme| iced::widget::text::Style {
          color: Some(delta_color),
        }),
    )
    .width(Length::Fill)
    .align_x(Horizontal::Right)
    .into(),
    container(
      text(ts_label(ts_secs))
        .font(mono::REGULAR)
        .size(10.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::TERTIARY),
        }),
    )
    .width(Length::Fill)
    .align_x(Horizontal::Right)
    .into(),
  ])
  .width(96.0)
  .into()
}
