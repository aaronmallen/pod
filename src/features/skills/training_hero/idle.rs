use iced::{
  Background, Border, Element, Length,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, container, text},
};

use super::{
  super::{
    Message,
    queue::{QueueStatus, skill_word},
  },
  hero_card,
};
use crate::ui::{
  components::eyebrow::eyebrow,
  style::{color, spacing, typography},
};

const HERO_ICON: f32 = 48.0;

pub(super) fn idle<'a>(status: QueueStatus) -> Element<'a, Message> {
  let (eyebrow_text, headline) = match status {
    QueueStatus::Paused {
      queued,
    } => (
      t!("skills.hero_idle.paused_eyebrow", count => queued, noun => skill_word(queued)).into_owned(),
      t!("skills.hero_idle.paused_headline").into_owned(),
    ),
    _ => (
      t!("skills.hero_idle.inactive_eyebrow").into_owned(),
      t!("skills.hero_idle.inactive_headline").into_owned(),
    ),
  };

  let supporting = match status {
    QueueStatus::Paused {
      ..
    } => t!("skills.hero_idle.paused_supporting"),
    _ => t!("skills.hero_idle.inactive_supporting"),
  };

  let icon = container(
    text("\u{26a0}")
      .font(typography::mono::REGULAR)
      .size(typography::size::LG)
      .style(|_| text::Style {
        color: Some(color::status::DANGER),
      }),
  )
  .width(Length::Fixed(HERO_ICON))
  .height(Length::Fixed(HERO_ICON))
  .align_x(Horizontal::Center)
  .align_y(Vertical::Center)
  .style(|_| container::Style {
    background: Some(Background::Color(color::with_alpha(color::status::DANGER, 0.10))),
    border: Border {
      color: color::with_alpha(color::status::DANGER, 0.35),
      width: 1.0,
      radius: (HERO_ICON / 2.0).into(),
    },
    ..container::Style::default()
  });

  let copy = Column::with_children(vec![
    eyebrow(&eyebrow_text, Some(color::status::DANGER)),
    text(headline)
      .font(typography::body::MEDIUM)
      .size(typography::size::LG)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    text(supporting)
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      })
      .into(),
  ])
  .spacing(spacing::UNIT)
  .width(Length::Fill);

  let body = Row::with_children(vec![icon.into(), copy.into()])
    .spacing(spacing::SPACE_6)
    .align_y(Vertical::Center);

  hero_card(body.into(), None, false, None)
}
