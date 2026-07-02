use chrono::{DateTime, Utc};
use iced::{
  Element, Length,
  alignment::Vertical,
  widget::{Column, Row, text},
};

use super::{
  super::{Message, fmt_duration, queue::ComputedQueueItem, queue_timing::roman},
  hero_card,
  pip_row::{level_transition, pip_ladder},
  right_col::{pulse_dot, rank_badge, right_col},
};
use crate::ui::{
  components::eyebrow::{eyebrow, eyebrow_text},
  style::{color, spacing, typography},
};

const HERO_SIDE_MARGIN: f32 = 28.0;

pub(super) fn active<'a>(item: &'a ComputedQueueItem, sp_rate: f64, now: DateTime<Utc>) -> Element<'a, Message> {
  let remain_secs = item.duration_secs.round() as i64;
  let pct = (item.progress * 100.0).round() as i64;

  let header = Row::with_children(vec![
    eyebrow(&t!("skills.hero_active.currently_training"), Some(color::accent())),
    pulse_dot(),
    eyebrow_text(&item.group_name, Some(color::text::secondary())).into(),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center);

  let title = Row::with_children(vec![
    text(item.skill_name.clone())
      .font(typography::body::MEDIUM)
      .size(32)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    text(roman(i64::from(item.to_level)))
      .font(typography::mono::MEDIUM)
      .size(22)
      .style(|_| text::Style {
        color: Some(color::accent()),
      })
      .into(),
    rank_badge(item.rank),
  ])
  .spacing(spacing::SPACE_3_5)
  .align_y(Vertical::Bottom);

  let ladder = Row::with_children(vec![
    pip_ladder(item.from_level, item.to_level),
    level_transition(item.from_level, item.to_level),
  ])
  .spacing(spacing::SPACE_3_5)
  .align_y(Vertical::Center);

  let remaining = Row::with_children(vec![
    text(fmt_duration(remain_secs))
      .font(typography::mono::MEDIUM)
      .size(28)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    text(t!("skills.hero_active.remaining"))
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      })
      .into(),
  ])
  .spacing(spacing::UNIT)
  .align_y(Vertical::Bottom);

  let left = Column::with_children(vec![header.into(), title.into(), ladder.into(), remaining.into()])
    .spacing(spacing::SPACE_2)
    .width(Length::Fill);

  let right = right_col(
    pct,
    item.sp_now,
    item.sp_to,
    item.primary,
    item.secondary,
    sp_rate,
    remain_secs,
    now,
  );

  let body = Row::with_children(vec![left.into(), right])
    .spacing(HERO_SIDE_MARGIN)
    .align_y(Vertical::Top);

  hero_card(body.into(), Some(item.progress))
}
