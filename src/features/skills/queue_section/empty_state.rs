use iced::{
  Background, Border, Element, Length, Padding,
  alignment::Horizontal,
  widget::{Column, container, text},
};

use super::super::Message;
use crate::ui::{
  components::eyebrow::eyebrow,
  style::{color, radius, spacing, typography},
};

const QUEUE_SIDE_MARGIN: f32 = 28.0;

pub(super) fn empty_state<'a>() -> Element<'a, Message> {
  let body = Column::with_children(vec![
    eyebrow("Queue \u{b7} 0 skills", Some(color::text::SECONDARY)),
    text("Empty queue")
      .font(typography::body::MEDIUM)
      .size(typography::size::LG)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    text("Apply a skill plan to start training.")
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_x(Horizontal::Center)
  .width(Length::Fill);

  let card = container(body)
    .width(Length::Fill)
    .align_x(Horizontal::Center)
    .padding(Padding {
      top: spacing::SPACE_6,
      right: QUEUE_SIDE_MARGIN,
      bottom: spacing::SPACE_6,
      left: QUEUE_SIDE_MARGIN,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, 0.10),
        width: 1.0,
        radius: radius::CARD.into(),
      },
      ..container::Style::default()
    });

  container(card)
    .width(Length::Fill)
    .padding(Padding {
      top: 0.0,
      right: QUEUE_SIDE_MARGIN,
      bottom: QUEUE_SIDE_MARGIN,
      left: QUEUE_SIDE_MARGIN,
    })
    .into()
}
