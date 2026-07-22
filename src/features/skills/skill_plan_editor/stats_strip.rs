use chrono::{DateTime, Utc};
use iced::{
  Background, Element, Length, Padding,
  alignment::Vertical,
  widget::{Space, column, container, row, text},
};

use super::{Message, fmt_duration, fmt_eta, fmt_sp};
use crate::ui::{
  components::eyebrow::eyebrow,
  style::{color, spacing, typography},
};

pub(super) fn stats_strip<'a>(
  steps: usize,
  total_sp: u64,
  total_sec: f64,
  is_template: bool,
  now: DateTime<Utc>,
) -> Element<'a, Message> {
  let total_secs = if total_sec.is_finite() {
    total_sec.clamp(0.0, i64::MAX as f64) as i64
  } else {
    0
  };

  let mut cells = vec![
    cell(&t!("skills.editor_stats.steps"), &steps.to_string()),
    Space::new().width(spacing::SPACE_6).into(),
    cell(&t!("skills.editor_stats.total_sp"), &fmt_sp(total_sp)),
  ];
  if !is_template {
    cells.push(Space::new().width(spacing::SPACE_6).into());
    cells.push(cell(
      &t!("skills.editor_stats.training_time"),
      &fmt_duration(total_secs),
    ));
    cells.push(Space::new().width(spacing::SPACE_6).into());
    cells.push(cell(&t!("skills.editor_stats.completes"), &fmt_eta(now, total_secs)));
  }

  container(row(cells).align_y(Vertical::Center).padding(Padding {
    top: spacing::SPACE_3,
    bottom: spacing::SPACE_3,
    left: spacing::SPACE_3_5,
    right: spacing::SPACE_3_5,
  }))
  .width(Length::Fill)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    ..container::Style::default()
  })
  .into()
}

fn cell<'a>(label: &str, value: &str) -> Element<'a, Message> {
  column(vec![
    eyebrow(label, None),
    Space::new().height(2.0).into(),
    text(value.to_owned())
      .font(typography::mono::MEDIUM)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
  ])
  .into()
}
