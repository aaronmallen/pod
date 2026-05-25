//! Loading placeholder for the plans tab.

use iced::{
  Element, Length, Padding,
  alignment::Horizontal,
  widget::{container, text},
};

use super::Message;
use crate::style::{color, spacing, typography::body};

pub struct Component;

impl Component {
  pub fn new() -> Self {
    Self
  }

  pub fn render(self) -> Element<'static, Message> {
    container(
      text("Loading plans\u{2026}")
        .font(body::REGULAR)
        .size(13.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        }),
    )
    .align_x(Horizontal::Center)
    .width(Length::Fill)
    .padding(Padding {
      top: 36.0,
      bottom: 36.0,
      left: spacing::SPACE_4,
      right: spacing::SPACE_4,
    })
    .into()
  }
}

impl Default for Component {
  fn default() -> Self {
    Self::new()
  }
}
