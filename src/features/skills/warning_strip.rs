use chrono::{DateTime, Utc};
use iced::{
  Background, Border, Element, Length, Padding,
  alignment::Vertical,
  widget::{Column, Row, container, text},
};

use super::{
  Message,
  queue::{ComputedQueue, QueueWarning, queue_status, queue_warnings},
};
use crate::{
  store::model::CharacterSkillqueue,
  ui::style::{color, radius, spacing, typography},
};

const QUEUE_SIDE_MARGIN: f32 = 28.0;

pub fn warning_strip<'a>(
  computed: &ComputedQueue,
  head: Option<&CharacterSkillqueue>,
  queued_count: usize,
  now: DateTime<Utc>,
) -> Option<Element<'a, Message>> {
  let warnings = queue_warnings(computed, queue_status(head, queued_count, now));
  if warnings.is_empty() {
    return None;
  }

  let banners = warnings.into_iter().map(banner);

  Some(
    container(
      Column::with_children(banners)
        .spacing(spacing::SPACE_2)
        .width(Length::Fill),
    )
    .width(Length::Fill)
    .padding(Padding {
      top: 0.0,
      right: QUEUE_SIDE_MARGIN,
      bottom: 0.0,
      left: QUEUE_SIDE_MARGIN,
    })
    .into(),
  )
}

fn banner<'a>(warning: QueueWarning) -> Element<'a, Message> {
  let body = Row::with_children(vec![
    text("\u{26a0}")
      .font(typography::mono::REGULAR)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::status::DANGER),
      })
      .into(),
    text(warning.message())
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center);

  container(body)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_2_5,
      right: spacing::SPACE_3,
      bottom: spacing::SPACE_2_5,
      left: spacing::SPACE_3,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::with_alpha(color::status::DANGER, 0.10))),
      border: Border {
        color: color::with_alpha(color::status::DANGER, 0.35),
        width: 1.0,
        radius: radius::CONTROL.into(),
      },
      ..container::Style::default()
    })
    .into()
}
