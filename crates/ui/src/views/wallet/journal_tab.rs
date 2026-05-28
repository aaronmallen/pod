//! Journal entry table for the wallet main panel.

use iced::{
  Border, Element, Length, Padding, Theme,
  alignment::Horizontal,
  widget::{Space, column, container, row, text},
};

use crate::{
  components::{DataTable, GlyphBadge},
  format,
  style::{
    color,
    typography::{body, mono},
  },
  views::wallet::{JournalEntry, SignFilter, State, journal_type_glyph, ts_label},
};

/// Messages produced by the journal tab.
#[derive(Clone, Debug)]
pub enum Message {
  ScrollUpdate(f32),
  SignFilterChanged(SignFilter),
}

fn entry_row(entry: &JournalEntry) -> Element<'_, Message> {
  use crate::style::spacing;

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

  let left_col: Element<'_, Message> = column([
    text(&entry.reference)
      .font(body::REGULAR)
      .size(13.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .width(Length::Fill)
      .into(),
    text(&entry.party)
      .font(mono::REGULAR)
      .size(10.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
  ])
  .width(Length::Fill)
  .into();

  let right_col: Element<'_, Message> = column([
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
      text(ts_label(entry.ts_secs))
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
  .into();

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
    left: spacing::SPACE_7,
    right: spacing::SPACE_7,
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

/// Builder for the journal table.
pub struct Component<'a> {
  state: &'a State,
}

impl<'a> Component<'a> {
  /// Creates a new journal table component.
  pub fn new(state: &'a State) -> Self {
    Self {
      state,
    }
  }

  /// Renders the journal table into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    use iced::widget::scrollable;

    let visible: Vec<_> = self.state.filtered_journal.iter()
      .take(self.state.visible_journal_count)
      .collect();

    scrollable(
      DataTable::new(visible.into_iter(), |entry, _, _| entry_row(entry))
        .empty_message("No journal entries match your filter.")
        .render()
    )
    .on_scroll(|vp| Message::ScrollUpdate(vp.relative_offset().y))
    .into()
  }
}
