mod active;
mod idle;
pub(super) mod pip_row;
mod queue_item;
pub(super) mod right_col;

use chrono::{DateTime, Utc};
use iced::{
  Background, Border, Element, Length, Padding,
  border::Radius,
  widget::{Column, Row, container, mouse_area, text},
};

use super::{
  Message,
  queue::{ComputedQueue, ComputedQueueItem, QueueSelection, QueueStatus, queue_status},
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

const HERO_SELECTED_BAR_WIDTH: f32 = 3.0;

pub fn training_hero<'a>(
  computed: &'a ComputedQueue,
  head: Option<&'a CharacterSkillqueue>,
  queued_count: usize,
  selection: &QueueSelection,
  now: DateTime<Utc>,
) -> Element<'a, Message> {
  let status = queue_status(head, queued_count, now);

  match (computed.items.first(), status) {
    (Some(item), QueueStatus::Training) => {
      let selected = selection.contains(item.queue_position);
      active::active(item, computed.sp_rate, selected, now)
    }
    (_, status) => idle::idle(status),
  }
}

// Built-but-not-yet-wired queue hero card (sibling of active/idle); keeps the queue_item module reachable until wired.
#[expect(
  dead_code,
  reason = "Built-but-not-yet-wired queue hero card; awaiting the queue UI to render it."
)]
pub fn queue_item<'a>(item: &'a ComputedQueueItem, sp_rate: f64, now: DateTime<Utc>) -> Element<'a, Message> {
  queue_item::queue_item(item, sp_rate, now)
}

fn hero_card<'a>(
  body: Element<'a, Message>,
  progress: Option<f32>,
  selected: bool,
  press_position: Option<i64>,
) -> Element<'a, Message> {
  let inner = container(body).width(Length::Fill).padding(Padding {
    top: spacing::SPACE_6,
    right: HERO_SIDE_MARGIN,
    bottom: spacing::SPACE_6,
    left: HERO_SIDE_MARGIN,
  });

  let framed = Row::with_children(vec![selection_bar(selected), inner.into()]).width(Length::Fill);

  let mut children: Vec<Element<'a, Message>> = Vec::with_capacity(2);
  if let Some(progress) = progress {
    children.push(progress_bar(progress, color::accent(), HERO_PROGRESS_HEIGHT));
  }
  children.push(framed.into());

  let card = container(Column::with_children(children).width(Length::Fill))
    .width(Length::Fill)
    .style(move |_| card_style(selected));

  let card: Element<'a, Message> = match press_position {
    Some(position) => mouse_area(card).on_press(Message::QueueRowClicked(position)).into(),
    None => card.into(),
  };

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

fn card_style(selected: bool) -> container::Style {
  let (background, border_color) = if selected {
    (color::with_alpha(color::accent(), 0.1), color::accent())
  } else {
    (color::surface::RAISED, color::rule())
  };
  container::Style {
    background: Some(Background::Color(background)),
    border: Border {
      color: border_color,
      width: 1.0,
      radius: radius::CARD.into(),
    },
    ..container::Style::default()
  }
}

fn selection_bar<'a>(selected: bool) -> Element<'a, Message> {
  let fill = if selected {
    color::accent()
  } else {
    iced::Color::TRANSPARENT
  };
  container(text(""))
    .width(Length::Fixed(HERO_SELECTED_BAR_WIDTH))
    .height(Length::Fill)
    .style(move |_| container::Style {
      background: Some(Background::Color(fill)),
      border: Border {
        radius: Radius {
          top_left: 0.0,
          top_right: HERO_SELECTED_BAR_WIDTH,
          bottom_right: HERO_SELECTED_BAR_WIDTH,
          bottom_left: 0.0,
        },
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into()
}

#[cfg(test)]
mod tests {
  use chrono::TimeZone as _;

  use super::*;
  use crate::features::skills::queue::{Attr, ClickKind};

  fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 6, 1, 12, 0, 0).unwrap()
  }

  fn head_item() -> ComputedQueueItem {
    ComputedQueueItem {
      cum_start_secs: 0.0,
      duration_secs: 3_600.0,
      from_level: 0,
      group_name: "Gunnery".to_owned(),
      primary: Attr::Perception,
      progress: 0.25,
      queue_position: 0,
      rank: 1,
      secondary: Attr::Willpower,
      skill_name: "Small Hybrid Turret".to_owned(),
      sp_needed: 250,
      sp_now: 0,
      sp_to: 250,
      to_level: 3,
    }
  }

  fn training_computed() -> ComputedQueue {
    ComputedQueue {
      items: vec![head_item()],
      sp_rate: 1.0,
      total_secs: 3_600.0,
      total_sp: 0,
    }
  }

  fn training_head() -> CharacterSkillqueue {
    CharacterSkillqueue {
      character_id: 42,
      finish_date: Some("2026-06-11T00:00:00Z".to_owned()),
      finished_level: 3,
      level_end_sp: None,
      level_start_sp: None,
      queue_position: 0,
      skill_id: 100,
      start_date: Some("2026-06-01T00:00:00Z".to_owned()),
      training_start_sp: None,
    }
  }

  #[test]
  fn it_builds_the_active_hero_when_the_head_is_selected() {
    let computed = training_computed();
    let head = training_head();
    let mut selection = QueueSelection::default();
    selection.apply(0, ClickKind::Plain, &[0]);

    let _el: Element<'_, Message> = training_hero(&computed, Some(&head), 1, &selection, now());
  }

  #[test]
  fn it_builds_the_active_hero_when_the_head_is_not_selected() {
    let computed = training_computed();
    let head = training_head();
    let selection = QueueSelection::default();

    let _el: Element<'_, Message> = training_hero(&computed, Some(&head), 1, &selection, now());
  }
}
