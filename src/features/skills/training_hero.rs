mod active;
mod idle;
pub(super) mod pip_row;
mod queue_item;
pub(super) mod right_col;

use chrono::{DateTime, Utc};
use iced::{
  Background, Border, Element, Length, Padding,
  widget::{Column, container},
};

use super::{
  Message,
  queue::{ComputedQueue, ComputedQueueItem},
};
use crate::{
  store::model::CharacterSkillqueue,
  ui::{
    components::progress_bar::progress_bar,
    style::{color, radius, spacing},
  },
};

const HERO_PROGRESS_HEIGHT: f32 = 3.0;

const HERO_SIDE_MARGIN: f32 = 28.0;

pub fn training_hero<'a>(
  computed: &'a ComputedQueue,
  head: Option<&'a CharacterSkillqueue>,
  now: DateTime<Utc>,
) -> Element<'a, Message> {
  let item = computed.items.first();
  let paused = head.is_none_or(|entry| entry.start_date().is_none() || entry.finish_date().is_none());

  match (item, paused) {
    (Some(item), false) => active::active(item, computed.sp_rate, now),
    _ => idle::idle(),
  }
}

#[allow(dead_code)]
pub fn queue_item<'a>(item: &'a ComputedQueueItem, sp_rate: f64, now: DateTime<Utc>) -> Element<'a, Message> {
  queue_item::queue_item(item, sp_rate, now)
}

fn hero_card<'a>(body: Element<'a, Message>, progress: Option<f32>) -> Element<'a, Message> {
  let inner = container(body).width(Length::Fill).padding(Padding {
    top: spacing::SPACE_6,
    right: HERO_SIDE_MARGIN,
    bottom: spacing::SPACE_6,
    left: HERO_SIDE_MARGIN,
  });

  let mut children: Vec<Element<'a, Message>> = Vec::with_capacity(2);
  if let Some(progress) = progress {
    children.push(progress_bar(progress, color::accent::PLASMA, HERO_PROGRESS_HEIGHT));
  }
  children.push(inner.into());

  let card = container(Column::with_children(children).width(Length::Fill))
    .width(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: radius::CARD.into(),
      },
      ..container::Style::default()
    });

  container(card)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_6,
      right: HERO_SIDE_MARGIN,
      bottom: 0.0,
      left: HERO_SIDE_MARGIN,
    })
    .into()
}
