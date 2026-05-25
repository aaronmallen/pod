//! Relative timestamp cell for a contracts table row.

use iced::{
  Element, Length, Padding, Theme,
  alignment::{Horizontal, Vertical},
  widget::{container, text},
};

use super::Message;
use crate::{
  style::{color, spacing, typography::mono},
  views::wallet::ts_label,
};

const ROW_PAD_H: f32 = spacing::SPACE_4;
const COL_WHEN: f32 = 84.0;

/// Builder for the relative timestamp cell.
pub struct Component {
  ts_secs: u64,
}

impl Component {
  /// Creates a new when cell component.
  pub fn new(ts_secs: u64) -> Self {
    Self {
      ts_secs,
    }
  }

  /// Renders the when cell into an iced element.
  pub fn render(self) -> Element<'static, Message> {
    container(
      text(ts_label(self.ts_secs))
        .font(mono::REGULAR)
        .size(11.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        }),
    )
    .width(COL_WHEN)
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
