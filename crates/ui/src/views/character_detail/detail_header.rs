//! Detail header component: character picker with stat chips for SP, ISK, and location.

use iced::{
  Background, Border, Element, Length, Padding, Theme,
  widget::{Space, column, container, row, text},
};

use crate::{
  style::{color, spacing, typography as font},
  views::character_detail::{Message, State},
};

/// Builder for the character detail header row.
pub struct Component<'a> {
  state: &'a State,
}

impl<'a> Component<'a> {
  /// Creates a new detail header component for the given state.
  pub fn new(state: &'a State) -> Self {
    Self {
      state,
    }
  }

  /// Renders the detail header.
  pub fn render(self) -> Element<'a, Message> {
    let state = self.state;
    let picker = state.picker.render().map(Message::Picker);
    let sp_stat = head_stat("Total SP", &format_sp(state));
    let mut row_items: Vec<Element<'_, Message>> = vec![picker, header_divider().into(), sp_stat];
    append_optional_stats(state, &mut row_items);
    row_items.push(Space::new().width(Length::Fill).into());
    header_container(row_items)
  }
}

fn format_sp(state: &State) -> String {
  let total_sp: i64 = state.character.skills().iter().map(|s| s.skillpoints).sum();
  if total_sp >= 1_000_000 {
    format!("{:.1}M", total_sp as f64 / 1_000_000.0)
  } else if total_sp > 0 {
    format!("{:.0}K", total_sp as f64 / 1_000.0)
  } else {
    "\u{2014}".to_string()
  }
}

fn append_optional_stats<'a>(state: &'a State, items: &mut Vec<Element<'a, Message>>) {
  if state.feat_wallet {
    let isk = format!("{} ISK", state.character.isk_formatted());
    items.push(header_divider().into());
    items.push(head_stat("Liquid", &isk));
  }
  if state.feat_location_tracking {
    let loc = state
      .character
      .location_name()
      .clone()
      .unwrap_or_else(|| "\u{2014}".to_string());
    items.push(header_divider().into());
    items.push(head_stat("Location", &loc));
  }
}

fn head_stat(label: &str, value: &str) -> Element<'static, Message> {
  column([
    text(label.to_uppercase())
      .font(font::mono::REGULAR)
      .size(9.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
    Space::new().height(4.0).into(),
    text(value.to_string())
      .font(font::mono::MEDIUM)
      .size(15.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
  ])
  .into()
}

fn header_divider() -> impl Into<Element<'static, Message>> {
  container(Space::new().width(1.0).height(44.0)).style(|_| container::Style {
    background: Some(Background::Color(color::border::SUBTLE)),
    ..container::Style::default()
  })
}

fn header_container(items: Vec<Element<'_, Message>>) -> Element<'_, Message> {
  container(
    row(items)
      .align_y(iced::alignment::Vertical::Center)
      .spacing(spacing::SPACE_4)
      .padding(Padding {
        top: 0.0,
        bottom: 0.0,
        left: spacing::SPACE_7,
        right: spacing::SPACE_7,
      }),
  )
  .width(Length::Fill)
  .center_y(spacing::layout::HEADER_HEIGHT)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::BASE)),
    border: Border {
      color: color::border::SUBTLE,
      width: 1.0,
      ..Border::default()
    },
    ..container::Style::default()
  })
  .into()
}
