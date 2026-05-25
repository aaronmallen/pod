//! Route / location cell for a contracts table row.

use iced::{
  Element, Length, Padding, Theme,
  alignment::Vertical,
  widget::{container, text},
};

use super::Message;
use crate::style::{color, spacing, typography::mono};

const ROW_PAD_H: f32 = spacing::SPACE_4;
const COL_LOCATION: f32 = 148.0;

/// Builder for the route / location cell.
pub struct Component<'a> {
  loc: &'a str,
}

impl<'a> Component<'a> {
  /// Creates a new location cell component.
  pub fn new(loc: &'a str) -> Self {
    Self {
      loc,
    }
  }

  /// Renders the location cell into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    container(
      text(self.loc)
        .font(mono::REGULAR)
        .size(11.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        }),
    )
    .width(COL_LOCATION)
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
