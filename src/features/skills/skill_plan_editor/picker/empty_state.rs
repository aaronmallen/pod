use iced::{
  Element, Length,
  alignment::{Horizontal, Vertical},
  widget::{container, text},
};

use super::super::Message;
use crate::ui::style::{color, spacing, typography};

pub(in crate::features::skills::skill_plan_editor) fn empty_state<'a>(message: String) -> Element<'a, Message> {
  container(
    text(message)
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      }),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .align_x(Horizontal::Center)
  .align_y(Vertical::Center)
  .padding(spacing::SPACE_6)
  .into()
}
