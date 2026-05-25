//! Collateral ISK cell for a contracts table row.

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
const COL_COLLATERAL: f32 = 96.0;

/// Builder for the collateral ISK cell.
pub struct Component {
  collateral: f64,
}

impl Component {
  /// Creates a new collateral cell component.
  pub fn new(collateral: f64) -> Self {
    Self {
      collateral,
    }
  }

  /// Renders the collateral cell into an iced element.
  pub fn render(self) -> Element<'static, Message> {
    let (label, c) = if self.collateral > 0.0 {
      (format::fmt_isk(self.collateral), color::text::WARNING)
    } else {
      ("\u{2014}".to_string(), color::text::TERTIARY)
    };
    container(
      text(label)
        .font(mono::REGULAR)
        .size(11.0)
        .style(move |_: &Theme| iced::widget::text::Style {
          color: Some(c),
        }),
    )
    .width(COL_COLLATERAL)
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
