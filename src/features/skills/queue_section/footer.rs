use chrono::{DateTime, Utc};
use iced::{
  Background, Border, Element, Length, Padding,
  alignment::Vertical,
  border::Radius,
  widget::{Column, Row, Space, container, text},
};

use super::super::{Message, fmt_duration, fmt_eta};
use crate::ui::{
  components::{eyebrow::eyebrow, rule},
  style::{color, radius, spacing, typography},
};

pub(super) fn footer<'a>(total_n: usize, total_secs: f64, now: DateTime<Utc>) -> Element<'a, Message> {
  let total_secs = total_secs.round() as i64;

  let body = Row::with_children(vec![
    eyebrow(&format!("Total \u{b7} {total_n} skills"), Some(color::text::SECONDARY)),
    Space::new().width(Length::Fill).into(),
    text(fmt_duration(total_secs))
      .font(typography::mono::MEDIUM)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    text(format!("finishes {} EVE", fmt_eta(now, total_secs)))
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
  ])
  .spacing(spacing::SPACE_3)
  .align_y(Vertical::Center);

  let bar = container(body)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_3_5,
      right: spacing::SPACE_3,
      bottom: spacing::SPACE_3_5,
      left: spacing::SPACE_3,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: Border {
        radius: Radius {
          top_left: 0.0,
          top_right: 0.0,
          bottom_right: radius::CONTROL,
          bottom_left: radius::CONTROL,
        },
        ..Border::default()
      },
      ..container::Style::default()
    });

  Column::with_children(vec![rule::horizontal(), bar.into()])
    .width(Length::Fill)
    .into()
}
