//! Cc recipient field row for the compose panel.

use iced::{
  Background, Border, Color, Element,
  widget::{row, text_input},
};

use super::{Component, Message, to_field::recipient_chip};
use crate::style::{color, typography as font};

/// Builder for the compose Cc field row.
pub struct ComposeCcField<'a> {
  panel: &'a Component,
}

impl<'a> ComposeCcField<'a> {
  /// Creates a new Cc field builder.
  pub fn new(panel: &'a Component) -> Self {
    Self {
      panel,
    }
  }

  /// Renders the Cc field row.
  pub fn render(self) -> Element<'a, Message> {
    cc_field(self.panel)
  }
}

pub(super) fn cc_field(panel: &Component) -> Element<'_, Message> {
  let cc_chips: Vec<Element<'_, Message>> = panel
    .cc
    .iter()
    .enumerate()
    .map(|(i, r)| recipient_chip(r.name.as_str(), Message::CcRemove(i)))
    .collect();

  let cc_input = text_input(
    if panel.cc.is_empty() { "Add Cc recipient…" } else { "" },
    &panel.cc_search,
  )
  .on_input(Message::CcSearchChanged)
  .on_submit(Message::CcAdd)
  .size(13.0)
  .font(font::body::REGULAR)
  .style(|_, _| text_input::Style {
    background: Background::Color(Color::TRANSPARENT),
    border: Border::default(),
    icon: color::text::SECONDARY,
    placeholder: color::text::TERTIARY,
    value: color::text::PRIMARY,
    selection: color::state::SELECTION,
  });

  let mut row_children: Vec<Element<'_, Message>> = cc_chips;
  row_children.push(cc_input.into());

  super::compose_field_row(
    "Cc",
    row(row_children)
      .spacing(6.0)
      .align_y(iced::alignment::Vertical::Center)
      .into(),
  )
}
