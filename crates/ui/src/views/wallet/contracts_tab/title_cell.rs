//! Contract title cell for a contracts table row.

use iced::{
  Element, Length, Padding, Theme,
  alignment::Vertical,
  widget::{container, text},
};

use super::Message;
use crate::style::{color, spacing, typography::body};

const ROW_PAD_H: f32 = spacing::SPACE_4;

/// Builder for the contract title cell.
pub struct Component<'a> {
  title: &'a str,
}

impl<'a> Component<'a> {
  /// Creates a new title cell component.
  pub fn new(title: &'a str) -> Self {
    Self {
      title,
    }
  }

  /// Renders the title cell into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    container(
      text(self.title)
        .font(body::REGULAR)
        .size(13.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::PRIMARY),
        }),
    )
    .width(Length::Fill)
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
