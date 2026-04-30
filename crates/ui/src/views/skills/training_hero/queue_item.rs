//! Queue-driven training hero component.

use iced::{
  Background, Border, Element, Length, Padding,
  alignment::Vertical,
  widget::{Space, column, container, row, text},
};

use super::{
  super::{ComputedQueueItem, Message},
  pip_row::{Component as PipRow, roman},
  progress_bar::Component as ProgressBar,
  right_col::Component as RightCol,
};
use crate::{
  format::fmt_dur,
  style::{
    color, spacing,
    typography::{body, mono},
  },
};

pub struct Component {
  entry: ComputedQueueItem,
  sp_rate: f32,
}

impl Component {
  pub fn new(entry: ComputedQueueItem, sp_rate: f32) -> Self {
    Self {
      entry,
      sp_rate,
    }
  }

  pub fn render(self) -> Element<'static, Message> {
    let entry = self.entry;
    let pct = (entry.progress * 100.0).round() as u32;
    let sp_min = (self.sp_rate * 60.0).round() as u64;
    let sp_day_k = ((self.sp_rate * 86400.0) as f64 / 1000.0) as u64;

    let left_col = column([
      hero_header_row(entry.group_name.to_uppercase()),
      Space::new().height(8.0).into(),
      hero_skill_name_row(entry.skill_name.clone(), entry.to_level, entry.rank),
      Space::new().height(16.0).into(),
      PipRow::new(entry.from_level, entry.to_level).render(),
      Space::new().height(18.0).into(),
      hero_remain_row(entry.duration_secs as u64),
    ]);

    let right_col = RightCol::new(
      pct,
      entry.sp_now,
      entry.sp_to,
      entry.primary,
      entry.secondary,
      sp_min,
      sp_day_k,
      entry.duration_secs as u64,
    )
    .render();

    let content_row = row([
      left_col.width(Length::Fill).into(),
      Space::new().width(spacing::SPACE_7).into(),
      right_col,
    ]);

    column([
      ProgressBar::new(entry.progress).render(),
      container(content_row)
        .padding(Padding {
          top: 22.0,
          bottom: 24.0,
          left: spacing::SPACE_7,
          right: spacing::SPACE_7,
        })
        .into(),
    ])
    .into()
  }
}

fn hero_header_row(group_name: String) -> Element<'static, Message> {
  row([
    text("Currently training")
      .font(mono::REGULAR)
      .size(9.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::accent::PLASMA),
      })
      .into(),
    Space::new().width(6.0).into(),
    container(Space::new().width(6.0).height(6.0))
      .width(6.0)
      .height(6.0)
      .style(|_| container::Style {
        background: Some(Background::Color(color::accent::PLASMA)),
        border: Border {
          radius: 3.0.into(),
          ..Border::default()
        },
        ..container::Style::default()
      })
      .into(),
    Space::new().width(6.0).into(),
    text(group_name)
      .font(mono::REGULAR)
      .size(9.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
  ])
  .align_y(Vertical::Center)
  .into()
}

fn hero_skill_name_row(name: String, to_level: u8, rank: u8) -> Element<'static, Message> {
  row([
    text(name)
      .font(body::MEDIUM)
      .size(32.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    Space::new().width(14.0).into(),
    text(roman(to_level))
      .font(mono::MEDIUM)
      .size(22.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::accent::PLASMA),
      })
      .into(),
    Space::new().width(spacing::SPACE_2).into(),
    container(
      text(format!("×{}", rank))
        .font(mono::REGULAR)
        .size(11.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        }),
    )
    .padding(Padding {
      top: 3.0,
      bottom: 3.0,
      left: 8.0,
      right: 8.0,
    })
    .style(|_| container::Style {
      border: Border {
        color: color::border::SUBTLE,
        radius: 4.0.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into(),
  ])
  .align_y(Vertical::Bottom)
  .into()
}

fn hero_remain_row(remaining_secs: u64) -> Element<'static, Message> {
  row([
    text(fmt_dur(remaining_secs))
      .font(mono::MEDIUM)
      .size(28.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    Space::new().width(spacing::SPACE_2).into(),
    text("remaining")
      .font(mono::REGULAR)
      .size(11.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
  ])
  .align_y(Vertical::Bottom)
  .into()
}
