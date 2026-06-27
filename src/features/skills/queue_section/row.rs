use chrono::{DateTime, Utc};
use iced::{
  Background, Border, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  border::Radius,
  widget::{Column, Row, container, mouse_area, text},
};

use super::super::{
  Message, fmt_eta, fmt_sp,
  format::fmt_dur_short,
  queue::ComputedQueueItem,
  queue_timing::roman,
  training_hero::{
    pip_row::pip_ladder,
    right_col::{attr_chip, rank_badge},
  },
};
use crate::ui::{
  components::{
    eyebrow::{eyebrow, eyebrow_text},
    rule,
  },
  style::{color, spacing, typography},
};

const COMPLETES_WIDTH: f32 = 135.0;
const DURATION_WIDTH: f32 = 110.0;
const SELECTED_BAR_WIDTH: f32 = 3.0;
const SP_WIDTH: f32 = 80.0;

pub(super) fn row<'a>(
  item: &'a ComputedQueueItem,
  display_index: usize,
  selected: bool,
  now: DateTime<Utc>,
) -> Element<'a, Message> {
  let row_secs = item.duration_secs.round() as i64;
  let cum_end_secs = (item.cum_start_secs + item.duration_secs).round() as i64;

  let inner = Row::with_children(vec![
    completes_col(item, display_index, cum_end_secs, now),
    skill_col(item),
    sp_col(item.sp_needed),
    duration_col(row_secs),
  ])
  .spacing(spacing::SPACE_3)
  .align_y(Vertical::Center)
  .width(Length::Fill);

  let body = container(inner)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_2_5,
      right: spacing::SPACE_3,
      bottom: spacing::SPACE_2_5,
      left: spacing::SPACE_3,
    })
    .align_y(Vertical::Center);

  let framed = container(Row::with_children(vec![selected_bar(selected), body.into()]).width(Length::Fill))
    .width(Length::Fill)
    .style(move |_| selected_fill(selected));

  let pressable: Element<'a, Message> = mouse_area(framed)
    .on_press(Message::QueueRowClicked(item.queue_position))
    .into();

  if display_index == 0 {
    return pressable;
  }
  Column::with_children(vec![rule::horizontal(), pressable])
    .width(Length::Fill)
    .into()
}

fn selected_bar<'a>(selected: bool) -> Element<'a, Message> {
  let color = if selected {
    color::accent::PLASMA
  } else {
    iced::Color::TRANSPARENT
  };
  container(text(""))
    .width(Length::Fixed(SELECTED_BAR_WIDTH))
    .height(Length::Fill)
    .style(move |_| container::Style {
      background: Some(Background::Color(color)),
      border: Border {
        radius: Radius {
          top_left: 0.0,
          top_right: SELECTED_BAR_WIDTH,
          bottom_right: SELECTED_BAR_WIDTH,
          bottom_left: 0.0,
        },
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into()
}

fn selected_fill(selected: bool) -> container::Style {
  if !selected {
    return container::Style::default();
  }
  container::Style {
    background: Some(Background::Color(color::with_alpha(color::accent::PLASMA, 0.1))),
    ..container::Style::default()
  }
}

fn completes_col<'a>(
  item: &ComputedQueueItem,
  display_index: usize,
  cum_end_secs: i64,
  now: DateTime<Utc>,
) -> Element<'a, Message> {
  let offset_label = if display_index == 0 {
    t!("skills.queue.next").into_owned()
  } else {
    format!("+{}", fmt_dur_short(item.cum_start_secs.round() as i64))
  };

  container(Column::with_children(vec![
    eyebrow(&offset_label, Some(color::text::tertiary())),
    text(fmt_eta(now, cum_end_secs))
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
  ]))
  .width(Length::Fixed(COMPLETES_WIDTH))
  .into()
}

fn skill_col<'a>(item: &'a ComputedQueueItem) -> Element<'a, Message> {
  let display_name = if item.skill_name.is_empty() {
    t!("skills.queue.unknown_skill").into_owned()
  } else {
    item.skill_name.clone()
  };

  let mut title_children: Vec<Element<'a, Message>> = vec![
    text(display_name)
      .font(typography::body::MEDIUM)
      .size(typography::size::LG)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    text(roman(i64::from(item.to_level)))
      .font(typography::mono::MEDIUM)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
  ];
  if item.rank > 0 {
    title_children.push(rank_badge(item.rank));
  }
  if !item.group_name.is_empty() {
    title_children.push(eyebrow_text(&item.group_name, Some(color::text::tertiary())).into());
  }
  let title = Row::with_children(title_children)
    .spacing(spacing::SPACE_2_5)
    .align_y(Vertical::Center);

  let ladder = Row::with_children(vec![
    pip_ladder(item.from_level, item.to_level),
    attr_chip(item.primary, true),
    attr_chip(item.secondary, false),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center);

  Column::with_children(vec![title.into(), ladder.into()])
    .spacing(spacing::SPACE_2)
    .width(Length::Fill)
    .into()
}

fn sp_col<'a>(sp_needed: u64) -> Element<'a, Message> {
  container(
    Column::with_children(vec![
      text(fmt_sp(sp_needed as i64))
        .font(typography::mono::MEDIUM)
        .size(typography::size::MD)
        .style(|_| text::Style {
          color: Some(color::text::PRIMARY),
        })
        .into(),
      text(t!("skills.queue.sp"))
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(|_| text::Style {
          color: Some(color::text::tertiary()),
        })
        .into(),
    ])
    .spacing(spacing::UNIT)
    .align_x(Horizontal::Right),
  )
  .width(Length::Fixed(SP_WIDTH))
  .align_x(Horizontal::Right)
  .into()
}

fn duration_col<'a>(row_secs: i64) -> Element<'a, Message> {
  container(
    text(fmt_dur_short(row_secs))
      .font(typography::mono::MEDIUM)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      }),
  )
  .width(Length::Fixed(DURATION_WIDTH))
  .align_x(Horizontal::Right)
  .into()
}
