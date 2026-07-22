use std::borrow::Cow;

use iced::{
  Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, container, text},
};

use super::super::Message;
use crate::ui::{
  components::rule,
  style::{color, spacing, typography},
};

const COMPLETES_WIDTH: f32 = 135.0;
const DURATION_WIDTH: f32 = 110.0;
const SP_WIDTH: f32 = 80.0;

pub(super) fn col_header<'a>() -> Element<'a, Message> {
  let labels = Row::with_children(vec![
    container(label(t!("skills.queue.col_completes")))
      .width(Length::Fixed(COMPLETES_WIDTH))
      .into(),
    container(label(t!("skills.queue.col_skill")))
      .width(Length::Fill)
      .into(),
    container(label(t!("skills.queue.col_sp")))
      .width(Length::Fixed(SP_WIDTH))
      .align_x(Horizontal::Right)
      .into(),
    container(label(t!("skills.queue.col_duration")))
      .width(Length::Fixed(DURATION_WIDTH))
      .align_x(Horizontal::Right)
      .into(),
  ])
  .spacing(spacing::SPACE_3)
  .align_y(Vertical::Center);

  let header = container(labels).width(Length::Fill).padding(Padding {
    top: spacing::SPACE_3,
    right: spacing::SPACE_3,
    bottom: spacing::SPACE_3,
    left: spacing::SPACE_3,
  });

  Column::with_children(vec![header.into(), rule::horizontal()])
    .width(Length::Fill)
    .into()
}

fn label<'a>(text_value: Cow<'a, str>) -> Element<'a, Message> {
  text(text_value)
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(|_| text::Style {
      color: Some(color::text::secondary()),
    })
    .into()
}
