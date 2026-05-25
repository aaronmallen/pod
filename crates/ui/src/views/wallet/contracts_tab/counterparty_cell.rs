//! Counterparty name cell for a contracts table row.

use iced::{
  Element, Length, Padding, Theme,
  alignment::Vertical,
  widget::{container, text},
};

use super::Message;
use crate::style::{color, spacing, typography::body};

const ROW_PAD_H: f32 = spacing::SPACE_4;
const COL_COUNTERPARTY: f32 = 136.0;

/// Builder for the counterparty name cell.
pub struct Component<'a> {
  cp: &'a str,
}

impl<'a> Component<'a> {
  /// Creates a new counterparty cell component.
  pub fn new(cp: &'a str) -> Self {
    Self {
      cp,
    }
  }

  /// Renders the counterparty cell into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    container(
      text(self.cp)
        .font(body::REGULAR)
        .size(12.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::STRONG),
        }),
    )
    .width(COL_COUNTERPARTY)
    .height(Length::Fill)
    .align_y(Vertical::Center)
    .clip(true)
    .padding(Padding {
      top: 0.0,
      bottom: 0.0,
      left: ROW_PAD_H,
      right: ROW_PAD_H,
    })
    .into()
  }
}
