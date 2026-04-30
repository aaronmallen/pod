//! Single queue entry row component (with gutter).

use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Space, column, container, row, text},
};
use pod_model::AttrKey;

use super::super::{
  ComputedQueueItem, Message, fmt_sp,
  training_hero::pip_row::{pip_ladder, roman},
};
use crate::{
  components,
  format::{fmt_dur_short, fmt_eta},
  style::{
    color, spacing,
    typography::{body, mono},
  },
};

pub struct Component {
  entry: ComputedQueueItem,
  index: usize,
}

impl Component {
  pub fn new(entry: ComputedQueueItem, index: usize) -> Self {
    Self {
      entry,
      index,
    }
  }

  pub fn render(self) -> Element<'static, Message> {
    let entry = self.entry;
    let offset_label = if entry.cum_start_secs < 60.0 {
      "Next".to_string()
    } else {
      format!("+{}", fmt_dur_short(entry.cum_start_secs as u64))
    };

    let cum_end_secs = (entry.cum_start_secs + entry.duration_secs) as u64;
    let gutter = queue_gutter(
      offset_label,
      fmt_eta(cum_end_secs),
      color::surface::SUNKEN,
      color::border::DEFAULT,
    );
    let skill_col = skill_col(&entry);
    let sp_col = sp_col(entry.sp_needed);
    let dur_col = dur_col(entry.duration_secs);

    let row_content = row([gutter, skill_col, sp_col, dur_col]).align_y(Vertical::Center);
    let row_container = container(row_content).width(Length::Fill);

    if self.index == 0 {
      row_container.into()
    } else {
      column([components::Separator::horizontal().render(), row_container.into()]).into()
    }
  }
}

fn queue_gutter(
  offset_label: String,
  eta_label: String,
  dot_color: Color,
  dot_border: Color,
) -> Element<'static, Message> {
  let timeline_dot = container(Space::new().width(7.0).height(7.0))
    .width(7.0)
    .height(7.0)
    .style(move |_| container::Style {
      background: Some(Background::Color(dot_color)),
      border: Border {
        color: dot_border,
        radius: 3.5.into(),
        width: 1.0,
      },
      ..container::Style::default()
    });

  container(column([
    text(offset_label)
      .font(mono::REGULAR)
      .size(9.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::TERTIARY),
      })
      .into(),
    Space::new().height(2.0).into(),
    text(eta_label)
      .font(mono::REGULAR)
      .size(11.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    Space::new().height(spacing::SPACE_2).into(),
    timeline_dot.into(),
  ]))
  .width(Length::Fixed(90.0))
  .padding(Padding {
    top: 10.0,
    bottom: 10.0,
    left: 14.0,
    right: 14.0,
  })
  .into()
}

fn skill_col(entry: &ComputedQueueItem) -> Element<'static, Message> {
  container(column([
    skill_name_row(entry),
    Space::new().height(8.0).into(),
    skill_pip_row(entry),
  ]))
  .width(Length::Fill)
  .padding(Padding {
    top: 12.0,
    bottom: 12.0,
    left: 0.0,
    right: 14.0,
  })
  .into()
}

fn skill_name_row(entry: &ComputedQueueItem) -> Element<'static, Message> {
  let display_name = if entry.skill_name.is_empty() {
    "Unknown skill".to_string()
  } else {
    entry.skill_name.clone()
  };

  let mut items: Vec<Element<'_, Message>> = vec![
    text(display_name)
      .font(body::MEDIUM)
      .size(15.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    Space::new().width(spacing::SPACE_3).into(),
    text(roman(entry.to_level))
      .font(mono::MEDIUM)
      .size(14.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
  ];

  if entry.rank > 0 {
    items.push(Space::new().width(spacing::SPACE_2).into());
    items.push(rank_badge(entry.rank));
  }

  if !entry.group_name.is_empty() {
    items.push(Space::new().width(spacing::SPACE_2).into());
    items.push(group_label(entry.group_name.clone()));
  }

  row(items).align_y(Vertical::Center).into()
}

fn rank_badge(rank: u8) -> Element<'static, Message> {
  container(
    text(format!("×{rank}"))
      .font(mono::REGULAR)
      .size(9.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .padding(Padding {
    top: 2.0,
    bottom: 2.0,
    left: 6.0,
    right: 6.0,
  })
  .style(|_| container::Style {
    border: Border {
      color: color::border::SUBTLE,
      radius: 3.0.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn group_label(group_name: String) -> Element<'static, Message> {
  text(group_name)
    .font(mono::REGULAR)
    .size(9.0)
    .style(|_| iced::widget::text::Style {
      color: Some(color::text::TERTIARY),
    })
    .into()
}

fn skill_pip_row(entry: &ComputedQueueItem) -> Element<'static, Message> {
  row([
    pip_ladder(entry.from_level, entry.to_level),
    Space::new().width(spacing::SPACE_2).into(),
    attr_chip(entry.primary, true),
    Space::new().width(6.0).into(),
    attr_chip(entry.secondary, false),
  ])
  .align_y(Vertical::Center)
  .into()
}

fn attr_chip<'a>(key: AttrKey, primary: bool) -> Element<'a, Message> {
  use super::super::training_hero::right_col::attr_chip as right_col_attr_chip;
  right_col_attr_chip(key, primary)
}

fn sp_col(sp_needed: u64) -> Element<'static, Message> {
  container(
    column([
      text(fmt_sp(sp_needed))
        .font(mono::MEDIUM)
        .size(13.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::PRIMARY),
        })
        .into(),
      text("SP")
        .font(mono::REGULAR)
        .size(9.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::TERTIARY),
        })
        .into(),
    ])
    .align_x(Horizontal::Right)
    .spacing(2.0),
  )
  .width(Length::Fixed(80.0))
  .align_y(Vertical::Center)
  .align_x(Horizontal::Right)
  .into()
}

fn dur_col(duration_secs: f32) -> Element<'static, Message> {
  container(
    text(fmt_dur_short(duration_secs as u64))
      .font(mono::MEDIUM)
      .size(13.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      }),
  )
  .width(Length::Fixed(110.0))
  .align_y(Vertical::Center)
  .align_x(Horizontal::Right)
  .padding(Padding {
    top: 0.0,
    bottom: 0.0,
    left: 0.0,
    right: 14.0,
  })
  .into()
}
