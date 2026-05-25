//! ISK price cell for a contracts table row.

use iced::{
  Element, Length, Padding, Theme,
  alignment::{Horizontal, Vertical},
  widget::{container, text},
};

use super::Message;
use crate::{
  format,
  style::{color, spacing, typography::mono},
};

const ROW_PAD_H: f32 = spacing::SPACE_4;
const COL_PRICE: f32 = 96.0;

/// Builder for the ISK price cell.
pub struct Component {
  price: f64,
}

impl Component {
  /// Creates a new price cell component.
  pub fn new(price: f64) -> Self {
    Self {
      price,
    }
  }

  /// Renders the price cell into an iced element.
  pub fn render(self) -> Element<'static, Message> {
    container(
      text(format::fmt_isk(self.price))
        .font(mono::MEDIUM)
        .size(13.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::PRIMARY),
        }),
    )
    .width(COL_PRICE)
    .height(Length::Fill)
    .align_x(Horizontal::Right)
    .align_y(Vertical::Center)
    .padding(Padding {
      top: 0.0,
      bottom: 0.0,
      left: ROW_PAD_H,
      right: ROW_PAD_H,
    })
    .into()
  }
}
